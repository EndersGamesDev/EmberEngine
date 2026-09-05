//! Wire-free stage-0 policy for future rendered-tile work.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

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
pub const TILE_SAMPLE_RECORD_BYTES: u64 = 16;
/// Bytes in one tile's descriptor header slot.
pub const TILE_HEADER_BYTES: u64 = 512;
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
    #[error("the queue already contains the stable job ID")]
    DuplicateJob,
    #[error("the resident profile has fewer tiles than protected backdrop slots")]
    InvalidResidentProfile,
}

/// Stable identity of one semantic rendered-content partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentIdentity(pub u64);

/// One MAIN generation within a content partition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MainIdentity {
    pub content: ContentIdentity,
    pub generation: u64,
}

/// One reference orbit accepted for a MAIN generation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReferenceIdentity {
    pub main: MainIdentity,
    pub generation: u64,
}

/// Reproducible final tie-break for one tile job.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableJobId(pub u64);

/// Parameterized physical/core/apron geometry for one source-screen tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileGeometry {
    physical_width: u32,
    physical_height: u32,
    apron: u32,
}

impl TileGeometry {
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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceScreenRect {
    pub x: i32,
    pub y: i32,
    pub geometry: TileGeometry,
}

impl SourceScreenRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, geometry: TileGeometry) -> Self {
        Self { x, y, geometry }
    }

    /// Drawn core rectangle `[x, y, width, height]` inside the physical source rectangle.
    ///
    /// # Errors
    ///
    /// Refuses an origin whose apron offset cannot be represented by `i32`.
    pub fn core_rect(self) -> Result<[i32; 4], TileJobError> {
        let apron =
            i32::try_from(self.geometry.apron).map_err(|_| TileJobError::ArithmeticOverflow)?;
        let [width, height] = self.geometry.core_extent();
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

/// Detail tiles outrank backdrop tiles only for the same canonical surface claim.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TileResidency {
    Backdrop = 0,
    Detail = 1,
}

/// Monotonic quality used only to prevent same-surface downgrades.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileQuality {
    pub residency: TileResidency,
    pub refinement: RefinementLevel,
    pub density: u32,
}

impl TileQuality {
    #[must_use]
    const fn effective_rung(self) -> u8 {
        match (self.residency, self.refinement) {
            (TileResidency::Backdrop, _) => 0,
            (TileResidency::Detail, RefinementLevel::Preview) => 1,
            (TileResidency::Detail, RefinementLevel::Interactive) => 2,
            (TileResidency::Detail, RefinementLevel::Final) => 3,
        }
    }

    /// A candidate replaces an incumbent only when its effective rung is greater.
    #[must_use]
    pub const fn should_replace(self, incumbent: Self) -> bool {
        self.effective_rung() > incumbent.effective_rung()
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
        let sample_count = job.source_rect.geometry.sample_count();
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
pub enum TileOutput {
    Value,
    Reconstruction,
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
pub struct ResidentTileCost {
    pub tile_count: u32,
    pub sample_bytes: u64,
    pub header_bytes: u64,
    pub logical_bytes: u64,
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
            .checked_mul(2)
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
    use super::*;
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
                .core_rect()
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
            refinement: RefinementLevel::Preview,
            density: 64,
        };
        let final_quality = TileQuality {
            residency: TileResidency::Detail,
            refinement: RefinementLevel::Final,
            density: 64,
        };
        assert!(!preview.should_replace(final_quality));
        assert!(final_quality.should_replace(preview));
        let dense_preview = TileQuality {
            residency: TileResidency::Detail,
            refinement: RefinementLevel::Preview,
            density: 4_096,
        };
        assert!(!dense_preview.should_replace(preview));
        assert!(!preview.should_replace(dense_preview));
        let backdrop_with_unused_final_label = TileQuality {
            residency: TileResidency::Backdrop,
            refinement: RefinementLevel::Final,
            density: 4_096,
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
        let _lease = leases.acquire(&first_job).expect("reference pins");
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
}
