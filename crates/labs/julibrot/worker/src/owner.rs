//! Copy-cell viewer owner and independently staged HOT and MAIN drains.

use std::cell::Cell;

use crate::OrbitResponseView;

/// Current refresh-rate controls and accepted-reference displacement.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct HotState {
    /// Base-two zoom exponent.
    pub zoom_log2: f64,
    /// First independent plane angle in radians.
    pub plane_theta_1: f64,
    /// Second independent plane angle in radians.
    pub plane_theta_2: f64,
    /// Desired centre minus accepted reference in current-zoom pixels.
    pub centre_from_reference_px: [f64; 2],
}

/// Arrival-rate reference, iteration, palette, and plane state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct MainState {
    /// Latest installed orbit generation, or zero when absent.
    pub generation_applied: u32,
    /// Authoritative-centre revision.
    pub centre_revision: u32,
    /// Requested iteration cap.
    pub requested_iter_cap: u32,
    /// Delivered kernel-level iteration cap.
    pub delivered_iter_cap: u32,
    /// Delivered reference precision in bits.
    pub precision_bits: u32,
    /// Number of delivered reference records.
    pub orbit_length: u32,
    /// Present-owned palette discriminant.
    pub palette_id: u32,
    /// App registry identifier, or zero when absent.
    pub orbit_id: u32,
    /// Non-authoritative display mirror in fractal-axis order.
    pub centre_f64: [f64; 4],
    /// Zero-based first seed axis.
    pub plane_axis_a: u32,
    /// Zero-based second seed axis.
    pub plane_axis_b: u32,
    /// Defining plane origin, including Julia's constant.
    pub plane_origin_f64: [f64; 4],
    /// New reference minus old reference in current-zoom pixels.
    pub reference_shift_px: [f64; 2],
}

/// One coherent owner publication.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub struct ViewerState {
    /// Shared publication epoch incremented by either drain.
    pub epoch: u64,
    /// Latest staged HOT state.
    pub hot: HotState,
    /// Latest staged MAIN state.
    pub main: MainState,
}

/// Full viewer snapshot returned by a HOT drain.
pub type HotDrain = ViewerState;
/// Full viewer snapshot returned by a MAIN drain.
pub type MainDrain = ViewerState;

/// Compact app-registry orbit identity.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OrbitHandle {
    /// Nonzero registry identifier.
    pub id: u32,
    /// Orbit generation stored under that identifier.
    pub generation: u32,
}

/// Result of generation-validating an orbit arrival.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrbitDisposition {
    /// Latest matching generation was staged for MAIN publication.
    Applied,
    /// Delayed or mismatched generation was not published.
    Stale,
}

/// Same-thread owner built only from `Cell` over `Copy` records.
#[derive(Debug)]
pub struct ViewerOwner {
    published: Cell<ViewerState>,
    staged_hot: Cell<HotState>,
    staged_main: Cell<MainState>,
    latest_requested_generation: Cell<u32>,
    epoch_exhausted: Cell<bool>,
}

impl ViewerOwner {
    /// Creates an epoch-zero owner from the supplied initial records.
    #[must_use]
    pub const fn new(initial: ViewerState) -> Self {
        let initial = ViewerState {
            epoch: 0,
            ..initial
        };
        Self {
            published: Cell::new(initial),
            staged_hot: Cell::new(initial.hot),
            staged_main: Cell::new(initial.main),
            latest_requested_generation: Cell::new(0),
            epoch_exhausted: Cell::new(false),
        }
    }

    /// Replaces any undrained refresh-rate state without allocation.
    pub fn stage_hot(&self, hot: HotState) {
        self.staged_hot.set(hot);
    }

    /// Replaces any undrained arrival-rate state without allocation.
    pub fn stage_main(&self, main: MainState) {
        self.staged_main.set(main);
    }

    /// Records the generation most recently accepted for submission.
    pub fn note_requested_generation(&self, generation: u32) {
        self.latest_requested_generation.set(generation);
    }

    /// Stages latest-generation orbit fields and accepted-reference motion.
    #[must_use]
    pub fn accept_orbit(
        &self,
        response: &OrbitResponseView,
        handle: OrbitHandle,
        reference_shift_px: [f64; 2],
    ) -> OrbitDisposition {
        let generation = response.generation();
        if generation != self.latest_requested_generation.get()
            || handle.id == 0
            || handle.generation != generation
        {
            return OrbitDisposition::Stale;
        }
        let mut main = self.staged_main.get();
        main.generation_applied = generation;
        main.centre_revision = response.centre_revision();
        main.precision_bits = response.precision_bits();
        main.orbit_length = response.length();
        main.orbit_id = handle.id;
        main.reference_shift_px = reference_shift_px;
        self.staged_main.set(main);

        let mut hot = self.staged_hot.get();
        hot.centre_from_reference_px[0] -= reference_shift_px[0];
        hot.centre_from_reference_px[1] -= reference_shift_px[1];
        self.staged_hot.set(hot);
        OrbitDisposition::Applied
    }

    /// Publishes a coherent HOT snapshot and advances the shared epoch.
    pub fn drain_hot(&self) -> HotDrain {
        self.drain()
    }

    /// Publishes a coherent MAIN snapshot and advances the shared epoch.
    pub fn drain_main(&self) -> MainDrain {
        self.drain()
    }

    /// Returns the latest publication without advancing its epoch.
    #[must_use]
    pub const fn snapshot(&self) -> ViewerState {
        self.published.get()
    }

    /// Reports whether checked epoch advancement has frozen publication.
    #[must_use]
    pub const fn epoch_exhausted(&self) -> bool {
        self.epoch_exhausted.get()
    }

    fn drain(&self) -> ViewerState {
        let published = self.published.get();
        let Some(epoch) = published.epoch.checked_add(1) else {
            self.epoch_exhausted.set(true);
            return published;
        };
        let next = ViewerState {
            epoch,
            hot: self.staged_hot.get(),
            main: self.staged_main.get(),
        };
        self.published.set(next);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::{HotState, MainState, ViewerOwner, ViewerState};

    #[test]
    fn exact_owner_layouts_are_pinned() {
        assert_eq!(size_of::<HotState>(), 40);
        assert_eq!(align_of::<HotState>(), 8);
        assert_eq!(std::mem::offset_of!(HotState, centre_from_reference_px), 24);
        assert_eq!(size_of::<MainState>(), 120);
        assert_eq!(align_of::<MainState>(), 8);
        assert_eq!(std::mem::offset_of!(MainState, centre_f64), 32);
        assert_eq!(std::mem::offset_of!(MainState, plane_axis_a), 64);
        assert_eq!(std::mem::offset_of!(MainState, plane_origin_f64), 72);
        assert_eq!(std::mem::offset_of!(MainState, reference_shift_px), 104);
        assert_eq!(size_of::<ViewerState>(), 168);
        assert_eq!(align_of::<ViewerState>(), 8);
        assert_eq!(std::mem::offset_of!(ViewerState, hot), 8);
        assert_eq!(std::mem::offset_of!(ViewerState, main), 48);
    }

    #[test]
    fn every_hot_main_interleaving_is_coherent_and_latest_wins() {
        #[derive(Clone, Copy)]
        enum Edit {
            Hot(f64),
            Main(u32),
            DrainHot,
            DrainMain,
        }
        let schedules = [
            [Edit::Hot(1.0), Edit::Main(1), Edit::DrainHot, Edit::DrainMain],
            [Edit::Main(1), Edit::Hot(1.0), Edit::DrainMain, Edit::DrainHot],
            [Edit::Hot(1.0), Edit::DrainHot, Edit::Main(1), Edit::DrainMain],
            [Edit::Main(1), Edit::DrainMain, Edit::Hot(1.0), Edit::DrainHot],
        ];
        for schedule in schedules {
            let owner = ViewerOwner::new(ViewerState::default());
            let mut last_epoch = 0;
            for edit in schedule {
                match edit {
                    Edit::Hot(zoom_log2) => owner.stage_hot(HotState {
                        zoom_log2,
                        ..HotState::default()
                    }),
                    Edit::Main(palette_id) => owner.stage_main(MainState {
                        palette_id,
                        ..MainState::default()
                    }),
                    Edit::DrainHot => {
                        let drained = owner.drain_hot();
                        assert!(drained.epoch > last_epoch);
                        last_epoch = drained.epoch;
                    }
                    Edit::DrainMain => {
                        let drained = owner.drain_main();
                        assert!(drained.epoch > last_epoch);
                        last_epoch = drained.epoch;
                    }
                }
            }
            let final_state = owner.drain_hot();
            assert_eq!(final_state.hot.zoom_log2, 1.0);
            assert_eq!(final_state.main.palette_id, 1);
        }
    }

    #[test]
    fn multiple_undrained_edits_publish_only_the_latest() {
        let owner = ViewerOwner::new(ViewerState::default());
        for zoom_log2 in [1.0, 2.0, 3.0] {
            owner.stage_hot(HotState {
                zoom_log2,
                ..HotState::default()
            });
        }
        let drained = owner.drain_hot();
        assert_eq!(drained.hot.zoom_log2, 3.0);
        assert_eq!(drained.epoch, 1);
        assert_eq!(owner.snapshot(), drained);
    }
}
