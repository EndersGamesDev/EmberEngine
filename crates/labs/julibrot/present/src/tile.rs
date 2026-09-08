//! Compatibility re-exports for the kernels-owned rendered-tile record vocabulary.

use ember_julibrot_math::{
    EscapeGridRecord, Homography, ObjectAngles, Pose, PoseMap, ReprojectionError,
    RetainedValueSample, SourceDepthRecord, ViewControls, construct_plane, retained_value_sample,
};

pub use ember_julibrot_kernels::{
    CanonicalChartCellKey, DescriptorAbiError, DescriptorCostLedger, DescriptorSamplePair,
    DescriptorTexel, ExactF32, ExactF64, PoseMapKey, RenderControlChange, SliceIdentity,
    SourceScreenRect as SourcePixelRect, TileContentKey, TileInvalidation, TilePoseHeader,
    TileQuality, TileRenderKey, TileResidency, TileRung, TransitionPresentation,
    select_same_surface_owner, tile_invalidation, transition_presentation, validate_pose_header,
};

use crate::planner::invert_3x3;

const EXACT_INTEGER_LIMIT: f32 = 16_777_216.0;
/// Four f32 ulps admit a sine/cosine pair after independent lane rounding.
const FACTOR_NORM_TOLERANCE: f64 = 4.768_371_582_031_25e-7;

/// Packs the source-pose lanes while preserving supplied policy and lifetime lanes.
pub fn pack_descriptor_header(
    render: &TileRenderKey,
    header: &TilePoseHeader,
) -> Option<TilePoseHeader> {
    let mut header = *header;
    if render.version != TileRenderKey::VERSION
        || render.source_identity.version != ember_julibrot_kernels::SourceIdentity::VERSION
        || !header.reserved_lanes_are_zero()
    {
        return None;
    }
    let PoseMapKey::Mapped(source_map) = render.source_map else {
        return None;
    };
    pack_angle_factors(
        &render.object,
        &mut header.texels[TilePoseHeader::H02_OBJECT_12_13..=TilePoseHeader::H04_OBJECT_24_34],
    )?;
    pack_angle_factors(
        &render.camera,
        &mut header.texels[TilePoseHeader::H05_CAMERA_12_13..=TilePoseHeader::H09_CAMERA_35_45],
    )?;
    header.texels[TilePoseHeader::H10_OBSERVER].lanes = [
        pack_finite(render.yaw.get().cos())?,
        pack_finite(render.yaw.get().sin())?,
        pack_finite(render.pitch.get().cos())?,
        pack_finite(render.pitch.get().sin())?,
    ];
    let (origin_high, origin_low) = pack_split_array(render.origin.map(ExactF64::get))?;
    header.texels[TilePoseHeader::H11_ORIGIN_HIGH].lanes = origin_high;
    header.texels[TilePoseHeader::H12_ORIGIN_LOW].lanes = origin_low;
    header.texels[TilePoseHeader::H13_TRANSLATION_0_3].lanes =
        pack_finite_array(core::array::from_fn(|axis| render.translation[axis].get()))?;
    header.texels[TilePoseHeader::H14_PROJECTION].lanes = pack_finite_array([
        render.translation[4].get(),
        render.height.get(),
        render.distance_five.get(),
        render.distance_four.get(),
    ])?;
    pack_extent_rect_and_map(render, source_map, &mut header)?;
    header.texels[TilePoseHeader::H00_IDENTITIES].lanes[2] =
        pack_unsigned(render.source_identity.anchor_id)?;
    header.texels[TilePoseHeader::H25_PROVENANCE].lanes[1] = pack_unsigned(render.main_generation)?;
    let chart_scale = 4.0 * source_map[19].get() / f64::from(render.extent[0]);
    if !chart_scale.is_finite() || chart_scale <= 0.0 {
        return None;
    }
    let [scale_high, scale_low] = pack_split(chart_scale)?;
    header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes[0] = scale_high;
    header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes[1] = scale_low;
    header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes[3] =
        pack_unsigned(render.source_identity.anchor_revision)?;
    descriptor_lanes_are_valid(&header).then_some(header)
}

/// Unpacks a validated header into the finite pose fields needed by projection.
pub fn unpack_descriptor_header(header: &TilePoseHeader) -> Option<Pose> {
    if !header.reserved_lanes_are_zero() || !descriptor_lanes_are_valid(header) {
        return None;
    }
    let object_factors: [f64; 6] = unpack_angle_factors(
        &header.texels[TilePoseHeader::H02_OBJECT_12_13..=TilePoseHeader::H04_OBJECT_24_34],
    )?;
    let object = ObjectAngles {
        rho_12: object_factors[0],
        rho_13: object_factors[1],
        rho_14: object_factors[2],
        rho_23: object_factors[3],
        rho_24: object_factors[4],
        rho_34: object_factors[5],
    };
    let camera: [f64; 10] = unpack_angle_factors(
        &header.texels[TilePoseHeader::H05_CAMERA_12_13..=TilePoseHeader::H09_CAMERA_35_45],
    )?;
    let observer = header.texels[TilePoseHeader::H10_OBSERVER].lanes;
    let extent = header.texels[TilePoseHeader::H15_EXTENT_DENSITY].lanes;
    let chart_scale = unpack_split(
        header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes[..2]
            .try_into()
            .ok()?,
    );
    let rows = unpack_source_map(header)?;
    let inverse = invert_3x3(rows)?;
    let grid_width = unpack_unsigned(extent[1])?;
    let grid_height = unpack_unsigned(extent[2])?;
    let projection = header.texels[TilePoseHeader::H14_PROJECTION].lanes;
    let pose = Pose {
        epoch: 0,
        orbit_generation: unpack_unsigned(header.texels[TilePoseHeader::H25_PROVENANCE].lanes[1])?,
        plane: construct_plane(object).ok()?,
        object,
        plane_origin: unpack_split_array(
            header.texels[TilePoseHeader::H11_ORIGIN_HIGH].lanes,
            header.texels[TilePoseHeader::H12_ORIGIN_LOW].lanes,
        ),
        zoom_log2: f64::from(extent[0]),
        view: ViewControls {
            camera,
            camera_translation: unpack_translation(header),
            camera_yaw: f64::from(observer[1]).atan2(f64::from(observer[0])),
            camera_pitch: f64::from(observer[3]).atan2(f64::from(observer[2])),
            height_scale: f64::from(projection[1]),
            distance_five: f64::from(projection[2]),
            distance_four: f64::from(projection[3]),
        },
        grid_width,
        grid_height,
        map: PoseMap::Mapped(Homography {
            rows,
            inverse,
            condition_number: 1.0,
            apron_scale: chart_scale * f64::from(grid_width) * 0.25,
        }),
        centre_from_reference_px: [0.0; 2],
    };
    (pose.view.is_valid() && chart_scale.is_finite() && chart_scale > 0.0).then_some(pose)
}

/// Packs one existing value record and its source reconstruction receipt without changing S0.
pub fn pack_descriptor_sample(
    value: EscapeGridRecord,
    depth: SourceDepthRecord,
) -> Option<DescriptorSamplePair> {
    let value_lanes = [
        value.smooth_iter,
        value.escaped,
        value.rebase_count,
        value.status,
    ];
    if !value_lanes.into_iter().all(f32::is_finite)
        || ![depth.a_f, depth.b_f, depth.zeta_f]
            .into_iter()
            .all(f64::is_finite)
        || (depth.valid && depth.zeta_f <= 0.0)
    {
        return None;
    }
    Some(DescriptorSamplePair::new(
        value_lanes,
        [
            pack_finite(depth.a_f)?,
            pack_finite(depth.b_f)?,
            pack_finite(depth.zeta_f)?,
            f32::from(u8::from(depth.valid)),
        ],
    ))
}

/// Unpacks one paired descriptor sample using the supplied delivered iteration cap.
///
/// # Errors
///
/// Returns a typed refusal for a non-finite lane, non-binary validity, or invalid value record.
pub fn unpack_descriptor_sample(
    pair: &DescriptorSamplePair,
    iteration_cap: u32,
) -> Result<(RetainedValueSample, SourceDepthRecord), ReprojectionError> {
    let value = retained_value_sample(
        EscapeGridRecord {
            smooth_iter: pair.s0.lanes[0],
            escaped: pair.s0.lanes[1],
            rebase_count: pair.s0.lanes[2],
            status: pair.s0.lanes[3],
        },
        iteration_cap,
    )?;
    let valid = match pair.s1.lanes[3].to_bits() {
        bits if bits == 0.0_f32.to_bits() => false,
        bits if bits == 1.0_f32.to_bits() => true,
        _ => return Err(ReprojectionError::InvalidSource),
    };
    let depth = SourceDepthRecord {
        a_f: f64::from(pair.s1.lanes[0]),
        b_f: f64::from(pair.s1.lanes[1]),
        zeta_f: f64::from(pair.s1.lanes[2]),
        valid,
    };
    if ![depth.a_f, depth.b_f, depth.zeta_f]
        .into_iter()
        .all(f64::is_finite)
        || (valid && depth.zeta_f <= 0.0)
    {
        return Err(ReprojectionError::InvalidSource);
    }
    Ok((value, depth))
}

fn pack_extent_rect_and_map(
    render: &TileRenderKey,
    source_map: [ExactF64; 20],
    header: &mut TilePoseHeader,
) -> Option<()> {
    if render.extent.contains(&0) || render.source_rect.width == 0 || render.source_rect.height == 0
    {
        return None;
    }
    header.texels[TilePoseHeader::H15_EXTENT_DENSITY].lanes[..3].copy_from_slice(&[
        pack_finite(render.zoom.get())?,
        pack_unsigned(render.extent[0])?,
        pack_unsigned(render.extent[1])?,
    ]);
    header.texels[TilePoseHeader::H16_SOURCE_RECT].lanes = [
        pack_signed(render.source_rect.x)?,
        pack_signed(render.source_rect.y)?,
        pack_unsigned(render.source_rect.width)?,
        pack_unsigned(render.source_rect.height)?,
    ];
    let rows = crate::pack_homography_rows(core::array::from_fn(|lane| source_map[lane].get()))?;
    for (destination, source) in header.texels
        [TilePoseHeader::H18_SOURCE_MAP_0..=TilePoseHeader::H20_SOURCE_MAP_2]
        .iter_mut()
        .zip(rows)
    {
        destination.lanes = source;
    }
    Some(())
}

fn pack_angle_factors(values: &[ExactF64], texels: &mut [DescriptorTexel]) -> Option<()> {
    if values.len() != texels.len() * 2 {
        return None;
    }
    let (pairs, _) = values.as_chunks::<2>();
    for (texel, pair) in texels.iter_mut().zip(pairs) {
        texel.lanes = [
            pack_finite(pair[0].get().cos())?,
            pack_finite(pair[0].get().sin())?,
            pack_finite(pair[1].get().cos())?,
            pack_finite(pair[1].get().sin())?,
        ];
    }
    Some(())
}

fn unpack_angle_factors<const N: usize>(texels: &[DescriptorTexel]) -> Option<[f64; N]> {
    if N != texels.len() * 2 {
        return None;
    }
    let mut angles = [0.0; N];
    for (angle, pair) in angles.iter_mut().zip(
        texels
            .iter()
            .flat_map(|texel| texel.lanes.as_chunks::<2>().0),
    ) {
        let norm = f64::from(pair[0]).hypot(f64::from(pair[1]));
        if (norm - 1.0).abs() > FACTOR_NORM_TOLERANCE {
            return None;
        }
        *angle = f64::from(pair[1]).atan2(f64::from(pair[0]));
    }
    Some(angles)
}

fn pack_split_array(values: [f64; 4]) -> Option<([f32; 4], [f32; 4])> {
    let mut high = [0.0; 4];
    let mut low = [0.0; 4];
    for ((high, low), value) in high.iter_mut().zip(&mut low).zip(values) {
        let split = pack_split(value)?;
        *high = split[0];
        *low = split[1];
    }
    Some((high, low))
}

fn unpack_split_array(high: [f32; 4], low: [f32; 4]) -> [f64; 4] {
    core::array::from_fn(|axis| unpack_split([high[axis], low[axis]]))
}

fn pack_split(value: f64) -> Option<[f32; 2]> {
    let high = pack_finite(value)?;
    let low = pack_finite(value - f64::from(high))?;
    Some([high, low])
}

fn unpack_split([high, low]: [f32; 2]) -> f64 {
    f64::from(high) + f64::from(low)
}

fn pack_finite_array(values: [f64; 4]) -> Option<[f32; 4]> {
    let mut packed = [0.0; 4];
    for (packed, value) in packed.iter_mut().zip(values) {
        *packed = pack_finite(value)?;
    }
    Some(packed)
}

fn pack_finite(value: f64) -> Option<f32> {
    crate::pack_homography_rows([value, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
        .map(|rows| rows[0][0])
}

fn pack_unsigned(value: u32) -> Option<f32> {
    let packed = pack_finite(f64::from(value))?;
    exact_unsigned(packed).then_some(packed)
}

fn pack_signed(value: i32) -> Option<f32> {
    let packed = pack_finite(f64::from(value))?;
    exact_signed(packed).then_some(packed)
}

fn unpack_unsigned(value: f32) -> Option<u32> {
    if !exact_unsigned(value) {
        return None;
    }
    let bits = value.to_bits();
    if bits == 0 {
        return Some(0);
    }
    let exponent = (bits >> 23) & 0xff;
    let significand = (bits & 0x7f_ff_ff) | 0x80_00_00;
    if exponent >= 150 {
        Some(significand << (exponent - 150))
    } else {
        Some(significand >> (150 - exponent))
    }
}

fn exact_unsigned(value: f32) -> bool {
    value.is_finite()
        && !value.is_sign_negative()
        && value < EXACT_INTEGER_LIMIT
        && value.fract().abs().to_bits() == 0.0_f32.to_bits()
}

fn exact_signed(value: f32) -> bool {
    value.is_finite()
        && value.abs() < EXACT_INTEGER_LIMIT
        && value.fract().abs().to_bits() == 0.0_f32.to_bits()
        && value.to_bits() != (-0.0_f32).to_bits()
}

fn descriptor_lanes_are_valid(header: &TilePoseHeader) -> bool {
    let unsigned = [
        (0, 0..4),
        (1, 0..4),
        (15, 1..3),
        (16, 2..4),
        (22, 0..4),
        (23, 0..4),
        (24, 2..4),
        (25, 0..4),
        (26, 0..4),
    ];
    header.texels[..TilePoseHeader::RESERVED_START]
        .iter()
        .flat_map(|texel| texel.lanes)
        .all(f32::is_finite)
        && unsigned.into_iter().all(|(texel, lanes)| {
            header.texels[texel].lanes[lanes]
                .iter()
                .all(|lane| exact_unsigned(*lane))
        })
        && header.texels[TilePoseHeader::H16_SOURCE_RECT].lanes[..2]
            .iter()
            .all(|lane| exact_signed(*lane))
        && header.texels[TilePoseHeader::H18_SOURCE_MAP_0..=TilePoseHeader::H20_SOURCE_MAP_2]
            .iter()
            .all(|texel| texel.lanes[3].to_bits() == 0.0_f32.to_bits())
}

fn unpack_source_map(header: &TilePoseHeader) -> Option<[f64; 9]> {
    let mut rows = [0.0; 9];
    let (unpacked, _) = rows.as_chunks_mut::<3>();
    for (row, texel) in unpacked
        .iter_mut()
        .zip(&header.texels[TilePoseHeader::H18_SOURCE_MAP_0..=TilePoseHeader::H20_SOURCE_MAP_2])
    {
        for (value, lane) in row.iter_mut().zip(texel.lanes) {
            *value = f64::from(lane);
        }
    }
    rows.into_iter().all(f64::is_finite).then_some(rows)
}

fn unpack_translation(header: &TilePoseHeader) -> [f64; 5] {
    let first = header.texels[TilePoseHeader::H13_TRANSLATION_0_3].lanes;
    let fifth = header.texels[TilePoseHeader::H14_PROJECTION].lanes[0];
    [
        f64::from(first[0]),
        f64::from(first[1]),
        f64::from(first[2]),
        f64::from(first[3]),
        f64::from(fifth),
    ]
}

#[cfg(test)]
mod tests {
    use bytemuck::{Zeroable, bytes_of};
    use ember_julibrot_kernels::SourceIdentity;
    use ember_julibrot_math::{
        ObjectAngles, Pose, PoseMap, ViewControls, construct_plane, screen_to_plane,
    };

    use super::*;

    /// Two f32 words retain these moderate finite-mirror origins within one residual rounding.
    const COMPENSATED_SPLIT_TOLERANCE: f64 = 1.0e-12;
    /// Rebuilding angles from packed sine/cosine pairs may move either factor by one f32 ulp.
    const FACTOR_ROUND_TRIP_TOLERANCE: f64 = 2.384_185_791_015_625e-7;

    fn pose() -> Pose {
        let object = ObjectAngles {
            rho_13: -0.37,
            rho_24: 0.61,
            rho_34: -0.19,
            ..ObjectAngles::IDENTITY
        };
        let mut camera = [0.0; 10];
        camera[1] = 0.23;
        camera[6] = -0.31;
        camera[9] = 0.17;
        let view = ViewControls {
            camera,
            camera_translation: [0.125, -0.25, 0.5, -0.75, 0.0625],
            camera_yaw: 0.27,
            camera_pitch: -0.14,
            height_scale: 0.8,
            distance_five: 7.0,
            distance_four: 9.0,
        };
        let map = screen_to_plane(&object, &view, 0.25, 960, 540, 16.0 / 9.0)
            .expect("descriptor fixture has a finite map");
        Pose {
            epoch: 11,
            orbit_generation: 23,
            plane: construct_plane(object).expect("descriptor fixture has a finite plane"),
            object,
            plane_origin: [0.1, -12.345_678_901, 0.000_123_456_789, 19.75],
            zoom_log2: 4.25,
            view,
            grid_width: 960,
            grid_height: 540,
            map: PoseMap::Mapped(map),
            centre_from_reference_px: [3.0, -5.0],
        }
    }

    fn policy_header() -> TilePoseHeader {
        let mut header = TilePoseHeader::zeroed();
        header.texels[TilePoseHeader::H00_IDENTITIES].lanes = [41.0, 42.0, 0.0, 1.0];
        header.texels[TilePoseHeader::H01_SPANS].lanes = [7.0, 8.0, 0.0, 9.0];
        header.texels[TilePoseHeader::H17_ANCHOR_DELTA].lanes = [0.25, 0.0, -0.5, 0.0];
        header.texels[TilePoseHeader::H21_BOUNDS].lanes = [1.0, 12.0, 0.01, 0.25];
        header.texels[TilePoseHeader::H22_QUALITY].lanes = [1.0, 2.0, 512.0, 37.0];
        header.texels[TilePoseHeader::H23_STATUS].lanes = [65_000.0, 3.0, 4.0, 2.0];
        header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes = [0.0, 0.0, 192.0, 0.0];
        header.texels[TilePoseHeader::H25_PROVENANCE].lanes = [17.0, 0.0, 1.0, 5.0];
        header.texels[TilePoseHeader::H26_OWNERSHIP].lanes = [6.0, 2.0, 31.0, 32.0];
        header
    }

    #[test]
    fn descriptor_header_pack_unpack_and_round_trip_preserve_source_pose() {
        let pose = pose();
        let rect = SourcePixelRect::from_extent(-31, 47, 256, 256);
        let source = SourceIdentity::new(73, 11, 19);
        let render = TileRenderKey::from_pose_and_source(&pose, rect, source);
        let header = pack_descriptor_header(&render, &policy_header())
            .expect("finite descriptor source pose packs");
        assert_eq!(
            header.texels[TilePoseHeader::H00_IDENTITIES].lanes,
            [41.0, 42.0, 73.0, 1.0]
        );
        assert_eq!(
            header.texels[TilePoseHeader::H16_SOURCE_RECT].lanes,
            [-31.0, 47.0, 256.0, 256.0]
        );
        assert_eq!(
            header.texels[TilePoseHeader::H25_PROVENANCE].lanes,
            [17.0, 23.0, 1.0, 5.0]
        );
        assert_eq!(
            header.texels[TilePoseHeader::H01_SPANS].lanes,
            policy_header().texels[TilePoseHeader::H01_SPANS].lanes
        );

        let unpacked = unpack_descriptor_header(&header).expect("packed descriptor unpacks");
        for (actual, expected) in unpacked.plane_origin.into_iter().zip(pose.plane_origin) {
            assert!((actual - expected).abs() <= COMPENSATED_SPLIT_TOLERANCE);
        }
        for (actual, expected) in unpacked.view.camera.into_iter().zip(pose.view.camera) {
            let factor_error = (actual.sin() - expected.sin())
                .abs()
                .max((actual.cos() - expected.cos()).abs());
            assert!(factor_error <= FACTOR_ROUND_TRIP_TOLERANCE);
        }
        assert_eq!([unpacked.grid_width, unpacked.grid_height], [960, 540]);
        assert!(header.reserved_lanes_are_zero());
    }

    #[test]
    fn descriptor_header_rejects_non_exact_integer_and_edge_on_lanes() {
        let pose = pose();
        let rect = SourcePixelRect::from_extent(0, 0, 256, 256);
        let too_wide = SourceIdentity::new(16_777_216, 0, 0);
        let render = TileRenderKey::from_pose_and_source(&pose, rect, too_wide);
        assert!(pack_descriptor_header(&render, &policy_header()).is_none());

        let mut edge = pose;
        edge.map = PoseMap::EdgeOn;
        let render = TileRenderKey::from_pose(&edge, rect);
        assert!(pack_descriptor_header(&render, &policy_header()).is_none());

        let mut fractional = policy_header();
        fractional.texels[TilePoseHeader::H22_QUALITY].lanes[3] = 1.5;
        let render = TileRenderKey::from_pose(&pose, rect);
        assert!(pack_descriptor_header(&render, &fractional).is_none());

        let mut invalid_factor = pack_descriptor_header(&render, &policy_header())
            .expect("finite descriptor source pose packs");
        invalid_factor.texels[TilePoseHeader::H02_OBJECT_12_13].lanes[..2]
            .copy_from_slice(&[0.0; 2]);
        assert!(unpack_descriptor_header(&invalid_factor).is_none());
    }

    #[test]
    fn descriptor_sample_pack_unpack_preserves_s0_bytes_and_exact_lanes() {
        let record = EscapeGridRecord {
            smooth_iter: 128.0,
            escaped: 1.0,
            rebase_count: 3.0,
            status: 0.0,
        };
        let depth = SourceDepthRecord {
            a_f: 0.125,
            b_f: -0.25,
            zeta_f: 8.0,
            valid: true,
        };
        let pair = pack_descriptor_sample(record, depth).expect("finite sample pair packs");
        let expected_s0 = DescriptorTexel {
            lanes: [128.0, 1.0, 3.0, 0.0],
        };
        assert_eq!(bytes_of(&pair.s0), bytes_of(&expected_s0));
        assert_eq!(pair.s1.lanes, [0.125, -0.25, 8.0, 1.0]);

        let (value, unpacked_depth) = unpack_descriptor_sample(&pair, 512)
            .expect("exact-in-f32 sample lanes unpack");
        assert_eq!(value.record_height, -1.0);
        assert_eq!(unpacked_depth, depth);

        let mut non_binary = pair;
        non_binary.s1.lanes[3] = 0.5;
        assert_eq!(
            unpack_descriptor_sample(&non_binary, 512),
            Err(ReprojectionError::InvalidSource)
        );
        let zero_depth = SourceDepthRecord {
            zeta_f: 0.0,
            ..depth
        };
        assert!(pack_descriptor_sample(record, zero_depth).is_none());
    }
}
