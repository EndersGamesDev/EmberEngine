//! Compatibility re-exports for the kernels-owned rendered-tile record vocabulary.

use ember_julibrot_math::{
    EscapeGridRecord, Homography, ObjectAngles, Plane, Pose, PoseMap, ProjectedSample,
    ReconstructedSample, ReprojectionError, SourceDepthRecord, ViewControls, construct_plane,
    plane_chart_relation, project_reconstructed_sample, retained_value_sample,
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
const SLICE_DETERMINANT_EPSILON: f64 = 1.0e-12;
/// A binary32 × binary32 × binary64 product has at most 101 significant bits. Requiring an
/// unbiased exponent of at least -974 keeps its lowest possible exact bit at the binary64
/// subnormal floor, -1074, where the fused residual remains representable.
const MIN_ERROR_FREE_TRIPLE_MAGNITUDE: f64 = f64::from_bits(49_u64 << 52);

/// Certified affine chart transform between two parameterizations of one sampled slice.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct SliceChartTransform {
    /// Row-major two-dimensional map from source to canonical coordinates.
    pub chart_map: [f64; 4],
    /// Source-origin offset expressed in canonical chart coordinates.
    pub origin_offset: [f64; 2],
    /// Certified semantic distance outside the canonical plane; zero for every accepted slice.
    pub out_of_plane_error: f64,
    /// Maximum admitted semantic out-of-plane error; exactly zero in version one.
    pub maximum_error: f64,
}

impl SliceChartTransform {
    /// Slice-transform schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 64;

    /// Maps one source chart coordinate into the certified canonical chart.
    #[must_use]
    pub const fn map_source_coordinate(&self, source: [f64; 2]) -> [f64; 2] {
        [
            self.chart_map[0].mul_add(
                source[0],
                self.chart_map[1].mul_add(source[1], self.origin_offset[0]),
            ),
            self.chart_map[2].mul_add(
                source[0],
                self.chart_map[3].mul_add(source[1], self.origin_offset[1]),
            ),
        ]
    }
}

/// One retained descriptor sample and the source pixel whose receipt it must reproduce.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct DescriptorFootprintSample {
    /// Centred source-screen pixel coordinate.
    pub source_pixel: [f64; 2],
    /// Paired value and lifted-position descriptor records.
    pub descriptor: DescriptorSamplePair,
}

impl DescriptorFootprintSample {
    /// Footprint-input schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 48;
}

/// Conservative canonical chart bounds and delivered local sample-density interval.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct DerivedChartFootprint {
    /// Canonical slice whose chart contains the bounds.
    pub slice: SliceIdentity,
    /// Outward-rounded lower chart coordinate after declared error.
    pub conservative_minimum: [f64; 2],
    /// Outward-rounded upper chart coordinate after declared error.
    pub conservative_maximum: [f64; 2],
    /// Outward-rounded minimum samples per canonical chart unit.
    pub density_minimum: f64,
    /// Outward-rounded maximum samples per canonical chart unit.
    pub density_maximum: f64,
    /// Conservative canonical-coordinate error applied on every bound edge.
    pub coordinate_error: f64,
    /// Number of valid reconstructed samples contributing to the bounds.
    pub valid_sample_count: u32,
}

impl DerivedChartFootprint {
    /// Derived-footprint schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 128;

    /// Reports whether the conservative rectangle covers a canonical chart coordinate.
    #[must_use]
    pub fn contains(&self, coordinate: [f64; 2]) -> bool {
        coordinate.into_iter().enumerate().all(|(axis, value)| {
            (self.conservative_minimum[axis]..=self.conservative_maximum[axis]).contains(&value)
        })
    }
}

/// Certifies that source and canonical identities name the same sampled affine slice.
///
/// The bases must pass the shared once-rounded plane relation. Every three-axis minor of the
/// canonical basis and the exact stored origin delta must then vanish under error-free dyadic
/// expansion arithmetic. The source chart scale is validated but never relaxes that proof.
#[must_use]
pub fn certify_same_slice(
    source: SliceIdentity,
    canonical: SliceIdentity,
    source_chart_scale: f64,
) -> Option<SliceChartTransform> {
    if !source_chart_scale.is_finite() || source_chart_scale <= 0.0 {
        return None;
    }
    let (source_plane, source_origin) = unpack_slice_identity(source)?;
    let (canonical_plane, canonical_origin) = unpack_slice_identity(canonical)?;
    let relation = plane_chart_relation(source_plane, canonical_plane)?;
    if !origin_delta_lies_in_plane(canonical_plane, source_origin, canonical_origin)? {
        return None;
    }
    let origin_delta = core::array::from_fn(|axis| source_origin[axis] - canonical_origin[axis]);
    let (origin_offset, _) = project_vector_to_plane(canonical_plane, origin_delta)?;
    Some(SliceChartTransform {
        chart_map: relation.chart_map,
        origin_offset,
        out_of_plane_error: 0.0,
        maximum_error: 0.0,
    })
}

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
    let camera_yaw = unpack_factor_pair(observer[..2].try_into().ok()?)?;
    let camera_pitch = unpack_factor_pair(observer[2..].try_into().ok()?)?;
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
            camera_yaw,
            camera_pitch,
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

/// Unpacks one paired descriptor sample without changing either record's declared fields.
///
/// # Errors
///
/// Returns a typed refusal for a non-finite lane, non-binary validity, or non-positive valid depth.
pub fn unpack_descriptor_sample(
    pair: &DescriptorSamplePair,
) -> Result<(EscapeGridRecord, SourceDepthRecord), ReprojectionError> {
    let value = EscapeGridRecord {
        smooth_iter: pair.s0.lanes[0],
        escaped: pair.s0.lanes[1],
        rebase_count: pair.s0.lanes[2],
        status: pair.s0.lanes[3],
    };
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
    if ![
        value.smooth_iter,
        value.escaped,
        value.rebase_count,
        value.status,
    ]
    .into_iter()
    .all(f32::is_finite)
        || ![depth.a_f, depth.b_f, depth.zeta_f]
            .into_iter()
            .all(f64::is_finite)
        || (valid && depth.zeta_f <= 0.0)
    {
        return Err(ReprojectionError::InvalidSource);
    }
    Ok((value, depth))
}

/// Reconstructs a paired descriptor and returns its recomputed source pixel and depth.
///
/// # Errors
///
/// Returns a typed refusal for an invalid descriptor or failed source self-round-trip.
pub fn reconstruct_descriptor_sample(
    header: &TilePoseHeader,
    pair: &DescriptorSamplePair,
    source_pixel: [f64; 2],
) -> Result<(ReconstructedSample, ProjectedSample), ReprojectionError> {
    let source = unpack_descriptor_header(header).ok_or(ReprojectionError::InvalidSource)?;
    let iteration_cap = unpack_unsigned(header.texels[TilePoseHeader::H22_QUALITY].lanes[2])
        .ok_or(ReprojectionError::InvalidSource)?;
    let bounds = header.texels[TilePoseHeader::H21_BOUNDS].lanes;
    let [depth_min, depth_max, coordinate_error, reprojection_error] = bounds.map(f64::from);
    let (record, depth) = unpack_descriptor_sample(pair)?;
    let value = retained_value_sample(record, iteration_cap)?;
    if depth_min < 0.0
        || depth_max < depth_min
        || coordinate_error < 0.0
        || reprojection_error < 0.0
        || depth.zeta_f < depth_min
        || depth.zeta_f > depth_max
    {
        return Err(ReprojectionError::InvalidSource);
    }
    let reconstructed = ReconstructedSample::from_source_receipt(
        &source,
        source_pixel,
        depth,
        value,
        reprojection_error,
        coordinate_error,
    )?;
    let source_receipt = project_reconstructed_sample(&source, reconstructed)?;
    Ok((reconstructed, source_receipt))
}

/// Reconstructs one paired descriptor and projects it through an independently requested pose.
///
/// # Errors
///
/// Returns a typed refusal for an invalid descriptor, failed source receipt, or target pole.
pub fn project_descriptor_sample(
    header: &TilePoseHeader,
    pair: &DescriptorSamplePair,
    source_pixel: [f64; 2],
    target: &Pose,
) -> Result<ProjectedSample, ReprojectionError> {
    let (reconstructed, _) = reconstruct_descriptor_sample(header, pair, source_pixel)?;
    let anchor = header.texels[TilePoseHeader::H17_ANCHOR_DELTA].lanes;
    let source_to_request_anchor_px = [
        unpack_split([anchor[0], anchor[1]]),
        unpack_split([anchor[2], anchor[3]]),
    ];
    let split_error_px = split_rounding_error(anchor[1]).hypot(split_rounding_error(anchor[3]));
    let independent_error_px = f64::from(header.texels[TilePoseHeader::H21_BOUNDS].lanes[3]);
    reconstructed.project_from_anchor(
        target,
        source_to_request_anchor_px,
        independent_error_px + split_error_px,
    )
}

/// Derives conservative canonical chart bounds and local density from valid descriptor samples.
///
/// Invalid visibility records do not contribute a surface point. Every contributing record must
/// pass the stage-0 source reconstruction receipt before it can widen the derived footprint.
///
/// # Errors
///
/// Returns a typed refusal for mismatched content provenance, an uncertified slice, an invalid
/// sample, or a corpus containing no valid reconstructed surface sample.
pub fn derive_chart_footprint(
    content: &TileContentKey,
    header: &TilePoseHeader,
    samples: &[DescriptorFootprintSample],
) -> Result<DerivedChartFootprint, ReprojectionError> {
    let source = unpack_descriptor_header(header).ok_or(ReprojectionError::InvalidSource)?;
    if !descriptor_matches_content(content, header) {
        return Err(ReprojectionError::InvalidSource);
    }
    let source_chart_scale = source_chart_scale(&source).ok_or(ReprojectionError::InvalidSource)?;
    let source_slice = SliceIdentity::new(source.plane, source.plane_origin);
    let transform = certify_same_slice(source_slice, content.slice, source_chart_scale)
        .ok_or(ReprojectionError::InvalidTarget)?;
    let source_coordinate_error = f64::from(header.texels[TilePoseHeader::H21_BOUNDS].lanes[2]);
    let coordinate_error = canonical_coordinate_error(transform, source_coordinate_error)
        .ok_or(ReprojectionError::InvalidSource)?;

    let mut footprint = ChartFootprintAccumulator::new();
    for sample in samples {
        let (_, depth) = unpack_descriptor_sample(&sample.descriptor)?;
        if !depth.valid {
            continue;
        }
        let (reconstructed, _) =
            reconstruct_descriptor_sample(header, &sample.descriptor, sample.source_pixel)?;
        let (source_coordinate, residual) =
            project_ambient_to_slice(source_slice, reconstructed.ambient_four)
                .ok_or(ReprojectionError::InvalidSource)?;
        if residual > source_coordinate_error {
            return Err(ReprojectionError::InvalidSource);
        }
        let canonical_coordinate = transform.map_source_coordinate(source_coordinate);
        let density = source_density_interval(&source, sample.source_pixel)
            .ok_or(ReprojectionError::InvalidSource)?;
        footprint.include(canonical_coordinate, density)?;
    }
    footprint.finish(content.slice, coordinate_error)
}

struct ChartFootprintAccumulator {
    minimum: [f64; 2],
    maximum: [f64; 2],
    density_minimum: f64,
    density_maximum: f64,
    valid_sample_count: u32,
}

impl ChartFootprintAccumulator {
    const fn new() -> Self {
        Self {
            minimum: [f64::INFINITY; 2],
            maximum: [f64::NEG_INFINITY; 2],
            density_minimum: f64::INFINITY,
            density_maximum: 0.0,
            valid_sample_count: 0,
        }
    }

    fn include(
        &mut self,
        coordinate: [f64; 2],
        density: [f64; 2],
    ) -> Result<(), ReprojectionError> {
        if !coordinate.into_iter().chain(density).all(f64::is_finite)
            || density[0] <= 0.0
            || density[1] < density[0]
        {
            return Err(ReprojectionError::InvalidSource);
        }
        for ((minimum, maximum), value) in self
            .minimum
            .iter_mut()
            .zip(&mut self.maximum)
            .zip(coordinate)
        {
            *minimum = minimum.min(value);
            *maximum = maximum.max(value);
        }
        self.density_minimum = self.density_minimum.min(density[0]);
        self.density_maximum = self.density_maximum.max(density[1]);
        self.valid_sample_count = self
            .valid_sample_count
            .checked_add(1)
            .ok_or(ReprojectionError::InvalidSource)?;
        Ok(())
    }

    fn finish(
        self,
        slice: SliceIdentity,
        coordinate_error: f64,
    ) -> Result<DerivedChartFootprint, ReprojectionError> {
        if self.valid_sample_count == 0 {
            return Err(ReprojectionError::InvalidSource);
        }
        Ok(DerivedChartFootprint {
            slice,
            conservative_minimum: self
                .minimum
                .map(|value| (value - coordinate_error).next_down()),
            conservative_maximum: self
                .maximum
                .map(|value| (value + coordinate_error).next_up()),
            density_minimum: self.density_minimum,
            density_maximum: self.density_maximum,
            coordinate_error,
            valid_sample_count: self.valid_sample_count,
        })
    }
}

fn descriptor_matches_content(content: &TileContentKey, header: &TilePoseHeader) -> bool {
    let quality = header.texels[TilePoseHeader::H22_QUALITY].lanes;
    let provenance = header.texels[TilePoseHeader::H25_PROVENANCE].lanes;
    content.version == TileContentKey::VERSION
        && unpack_unsigned(quality[2]) == Some(content.iteration_cap)
        && unpack_unsigned(provenance[1]) == Some(content.main_generation)
        && unpack_unsigned(provenance[2]) == Some(content.record_abi)
        && unpack_unsigned(provenance[3]) == Some(content.reference_generation)
}

fn source_chart_scale(source: &Pose) -> Option<f64> {
    let PoseMap::Mapped(map) = source.map else {
        return None;
    };
    if source.grid_width == 0 {
        return None;
    }
    let scale = 4.0 * map.apron_scale / f64::from(source.grid_width);
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

/// Closed binary64 interval whose arithmetic expands every nearest-rounded result by one
/// representable value toward each infinity.
#[derive(Clone, Copy)]
struct DirectedInterval {
    lower: f64,
    upper: f64,
}

impl DirectedInterval {
    const fn point(value: f64) -> Option<Self> {
        if value.is_finite() {
            Some(Self {
                lower: value,
                upper: value,
            })
        } else {
            None
        }
    }

    const fn contains_zero(self) -> bool {
        self.lower <= 0.0 && self.upper >= 0.0
    }

    const fn negated(self) -> Self {
        Self {
            lower: -self.upper,
            upper: -self.lower,
        }
    }

    fn add(self, addend: Self) -> Option<Self> {
        outward_interval([
            self.lower + addend.lower,
            self.upper + addend.upper,
        ])
    }

    fn subtract(self, subtrahend: Self) -> Option<Self> {
        outward_interval([
            self.lower - subtrahend.upper,
            self.upper - subtrahend.lower,
        ])
    }

    fn multiply(self, multiplier: Self) -> Option<Self> {
        outward_interval([
            self.lower * multiplier.lower,
            self.lower * multiplier.upper,
            self.upper * multiplier.lower,
            self.upper * multiplier.upper,
        ])
    }

    fn divide(self, divisor: Self) -> Option<Self> {
        if divisor.contains_zero() {
            return None;
        }
        outward_interval([
            self.lower / divisor.lower,
            self.lower / divisor.upper,
            self.upper / divisor.lower,
            self.upper / divisor.upper,
        ])
    }

    fn mul_add(self, multiplier: Self, addend: Self) -> Option<Self> {
        outward_interval([
            self.lower.mul_add(multiplier.lower, addend.lower),
            self.lower.mul_add(multiplier.lower, addend.upper),
            self.lower.mul_add(multiplier.upper, addend.lower),
            self.lower.mul_add(multiplier.upper, addend.upper),
            self.upper.mul_add(multiplier.lower, addend.lower),
            self.upper.mul_add(multiplier.lower, addend.upper),
            self.upper.mul_add(multiplier.upper, addend.lower),
            self.upper.mul_add(multiplier.upper, addend.upper),
        ])
    }

    const fn absolute(self) -> Self {
        if self.contains_zero() {
            Self {
                lower: 0.0,
                upper: self.lower.abs().max(self.upper.abs()),
            }
        } else {
            Self {
                lower: self.lower.abs().min(self.upper.abs()),
                upper: self.lower.abs().max(self.upper.abs()),
            }
        }
    }

    fn hypot(self, other: Self) -> Option<Self> {
        let left = self.absolute();
        let right = other.absolute();
        outward_interval([
            left.lower.hypot(right.lower),
            left.upper.hypot(right.upper),
        ])
        .map(|interval| Self {
            lower: interval.lower.max(0.0),
            upper: interval.upper,
        })
    }

    fn square_root(self) -> Option<Self> {
        if self.upper < 0.0 {
            return None;
        }
        outward_interval([self.lower.max(0.0).sqrt(), self.upper.sqrt()]).map(|interval| Self {
            lower: interval.lower.max(0.0),
            upper: interval.upper,
        })
    }
}

fn outward_interval(values: impl IntoIterator<Item = f64>) -> Option<DirectedInterval> {
    let mut values = values.into_iter();
    let first = values.next()?;
    if !first.is_finite() {
        return None;
    }
    let mut lower = first;
    let mut upper = first;
    for value in values {
        if !value.is_finite() {
            return None;
        }
        lower = lower.min(value);
        upper = upper.max(value);
    }
    let interval = DirectedInterval {
        lower: lower.next_down(),
        upper: upper.next_up(),
    };
    (interval.lower.is_finite() && interval.upper.is_finite()).then_some(interval)
}

fn source_density_interval(source: &Pose, pixel: [f64; 2]) -> Option<[f64; 2]> {
    let PoseMap::Mapped(map) = source.map else {
        return None;
    };
    if source.grid_width == 0 {
        return None;
    }
    let [first, second, third, fourth, fifth, sixth, seventh, eighth, ninth] =
        map.rows.map(DirectedInterval::point);
    let coefficients = [
        first?, second?, third?, fourth?, fifth?, sixth?, seventh?, eighth?, ninth?,
    ];
    let four = DirectedInterval::point(4.0)?;
    let width = DirectedInterval::point(f64::from(source.grid_width))?;
    let scale = four
        .multiply(DirectedInterval::point(map.apron_scale)?)?
        .divide(width)?;
    let jacobian = density_jacobian(&coefficients, scale, pixel)?;
    density_from_jacobian(jacobian)
}

fn density_jacobian(
    coefficients: &[DirectedInterval; 9],
    scale: DirectedInterval,
    pixel: [f64; 2],
) -> Option<[DirectedInterval; 4]> {
    let [x, y] = pixel.map(DirectedInterval::point);
    let x = x?;
    let y = y?;
    let denominator = coefficients[6].mul_add(
        x,
        coefficients[7].mul_add(y, coefficients[8])?,
    )?;
    if denominator.lower <= 0.0 {
        return None;
    }
    let numerator_x =
        coefficients[0].mul_add(x, coefficients[1].mul_add(y, coefficients[2])?)?;
    let numerator_y =
        coefficients[3].mul_add(x, coefficients[4].mul_add(y, coefficients[5])?)?;
    let denominator_squared = denominator.multiply(denominator)?;
    Some([
        scale
            .multiply(coefficients[0].mul_add(
                denominator,
                numerator_x.multiply(coefficients[6])?.negated(),
            )?)?
            .divide(denominator_squared)?,
        scale
            .multiply(coefficients[1].mul_add(
                denominator,
                numerator_x.multiply(coefficients[7])?.negated(),
            )?)?
            .divide(denominator_squared)?,
        scale
            .multiply(coefficients[3].mul_add(
                denominator,
                numerator_y.multiply(coefficients[6])?.negated(),
            )?)?
            .divide(denominator_squared)?,
        scale
            .multiply(coefficients[4].mul_add(
                denominator,
                numerator_y.multiply(coefficients[7])?.negated(),
            )?)?
            .divide(denominator_squared)?,
    ])
}

fn density_from_jacobian(jacobian: [DirectedInterval; 4]) -> Option<[f64; 2]> {
    let horizontal_metric =
        jacobian[0].mul_add(jacobian[0], jacobian[2].multiply(jacobian[2])?)?;
    let mixed_metric =
        jacobian[0].mul_add(jacobian[1], jacobian[2].multiply(jacobian[3])?)?;
    let vertical_metric =
        jacobian[1].mul_add(jacobian[1], jacobian[3].multiply(jacobian[3])?)?;
    let two = DirectedInterval::point(2.0)?;
    let discriminant = horizontal_metric
        .subtract(vertical_metric)?
        .hypot(two.multiply(mixed_metric)?)?;
    let metric_sum = horizontal_metric
        .add(vertical_metric)?
        .add(discriminant)?;
    let sigma_maximum = DirectedInterval::point(0.5)?
        .multiply(metric_sum)?
        .square_root()?;
    if sigma_maximum.contains_zero() {
        return None;
    }
    let determinant = jacobian[0].mul_add(
        jacobian[3],
        jacobian[1].multiply(jacobian[2])?.negated(),
    )?;
    if determinant.contains_zero() {
        return None;
    }
    let sigma_minimum = determinant.absolute().divide(sigma_maximum)?;
    if sigma_minimum.contains_zero() {
        return None;
    }
    let one = DirectedInterval::point(1.0)?;
    let minimum_density = one.divide(sigma_maximum)?;
    let maximum_density = one.divide(sigma_minimum)?;
    (minimum_density.lower > 0.0 && maximum_density.upper >= minimum_density.lower)
        .then_some([minimum_density.lower, maximum_density.upper])
}

fn canonical_coordinate_error(transform: SliceChartTransform, source_error: f64) -> Option<f64> {
    if !source_error.is_finite() || source_error < 0.0 {
        return None;
    }
    let row_norm = transform.chart_map[..2]
        .iter()
        .map(|value| value.abs())
        .sum::<f64>()
        .max(
            transform.chart_map[2..]
                .iter()
                .map(|value| value.abs())
                .sum(),
        );
    let error = source_error.mul_add(row_norm, transform.out_of_plane_error);
    error.is_finite().then_some(error)
}

fn unpack_slice_identity(slice: SliceIdentity) -> Option<(Plane, [f64; 4])> {
    let plane = Plane {
        basis_u: slice.basis_u.map(ExactF32::get),
        basis_v: slice.basis_v.map(ExactF32::get),
    };
    let origin = slice.origin.map(ExactF64::get);
    plane
        .basis_u
        .into_iter()
        .chain(plane.basis_v)
        .map(f64::from)
        .chain(origin)
        .all(f64::is_finite)
        .then_some((plane, origin))
}

fn project_ambient_to_slice(slice: SliceIdentity, ambient: [f64; 4]) -> Option<([f64; 2], f64)> {
    let (plane, origin) = unpack_slice_identity(slice)?;
    let relative = core::array::from_fn(|axis| ambient[axis] - origin[axis]);
    project_vector_to_plane(plane, relative)
}

fn project_vector_to_plane(plane: Plane, vector: [f64; 4]) -> Option<([f64; 2], f64)> {
    if !vector.into_iter().all(f64::is_finite) {
        return None;
    }
    let basis_u = plane.basis_u.map(f64::from);
    let basis_v = plane.basis_v.map(f64::from);
    let primary_gram = dot_four(basis_u, basis_u);
    let mixed_gram = dot_four(basis_u, basis_v);
    let secondary_gram = dot_four(basis_v, basis_v);
    let determinant = primary_gram.mul_add(secondary_gram, -mixed_gram * mixed_gram);
    if !determinant.is_finite() || determinant.abs() <= SLICE_DETERMINANT_EPSILON {
        return None;
    }
    let dot_u = dot_four(vector, basis_u);
    let dot_v = dot_four(vector, basis_v);
    let coordinate = [
        dot_u.mul_add(secondary_gram, -dot_v * mixed_gram) / determinant,
        dot_v.mul_add(primary_gram, -dot_u * mixed_gram) / determinant,
    ];
    let projected = plane.local_point(coordinate);
    let residual = vector
        .into_iter()
        .zip(projected)
        .map(|(value, projected)| (value - projected).powi(2))
        .sum::<f64>()
        .sqrt();
    coordinate
        .into_iter()
        .chain([residual])
        .all(f64::is_finite)
        .then_some((coordinate, residual))
}

fn origin_delta_lies_in_plane(
    plane: Plane,
    source_origin: [f64; 4],
    canonical_origin: [f64; 4],
) -> Option<bool> {
    const AXIS_TRIPLES: [[usize; 3]; 4] = [[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]];

    for axes in AXIS_TRIPLES {
        if !exact_plane_minor_is_zero(plane, source_origin, canonical_origin, axes)? {
            return Some(false);
        }
    }
    Some(true)
}

fn exact_plane_minor_is_zero(
    plane: Plane,
    source_origin: [f64; 4],
    canonical_origin: [f64; 4],
    axes: [usize; 3],
) -> Option<bool> {
    let mut expansion = Vec::with_capacity(24);
    let [first, second, third] = axes;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[first],
        plane.basis_v[second],
        source_origin[third],
        canonical_origin[third],
        false,
    )?;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[second],
        plane.basis_v[third],
        source_origin[first],
        canonical_origin[first],
        false,
    )?;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[third],
        plane.basis_v[first],
        source_origin[second],
        canonical_origin[second],
        false,
    )?;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[first],
        plane.basis_v[third],
        source_origin[second],
        canonical_origin[second],
        true,
    )?;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[second],
        plane.basis_v[first],
        source_origin[third],
        canonical_origin[third],
        true,
    )?;
    add_origin_delta_product(
        &mut expansion,
        plane.basis_u[third],
        plane.basis_v[second],
        source_origin[first],
        canonical_origin[first],
        true,
    )?;
    Some(expansion.is_empty())
}

fn add_origin_delta_product(
    expansion: &mut Vec<f64>,
    basis_left: f32,
    basis_right: f32,
    source_origin: f64,
    canonical_origin: f64,
    negative: bool,
) -> Option<()> {
    add_exact_triple_product(
        expansion,
        basis_left,
        basis_right,
        source_origin,
        negative,
    )?;
    add_exact_triple_product(
        expansion,
        basis_left,
        basis_right,
        canonical_origin,
        !negative,
    )
}

fn add_exact_triple_product(
    expansion: &mut Vec<f64>,
    basis_left: f32,
    basis_right: f32,
    origin: f64,
    negative: bool,
) -> Option<()> {
    let basis_product = f64::from(basis_left) * f64::from(basis_right);
    if basis_product == 0.0 || origin == 0.0 {
        return Some(());
    }
    let product = basis_product * origin;
    if !product.is_normal() || product.abs() < MIN_ERROR_FREE_TRIPLE_MAGNITUDE {
        return None;
    }
    let roundoff = basis_product.mul_add(origin, -product);
    let sign = if negative { -1.0 } else { 1.0 };
    add_expansion_component(expansion, sign * roundoff)?;
    add_expansion_component(expansion, sign * product)
}

fn add_expansion_component(expansion: &mut Vec<f64>, component: f64) -> Option<()> {
    if component == 0.0 {
        return Some(());
    }
    let previous = core::mem::take(expansion);
    let mut grown = Vec::with_capacity(previous.len() + 1);
    let mut total = component;
    for value in previous {
        let (sum, roundoff) = error_free_sum(total, value)?;
        if roundoff != 0.0 {
            grown.push(roundoff);
        }
        total = sum;
    }
    if total != 0.0 {
        grown.push(total);
    }
    *expansion = grown;
    Some(())
}

const fn error_free_sum(accumulator: f64, addend: f64) -> Option<(f64, f64)> {
    let sum = accumulator + addend;
    if !sum.is_finite() {
        return None;
    }
    let rounded_addend = sum - accumulator;
    let restored_accumulator = sum - rounded_addend;
    let addend_error = addend - rounded_addend;
    let accumulator_error = accumulator - restored_accumulator;
    Some((sum, accumulator_error + addend_error))
}

fn dot_four(left: [f64; 4], right: [f64; 4]) -> f64 {
    left.into_iter()
        .zip(right)
        .fold(0.0, |sum, (left, right)| left.mul_add(right, sum))
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
        *angle = unpack_factor_pair(*pair)?;
    }
    Some(angles)
}

fn unpack_factor_pair([cosine, sine]: [f32; 2]) -> Option<f64> {
    let norm = f64::from(cosine).hypot(f64::from(sine));
    ((norm - 1.0).abs() <= FACTOR_NORM_TOLERANCE)
        .then_some(f64::from(sine).atan2(f64::from(cosine)))
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

fn split_rounding_error(low: f32) -> f64 {
    let magnitude = low.abs();
    let bits = magnitude.to_bits();
    let adjacent = if bits == f32::MAX.to_bits() {
        f32::from_bits(bits - 1)
    } else {
        f32::from_bits(bits + 1)
    };
    0.5 * (f64::from(adjacent) - f64::from(magnitude)).abs()
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
    use core::mem::size_of;

    use bytemuck::{Zeroable, bytes_of};
    use ember_julibrot_kernels::SourceIdentity;
    use ember_julibrot_math::{
        ObjectAngles, Pose, PoseMap, PrecisionMode, ViewControls, construct_plane,
        reconstruct_source_sample, screen_to_plane, source_depth_record,
    };

    use super::*;

    /// Two f32 words retain these moderate finite-mirror origins within one residual rounding.
    const COMPENSATED_SPLIT_TOLERANCE: f64 = 1.0e-12;
    /// Rebuilding angles from packed sine/cosine pairs may move either factor by two f32
    /// epsilon units.
    const FACTOR_ROUND_TRIP_TOLERANCE: f64 = 2.384_185_791_015_625e-7;
    /// H21 admits one hundredth of source linear depth after the S1 f32 lane is decoded.
    const SOURCE_DEPTH_RECEIPT_TOLERANCE: f32 = 0.01;
    /// H21 admits one quarter source pixel after every source-pose factor is decoded.
    const SOURCE_PIXEL_RECEIPT_TOLERANCE_PX: f32 = 0.25;
    /// One millionth retains reconstructed ambient coordinates across descriptor f32 rounding.
    const SOURCE_COORDINATE_TOLERANCE: f64 = 0.000_001;
    /// The descriptor study admits a target vertex only at or below one direct-f64 pixel.
    const TARGET_PROJECTION_TOLERANCE_PX: f64 = 1.0;
    /// A hundredth retains target linear depth across source-pose and S1 f32 rounding.
    const TARGET_DEPTH_TOLERANCE: f64 = 0.01;
    /// One hundred-thousandth retains normalized raster depth across descriptor rounding.
    const TARGET_RASTER_DEPTH_TOLERANCE: f64 = 0.000_01;
    /// The deepest binary64-scale contract exercised by the existing math scale oracle.
    const MAXIMAL_DESCRIPTOR_ZOOM_LOG2: f64 = 1000.25;
    /// A quarter-pixel exact anchor delta must have a material projected effect.
    const ANCHOR_EFFECT_MINIMUM_PX: f64 = 0.1;
    /// The identity density chain has fewer than 128 outward-rounded endpoint operations; at
    /// density 64 each endpoint ulp is 64 binary64 epsilon units.
    const IDENTITY_DENSITY_INTERVAL_BOUND: f64 = 8_192.0 * f64::EPSILON;
    const FOOTPRINT_CORPUS: [[f64; 2]; 9] = [
        [-95.5, -47.25],
        [0.25, -47.25],
        [95.75, -47.25],
        [-95.5, 0.5],
        [0.25, 0.5],
        [95.75, 0.5],
        [-95.5, 47.75],
        [0.25, 47.75],
        [95.75, 47.75],
    ];

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
        let map = screen_to_plane(&object, &view, 4.25, 960, 540, 16.0 / 9.0)
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

    fn rebuild_map(pose: &mut Pose) {
        let map = screen_to_plane(
            &pose.object,
            &pose.view,
            pose.zoom_log2,
            pose.grid_width,
            pose.grid_height,
            f64::from(pose.grid_width) / f64::from(pose.grid_height),
        )
        .expect("descriptor fixture has a finite map");
        pose.map = PoseMap::Mapped(map);
    }

    fn source_pose() -> Pose {
        let mut source = pose();
        source.view.height_scale = 0.0;
        rebuild_map(&mut source);
        source
    }

    fn requested_pose() -> Pose {
        let mut target = pose();
        target.view.camera[2] += 0.09;
        target.view.camera[7] -= 0.06;
        target.view.camera_translation[3] += 0.125;
        target.view.camera_yaw -= 0.08;
        target.view.camera_pitch += 0.05;
        target.view.distance_five = 6.5;
        target.view.distance_four = 8.5;
        rebuild_map(&mut target);
        target
    }

    fn policy_header() -> TilePoseHeader {
        let mut header = TilePoseHeader::zeroed();
        header.texels[TilePoseHeader::H00_IDENTITIES].lanes = [41.0, 42.0, 0.0, 1.0];
        header.texels[TilePoseHeader::H01_SPANS].lanes = [7.0, 8.0, 0.0, 9.0];
        header.texels[TilePoseHeader::H21_BOUNDS].lanes = [
            1.0,
            12.0,
            SOURCE_DEPTH_RECEIPT_TOLERANCE,
            SOURCE_PIXEL_RECEIPT_TOLERANCE_PX,
        ];
        header.texels[TilePoseHeader::H22_QUALITY].lanes = [1.0, 2.0, 512.0, 37.0];
        header.texels[TilePoseHeader::H23_STATUS].lanes = [65_000.0, 3.0, 4.0, 2.0];
        header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes = [0.0, 0.0, 192.0, 0.0];
        header.texels[TilePoseHeader::H25_PROVENANCE].lanes = [17.0, 0.0, 1.0, 5.0];
        header.texels[TilePoseHeader::H26_OWNERSHIP].lanes = [6.0, 2.0, 31.0, 32.0];
        header
    }

    fn footprint_header(source: &Pose) -> TilePoseHeader {
        let rect = SourcePixelRect::from_extent(0, 0, source.grid_width, source.grid_height);
        let render = TileRenderKey::from_pose(source, rect);
        pack_descriptor_header(&render, &policy_header())
            .expect("frozen footprint source header packs")
    }

    fn footprint_content(source: &Pose, slice: SliceIdentity) -> TileContentKey {
        TileContentKey {
            version: TileContentKey::VERSION,
            slice,
            main_generation: source.orbit_generation,
            iteration_cap: 512,
            formula_abi: 1,
            precision_mode: PrecisionMode::PictureFast,
            record_abi: 1,
            reference_generation: 5,
        }
    }

    fn footprint_sample(source: &Pose, source_pixel: [f64; 2]) -> DescriptorFootprintSample {
        let record = EscapeGridRecord {
            smooth_iter: 128.0,
            escaped: 1.0,
            rebase_count: 3.0,
            status: 0.0,
        };
        let value = retained_value_sample(record, 512).expect("footprint value is finite");
        let depth = source_depth_record(source, source_pixel, value)
            .expect("footprint source receipt is finite");
        let descriptor = pack_descriptor_sample(record, depth).expect("footprint sample packs");
        DescriptorFootprintSample {
            source_pixel,
            descriptor,
        }
    }

    fn rotated_footprint_slice(source: &Pose) -> SliceIdentity {
        const DIAGONAL: f32 = 0.707_106_77;
        let plane = Plane {
            basis_u: [0.0, 0.0, DIAGONAL, DIAGONAL],
            basis_v: [0.0, 0.0, -DIAGONAL, DIAGONAL],
        };
        let mut origin = source.plane_origin;
        origin[2] += 0.25;
        origin[3] -= 0.5;
        SliceIdentity::new(plane, origin)
    }

    fn set_anchor_delta(header: &mut TilePoseHeader, delta: [f64; 2]) {
        let x = pack_split(delta[0]).expect("finite exact anchor x splits");
        let y = pack_split(delta[1]).expect("finite exact anchor y splits");
        header.texels[TilePoseHeader::H17_ANCHOR_DELTA].lanes = [x[0], x[1], y[0], y[1]];
    }

    fn byte_pin_pose() -> Pose {
        Pose {
            epoch: 11,
            orbit_generation: 23,
            plane: construct_plane(ObjectAngles::IDENTITY)
                .expect("byte fixture has a finite plane"),
            object: ObjectAngles::IDENTITY,
            plane_origin: [1.0, -2.0, 0.5, -0.25],
            zoom_log2: 4.0,
            view: ViewControls::NEUTRAL,
            grid_width: 256,
            grid_height: 128,
            map: PoseMap::Mapped(Homography::IDENTITY),
            centre_from_reference_px: [0.0; 2],
        }
    }

    fn footprint_source_pose() -> Pose {
        let mut source = byte_pin_pose();
        source.view = ViewControls::MANDELBROT_FLAT;
        rebuild_map(&mut source);
        source
    }

    fn rounded_density_source_pose() -> Pose {
        let mut source = footprint_source_pose();
        source.view.camera[0] = 0.17;
        source.view.camera[1] += 0.1;
        rebuild_map(&mut source);
        source
    }

    fn expected_byte_pin_header() -> TilePoseHeader {
        let mut expected = TilePoseHeader::zeroed();
        expected.texels[TilePoseHeader::H00_IDENTITIES].lanes = [41.0, 42.0, 73.0, 1.0];
        expected.texels[TilePoseHeader::H01_SPANS].lanes = [7.0, 8.0, 0.0, 9.0];
        expected.texels[TilePoseHeader::H02_OBJECT_12_13].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H03_OBJECT_14_23].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H04_OBJECT_24_34].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H05_CAMERA_12_13].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H06_CAMERA_14_23].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H07_CAMERA_24_34].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H08_CAMERA_15_25].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H09_CAMERA_35_45].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H10_OBSERVER].lanes = [1.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H11_ORIGIN_HIGH].lanes = [1.0, -2.0, 0.5, -0.25];
        expected.texels[TilePoseHeader::H12_ORIGIN_LOW].lanes = [0.0; 4];
        expected.texels[TilePoseHeader::H13_TRANSLATION_0_3].lanes = [0.0; 4];
        expected.texels[TilePoseHeader::H14_PROJECTION].lanes = [0.0, 0.0, 8.0, 8.0];
        expected.texels[TilePoseHeader::H15_EXTENT_DENSITY].lanes = [4.0, 256.0, 128.0, 0.0];
        expected.texels[TilePoseHeader::H16_SOURCE_RECT].lanes = [-3.0, 5.0, 64.0, 32.0];
        expected.texels[TilePoseHeader::H17_ANCHOR_DELTA].lanes = [0.25, 0.0, -0.5, 0.0];
        expected.texels[TilePoseHeader::H18_SOURCE_MAP_0].lanes = [1.0, 0.0, 0.0, 0.0];
        expected.texels[TilePoseHeader::H19_SOURCE_MAP_1].lanes = [0.0, 1.0, 0.0, 0.0];
        expected.texels[TilePoseHeader::H20_SOURCE_MAP_2].lanes = [0.0, 0.0, 1.0, 0.0];
        expected.texels[TilePoseHeader::H21_BOUNDS].lanes = [1.0, 12.0, 0.01, 0.25];
        expected.texels[TilePoseHeader::H22_QUALITY].lanes = [1.0, 2.0, 512.0, 37.0];
        expected.texels[TilePoseHeader::H23_STATUS].lanes = [65_000.0, 3.0, 4.0, 2.0];
        expected.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes = [0.015_625, 0.0, 192.0, 11.0];
        expected.texels[TilePoseHeader::H25_PROVENANCE].lanes = [17.0, 23.0, 1.0, 5.0];
        expected.texels[TilePoseHeader::H26_OWNERSHIP].lanes = [6.0, 2.0, 31.0, 32.0];
        expected
    }

    #[test]
    fn slice_chart_transform_has_pinned_version_and_size() {
        assert_eq!(SliceChartTransform::VERSION, 1);
        assert_eq!(SliceChartTransform::BYTE_SIZE, 64);
        assert_eq!(size_of::<SliceChartTransform>(), 64);
    }

    #[test]
    fn footprint_records_have_pinned_versions_and_sizes() {
        assert_eq!(DescriptorFootprintSample::VERSION, 1);
        assert_eq!(DescriptorFootprintSample::BYTE_SIZE, 48);
        assert_eq!(size_of::<DescriptorFootprintSample>(), 48);
        assert_eq!(DerivedChartFootprint::VERSION, 1);
        assert_eq!(DerivedChartFootprint::BYTE_SIZE, 128);
        assert_eq!(size_of::<DerivedChartFootprint>(), 128);
    }

    #[test]
    fn derived_footprint_conservatively_covers_every_reconstructed_corpus_sample() {
        let source = footprint_source_pose();
        let header = footprint_header(&source);
        let slice = rotated_footprint_slice(&source);
        let content = footprint_content(&source, slice);
        let samples = FOOTPRINT_CORPUS.map(|pixel| footprint_sample(&source, pixel));
        let footprint = derive_chart_footprint(&content, &header, &samples)
            .expect("same-slice corpus derives one footprint");
        assert_eq!(footprint.slice, slice);
        assert_eq!(footprint.valid_sample_count, 9);

        let source_error = f64::from(SOURCE_DEPTH_RECEIPT_TOLERANCE);
        let expected_error = source_error * 2.0_f64.sqrt();
        assert!((footprint.coordinate_error - expected_error).abs() <= 4.0 * f64::EPSILON);
        assert!(footprint.density_minimum < 64.0);
        assert!(footprint.density_maximum > 64.0);
        assert!(64.0 - footprint.density_minimum <= IDENTITY_DENSITY_INTERVAL_BOUND);
        assert!(footprint.density_maximum - 64.0 <= IDENTITY_DENSITY_INTERVAL_BOUND);

        for sample in samples {
            let (reconstructed, _) =
                reconstruct_descriptor_sample(&header, &sample.descriptor, sample.source_pixel)
                    .expect("frozen corpus sample reconstructs");
            let (coordinate, residual) =
                project_ambient_to_slice(slice, reconstructed.ambient_four)
                    .expect("reconstructed sample maps into the canonical chart");
            assert!(residual <= footprint.coordinate_error);
            assert!(footprint.contains(coordinate));
            for ((minimum, maximum), value) in footprint
                .conservative_minimum
                .iter()
                .zip(&footprint.conservative_maximum)
                .zip(coordinate)
            {
                assert!(*minimum <= value - source_error);
                assert!(*maximum >= value + source_error);
            }
        }
    }

    #[test]
    fn density_interval_covers_multi_operation_rounding_regression() {
        const EXACT_MINIMUM_FLOOR: f64 = f64::from_bits(0x4050_4fd2_1e53_46e3);
        const EXACT_MAXIMUM_CEILING: f64 = f64::from_bits(0x4050_9345_fbf6_67bf);

        // The reviewer's packed projective tuple is stopped by the R3-02 SourceRoundTrip receipt
        // before density. This pose-derived map crosses that gate while rotation and
        // foreshortening exercise the complete non-identity density chain.
        let source = rounded_density_source_pose();
        let header = footprint_header(&source);
        let slice = SliceIdentity::new(source.plane, source.plane_origin);
        let content = footprint_content(&source, slice);
        let source_pixel = [95.75, 47.75];
        let samples = [footprint_sample(&source, source_pixel)];
        let footprint = derive_chart_footprint(&content, &header, &samples)
            .expect("pose-derived source derives one density interval");

        // Exact arithmetic over the packed map puts the lower endpoint above the floor and the
        // upper endpoint below the ceiling, so covering both constants proves outwardness.
        assert!(footprint.density_minimum <= EXACT_MINIMUM_FLOOR);
        assert!(footprint.density_maximum >= EXACT_MAXIMUM_CEILING);
        assert!(footprint.density_minimum > 65.24);
        assert!(footprint.density_maximum < 66.31);
    }

    #[test]
    fn density_interval_refuses_zero_denominator_and_determinant_intervals() {
        let mut source = footprint_source_pose();
        source.map = PoseMap::Mapped(Homography {
            rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            ..Homography::IDENTITY
        });
        assert!(source_density_interval(&source, [0.0; 2]).is_none());

        source.map = PoseMap::Mapped(Homography {
            rows: [1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            ..Homography::IDENTITY
        });
        assert!(source_density_interval(&source, [0.0; 2]).is_none());
    }

    #[test]
    fn footprint_derivation_skips_holes_and_refuses_corrupt_or_mismatched_inputs() {
        let source = footprint_source_pose();
        let header = footprint_header(&source);
        let content = footprint_content(&source, rotated_footprint_slice(&source));
        let samples = FOOTPRINT_CORPUS.map(|pixel| footprint_sample(&source, pixel));
        let mut with_hole = samples;
        with_hole[4].descriptor.s1.lanes[3] = 0.0;
        let footprint = derive_chart_footprint(&content, &header, &with_hole)
            .expect("one visibility hole leaves a conservative footprint");
        assert_eq!(footprint.valid_sample_count, 8);

        let all_holes = samples.map(|mut sample| {
            sample.descriptor.s1.lanes[3] = 0.0;
            sample
        });
        assert_eq!(
            derive_chart_footprint(&content, &header, &all_holes),
            Err(ReprojectionError::InvalidSource)
        );
        let mut corrupt = samples;
        corrupt[0].descriptor.s1.lanes[3] = 0.5;
        assert_eq!(
            derive_chart_footprint(&content, &header, &corrupt),
            Err(ReprojectionError::InvalidSource)
        );
        let wrong_main = TileContentKey {
            main_generation: content.main_generation + 1,
            ..content
        };
        assert_eq!(
            derive_chart_footprint(&wrong_main, &header, &samples),
            Err(ReprojectionError::InvalidSource)
        );
        let tilted = TileContentKey {
            slice: SliceIdentity::new(Plane::CANONICAL_JULIA_PLANE, source.plane_origin),
            ..content
        };
        assert_eq!(
            derive_chart_footprint(&tilted, &header, &samples),
            Err(ReprojectionError::InvalidTarget)
        );
    }

    #[test]
    fn same_slice_certification_maps_in_plane_origins_and_refuses_tilts() {
        let source_plane = Plane::CANONICAL_JULIA_PLANE;
        let canonical_plane = Plane {
            basis_u: [0.0, 1.0, 0.0, 0.0],
            basis_v: [-1.0, 0.0, 0.0, 0.0],
        };
        let source_origin = [3.0, -2.0, 0.0, 0.0];
        let canonical_origin = [1.0, 4.0, 0.0, 0.0];
        let source = SliceIdentity::new(source_plane, source_origin);
        let canonical = SliceIdentity::new(canonical_plane, canonical_origin);
        let transform = certify_same_slice(source, canonical, 0.01)
            .expect("rotated basis and in-plane origin remain the same slice");
        for (actual, expected) in transform.chart_map.into_iter().zip([0.0, 1.0, -1.0, 0.0]) {
            assert!((actual - expected).abs() <= f64::EPSILON);
        }
        assert_eq!(transform.origin_offset, [-6.0, -2.0]);
        assert_eq!(transform.out_of_plane_error, 0.0);
        assert_eq!(transform.maximum_error, 0.0);

        let source_coordinate = [0.25, -0.75];
        let source_local = source_plane.local_point(source_coordinate);
        let ambient: [f64; 4] =
            core::array::from_fn(|axis| source_origin[axis] + source_local[axis]);
        let relative: [f64; 4] =
            core::array::from_fn(|axis| ambient[axis] - canonical_origin[axis]);
        let (direct, residual) = project_vector_to_plane(canonical_plane, relative)
            .expect("same-slice ambient point has canonical coordinates");
        assert_eq!(residual, 0.0);
        for (actual, expected) in transform
            .map_source_coordinate(source_coordinate)
            .into_iter()
            .zip(direct)
        {
            assert!((actual - expected).abs() <= 4.0 * f64::EPSILON);
        }

        let boundary = SliceIdentity::new(canonical_plane, [1.0, 4.0, 0.5, 0.0]);
        assert!(certify_same_slice(source, boundary, 1.0).is_none());
        let in_plane_boundary =
            SliceIdentity::new(canonical_plane, [1.5, 4.0, 0.0, 0.0]);
        let in_plane_transform = certify_same_slice(source, in_plane_boundary, 1.0)
            .expect("half-source-sample in-plane pan remains the same slice");
        assert_eq!(in_plane_transform.out_of_plane_error, 0.0);
        assert_eq!(in_plane_transform.maximum_error, 0.0);
        let beyond = SliceIdentity::new(canonical_plane, [1.0, 4.0, 0.5_f64.next_up(), 0.0]);
        assert!(certify_same_slice(source, beyond, 1.0).is_none());

        let tilted = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 0.0, 1.0, 0.0],
        };
        let tilted = SliceIdentity::new(tilted, canonical_origin);
        assert!(certify_same_slice(source, tilted, 0.01).is_none());
        assert!(certify_same_slice(source, canonical, 0.0).is_none());
        assert!(certify_same_slice(source, canonical, f64::NAN).is_none());
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
    fn descriptor_header_packing_matches_all_512_expected_bytes() {
        let pose = byte_pin_pose();
        let rect = SourcePixelRect::from_extent(-3, 5, 64, 32);
        let source = SourceIdentity::new(73, 11, 19);
        let render = TileRenderKey::from_pose_and_source(&pose, rect, source);
        let mut policy = policy_header();
        set_anchor_delta(&mut policy, [0.25, -0.5]);
        let packed =
            pack_descriptor_header(&render, &policy).expect("exact byte fixture descriptor packs");
        let expected = expected_byte_pin_header();
        assert_eq!(bytes_of(&packed), bytes_of(&expected));
        assert_eq!(bytes_of(&packed).len(), TilePoseHeader::BYTE_SIZE);
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

        let packed = pack_descriptor_header(&render, &policy_header())
            .expect("finite descriptor source pose packs");
        let mut invalid_yaw = packed;
        invalid_yaw.texels[TilePoseHeader::H10_OBSERVER].lanes[..2].copy_from_slice(&[0.0; 2]);
        assert!(unpack_descriptor_header(&invalid_yaw).is_none());
        let mut invalid_pitch = packed;
        invalid_pitch.texels[TilePoseHeader::H10_OBSERVER].lanes[2..].copy_from_slice(&[0.0; 2]);
        assert!(unpack_descriptor_header(&invalid_pitch).is_none());
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

        let (unpacked_record, unpacked_depth) =
            unpack_descriptor_sample(&pair).expect("exact-in-f32 sample lanes unpack");
        assert_eq!(unpacked_record, record);
        assert_eq!(unpacked_depth, depth);

        let mut non_binary = pair;
        non_binary.s1.lanes[3] = 0.5;
        assert_eq!(
            unpack_descriptor_sample(&non_binary),
            Err(ReprojectionError::InvalidSource)
        );
        let zero_depth = SourceDepthRecord {
            zeta_f: 0.0,
            ..depth
        };
        assert!(pack_descriptor_sample(record, zero_depth).is_none());
    }

    #[test]
    fn descriptor_source_reconstruction_returns_pixel_and_depth() {
        let source = source_pose();
        let source_pixel = [37.5, -21.5];
        let record = EscapeGridRecord {
            smooth_iter: 128.0,
            escaped: 1.0,
            rebase_count: 3.0,
            status: 0.0,
        };
        let value = retained_value_sample(record, 512).expect("value record has a finite height");
        let depth = source_depth_record(&source, source_pixel, value)
            .expect("flat source sample has a finite depth receipt");
        let rect = SourcePixelRect::from_extent(0, 0, source.grid_width, source.grid_height);
        let render = TileRenderKey::from_pose(&source, rect);
        let header = pack_descriptor_header(&render, &policy_header())
            .expect("source descriptor header packs");
        let pair = pack_descriptor_sample(record, depth).expect("source sample pair packs");

        let (reconstructed, source_receipt) =
            reconstruct_descriptor_sample(&header, &pair, source_pixel)
                .expect("packed source sample passes its declared receipt");
        let source_error = (source_receipt.screen[0] - source_pixel[0])
            .hypot(source_receipt.screen[1] - source_pixel[1]);
        assert!(source_error <= f64::from(SOURCE_PIXEL_RECEIPT_TOLERANCE_PX));
        assert!(
            (source_receipt.linear_depth - depth.zeta_f).abs()
                <= f64::from(SOURCE_DEPTH_RECEIPT_TOLERANCE)
        );
        assert_eq!(reconstructed.value, value);

        let exact = reconstruct_source_sample(&source, source_pixel, depth, value)
            .expect("binary64 source fixture round-trips");
        for (actual, expected) in reconstructed
            .ambient_four
            .into_iter()
            .zip(exact.ambient_four)
        {
            assert!((actual - expected).abs() <= SOURCE_COORDINATE_TOLERANCE);
        }
    }

    #[test]
    fn descriptor_requested_projection_matches_direct_f64_and_refuses_poles() {
        let source = source_pose();
        let target = requested_pose();
        let source_pixel = [37.5, -21.5];
        let record = EscapeGridRecord {
            smooth_iter: 128.0,
            escaped: 1.0,
            rebase_count: 3.0,
            status: 0.0,
        };
        let value = retained_value_sample(record, 512).expect("value record has a finite height");
        let depth = source_depth_record(&source, source_pixel, value)
            .expect("flat source sample has a finite depth receipt");
        let rect = SourcePixelRect::from_extent(0, 0, source.grid_width, source.grid_height);
        let render = TileRenderKey::from_pose(&source, rect);
        let header = pack_descriptor_header(&render, &policy_header())
            .expect("source descriptor header packs");
        let pair = pack_descriptor_sample(record, depth).expect("source sample pair packs");

        let exact = reconstruct_source_sample(&source, source_pixel, depth, value)
            .expect("binary64 source fixture round-trips");
        let direct = project_reconstructed_sample(&target, exact)
            .expect("binary64 source sample projects to the requested pose");
        let projected = project_descriptor_sample(&header, &pair, source_pixel, &target)
            .expect("descriptor sample projects through the requested pose");
        let target_error =
            (projected.screen[0] - direct.screen[0]).hypot(projected.screen[1] - direct.screen[1]);
        assert!(target_error <= TARGET_PROJECTION_TOLERANCE_PX);
        assert!((projected.linear_depth - direct.linear_depth).abs() <= TARGET_DEPTH_TOLERANCE);
        assert!(
            (projected.raster_depth - direct.raster_depth).abs() <= TARGET_RASTER_DEPTH_TOLERANCE
        );

        let mut edge = target;
        edge.map = PoseMap::EdgeOn;
        assert_eq!(
            project_descriptor_sample(&header, &pair, source_pixel, &edge),
            Err(ReprojectionError::ProjectionPole)
        );
        let mut pole = target;
        pole.view.distance_four = ProjectedSample::POLE_EPSILON * 0.5;
        assert_eq!(
            project_descriptor_sample(&header, &pair, source_pixel, &pole),
            Err(ReprojectionError::ProjectionPole)
        );
    }

    #[test]
    fn maximal_zoom_projection_applies_certified_exact_anchor_delta() {
        let mut source = source_pose();
        source.zoom_log2 = MAXIMAL_DESCRIPTOR_ZOOM_LOG2 - 1.0;
        rebuild_map(&mut source);
        let mut target = requested_pose();
        target.zoom_log2 = MAXIMAL_DESCRIPTOR_ZOOM_LOG2;
        target.plane_origin = source.plane_origin;
        rebuild_map(&mut target);
        assert_eq!(source.plane_origin, target.plane_origin);

        let source_pixel = [37.5, -21.5];
        let record = EscapeGridRecord {
            smooth_iter: 128.0,
            escaped: 1.0,
            rebase_count: 3.0,
            status: 0.0,
        };
        let value = retained_value_sample(record, 512).expect("value record has a finite height");
        let depth = source_depth_record(&source, source_pixel, value)
            .expect("maximal-zoom source sample has a finite receipt");
        let exact = reconstruct_source_sample(&source, source_pixel, depth, value)
            .expect("maximal-zoom binary64 source sample round-trips");
        let anchor_delta_px = [0.25, -0.5];
        let scale_ratio = (target.zoom_log2 - source.zoom_log2).exp2();
        let chart_scale = 4.0 / f64::from(target.grid_width);
        let anchor_chart = anchor_delta_px.map(|value| chart_scale * value);
        let anchor_local = target.plane.local_point(anchor_chart);
        let requested_local: [f64; 4] = core::array::from_fn(|axis| {
            scale_ratio.mul_add(exact.source_local_four[axis], anchor_local[axis])
        });
        let direct = ProjectedSample::from_local_point(&target, requested_local, value)
            .expect("direct maximal-zoom requested point projects");

        let rect = SourcePixelRect::from_extent(0, 0, source.grid_width, source.grid_height);
        let render = TileRenderKey::from_pose(&source, rect);
        let mut policy = policy_header();
        set_anchor_delta(&mut policy, anchor_delta_px);
        let header =
            pack_descriptor_header(&render, &policy).expect("maximal-zoom descriptor header packs");
        let pair = pack_descriptor_sample(record, depth).expect("source sample pair packs");
        let projected = project_descriptor_sample(&header, &pair, source_pixel, &target)
            .expect("certified maximal-zoom placement projects");
        let target_error =
            (projected.screen[0] - direct.screen[0]).hypot(projected.screen[1] - direct.screen[1]);
        assert!(target_error <= TARGET_PROJECTION_TOLERANCE_PX);

        // Independent recomputation isolates 0.3031873096 px of H17 movement; the former
        // unscaled baseline mixed zoom into the comparison and measured 28.7457515889 px.
        let scaled_without_anchor = exact
            .project_from_anchor(&target, [0.0; 2], 0.0)
            .expect("zero-anchor maximal-zoom placement projects");
        let anchor_effect = (projected.screen[0] - scaled_without_anchor.screen[0])
            .hypot(projected.screen[1] - scaled_without_anchor.screen[1]);
        assert!(anchor_effect >= ANCHOR_EFFECT_MINIMUM_PX);

        let mut uncertified_policy = policy_header();
        uncertified_policy.texels[TilePoseHeader::H17_ANCHOR_DELTA].lanes =
            [f32::MAX, 33_554_432.0, 0.0, 0.0];
        let uncertified_header = pack_descriptor_header(&render, &uncertified_policy)
            .expect("finite but uncertifiable anchor split packs");
        assert_eq!(
            project_descriptor_sample(&uncertified_header, &pair, source_pixel, &target),
            Err(ReprojectionError::UncertifiedPlacement)
        );
    }

    #[test]
    fn large_exact_anchor_refuses_a_uniformly_mis_scaled_target_plane() {
        let mut source = source_pose();
        source.zoom_log2 = MAXIMAL_DESCRIPTOR_ZOOM_LOG2 - 1.0;
        rebuild_map(&mut source);
        let mut target = requested_pose();
        target.zoom_log2 = MAXIMAL_DESCRIPTOR_ZOOM_LOG2;
        target.plane_origin = source.plane_origin;
        rebuild_map(&mut target);

        let anchor_delta_px = [33_554_432.0, 0.0];
        assert_eq!(pack_split(anchor_delta_px[0]), Some([33_554_432.0, 0.0]));
        let scale_ratio = (target.zoom_log2 - source.zoom_log2).exp2();
        let chart_scale = 4.0 / f64::from(target.grid_width);
        let anchor_chart = anchor_delta_px.map(|value| chart_scale * value);
        let source_coordinate = anchor_chart.map(|value| -value / scale_ratio);
        let source_local_four = target.plane.local_point(source_coordinate);
        let ambient_four =
            core::array::from_fn(|axis| source.plane_origin[axis] + source_local_four[axis]);
        let value = retained_value_sample(
            EscapeGridRecord {
                smooth_iter: 0.0,
                escaped: 0.0,
                rebase_count: 0.0,
                status: 0.0,
            },
            1,
        )
        .expect("interior record has the cancellation fixture's minus-two height");
        let sample = ReconstructedSample {
            ambient_four,
            source_local_four,
            source_zoom_log2: source.zoom_log2,
            value,
        };
        sample
            .project_from_anchor(&target, anchor_delta_px, 0.0)
            .expect("constructed target accepts the large-anchor cancellation fixture");

        // A one-epsilon uniform basis scale moved this independently recomputed target by
        // 1.166644886023508 px, beyond the one-pixel placement contract.
        target.plane.basis_u = target
            .plane
            .basis_u
            .map(|component| component * (1.0 + f32::EPSILON));
        assert_eq!(
            sample.project_from_anchor(&target, anchor_delta_px, 0.0),
            Err(ReprojectionError::InvalidTarget)
        );
    }
}
