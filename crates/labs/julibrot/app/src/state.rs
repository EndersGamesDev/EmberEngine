//! Requested controls and worker-owned HOT/MAIN publication integration.

use ember_julibrot_math::{
    Axis4, Plane, PlaneAngles, PlanePreset, PlaneSpec, Pose, ViewMode, construct_plane_from_spec,
    preset_spec,
};
use ember_julibrot_present::PaletteId;
use ember_julibrot_worker::{HotState, MainState, MIN_MAX_ITER, ViewerOwner, ViewerState};

use crate::AppError;

/// Initial requested iteration cap; it is a policy, not a delivered fact.
pub const INITIAL_ITERATION_CAP: u32 = 512;

/// Controls retain requested values independently of delayed worker or GPU work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RequestedControls {
    /// Named plane seed and Julia constant, if selected.
    pub preset: PlanePreset,
    /// Desired base-two zoom exponent.
    pub zoom_log2: f64,
    /// Independent plane angles in radians.
    pub plane_angles: PlaneAngles,
    /// Requested orbit and kernel iteration cap.
    pub iteration_cap: u32,
    /// Requested present-owned palette.
    pub palette: PaletteId,
    /// Requested flat or tumbled presentation.
    pub view: ViewMode,
}

impl Default for RequestedControls {
    fn default() -> Self {
        Self {
            preset: PlanePreset::Mandelbrot,
            zoom_log2: 0.0,
            plane_angles: PlaneAngles {
                theta_1: 0.0,
                theta_2: 0.0,
            },
            iteration_cap: INITIAL_ITERATION_CAP,
            palette: PaletteId::Classic,
            view: ViewMode::Flat,
        }
    }
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

/// App-facing controller whose storage authority remains the worker-owned records.
#[derive(Debug)]
pub struct ViewerController {
    owner: ViewerOwner,
    requested: RequestedControls,
    staged_hot: HotState,
    staged_main: MainState,
}

impl ViewerController {
    /// Creates the canonical Mandelbrot owner and requested control state.
    ///
    /// # Errors
    ///
    /// Returns a math error if the canonical preset contract is unavailable.
    pub fn new() -> Result<Self, AppError> {
        let requested = RequestedControls::default();
        let spec = preset_spec(requested.preset).map_err(math_error)?;
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
                centre_f64: spec.plane_origin,
                plane_axis_a: spec.axis_a as u32,
                plane_axis_b: spec.axis_b as u32,
                plane_origin_f64: spec.plane_origin,
                ..MainState::default()
            },
        };
        Ok(Self {
            owner: ViewerOwner::new(initial),
            requested,
            staged_hot: initial.hot,
            staged_main: initial.main,
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
        if !delta_log2.is_finite()
            || !anchor_px_up.iter().all(|component| component.is_finite())
        {
            return Err(AppError::Math("wheel input is not finite".to_string()));
        }
        let ratio = delta_log2.exp2();
        let zoom_log2 = self.requested.zoom_log2 + delta_log2;
        if !ratio.is_finite() || !zoom_log2.is_finite() {
            return Err(AppError::Math("wheel zoom exceeded finite range".to_string()));
        }
        let mut hot = self.staged_hot;
        hot.centre_from_reference_px = core::array::from_fn(|axis| {
            ratio.mul_add(
                hot.centre_from_reference_px[axis],
                (ratio - 1.0) * anchor_px_up[axis],
            )
        });
        hot.zoom_log2 = zoom_log2;
        if !hot
            .centre_from_reference_px
            .iter()
            .all(|component| component.is_finite())
        {
            return Err(AppError::Math(
                "pointer-anchored centre exceeded finite range".to_string(),
            ));
        }
        self.requested.zoom_log2 = zoom_log2;
        self.staged_hot = hot;
        self.owner.stage_hot(hot);
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
        let centre_delta_px = [-delta_dom[0], delta_dom[1]];
        let mut hot = self.staged_hot;
        for axis in 0..2 {
            hot.centre_from_reference_px[axis] += centre_delta_px[axis];
        }
        if !hot
            .centre_from_reference_px
            .iter()
            .all(|component| component.is_finite())
        {
            return Err(AppError::Math("pan exceeded finite range".to_string()));
        }
        self.staged_hot = hot;
        self.owner.stage_hot(hot);
        Ok(NavigationEdit::Pan { centre_delta_px })
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
        self.requested.plane_angles = angles;
        let mut hot = self.staged_hot;
        hot.plane_theta_1 = angles.theta_1;
        hot.plane_theta_2 = angles.theta_2;
        self.staged_hot = hot;
        self.owner.stage_hot(hot);
        Ok(())
    }

    /// Selects Mandelbrot or Julia, resets its centre to the defining origin, and preserves cap,
    /// palette, and view requests.
    ///
    /// # Errors
    ///
    /// Returns a typed math or centre-revision overflow failure.
    pub fn set_preset(&mut self, preset: PlanePreset) -> Result<(), AppError> {
        let spec = preset_spec(preset).map_err(math_error)?;
        self.requested.preset = preset;
        self.requested.zoom_log2 = 0.0;
        self.requested.plane_angles = PlaneAngles {
            theta_1: 0.0,
            theta_2: 0.0,
        };
        let centre_revision = self
            .staged_main
            .centre_revision
            .checked_add(1)
            .ok_or(AppError::GenerationExhausted)?;
        let hot = HotState {
            zoom_log2: 0.0,
            plane_theta_1: 0.0,
            plane_theta_2: 0.0,
            centre_from_reference_px: [0.0; 2],
        };
        let main = MainState {
            generation_applied: 0,
            centre_revision,
            centre_f64: spec.plane_origin,
            plane_axis_a: spec.axis_a as u32,
            plane_axis_b: spec.axis_b as u32,
            plane_origin_f64: spec.plane_origin,
            orbit_length: 0,
            orbit_id: 0,
            precision_bits: 0,
            reference_shift_px: [0.0; 2],
            ..self.staged_main
        };
        self.staged_hot = hot;
        self.staged_main = main;
        self.owner.stage_hot(hot);
        self.owner.stage_main(main);
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
        self.requested.iteration_cap = max_iter;
        let mut main = self.staged_main;
        main.requested_iter_cap = max_iter;
        self.staged_main = main;
        self.owner.stage_main(main);
        Ok(())
    }

    /// Stages one of present's exact palette identifiers.
    pub fn set_palette(&mut self, palette: PaletteId) {
        self.requested.palette = palette;
        let mut main = self.staged_main;
        main.palette_id = palette as u32;
        self.staged_main = main;
        self.owner.stage_main(main);
    }

    /// Changes the requested view without changing the fractal plane.
    pub const fn set_view(&mut self, view: ViewMode) {
        self.requested.view = view;
    }

    /// Performs the mandatory HOT drain and constructs the current math pose.
    ///
    /// # Errors
    ///
    /// Returns a typed axis, plane, extent, or time failure.
    pub fn drain_hot(
        &self,
        grid_extent: [u32; 2],
        view_time_seconds: f64,
    ) -> Result<HotFrame, AppError> {
        if grid_extent[0] == 0 || grid_extent[1] == 0 || !view_time_seconds.is_finite() {
            return Err(AppError::Math("pose extent or time is invalid".to_string()));
        }
        let state = self.owner.drain_hot();
        if self.owner.epoch_exhausted() {
            return Err(AppError::EpochExhausted);
        }
        let spec = PlaneSpec {
            axis_a: axis(state.main.plane_axis_a)?,
            axis_b: axis(state.main.plane_axis_b)?,
            plane_origin: state.main.plane_origin_f64,
        };
        let plane = construct_plane_from_spec(
            spec,
            PlaneAngles {
                theta_1: state.hot.plane_theta_1,
                theta_2: state.hot.plane_theta_2,
            },
        )
        .map_err(math_error)?;
        let pose = Pose {
            epoch: state.epoch,
            orbit_generation: state.main.generation_applied,
            plane,
            plane_theta_1: state.hot.plane_theta_1,
            plane_theta_2: state.hot.plane_theta_2,
            zoom_log2: state.hot.zoom_log2,
            view_theta_1: 0.4 * view_time_seconds,
            grid_width: grid_extent[0],
            grid_height: grid_extent[1],
            view: self.requested.view,
            centre_from_reference_px: state.hot.centre_from_reference_px,
        };
        Ok(HotFrame { state, plane, pose })
    }

    /// Performs an infallible MAIN drain and checks only the impossible epoch wall.
    ///
    /// # Errors
    ///
    /// Returns epoch exhaustion after the worker owner freezes publication.
    pub fn drain_main(&self) -> Result<ViewerState, AppError> {
        let state = self.owner.drain_main();
        if self.owner.epoch_exhausted() {
            Err(AppError::EpochExhausted)
        } else {
            Ok(state)
        }
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

#[cfg(test)]
mod tests {
    use ember_julibrot_math::{PlaneAngles, PlanePreset, ViewMode};
    use ember_julibrot_present::PaletteId;

    use super::{NavigationEdit, ViewerController};

    #[test]
    fn pointer_zoom_and_dom_drag_stage_smooth_hot_state() {
        let mut viewer = ViewerController::new().expect("canonical viewer");
        assert_eq!(
            viewer
                .wheel_zoom(1.0, [20.0, -10.0])
                .expect("finite wheel"),
            NavigationEdit::Zoom {
                delta_log2: 1.0,
                anchor_px_up: [20.0, -10.0]
            }
        );
        viewer.drag_pan([5.0, 7.0]).expect("finite drag");
        let hot = viewer.drain_hot([960, 540], 2.5).expect("valid pose");
        assert_eq!(hot.state.hot.zoom_log2, 1.0);
        assert_eq!(hot.state.hot.centre_from_reference_px, [15.0, -3.0]);
        assert_eq!(hot.pose.view_theta_1, 1.0);
    }

    #[test]
    fn requested_controls_do_not_snap_back_on_drains() {
        let mut viewer = ViewerController::new().expect("canonical viewer");
        viewer
            .set_plane_angles(PlaneAngles {
                theta_1: 0.4,
                theta_2: 0.7,
            })
            .expect("finite angles");
        viewer.set_iteration_cap(2_048).expect("valid cap");
        viewer.set_palette(PaletteId::Ice);
        viewer.set_view(ViewMode::Tumbled);
        let first = viewer.drain_hot([800, 600], 0.0).expect("first drain");
        let second = viewer.drain_main().expect("main drain");
        assert!(second.epoch > first.state.epoch);
        assert_eq!(viewer.requested().iteration_cap, 2_048);
        assert_eq!(viewer.requested().palette, PaletteId::Ice);
        assert_eq!(viewer.requested().view, ViewMode::Tumbled);
        assert_eq!(second.main.requested_iter_cap, 2_048);
        assert_eq!(second.main.palette_id, PaletteId::Ice as u32);
    }

    #[test]
    fn preset_resets_centre_and_plane_without_resetting_other_controls() {
        let mut viewer = ViewerController::new().expect("canonical viewer");
        viewer.set_iteration_cap(1_024).expect("valid cap");
        viewer.set_palette(PaletteId::Ember);
        viewer
            .set_preset(PlanePreset::Julia { c0: [-0.8, 0.156] })
            .expect("finite Julia constant");
        let frame = viewer.drain_hot([640, 480], 0.0).expect("valid frame");
        assert_eq!(frame.state.main.plane_axis_a, 0);
        assert_eq!(frame.state.main.plane_axis_b, 1);
        assert_eq!(frame.state.main.plane_origin_f64, [0.0, 0.0, -0.8, 0.156]);
        assert_eq!(frame.state.main.centre_f64, frame.state.main.plane_origin_f64);
        assert_eq!(frame.state.main.requested_iter_cap, 1_024);
        assert_eq!(frame.state.main.palette_id, PaletteId::Ember as u32);
        assert_eq!(frame.plane.basis_u, [1.0, 0.0, 0.0, 0.0]);
    }
}
