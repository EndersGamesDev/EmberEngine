//! Requested controls and worker-owned HOT/MAIN publication integration.

use ember_julibrot_math::{
    Axis4, BigCentre, NavigationDelta, Plane, PlaneAngles, Pose, SEED_AXES, ViewControls,
    construct_plane,
};
use ember_julibrot_present::PaletteId;
use ember_julibrot_worker::{
    HotState, MIN_MAX_ITER, MainState, NavigationConfig, NavigationSubmission, OrbitReason,
    ViewerOwner, ViewerState,
};

use crate::AppError;

/// Initial requested iteration cap; it is a policy, not a delivered fact.
pub const INITIAL_ITERATION_CAP: u32 = 512;

const NAVIGATION_PRECISION_BITS: u32 = 1_024;

/// The `scale` control's ends, in base-two zoom exponent.
///
/// The lower end is a step out from the whole chart rather than zero, so the picture can be
/// pulled back off the edges; the upper end is far past where binary64 gives out, which is the
/// point of holding the centre in bignum.
pub const SCALE_RANGE_LOG2: [f64; 2] = [-2.0, 120.0];

/// A drag whose shorter side is under this many CSS pixels is a click, not a box.
///
/// Without a floor there is no click at all: a pointer that moves one pixel between press and
/// release would zoom by three orders of magnitude, which is the opposite of what the hand meant.
pub const BOX_CLICK_THRESHOLD_PX: f64 = 4.0;

/// Controls retain requested values independently of delayed worker or GPU work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RequestedControls {
    /// Absolute plane origin `(z.re, z.im, c.re, c.im)`.
    pub plane_origin: [f64; 4],
    /// Desired base-two zoom exponent.
    pub zoom_log2: f64,
    /// Independent plane angles in radians.
    pub plane_angles: PlaneAngles,
    /// Requested orbit and kernel iteration cap.
    pub iteration_cap: u32,
    /// Requested present-owned palette.
    pub palette: PaletteId,
    /// Every VIEW control.
    pub view: ViewControls,
}

impl Default for RequestedControls {
    fn default() -> Self {
        Self {
            plane_origin: [0.0; 4],
            zoom_log2: 0.0,
            plane_angles: PlaneAngles {
                theta_1: 0.0,
                theta_2: 0.0,
            },
            iteration_cap: INITIAL_ITERATION_CAP,
            palette: PaletteId::Classic,
            view: ViewControls::NEUTRAL,
        }
    }
}

/// One preset: a named row of control values and nothing else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetRow {
    /// Stable name shown by the page.
    pub name: &'static str,
    /// Both plane angles in radians.
    pub plane_angles: [f64; 2],
    /// Absolute plane origin.
    pub plane_origin: [f64; 4],
    /// Every VIEW control.
    pub view: ViewControls,
}

const QUARTER_TURN: f64 = core::f64::consts::FRAC_PI_2;

/// The relief rows' observer, which is the orientation the retired fixed mount had.
const RELIEF_VIEW: ViewControls = ViewControls {
    theta_1: 0.6,
    theta_2: 0.97,
    camera_yaw: 0.349,
    camera_pitch: 0.262,
    height_scale: 1.0,
    distance_five: 8.0,
    distance_four: 8.0,
};

/// The Julia row's constant.
const JULIA_C0: [f64; 2] = [-0.8, 0.156];

/// Every preset, defined once as pure data.
pub const PRESET_ROWS: [PresetRow; 4] = [
    PresetRow {
        name: "Mandelbrot",
        plane_angles: [0.0, 0.0],
        plane_origin: [0.0; 4],
        view: ViewControls::NEUTRAL,
    },
    PresetRow {
        name: "Julia",
        plane_angles: [-QUARTER_TURN, -QUARTER_TURN],
        plane_origin: [0.0, 0.0, JULIA_C0[0], JULIA_C0[1]],
        view: ViewControls::NEUTRAL,
    },
    PresetRow {
        name: "Mandelbrot relief",
        plane_angles: [0.0, 0.0],
        plane_origin: [0.0; 4],
        view: RELIEF_VIEW,
    },
    PresetRow {
        name: "Julia relief",
        plane_angles: [-QUARTER_TURN, -QUARTER_TURN],
        plane_origin: [0.0, 0.0, JULIA_C0[0], JULIA_C0[1]],
        view: RELIEF_VIEW,
    },
];

/// Returns one preset row by identifier.
#[must_use]
pub fn preset_row(id: u32) -> Option<PresetRow> {
    PRESET_ROWS.get(id as usize).copied()
}

/// Returns the per-axis factor that converts one CSS pixel of the canvas box into render-grid
/// pixels.
///
/// The canvas backing store is the render grid named by its `width`/`height` attributes, while CSS
/// lays the element out at an unrelated size; the device pixel ratio is not a term in either, so it
/// never appears here.
///
/// # Errors
///
/// Returns a math failure for a non-finite or non-positive client rectangle or a zero grid extent.
fn css_to_grid_scale(rect_css: [f64; 2], grid: [u32; 2]) -> Result<[f64; 2], AppError> {
    if !rect_css
        .iter()
        .all(|extent| extent.is_finite() && *extent > 0.0)
    {
        return Err(AppError::Math(
            "canvas client rectangle is not a positive finite size".to_string(),
        ));
    }
    if grid.contains(&0) {
        return Err(AppError::Math("render grid extent is zero".to_string()));
    }
    Ok([
        f64::from(grid[0]) / rect_css[0],
        f64::from(grid[1]) / rect_css[1],
    ])
}

/// Converts a canvas-relative DOM pointer position in CSS pixels into the canvas-centred
/// render-grid pixels with positive y upward that `NavigationDelta` requires.
///
/// # Errors
///
/// Returns a math failure for non-finite input, a degenerate client rectangle, or a zero grid.
pub fn anchor_px_up(
    pointer_css: [f64; 2],
    rect_css: [f64; 2],
    grid: [u32; 2],
) -> Result<[f64; 2], AppError> {
    if !pointer_css.iter().all(|value| value.is_finite()) {
        return Err(AppError::Math("pointer position is not finite".to_string()));
    }
    let scale = css_to_grid_scale(rect_css, grid)?;
    Ok([
        (pointer_css[0] - rect_css[0] / 2.0) * scale[0],
        (rect_css[1] / 2.0 - pointer_css[1]) * scale[1],
    ])
}

/// Converts a DOM drag displacement in CSS pixels into render-grid pixels, keeping DOM-down y for
/// the control boundary that flips it.
///
/// # Errors
///
/// Returns a math failure for non-finite input, a degenerate client rectangle, or a zero grid.
pub fn drag_delta_px_down(
    delta_css: [f64; 2],
    rect_css: [f64; 2],
    grid: [u32; 2],
) -> Result<[f64; 2], AppError> {
    if !delta_css.iter().all(|value| value.is_finite()) {
        return Err(AppError::Math(
            "drag displacement is not finite".to_string(),
        ));
    }
    let scale = css_to_grid_scale(rect_css, grid)?;
    Ok([delta_css[0] * scale[0], delta_css[1] * scale[1]])
}

/// Reports whether a drag rectangle is a box selection rather than a click.
#[must_use]
pub fn is_box_selection(box_css: [f64; 2]) -> bool {
    box_css
        .iter()
        .all(|side| side.is_finite() && *side >= BOX_CLICK_THRESHOLD_PX)
}

/// Returns the zoom change that makes a screen box fill the screen without spilling past it.
///
/// The factor is the smaller of the two ratios, so the box fits inside the viewport on its
/// limiting side and the aspect ratio of the picture is untouched; taking the larger one would
/// crop the box the user just drew.
///
/// # Errors
///
/// Returns a math failure for a non-finite or non-positive box or client rectangle.
pub fn box_zoom_delta_log2(box_css: [f64; 2], rect_css: [f64; 2]) -> Result<f64, AppError> {
    if !box_css
        .iter()
        .chain(rect_css.iter())
        .all(|side| side.is_finite() && *side > 0.0)
    {
        return Err(AppError::Math(
            "selection box is not a positive finite size".to_string(),
        ));
    }
    let factor = (rect_css[0] / box_css[0]).min(rect_css[1] / box_css[1]);
    if !factor.is_finite() || factor <= 0.0 {
        return Err(AppError::Math(
            "selection box has no finite zoom factor".to_string(),
        ));
    }
    let delta = factor.log2();
    delta
        .is_finite()
        .then_some(delta)
        .ok_or_else(|| AppError::Math("selection box has no finite zoom factor".to_string()))
}

/// Worker navigation instruction paired with immediately staged visual HOT state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavigationEdit {
    /// Pointer-anchored zoom in canvas-centred pixels with positive y upward.
    Zoom {
        /// Change in base-two zoom exponent.
        delta_log2: f64,
        /// Pointer position relative to canvas centre.
        anchor_px_up: [f64; 2],
    },
    /// Drag displacement in DOM pixels; the worker receives the converted plane displacement.
    Pan {
        /// Desired-centre change in current pixels along `(u,v)`.
        centre_delta_px: [f64; 2],
    },
    /// A target placed at one screen point, optionally carrying the box's zoom change.
    Target {
        /// Target position relative to canvas centre, in render-grid pixels with `+y` up.
        anchor_px_up: [f64; 2],
        /// Change in base-two zoom exponent; zero for a click.
        delta_log2: f64,
    },
}

/// Result of the mandatory HOT drain and math plane construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HotFrame {
    /// Coherent worker publication.
    pub state: ViewerState,
    /// Corrected hybrid-capable plane derived from MAIN seed axes and HOT angles.
    pub plane: Plane,
    /// Math-owned pose consumed by present's warp planner.
    pub pose: Pose,
}

/// One worker-owned centre snapshot paired with the app's accumulated request reason.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceSubmission {
    /// Exact navigation snapshot released by the owner.
    pub navigation: NavigationSubmission,
    /// All reasons coalesced since the preceding released request.
    pub reason: OrbitReason,
}

/// App-facing controller whose storage authority remains the worker-owned records.
#[derive(Debug)]
pub struct ViewerController {
    owner: ViewerOwner,
    requested: RequestedControls,
    staged_hot: HotState,
    staged_main: MainState,
    pending_reason: Option<OrbitReason>,
    grid_width: u32,
}

impl ViewerController {
    /// Creates the canonical Mandelbrot owner and requested control state.
    ///
    /// # Errors
    ///
    /// Returns a math error if the canonical preset contract is unavailable.
    pub fn new(grid_width: u32) -> Result<Self, AppError> {
        let requested = RequestedControls::default();
        let origin = requested.plane_origin;
        let plane = construct_plane(requested.plane_angles).map_err(math_error)?;
        let centre = BigCentre::from_f64(origin, NAVIGATION_PRECISION_BITS).map_err(math_error)?;
        let initial = ViewerState {
            epoch: 0,
            hot: HotState {
                zoom_log2: requested.zoom_log2,
                plane_theta_1: requested.plane_angles.theta_1,
                plane_theta_2: requested.plane_angles.theta_2,
                centre_from_reference_px: [0.0; 2],
            },
            main: MainState {
                requested_iter_cap: requested.iteration_cap,
                palette_id: requested.palette as u32,
                centre_f64: origin,
                plane_axis_a: SEED_AXES[0] as u32,
                plane_axis_b: SEED_AXES[1] as u32,
                plane_origin_f64: origin,
                ..MainState::default()
            },
        };
        let mut owner = ViewerOwner::new(initial);
        owner
            .configure_navigation(NavigationConfig {
                centre: centre.clone(),
                reference_centre: centre,
                plane,
                grid_width,
            })
            .map_err(owner_error)?;
        let generation = owner.navigate(NavigationDelta::default());
        if let Some(error) = owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        debug_assert_eq!(generation, 1);
        Ok(Self {
            owner,
            requested,
            staged_hot: initial.hot,
            staged_main: initial.main,
            pending_reason: Some(OrbitReason::INITIAL),
            grid_width,
        })
    }

    /// Returns requested controls without consulting delayed delivered state.
    #[must_use]
    pub const fn requested(&self) -> RequestedControls {
        self.requested
    }

    /// Returns the worker owner for response acceptance and generation notes.
    #[must_use]
    pub const fn owner(&self) -> &ViewerOwner {
        &self.owner
    }

    /// Returns the worker owner for response acceptance and navigation release.
    #[must_use]
    pub const fn owner_mut(&mut self) -> &mut ViewerOwner {
        &mut self.owner
    }

    /// Stages a pointer-anchored zoom immediately and returns the edit for bignum navigation.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result.
    pub fn wheel_zoom(
        &mut self,
        delta_log2: f64,
        anchor_px_up: [f64; 2],
    ) -> Result<NavigationEdit, AppError> {
        if !delta_log2.is_finite() || !anchor_px_up.iter().all(|component| component.is_finite()) {
            return Err(AppError::Math("wheel input is not finite".to_string()));
        }
        let zoom_log2 = self.requested.zoom_log2 + delta_log2;
        if !zoom_log2.is_finite() {
            return Err(AppError::Math(
                "wheel zoom exceeded finite range".to_string(),
            ));
        }
        self.owner.navigate(NavigationDelta {
            pan_canvas_px: [0.0; 2],
            zoom_delta_log2: delta_log2,
            anchor_canvas_px: anchor_px_up,
        });
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.requested.zoom_log2 = zoom_log2;
        self.add_reason(OrbitReason::ZOOM_THRESHOLD.union(OrbitReason::CENTRE_THRESHOLD));
        Ok(NavigationEdit::Zoom {
            delta_log2,
            anchor_px_up,
        })
    }

    /// Converts DOM-down drag input to plane-up centre motion and stages it immediately.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result.
    pub fn drag_pan(&mut self, delta_dom: [f64; 2]) -> Result<NavigationEdit, AppError> {
        if !delta_dom.iter().all(|component| component.is_finite()) {
            return Err(AppError::Math("drag input is not finite".to_string()));
        }
        let pan_canvas_px = [delta_dom[0], -delta_dom[1]];
        self.owner.navigate(NavigationDelta {
            pan_canvas_px,
            ..NavigationDelta::default()
        });
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        let centre_delta_px = [-pan_canvas_px[0], -pan_canvas_px[1]];
        Ok(NavigationEdit::Pan { centre_delta_px })
    }

    /// Places the target at one screen point, carrying an optional zoom change with it.
    ///
    /// The plane point under the target becomes the centre, so the marker comes to rest at the
    /// screen centre. The edit is the existing anchored-zoom path with the pan set to minus the
    /// anchor: the anchored term contributes `(s_before - s_after)A` and the pan term `+s_after·A`,
    /// which sum to `s_before·A` — the plane offset the point had at the scale the user was
    /// looking at, independent of how much the zoom then changes. A click passes zero, a box
    /// release passes the factor that makes it fill the screen, and both are one navigation edit.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result, or a typed owner refusal.
    pub fn set_target(
        &mut self,
        anchor_px_up: [f64; 2],
        delta_log2: f64,
    ) -> Result<NavigationEdit, AppError> {
        if !delta_log2.is_finite() || !anchor_px_up.iter().all(|component| component.is_finite()) {
            return Err(AppError::Math("target input is not finite".to_string()));
        }
        let zoom_log2 = self.requested.zoom_log2 + delta_log2;
        if !zoom_log2.is_finite() {
            return Err(AppError::Math(
                "target zoom exceeded finite range".to_string(),
            ));
        }
        self.owner.navigate(NavigationDelta {
            pan_canvas_px: [-anchor_px_up[0], -anchor_px_up[1]],
            zoom_delta_log2: delta_log2,
            anchor_canvas_px: anchor_px_up,
        });
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.requested.zoom_log2 = zoom_log2;
        self.add_reason(OrbitReason::ZOOM_THRESHOLD.union(OrbitReason::CENTRE_THRESHOLD));
        Ok(NavigationEdit::Target {
            anchor_px_up,
            delta_log2,
        })
    }

    /// Moves the `scale` control to an absolute zoom exponent, about the screen centre.
    ///
    /// The worker's centre update needs the scale before and the scale after, so an absolute
    /// slider reaches it as the difference; the accumulated sum equals the slider's own number to
    /// within one unit in its last place, which no readout in the lab resolves.
    ///
    /// # Errors
    ///
    /// Returns a math failure outside the control's own ends, or a typed owner refusal.
    pub fn set_zoom_log2(&mut self, zoom_log2: f64) -> Result<NavigationEdit, AppError> {
        if !zoom_log2.is_finite()
            || zoom_log2 < SCALE_RANGE_LOG2[0]
            || zoom_log2 > SCALE_RANGE_LOG2[1]
        {
            return Err(AppError::Math(format!(
                "scale {zoom_log2} is outside the control range"
            )));
        }
        self.wheel_zoom(zoom_log2 - self.requested.zoom_log2, [0.0, 0.0])
    }

    /// Stages two independent plane angles without resetting other HOT controls.
    ///
    /// # Errors
    ///
    /// Returns a math failure when either angle is non-finite.
    pub fn set_plane_angles(&mut self, angles: PlaneAngles) -> Result<(), AppError> {
        if !angles.theta_1.is_finite() || !angles.theta_2.is_finite() {
            return Err(AppError::Math("plane angles are not finite".to_string()));
        }
        self.synchronize_shadow()?;
        self.requested.plane_angles = angles;
        let mut hot = self.staged_hot;
        hot.plane_theta_1 = angles.theta_1;
        hot.plane_theta_2 = angles.theta_2;
        self.staged_hot = hot;
        self.owner.stage_hot(hot);
        self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        Ok(())
    }

    /// Moves the absolute plane origin, resetting the centre to it and preserving cap, palette,
    /// and every VIEW control.
    ///
    /// This is MAIN work: a new origin selects different samples and needs a new reference orbit,
    /// which is exactly the publication the retired preset selection performed.
    ///
    /// # Errors
    ///
    /// Returns a typed math or centre-revision overflow failure.
    pub fn set_plane_origin(&mut self, origin: [f64; 4]) -> Result<(), AppError> {
        if !origin.iter().all(|value| value.is_finite()) {
            return Err(AppError::Math("plane origin is not finite".to_string()));
        }
        self.synchronize_shadow()?;
        self.requested.plane_origin = origin;
        self.requested.zoom_log2 = 0.0;
        let angles = self.requested.plane_angles;
        let hot = HotState {
            zoom_log2: 0.0,
            plane_theta_1: angles.theta_1,
            plane_theta_2: angles.theta_2,
            centre_from_reference_px: [0.0; 2],
        };
        let main = MainState {
            generation_applied: 0,
            centre_revision: self.staged_main.centre_revision,
            centre_f64: origin,
            plane_axis_a: SEED_AXES[0] as u32,
            plane_axis_b: SEED_AXES[1] as u32,
            plane_origin_f64: origin,
            orbit_length: 0,
            orbit_id: 0,
            precision_bits: 0,
            reference_shift_px: [0.0; 2],
            ..self.staged_main
        };
        self.staged_hot = hot;
        self.staged_main = main;
        let plane = construct_plane(angles).map_err(math_error)?;
        let centre = BigCentre::from_f64(origin, NAVIGATION_PRECISION_BITS).map_err(math_error)?;
        self.owner.stage_hot(hot);
        self.owner.stage_main(main);
        self.owner
            .configure_navigation(NavigationConfig {
                centre: centre.clone(),
                reference_centre: centre,
                plane,
                grid_width: self.grid_width,
            })
            .map_err(owner_error)?;
        self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.pending_reason = Some(OrbitReason::INITIAL);
        Ok(())
    }

    /// Stages an iteration request without changing the last delivered value.
    ///
    /// # Errors
    ///
    /// Returns a worker wall for values below the minimum request cap.
    pub fn set_iteration_cap(&mut self, max_iter: u32) -> Result<(), AppError> {
        if max_iter < MIN_MAX_ITER {
            return Err(AppError::Worker(format!(
                "iteration cap {max_iter} is below minimum {MIN_MAX_ITER}"
            )));
        }
        self.synchronize_shadow()?;
        self.requested.iteration_cap = max_iter;
        let mut main = self.staged_main;
        main.requested_iter_cap = max_iter;
        self.staged_main = main;
        self.owner.stage_main(main);
        self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.add_reason(OrbitReason::MAX_ITER_CHANGE);
        Ok(())
    }

    /// Stages one of present's exact palette identifiers.
    ///
    /// # Errors
    ///
    /// Returns epoch exhaustion when the full worker-owned record can no longer synchronize.
    pub fn set_palette(&mut self, palette: PaletteId) -> Result<(), AppError> {
        self.synchronize_shadow()?;
        self.requested.palette = palette;
        let mut main = self.staged_main;
        main.palette_id = palette as u32;
        self.staged_main = main;
        self.owner.stage_main(main);
        Ok(())
    }

    /// Stages every VIEW control without touching the fractal plane.
    ///
    /// # Errors
    ///
    /// Returns a math failure when a control is non-finite or outside its range.
    pub fn set_view_controls(&mut self, view: ViewControls) -> Result<(), AppError> {
        if !view.is_valid() {
            return Err(AppError::Math(
                "a VIEW control is not finite or is outside its range".to_string(),
            ));
        }
        self.requested.view = view;
        Ok(())
    }

    /// Applies one preset row through the same paths a user's own movement reaches.
    ///
    /// # Errors
    ///
    /// Returns the typed failure of whichever staged control refused.
    pub fn apply_preset(&mut self, row: PresetRow) -> Result<(), AppError> {
        self.set_view_controls(row.view)?;
        self.set_plane_angles(PlaneAngles {
            theta_1: row.plane_angles[0],
            theta_2: row.plane_angles[1],
        })?;
        self.set_plane_origin(row.plane_origin)
    }

    /// Performs the mandatory HOT drain and constructs the current math pose.
    ///
    /// # Errors
    ///
    /// Returns a typed axis, plane, extent, or time failure.
    pub fn drain_hot(&mut self, grid_extent: [u32; 2]) -> Result<HotFrame, AppError> {
        if grid_extent[0] == 0 || grid_extent[1] == 0 || !self.requested.view.is_valid() {
            return Err(AppError::Math(
                "pose extent or VIEW control is invalid".to_string(),
            ));
        }
        let state = self.owner.drain_hot();
        if self.owner.epoch_exhausted() {
            return Err(AppError::EpochExhausted);
        }
        // The worker record still carries the seed-axis words; no control selects axes, so the
        // app pins them and refuses a record that disagrees rather than drawing a different plane.
        if axis(state.main.plane_axis_a)? != SEED_AXES[0]
            || axis(state.main.plane_axis_b)? != SEED_AXES[1]
        {
            return Err(AppError::Math(
                "the owner record names seed axes the lab no longer has".to_string(),
            ));
        }
        let plane = construct_plane(PlaneAngles {
            theta_1: state.hot.plane_theta_1,
            theta_2: state.hot.plane_theta_2,
        })
        .map_err(math_error)?;
        let pose = Pose {
            epoch: state.epoch,
            orbit_generation: state.main.generation_applied,
            plane,
            plane_theta_1: state.hot.plane_theta_1,
            plane_theta_2: state.hot.plane_theta_2,
            zoom_log2: state.hot.zoom_log2,
            view: self.requested.view,
            grid_width: grid_extent[0],
            grid_height: grid_extent[1],
            centre_from_reference_px: state.hot.centre_from_reference_px,
        };
        self.staged_hot = state.hot;
        self.staged_main = state.main;
        Ok(HotFrame { state, plane, pose })
    }

    /// Performs an infallible MAIN drain and checks only the impossible epoch wall.
    ///
    /// # Errors
    ///
    /// Returns epoch exhaustion after the worker owner freezes publication.
    pub fn drain_main(&mut self) -> Result<ViewerState, AppError> {
        let state = self.owner.drain_main();
        if self.owner.epoch_exhausted() {
            Err(AppError::EpochExhausted)
        } else {
            self.staged_hot = state.hot;
            self.staged_main = state.main;
            Ok(state)
        }
    }

    /// Releases one newest exact centre snapshot and its coalesced request reason.
    pub fn take_reference_submission(&mut self) -> Option<ReferenceSubmission> {
        let navigation = self.owner.take_navigation_submission()?;
        let reason = self
            .pending_reason
            .take()
            .unwrap_or(OrbitReason::CENTRE_THRESHOLD);
        Some(ReferenceSubmission { navigation, reason })
    }

    /// Marks a worker response terminal so a coalesced successor may be released.
    #[must_use]
    pub fn finish_reference_submission(&mut self, generation: u32) -> bool {
        self.owner.finish_navigation_submission(generation)
    }

    /// Replaces the owner's projection context after accepting the exact matching reference.
    ///
    /// # Errors
    ///
    /// Returns a typed math refusal for an incompatible centre, plane, scale, or extent.
    pub fn configure_navigation_context(
        &mut self,
        centre: BigCentre,
        reference_centre: BigCentre,
        plane: Plane,
    ) -> Result<(), AppError> {
        self.owner
            .configure_navigation(NavigationConfig {
                centre,
                reference_centre,
                plane,
                grid_width: self.grid_width,
            })
            .map_err(owner_error)
    }

    fn synchronize_shadow(&mut self) -> Result<(), AppError> {
        let state = self.owner.drain_hot();
        if self.owner.epoch_exhausted() {
            return Err(AppError::EpochExhausted);
        }
        self.staged_hot = state.hot;
        self.staged_main = state.main;
        Ok(())
    }

    fn add_reason(&mut self, reason: OrbitReason) {
        self.pending_reason = Some(
            self.pending_reason
                .map_or(reason, |pending| pending.union(reason)),
        );
    }
}

fn axis(value: u32) -> Result<Axis4, AppError> {
    match value {
        0 => Ok(Axis4::E1),
        1 => Ok(Axis4::E2),
        2 => Ok(Axis4::E3),
        3 => Ok(Axis4::E4),
        _ => Err(AppError::Worker(format!(
            "plane seed axis {value} is outside 0..3"
        ))),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Result::map_err supplies the owned sibling error"
)]
fn math_error(error: ember_julibrot_math::MathError) -> AppError {
    AppError::Math(error.to_string())
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Result::map_err supplies the owned sibling error"
)]
fn owner_error(error: ember_julibrot_worker::OwnerError) -> AppError {
    AppError::Worker(error.to_string())
}

#[cfg(test)]
mod tests {
    use ember_julibrot_math::{PlaneAngles, ViewControls};
    use ember_julibrot_present::PaletteId;

    use super::{
        BOX_CLICK_THRESHOLD_PX, NavigationEdit, PRESET_ROWS, SCALE_RANGE_LOG2, ViewerController,
        anchor_px_up, box_zoom_delta_log2, drag_delta_px_down, is_box_selection, preset_row,
    };

    /// The reference browser geometry: a 960x540 render grid laid out at this client rectangle.
    const REFERENCE_RECT: [f64; 2] = [1_022.793_762_207_031_2, 575.315_673_828_125];
    const REFERENCE_GRID: [u32; 2] = [960, 540];

    /// A box that is half the screen on its limiting side is exactly one zoom step.
    #[test]
    fn a_selection_box_zooms_by_its_limiting_ratio() {
        let half = box_zoom_delta_log2(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 2.0],
            REFERENCE_RECT,
        )
        .expect("a half-screen box has a finite factor");
        assert!((half - 1.0).abs() < 1.0e-12);
        // A wide, short box is limited by its width, not by the side that would crop it.
        let wide = box_zoom_delta_log2(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 8.0],
            REFERENCE_RECT,
        )
        .expect("a wide box has a finite factor");
        assert!((wide - 1.0).abs() < 1.0e-12);
        assert!(box_zoom_delta_log2([0.0, 10.0], REFERENCE_RECT).is_err());
        assert!(box_zoom_delta_log2([10.0, f64::NAN], REFERENCE_RECT).is_err());
    }

    /// Under four pixels on either side there is no box, so the gesture is the click it looked like.
    #[test]
    fn a_box_under_the_threshold_is_a_click() {
        assert!(is_box_selection([
            BOX_CLICK_THRESHOLD_PX,
            BOX_CLICK_THRESHOLD_PX
        ]));
        assert!(!is_box_selection([BOX_CLICK_THRESHOLD_PX - 0.001, 200.0]));
        assert!(!is_box_selection([200.0, 0.0]));
        assert!(!is_box_selection([f64::NAN, 200.0]));
    }

    /// A target click recentres without changing scale, and reports the edit it staged.
    ///
    /// The displacement the worker publishes is the click's own pixel offset, because the plane
    /// point that was under the click becomes the centre and the reference has not moved.
    #[test]
    fn a_target_click_moves_the_centre_by_its_own_pixels() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let before = viewer.requested().zoom_log2;
        let edit = viewer
            .set_target([120.0, -45.0], 0.0)
            .expect("finite target");
        assert_eq!(
            edit,
            NavigationEdit::Target {
                anchor_px_up: [120.0, -45.0],
                delta_log2: 0.0
            }
        );
        assert!((viewer.requested().zoom_log2 - before).abs() < f64::EPSILON);
        let displacement = viewer.owner().drain_hot().hot.centre_from_reference_px;
        assert!((displacement[0] - 120.0).abs() < 1.0e-9);
        assert!((displacement[1] + 45.0).abs() < 1.0e-9);
    }

    /// A box release is one edit whose anchor is read at the scale the user was looking at.
    ///
    /// One step of zoom halves the pixel scale, so the same plane offset is twice as many pixels
    /// afterwards; a displacement of exactly the anchor would mean the anchored term had been
    /// evaluated after the zoom instead of before it.
    #[test]
    fn a_box_release_anchors_on_the_scale_before_its_zoom() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let edit = viewer.set_target([80.0, 30.0], 1.0).expect("finite box");
        assert_eq!(
            edit,
            NavigationEdit::Target {
                anchor_px_up: [80.0, 30.0],
                delta_log2: 1.0
            }
        );
        assert!((viewer.requested().zoom_log2 - 1.0).abs() < f64::EPSILON);
        let displacement = viewer.owner().drain_hot().hot.centre_from_reference_px;
        assert!((displacement[0] - 160.0).abs() < 1.0e-9);
        assert!((displacement[1] - 60.0).abs() < 1.0e-9);
    }

    /// The scale control is absolute, refuses its own ends, and zooms about the screen centre.
    #[test]
    fn the_scale_control_is_absolute_and_bounded() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        viewer
            .set_zoom_log2(12.5)
            .expect("a scale inside the range");
        let reached = viewer.requested().zoom_log2;
        assert!(
            (reached - 12.5).abs() <= f64::EPSILON * 12.5,
            "scale reached {reached} rather than 12.5"
        );
        let displacement = viewer.owner().drain_hot().hot.centre_from_reference_px;
        assert!(
            displacement.iter().all(|value| value.abs() < 1.0e-9),
            "a centred zoom moved the centre"
        );
        assert!(viewer.set_zoom_log2(SCALE_RANGE_LOG2[0] - 0.001).is_err());
        assert!(viewer.set_zoom_log2(SCALE_RANGE_LOG2[1] + 0.001).is_err());
        assert!(viewer.set_zoom_log2(f64::NAN).is_err());
    }

    #[test]
    fn the_anchor_is_canvas_centred_render_grid_pixels_with_y_up() {
        let centre = anchor_px_up(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 2.0],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("the canvas centre maps");
        assert!(centre[0].abs() < 1.0e-9 && centre[1].abs() < 1.0e-9);

        let right_edge = anchor_px_up(
            [REFERENCE_RECT[0], REFERENCE_RECT[1] / 2.0],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("the right edge maps");
        assert!((right_edge[0] - 480.0).abs() < 1.0e-9 && right_edge[1].abs() < 1.0e-9);

        let top_edge = anchor_px_up(
            [REFERENCE_RECT[0] / 2.0, 0.0],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("the top edge maps");
        assert!(top_edge[0].abs() < 1.0e-9 && (top_edge[1] - 270.0).abs() < 1.0e-9);

        let bottom_edge = anchor_px_up(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1]],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("the bottom edge maps");
        assert!((bottom_edge[1] + 270.0).abs() < 1.0e-9);
    }

    #[test]
    fn a_non_unit_css_scale_is_applied_in_both_directions() {
        // CSS pixels are larger than grid pixels here, so the anchor shrinks; the device pixel
        // ratio of the page (1.667 when this geometry was measured) is not a term.
        let shrunk = anchor_px_up([800.0, 150.0], REFERENCE_RECT, REFERENCE_GRID)
            .expect("an interior point maps");
        let expected = [
            (800.0 - REFERENCE_RECT[0] / 2.0) * 960.0 / REFERENCE_RECT[0],
            (REFERENCE_RECT[1] / 2.0 - 150.0) * 540.0 / REFERENCE_RECT[1],
        ];
        assert!((shrunk[0] - expected[0]).abs() < 1.0e-9);
        assert!((shrunk[1] - expected[1]).abs() < 1.0e-9);
        assert!((shrunk[0] - 270.884_516_877_356).abs() < 1.0e-9);
        assert!((shrunk[1] - 129.207_729_452_198).abs() < 1.0e-9);

        // A canvas laid out smaller than its grid grows the anchor by the same rule.
        let grown = anchor_px_up([480.0, 135.0], [480.0, 270.0], REFERENCE_GRID)
            .expect("a half-size layout maps");
        assert!((grown[0] - 480.0).abs() < 1.0e-9 && grown[1].abs() < 1.0e-9);
    }

    #[test]
    fn drag_scales_to_the_grid_and_leaves_the_dom_y_flip_to_the_controller() {
        let delta = drag_delta_px_down([100.0, 50.0], REFERENCE_RECT, REFERENCE_GRID)
            .expect("a finite drag maps");
        assert!((delta[0] - 100.0 * 960.0 / REFERENCE_RECT[0]).abs() < 1.0e-9);
        assert!((delta[1] - 50.0 * 540.0 / REFERENCE_RECT[1]).abs() < 1.0e-9);
        assert!(delta[1] > 0.0, "DOM-down y survives the scale unflipped");
    }

    #[test]
    fn a_degenerate_rectangle_or_grid_is_refused_rather_than_dividing() {
        assert!(anchor_px_up([1.0, 1.0], [0.0, 575.0], REFERENCE_GRID).is_err());
        assert!(anchor_px_up([1.0, 1.0], [1_022.8, f64::NAN], REFERENCE_GRID).is_err());
        assert!(anchor_px_up([f64::INFINITY, 1.0], REFERENCE_RECT, REFERENCE_GRID).is_err());
        assert!(anchor_px_up([1.0, 1.0], REFERENCE_RECT, [960, 0]).is_err());
        assert!(drag_delta_px_down([1.0, 1.0], [-4.0, 4.0], REFERENCE_GRID).is_err());
    }

    #[test]
    fn pointer_zoom_and_dom_drag_stage_smooth_hot_state() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        assert_eq!(
            viewer.wheel_zoom(1.0, [20.0, -10.0]).expect("finite wheel"),
            NavigationEdit::Zoom {
                delta_log2: 1.0,
                anchor_px_up: [20.0, -10.0]
            }
        );
        viewer.drag_pan([5.0, 7.0]).expect("finite drag");
        let hot = viewer.drain_hot([960, 540]).expect("valid pose");
        assert_eq!(hot.state.hot.zoom_log2, 1.0);
        assert_eq!(hot.state.hot.centre_from_reference_px, [15.0, -3.0]);
        // A drain reads the controls; it has no time argument and cannot invent an angle.
        assert_eq!(hot.pose.view, ViewControls::NEUTRAL);
    }

    #[test]
    fn requested_controls_do_not_snap_back_on_drains() {
        let mut viewer = ViewerController::new(800).expect("canonical viewer");
        viewer
            .set_plane_angles(PlaneAngles {
                theta_1: 0.4,
                theta_2: 0.7,
            })
            .expect("finite angles");
        viewer.set_iteration_cap(2_048).expect("valid cap");
        viewer.set_palette(PaletteId::Ice).expect("valid palette");
        let relief = preset_row(2).expect("the relief row exists").view;
        viewer
            .set_view_controls(relief)
            .expect("the relief row is in range");
        let first = viewer.drain_hot([800, 600]).expect("first drain");
        let second = viewer.drain_main().expect("main drain");
        assert!(second.epoch > first.state.epoch);
        assert_eq!(viewer.requested().iteration_cap, 2_048);
        assert_eq!(viewer.requested().palette, PaletteId::Ice);
        assert_eq!(viewer.requested().view, relief);
        assert_eq!(second.main.requested_iter_cap, 2_048);
        assert_eq!(second.main.palette_id, PaletteId::Ice as u32);
    }

    #[test]
    fn a_preset_is_a_row_of_controls_and_leaves_the_others_alone() {
        let mut viewer = ViewerController::new(640).expect("canonical viewer");
        viewer.set_iteration_cap(1_024).expect("valid cap");
        viewer.set_palette(PaletteId::Ember).expect("valid palette");
        let julia = preset_row(1).expect("the Julia row exists");
        viewer
            .apply_preset(julia)
            .expect("the Julia row is in range");
        let frame = viewer.drain_hot([640, 480]).expect("valid frame");
        // The seed axes are pinned; a row moves angles and an origin, never an axis.
        assert_eq!(frame.state.main.plane_axis_a, 2);
        assert_eq!(frame.state.main.plane_axis_b, 3);
        assert_eq!(frame.state.main.plane_origin_f64, [0.0, 0.0, -0.8, 0.156]);
        assert_eq!(
            frame.state.main.centre_f64,
            frame.state.main.plane_origin_f64
        );
        assert_eq!(frame.state.main.requested_iter_cap, 1_024);
        assert_eq!(frame.state.main.palette_id, PaletteId::Ember as u32);
        // The quarter turn carries the one seed onto the Julia pair.
        assert_eq!(frame.plane.basis_u[0], 1.0);
        assert!(frame.plane.basis_u[2].abs() <= f32::EPSILON);
        assert_eq!(frame.plane.basis_v[1], 1.0);
    }

    #[test]
    fn every_preset_row_is_a_reachable_control_position() {
        // A preset that a control cannot express or leave would be a mode in disguise.
        let mut viewer = ViewerController::new(640).expect("canonical viewer");
        for (index, row) in PRESET_ROWS.iter().enumerate() {
            let id = u32::try_from(index).expect("four rows fit u32");
            assert_eq!(preset_row(id), Some(*row));
            viewer.apply_preset(*row).expect("every row is in range");
            let requested = viewer.requested();
            assert_eq!(requested.view, row.view);
            assert_eq!(requested.plane_origin, row.plane_origin);
            assert_eq!(
                [
                    requested.plane_angles.theta_1,
                    requested.plane_angles.theta_2
                ],
                row.plane_angles
            );
            assert!(row.view.is_valid());
        }
        let past_end = u32::try_from(PRESET_ROWS.len()).expect("four rows fit u32");
        assert_eq!(preset_row(past_end), None);
        // Leaving a row is one control move, not a mode switch.
        viewer
            .set_view_controls(ViewControls {
                height_scale: 0.5,
                ..viewer.requested().view
            })
            .expect("a half-height row is in range");
        assert!((viewer.requested().view.height_scale - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn partial_controls_preserve_undrained_navigation_fields() {
        let mut viewer = ViewerController::new(800).expect("canonical viewer");
        viewer.wheel_zoom(2.0, [24.0, -12.0]).expect("finite wheel");
        viewer.set_palette(PaletteId::Ice).expect("valid palette");
        let navigation_hot = viewer.owner().snapshot().hot;
        viewer
            .set_plane_angles(PlaneAngles {
                theta_1: 0.2,
                theta_2: -0.3,
            })
            .expect("finite angles");
        viewer.set_iteration_cap(1_024).expect("valid cap");
        let frame = viewer.drain_hot([800, 600]).expect("valid frame");
        assert_eq!(frame.state.hot.zoom_log2, 2.0);
        assert_eq!(
            frame.state.hot.centre_from_reference_px,
            navigation_hot.centre_from_reference_px
        );
        assert_eq!(frame.state.main.palette_id, PaletteId::Ice as u32);
        assert_eq!(frame.state.main.requested_iter_cap, 1_024);
        assert_eq!(frame.state.main.centre_revision, 4);
    }
}
