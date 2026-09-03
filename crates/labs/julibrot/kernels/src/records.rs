// CPU mirrors intentionally reproduce WGSL's fixed-width conversions and written operation order.
#![allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]

use bytemuck::{Pod, Zeroable};
use ember_julibrot_math::{CentreSplit, Homography, Plane, ScaleSplit};
use ember_lab_heap::DataSpan;

/// Nonzero pixel extent of one active refinement grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct GridExtent {
    pub width: u32,
    pub height: u32,
}

/// Closed refinement-level ABI shared with presentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RefinementLevel {
    Preview = 0,
    Interactive = 1,
    Final = 2,
}

/// Closed kernel-selection ABI displayed by the app.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KernelMode {
    Shallow = 0,
    Perturbation = 1,
}

/// Exact terminal status stored in the fourth escape-grid lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SampleStatus {
    Sampled = 0,
    Glitch = 1,
    Horizon = 2,
    MapUncertain = 3,
}

impl SampleStatus {
    #[must_use]
    pub const fn as_f32(self) -> f32 {
        match self {
            Self::Sampled => 0.0,
            Self::Glitch => 1.0,
            Self::Horizon => 2.0,
            Self::MapUncertain => 3.0,
        }
    }

    #[must_use]
    pub const fn from_f32(value: f32) -> Option<Self> {
        match value.to_bits() {
            bits if bits == 0.0_f32.to_bits() => Some(Self::Sampled),
            bits if bits == 1.0_f32.to_bits() => Some(Self::Glitch),
            bits if bits == 2.0_f32.to_bits() => Some(Self::Horizon),
            bits if bits == 3.0_f32.to_bits() => Some(Self::MapUncertain),
            _ => None,
        }
    }
}

impl KernelMode {
    /// Selects the displayed shallow/deep policy.
    #[must_use]
    pub const fn for_zoom(zoom_log2: f64) -> Self {
        if zoom_log2 >= 14.0 {
            Self::Perturbation
        } else {
            Self::Shallow
        }
    }
}

/// Returns the mapped plane-relative sample offset for one row-major pixel index.
pub fn pixel_offset(
    index: u32,
    extent: GridExtent,
    plane: Plane,
    screen_to_plane: [[f32; 4]; 3],
    pixel_scale: f32,
) -> Result<[f32; 4], SampleStatus> {
    let column = index % extent.width;
    let row = index / extent.width;
    let x = column as f32 + 0.5 - 0.5 * extent.width as f32;
    let y = row as f32 + 0.5 - 0.5 * extent.height as f32;
    let homogeneous = screen_to_plane.map(|map_row| map_row[0] * x + map_row[1] * y + map_row[2]);
    let denominator = homogeneous[2];
    if !denominator.is_finite() {
        return Err(SampleStatus::MapUncertain);
    }
    if denominator <= 0.0 {
        return Err(SampleStatus::Horizon);
    }

    // Eight unit roundoffs cover f64-to-f32 coefficient rounding, two products, two additions,
    // and the divide. The quotient bound is then tested in plane-pixel units.
    let error_factor = 4.0 * f32::EPSILON;
    let scales = screen_to_plane
        .map(|map_row| map_row[0].abs() * x.abs() + map_row[1].abs() * y.abs() + map_row[2].abs());
    let errors = scales.map(|scale| error_factor * scale);
    if denominator <= errors[2] {
        return Err(SampleStatus::MapUncertain);
    }
    let mapped = [homogeneous[0] / denominator, homogeneous[1] / denominator];
    let safe_denominator = denominator - errors[2];
    let quotient_errors = [
        (errors[0] + mapped[0].abs() * errors[2]) / safe_denominator,
        (errors[1] + mapped[1].abs() * errors[2]) / safe_denominator,
    ];
    if !mapped.iter().all(|value| value.is_finite())
        || quotient_errors[0] * quotient_errors[0] + quotient_errors[1] * quotient_errors[1]
            > 0.0625
    {
        return Err(SampleStatus::MapUncertain);
    }
    Ok(std::array::from_fn(|axis| {
        (mapped[0] * plane.basis_u[axis] + mapped[1] * plane.basis_v[axis]) * pixel_scale
    }))
}

#[allow(clippy::cast_possible_truncation)]
fn pack_screen_to_plane(map: &Homography) -> Result<[[f32; 4]; 3], crate::KernelError> {
    let mut rows = [[0.0; 4]; 3];
    for (destination, source) in rows.iter_mut().zip(map.rows.as_chunks::<3>().0) {
        for (packed, value) in destination[..3].iter_mut().zip(source) {
            *packed = *value as f32;
        }
    }
    rows.iter()
        .flatten()
        .all(|value| value.is_finite())
        .then_some(rows)
        .ok_or(crate::KernelError::InvalidMap)
}

/// One kernels-owned allocation whose initialized prefix is presented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EscapeGrid {
    pub span: DataSpan,
    pub width: u32,
    pub height: u32,
    pub level: RefinementLevel,
}

/// Borrowed, generation-tagged reference input for one deep dispatch.
#[derive(Clone, Copy, Debug)]
pub struct ReferenceOrbitInput<'a> {
    pub span: &'a DataSpan,
    pub generation: u32,
    pub length: u32,
    pub precision_bits: u32,
    /// Precision policy that produced the captured reference.
    pub precision_mode: &'static str,
}

/// Exact 144-byte shallow uniform payload.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct ShallowUniform {
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
    pub screen_to_plane_row_0: [f32; 4],
    pub screen_to_plane_row_1: [f32; 4],
    pub screen_to_plane_row_2: [f32; 4],
    pub centre_hi: [f32; 4],
    pub centre_lo: [f32; 4],
    pub pixel_scale: f32,
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    pub bailout: f32,
    pub level: u32,
    pub padding: [u32; 2],
}

impl ShallowUniform {
    pub(crate) const fn from_parts(
        plane: Plane,
        screen_to_plane: [[f32; 4]; 3],
        centre: CentreSplit,
        pixel_scale: f32,
        extent: GridExtent,
        max_iter: u32,
        level: RefinementLevel,
    ) -> Self {
        Self {
            basis_u: plane.basis_u,
            basis_v: plane.basis_v,
            screen_to_plane_row_0: screen_to_plane[0],
            screen_to_plane_row_1: screen_to_plane[1],
            screen_to_plane_row_2: screen_to_plane[2],
            centre_hi: centre.hi,
            centre_lo: centre.lo,
            pixel_scale,
            width: extent.width,
            height: extent.height,
            max_iter,
            bailout: ember_julibrot_math::EscapeParams::BAILOUT,
            level: level as u32,
            padding: [0; 2],
        }
    }

    /// Returns the exact little-endian wasm/native payload bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

/// Exact 112-byte scaled-perturbation uniform payload.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct PerturbUniform {
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
    pub screen_to_plane_row_0: [f32; 4],
    pub screen_to_plane_row_1: [f32; 4],
    pub screen_to_plane_row_2: [f32; 4],
    pub pixel_scale: f32,
    pub width: u32,
    pub height: u32,
    pub max_iter: u32,
    pub bailout: f32,
    pub orbit_length: u32,
    pub level: u32,
    pub scale_exponent: i32,
}

impl PerturbUniform {
    pub(crate) const fn from_parts(
        plane: Plane,
        screen_to_plane: [[f32; 4]; 3],
        scale: ScaleSplit,
        extent: GridExtent,
        max_iter: u32,
        orbit_length: u32,
        level: RefinementLevel,
    ) -> Self {
        Self {
            basis_u: plane.basis_u,
            basis_v: plane.basis_v,
            screen_to_plane_row_0: screen_to_plane[0],
            screen_to_plane_row_1: screen_to_plane[1],
            screen_to_plane_row_2: screen_to_plane[2],
            pixel_scale: scale.mantissa,
            width: extent.width,
            height: extent.height,
            max_iter,
            bailout: ember_julibrot_math::EscapeParams::BAILOUT,
            orbit_length,
            level: level as u32,
            scale_exponent: scale.exponent,
        }
    }

    /// Returns the exact little-endian wasm/native payload bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

pub fn pack_map_rows(map: &Homography) -> Result<[[f32; 4]; 3], crate::KernelError> {
    pack_screen_to_plane(map)
}

#[cfg(test)]
mod tests {
    use super::{
        GridExtent, KernelMode, PerturbUniform, RefinementLevel, SampleStatus, ShallowUniform,
    };
    use ember_julibrot_math::{
        CentreSplit, EscapeGridRecord, EscapeParams, Homography, Plane, ReferenceOrbitRecord,
        ScaleSplit,
    };
    use std::mem::{align_of, offset_of, size_of};

    const PLANE: Plane = Plane {
        basis_u: [0.6, 0.0, 0.8, 0.0],
        basis_v: [0.0, 0.8, 0.0, 0.6],
    };

    #[test]
    fn gpu_uniform_layouts_are_exact() {
        assert_eq!(size_of::<Plane>(), 32);
        assert_eq!(align_of::<Plane>(), 16);
        assert_eq!(size_of::<CentreSplit>(), 32);
        assert_eq!(align_of::<CentreSplit>(), 16);
        assert_eq!(size_of::<EscapeParams>(), 8);
        assert_eq!(size_of::<ReferenceOrbitRecord>(), 8);
        assert_eq!(size_of::<EscapeGridRecord>(), 16);
        assert_eq!(size_of::<ShallowUniform>(), 144);
        assert_eq!(align_of::<ShallowUniform>(), 16);
        assert_eq!(offset_of!(ShallowUniform, basis_u), 0);
        assert_eq!(offset_of!(ShallowUniform, basis_v), 16);
        assert_eq!(offset_of!(ShallowUniform, screen_to_plane_row_0), 32);
        assert_eq!(offset_of!(ShallowUniform, screen_to_plane_row_1), 48);
        assert_eq!(offset_of!(ShallowUniform, screen_to_plane_row_2), 64);
        assert_eq!(offset_of!(ShallowUniform, centre_hi), 80);
        assert_eq!(offset_of!(ShallowUniform, centre_lo), 96);
        assert_eq!(offset_of!(ShallowUniform, pixel_scale), 112);
        assert_eq!(offset_of!(ShallowUniform, width), 116);
        assert_eq!(offset_of!(ShallowUniform, height), 120);
        assert_eq!(offset_of!(ShallowUniform, max_iter), 124);
        assert_eq!(offset_of!(ShallowUniform, bailout), 128);
        assert_eq!(offset_of!(ShallowUniform, level), 132);
        assert_eq!(offset_of!(ShallowUniform, padding), 136);
        assert_eq!(size_of::<PerturbUniform>(), 112);
        assert_eq!(align_of::<PerturbUniform>(), 16);
        assert_eq!(offset_of!(PerturbUniform, basis_u), 0);
        assert_eq!(offset_of!(PerturbUniform, basis_v), 16);
        assert_eq!(offset_of!(PerturbUniform, screen_to_plane_row_0), 32);
        assert_eq!(offset_of!(PerturbUniform, screen_to_plane_row_1), 48);
        assert_eq!(offset_of!(PerturbUniform, screen_to_plane_row_2), 64);
        assert_eq!(offset_of!(PerturbUniform, pixel_scale), 80);
        assert_eq!(offset_of!(PerturbUniform, width), 84);
        assert_eq!(offset_of!(PerturbUniform, height), 88);
        assert_eq!(offset_of!(PerturbUniform, max_iter), 92);
        assert_eq!(offset_of!(PerturbUniform, bailout), 96);
        assert_eq!(offset_of!(PerturbUniform, orbit_length), 100);
        assert_eq!(offset_of!(PerturbUniform, level), 104);
        assert_eq!(offset_of!(PerturbUniform, scale_exponent), 108);
    }

    #[test]
    fn packing_copies_math_records_and_zeroes_reserved_words() {
        let centre = CentreSplit {
            hi: [1.0, 2.0, 3.0, 4.0],
            lo: [0.25, 0.5, 0.75, 1.0],
        };
        let extent = GridExtent {
            width: 960,
            height: 540,
        };
        let map = super::pack_map_rows(&Homography::IDENTITY).expect("identity map packs");
        let shallow = ShallowUniform::from_parts(
            PLANE,
            map,
            centre,
            0.125,
            extent,
            64,
            RefinementLevel::Preview,
        );
        assert_eq!(&shallow.bytes()[32..48], bytemuck::cast_slice(&map[0]));
        assert_eq!(&shallow.bytes()[80..96], bytemuck::cast_slice(&centre.hi));
        assert_eq!(&shallow.bytes()[96..112], bytemuck::cast_slice(&centre.lo));
        assert_eq!(&shallow.bytes()[136..144], &[0; 8]);
        let deep = PerturbUniform::from_parts(
            PLANE,
            map,
            ScaleSplit {
                mantissa: 0.75,
                exponent: -173,
            },
            extent,
            256,
            199,
            RefinementLevel::Interactive,
        );
        assert_eq!(&deep.bytes()[108..112], &(-173_i32).to_le_bytes());
    }

    #[test]
    fn closed_discriminants_and_zoom_switch_are_pinned() {
        assert_eq!(RefinementLevel::Preview as u32, 0);
        assert_eq!(RefinementLevel::Interactive as u32, 1);
        assert_eq!(RefinementLevel::Final as u32, 2);
        assert_eq!(KernelMode::Shallow as u32, 0);
        assert_eq!(KernelMode::Perturbation as u32, 1);
        assert_eq!(SampleStatus::Sampled as u32, 0);
        assert_eq!(SampleStatus::Glitch as u32, 1);
        assert_eq!(SampleStatus::Horizon as u32, 2);
        assert_eq!(SampleStatus::MapUncertain as u32, 3);
        assert_eq!(KernelMode::for_zoom(13.999), KernelMode::Shallow);
        assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    }

    #[test]
    fn pixel_centres_are_bottom_up_and_hybrid_in_both_subspaces() {
        let extent = GridExtent {
            width: 2,
            height: 2,
        };
        let map = super::pack_map_rows(&Homography::IDENTITY).expect("identity map packs");
        let bottom_left =
            super::pixel_offset(0, extent, PLANE, map, 1.0).expect("identity has no horizon");
        let top_left =
            super::pixel_offset(2, extent, PLANE, map, 1.0).expect("identity has no horizon");
        let vertical: [f32; 4] = std::array::from_fn(|axis| top_left[axis] - bottom_left[axis]);
        assert_eq!(bottom_left, [-0.3, -0.4, -0.4, -0.3]);
        assert_eq!(vertical, PLANE.basis_v);
        assert!(bottom_left[..2].iter().any(|value| *value != 0.0));
        assert!(bottom_left[2..].iter().any(|value| *value != 0.0));
        let odd_centre = super::pixel_offset(
            4,
            GridExtent {
                width: 3,
                height: 3,
            },
            PLANE,
            map,
            1.0,
        )
        .expect("identity has no horizon");
        assert_eq!(odd_centre, [0.0; 4]);
    }
}
