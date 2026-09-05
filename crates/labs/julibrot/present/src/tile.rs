//! Stage-0 rendered-view identity, descriptor ABI, ownership, and invalidation oracles.

use core::cmp::Ordering;

use bytemuck::{Pod, Zeroable};
use ember_julibrot_math::{ObjectAngles, Plane, Pose, PoseMap, PrecisionMode, ViewControls};
use thiserror::Error;

/// Exact-equality binary64 key lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactF64(u64);

impl ExactF64 {
    /// Captures one binary64 value without normalization.
    #[must_use]
    pub const fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    /// Restores the captured binary64 value.
    #[must_use]
    pub const fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

/// Exact-equality binary32 key lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactF32(u32);

impl ExactF32 {
    /// Captures one binary32 value without normalization.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self(value.to_bits())
    }
}

/// Exact semantic identity of one canonical affine slice.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SliceIdentity {
    /// Once-rounded first plane basis vector.
    pub basis_u: [ExactF32; 4],
    /// Once-rounded second plane basis vector.
    pub basis_v: [ExactF32; 4],
    /// Exact finite mirror of the defining plane origin.
    pub origin: [ExactF64; 4],
}

impl SliceIdentity {
    /// Captures the slice components used by current presentation state.
    #[must_use]
    pub fn new(plane: Plane, origin: [f64; 4]) -> Self {
        Self {
            basis_u: plane.basis_u.map(ExactF32::new),
            basis_v: plane.basis_v.map(ExactF32::new),
            origin: origin.map(ExactF64::new),
        }
    }
}

/// Versioned semantic content key for a rendered view or tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileContentKey {
    /// Content-key schema version.
    pub version: u32,
    /// Canonical sampled slice.
    pub slice: SliceIdentity,
    /// MAIN generation whose records were accepted.
    pub main_generation: u32,
    /// Delivered iteration-cap semantics.
    pub iteration_cap: u32,
    /// Formula semantics used to interpret the value records.
    pub formula_abi: u32,
    /// Precision policy used to produce values.
    pub precision_mode: PrecisionMode,
    /// Existing escape-record interpretation version.
    pub record_abi: u32,
    /// Strict version-one reference generation.
    pub reference_generation: u32,
}

impl TileContentKey {
    /// Stage-0 content-key schema.
    pub const VERSION: u32 = 1;
}

/// Integer source-screen rectangle retained by one rendered tile.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourcePixelRect {
    /// Left source pixel.
    pub x: u32,
    /// Bottom source pixel.
    pub y: u32,
    /// Physical source width including any apron.
    pub width: u32,
    /// Physical source height including any apron.
    pub height: u32,
}

impl SourcePixelRect {
    /// Physical side of a version-one rendered tile, including aprons.
    pub const PHYSICAL_SIDE: u32 = 256;
    /// Drawn core side of a version-one rendered tile.
    pub const CORE_SIDE: u32 = 254;
    /// Retained sample apron on each core edge.
    pub const APRON_SAMPLES: u32 = 1;
}

/// Exact key form of a mapped or edge-on source screen transform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PoseMapKey {
    /// Complete accepted source map, including its explicit inverse and apron.
    Mapped([ExactF64; 20]),
    /// Physical edge-on source pose.
    EdgeOn,
}

impl From<PoseMap> for PoseMapKey {
    fn from(value: PoseMap) -> Self {
        match value {
            PoseMap::Mapped(map) => {
                let mut lanes = [ExactF64::new(0.0); 20];
                for (target, value) in lanes.iter_mut().zip(
                    map.rows
                        .into_iter()
                        .chain(map.inverse)
                        .chain([map.condition_number, map.apron_scale]),
                ) {
                    *target = ExactF64::new(value);
                }
                Self::Mapped(lanes)
            }
            PoseMap::EdgeOn => Self::EdgeOn,
        }
    }
}

/// Versioned exact source-render identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TileRenderKey {
    /// Render-key schema version.
    pub version: u32,
    /// Six ordered object angles `12,13,14,23,24,34`.
    pub object: [ExactF64; 6],
    /// Source plane origin.
    pub origin: [ExactF64; 4],
    /// Ten ordered source camera angles.
    pub camera: [ExactF64; 10],
    /// Source five-dimensional camera translation.
    pub translation: [ExactF64; 5],
    /// Source height amplitude.
    pub height: ExactF64,
    /// Source five-to-four perspective distance.
    pub distance_five: ExactF64,
    /// Source four-to-three perspective distance.
    pub distance_four: ExactF64,
    /// Source observer yaw.
    pub yaw: ExactF64,
    /// Source observer pitch.
    pub pitch: ExactF64,
    /// Source base-two zoom exponent.
    pub zoom: ExactF64,
    /// Source canvas extent.
    pub extent: [u32; 2],
    /// Retained source-screen rectangle.
    pub source_rect: SourcePixelRect,
    /// Exact source map accepted for the render.
    pub source_map: PoseMapKey,
    /// Canonical sampled slice identity.
    pub slice: SliceIdentity,
    /// MAIN generation whose pose was rendered.
    pub main_generation: u32,
}

impl TileRenderKey {
    /// Stage-0 render-key schema.
    pub const VERSION: u32 = 1;

    /// Captures every source-pose field that can affect rendered geometry.
    #[must_use]
    pub fn from_pose(pose: &Pose, source_rect: SourcePixelRect) -> Self {
        let ViewControls {
            camera,
            camera_translation,
            camera_yaw,
            camera_pitch,
            height_scale,
            distance_five,
            distance_four,
        } = pose.view;
        Self {
            version: Self::VERSION,
            object: object_array(pose.object).map(ExactF64::new),
            origin: pose.plane_origin.map(ExactF64::new),
            camera: camera.map(ExactF64::new),
            translation: camera_translation.map(ExactF64::new),
            height: ExactF64::new(height_scale),
            distance_five: ExactF64::new(distance_five),
            distance_four: ExactF64::new(distance_four),
            yaw: ExactF64::new(camera_yaw),
            pitch: ExactF64::new(camera_pitch),
            zoom: ExactF64::new(pose.zoom_log2),
            extent: [pose.grid_width, pose.grid_height],
            source_rect,
            source_map: pose.map.into(),
            slice: SliceIdentity::new(pose.plane, pose.plane_origin),
            main_generation: pose.orbit_generation,
        }
    }
}

const fn object_array(object: ObjectAngles) -> [f64; 6] {
    [
        object.rho_12,
        object.rho_13,
        object.rho_14,
        object.rho_23,
        object.rho_24,
        object.rho_34,
    ]
}

/// One physical RGBA32F descriptor-map texel.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct DescriptorTexel {
    /// Four binary32 lanes in the documented header or sample order.
    pub lanes: [f32; 4],
}

/// Exact version-one 32-texel rendered-tile pose header.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct TilePoseHeader {
    /// Header texels `H00` through `H31`; `H27` through `H31` must remain zero.
    pub texels: [DescriptorTexel; 32],
}

impl TilePoseHeader {
    /// Number of RGBA32F texels in one header slot.
    pub const TEXELS: usize = 32;
    /// First reserved header texel.
    pub const RESERVED_START: usize = 27;
    /// `H00`: stable identities and flags.
    pub const H00_IDENTITIES: usize = 0;
    /// `H01`: sample spans, ownership base, and header generation.
    pub const H01_SPANS: usize = 1;
    /// `H02`: object factors 12 and 13.
    pub const H02_OBJECT_12_13: usize = 2;
    /// `H03`: object factors 14 and 23.
    pub const H03_OBJECT_14_23: usize = 3;
    /// `H04`: object factors 24 and 34.
    pub const H04_OBJECT_24_34: usize = 4;
    /// `H05`: camera factors 12 and 13.
    pub const H05_CAMERA_12_13: usize = 5;
    /// `H06`: camera factors 14 and 23.
    pub const H06_CAMERA_14_23: usize = 6;
    /// `H07`: camera factors 24 and 34.
    pub const H07_CAMERA_24_34: usize = 7;
    /// `H08`: camera factors 15 and 25.
    pub const H08_CAMERA_15_25: usize = 8;
    /// `H09`: camera factors 35 and 45.
    pub const H09_CAMERA_35_45: usize = 9;
    /// `H10`: observer yaw and pitch factors.
    pub const H10_OBSERVER: usize = 10;
    /// `H11`: high source-origin lanes.
    pub const H11_ORIGIN_HIGH: usize = 11;
    /// `H12`: low source-origin lanes.
    pub const H12_ORIGIN_LOW: usize = 12;
    /// `H13`: source translations zero through three.
    pub const H13_TRANSLATION_0_3: usize = 13;
    /// `H14`: fifth translation, height, and perspective distances.
    pub const H14_PROJECTION: usize = 14;
    /// `H15`: zoom, extent, and chart density.
    pub const H15_EXTENT_DENSITY: usize = 15;
    /// `H16`: integer source rectangle.
    pub const H16_SOURCE_RECT: usize = 16;
    /// `H17`: compensated target-relative anchor delta.
    pub const H17_ANCHOR_DELTA: usize = 17;
    /// `H18`: accepted source-map row zero.
    pub const H18_SOURCE_MAP_0: usize = 18;
    /// `H19`: accepted source-map row one.
    pub const H19_SOURCE_MAP_1: usize = 19;
    /// `H20`: accepted source-map row two.
    pub const H20_SOURCE_MAP_2: usize = 20;
    /// `H21`: depth and error bounds.
    pub const H21_BOUNDS: usize = 21;
    /// `H22`: same-surface quality and scheduling facts.
    pub const H22_QUALITY: usize = 22;
    /// `H23`: sample status and mesh class.
    pub const H23_STATUS: usize = 23;
    /// `H24`: chart scale and exact-anchor provenance.
    pub const H24_SCALE_ANCHOR: usize = 24;
    /// `H25`: semantic and record provenance.
    pub const H25_PROVENANCE: usize = 25;
    /// `H26`: ownership and sample generations.
    pub const H26_OWNERSHIP: usize = 26;

    /// Validates the version-one reserved region.
    #[must_use]
    pub fn reserved_lanes_are_zero(&self) -> bool {
        self.texels[Self::RESERVED_START..]
            .iter()
            .flat_map(|texel| texel.lanes)
            .all(|lane| lane.to_bits() == 0.0_f32.to_bits())
    }
}

/// Exact two-RGBA32F sample pair `S0/S1`.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct DescriptorSamplePair {
    /// Existing escape-value record, byte-for-byte and lane-for-lane unchanged.
    pub s0: DescriptorTexel,
    /// `(a_F,b_F,zeta_F,validity)` lifted source reconstruction record.
    pub s1: DescriptorTexel,
}

impl DescriptorSamplePair {
    /// Packs the existing value record and the decided lifted record.
    #[must_use]
    pub const fn new(value: [f32; 4], lifted: [f32; 4]) -> Self {
        Self {
            s0: DescriptorTexel { lanes: value },
            s1: DescriptorTexel { lanes: lifted },
        }
    }
}

/// Version-one descriptor-map resource arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorCostLedger;

impl DescriptorCostLedger {
    /// Descriptor ABI version.
    pub const ABI_VERSION: u32 = 1;
    /// Bytes in one RGBA32F texel.
    pub const TEXEL_BYTES: u64 = 16;
    /// Physical sample count in one 256-square tile.
    pub const SAMPLES_PER_TILE: u64 =
        SourcePixelRect::PHYSICAL_SIDE as u64 * SourcePixelRect::PHYSICAL_SIDE as u64;
    /// Active-instance records at the start of the shared descriptor page.
    pub const ACTIVE_PREFIX_RECORDS: u64 = 64;
    /// Complete pose-header slots in the shared descriptor page.
    pub const HEADER_SLOTS: u64 = 64;
    /// Compact ownership records after the active prefix and header slots.
    pub const OWNERSHIP_RECORDS: u64 = 63_424;
    /// Total records in one shared 256-square descriptor page.
    pub const DESCRIPTOR_PAGE_RECORDS: u64 = 256 * 256;
    /// Bytes in the paired sample columns for one tile.
    pub const SAMPLE_BYTES_PER_TILE: u64 = 2 * Self::TEXEL_BYTES * Self::SAMPLES_PER_TILE;
    /// Bytes in one 32-texel pose header.
    pub const HEADER_BYTES_PER_TILE: u64 = 32 * Self::TEXEL_BYTES;
    /// Logical bytes in one complete resident tile.
    pub const LOGICAL_BYTES_PER_TILE: u64 =
        Self::SAMPLE_BYTES_PER_TILE + Self::HEADER_BYTES_PER_TILE;

    /// Computes the exact logical bytes for a resident tile count.
    #[must_use]
    pub const fn logical_bytes(tile_count: u64) -> Option<u64> {
        Self::LOGICAL_BYTES_PER_TILE.checked_mul(tile_count)
    }
}

/// Same-surface residency class, ordered from fallback to detail.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TileResidency {
    /// Protected coarse fallback.
    Backdrop,
    /// Ordinary detailed/history tile.
    Detail,
}

/// Same-surface refinement rung, ordered from coarse to final.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TileRung {
    /// Coarse backdrop result.
    Backdrop,
    /// Fast preview result.
    Preview,
    /// Intermediate result.
    Interactive,
    /// Final result.
    Final,
}

/// Canonical chart microcell used only to recognize competing representations of one surface.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalChartCellKey {
    /// Canonical slice containing this cell.
    pub slice: SliceIdentity,
    /// Signed dyadic pyramid level.
    pub level: i32,
    /// Signed dyadic horizontal coordinate.
    pub x: i64,
    /// Signed dyadic vertical coordinate.
    pub y: i64,
}

/// Deterministic same-surface quality tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileQuality {
    /// Detail wins over Backdrop.
    pub residency: TileResidency,
    /// Final wins over Interactive, Preview, and Backdrop.
    pub rung: TileRung,
    /// Higher certified samples-per-chart-unit wins.
    pub density: ExactF64,
    /// Lower total coordinate/depth/reprojection error wins.
    pub error: ExactF64,
    /// Newer deterministic serial wins after geometric quality.
    pub age: u64,
    /// Lower stable tile identity resolves the final exact tie.
    pub tile_id: u64,
}

impl Ord for TileQuality {
    fn cmp(&self, other: &Self) -> Ordering {
        self.residency
            .cmp(&other.residency)
            .then_with(|| self.rung.cmp(&other.rung))
            .then_with(|| self.density.get().total_cmp(&other.density.get()))
            .then_with(|| other.error.get().total_cmp(&self.error.get()))
            .then_with(|| self.age.cmp(&other.age))
            .then_with(|| other.tile_id.cmp(&self.tile_id))
    }
}

impl PartialOrd for TileQuality {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Selects one same-surface owner without depending on catalog order.
#[must_use]
pub fn select_same_surface_owner(
    candidates: impl IntoIterator<Item = TileQuality>,
) -> Option<TileQuality> {
    candidates.into_iter().max()
}

/// Control-class rows in the stage-0 invalidation matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderControlChange {
    /// Camera `Q` factor.
    Camera,
    /// Five-dimensional camera translation.
    Translation,
    /// Requested height amplitude.
    Height,
    /// Five-to-four perspective distance.
    DistanceFive,
    /// Four-to-three perspective distance.
    DistanceFour,
    /// Observer yaw or pitch.
    Observer,
    /// Requested zoom.
    Zoom,
    /// Requested canvas extent.
    Extent,
    /// Plane-preserving object parameterization.
    PlanePreservingObject,
    /// In-plane origin move.
    InPlaneOrigin,
    /// Palette, exposure, tone, or output encoding.
    Display,
    /// Slice tilt.
    SliceTilt,
    /// Out-of-plane origin move.
    OutOfPlaneOrigin,
    /// Delivered iteration-cap change.
    IterationCap,
    /// Precision-policy change.
    Precision,
    /// Escape-record ABI change.
    RecordAbi,
    /// Strict version-one reference/MAIN generation change.
    MainGeneration,
}

/// Semantic effect of one control-class change on resident rendered content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileInvalidation {
    /// Matching-content tiles remain semantically valid and may be reprojected.
    Keep,
    /// A new content partition starts; prior content may only be held unchanged.
    NewPartition,
}

/// Presentation admitted during a control transition before replacement content completes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionPresentation {
    /// Matching-content geometry may be reprojected normally.
    Reproject,
    /// Prior content may remain only as an unchanged held frame.
    HoldPrevious,
}

/// Returns the version-one invalidation row for one control class.
#[must_use]
pub const fn tile_invalidation(change: RenderControlChange) -> TileInvalidation {
    match change {
        RenderControlChange::Camera
        | RenderControlChange::Translation
        | RenderControlChange::Height
        | RenderControlChange::DistanceFive
        | RenderControlChange::DistanceFour
        | RenderControlChange::Observer
        | RenderControlChange::Zoom
        | RenderControlChange::Extent
        | RenderControlChange::PlanePreservingObject
        | RenderControlChange::InPlaneOrigin
        | RenderControlChange::Display => TileInvalidation::Keep,
        RenderControlChange::SliceTilt
        | RenderControlChange::OutOfPlaneOrigin
        | RenderControlChange::IterationCap
        | RenderControlChange::Precision
        | RenderControlChange::RecordAbi
        | RenderControlChange::MainGeneration => TileInvalidation::NewPartition,
    }
}

/// Returns the only honest transitional presentation for one invalidation row.
#[must_use]
pub const fn transition_presentation(change: RenderControlChange) -> TransitionPresentation {
    match tile_invalidation(change) {
        TileInvalidation::Keep => TransitionPresentation::Reproject,
        TileInvalidation::NewPartition => TransitionPresentation::HoldPrevious,
    }
}

/// Typed descriptor-map construction refusal.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum DescriptorAbiError {
    /// Version-one reserved lanes were not all positive zero.
    #[error("descriptor header reserved lanes must be zero")]
    ReservedHeaderLane,
}

/// Validates one exact version-one pose header.
///
/// # Errors
///
/// Returns a typed refusal when any reserved lane is nonzero.
pub fn validate_pose_header(header: &TilePoseHeader) -> Result<(), DescriptorAbiError> {
    if header.reserved_lanes_are_zero() {
        Ok(())
    } else {
        Err(DescriptorAbiError::ReservedHeaderLane)
    }
}

#[cfg(test)]
mod tests {
    use core::mem::{align_of, size_of};

    use ember_julibrot_math::{Homography, Plane};

    use super::*;

    fn slice() -> SliceIdentity {
        SliceIdentity::new(
            Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            [0.0; 4],
        )
    }

    #[test]
    fn keys_use_exact_float_equality_and_capture_the_complete_source_pose() {
        let pose = Pose {
            epoch: 9,
            orbit_generation: 17,
            plane: Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            object: ObjectAngles::JULIA,
            plane_origin: [0.0; 4],
            zoom_log2: 3.0,
            view: ViewControls::NEUTRAL,
            grid_width: 960,
            grid_height: 540,
            map: PoseMap::Mapped(Homography::IDENTITY),
            centre_from_reference_px: [0.0; 2],
        };
        let rect = SourcePixelRect {
            x: 4,
            y: 8,
            width: 256,
            height: 256,
        };
        let key = TileRenderKey::from_pose(&pose, rect);
        assert_eq!(key, TileRenderKey::from_pose(&pose, rect));
        let mut changed = pose;
        changed.zoom_log2 = f64::from_bits(pose.zoom_log2.to_bits() + 1);
        assert_ne!(key, TileRenderKey::from_pose(&changed, rect));
        changed = pose;
        changed.view.camera_translation[4] = -0.0;
        assert_ne!(key, TileRenderKey::from_pose(&changed, rect));
        assert_eq!(key.extent, [960, 540]);
        assert_eq!(key.main_generation, 17);
    }

    #[test]
    fn descriptor_map_abi_round_trips_bytes_and_matches_the_cost_ledger() {
        #[repr(C, align(16))]
        struct AlignedBytes<const N: usize>([u8; N]);

        let named_indices = [
            TilePoseHeader::H00_IDENTITIES,
            TilePoseHeader::H01_SPANS,
            TilePoseHeader::H02_OBJECT_12_13,
            TilePoseHeader::H03_OBJECT_14_23,
            TilePoseHeader::H04_OBJECT_24_34,
            TilePoseHeader::H05_CAMERA_12_13,
            TilePoseHeader::H06_CAMERA_14_23,
            TilePoseHeader::H07_CAMERA_24_34,
            TilePoseHeader::H08_CAMERA_15_25,
            TilePoseHeader::H09_CAMERA_35_45,
            TilePoseHeader::H10_OBSERVER,
            TilePoseHeader::H11_ORIGIN_HIGH,
            TilePoseHeader::H12_ORIGIN_LOW,
            TilePoseHeader::H13_TRANSLATION_0_3,
            TilePoseHeader::H14_PROJECTION,
            TilePoseHeader::H15_EXTENT_DENSITY,
            TilePoseHeader::H16_SOURCE_RECT,
            TilePoseHeader::H17_ANCHOR_DELTA,
            TilePoseHeader::H18_SOURCE_MAP_0,
            TilePoseHeader::H19_SOURCE_MAP_1,
            TilePoseHeader::H20_SOURCE_MAP_2,
            TilePoseHeader::H21_BOUNDS,
            TilePoseHeader::H22_QUALITY,
            TilePoseHeader::H23_STATUS,
            TilePoseHeader::H24_SCALE_ANCHOR,
            TilePoseHeader::H25_PROVENANCE,
            TilePoseHeader::H26_OWNERSHIP,
        ];
        assert_eq!(
            named_indices,
            core::array::from_fn::<_, 27, _>(|index| index)
        );

        let mut header_bytes = AlignedBytes([0_u8; 512]);
        for texel in 0..TilePoseHeader::RESERVED_START {
            for lane in 0..4 {
                let ordinal = u16::try_from(texel * 4 + lane + 1).expect("header lane fits");
                let start = (texel * 4 + lane) * size_of::<f32>();
                header_bytes.0[start..start + size_of::<f32>()]
                    .copy_from_slice(&f32::from(ordinal).to_ne_bytes());
            }
        }
        let header = *bytemuck::from_bytes::<TilePoseHeader>(&header_bytes.0);
        validate_pose_header(&header).expect("reserved header lanes are zero");
        assert_eq!(
            header.texels[TilePoseHeader::H00_IDENTITIES].lanes,
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            header.texels[TilePoseHeader::H26_OWNERSHIP].lanes,
            [105.0, 106.0, 107.0, 108.0]
        );
        assert_eq!(bytemuck::bytes_of(&header), header_bytes.0);

        let pair_lanes = [31.0_f32, 1.0, 2.0, 0.0, 0.25, -0.5, 7.0, 1.0];
        let mut pair_bytes = AlignedBytes([0_u8; 32]);
        for (lane, value) in pair_lanes.into_iter().enumerate() {
            let start = lane * size_of::<f32>();
            pair_bytes.0[start..start + size_of::<f32>()].copy_from_slice(&value.to_ne_bytes());
        }
        let pair = *bytemuck::from_bytes::<DescriptorSamplePair>(&pair_bytes.0);
        assert_eq!(pair.s0.lanes, [31.0, 1.0, 2.0, 0.0]);
        assert_eq!(pair.s1.lanes, [0.25, -0.5, 7.0, 1.0]);
        assert_eq!(bytemuck::bytes_of(&pair), pair_bytes.0);
        assert_eq!(size_of::<DescriptorTexel>(), 16);
        assert_eq!(align_of::<DescriptorTexel>(), 16);
        assert_eq!(size_of::<TilePoseHeader>(), 512);
        assert_eq!(size_of::<DescriptorSamplePair>(), 32);
        assert_eq!(DescriptorCostLedger::SAMPLE_BYTES_PER_TILE, 2_097_152);
        assert_eq!(DescriptorCostLedger::HEADER_BYTES_PER_TILE, 512);
        assert_eq!(DescriptorCostLedger::LOGICAL_BYTES_PER_TILE, 2_097_664);
        assert_eq!(SourcePixelRect::PHYSICAL_SIDE, 256);
        assert_eq!(SourcePixelRect::CORE_SIDE, 254);
        assert_eq!(SourcePixelRect::APRON_SAMPLES, 1);
        assert_eq!(DescriptorCostLedger::ACTIVE_PREFIX_RECORDS, 64);
        assert_eq!(DescriptorCostLedger::HEADER_SLOTS, 64);
        assert_eq!(DescriptorCostLedger::OWNERSHIP_RECORDS, 63_424);
        assert_eq!(DescriptorCostLedger::DESCRIPTOR_PAGE_RECORDS, 65_536);
        assert_eq!(
            DescriptorCostLedger::ACTIVE_PREFIX_RECORDS
                + DescriptorCostLedger::HEADER_SLOTS * TilePoseHeader::TEXELS as u64
                + DescriptorCostLedger::OWNERSHIP_RECORDS,
            DescriptorCostLedger::DESCRIPTOR_PAGE_RECORDS
        );
        assert_eq!(DescriptorCostLedger::logical_bytes(1), Some(2_097_664));
        assert_eq!(DescriptorCostLedger::logical_bytes(9), Some(18_878_976));
        assert_eq!(DescriptorCostLedger::logical_bytes(12), Some(25_171_968));
        assert_eq!(DescriptorCostLedger::logical_bytes(16), Some(33_562_624));
        assert_eq!(DescriptorCostLedger::logical_bytes(28), Some(58_734_592));
        assert_eq!(DescriptorCostLedger::logical_bytes(44), Some(92_297_216));
        assert_eq!(DescriptorCostLedger::logical_bytes(56), Some(117_469_184));
    }

    #[test]
    fn same_surface_quality_owner_is_independent_of_catalog_order() {
        let backdrop = TileQuality {
            residency: TileResidency::Backdrop,
            rung: TileRung::Backdrop,
            density: ExactF64::new(0.5),
            error: ExactF64::new(0.2),
            age: 20,
            tile_id: 4,
        };
        let preview = TileQuality {
            residency: TileResidency::Detail,
            rung: TileRung::Preview,
            density: ExactF64::new(1.0),
            error: ExactF64::new(0.1),
            age: 10,
            tile_id: 3,
        };
        let final_tile = TileQuality {
            residency: TileResidency::Detail,
            rung: TileRung::Final,
            density: ExactF64::new(2.0),
            error: ExactF64::new(0.01),
            age: 1,
            tile_id: 2,
        };
        for order in [
            [backdrop, preview, final_tile],
            [final_tile, backdrop, preview],
            [preview, final_tile, backdrop],
        ] {
            assert_eq!(select_same_surface_owner(order), Some(final_tile));
        }
        let worse_error = TileQuality {
            error: ExactF64::new(0.02),
            age: 99,
            tile_id: 1,
            ..final_tile
        };
        let older = TileQuality {
            age: 0,
            tile_id: 1,
            ..final_tile
        };
        let higher_tile_id = TileQuality {
            tile_id: 3,
            ..final_tile
        };
        for contender in [worse_error, older, higher_tile_id] {
            assert_eq!(
                select_same_surface_owner([final_tile, contender]),
                Some(final_tile)
            );
            assert_eq!(
                select_same_surface_owner([contender, final_tile]),
                Some(final_tile)
            );
        }
        let cell = CanonicalChartCellKey {
            slice: slice(),
            level: 5,
            x: -7,
            y: 11,
        };
        assert_eq!(cell, cell);
    }

    #[test]
    fn invalidation_matrix_covers_reprojection_and_held_partition_transitions() {
        let rows = [
            (RenderControlChange::Camera, TileInvalidation::Keep),
            (RenderControlChange::Translation, TileInvalidation::Keep),
            (RenderControlChange::Height, TileInvalidation::Keep),
            (RenderControlChange::DistanceFive, TileInvalidation::Keep),
            (RenderControlChange::DistanceFour, TileInvalidation::Keep),
            (RenderControlChange::Observer, TileInvalidation::Keep),
            (RenderControlChange::Zoom, TileInvalidation::Keep),
            (RenderControlChange::Extent, TileInvalidation::Keep),
            (
                RenderControlChange::PlanePreservingObject,
                TileInvalidation::Keep,
            ),
            (RenderControlChange::InPlaneOrigin, TileInvalidation::Keep),
            (RenderControlChange::Display, TileInvalidation::Keep),
            (
                RenderControlChange::SliceTilt,
                TileInvalidation::NewPartition,
            ),
            (
                RenderControlChange::OutOfPlaneOrigin,
                TileInvalidation::NewPartition,
            ),
            (
                RenderControlChange::IterationCap,
                TileInvalidation::NewPartition,
            ),
            (
                RenderControlChange::Precision,
                TileInvalidation::NewPartition,
            ),
            (
                RenderControlChange::RecordAbi,
                TileInvalidation::NewPartition,
            ),
            (
                RenderControlChange::MainGeneration,
                TileInvalidation::NewPartition,
            ),
        ];
        for (change, expected) in rows {
            assert_eq!(tile_invalidation(change), expected);
            assert_eq!(
                transition_presentation(change),
                if expected == TileInvalidation::Keep {
                    TransitionPresentation::Reproject
                } else {
                    TransitionPresentation::HoldPrevious
                }
            );
        }
    }

    #[test]
    fn nonzero_reserved_header_lane_is_refused() {
        let mut header = TilePoseHeader::zeroed();
        header.texels[31].lanes[3] = -0.0;
        assert_eq!(
            validate_pose_header(&header),
            Err(DescriptorAbiError::ReservedHeaderLane)
        );
    }

    #[test]
    fn content_key_names_the_slice_and_main_partition_exactly() {
        let first = TileContentKey {
            version: TileContentKey::VERSION,
            slice: slice(),
            main_generation: 4,
            iteration_cap: 512,
            formula_abi: 1,
            precision_mode: PrecisionMode::PictureFast,
            record_abi: 1,
            reference_generation: 4,
        };
        assert_eq!(first, first);
        assert_ne!(
            first,
            TileContentKey {
                main_generation: 5,
                ..first
            }
        );
    }
}
