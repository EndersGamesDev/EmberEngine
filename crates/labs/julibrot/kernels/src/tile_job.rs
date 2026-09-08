//! Engine-neutral rendered-tile records and wire-free stage-0 policy.
//!
//! Descriptor-map ABI version one fixes the following field, byte-offset, byte-size, and lane
//! table. Header offsets are relative to a 512-byte header slot; sample offsets are relative to
//! one 32-byte logical sample pair.
//!
//! |Field|Offset|Size|Lane|
//! |-----|-----:|---:|----|
//! |`tile_id,content_key_id,anchor_id,flags`|0|16|`H00`|
//! |`value_span,lifted_span,ownership_base,header_generation`|16|16|`H01`|
//! |Six ordered `(cos O_ij,sin O_ij)` pairs|32|48|`H02-H04`|
//! |Ten ordered `(cos Q_ij,sin Q_ij)` pairs|80|80|`H05-H09`|
//! |`cos_yaw,sin_yaw,cos_pitch,sin_pitch`|160|16|`H10`|
//! |`origin0_hi..origin3_hi,origin0_lo..origin3_lo`|176|32|`H11-H12`|
//! |`t0,t1,t2,t3`|208|16|`H13`|
//! |`t4,height,d5,d4`|224|16|`H14`|
//! |`zoom_log2,extent_w,extent_h,chart_density`|240|16|`H15`|
//! |`rect_x,rect_y,rect_w,rect_h`|256|16|`H16`|
//! |`anchor_dx_hi,anchor_dx_lo,anchor_dy_hi,anchor_dy_lo`|272|16|`H17`|
//! |Three padded rows of the accepted source map|288|48|`H18-H20`|
//! |`depth_min,depth_max,coordinate_error,reprojection_error`|336|16|`H21`|
//! |`residency_rank,refinement_rung,iteration_cap,age_rank`|352|16|`H22`|
//! |`valid_count,glitch_count,uncertain_count,mesh_class`|368|16|`H23`|
//! |`chart_scale_hi,chart_scale_lo,anchor_precision_bits,anchor_revision`|384|16|`H24`|
//! |`slice_key_id,MAIN_generation,record_ABI,reference_generation`|400|16|`H25`|
//! |`ownership_count,ownership_revision,value_generation,lifted_generation`|416|16|`H26`|
//! |Positive zero|432|80|`H27-H31`|
//! |`smooth_iter,escaped,rebase_count,status`|0|16|`S0[k]`|
//! |`a_F,b_F,zeta_F,validity`|16|16|`S1[k]`|
//!
//! The version-one invalidation matrix is complete and keeps index work distinct from current presentation:
//!
//! |State|Event|Result|
//! |-----|-----|------|
//! |Keep|Camera|Query footprint; reproject|
//! |Keep|Translation|Query footprint; reproject|
//! |Keep|Height|Query footprint; reproject|
//! |Keep|DistanceFive|Query footprint; reproject|
//! |Keep|DistanceFour|Query footprint; reproject|
//! |Keep|Observer|Query footprint; reproject|
//! |Keep|Zoom|Query footprint; reproject|
//! |Keep|Extent|Query footprint; reproject|
//! |Keep|PlanePreservingObject|Transform chart; reproject|
//! |Keep|InPlaneOrigin|Transform chart; reproject|
//! |Keep|Display|No index action; shade current|
//! |NewPartition|SliceTilt|New slice index; hold|
//! |NewPartition|OutOfPlaneOrigin|New slice index; hold|
//! |NewPartition|IterationCap|New MAIN index; hold|
//! |NewPartition|FormulaAbi|New MAIN index; hold|
//! |NewPartition|Precision|New MAIN index; hold|
//! |NewPartition|RecordAbi|New MAIN index; hold|
//! |NewPartition|MainGeneration|New MAIN index; hold|

use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use bytemuck::{Pod, Zeroable};
use ember_julibrot_math::{ObjectAngles, Plane, Pose, PoseMap, PrecisionMode, ViewControls};
use ember_lab_heap::DataSpan;
use thiserror::Error;

use crate::RefinementLevel;

/// The initial physical tile side selected by the rendered-tile design.
pub const DEFAULT_TILE_SIDE: u32 = 256;
/// The initial retained apron on every physical tile edge.
pub const DEFAULT_TILE_APRON: u32 = 1;
/// The initial drawn core side after removing both aprons.
pub const DEFAULT_TILE_CORE_SIDE: u32 = DEFAULT_TILE_SIDE - 2 * DEFAULT_TILE_APRON;
/// Bytes in one RGBA32F value or reconstruction sample.
pub const TILE_SAMPLE_RECORD_BYTES: u64 = DescriptorTexel::BYTE_SIZE as u64;
/// Bytes in one tile's descriptor header slot.
pub const TILE_HEADER_BYTES: u64 = TilePoseHeader::BYTE_SIZE as u64;
/// Logical bytes in the paired sample columns of one default tile.
pub const DEFAULT_TILE_SAMPLE_BYTES: u64 =
    DEFAULT_TILE_SIDE as u64 * DEFAULT_TILE_SIDE as u64 * 2 * TILE_SAMPLE_RECORD_BYTES;
/// Logical bytes in one default resident tile, including its header slot.
pub const DEFAULT_TILE_LOGICAL_BYTES: u64 = DEFAULT_TILE_SAMPLE_BYTES + TILE_HEADER_BYTES;

/// Typed refusal from stage-0 tile policy arithmetic or lifetime validation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TileJobError {
    #[error("tile geometry is empty or its apron consumes the core")]
    InvalidGeometry,
    #[error("tile arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("the reference does not belong to the job's MAIN identity")]
    ReferenceMainMismatch,
    #[error("the MAIN generation already has a different reference orbit")]
    ReferenceConflict,
    #[error("the reference lease token is stale or belongs to another lease set")]
    StaleReferenceLease,
    #[error("the paired output spans do not match the tile sample count")]
    OutputShapeMismatch,
    #[error("the value and reconstruction outputs alias one span")]
    OutputAlias,
    #[error("the resident output profile cannot hold the paired reservation")]
    OutputProfileTooSmall,
    #[error("a paired output completion belongs to a stale MAIN generation")]
    StaleOutputCompletion,
    #[error("a paired output completion names the wrong output side")]
    OutputCompletionMismatch,
    #[error("the queue already contains the stable job ID")]
    DuplicateJob,
    #[error("the resident profile has fewer tiles than protected backdrop slots")]
    InvalidResidentProfile,
}

/// Exact-equality binary64 key lane.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ExactF64(u64);

impl ExactF64 {
    /// Exact key-lane schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 8;

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
#[repr(transparent)]
pub struct ExactF32(u32);

impl ExactF32 {
    /// Exact key-lane schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;

    /// Captures one binary32 value without normalization.
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// Restores the captured binary32 value.
    #[must_use]
    pub const fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Exact semantic identity of one canonical affine slice.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct SliceIdentity {
    /// Once-rounded first plane basis vector.
    pub basis_u: [ExactF32; 4],
    /// Once-rounded second plane basis vector.
    pub basis_v: [ExactF32; 4],
    /// Exact finite mirror of the defining plane origin.
    pub origin: [ExactF64; 4],
}

impl SliceIdentity {
    /// Canonical-slice schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 64;

    /// Captures the canonical slice components used by a render pose.
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
#[repr(C)]
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
    /// Content-key schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 96;
}

impl Hash for TileContentKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.version.hash(state);
        self.slice.hash(state);
        self.main_generation.hash(state);
        self.iteration_cap.hash(state);
        self.formula_abi.hash(state);
        (self.precision_mode as u32).hash(state);
        self.record_abi.hash(state);
        self.reference_generation.hash(state);
    }
}

/// Interned exact source anchor and scale provenance.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct SourceIdentity {
    /// Source-identity schema version.
    pub version: u32,
    /// Stable exact-anchor interner index.
    pub anchor_id: u32,
    /// Revision of the interned anchor value.
    pub anchor_revision: u32,
    /// Stable provenance index for the exact source scale.
    pub scale_provenance: u32,
}

impl SourceIdentity {
    /// Source-identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 16;
    /// Identity used when the finite pose mirror is the only available source authority.
    pub const FINITE_MIRROR: Self = Self {
        version: Self::VERSION,
        anchor_id: 0,
        anchor_revision: 0,
        scale_provenance: 0,
    };

    /// Names one interned exact anchor and scale receipt.
    #[must_use]
    pub const fn new(anchor_id: u32, anchor_revision: u32, scale_provenance: u32) -> Self {
        Self {
            version: Self::VERSION,
            anchor_id,
            anchor_revision,
            scale_provenance,
        }
    }
}

/// Exact key form of a mapped or edge-on source screen transform.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub enum PoseMapKey {
    /// Complete accepted source map, including its explicit inverse and apron.
    Mapped([ExactF64; 20]),
    /// Physical edge-on source pose.
    EdgeOn,
}

impl PoseMapKey {
    /// Source-map key schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 168;
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

/// Stable identity of one semantic rendered-content partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ContentIdentity(pub u64);

impl ContentIdentity {
    /// Stable-content identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 8;
}

/// One MAIN generation within a content partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct MainIdentity {
    pub content: ContentIdentity,
    pub generation: u64,
}

impl MainIdentity {
    /// MAIN-identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 16;
}

/// One reference orbit accepted for a MAIN generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct ReferenceIdentity {
    pub main: MainIdentity,
    pub generation: u64,
}

impl ReferenceIdentity {
    /// Reference-identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 24;
}

/// Reproducible final tie-break for one tile job.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct StableJobId(pub u64);

impl StableJobId {
    /// Stable-job identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 8;
}

/// Parameterized physical/core/apron geometry for one source-screen tile.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct TileGeometry {
    physical_width: u32,
    physical_height: u32,
    apron: u32,
}

impl TileGeometry {
    /// Tile-geometry schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 12;
    /// The design's initial 256 physical, 254 core, one-sample-apron geometry.
    pub const DEFAULT: Self = Self {
        physical_width: DEFAULT_TILE_SIDE,
        physical_height: DEFAULT_TILE_SIDE,
        apron: DEFAULT_TILE_APRON,
    };

    /// Builds a rectangular physical tile with the same apron on every edge.
    ///
    /// # Errors
    ///
    /// Refuses empty geometry, an apron that consumes either core dimension, or a sample count
    /// that cannot be represented by an output span.
    pub fn new(
        physical_width: u32,
        physical_height: u32,
        apron: u32,
    ) -> Result<Self, TileJobError> {
        let doubled_apron = apron
            .checked_mul(2)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        if physical_width <= doubled_apron || physical_height <= doubled_apron {
            return Err(TileJobError::InvalidGeometry);
        }
        physical_width
            .checked_mul(physical_height)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        Ok(Self {
            physical_width,
            physical_height,
            apron,
        })
    }

    #[must_use]
    pub const fn physical_extent(self) -> [u32; 2] {
        [self.physical_width, self.physical_height]
    }

    #[must_use]
    pub const fn core_extent(self) -> [u32; 2] {
        [
            self.physical_width - 2 * self.apron,
            self.physical_height - 2 * self.apron,
        ]
    }

    #[must_use]
    pub const fn apron(self) -> u32 {
        self.apron
    }

    #[must_use]
    pub const fn sample_count(self) -> u32 {
        self.physical_width * self.physical_height
    }
}

/// Integer source-screen rectangle including the retained apron.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct SourceScreenRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SourceScreenRect {
    /// Source-rectangle schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 16;
    /// Physical side of a version-one rendered tile, including aprons.
    pub const PHYSICAL_SIDE: u32 = DEFAULT_TILE_SIDE;
    /// Drawn core side of a version-one rendered tile.
    pub const CORE_SIDE: u32 = DEFAULT_TILE_CORE_SIDE;
    /// Retained sample apron on each core edge.
    pub const APRON_SAMPLES: u32 = DEFAULT_TILE_APRON;

    #[must_use]
    pub const fn new(x: i32, y: i32, geometry: TileGeometry) -> Self {
        let [width, height] = geometry.physical_extent();
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Names an arbitrary signed source-screen rectangle.
    #[must_use]
    pub const fn from_extent(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the physical sample count.
    ///
    /// # Errors
    ///
    /// Refuses dimensions whose product cannot be represented by an output span.
    pub const fn sample_count(self) -> Result<u32, TileJobError> {
        match self.width.checked_mul(self.height) {
            Some(count) => Ok(count),
            None => Err(TileJobError::ArithmeticOverflow),
        }
    }

    /// Drawn core rectangle `[x, y, width, height]` inside the physical source rectangle.
    ///
    /// # Errors
    ///
    /// Refuses an origin whose apron offset cannot be represented by `i32`.
    pub fn core_rect(self, geometry: TileGeometry) -> Result<[i32; 4], TileJobError> {
        if geometry.physical_extent() != [self.width, self.height] {
            return Err(TileJobError::InvalidGeometry);
        }
        let apron = i32::try_from(geometry.apron).map_err(|_| TileJobError::ArithmeticOverflow)?;
        let [width, height] = geometry.core_extent();
        Ok([
            self.x
                .checked_add(apron)
                .ok_or(TileJobError::ArithmeticOverflow)?,
            self.y
                .checked_add(apron)
                .ok_or(TileJobError::ArithmeticOverflow)?,
            i32::try_from(width).map_err(|_| TileJobError::ArithmeticOverflow)?,
            i32::try_from(height).map_err(|_| TileJobError::ArithmeticOverflow)?,
        ])
    }
}

/// Versioned exact source-render identity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
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
    /// Retained signed source-screen rectangle.
    pub source_rect: SourceScreenRect,
    /// Exact source map accepted for the render.
    pub source_map: PoseMapKey,
    /// Canonical sampled slice identity.
    pub slice: SliceIdentity,
    /// MAIN generation whose pose was rendered.
    pub main_generation: u32,
    /// Interned exact anchor and scale provenance.
    pub source_identity: SourceIdentity,
}

impl TileRenderKey {
    /// Render-key schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 536;

    /// Captures every finite source-pose field with finite-mirror source provenance.
    #[must_use]
    pub fn from_pose(pose: &Pose, source_rect: SourceScreenRect) -> Self {
        Self::from_pose_and_source(pose, source_rect, SourceIdentity::FINITE_MIRROR)
    }

    /// Captures every source-pose field and its exact anchor/scale identity.
    #[must_use]
    pub fn from_pose_and_source(
        pose: &Pose,
        source_rect: SourceScreenRect,
        source_identity: SourceIdentity,
    ) -> Self {
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
            source_identity,
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

/// Generation-checked handle for one reusable source mesh.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct TileMeshHandle {
    /// Stable mesh-directory index.
    pub index: u32,
    /// Generation occupying the indexed slot.
    pub generation: u32,
}

impl TileMeshHandle {
    /// Mesh-handle schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 8;

    /// Names one generation-checked mesh slot.
    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
}

/// Generation-checked identity of one logical RGBA32F DATA span.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
pub struct TileSpanIdentity {
    /// Span-directory index used by descriptor accessors.
    pub directory_index: u32,
    /// Generation occupying the directory slot.
    pub generation: u32,
    /// Addressable logical record count.
    pub logical_len: u32,
}

impl TileSpanIdentity {
    /// Span-identity schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 12;

    /// Names one generation-checked logical span.
    #[must_use]
    pub const fn new(directory_index: u32, generation: u32, logical_len: u32) -> Self {
        Self {
            directory_index,
            generation,
            logical_len,
        }
    }
}

/// Paired value and reconstruction span identities for one descriptor source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PairedTileSpanIdentities {
    /// Palette-independent value records declared as `S0`.
    pub value: TileSpanIdentity,
    /// Source reconstruction records declared as `S1`.
    pub reconstruction: TileSpanIdentity,
}

impl PairedTileSpanIdentities {
    /// Paired-span schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 24;

    /// Binds equal-length spans whose directory slots do not alias.
    ///
    /// # Errors
    ///
    /// Refuses the same directory slot or unequal logical record counts.
    pub const fn new(
        value: TileSpanIdentity,
        reconstruction: TileSpanIdentity,
    ) -> Result<Self, TileJobError> {
        if value.directory_index == reconstruction.directory_index {
            return Err(TileJobError::OutputAlias);
        }
        if value.logical_len != reconstruction.logical_len {
            return Err(TileJobError::OutputShapeMismatch);
        }
        Ok(Self {
            value,
            reconstruction,
        })
    }
}

/// Generation-tagged allocation receipt for one `S0`/`S1` output pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PairedOutputAllocation {
    /// MAIN generation shared by both span identities.
    pub generation: u32,
    /// Non-aliasing value and reconstruction span identities.
    pub spans: PairedTileSpanIdentities,
    /// Exact logical bytes in both sample columns, excluding physical padding.
    pub logical_bytes: u64,
    /// Exact physical bytes reserved by both sample spans.
    pub reserved_bytes: u64,
}

impl PairedOutputAllocation {
    /// Paired-allocation schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 48;

    /// Records two spans allocated together under one MAIN generation.
    ///
    /// # Errors
    ///
    /// Refuses aliased or unequal spans, byte arithmetic overflow, or a resident profile whose
    /// DATA capacity cannot hold the exact paired reservation.
    pub fn from_spans(
        generation: u32,
        value: &DataSpan,
        reconstruction: &DataSpan,
        profile_reserved_bytes: u64,
    ) -> Result<Self, TileJobError> {
        let spans = PairedTileSpanIdentities::new(
            TileSpanIdentity::new(value.directory_index, generation, value.logical_len),
            TileSpanIdentity::new(
                reconstruction.directory_index,
                generation,
                reconstruction.logical_len,
            ),
        )?;
        let logical_bytes =
            DescriptorCostLedger::paired_sample_logical_bytes(u64::from(value.logical_len))
                .ok_or(TileJobError::ArithmeticOverflow)?;
        let value_reserved_bytes = value
            .reserved_records()
            .checked_mul(DescriptorCostLedger::TEXEL_BYTES)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let reconstruction_reserved_bytes = reconstruction
            .reserved_records()
            .checked_mul(DescriptorCostLedger::TEXEL_BYTES)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let reserved_bytes = DescriptorCostLedger::paired_reserved_bytes(
            value_reserved_bytes,
            reconstruction_reserved_bytes,
        )
        .ok_or(TileJobError::ArithmeticOverflow)?;
        if reserved_bytes > profile_reserved_bytes {
            return Err(TileJobError::OutputProfileTooSmall);
        }
        Ok(Self {
            generation,
            spans,
            logical_bytes,
            reserved_bytes,
        })
    }
}

/// One physical RGBA32F descriptor-map texel.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct DescriptorTexel {
    /// Four binary32 lanes in the documented header or sample order.
    pub lanes: [f32; 4],
}

impl DescriptorTexel {
    /// Descriptor-texel schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 16;
}

/// Exact version-one 32-texel rendered-tile pose header.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct TilePoseHeader {
    /// Header texels `H00` through `H31`; `H27` through `H31` must remain zero.
    pub texels: [DescriptorTexel; 32],
}

impl TilePoseHeader {
    /// Pose-header schema and descriptor-map ABI version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size and header-slot stride.
    pub const BYTE_SIZE: usize = 512;
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

/// Exact two-RGBA32F sample declaration `S0/S1`.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct DescriptorSamplePair {
    /// Existing escape-value record, byte-for-byte and lane-for-lane unchanged.
    pub s0: DescriptorTexel,
    /// `(a_F,b_F,zeta_F,validity)` lifted source reconstruction record.
    pub s1: DescriptorTexel,
}

impl DescriptorSamplePair {
    /// Sample-pair schema and descriptor-map ABI version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 32;
    /// Value-record lane declaration.
    pub const S0_LANES: &'static str = "smooth_iter,escaped,rebase_count,status";
    /// Reconstruction-record lane declaration.
    pub const S1_LANES: &'static str = "a_F,b_F,zeta_F,validity";

    /// Declares the existing value record and the lifted reconstruction record.
    #[must_use]
    pub const fn new(value: [f32; 4], lifted: [f32; 4]) -> Self {
        Self {
            s0: DescriptorTexel { lanes: value },
            s1: DescriptorTexel { lanes: lifted },
        }
    }
}

/// Source-pose lanes appended to a value kernel when it produces `S0` and `S1` together.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct SourceReconstructionUniform {
    /// Five ordered ambient-camera factor pairs.
    pub camera_rotation_pairs: [[f32; 4]; 5],
    /// Four low coordinates followed by the padded fifth coordinate.
    pub camera_translation: [[f32; 4]; 2],
    /// `(cos_yaw,sin_yaw,cos_pitch,sin_pitch)`.
    pub observer_rotation: [f32; 4],
    /// `(height_scale,distance_five,distance_four,chart_scale)`.
    pub view_scale: [f32; 4],
}

impl SourceReconstructionUniform {
    /// Source-reconstruction uniform schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 144;

    /// Selects the source projection lanes from one validated descriptor header.
    #[must_use]
    pub fn from_header(header: &TilePoseHeader) -> Option<Self> {
        if validate_pose_header(header).is_err() {
            return None;
        }
        let camera_rotation_pairs = core::array::from_fn(|index| {
            header.texels[TilePoseHeader::H05_CAMERA_12_13 + index].lanes
        });
        let projection = header.texels[TilePoseHeader::H14_PROJECTION].lanes;
        let chart_scale = header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes;
        let uniform = Self {
            camera_rotation_pairs,
            camera_translation: [
                header.texels[TilePoseHeader::H13_TRANSLATION_0_3].lanes,
                [projection[0], 0.0, 0.0, 0.0],
            ],
            observer_rotation: header.texels[TilePoseHeader::H10_OBSERVER].lanes,
            view_scale: [
                projection[1],
                projection[2],
                projection[3],
                chart_scale[0] + chart_scale[1],
            ],
        };
        let finite = uniform
            .camera_rotation_pairs
            .into_iter()
            .flatten()
            .chain(uniform.camera_translation.into_iter().flatten())
            .chain(uniform.observer_rotation)
            .chain(uniform.view_scale)
            .all(f32::is_finite);
        let [height_scale, distance_five, distance_four, source_scale] = uniform.view_scale;
        (finite
            && height_scale >= 0.0
            && distance_five > 0.0
            && distance_four > 0.0
            && source_scale > 0.0)
            .then_some(uniform)
    }

    /// Returns the exact little-endian wasm/native payload bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

/// One row of the exact descriptor-map ABI table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorAbiField {
    /// Stable semantic field name.
    pub field: &'static str,
    /// Byte offset within the header slot or logical sample pair.
    pub offset: u16,
    /// Consecutive byte size.
    pub size: u16,
    /// Header or sample lane declaration.
    pub lane: &'static str,
}

/// Exact descriptor-map ABI table mirrored by the module documentation.
pub const DESCRIPTOR_ABI_LAYOUT: [DescriptorAbiField; 21] = [
    DescriptorAbiField {
        field: "tile_id,content_key_id,anchor_id,flags",
        offset: 0,
        size: 16,
        lane: "H00",
    },
    DescriptorAbiField {
        field: "value_span,lifted_span,ownership_base,header_generation",
        offset: 16,
        size: 16,
        lane: "H01",
    },
    DescriptorAbiField {
        field: "six ordered object rotation factor pairs",
        offset: 32,
        size: 48,
        lane: "H02-H04",
    },
    DescriptorAbiField {
        field: "ten ordered camera rotation factor pairs",
        offset: 80,
        size: 80,
        lane: "H05-H09",
    },
    DescriptorAbiField {
        field: "cos_yaw,sin_yaw,cos_pitch,sin_pitch",
        offset: 160,
        size: 16,
        lane: "H10",
    },
    DescriptorAbiField {
        field: "origin0_hi..origin3_hi,origin0_lo..origin3_lo",
        offset: 176,
        size: 32,
        lane: "H11-H12",
    },
    DescriptorAbiField {
        field: "t0,t1,t2,t3",
        offset: 208,
        size: 16,
        lane: "H13",
    },
    DescriptorAbiField {
        field: "t4,height,d5,d4",
        offset: 224,
        size: 16,
        lane: "H14",
    },
    DescriptorAbiField {
        field: "zoom_log2,extent_w,extent_h,chart_density",
        offset: 240,
        size: 16,
        lane: "H15",
    },
    DescriptorAbiField {
        field: "rect_x,rect_y,rect_w,rect_h",
        offset: 256,
        size: 16,
        lane: "H16",
    },
    DescriptorAbiField {
        field: "anchor_dx_hi,anchor_dx_lo,anchor_dy_hi,anchor_dy_lo",
        offset: 272,
        size: 16,
        lane: "H17",
    },
    DescriptorAbiField {
        field: "three padded accepted source-map rows",
        offset: 288,
        size: 48,
        lane: "H18-H20",
    },
    DescriptorAbiField {
        field: "depth_min,depth_max,coordinate_error,reprojection_error",
        offset: 336,
        size: 16,
        lane: "H21",
    },
    DescriptorAbiField {
        field: "residency_rank,refinement_rung,iteration_cap,age_rank",
        offset: 352,
        size: 16,
        lane: "H22",
    },
    DescriptorAbiField {
        field: "valid_count,glitch_count,uncertain_count,mesh_class",
        offset: 368,
        size: 16,
        lane: "H23",
    },
    DescriptorAbiField {
        field: "chart_scale_hi,chart_scale_lo,anchor_precision_bits,anchor_revision",
        offset: 384,
        size: 16,
        lane: "H24",
    },
    DescriptorAbiField {
        field: "slice_key_id,MAIN_generation,record_ABI,reference_generation",
        offset: 400,
        size: 16,
        lane: "H25",
    },
    DescriptorAbiField {
        field: "ownership_count,ownership_revision,value_generation,lifted_generation",
        offset: 416,
        size: 16,
        lane: "H26",
    },
    DescriptorAbiField {
        field: "positive zero",
        offset: 432,
        size: 80,
        lane: "H27-H31",
    },
    DescriptorAbiField {
        field: "smooth_iter,escaped,rebase_count,status",
        offset: 0,
        size: 16,
        lane: "S0[k]",
    },
    DescriptorAbiField {
        field: "a_F,b_F,zeta_F,validity",
        offset: 16,
        size: 16,
        lane: "S1[k]",
    },
];

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

/// Version-one descriptor-map resource arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DescriptorCostLedger;

impl DescriptorCostLedger {
    /// Descriptor ABI version.
    pub const ABI_VERSION: u32 = 1;
    /// Bytes in one RGBA32F texel.
    pub const TEXEL_BYTES: u64 = 16;
    /// Physical sample count in one default tile.
    pub const SAMPLES_PER_TILE: u64 =
        SourceScreenRect::PHYSICAL_SIDE as u64 * SourceScreenRect::PHYSICAL_SIDE as u64;
    /// Active-instance records at the start of the shared descriptor page.
    pub const ACTIVE_PREFIX_RECORDS: u64 = 64;
    /// Complete pose-header slots in the shared descriptor page.
    pub const HEADER_SLOTS: u64 = 64;
    /// Compact ownership records after the active prefix and header slots.
    pub const OWNERSHIP_RECORDS: u64 = 63_424;
    /// Total records in one shared 256-square descriptor page.
    pub const DESCRIPTOR_PAGE_RECORDS: u64 = 256 * 256;
    /// Exact physical bytes in one DATA page.
    pub const DATA_PAGE_BYTES: u64 = Self::DESCRIPTOR_PAGE_RECORDS * Self::TEXEL_BYTES;
    /// Physical sample pages retained by each complete tile.
    pub const SAMPLE_PAGES_PER_TILE: u32 = 2;
    /// Bytes in the paired sample columns for one tile.
    pub const SAMPLE_BYTES_PER_TILE: u64 = 2 * Self::TEXEL_BYTES * Self::SAMPLES_PER_TILE;
    /// Bytes in one pose header.
    pub const HEADER_BYTES_PER_TILE: u64 = TilePoseHeader::BYTE_SIZE as u64;
    /// Logical bytes in one complete resident tile.
    pub const LOGICAL_BYTES_PER_TILE: u64 =
        Self::SAMPLE_BYTES_PER_TILE + Self::HEADER_BYTES_PER_TILE;

    /// Computes exact logical bytes for equal-length `S0` and `S1` sample columns.
    #[must_use]
    pub const fn paired_sample_logical_bytes(sample_count: u64) -> Option<u64> {
        match sample_count.checked_mul(2) {
            Some(records) => records.checked_mul(Self::TEXEL_BYTES),
            None => None,
        }
    }

    /// Adds the exact physical reservations for two independently padded sample spans.
    #[must_use]
    pub const fn paired_reserved_bytes(
        value_reserved_bytes: u64,
        reconstruction_reserved_bytes: u64,
    ) -> Option<u64> {
        value_reserved_bytes.checked_add(reconstruction_reserved_bytes)
    }

    /// Computes the exact logical bytes for a resident tile count.
    #[must_use]
    pub const fn logical_bytes(tile_count: u64) -> Option<u64> {
        Self::LOGICAL_BYTES_PER_TILE.checked_mul(tile_count)
    }

    /// Computes exact physical DATA allocation for a page count.
    #[must_use]
    pub const fn physical_data_bytes(page_count: u64) -> Option<u64> {
        Self::DATA_PAGE_BYTES.checked_mul(page_count)
    }
}

/// Whether a job closes a requested-frame hole or only upgrades covered pixels.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CoverageClass {
    ClosesHole = 0,
    DetailUpgrade = 1,
}

/// Exact ascending scheduler key `(coverage_class, -visible_benefit, work_cost, stable_job_id)`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DemandKey {
    coverage_class: CoverageClass,
    negative_visible_benefit: Reverse<u64>,
    work_cost: u64,
    stable_job_id: StableJobId,
}

impl DemandKey {
    #[must_use]
    pub const fn new(
        coverage_class: CoverageClass,
        visible_benefit: u64,
        work_cost: u64,
        stable_job_id: StableJobId,
    ) -> Self {
        Self {
            coverage_class,
            negative_visible_benefit: Reverse(visible_benefit),
            work_cost,
            stable_job_id,
        }
    }

    /// Builds `visible_benefit = visible_area * quality_gain` with checked arithmetic.
    ///
    /// # Errors
    ///
    /// Refuses multiplication overflow.
    pub fn from_visible_work(
        coverage_class: CoverageClass,
        visible_area: u64,
        quality_gain: u64,
        work_cost: u64,
        stable_job_id: StableJobId,
    ) -> Result<Self, TileJobError> {
        let visible_benefit = visible_area
            .checked_mul(quality_gain)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        Ok(Self::new(
            coverage_class,
            visible_benefit,
            work_cost,
            stable_job_id,
        ))
    }

    #[must_use]
    pub const fn coverage_class(self) -> CoverageClass {
        self.coverage_class
    }

    #[must_use]
    pub const fn visible_benefit(self) -> u64 {
        self.negative_visible_benefit.0
    }

    #[must_use]
    pub const fn work_cost(self) -> u64 {
        self.work_cost
    }

    #[must_use]
    pub const fn stable_job_id(self) -> StableJobId {
        self.stable_job_id
    }
}

/// A future tile computation request with no transport representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TileJob {
    pub content: ContentIdentity,
    pub main: MainIdentity,
    pub reference: ReferenceIdentity,
    pub source_rect: SourceScreenRect,
    pub refinement: RefinementLevel,
    pub demand: DemandKey,
}

impl TileJob {
    /// Validates that content, MAIN, reference, and stable demand identity agree.
    ///
    /// # Errors
    ///
    /// Refuses cross-partition MAIN or reference identities.
    pub fn new(
        main: MainIdentity,
        reference: ReferenceIdentity,
        source_rect: SourceScreenRect,
        refinement: RefinementLevel,
        demand: DemandKey,
    ) -> Result<Self, TileJobError> {
        if reference.main != main {
            return Err(TileJobError::ReferenceMainMismatch);
        }
        Ok(Self {
            content: main.content,
            main,
            reference,
            source_rect,
            refinement,
            demand,
        })
    }

    #[must_use]
    pub const fn stable_id(&self) -> StableJobId {
        self.demand.stable_job_id
    }
}

/// Insertion-order-independent ascending queue over exact demand keys.
#[derive(Debug, Default)]
pub struct TileDemandQueue {
    queued: BTreeMap<DemandKey, TileJob>,
    ids: BTreeSet<StableJobId>,
}

impl TileDemandQueue {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queued: BTreeMap::new(),
            ids: BTreeSet::new(),
        }
    }

    /// Inserts one stable job without replacing an existing identity.
    ///
    /// # Errors
    ///
    /// Returns `DuplicateJob` when the stable ID is already queued.
    pub fn push(&mut self, job: TileJob) -> Result<(), TileJobError> {
        if !self.ids.insert(job.stable_id()) {
            return Err(TileJobError::DuplicateJob);
        }
        self.queued.insert(job.demand, job);
        Ok(())
    }

    /// Removes the smallest exact demand key.
    pub fn pop(&mut self) -> Option<TileJob> {
        let (_, job) = self.queued.pop_first()?;
        self.ids.remove(&job.stable_id());
        Some(job)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.queued.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queued.is_empty()
    }
}

/// Same-surface residency class, ordered from fallback to detail.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum TileResidency {
    /// Protected coarse fallback.
    Backdrop = 0,
    /// Ordinary detailed/history tile.
    Detail = 1,
}

impl TileResidency {
    /// Residency schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
}

/// Same-surface refinement rung, ordered from coarse to final.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u32)]
pub enum TileRung {
    /// Fast preview result.
    Preview = 0,
    /// Intermediate result.
    Interactive = 1,
    /// Final result.
    Final = 2,
}

impl TileRung {
    /// Refinement-rung schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
}

/// Canonical chart microcell used only for competing representations of one surface.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(C)]
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

impl CanonicalChartCellKey {
    /// Chart-cell key schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 88;
}

/// Deterministic same-surface quality tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
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

impl TileQuality {
    /// Quality-record schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 40;

    /// Reports whether this same-surface candidate strictly outranks the incumbent.
    #[must_use]
    pub fn should_replace(self, incumbent: Self) -> bool {
        self > incumbent
    }
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

/// Control-class rows in the version-one invalidation matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
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
    /// Formula-semantics ABI change.
    FormulaAbi,
    /// Precision-policy change.
    Precision,
    /// Escape-record ABI change.
    RecordAbi,
    /// Strict version-one reference/MAIN generation change.
    MainGeneration,
}

impl RenderControlChange {
    /// Control-change schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
    /// Every event in stable matrix order.
    pub const ALL: [Self; 18] = [
        Self::Camera,
        Self::Translation,
        Self::Height,
        Self::DistanceFive,
        Self::DistanceFour,
        Self::Observer,
        Self::Zoom,
        Self::Extent,
        Self::PlanePreservingObject,
        Self::InPlaneOrigin,
        Self::Display,
        Self::SliceTilt,
        Self::OutOfPlaneOrigin,
        Self::IterationCap,
        Self::FormulaAbi,
        Self::Precision,
        Self::RecordAbi,
        Self::MainGeneration,
    ];
}

/// Semantic effect of one control-class change on resident rendered content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TileInvalidation {
    /// Matching-content tiles remain semantically valid and may be reprojected.
    Keep = 0,
    /// A new content partition starts; prior content may only be held unchanged.
    NewPartition = 1,
}

impl TileInvalidation {
    /// Invalidation-record schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
}

/// Presentation admitted during a control transition before replacement content completes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TransitionPresentation {
    /// Matching-content geometry may be reprojected normally.
    Reproject = 0,
    /// Prior content may remain only as an unchanged held frame.
    HoldPrevious = 1,
    /// Existing fragments are shaded again with current display inputs.
    ShadeCurrent = 2,
}

impl TransitionPresentation {
    /// Transition-presentation schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
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
        | RenderControlChange::FormulaAbi
        | RenderControlChange::Precision
        | RenderControlChange::RecordAbi
        | RenderControlChange::MainGeneration => TileInvalidation::NewPartition,
    }
}

/// Returns the only honest transitional presentation for one invalidation row.
#[must_use]
pub const fn transition_presentation(change: RenderControlChange) -> TransitionPresentation {
    if matches!(change, RenderControlChange::Display) {
        return TransitionPresentation::ShadeCurrent;
    }
    match tile_invalidation(change) {
        TileInvalidation::Keep => TransitionPresentation::Reproject,
        TileInvalidation::NewPartition => TransitionPresentation::HoldPrevious,
    }
}

/// The two distinct DATA spans that a tile job must publish together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedOutputSpanPlan {
    pub job_id: StableJobId,
    pub value: DataSpan,
    pub reconstruction: DataSpan,
}

impl PairedOutputSpanPlan {
    /// Binds equal-length, non-aliasing value and reconstruction spans to one job.
    ///
    /// # Errors
    ///
    /// Refuses aliasing spans or spans whose logical lengths differ from the physical sample grid.
    pub fn new(
        job: &TileJob,
        value: DataSpan,
        reconstruction: DataSpan,
    ) -> Result<Self, TileJobError> {
        if value == reconstruction {
            return Err(TileJobError::OutputAlias);
        }
        let sample_count = job.source_rect.sample_count()?;
        if value.logical_len != sample_count || reconstruction.logical_len != sample_count {
            return Err(TileJobError::OutputShapeMismatch);
        }
        Ok(Self {
            job_id: job.stable_id(),
            value,
            reconstruction,
        })
    }

    #[must_use]
    pub const fn begin(self) -> PairedOutputCompletion {
        PairedOutputCompletion {
            plan: self,
            value_complete: false,
            reconstruction_complete: false,
        }
    }
}

/// One side of a paired tile output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TileOutput {
    Value,
    Reconstruction,
}

impl TileOutput {
    /// Output-side schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 4;
}

/// One generation-tagged observation that a paired output side completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct TileOutputCompletion {
    /// MAIN generation observed at completion.
    pub generation: u32,
    /// Output side whose commands completed.
    pub output: TileOutput,
}

impl TileOutputCompletion {
    /// Output-completion schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 8;

    /// Records one completed side of a generation-tagged output pair.
    #[must_use]
    pub const fn new(generation: u32, output: TileOutput) -> Self {
        Self { generation, output }
    }
}

/// One receipt that represents both completions of an allocated output pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PairedOutputReceipt {
    /// The generation-tagged allocation whose two sides completed.
    pub allocation: PairedOutputAllocation,
    /// Value followed by reconstruction completion.
    pub completions: [TileOutputCompletion; 2],
}

impl PairedOutputReceipt {
    /// Paired-output receipt schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 64;

    /// Joins the two correctly ordered completions into one publication receipt.
    ///
    /// # Errors
    ///
    /// Refuses a completion from another generation or one naming the wrong output side.
    pub const fn new(
        allocation: PairedOutputAllocation,
        value: TileOutputCompletion,
        reconstruction: TileOutputCompletion,
    ) -> Result<Self, TileJobError> {
        if value.generation != allocation.generation
            || reconstruction.generation != allocation.generation
        {
            return Err(TileJobError::StaleOutputCompletion);
        }
        if !matches!(value.output, TileOutput::Value)
            || !matches!(reconstruction.output, TileOutput::Reconstruction)
        {
            return Err(TileJobError::OutputCompletionMismatch);
        }
        Ok(Self {
            allocation,
            completions: [value, reconstruction],
        })
    }
}

/// Completion state that cannot yield a publication until both output spans finish.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairedOutputCompletion {
    plan: PairedOutputSpanPlan,
    value_complete: bool,
    reconstruction_complete: bool,
}

impl PairedOutputCompletion {
    pub const fn complete(&mut self, output: TileOutput) {
        match output {
            TileOutput::Value => self.value_complete = true,
            TileOutput::Reconstruction => self.reconstruction_complete = true,
        }
    }

    #[must_use]
    pub const fn is_publishable(&self) -> bool {
        self.value_complete && self.reconstruction_complete
    }

    /// Converts only a complete pair into the publishable type, otherwise returns the state.
    ///
    /// # Errors
    ///
    /// Returns the unchanged completion state while either output is incomplete.
    pub fn try_publish(self) -> Result<PublishedTileOutputs, Self> {
        if self.is_publishable() {
            Ok(PublishedTileOutputs(self.plan))
        } else {
            Err(self)
        }
    }
}

/// A value/reconstruction pair proved complete as one publication unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedTileOutputs(PairedOutputSpanPlan);

impl PublishedTileOutputs {
    #[must_use]
    pub const fn spans(&self) -> &PairedOutputSpanPlan {
        &self.0
    }
}

#[derive(Clone, Debug)]
struct ReferenceLeaseEntry {
    reference: ReferenceIdentity,
    tokens: BTreeSet<u64>,
}

/// Move-only pin held by a tile job across render-pose navigation.
#[derive(Debug, Eq, PartialEq)]
pub struct ReferenceLease {
    main: MainIdentity,
    reference: ReferenceIdentity,
    serial: u64,
}

impl ReferenceLease {
    #[must_use]
    pub const fn main(&self) -> MainIdentity {
        self.main
    }

    #[must_use]
    pub const fn reference(&self) -> ReferenceIdentity {
        self.reference
    }
}

/// Counted, one-reference-per-MAIN lease registry.
#[derive(Debug, Default)]
pub struct ReferenceLeaseSet {
    entries: BTreeMap<MainIdentity, ReferenceLeaseEntry>,
    next_serial: u64,
}

impl ReferenceLeaseSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_serial: 1,
        }
    }

    /// Pins the job's reference or shares the already-pinned identical reference.
    ///
    /// # Errors
    ///
    /// Refuses a second reference identity for a MAIN generation or serial exhaustion.
    pub fn acquire(&mut self, job: &TileJob) -> Result<ReferenceLease, TileJobError> {
        let serial = self.next_serial;
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let entry = self
            .entries
            .entry(job.main)
            .or_insert_with(|| ReferenceLeaseEntry {
                reference: job.reference,
                tokens: BTreeSet::new(),
            });
        if entry.reference != job.reference {
            return Err(TileJobError::ReferenceConflict);
        }
        entry.tokens.insert(serial);
        Ok(ReferenceLease {
            main: job.main,
            reference: job.reference,
            serial,
        })
    }

    /// Releases one pin and returns the remaining count for that MAIN generation.
    ///
    /// # Errors
    ///
    /// Refuses a consumed, foreign, or otherwise stale lease token.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "consuming the move-only token prevents a successful lease from being released twice"
    )]
    pub fn release(&mut self, lease: ReferenceLease) -> Result<u32, TileJobError> {
        let entry = self
            .entries
            .get_mut(&lease.main)
            .filter(|entry| entry.reference == lease.reference)
            .ok_or(TileJobError::StaleReferenceLease)?;
        if !entry.tokens.remove(&lease.serial) {
            return Err(TileJobError::StaleReferenceLease);
        }
        let count =
            u32::try_from(entry.tokens.len()).map_err(|_| TileJobError::ArithmeticOverflow)?;
        if count == 0 {
            self.entries.remove(&lease.main);
        }
        Ok(count)
    }

    #[must_use]
    pub fn lease_count(&self, main: MainIdentity) -> u32 {
        self.entries
            .get(&main)
            .and_then(|entry| u32::try_from(entry.tokens.len()).ok())
            .unwrap_or(0)
    }

    #[must_use]
    pub fn pinned_reference(&self, main: MainIdentity) -> Option<ReferenceIdentity> {
        self.entries.get(&main).map(|entry| entry.reference)
    }
}

/// Exact logical descriptor-record cost for a resident tile count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ResidentTileCost {
    pub tile_count: u32,
    pub sample_bytes: u64,
    pub header_bytes: u64,
    pub logical_bytes: u64,
}

impl ResidentTileCost {
    /// Cost-record schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 32;
}

/// Computes the exact logical cost-table row for `tile_count` tiles of `geometry`.
///
/// # Errors
///
/// Refuses byte arithmetic overflow.
pub fn tile_cost(
    geometry: TileGeometry,
    tile_count: u32,
) -> Result<ResidentTileCost, TileJobError> {
    let sample_bytes_per_tile = u64::from(geometry.sample_count())
        .checked_mul(2)
        .and_then(|records| records.checked_mul(TILE_SAMPLE_RECORD_BYTES))
        .ok_or(TileJobError::ArithmeticOverflow)?;
    let sample_bytes = u64::from(tile_count)
        .checked_mul(sample_bytes_per_tile)
        .ok_or(TileJobError::ArithmeticOverflow)?;
    let header_bytes = u64::from(tile_count)
        .checked_mul(TILE_HEADER_BYTES)
        .ok_or(TileJobError::ArithmeticOverflow)?;
    let logical_bytes = sample_bytes
        .checked_add(header_bytes)
        .ok_or(TileJobError::ArithmeticOverflow)?;
    Ok(ResidentTileCost {
        tile_count,
        sample_bytes,
        header_bytes,
        logical_bytes,
    })
}

/// Computes the exact logical cost-table row for `tile_count` default tiles.
///
/// # Errors
///
/// Refuses byte arithmetic overflow.
pub fn resident_tile_cost(tile_count: u32) -> Result<ResidentTileCost, TileJobError> {
    tile_cost(TileGeometry::DEFAULT, tile_count)
}

/// Derived capacity facts for a protected-backdrop resident profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ResidentTileProfile {
    pub total_tiles: u32,
    pub backdrop_tiles: u32,
    pub detail_tiles: u32,
    pub sample_pages: u32,
    pub descriptor_pages: u32,
    pub other_pages: u32,
    pub total_data_pages: u32,
    pub span_directory_entries: u32,
    pub minimum_span_capacity: u32,
    pub cost: ResidentTileCost,
}

impl ResidentTileProfile {
    /// Resident-profile schema version.
    pub const VERSION: u32 = 1;
    /// Exact encoded byte size.
    pub const BYTE_SIZE: usize = 72;

    /// Returns exact physical DATA bytes for this delivered profile.
    #[must_use]
    pub const fn physical_data_bytes(self) -> Option<u64> {
        DescriptorCostLedger::physical_data_bytes(self.total_data_pages as u64)
    }

    /// Derives the page and span-directory requirements for the version-one two-page tile ABI.
    ///
    /// # Errors
    ///
    /// Refuses an impossible backdrop count or fixed-width arithmetic overflow.
    pub fn new(
        total_tiles: u32,
        backdrop_tiles: u32,
        other_pages: u32,
    ) -> Result<Self, TileJobError> {
        let detail_tiles = total_tiles
            .checked_sub(backdrop_tiles)
            .ok_or(TileJobError::InvalidResidentProfile)?;
        let sample_pages = total_tiles
            .checked_mul(DescriptorCostLedger::SAMPLE_PAGES_PER_TILE)
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let descriptor_pages = 1_u32;
        let total_data_pages = sample_pages
            .checked_add(descriptor_pages)
            .and_then(|pages| pages.checked_add(other_pages))
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let span_directory_entries = total_tiles
            .checked_mul(2)
            .and_then(|entries| entries.checked_add(1))
            .ok_or(TileJobError::ArithmeticOverflow)?;
        let minimum_span_capacity = span_directory_entries
            .checked_next_power_of_two()
            .ok_or(TileJobError::ArithmeticOverflow)?;
        Ok(Self {
            total_tiles,
            backdrop_tiles,
            detail_tiles,
            sample_pages,
            descriptor_pages,
            other_pages,
            total_data_pages,
            span_directory_entries,
            minimum_span_capacity,
            cost: resident_tile_cost(total_tiles)?,
        })
    }

    /// Existing 64-page DATA profile: 28 tiles, 12 backdrop, and 16 Detail/history.
    ///
    /// # Errors
    ///
    /// Returns an arithmetic refusal if the fixed profile no longer fits its integer fields.
    pub fn constrained() -> Result<Self, TileJobError> {
        Self::new(28, 12, 7)
    }

    /// Expanded 120-page DATA profile: 56 tiles, 12 backdrop, and 44 Detail/history.
    ///
    /// # Errors
    ///
    /// Returns an arithmetic refusal if the fixed profile no longer fits its integer fields.
    pub fn expanded() -> Result<Self, TileJobError> {
        Self::new(56, 12, 7)
    }
}

#[cfg(test)]
mod tests {
    use core::mem::{align_of, size_of};

    use super::*;
    use ember_julibrot_math::{Homography, Plane};
    use ember_lab_heap::SpanArena;

    fn identities() -> (MainIdentity, ReferenceIdentity) {
        let main = MainIdentity {
            content: ContentIdentity(7),
            generation: 11,
        };
        let reference = ReferenceIdentity {
            main,
            generation: 13,
        };
        (main, reference)
    }

    fn job(id: u64, coverage: CoverageClass, benefit: u64, refinement: RefinementLevel) -> TileJob {
        let (main, reference) = identities();
        TileJob::new(
            main,
            reference,
            SourceScreenRect::new(-1, 255, TileGeometry::DEFAULT),
            refinement,
            DemandKey::new(coverage, benefit, 100, StableJobId(id)),
        )
        .expect("fixture job is valid")
    }

    #[test]
    fn default_source_geometry_is_256_physical_254_core_with_apron() {
        assert_eq!(TileGeometry::DEFAULT.physical_extent(), [256, 256]);
        assert_eq!(TileGeometry::DEFAULT.core_extent(), [254, 254]);
        assert_eq!(TileGeometry::DEFAULT.apron(), 1);
        assert_eq!(TileGeometry::DEFAULT.sample_count(), 65_536);
        assert_eq!(
            SourceScreenRect::new(-1, 255, TileGeometry::DEFAULT)
                .core_rect(TileGeometry::DEFAULT)
                .expect("core is representable"),
            [0, 256, 254, 254]
        );
        assert_eq!(
            TileGeometry::new(128, 64, 2)
                .expect("parameterized geometry")
                .core_extent(),
            [124, 60]
        );
    }

    #[test]
    fn demand_queue_is_order_independent_and_every_hole_precedes_every_upgrade() {
        let jobs = [
            job(
                40,
                CoverageClass::DetailUpgrade,
                10_000,
                RefinementLevel::Final,
            ),
            job(30, CoverageClass::ClosesHole, 1, RefinementLevel::Final),
            job(20, CoverageClass::ClosesHole, 1, RefinementLevel::Preview),
            job(
                10,
                CoverageClass::DetailUpgrade,
                20_000,
                RefinementLevel::Interactive,
            ),
        ];
        let drain = |order: [usize; 4]| {
            let mut queue = TileDemandQueue::new();
            for index in order {
                queue.push(jobs[index].clone()).expect("unique fixture job");
            }
            std::iter::from_fn(|| queue.pop().map(|next| next.stable_id())).collect::<Vec<_>>()
        };
        let expected = [
            StableJobId(20),
            StableJobId(30),
            StableJobId(10),
            StableJobId(40),
        ];
        for order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2], [2, 0, 3, 1]] {
            assert_eq!(drain(order), expected);
        }
    }

    #[test]
    fn visible_benefit_descends_then_work_and_id_ascend() {
        let keys = [
            DemandKey::new(CoverageClass::ClosesHole, 5, 20, StableJobId(3)),
            DemandKey::new(CoverageClass::ClosesHole, 6, 30, StableJobId(4)),
            DemandKey::new(CoverageClass::ClosesHole, 5, 10, StableJobId(2)),
            DemandKey::new(CoverageClass::ClosesHole, 5, 10, StableJobId(1)),
        ];
        let mut sorted = keys;
        sorted.sort();
        assert_eq!(
            sorted.map(DemandKey::stable_job_id),
            [
                StableJobId(4),
                StableJobId(1),
                StableJobId(2),
                StableJobId(3)
            ]
        );
    }

    #[test]
    fn a_draft_can_fill_a_hole_but_never_displaces_better_same_surface_quality() {
        let preview = TileQuality {
            residency: TileResidency::Detail,
            rung: TileRung::Preview,
            density: ExactF64::new(64.0),
            error: ExactF64::new(0.1),
            age: 1,
            tile_id: 4,
        };
        let final_quality = TileQuality {
            rung: TileRung::Final,
            ..preview
        };
        assert!(!preview.should_replace(final_quality));
        assert!(final_quality.should_replace(preview));
        let dense_preview = TileQuality {
            density: ExactF64::new(4_096.0),
            ..preview
        };
        assert!(dense_preview.should_replace(preview));
        assert!(!preview.should_replace(dense_preview));
        let backdrop_with_unused_final_label = TileQuality {
            residency: TileResidency::Backdrop,
            rung: TileRung::Final,
            density: ExactF64::new(4_096.0),
            ..preview
        };
        assert!(preview.should_replace(backdrop_with_unused_final_label));
        assert!(!backdrop_with_unused_final_label.should_replace(preview));
        assert_eq!(
            job(1, CoverageClass::ClosesHole, 1, RefinementLevel::Preview)
                .demand
                .coverage_class(),
            CoverageClass::ClosesHole
        );
    }

    #[test]
    fn paired_outputs_publish_only_after_both_equal_spans_complete() {
        let mut arena = SpanArena::new(256, 2, 8, 512, 16).expect("fixture arena");
        let value = arena.allocate_span(65_536, 256).expect("value span fits");
        let reconstruction = arena
            .allocate_span(65_536, 256)
            .expect("reconstruction span fits");
        let plan = PairedOutputSpanPlan::new(
            &job(1, CoverageClass::ClosesHole, 1, RefinementLevel::Preview),
            value,
            reconstruction,
        )
        .expect("paired plan matches tile");
        let mut completion = plan.begin();
        completion.complete(TileOutput::Value);
        assert!(!completion.is_publishable());
        let mut completion = completion
            .try_publish()
            .expect_err("value-only is not publishable");
        completion.complete(TileOutput::Reconstruction);
        let published = completion
            .try_publish()
            .expect("the complete pair publishes");
        assert_eq!(published.spans().job_id, StableJobId(1));

        let value_identity = TileSpanIdentity::new(3, 7, 65_536);
        let reconstruction_identity = TileSpanIdentity::new(4, 9, 65_536);
        assert_eq!(
            PairedTileSpanIdentities::new(value_identity, reconstruction_identity),
            Ok(PairedTileSpanIdentities {
                value: value_identity,
                reconstruction: reconstruction_identity,
            })
        );
        assert_eq!(
            PairedTileSpanIdentities::new(value_identity, TileSpanIdentity::new(3, 8, 65_536),),
            Err(TileJobError::OutputAlias)
        );
        assert_eq!(
            PairedTileSpanIdentities::new(value_identity, TileSpanIdentity::new(4, 9, 65_535),),
            Err(TileJobError::OutputShapeMismatch)
        );
    }

    fn canonical_slice() -> SliceIdentity {
        SliceIdentity::new(
            Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            [0.0; 4],
        )
    }

    fn source_pose() -> Pose {
        Pose {
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
        }
    }

    #[test]
    fn keys_use_exact_equality_and_capture_render_and_source_identity() {
        let pose = source_pose();
        let rect = SourceScreenRect::new(-4, 8, TileGeometry::DEFAULT);
        let source = SourceIdentity::new(6, 7, 8);
        let key = TileRenderKey::from_pose_and_source(&pose, rect, source);
        assert_eq!(
            key,
            TileRenderKey::from_pose_and_source(&pose, rect, source)
        );
        let mut changed = pose;
        changed.zoom_log2 = f64::from_bits(pose.zoom_log2.to_bits() + 1);
        assert_ne!(
            key,
            TileRenderKey::from_pose_and_source(&changed, rect, source)
        );
        changed = pose;
        changed.view.camera_translation[4] = -0.0;
        assert_ne!(
            key,
            TileRenderKey::from_pose_and_source(&changed, rect, source)
        );
        assert_ne!(
            key,
            TileRenderKey::from_pose_and_source(&pose, rect, SourceIdentity::new(6, 8, 8))
        );
        assert_eq!(key.extent, [960, 540]);
        assert_eq!(key.main_generation, 17);
        assert_eq!(key.source_rect.x, -4);
        assert_eq!(
            TileRenderKey::from_pose(&pose, rect).source_identity,
            SourceIdentity::FINITE_MIRROR
        );
        assert_ne!(TileMeshHandle::new(3, 7), TileMeshHandle::new(3, 8));

        let content = TileContentKey {
            version: TileContentKey::VERSION,
            slice: canonical_slice(),
            main_generation: 4,
            iteration_cap: 512,
            formula_abi: 1,
            precision_mode: PrecisionMode::PictureFast,
            record_abi: 1,
            reference_generation: 4,
        };
        assert_eq!(content, content);
        assert_ne!(
            content,
            TileContentKey {
                main_generation: 5,
                ..content
            }
        );
    }

    const EXPECTED_DESCRIPTOR_ABI_LAYOUT: [(&str, u16, u16, &str); 21] = [
        ("tile_id,content_key_id,anchor_id,flags", 0, 16, "H00"),
        (
            "value_span,lifted_span,ownership_base,header_generation",
            16,
            16,
            "H01",
        ),
        (
            "six ordered object rotation factor pairs",
            32,
            48,
            "H02-H04",
        ),
        (
            "ten ordered camera rotation factor pairs",
            80,
            80,
            "H05-H09",
        ),
        ("cos_yaw,sin_yaw,cos_pitch,sin_pitch", 160, 16, "H10"),
        (
            "origin0_hi..origin3_hi,origin0_lo..origin3_lo",
            176,
            32,
            "H11-H12",
        ),
        ("t0,t1,t2,t3", 208, 16, "H13"),
        ("t4,height,d5,d4", 224, 16, "H14"),
        ("zoom_log2,extent_w,extent_h,chart_density", 240, 16, "H15"),
        ("rect_x,rect_y,rect_w,rect_h", 256, 16, "H16"),
        (
            "anchor_dx_hi,anchor_dx_lo,anchor_dy_hi,anchor_dy_lo",
            272,
            16,
            "H17",
        ),
        ("three padded accepted source-map rows", 288, 48, "H18-H20"),
        (
            "depth_min,depth_max,coordinate_error,reprojection_error",
            336,
            16,
            "H21",
        ),
        (
            "residency_rank,refinement_rung,iteration_cap,age_rank",
            352,
            16,
            "H22",
        ),
        (
            "valid_count,glitch_count,uncertain_count,mesh_class",
            368,
            16,
            "H23",
        ),
        (
            "chart_scale_hi,chart_scale_lo,anchor_precision_bits,anchor_revision",
            384,
            16,
            "H24",
        ),
        (
            "slice_key_id,MAIN_generation,record_ABI,reference_generation",
            400,
            16,
            "H25",
        ),
        (
            "ownership_count,ownership_revision,value_generation,lifted_generation",
            416,
            16,
            "H26",
        ),
        ("positive zero", 432, 80, "H27-H31"),
        ("smooth_iter,escaped,rebase_count,status", 0, 16, "S0[k]"),
        ("a_F,b_F,zeta_F,validity", 16, 16, "S1[k]"),
    ];

    #[test]
    fn descriptor_abi_table_pins_every_offset_size_and_lane() {
        let actual =
            DESCRIPTOR_ABI_LAYOUT.map(|field| (field.field, field.offset, field.size, field.lane));
        assert_eq!(actual, EXPECTED_DESCRIPTOR_ABI_LAYOUT);

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
        assert_eq!(named_indices, core::array::from_fn(|index| index));
    }

    #[test]
    fn module_documentation_states_the_exact_descriptor_abi_table() {
        let source = include_str!("tile_job.rs");
        for row in [
            "//! |`tile_id,content_key_id,anchor_id,flags`|0|16|`H00`|",
            "//! |`value_span,lifted_span,ownership_base,header_generation`|16|16|`H01`|",
            "//! |Six ordered `(cos O_ij,sin O_ij)` pairs|32|48|`H02-H04`|",
            "//! |Ten ordered `(cos Q_ij,sin Q_ij)` pairs|80|80|`H05-H09`|",
            "//! |`cos_yaw,sin_yaw,cos_pitch,sin_pitch`|160|16|`H10`|",
            "//! |`origin0_hi..origin3_hi,origin0_lo..origin3_lo`|176|32|`H11-H12`|",
            "//! |`t0,t1,t2,t3`|208|16|`H13`|",
            "//! |`t4,height,d5,d4`|224|16|`H14`|",
            "//! |`zoom_log2,extent_w,extent_h,chart_density`|240|16|`H15`|",
            "//! |`rect_x,rect_y,rect_w,rect_h`|256|16|`H16`|",
            "//! |`anchor_dx_hi,anchor_dx_lo,anchor_dy_hi,anchor_dy_lo`|272|16|`H17`|",
            "//! |Three padded rows of the accepted source map|288|48|`H18-H20`|",
            "//! |`depth_min,depth_max,coordinate_error,reprojection_error`|336|16|`H21`|",
            "//! |`residency_rank,refinement_rung,iteration_cap,age_rank`|352|16|`H22`|",
            "//! |`valid_count,glitch_count,uncertain_count,mesh_class`|368|16|`H23`|",
            "//! |`chart_scale_hi,chart_scale_lo,anchor_precision_bits,anchor_revision`|384|16|`H24`|",
            "//! |`slice_key_id,MAIN_generation,record_ABI,reference_generation`|400|16|`H25`|",
            "//! |`ownership_count,ownership_revision,value_generation,lifted_generation`|416|16|`H26`|",
            "//! |Positive zero|432|80|`H27-H31`|",
            "//! |`smooth_iter,escaped,rebase_count,status`|0|16|`S0[k]`|",
            "//! |`a_F,b_F,zeta_F,validity`|16|16|`S1[k]`|",
        ] {
            assert!(source.contains(row), "missing ABI documentation row: {row}");
        }
    }

    #[test]
    fn record_versions_and_byte_sizes_are_exact() {
        macro_rules! assert_record {
            ($record:ty, $version:expr, $bytes:expr) => {
                assert_eq!(<$record>::VERSION, $version);
                assert_eq!(<$record>::BYTE_SIZE, $bytes);
                assert_eq!(size_of::<$record>(), $bytes);
            };
        }

        assert_record!(ExactF32, 1, 4);
        assert_record!(ExactF64, 1, 8);
        assert_record!(SliceIdentity, 1, 64);
        assert_record!(TileContentKey, 1, 96);
        assert_record!(SourceIdentity, 1, 16);
        assert_record!(PoseMapKey, 1, 168);
        assert_record!(ContentIdentity, 1, 8);
        assert_record!(MainIdentity, 1, 16);
        assert_record!(ReferenceIdentity, 1, 24);
        assert_record!(StableJobId, 1, 8);
        assert_record!(TileGeometry, 1, 12);
        assert_record!(SourceScreenRect, 1, 16);
        assert_record!(TileRenderKey, 1, 536);
        assert_record!(TileMeshHandle, 1, 8);
        assert_record!(TileSpanIdentity, 1, 12);
        assert_record!(PairedTileSpanIdentities, 1, 24);
        assert_record!(PairedOutputAllocation, 1, 48);
        assert_record!(TileOutput, 1, 4);
        assert_record!(TileOutputCompletion, 1, 8);
        assert_record!(PairedOutputReceipt, 1, 64);
        assert_record!(DescriptorTexel, 1, 16);
        assert_record!(TilePoseHeader, 1, 512);
        assert_record!(DescriptorSamplePair, 1, 32);
        assert_record!(SourceReconstructionUniform, 1, 144);
        assert_record!(TileResidency, 1, 4);
        assert_record!(TileRung, 1, 4);
        assert_eq!(TileRung::Preview as u32, 0);
        assert_eq!(TileRung::Interactive as u32, 1);
        assert_eq!(TileRung::Final as u32, 2);
        assert_record!(CanonicalChartCellKey, 1, 88);
        assert_record!(TileQuality, 1, 40);
        assert_record!(RenderControlChange, 1, 4);
        assert_record!(TileInvalidation, 1, 4);
        assert_record!(TransitionPresentation, 1, 4);
        assert_record!(ResidentTileCost, 1, 32);
        assert_record!(ResidentTileProfile, 1, 72);
        assert_eq!(align_of::<DescriptorTexel>(), 16);
        assert_eq!(align_of::<TilePoseHeader>(), 16);
        assert_eq!(DescriptorCostLedger::ABI_VERSION, 1);
    }

    #[test]
    fn descriptor_lanes_header_slot_and_cost_ledger_are_exact() {
        let pair = DescriptorSamplePair::new([31.0, 1.0, 2.0, 0.0], [0.25, -0.5, 7.0, 1.0]);
        assert_eq!(pair.s0.lanes, [31.0, 1.0, 2.0, 0.0]);
        assert_eq!(pair.s1.lanes, [0.25, -0.5, 7.0, 1.0]);
        assert_eq!(
            DescriptorSamplePair::S0_LANES,
            "smooth_iter,escaped,rebase_count,status"
        );
        assert_eq!(DescriptorSamplePair::S1_LANES, "a_F,b_F,zeta_F,validity");
        assert_eq!(DescriptorCostLedger::SAMPLE_BYTES_PER_TILE, 2_097_152);
        assert_eq!(DescriptorCostLedger::HEADER_BYTES_PER_TILE, 512);
        assert_eq!(DescriptorCostLedger::LOGICAL_BYTES_PER_TILE, 2_097_664);
        assert_eq!(SourceScreenRect::PHYSICAL_SIDE, 256);
        assert_eq!(SourceScreenRect::CORE_SIDE, 254);
        assert_eq!(SourceScreenRect::APRON_SAMPLES, 1);
        assert_eq!(DescriptorCostLedger::ACTIVE_PREFIX_RECORDS, 64);
        assert_eq!(DescriptorCostLedger::HEADER_SLOTS, 64);
        assert_eq!(DescriptorCostLedger::OWNERSHIP_RECORDS, 63_424);
        assert_eq!(DescriptorCostLedger::DESCRIPTOR_PAGE_RECORDS, 65_536);
        assert_eq!(DescriptorCostLedger::DATA_PAGE_BYTES, 1_048_576);
        assert_eq!(DescriptorCostLedger::SAMPLE_PAGES_PER_TILE, 2);
        assert_eq!(
            DescriptorCostLedger::paired_sample_logical_bytes(65_536),
            Some(2_097_152)
        );
        assert_eq!(
            DescriptorCostLedger::paired_reserved_bytes(1_048_576, 1_048_576),
            Some(2_097_152)
        );
        assert_eq!(
            DescriptorCostLedger::paired_reserved_bytes(u64::MAX, 1),
            None
        );
        assert_eq!(
            DescriptorCostLedger::paired_sample_logical_bytes(u64::MAX),
            None
        );
        assert_eq!(
            DescriptorCostLedger::ACTIVE_PREFIX_RECORDS
                + DescriptorCostLedger::HEADER_SLOTS * TilePoseHeader::TEXELS as u64
                + DescriptorCostLedger::OWNERSHIP_RECORDS,
            DescriptorCostLedger::DESCRIPTOR_PAGE_RECORDS
        );
        for (count, bytes) in [
            (1, 2_097_664),
            (9, 18_878_976),
            (12, 25_171_968),
            (16, 33_562_624),
            (28, 58_734_592),
            (44, 92_297_216),
            (56, 117_469_184),
        ] {
            assert_eq!(DescriptorCostLedger::logical_bytes(count), Some(bytes));
        }
    }

    #[test]
    fn paired_allocation_is_generation_tagged_non_aliasing_and_profile_checked() {
        let mut arena = SpanArena::new(256, 2, 8, 512, 16).expect("fixture arena");
        let [value, reconstruction] = arena
            .allocate_pair(65_536, 256)
            .expect("one whole-grid pair fits atomically");
        let allocation = PairedOutputAllocation::from_spans(23, &value, &reconstruction, 2_097_152)
            .expect("the exact pair fits its resident profile");
        assert_eq!(
            allocation,
            PairedOutputAllocation {
                generation: 23,
                spans: PairedTileSpanIdentities {
                    value: TileSpanIdentity::new(value.directory_index, 23, 65_536),
                    reconstruction: TileSpanIdentity::new(
                        reconstruction.directory_index,
                        23,
                        65_536,
                    ),
                },
                logical_bytes: 2_097_152,
                reserved_bytes: 2_097_152,
            }
        );
        assert_ne!(
            allocation.spans.value.directory_index,
            allocation.spans.reconstruction.directory_index
        );
        assert_eq!(allocation.spans.value.generation, allocation.generation);
        assert_eq!(
            allocation.spans.reconstruction.generation,
            allocation.generation
        );
        assert_eq!(
            PairedOutputAllocation::from_spans(23, &value, &reconstruction, 2_097_151),
            Err(TileJobError::OutputProfileTooSmall)
        );
        assert_eq!(
            PairedOutputAllocation::from_spans(23, &value, &value, 2_097_152),
            Err(TileJobError::OutputAlias)
        );
        let value_completion = TileOutputCompletion::new(23, TileOutput::Value);
        let reconstruction_completion = TileOutputCompletion::new(23, TileOutput::Reconstruction);
        let receipt =
            PairedOutputReceipt::new(allocation, value_completion, reconstruction_completion)
                .expect("both current-generation completions produce one receipt");
        assert_eq!(receipt.allocation, allocation);
        assert_eq!(
            receipt.completions,
            [value_completion, reconstruction_completion]
        );
        assert_eq!(
            PairedOutputReceipt::new(
                allocation,
                TileOutputCompletion::new(22, TileOutput::Value),
                reconstruction_completion,
            ),
            Err(TileJobError::StaleOutputCompletion)
        );
        assert_eq!(
            PairedOutputReceipt::new(
                allocation,
                reconstruction_completion,
                reconstruction_completion,
            ),
            Err(TileJobError::OutputCompletionMismatch)
        );
    }

    #[test]
    fn reconstruction_uniform_selects_exact_source_header_lanes() {
        let mut header = TilePoseHeader::zeroed();
        for texel in
            &mut header.texels[TilePoseHeader::H05_CAMERA_12_13..=TilePoseHeader::H09_CAMERA_35_45]
        {
            texel.lanes = [1.0, 0.0, 1.0, 0.0];
        }
        header.texels[TilePoseHeader::H10_OBSERVER].lanes = [1.0, 0.0, 1.0, 0.0];
        header.texels[TilePoseHeader::H13_TRANSLATION_0_3].lanes = [1.0, 2.0, 3.0, 4.0];
        header.texels[TilePoseHeader::H14_PROJECTION].lanes = [5.0, 2.0, 8.0, 16.0];
        header.texels[TilePoseHeader::H24_SCALE_ANCHOR].lanes =
            [0.003_906_25, 0.000_000_25, 0.0, 0.0];
        let uniform = SourceReconstructionUniform::from_header(&header)
            .expect("valid source lanes construct the paired uniform");
        assert_eq!(uniform.camera_rotation_pairs, [[1.0, 0.0, 1.0, 0.0]; 5]);
        assert_eq!(uniform.camera_translation[0], [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(uniform.camera_translation[1], [5.0, 0.0, 0.0, 0.0]);
        assert_eq!(uniform.observer_rotation, [1.0, 0.0, 1.0, 0.0]);
        assert_eq!(uniform.view_scale, [2.0, 8.0, 16.0, 0.003_906_5]);
        assert_eq!(
            uniform.bytes().len(),
            SourceReconstructionUniform::BYTE_SIZE
        );

        header.texels[TilePoseHeader::H14_PROJECTION].lanes[2] = 0.0;
        assert_eq!(SourceReconstructionUniform::from_header(&header), None);
    }

    #[test]
    fn descriptor_pod_layout_round_trips_raw_bytes() {
        #[repr(C, align(16))]
        struct AlignedBytes<const N: usize>([u8; N]);

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
    }

    #[test]
    fn same_surface_owner_is_independent_of_catalog_order() {
        let backdrop = TileQuality {
            residency: TileResidency::Backdrop,
            rung: TileRung::Preview,
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
            slice: canonical_slice(),
            level: 5,
            x: -7,
            y: 11,
        };
        assert_eq!(cell, cell);
    }

    fn verify_quality_permutations(
        candidates: &mut [TileQuality],
        remaining: usize,
        expected: TileQuality,
        permutation_count: &mut u32,
    ) {
        if remaining == 1 {
            assert_eq!(
                select_same_surface_owner(candidates.iter().copied()),
                Some(expected)
            );
            *permutation_count += 1;
            return;
        }
        for index in 0..remaining {
            verify_quality_permutations(candidates, remaining - 1, expected, permutation_count);
            let swap_index = if remaining.is_multiple_of(2) {
                index
            } else {
                0
            };
            candidates.swap(swap_index, remaining - 1);
        }
    }

    #[test]
    fn every_same_surface_quality_corpus_permutation_has_one_owner() {
        let winner = TileQuality {
            residency: TileResidency::Detail,
            rung: TileRung::Final,
            density: ExactF64::new(2.0),
            error: ExactF64::new(0.01),
            age: 9,
            tile_id: 7,
        };
        let mut corpus = [
            TileQuality {
                residency: TileResidency::Backdrop,
                rung: TileRung::Final,
                density: ExactF64::new(4_096.0),
                error: ExactF64::new(0.0),
                age: u64::MAX,
                tile_id: 1,
            },
            TileQuality {
                rung: TileRung::Preview,
                density: ExactF64::new(4_096.0),
                error: ExactF64::new(0.0),
                age: u64::MAX,
                tile_id: 1,
                ..winner
            },
            TileQuality {
                rung: TileRung::Interactive,
                density: ExactF64::new(4_096.0),
                error: ExactF64::new(0.0),
                age: u64::MAX,
                tile_id: 1,
                ..winner
            },
            TileQuality {
                density: ExactF64::new(1.0),
                error: ExactF64::new(0.0),
                age: u64::MAX,
                tile_id: 1,
                ..winner
            },
            TileQuality {
                error: ExactF64::new(0.02),
                age: u64::MAX,
                tile_id: 1,
                ..winner
            },
            TileQuality { age: 8, ..winner },
            TileQuality {
                tile_id: 8,
                ..winner
            },
            winner,
        ];
        let mut permutation_count = 0;
        let remaining = corpus.len();
        verify_quality_permutations(&mut corpus, remaining, winner, &mut permutation_count);
        assert_eq!(permutation_count, 40_320);
    }

    #[test]
    fn invalidation_matrix_covers_reprojection_and_partition_transitions() {
        let keep = [
            RenderControlChange::Camera,
            RenderControlChange::Translation,
            RenderControlChange::Height,
            RenderControlChange::DistanceFive,
            RenderControlChange::DistanceFour,
            RenderControlChange::Observer,
            RenderControlChange::Zoom,
            RenderControlChange::Extent,
            RenderControlChange::PlanePreservingObject,
            RenderControlChange::InPlaneOrigin,
        ];
        let partition = [
            RenderControlChange::SliceTilt,
            RenderControlChange::OutOfPlaneOrigin,
            RenderControlChange::IterationCap,
            RenderControlChange::FormulaAbi,
            RenderControlChange::Precision,
            RenderControlChange::RecordAbi,
            RenderControlChange::MainGeneration,
        ];
        for change in keep {
            assert_eq!(tile_invalidation(change), TileInvalidation::Keep);
            assert_eq!(
                transition_presentation(change),
                TransitionPresentation::Reproject
            );
        }
        for change in partition {
            assert_eq!(tile_invalidation(change), TileInvalidation::NewPartition);
            assert_eq!(
                transition_presentation(change),
                TransitionPresentation::HoldPrevious
            );
        }
        assert_eq!(
            tile_invalidation(RenderControlChange::Display),
            TileInvalidation::Keep
        );
        assert_eq!(
            transition_presentation(RenderControlChange::Display),
            TransitionPresentation::ShadeCurrent
        );
    }

    const EXPECTED_INVALIDATION_MATRIX: [(
        RenderControlChange,
        TileInvalidation,
        TransitionPresentation,
        &str,
    ); 18] = [
        (
            RenderControlChange::Camera,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Camera|Query footprint; reproject|",
        ),
        (
            RenderControlChange::Translation,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Translation|Query footprint; reproject|",
        ),
        (
            RenderControlChange::Height,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Height|Query footprint; reproject|",
        ),
        (
            RenderControlChange::DistanceFive,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|DistanceFive|Query footprint; reproject|",
        ),
        (
            RenderControlChange::DistanceFour,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|DistanceFour|Query footprint; reproject|",
        ),
        (
            RenderControlChange::Observer,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Observer|Query footprint; reproject|",
        ),
        (
            RenderControlChange::Zoom,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Zoom|Query footprint; reproject|",
        ),
        (
            RenderControlChange::Extent,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|Extent|Query footprint; reproject|",
        ),
        (
            RenderControlChange::PlanePreservingObject,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|PlanePreservingObject|Transform chart; reproject|",
        ),
        (
            RenderControlChange::InPlaneOrigin,
            TileInvalidation::Keep,
            TransitionPresentation::Reproject,
            "//! |Keep|InPlaneOrigin|Transform chart; reproject|",
        ),
        (
            RenderControlChange::Display,
            TileInvalidation::Keep,
            TransitionPresentation::ShadeCurrent,
            "//! |Keep|Display|No index action; shade current|",
        ),
        (
            RenderControlChange::SliceTilt,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|SliceTilt|New slice index; hold|",
        ),
        (
            RenderControlChange::OutOfPlaneOrigin,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|OutOfPlaneOrigin|New slice index; hold|",
        ),
        (
            RenderControlChange::IterationCap,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|IterationCap|New MAIN index; hold|",
        ),
        (
            RenderControlChange::FormulaAbi,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|FormulaAbi|New MAIN index; hold|",
        ),
        (
            RenderControlChange::Precision,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|Precision|New MAIN index; hold|",
        ),
        (
            RenderControlChange::RecordAbi,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|RecordAbi|New MAIN index; hold|",
        ),
        (
            RenderControlChange::MainGeneration,
            TileInvalidation::NewPartition,
            TransitionPresentation::HoldPrevious,
            "//! |NewPartition|MainGeneration|New MAIN index; hold|",
        ),
    ];

    #[test]
    fn module_invalidation_matrix_pins_every_state_event_and_result() {
        assert_eq!(
            EXPECTED_INVALIDATION_MATRIX.map(|(event, _, _, _)| event),
            RenderControlChange::ALL
        );
        let source = include_str!("tile_job.rs");
        for (event, state, result, documented_row) in EXPECTED_INVALIDATION_MATRIX {
            assert_eq!(tile_invalidation(event), state);
            assert_eq!(transition_presentation(event), result);
            assert!(source.contains(documented_row));
        }
    }

    #[test]
    fn every_reserved_header_texel_and_negative_zero_are_refused() {
        assert_eq!(TilePoseHeader::RESERVED_START, 27);
        for texel in 27..32 {
            let mut header = TilePoseHeader::zeroed();
            header.texels[texel].lanes[0] = 1.0;
            assert_eq!(
                validate_pose_header(&header),
                Err(DescriptorAbiError::ReservedHeaderLane)
            );
        }

        let mut header = TilePoseHeader::zeroed();
        header.texels[31].lanes[3] = -0.0;
        assert_eq!(
            validate_pose_header(&header),
            Err(DescriptorAbiError::ReservedHeaderLane)
        );
    }

    #[test]
    fn reference_is_shared_and_counted_once_per_main_generation() {
        let mut leases = ReferenceLeaseSet::new();
        let first_job = job(1, CoverageClass::ClosesHole, 1, RefinementLevel::Preview);
        let second_job = job(2, CoverageClass::DetailUpgrade, 2, RefinementLevel::Final);
        let first = leases
            .acquire(&first_job)
            .expect("first job pins reference");
        let second = leases
            .acquire(&second_job)
            .expect("second job shares reference");
        assert_eq!(leases.lease_count(first_job.main), 2);
        assert_eq!(
            leases.pinned_reference(first_job.main),
            Some(first_job.reference)
        );
        assert_eq!(leases.release(first).expect("first pin releases"), 1);
        assert_eq!(leases.release(second).expect("second pin releases"), 0);
        assert_eq!(leases.pinned_reference(first_job.main), None);
    }

    #[test]
    fn conflicting_reference_is_refused_while_a_main_generation_is_pinned() {
        let mut leases = ReferenceLeaseSet::new();
        let first_job = job(1, CoverageClass::ClosesHole, 1, RefinementLevel::Preview);
        assert!(leases.acquire(&first_job).is_ok());
        let mut conflicting = first_job.clone();
        conflicting.reference.generation += 1;
        assert_eq!(
            leases.acquire(&conflicting),
            Err(TileJobError::ReferenceConflict)
        );
    }

    #[test]
    fn cost_table_and_both_profiles_are_exact() {
        assert_eq!(DEFAULT_TILE_CORE_SIDE, 254);
        assert_eq!(DEFAULT_TILE_SAMPLE_BYTES, 2_097_152);
        assert_eq!(DEFAULT_TILE_LOGICAL_BYTES, 2_097_664);
        let expected = [
            (1, 2_097_152, 512, 2_097_664),
            (9, 18_874_368, 4_608, 18_878_976),
            (12, 25_165_824, 6_144, 25_171_968),
            (16, 33_554_432, 8_192, 33_562_624),
            (28, 58_720_256, 14_336, 58_734_592),
            (44, 92_274_688, 22_528, 92_297_216),
            (56, 117_440_512, 28_672, 117_469_184),
        ];
        for (count, sample_bytes, header_bytes, logical_bytes) in expected {
            assert_eq!(
                resident_tile_cost(count).expect("cost fits"),
                ResidentTileCost {
                    tile_count: count,
                    sample_bytes,
                    header_bytes,
                    logical_bytes,
                }
            );
        }
        assert_eq!(
            tile_cost(TileGeometry::new(128, 128, 1).expect("128 tile"), 1).expect("cost fits"),
            ResidentTileCost {
                tile_count: 1,
                sample_bytes: 524_288,
                header_bytes: 512,
                logical_bytes: 524_800,
            }
        );
        assert_eq!(
            tile_cost(TileGeometry::new(512, 512, 1).expect("512 tile"), 1).expect("cost fits"),
            ResidentTileCost {
                tile_count: 1,
                sample_bytes: 8_388_608,
                header_bytes: 512,
                logical_bytes: 8_389_120,
            }
        );
        let constrained = ResidentTileProfile::constrained().expect("profile fits");
        assert_eq!(
            (
                constrained.total_tiles,
                constrained.backdrop_tiles,
                constrained.detail_tiles,
                constrained.sample_pages,
                constrained.total_data_pages,
                constrained.span_directory_entries,
                constrained.minimum_span_capacity,
            ),
            (28, 12, 16, 56, 64, 57, 64)
        );
        let expanded = ResidentTileProfile::expanded().expect("profile fits");
        assert_eq!(
            (
                expanded.total_tiles,
                expanded.backdrop_tiles,
                expanded.detail_tiles,
                expanded.sample_pages,
                expanded.total_data_pages,
                expanded.span_directory_entries,
                expanded.minimum_span_capacity,
            ),
            (56, 12, 44, 112, 120, 113, 128)
        );
    }

    #[test]
    fn frozen_profiles_pin_physical_bytes_and_overflow_refusals() {
        let constrained = ResidentTileProfile::constrained().expect("constrained profile fits");
        assert_eq!(constrained.physical_data_bytes(), Some(67_108_864));
        assert_eq!(constrained.cost.logical_bytes, 58_734_592);
        let expanded = ResidentTileProfile::expanded().expect("expanded profile fits");
        assert_eq!(expanded.physical_data_bytes(), Some(125_829_120));
        assert_eq!(expanded.cost.logical_bytes, 117_469_184);

        assert_eq!(DescriptorCostLedger::logical_bytes(u64::MAX), None);
        assert_eq!(DescriptorCostLedger::physical_data_bytes(u64::MAX), None);
        let largest_geometry =
            TileGeometry::new(u32::MAX, 1, 0).expect("largest one-row geometry fits");
        assert_eq!(
            tile_cost(largest_geometry, u32::MAX),
            Err(TileJobError::ArithmeticOverflow)
        );
        assert_eq!(
            ResidentTileProfile::new(u32::MAX, 0, 0),
            Err(TileJobError::ArithmeticOverflow)
        );
    }
}
