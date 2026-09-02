// CPU mirrors intentionally reproduce WGSL's fixed-width integer-to-f32 conversion.
#![allow(clippy::cast_precision_loss)]

use bytemuck::{Pod, Zeroable};
use ember_julibrot_math::{CentreSplit, Plane, ScaleSplit};
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

pub(crate) fn pixel_offset(
    index: u32,
    extent: GridExtent,
    plane: Plane,
    pixel_scale: f32,
) -> [f32; 4] {
    let column = index % extent.width;
    let row = index / extent.width;
    let x = column as f32 + 0.5 - 0.5 * extent.width as f32;
    let y = row as f32 + 0.5 - 0.5 * extent.height as f32;
    std::array::from_fn(|axis| {
        (x * plane.basis_u[axis] + y * plane.basis_v[axis]) * pixel_scale
    })
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
}

/// Exact 96-byte shallow uniform payload.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct ShallowUniform {
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
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
        centre: CentreSplit,
        pixel_scale: f32,
        extent: GridExtent,
        max_iter: u32,
        level: RefinementLevel,
    ) -> Self {
        Self {
            basis_u: plane.basis_u,
            basis_v: plane.basis_v,
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

/// Exact 64-byte scaled-perturbation uniform payload.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct PerturbUniform {
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
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
        scale: ScaleSplit,
        extent: GridExtent,
        max_iter: u32,
        orbit_length: u32,
        level: RefinementLevel,
    ) -> Self {
        Self {
            basis_u: plane.basis_u,
            basis_v: plane.basis_v,
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

#[cfg(test)]
mod tests {
    use super::{
        GridExtent, KernelMode, PerturbUniform, RefinementLevel, ShallowUniform,
    };
    use ember_julibrot_math::{CentreSplit, Plane, ScaleSplit};
    use std::mem::{align_of, offset_of, size_of};

    const PLANE: Plane = Plane {
        basis_u: [0.6, 0.0, 0.8, 0.0],
        basis_v: [0.0, 0.8, 0.0, 0.6],
    };

    #[test]
    fn gpu_uniform_layouts_are_exact() {
        assert_eq!(size_of::<ShallowUniform>(), 96);
        assert_eq!(align_of::<ShallowUniform>(), 16);
        assert_eq!(offset_of!(ShallowUniform, basis_u), 0);
        assert_eq!(offset_of!(ShallowUniform, basis_v), 16);
        assert_eq!(offset_of!(ShallowUniform, centre_hi), 32);
        assert_eq!(offset_of!(ShallowUniform, centre_lo), 48);
        assert_eq!(offset_of!(ShallowUniform, pixel_scale), 64);
        assert_eq!(offset_of!(ShallowUniform, width), 68);
        assert_eq!(offset_of!(ShallowUniform, height), 72);
        assert_eq!(offset_of!(ShallowUniform, max_iter), 76);
        assert_eq!(offset_of!(ShallowUniform, bailout), 80);
        assert_eq!(offset_of!(ShallowUniform, level), 84);
        assert_eq!(offset_of!(ShallowUniform, padding), 88);
        assert_eq!(size_of::<PerturbUniform>(), 64);
        assert_eq!(align_of::<PerturbUniform>(), 16);
        assert_eq!(offset_of!(PerturbUniform, basis_u), 0);
        assert_eq!(offset_of!(PerturbUniform, basis_v), 16);
        assert_eq!(offset_of!(PerturbUniform, pixel_scale), 32);
        assert_eq!(offset_of!(PerturbUniform, width), 36);
        assert_eq!(offset_of!(PerturbUniform, height), 40);
        assert_eq!(offset_of!(PerturbUniform, max_iter), 44);
        assert_eq!(offset_of!(PerturbUniform, bailout), 48);
        assert_eq!(offset_of!(PerturbUniform, orbit_length), 52);
        assert_eq!(offset_of!(PerturbUniform, level), 56);
        assert_eq!(offset_of!(PerturbUniform, scale_exponent), 60);
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
        let shallow = ShallowUniform::from_parts(
            PLANE,
            centre,
            0.125,
            extent,
            64,
            RefinementLevel::Preview,
        );
        assert_eq!(&shallow.bytes()[32..48], bytemuck::cast_slice(&centre.hi));
        assert_eq!(&shallow.bytes()[48..64], bytemuck::cast_slice(&centre.lo));
        assert_eq!(&shallow.bytes()[88..96], &[0; 8]);
        let deep = PerturbUniform::from_parts(
            PLANE,
            ScaleSplit {
                mantissa: 0.75,
                exponent: -173,
            },
            extent,
            256,
            199,
            RefinementLevel::Interactive,
        );
        assert_eq!(&deep.bytes()[60..64], &(-173_i32).to_le_bytes());
    }

    #[test]
    fn closed_discriminants_and_zoom_switch_are_pinned() {
        assert_eq!(RefinementLevel::Preview as u32, 0);
        assert_eq!(RefinementLevel::Interactive as u32, 1);
        assert_eq!(RefinementLevel::Final as u32, 2);
        assert_eq!(KernelMode::Shallow as u32, 0);
        assert_eq!(KernelMode::Perturbation as u32, 1);
        assert_eq!(KernelMode::for_zoom(13.999), KernelMode::Shallow);
        assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    }

    #[test]
    fn pixel_centres_are_bottom_up_and_hybrid_in_both_subspaces() {
        let extent = GridExtent {
            width: 2,
            height: 2,
        };
        let bottom_left = super::pixel_offset(0, extent, PLANE, 1.0);
        let top_left = super::pixel_offset(2, extent, PLANE, 1.0);
        let vertical: [f32; 4] =
            std::array::from_fn(|axis| top_left[axis] - bottom_left[axis]);
        assert_eq!(bottom_left, [-0.3, -0.4, -0.4, -0.3]);
        assert_eq!(vertical, PLANE.basis_v);
        assert!(bottom_left[..2].iter().any(|value| *value != 0.0));
        assert!(bottom_left[2..].iter().any(|value| *value != 0.0));
    }
}
