//! Copy-cell viewer owner and independently staged HOT and MAIN drains.

use std::{cell::Cell, fmt};

use ember_julibrot_math::{
    BigCentre, MathError, NavigationDelta, Plane, mirror_centre, pixel_scale,
};

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

/// Typed refusal from authoritative owner navigation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OwnerError {
    /// No bignum centre, reference, plane, and grid width were configured.
    NavigationUnconfigured,
    /// The monotonic orbit generation could not advance.
    GenerationExhausted,
    /// The authoritative-centre revision could not advance.
    CentreRevisionExhausted,
    /// Math refused an input or bignum operation.
    Math(MathError),
}

impl From<MathError> for OwnerError {
    fn from(error: MathError) -> Self {
        Self::Math(error)
    }
}

impl fmt::Display for OwnerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NavigationUnconfigured => formatter.write_str("owner navigation is unconfigured"),
            Self::GenerationExhausted => formatter.write_str("owner generation is exhausted"),
            Self::CentreRevisionExhausted => {
                formatter.write_str("owner centre revision is exhausted")
            }
            Self::Math(error) => write!(formatter, "owner navigation math: {error}"),
        }
    }
}

impl std::error::Error for OwnerError {}

/// Authoritative values required before the owner can apply navigation.
#[derive(Clone, Debug, PartialEq)]
pub struct NavigationConfig {
    /// Desired bignum centre.
    pub centre: BigCentre,
    /// Centre of the accepted reference orbit.
    pub reference_centre: BigCentre,
    /// Current math-produced plane basis.
    pub plane: Plane,
    /// Current canvas width in physical pixels.
    pub grid_width: u32,
}

/// One latest-wins reference submission released by the owner.
#[derive(Clone, Debug, PartialEq)]
pub struct NavigationSubmission {
    /// Monotonic generation represented by this snapshot.
    pub generation: u32,
    /// Authoritative-centre revision represented by this snapshot.
    pub centre_revision: u32,
    /// Desired bignum centre after every coalesced edit.
    pub centre: BigCentre,
    /// Desired base-two zoom exponent.
    pub zoom_log2: f64,
}

#[derive(Debug)]
struct NavigationState {
    centre: BigCentre,
    reference_centre: BigCentre,
    plane: Plane,
    grid_width: u32,
    pending_generation: Option<u32>,
    in_flight_generation: Option<u32>,
}

/// Same-thread owner built only from `Cell` over `Copy` records.
#[derive(Debug)]
pub struct ViewerOwner {
    published: Cell<ViewerState>,
    staged_hot: Cell<HotState>,
    staged_main: Cell<MainState>,
    latest_requested_generation: Cell<u32>,
    epoch_exhausted: Cell<bool>,
    navigation: Option<NavigationState>,
    navigation_error: Option<OwnerError>,
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
            navigation: None,
            navigation_error: None,
        }
    }

    /// Installs the authoritative centre and its math-produced projection context.
    ///
    /// # Errors
    ///
    /// Returns a typed math refusal for mismatched precision, invalid extent, scale, or centre.
    pub fn configure_navigation(&mut self, config: NavigationConfig) -> Result<(), OwnerError> {
        let zoom_log2 = self.staged_hot.get().zoom_log2;
        let scale = pixel_scale(zoom_log2, config.grid_width)?;
        let displacement =
            config
                .centre
                .displacement_px(&config.reference_centre, &config.plane, scale)?;
        let centre_f64 = mirror_centre(&config.centre)?.coords;
        let mut hot = self.staged_hot.get();
        hot.centre_from_reference_px = displacement;
        self.staged_hot.set(hot);
        let mut main = self.staged_main.get();
        main.centre_f64 = centre_f64;
        self.staged_main.set(main);
        self.navigation = Some(NavigationState {
            centre: config.centre,
            reference_centre: config.reference_centre,
            plane: config.plane,
            grid_width: config.grid_width,
            pending_generation: None,
            in_flight_generation: None,
        });
        self.navigation_error = None;
        Ok(())
    }

    /// Applies one exact navigation edit and returns its new generation.
    ///
    /// A refusal leaves generation and owner records unchanged and is available through
    /// [`Self::take_navigation_error`]. Callers that propagate errors directly can use
    /// [`Self::navigate_checked`].
    pub fn navigate(&mut self, delta: NavigationDelta) -> u32 {
        match self.navigate_checked(delta) {
            Ok(generation) => generation,
            Err(error) => {
                self.navigation_error = Some(error);
                self.latest_requested_generation.get()
            }
        }
    }

    /// Applies one exact navigation edit with an immediate typed refusal path.
    ///
    /// # Errors
    ///
    /// Returns a typed owner refusal for missing configuration, exhausted counters, or any math
    /// error. No owner state is changed on refusal.
    pub fn navigate_checked(&mut self, delta: NavigationDelta) -> Result<u32, OwnerError> {
        let generation = self
            .latest_requested_generation
            .get()
            .checked_add(1)
            .ok_or(OwnerError::GenerationExhausted)?;
        let mut main = self.staged_main.get();
        let centre_revision = main
            .centre_revision
            .checked_add(1)
            .ok_or(OwnerError::CentreRevisionExhausted)?;
        let mut hot = self.staged_hot.get();
        let zoom_log2_after = hot.zoom_log2 + delta.zoom_delta_log2;
        let navigation = self
            .navigation
            .as_mut()
            .ok_or(OwnerError::NavigationUnconfigured)?;
        let mut centre = navigation.centre.clone();
        centre.apply_navigation(
            &delta,
            &navigation.plane,
            hot.zoom_log2,
            zoom_log2_after,
            navigation.grid_width,
        )?;
        let scale = pixel_scale(zoom_log2_after, navigation.grid_width)?;
        let displacement =
            centre.displacement_px(&navigation.reference_centre, &navigation.plane, scale)?;
        let centre_f64 = mirror_centre(&centre)?.coords;

        navigation.centre = centre;
        navigation.pending_generation = Some(generation);
        hot.zoom_log2 = zoom_log2_after;
        hot.centre_from_reference_px = displacement;
        main.centre_revision = centre_revision;
        main.centre_f64 = centre_f64;
        self.staged_hot.set(hot);
        self.staged_main.set(main);
        self.latest_requested_generation.set(generation);
        self.navigation_error = None;
        Ok(generation)
    }

    /// Releases the newest pending centre only when no earlier submission is in flight.
    pub fn take_navigation_submission(&mut self) -> Option<NavigationSubmission> {
        let navigation = self.navigation.as_mut()?;
        if navigation.in_flight_generation.is_some() {
            return None;
        }
        let generation = navigation.pending_generation.take()?;
        navigation.in_flight_generation = Some(generation);
        Some(NavigationSubmission {
            generation,
            centre_revision: self.staged_main.get().centre_revision,
            centre: navigation.centre.clone(),
            zoom_log2: self.staged_hot.get().zoom_log2,
        })
    }

    /// Marks the named submission complete so the one coalesced successor can be released.
    #[must_use]
    pub fn finish_navigation_submission(&mut self, generation: u32) -> bool {
        let Some(navigation) = self.navigation.as_mut() else {
            return false;
        };
        if navigation.in_flight_generation != Some(generation) {
            return false;
        }
        navigation.in_flight_generation = None;
        true
    }

    /// Reports whether one coalesced navigation submission is waiting.
    #[must_use]
    pub fn navigation_pending_depth(&self) -> u32 {
        self.navigation.as_ref().map_or(0, |navigation| {
            u32::from(navigation.pending_generation.is_some())
        })
    }

    /// Returns and clears the latest typed navigation refusal.
    #[must_use]
    pub const fn take_navigation_error(&mut self) -> Option<OwnerError> {
        self.navigation_error.take()
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
        &mut self,
        response: &OrbitResponseView,
        handle: OrbitHandle,
        reference_shift_px: [f64; 2],
    ) -> OrbitDisposition {
        let generation = response.generation();
        if let Some(navigation) = self.navigation.as_mut()
            && navigation.in_flight_generation == Some(generation)
        {
            navigation.in_flight_generation = None;
        }
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
        if let Some(navigation) = self.navigation.as_mut() {
            navigation.reference_centre = navigation.centre.clone();
            hot.centre_from_reference_px = [0.0; 2];
        } else {
            hot.centre_from_reference_px[0] -= reference_shift_px[0];
            hot.centre_from_reference_px[1] -= reference_shift_px[1];
        }
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
    use ember_julibrot_math::{BigCentre, NavigationDelta, PlaneAngles, construct_plane};

    use super::{
        HotState, MainState, NavigationConfig, OrbitDisposition, OrbitHandle, OwnerError,
        ViewerOwner, ViewerState,
    };
    use crate::{
        CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest, ReferenceOrbitRecord,
        WorkerChannel, WorkerConfig, WorkerMode,
    };

    #[derive(Clone, Copy)]
    enum Edit {
        Hot(f64),
        Main(u32),
        DrainHot,
        DrainMain,
    }

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
        let edits = [
            Edit::Hot(1.0),
            Edit::Main(1),
            Edit::DrainHot,
            Edit::DrainMain,
        ];
        for (a, first) in edits.iter().copied().enumerate() {
            for (b, second) in edits.iter().copied().enumerate() {
                for (c, third) in edits.iter().copied().enumerate() {
                    for (d, fourth) in edits.iter().copied().enumerate() {
                        if a == b || a == c || a == d || b == c || b == d || c == d {
                            continue;
                        }
                        assert_schedule([first, second, third, fourth]);
                    }
                }
            }
        }
    }

    fn assert_schedule(schedule: [Edit; 4]) {
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

    fn navigation_owner() -> Result<ViewerOwner, OwnerError> {
        let plane = construct_plane(PlaneAngles {
            theta_1: 0.21,
            theta_2: -0.13,
        })?;
        let centre = BigCentre::from_f64([0.0; 4], 384)?;
        let mut owner = ViewerOwner::new(ViewerState {
            main: MainState {
                requested_iter_cap: 64,
                ..MainState::default()
            },
            ..ViewerState::default()
        });
        owner.configure_navigation(NavigationConfig {
            reference_centre: centre.clone(),
            centre,
            plane,
            grid_width: 1_024,
        })?;
        Ok(owner)
    }

    #[test]
    fn ten_thousand_mixed_navigation_edits_stay_finite_and_monotone() -> Result<(), OwnerError> {
        let mut owner = navigation_owner()?;
        let mut previous_generation = 0;
        for index in 0..10_000 {
            let direction = if index % 2 == 0 { 1.0 } else { -1.0 };
            let zoom_delta_log2 = if index % 3 == 0 { 0.002 } else { -0.001 };
            let generation = owner.navigate(NavigationDelta {
                pan_canvas_px: [direction * 0.25, -direction * 0.125],
                zoom_delta_log2,
                anchor_canvas_px: [23.5, -11.25],
            });
            assert!(generation > previous_generation);
            previous_generation = generation;
            assert!(
                owner
                    .drain_hot()
                    .hot
                    .centre_from_reference_px
                    .iter()
                    .all(|component| component.is_finite())
            );
        }
        assert_eq!(previous_generation, 10_000);
        assert_eq!(owner.take_navigation_error(), None);
        Ok(())
    }

    #[test]
    fn three_edits_while_one_request_is_in_flight_collapse_to_one_submission()
    -> Result<(), OwnerError> {
        let mut owner = navigation_owner()?;
        let delta = NavigationDelta {
            pan_canvas_px: [1.0, -0.5],
            zoom_delta_log2: 0.001,
            anchor_canvas_px: [7.0, 3.0],
        };
        let first_generation = owner.navigate(delta);
        let Some(first) = owner.take_navigation_submission() else {
            return Err(OwnerError::NavigationUnconfigured);
        };
        assert_eq!(first.generation, first_generation);

        assert_eq!(owner.navigate(delta), 2);
        assert_eq!(owner.navigate(delta), 3);
        assert_eq!(owner.navigate(delta), 4);
        assert_eq!(owner.navigation_pending_depth(), 1);
        assert_eq!(owner.take_navigation_submission(), None);
        assert!(owner.finish_navigation_submission(first_generation));

        let Some(coalesced) = owner.take_navigation_submission() else {
            return Err(OwnerError::NavigationUnconfigured);
        };
        assert_eq!(coalesced.generation, 4);
        assert_eq!(coalesced.centre_revision, 4);
        assert_eq!(owner.navigation_pending_depth(), 0);
        assert_eq!(owner.take_navigation_submission(), None);
        Ok(())
    }

    #[test]
    fn acceptance_checks_both_generations_and_rebases_hot_displacement() {
        let (endpoint, producer) =
            WorkerChannel::new(WorkerConfig { max_iter: 64 }, WorkerMode::SameThread).unwrap();
        let request = OrbitRequest::new(
            7,
            EncodedCentre {
                revision: 19,
                coordinates: [CoordinateDescriptor::default(); 4],
                limbs: Vec::new(),
            },
            0,
            64,
            64,
            OrbitReason::INITIAL,
        )
        .unwrap();
        assert_eq!(endpoint.submit(request), crate::SubmitOutcome::Transferred);
        let request = producer.next_request().unwrap().unwrap();
        producer
            .complete(
                request,
                &[ReferenceOrbitRecord {
                    re_hi: 0.0,
                    im_hi: 0.0,
                    re_lo: 0.0,
                    im_lo: 0.0,
                }],
                128,
                40,
                250_000,
            )
            .unwrap();
        let mut response = endpoint.next_arrival().unwrap();
        let mut owner = ViewerOwner::new(ViewerState {
            hot: HotState {
                centre_from_reference_px: [10.0, -3.0],
                ..HotState::default()
            },
            ..ViewerState::default()
        });
        owner.note_requested_generation(7);
        assert_eq!(
            owner.accept_orbit(
                &response,
                OrbitHandle {
                    id: 3,
                    generation: 7,
                },
                [4.0, -1.0],
            ),
            OrbitDisposition::Applied
        );
        let published = owner.drain_main();
        assert_eq!(published.main.generation_applied, 7);
        assert_eq!(published.main.centre_revision, 19);
        assert_eq!(published.main.precision_bits, 128);
        assert_eq!(published.main.orbit_length, 1);
        assert_eq!(published.main.reference_shift_px, [4.0, -1.0]);
        assert_eq!(published.hot.centre_from_reference_px, [6.0, -2.0]);
        assert_eq!(
            owner.accept_orbit(
                &response,
                OrbitHandle {
                    id: 3,
                    generation: 6,
                },
                [0.0, 0.0],
            ),
            OrbitDisposition::Stale
        );
        response
            .records
            .return_credit(OrbitDisposition::Applied, 0)
            .unwrap();
    }
}
