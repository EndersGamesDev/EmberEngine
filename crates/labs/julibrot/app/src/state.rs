//! Requested controls and worker-owned HOT/MAIN publication integration.

#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;

use ember_julibrot_math::{
    Axis4, BigCentre, Homography, MathError, NavigationDelta, ObjectAngles, Plane, PlaneAngles,
    Pose, PoseMap, PrecisionMode, SEED_AXES, SceneFootprint, ViewControls, construct_plane,
    navigation_delta, pixel_scale, plane_chart_relation, plane_to_screen, scene_footprint,
    screen_to_plane,
};
use ember_julibrot_present::PaletteId;
use ember_julibrot_worker::{
    HotState, MIN_MAX_ITER, MainState, NavigationConfig, NavigationSubmission, OrbitReason,
    ViewerOwner, ViewerState,
};

use crate::{AppError, SavedView};

/// Initial requested iteration cap; it is a policy, not a delivered fact.
pub const INITIAL_ITERATION_CAP: u32 = 512;

pub const NAVIGATION_PRECISION_BITS: u32 = 1_024;

/// The boundary point a fresh whole-set view marks as its zoom target.
pub const SEAHORSE_VALLEY_TARGET: [f64; 4] =
    [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33];

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
    /// Six ordered object angles in radians.
    pub object_angles: ObjectAngles,
    /// Requested orbit and kernel iteration cap.
    pub iteration_cap: u32,
    /// Requested precision policy.
    pub precision_mode: PrecisionMode,
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
            object_angles: ObjectAngles::IDENTITY,
            iteration_cap: INITIAL_ITERATION_CAP,
            precision_mode: PrecisionMode::PictureFast,
            palette: PaletteId::Classic,
            view: ViewControls::MANDELBROT_FLAT,
        }
    }
}

/// One preset: a named row of control values and nothing else.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PresetRow {
    /// Stable name shown by the page.
    pub name: &'static str,
    /// Six ordered object angles in radians.
    pub object_angles: ObjectAngles,
    /// Absolute plane origin.
    pub plane_origin: [f64; 4],
    /// Every VIEW control.
    pub view: ViewControls,
}

const QUARTER_TURN: f64 = core::f64::consts::FRAC_PI_2;

/// The relief rows' observer, which is the orientation the retired fixed mount had.
const MANDELBROT_RELIEF_VIEW: ViewControls = ViewControls {
    camera: [
        0.6,
        -QUARTER_TURN,
        0.0,
        0.0,
        -QUARTER_TURN,
        0.0,
        0.0,
        0.0,
        0.97,
        0.0,
    ],
    camera_translation: [0.0; 5],
    camera_yaw: 0.349,
    camera_pitch: 0.262,
    height_scale: 1.0,
    distance_five: 8.0,
    distance_four: 8.0,
};

const JULIA_RELIEF_VIEW: ViewControls = ViewControls {
    camera: [0.6, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.97, 0.0],
    camera_translation: [0.0; 5],
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
        object_angles: ObjectAngles::IDENTITY,
        plane_origin: [0.0; 4],
        view: ViewControls::MANDELBROT_FLAT,
    },
    PresetRow {
        name: "Julia",
        object_angles: ObjectAngles::JULIA,
        plane_origin: [0.0, 0.0, JULIA_C0[0], JULIA_C0[1]],
        view: ViewControls::NEUTRAL,
    },
    PresetRow {
        name: "Mandelbrot relief",
        object_angles: ObjectAngles::IDENTITY,
        plane_origin: [0.0; 4],
        view: MANDELBROT_RELIEF_VIEW,
    },
    PresetRow {
        name: "Julia relief",
        object_angles: ObjectAngles::JULIA,
        plane_origin: [0.0, 0.0, JULIA_C0[0], JULIA_C0[1]],
        view: JULIA_RELIEF_VIEW,
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

/// Projects a canvas-centred render-grid offset with positive y upward back onto the DOM position
/// inside the canvas box, in CSS pixels with y downward.
///
/// It is the exact inverse of `anchor_px_up`, and it exists so that one screen point converted to
/// a point on the slice can be drawn again wherever the current map puts it. A crosshair drawn by
/// remembering the pixel it was clicked at is a crosshair that lies the moment the picture moves;
/// a crosshair re-projected from the point it was set on is the only one that can be an accuracy
/// oracle.
///
/// # Errors
///
/// Returns a math failure for non-finite input, a degenerate client rectangle, or a zero grid.
pub fn css_from_anchor_px_up(
    anchor_px_up: [f64; 2],
    rect_css: [f64; 2],
    grid: [u32; 2],
) -> Result<[f64; 2], AppError> {
    if !anchor_px_up.iter().all(|value| value.is_finite()) {
        return Err(AppError::Math("anchor offset is not finite".to_string()));
    }
    let scale = css_to_grid_scale(rect_css, grid)?;
    Ok([
        anchor_px_up[0] / scale[0] + rect_css[0] / 2.0,
        rect_css[1] / 2.0 - anchor_px_up[1] / scale[1],
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

/// The bit-exact inputs that determine one checked neutral-height map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScreenMapKey {
    object: [u64; 6],
    camera: [u64; 10],
    camera_translation: [u64; 5],
    camera_yaw: u64,
    camera_pitch: u64,
    height_scale: u64,
    distance_five: u64,
    distance_four: u64,
    zoom_log2: u64,
    extent: [u32; 2],
}

impl ScreenMapKey {
    fn new(object: ObjectAngles, view: ViewControls, zoom_log2: f64, extent: [u32; 2]) -> Self {
        Self {
            object: object.as_array().map(f64::to_bits),
            camera: view.camera.map(f64::to_bits),
            camera_translation: view.camera_translation.map(f64::to_bits),
            camera_yaw: view.camera_yaw.to_bits(),
            camera_pitch: view.camera_pitch.to_bits(),
            height_scale: view.height_scale.to_bits(),
            distance_five: view.distance_five.to_bits(),
            distance_four: view.distance_four.to_bits(),
            zoom_log2: zoom_log2.to_bits(),
            extent,
        }
    }
}

/// One checked neutral-height map and the exact key that produced it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CheckedScreenMap {
    key: ScreenMapKey,
    map: PoseMap,
}

/// The bit-exact inputs that determine one rasterized scene footprint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneFootprintKey {
    object: [u64; 6],
    view: [u64; 20],
    extent: [u32; 2],
}

impl SceneFootprintKey {
    fn new(object: ObjectAngles, view: ViewControls, extent: [u32; 2]) -> Self {
        Self {
            object: object.as_array().map(f64::to_bits),
            view: view.as_array().map(f64::to_bits),
            extent,
        }
    }
}

/// One rasterized footprint and the exact key that produced it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CheckedSceneFootprint {
    key: SceneFootprintKey,
    footprint: SceneFootprint,
}

/// One worker-owned centre snapshot paired with the app's accumulated request reason.
#[derive(Clone, Debug, PartialEq)]
pub struct ReferenceSubmission {
    /// Exact navigation snapshot released by the owner.
    pub navigation: NavigationSubmission,
    /// Exact point whose orbit will serve that navigation snapshot.
    pub reference_centre: BigCentre,
    /// All reasons coalesced since the preceding released request.
    pub reason: OrbitReason,
}

/// App-facing controller whose storage authority remains the worker-owned records.
///
/// Its exact three-entry map and footprint caches use FIFO with promote-on-hit, preserving
/// recently reused refresh keys across a fourth insertion.
#[derive(Debug)]
pub struct ViewerController {
    owner: ViewerOwner,
    requested: RequestedControls,
    requested_revision: u64,
    checked_plane: Plane,
    checked_screen_maps: RefCell<[Option<CheckedScreenMap>; 3]>,
    checked_scene_footprints: RefCell<[Option<CheckedSceneFootprint>; 3]>,
    #[cfg(test)]
    plane_constructions: u64,
    #[cfg(test)]
    main_state_rebuilds: u64,
    #[cfg(test)]
    map_constructions: Cell<u64>,
    #[cfg(test)]
    footprint_constructions: Cell<u64>,
    navigation_centre_f64: [f64; 4],
    staged_hot: HotState,
    staged_main: MainState,
    pending_reason: Option<OrbitReason>,
    pending_reference_centre: Option<(u32, BigCentre)>,
    grid_width: u32,
    crosshair: Option<BigCentre>,
    grid_extent: [u32; 2],
}

/// Accepted constructor forms for callers that know only width or the complete render extent.
pub trait IntoGridExtent {
    /// Converts the caller's extent into `[width, height]`.
    fn into_grid_extent(self) -> [u32; 2];
}

impl IntoGridExtent for u32 {
    fn into_grid_extent(self) -> [u32; 2] {
        [self, self]
    }
}

impl IntoGridExtent for [u32; 2] {
    fn into_grid_extent(self) -> [u32; 2] {
        self
    }
}

impl ViewerController {
    /// Creates the canonical Mandelbrot owner and requested control state.
    ///
    /// # Errors
    ///
    /// Returns a math error if the canonical preset contract is unavailable.
    pub fn new(grid_extent: impl IntoGridExtent) -> Result<Self, AppError> {
        let grid_extent = grid_extent.into_grid_extent();
        let grid_width = grid_extent[0];
        let requested = RequestedControls::default();
        let origin = requested.plane_origin;
        let plane = construct_plane(requested.object_angles).map_err(math_error)?;
        let centre = BigCentre::from_f64(origin, NAVIGATION_PRECISION_BITS).map_err(math_error)?;
        let target = BigCentre::from_f64(SEAHORSE_VALLEY_TARGET, NAVIGATION_PRECISION_BITS)
            .map_err(math_error)?;
        let initial = ViewerState {
            epoch: 0,
            hot: HotState {
                zoom_log2: requested.zoom_log2,
                plane_theta_1: requested.object_angles.rho_13,
                plane_theta_2: requested.object_angles.rho_24,
                centre_from_reference_px: [0.0; 2],
            },
            main: MainState {
                requested_iter_cap: requested.iteration_cap,
                precision_mode: requested.precision_mode as u32,
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
            requested_revision: 0,
            checked_plane: plane,
            checked_screen_maps: RefCell::new([None; 3]),
            checked_scene_footprints: RefCell::new([None; 3]),
            #[cfg(test)]
            plane_constructions: 1,
            #[cfg(test)]
            main_state_rebuilds: 0,
            #[cfg(test)]
            map_constructions: Cell::new(0),
            #[cfg(test)]
            footprint_constructions: Cell::new(0),
            navigation_centre_f64: origin,
            staged_hot: initial.hot,
            staged_main: initial.main,
            pending_reason: Some(OrbitReason::INITIAL),
            pending_reference_centre: None,
            grid_width,
            crosshair: Some(target),
            grid_extent,
        })
    }

    /// Returns requested controls without consulting delayed delivered state.
    #[must_use]
    pub const fn requested(&self) -> RequestedControls {
        self.requested
    }

    /// Returns the serial of the last bit-distinct requested picture state.
    #[must_use]
    pub const fn requested_revision(&self) -> u64 {
        self.requested_revision
    }

    /// Returns the checked plane cached for the current object angles.
    #[must_use]
    pub const fn checked_plane(&self) -> Plane {
        self.checked_plane
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
        let map = self.mapped_screen_map(self.grid_extent)?;
        let delta =
            navigation_delta(&map, [0.0; 2], delta_log2, anchor_px_up).map_err(math_error)?;
        self.owner.navigate(delta);
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.refresh_navigation_centre_mirror();
        self.requested.zoom_log2 = zoom_log2;
        self.note_requested_change();
        self.add_reason(OrbitReason::ZOOM_THRESHOLD.union(OrbitReason::CENTRE_THRESHOLD));
        Ok(NavigationEdit::Zoom {
            delta_log2,
            anchor_px_up,
        })
    }

    /// Converts DOM-down drag input through the inverse screen map and stages it immediately.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result.
    pub fn drag_pan(&mut self, delta_dom: [f64; 2]) -> Result<NavigationEdit, AppError> {
        if !delta_dom.iter().all(|component| component.is_finite()) {
            return Err(AppError::Math("drag input is not finite".to_string()));
        }
        let map = self.mapped_screen_map(self.grid_extent)?;
        let delta = navigation_delta(&map, delta_dom, 0.0, [0.0; 2]).map_err(math_error)?;
        self.owner.navigate(delta);
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.refresh_navigation_centre_mirror();
        self.note_requested_change();
        self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        let centre_delta_px = [-delta.pan_canvas_px[0], -delta.pan_canvas_px[1]];
        Ok(NavigationEdit::Pan { centre_delta_px })
    }

    /// Stores one screen point as a point on the slice, without moving the picture.
    ///
    /// A click is not a navigation edit. The point under the pointer is converted once, through
    /// the very plane basis and pixel scale the owner's own navigation arithmetic uses, into the
    /// bignum point of the slice it names; the picture does not move and no reference orbit is
    /// asked for. Everything else about the crosshair follows from that one conversion: it is
    /// re-projected for drawing, it rides along under a pan because the point did not move, and it
    /// is the anchor every later zoom is taken about.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or unconfigured navigation.
    pub fn set_crosshair(&mut self, anchor_px_up: [f64; 2]) -> Result<(), AppError> {
        if !anchor_px_up.iter().all(|component| component.is_finite()) {
            return Err(AppError::Math("crosshair input is not finite".to_string()));
        }
        let map = self.mapped_screen_map(self.grid_extent)?;
        let plane_offset = navigation_delta(&map, [0.0; 2], 0.0, anchor_px_up)
            .map_err(math_error)?
            .anchor_canvas_px;
        let (centre, plane, grid_width) = self.navigation_frame()?;
        let zoom_log2 = self.requested.zoom_log2;
        let mut point = centre;
        point
            .apply_navigation(
                &NavigationDelta {
                    pan_canvas_px: [-plane_offset[0], -plane_offset[1]],
                    zoom_delta_log2: 0.0,
                    anchor_canvas_px: [0.0; 2],
                },
                &plane,
                zoom_log2,
                zoom_log2,
                grid_width,
            )
            .map_err(math_error)?;
        self.crosshair = Some(point);
        Ok(())
    }

    /// Forgets the stored point, so later zooms are taken about the screen centre again.
    pub fn clear_crosshair(&mut self) {
        self.crosshair = None;
    }

    /// Returns the stored point's offset from the current centre in render-grid pixels, `+y` up.
    ///
    /// This is the projection: it is recomputed from the point and the current map every time it
    /// is asked for, so a pan, a zoom, or an accepted reference all move the drawn crosshair with
    /// the feature it was set on rather than leaving it behind.
    #[must_use]
    pub fn crosshair_plane_px(&self) -> Option<[f64; 2]> {
        let target = self.crosshair.as_ref()?;
        let (centre, plane, grid_width) = self.navigation_frame().ok()?;
        let scale = pixel_scale(self.requested.zoom_log2, grid_width).ok()?;
        let bits = target.precision_bits.max(centre.precision_bits);
        let target = target.with_precision(bits).ok()?;
        let centre = centre.with_precision(bits).ok()?;
        let plane_offset = target.displacement_px(&centre, &plane, scale).ok()?;
        let map = self.mapped_screen_map(self.grid_extent).ok()?;
        plane_to_screen(&map, plane_offset).ok()
    }

    /// Returns the Astro-float precision the stored point is held at.
    #[must_use]
    pub fn crosshair_precision_bits(&self) -> Option<u32> {
        self.crosshair.as_ref().map(|point| point.precision_bits)
    }

    /// Returns the finite mirror of the stored point, for the facts overlay only.
    #[must_use]
    pub fn crosshair_centre_f64(&self) -> Option<[f64; 4]> {
        self.crosshair
            .as_ref()
            .map(ember_julibrot_math::BigCentre::to_f64_mirror)
    }

    pub(crate) const fn crosshair(&self) -> Option<&BigCentre> {
        self.crosshair.as_ref()
    }

    /// Translates the picture by a DOM drag displacement, leaving the stored point where it is.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result.
    pub fn pan_px(&mut self, delta_dom: [f64; 2]) -> Result<NavigationEdit, AppError> {
        self.drag_pan(delta_dom)
    }

    /// Changes the zoom about the stored point, or about the screen centre when none is stored.
    ///
    /// # Errors
    ///
    /// Returns a math failure for non-finite input or result, or a typed owner refusal.
    pub fn zoom_about_crosshair(&mut self, delta_log2: f64) -> Result<NavigationEdit, AppError> {
        let anchor = self.crosshair_plane_px().unwrap_or([0.0; 2]);
        self.wheel_zoom(delta_log2, anchor)
    }

    /// Returns the centre, plane basis and grid width one conversion needs, or a typed refusal.
    fn navigation_frame(&self) -> Result<(BigCentre, Plane, u32), AppError> {
        let centre = self
            .owner
            .navigation_centre()
            .ok_or_else(|| AppError::Math("navigation is unconfigured".to_string()))?;
        let plane = self
            .owner
            .navigation_plane()
            .ok_or_else(|| AppError::Math("navigation is unconfigured".to_string()))?;
        let grid_width = self
            .owner
            .navigation_grid_width()
            .ok_or_else(|| AppError::Math("navigation is unconfigured".to_string()))?;
        Ok((centre, plane, grid_width))
    }

    /// Moves the `scale` control to an absolute zoom exponent, about the stored point.
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
        self.zoom_about_crosshair(zoom_log2 - self.requested.zoom_log2)
    }

    /// Requests a new reference at one deterministic pixel of a completed refinement level.
    ///
    /// The navigation centre stays fixed. Only the orbit point moves, so the next perturbation
    /// uniform carries the centre-minus-reference displacement needed to sample the same view.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for an invalid extent/index, an uncertified screen map, bignum
    /// arithmetic failure, or generation exhaustion.
    pub fn request_reference_for_pixel(
        &mut self,
        index: u32,
        grid_extent: [u32; 2],
    ) -> Result<u32, AppError> {
        let [width, height] = grid_extent;
        let active_len = width
            .checked_mul(height)
            .filter(|length| *length > 0 && index < *length)
            .ok_or_else(|| {
                AppError::Math("reference sample index is outside its grid".to_string())
            })?;
        debug_assert!(active_len > index);
        let column = index % width;
        let row = index / width;
        let anchor = [
            0.5f64.mul_add(-f64::from(width), f64::from(column) + 0.5),
            0.5f64.mul_add(-f64::from(height), f64::from(row) + 0.5),
        ];
        let map = self.screen_map(grid_extent)?;
        let PoseMap::Mapped(map) = map else {
            return Err(AppError::Math(
                "an edge-on view has no reference sample".to_string(),
            ));
        };
        let plane_offset = navigation_delta(&map, [0.0; 2], 0.0, anchor)
            .map_err(math_error)?
            .anchor_canvas_px;
        let (mut point, plane, _) = self.navigation_frame()?;
        point
            .apply_navigation(
                &NavigationDelta {
                    pan_canvas_px: [-plane_offset[0], -plane_offset[1]],
                    zoom_delta_log2: 0.0,
                    anchor_canvas_px: [0.0; 2],
                },
                &plane,
                self.requested.zoom_log2,
                self.requested.zoom_log2,
                width,
            )
            .map_err(math_error)?;
        let generation = self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.pending_reference_centre = Some((generation, point));
        self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        Ok(generation)
    }

    /// Stages all six ordered object angles without resetting other HOT controls.
    ///
    /// # Errors
    ///
    /// Returns a math failure when an angle is non-finite or outside its range.
    pub fn set_object_angles(&mut self, angles: ObjectAngles) -> Result<(), AppError> {
        if !angles.is_valid() {
            return Err(AppError::Math("object angles are not valid".to_string()));
        }
        // Requested controls are bit keys, so signed zero remains a distinct requested edit.
        if f64_bits_eq(angles.as_array(), self.requested.object_angles.as_array()) {
            return Ok(());
        }
        self.synchronize_shadow()?;
        let checked_plane = construct_plane(angles).map_err(math_error)?;
        let plane_preserving = plane_chart_relation(self.checked_plane, checked_plane).is_some();
        let rotated_displacement = if plane_preserving {
            Some(
                self.owner
                    .reorient_navigation_plane(checked_plane)
                    .map_err(owner_error)?,
            )
        } else {
            None
        };
        if !plane_preserving {
            self.clear_crosshair();
        }
        self.requested.object_angles = angles;
        self.checked_plane = checked_plane;
        #[cfg(test)]
        {
            self.plane_constructions = self.plane_constructions.saturating_add(1);
        }
        let mut hot = self.staged_hot;
        hot.plane_theta_1 = angles.rho_13;
        hot.plane_theta_2 = angles.rho_24;
        if let Some(displacement) = rotated_displacement {
            hot.centre_from_reference_px = displacement;
        }
        self.staged_hot = hot;
        self.owner.stage_hot(hot);
        if !plane_preserving {
            self.owner.navigate(NavigationDelta::default());
            if let Some(error) = self.owner.take_navigation_error() {
                return Err(owner_error(error));
            }
            self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        }
        self.note_requested_change();
        Ok(())
    }

    /// Updates the two legacy plane-angle aliases while preserving the other object factors.
    ///
    /// # Errors
    ///
    /// Returns the same typed failure as [`Self::set_object_angles`].
    pub fn set_plane_angles(&mut self, angles: PlaneAngles) -> Result<(), AppError> {
        self.set_object_angles(ObjectAngles {
            rho_13: angles.theta_1,
            rho_24: angles.theta_2,
            ..self.requested.object_angles
        })
    }

    /// Moves the absolute plane origin, resetting the centre and target while preserving cap,
    /// palette, and every VIEW control.
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
        if f64_bits_eq(origin, self.requested.plane_origin) {
            return Ok(());
        }
        self.synchronize_shadow()?;
        self.requested.plane_origin = origin;
        self.requested.zoom_log2 = 0.0;
        let angles = self.requested.object_angles;
        let hot = HotState {
            zoom_log2: 0.0,
            plane_theta_1: angles.rho_13,
            plane_theta_2: angles.rho_24,
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
        #[cfg(test)]
        {
            self.main_state_rebuilds = self.main_state_rebuilds.saturating_add(1);
        }
        let plane = self.checked_plane;
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
        self.pending_reference_centre = None;
        self.navigation_centre_f64 = origin;
        self.clear_crosshair();
        self.note_requested_change();
        Ok(())
    }

    /// Installs an authoritative centre as its own reference, preserving every other control.
    ///
    /// Loading a saved view is the one edit that names a centre outright rather than nudging the
    /// one already there, so it configures navigation the way a new plane origin does; the loaded
    /// centre becomes the reference too, since no orbit computed elsewhere is valid at it.
    ///
    /// # Errors
    ///
    /// Returns a typed math or owner refusal for an invalid plane, scale, or centre.
    pub fn set_centre(&mut self, centre: BigCentre) -> Result<(), AppError> {
        if self.owner.navigation_centre().as_ref() == Some(&centre)
            && self.owner.reference_centre().as_ref() == Some(&centre)
        {
            return Ok(());
        }
        self.synchronize_shadow()?;
        let plane = self.checked_plane;
        let centre_f64 = centre.to_f64_mirror();
        self.owner
            .configure_navigation(NavigationConfig {
                centre: centre.clone(),
                reference_centre: centre,
                plane,
                grid_width: self.grid_extent[0],
            })
            .map_err(owner_error)?;
        self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.navigation_centre_f64 = centre_f64;
        self.add_reason(OrbitReason::CENTRE_THRESHOLD);
        self.note_requested_change();
        Ok(())
    }

    /// Applies a complete stored row as one bit-filtered transaction.
    ///
    /// Pose-only changes stage no navigation. A changed centre or scale configures navigation
    /// once, and a slice change additionally retires the old orbit metadata before that one
    /// navigation request is released.
    ///
    /// # Errors
    ///
    /// Returns a typed math, owner, or epoch refusal without applying an invalid row.
    #[allow(
        clippy::too_many_lines,
        reason = "validation, bit comparison, staging, and the single navigation decision stay one transaction"
    )]
    pub fn apply_saved_view(&mut self, row: &SavedView) -> Result<(), AppError> {
        let object = row.object_angles();
        let origin = row.origin;
        let view = row.view();
        let zoom_log2 = row.zoom_log2;
        let centre = row.centre()?;
        let target = row.target()?;
        let centre_f64 = centre.to_f64_mirror();
        if !object.is_valid() {
            return Err(AppError::Math(
                "saved object angles are not valid".to_string(),
            ));
        }
        if !origin.iter().all(|value| value.is_finite()) {
            return Err(AppError::Math(
                "saved plane origin is not finite".to_string(),
            ));
        }
        if !view.is_valid() {
            return Err(AppError::Math(
                "saved VIEW controls are not valid".to_string(),
            ));
        }
        if !zoom_log2.is_finite()
            || zoom_log2 < SCALE_RANGE_LOG2[0]
            || zoom_log2 > SCALE_RANGE_LOG2[1]
        {
            return Err(AppError::Math(format!(
                "saved scale {zoom_log2} is outside the control range"
            )));
        }

        let object_changed =
            !f64_bits_eq(object.as_array(), self.requested.object_angles.as_array());
        let origin_changed = !f64_bits_eq(origin, self.requested.plane_origin);
        let view_changed = !f64_bits_eq(view.as_array(), self.requested.view.as_array());
        let zoom_changed = zoom_log2.to_bits() != self.requested.zoom_log2.to_bits();
        let centre_changed = self.owner.navigation_centre().as_ref() != Some(&centre)
            || self.owner.reference_centre().as_ref() != Some(&centre);
        let target_changed = self.crosshair.as_ref() != target.as_ref();
        if !object_changed
            && !origin_changed
            && !view_changed
            && !zoom_changed
            && !centre_changed
            && !target_changed
        {
            return Ok(());
        }

        let checked_plane = if object_changed {
            construct_plane(object).map_err(math_error)?
        } else {
            self.checked_plane
        };
        let plane_preserving = plane_chart_relation(self.checked_plane, checked_plane).is_some();
        let slice_changed = !plane_preserving
            || (origin_changed
                && !origins_share_slice(
                    self.requested.plane_origin,
                    origin,
                    checked_plane,
                    zoom_log2,
                    self.grid_width,
                ));
        let navigation_changed = slice_changed || centre_changed || zoom_changed;

        if object_changed || origin_changed || navigation_changed {
            self.synchronize_shadow()?;
        }
        let reoriented_displacement = if object_changed && plane_preserving && !navigation_changed {
            Some(
                self.owner
                    .reorient_navigation_plane(checked_plane)
                    .map_err(owner_error)?,
            )
        } else {
            None
        };

        if object_changed {
            self.requested.object_angles = object;
            self.checked_plane = checked_plane;
            #[cfg(test)]
            {
                self.plane_constructions = self.plane_constructions.saturating_add(1);
            }
        }
        if origin_changed {
            self.requested.plane_origin = origin;
        }
        if view_changed {
            self.requested.view = view;
        }
        if zoom_changed {
            self.requested.zoom_log2 = zoom_log2;
        }
        if object_changed || zoom_changed || reoriented_displacement.is_some() {
            let mut hot = self.staged_hot;
            if zoom_changed {
                hot.zoom_log2 = zoom_log2;
            }
            if object_changed {
                hot.plane_theta_1 = object.rho_13;
                hot.plane_theta_2 = object.rho_24;
            }
            if let Some(displacement) = reoriented_displacement {
                hot.centre_from_reference_px = displacement;
            }
            self.staged_hot = hot;
            self.owner.stage_hot(hot);
        }

        if origin_changed || slice_changed {
            let mut main = self.staged_main;
            main.plane_origin_f64 = origin;
            if slice_changed {
                main.generation_applied = 0;
                main.orbit_length = 0;
                main.orbit_id = 0;
                main.precision_bits = 0;
                main.reference_shift_px = [0.0; 2];
                #[cfg(test)]
                {
                    self.main_state_rebuilds = self.main_state_rebuilds.saturating_add(1);
                }
            }
            self.staged_main = main;
            self.owner.stage_main(main);
        }

        if navigation_changed {
            self.owner
                .configure_navigation(NavigationConfig {
                    centre: centre.clone(),
                    reference_centre: centre,
                    plane: checked_plane,
                    grid_width: self.grid_width,
                })
                .map_err(owner_error)?;
            self.owner.navigate(NavigationDelta::default());
            if let Some(error) = self.owner.take_navigation_error() {
                return Err(owner_error(error));
            }
            self.navigation_centre_f64 = centre_f64;
            self.pending_reference_centre = None;
            self.pending_reason = Some(if slice_changed {
                OrbitReason::INITIAL
            } else if centre_changed {
                OrbitReason::CENTRE_THRESHOLD
            } else {
                OrbitReason::ZOOM_THRESHOLD
            });
        }
        self.crosshair = target;
        if object_changed || origin_changed || view_changed || navigation_changed {
            self.note_requested_change();
        }
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
        if self.requested.iteration_cap == max_iter {
            return Ok(());
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
        self.note_requested_change();
        Ok(())
    }

    /// Stages a precision-policy change as incompatible MAIN work.
    ///
    /// # Errors
    ///
    /// Returns the owner's typed navigation or epoch refusal.
    pub fn set_precision_mode(&mut self, precision_mode: PrecisionMode) -> Result<(), AppError> {
        if self.requested.precision_mode == precision_mode {
            return Ok(());
        }
        self.synchronize_shadow()?;
        self.requested.precision_mode = precision_mode;
        let mut main = self.staged_main;
        main.precision_mode = precision_mode as u32;
        self.staged_main = main;
        self.owner.stage_main(main);
        self.owner.navigate(NavigationDelta::default());
        if let Some(error) = self.owner.take_navigation_error() {
            return Err(owner_error(error));
        }
        self.add_reason(OrbitReason::PRECISION_MODE_CHANGE);
        self.note_requested_change();
        Ok(())
    }

    /// Stages one of present's exact palette identifiers.
    ///
    /// # Errors
    ///
    /// Returns epoch exhaustion when the full worker-owned record can no longer synchronize.
    pub fn set_palette(&mut self, palette: PaletteId) -> Result<(), AppError> {
        if self.requested.palette == palette {
            return Ok(());
        }
        self.synchronize_shadow()?;
        self.requested.palette = palette;
        let mut main = self.staged_main;
        main.palette_id = palette as u32;
        self.staged_main = main;
        self.owner.stage_main(main);
        self.note_requested_change();
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
        if f64_bits_eq(view.as_array(), self.requested.view.as_array()) {
            return Ok(());
        }
        self.requested.view = view;
        self.note_requested_change();
        Ok(())
    }

    /// Applies one preset row through the same paths a user's own movement reaches.
    ///
    /// # Errors
    ///
    /// Returns the typed failure of whichever staged control refused.
    pub fn apply_preset(&mut self, row: PresetRow) -> Result<(), AppError> {
        let saved = SavedView::from_preset(row)?;
        self.apply_saved_view(&saved)
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
        if state.hot.zoom_log2.to_bits() != self.requested.zoom_log2.to_bits() {
            return Err(AppError::Math(
                "HOT zoom disagrees with the requested zoom".to_string(),
            ));
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
        let object = self.requested.object_angles;
        let plane = self.checked_plane;
        let map = self.screen_map(grid_extent)?;
        let displacement_scale = f64::from(grid_extent[0]) / f64::from(self.grid_width);
        let pose = Pose {
            epoch: state.epoch,
            orbit_generation: state.main.generation_applied,
            plane,
            object,
            plane_origin: self.requested.plane_origin,
            zoom_log2: state.hot.zoom_log2,
            view: self.requested.view,
            grid_width: grid_extent[0],
            grid_height: grid_extent[1],
            map,
            centre_from_reference_px: state
                .hot
                .centre_from_reference_px
                .map(|value| value * displacement_scale),
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
        let reference_centre = self
            .pending_reference_centre
            .take()
            .filter(|(generation, _)| *generation == navigation.generation)
            .map_or_else(|| navigation.centre.clone(), |(_, centre)| centre);
        let reason = self
            .pending_reason
            .take()
            .unwrap_or(OrbitReason::CENTRE_THRESHOLD);
        Some(ReferenceSubmission {
            navigation,
            reference_centre,
            reason,
        })
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
        let centre_f64 = centre.to_f64_mirror();
        self.owner
            .configure_navigation(NavigationConfig {
                centre,
                reference_centre,
                plane,
                grid_width: self.grid_width,
            })
            .map_err(owner_error)?;
        self.navigation_centre_f64 = centre_f64;
        Ok(())
    }

    /// Returns the cached finite navigation-centre mirror for allocation-free page facts.
    #[must_use]
    pub const fn navigation_centre_f64(&self) -> [f64; 4] {
        self.navigation_centre_f64
    }

    /// Builds the accepted neutral-height screen map for one refinement extent.
    ///
    /// # Errors
    ///
    /// Returns a typed math refusal for an invalid extent or degenerate accepted view.
    pub fn screen_map(&self, grid_extent: [u32; 2]) -> Result<PoseMap, AppError> {
        let object = self.requested.object_angles;
        let view = self.requested.view;
        let zoom_log2 = self.requested.zoom_log2;
        let key = ScreenMapKey::new(object, view, zoom_log2, grid_extent);
        {
            let mut cached = self.checked_screen_maps.borrow_mut();
            if let Some((index, hit)) =
                cached
                    .iter()
                    .copied()
                    .enumerate()
                    .find_map(|(index, cached)| {
                        cached
                            .filter(|candidate| candidate.key == key)
                            .map(|candidate| (index, candidate))
                    })
            {
                cached[index..].rotate_left(1);
                return Ok(hit.map);
            }
        }
        let map = map_for(object, view, zoom_log2, grid_extent)?;
        let mut cached = self.checked_screen_maps.borrow_mut();
        cached.rotate_left(1);
        cached[2] = Some(CheckedScreenMap { key, map });
        #[cfg(test)]
        self.map_constructions
            .set(self.map_constructions.get().saturating_add(1));
        Ok(map)
    }

    /// Returns the cached rasterized footprint for the current object, view, and extent.
    ///
    /// # Errors
    ///
    /// Returns a typed math refusal for an invalid extent, footprint, or map.
    pub fn scene_footprint(&self, grid_extent: [u32; 2]) -> Result<SceneFootprint, AppError> {
        let object = self.requested.object_angles;
        let view = self.requested.view;
        let key = SceneFootprintKey::new(object, view, grid_extent);
        {
            let mut cached = self.checked_scene_footprints.borrow_mut();
            if let Some((index, hit)) =
                cached
                    .iter()
                    .copied()
                    .enumerate()
                    .find_map(|(index, cached)| {
                        cached
                            .filter(|candidate| candidate.key == key)
                            .map(|candidate| (index, candidate))
                    })
            {
                cached[index..].rotate_left(1);
                return Ok(hit.footprint);
            }
        }
        let footprint =
            scene_footprint(&object, &view, grid_extent[0], grid_extent[1]).map_err(math_error)?;
        let mut cached = self.checked_scene_footprints.borrow_mut();
        cached.rotate_left(1);
        cached[2] = Some(CheckedSceneFootprint { key, footprint });
        #[cfg(test)]
        self.footprint_constructions
            .set(self.footprint_constructions.get().saturating_add(1));
        Ok(footprint)
    }

    /// Builds the coarse backdrop map from the cached main homography at the same extent.
    ///
    /// The cache owns only the neutral-height presented-camera rows. The returned copy carries the
    /// footprint's raster-selected apron, so backdrop use cannot evict an otherwise identical main
    /// map.
    ///
    /// # Errors
    ///
    /// Returns a typed math refusal for an invalid extent, footprint, or map.
    pub fn backdrop_map(&self, grid_extent: [u32; 2]) -> Result<Option<PoseMap>, AppError> {
        let footprint = self.scene_footprint(grid_extent)?;
        if footprint.apron_scale.to_bits() == 1.0_f64.to_bits() {
            return Ok(None);
        }
        match self.screen_map(grid_extent)? {
            PoseMap::Mapped(mut map) => {
                map.apron_scale = footprint.apron_scale;
                Ok(Some(PoseMap::Mapped(map)))
            }
            PoseMap::EdgeOn => Ok(None),
        }
    }

    #[cfg(test)]
    pub(crate) const fn map_construction_count(&self) -> u64 {
        self.map_constructions.get()
    }

    #[cfg(test)]
    pub(crate) const fn footprint_construction_count(&self) -> u64 {
        self.footprint_constructions.get()
    }

    #[cfg(test)]
    pub(crate) const fn main_state_rebuild_count(&self) -> u64 {
        self.main_state_rebuilds
    }

    fn mapped_screen_map(&self, grid_extent: [u32; 2]) -> Result<Homography, AppError> {
        match self.screen_map(grid_extent)? {
            PoseMap::Mapped(map) => Ok(map),
            PoseMap::EdgeOn => Err(AppError::Math(
                "navigation is undefined for an edge-on plane".to_string(),
            )),
        }
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

    fn refresh_navigation_centre_mirror(&mut self) {
        if let Some(centre) = self.owner.navigation_centre() {
            self.navigation_centre_f64 = centre.to_f64_mirror();
        }
    }

    const fn note_requested_change(&mut self) {
        self.requested_revision = self.requested_revision.saturating_add(1);
    }
}

fn f64_bits_eq<const N: usize>(first: [f64; N], second: [f64; N]) -> bool {
    first
        .into_iter()
        .zip(second)
        .all(|(first, second)| first.to_bits() == second.to_bits())
}

fn origins_share_slice(
    first: [f64; 4],
    second: [f64; 4],
    plane: Plane,
    zoom_log2: f64,
    grid_width: u32,
) -> bool {
    let delta: [f64; 4] = core::array::from_fn(|axis| second[axis] - first[axis]);
    let projection = [
        dot_plane_axis(plane.basis_u, delta),
        dot_plane_axis(plane.basis_v, delta),
    ];
    let residual: [f64; 4] = core::array::from_fn(|axis| {
        delta[axis]
            - f64::from(plane.basis_u[axis]).mul_add(
                projection[0],
                f64::from(plane.basis_v[axis]) * projection[1],
            )
    });
    let pixels_per_chart = 0.25 * f64::from(grid_width) * zoom_log2.exp2();
    residual
        .into_iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        * pixels_per_chart
        <= 0.5
}

fn dot_plane_axis(axis: [f32; 4], vector: [f64; 4]) -> f64 {
    axis.into_iter()
        .zip(vector)
        .fold(0.0, |sum, (axis, value)| {
            f64::from(axis).mul_add(value, sum)
        })
}

fn map_for(
    object: ObjectAngles,
    view: ViewControls,
    zoom_log2: f64,
    grid_extent: [u32; 2],
) -> Result<PoseMap, AppError> {
    let [width, height] = grid_extent;
    let aspect = f64::from(width) / f64::from(height);
    match screen_to_plane(&object, &view, zoom_log2, width, height, aspect) {
        Ok(map) => Ok(PoseMap::Mapped(map)),
        Err(MathError::DegenerateViewMap) => Ok(PoseMap::EdgeOn),
        Err(error) => Err(math_error(error)),
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
    use ember_julibrot_math::{
        ObjectAngles, PlaneAngles, PoseMap, PrecisionMode, ViewControls, pixel_scale,
    };
    use ember_julibrot_present::PaletteId;
    use ember_julibrot_worker::OrbitReason;

    use super::{
        BOX_CLICK_THRESHOLD_PX, NavigationEdit, PRESET_ROWS, SCALE_RANGE_LOG2,
        SEAHORSE_VALLEY_TARGET, ViewerController, anchor_px_up, box_zoom_delta_log2,
        css_from_anchor_px_up, drag_delta_px_down, is_box_selection, preset_row,
    };

    /// The reference browser geometry: a 960x540 render grid laid out at this client rectangle.
    const REFERENCE_RECT: [f64; 2] = [1_022.793_762_207_031_2, 575.315_673_828_125];
    const REFERENCE_GRID: [u32; 2] = [960, 540];

    fn set_close_owner_row(viewer: &mut ViewerController) {
        viewer
            .set_object_angles(ObjectAngles {
                rho_13: -1.316_653_720_171_549_4,
                rho_24: -1.316_653_720_171_549_4,
                ..ObjectAngles::IDENTITY
            })
            .expect("owner object angles");
        let mut camera = [0.0; 10];
        camera[1] = -0.254_142_606_623_347_1;
        camera[4] = -0.254_142_606_623_347_1;
        viewer
            .set_view_controls(ViewControls {
                height_scale: 4.0,
                distance_five: 2.0,
                distance_four: 2.0,
                camera_yaw: 0.960_422_302_787_256,
                camera_pitch: core::f64::consts::PI,
                camera,
                camera_translation: [0.0; 5],
            })
            .expect("owner relief view");
    }

    fn object_angles_from_array(values: [f64; 6]) -> ObjectAngles {
        let [rho_12, rho_13, rho_14, rho_23, rho_24, rho_34] = values;
        ObjectAngles {
            rho_12,
            rho_13,
            rho_14,
            rho_23,
            rho_24,
            rho_34,
        }
    }

    fn view_controls_from_array(values: [f64; 20]) -> ViewControls {
        let [
            c12,
            c13,
            c14,
            c23,
            c24,
            c34,
            c15,
            c25,
            c35,
            c45,
            t1,
            t2,
            t3,
            t4,
            t5,
            camera_yaw,
            camera_pitch,
            height_scale,
            distance_five,
            distance_four,
        ] = values;
        ViewControls {
            camera: [c12, c13, c14, c23, c24, c34, c15, c25, c35, c45],
            camera_translation: [t1, t2, t3, t4, t5],
            camera_yaw,
            camera_pitch,
            height_scale,
            distance_five,
            distance_four,
        }
    }

    /// The screen-to-slice conversion and the slice-to-screen one are each other's inverse.
    ///
    /// The crosshair is drawn from the projection and the point is stored from the conversion, so
    /// a discrepancy between the two would be a marker that sits beside the feature it names — the
    /// exact complaint this pair of functions exists to answer.
    #[test]
    fn the_pointer_conversion_and_its_projection_round_trip() {
        for point in [
            [0.0, 0.0],
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 2.0],
            [REFERENCE_RECT[0], REFERENCE_RECT[1]],
            [17.5, 431.25],
            [1_002.5, 3.75],
        ] {
            let anchor =
                anchor_px_up(point, REFERENCE_RECT, REFERENCE_GRID).expect("a finite anchor");
            let back = css_from_anchor_px_up(anchor, REFERENCE_RECT, REFERENCE_GRID)
                .expect("a finite projection");
            assert!((back[0] - point[0]).abs() < 1.0e-9, "x round trip");
            assert!((back[1] - point[1]).abs() < 1.0e-9, "y round trip");
        }
        // The centre of the client rectangle is the origin of the slice offset, and the y axis is
        // flipped exactly once on the way through.
        let centre = anchor_px_up(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 2.0],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("a finite anchor");
        assert!(centre[0].abs() < 1.0e-12 && centre[1].abs() < 1.0e-12);
        let above = anchor_px_up(
            [REFERENCE_RECT[0] / 2.0, REFERENCE_RECT[1] / 2.0 - 10.0],
            REFERENCE_RECT,
            REFERENCE_GRID,
        )
        .expect("a finite anchor");
        assert!(above[1] > 0.0, "a pointer above the centre is +y up");
        assert!(css_from_anchor_px_up([f64::NAN, 0.0], REFERENCE_RECT, REFERENCE_GRID).is_err());
        assert!(css_from_anchor_px_up([0.0, 0.0], [0.0, 1.0], REFERENCE_GRID).is_err());
    }

    /// A click stores a point on the slice and moves nothing.
    #[test]
    fn a_click_stores_a_point_and_leaves_the_picture_where_it_was() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let before_zoom = viewer.requested().zoom_log2;
        let before_centre = viewer
            .owner()
            .navigation_centre()
            .expect("configured navigation")
            .to_f64_mirror();
        assert_eq!(
            viewer
                .crosshair_centre_f64()
                .map(|point| point.map(f64::to_bits)),
            Some(SEAHORSE_VALLEY_TARGET.map(f64::to_bits))
        );
        assert!(viewer.crosshair_plane_px().is_some());
        viewer.set_crosshair([120.0, -45.0]).expect("finite click");
        let after_centre = viewer
            .owner()
            .navigation_centre()
            .expect("configured navigation")
            .to_f64_mirror();
        assert!((viewer.requested().zoom_log2 - before_zoom).abs() < f64::EPSILON);
        assert_eq!(
            before_centre, after_centre,
            "a click is not a navigation edit"
        );
        let drawn = viewer
            .crosshair_plane_px()
            .expect("a stored point projects");
        assert!((drawn[0] - 120.0).abs() < 1.0e-6);
        assert!((drawn[1] + 45.0).abs() < 1.0e-6);
        assert!(viewer.crosshair_precision_bits().is_some());
        assert_ne!(
            viewer
                .crosshair_centre_f64()
                .map(|point| point.map(f64::to_bits)),
            Some(SEAHORSE_VALLEY_TARGET.map(f64::to_bits))
        );
        viewer.clear_crosshair();
        assert_eq!(viewer.crosshair_plane_px(), None);
    }

    /// Zooming about the crosshair leaves the point under it, which is the accuracy oracle.
    #[test]
    fn a_zoom_about_the_crosshair_leaves_the_point_under_it() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        viewer.set_crosshair([200.0, 130.0]).expect("finite click");
        for step in [3.0, 7.5, -2.25] {
            viewer.zoom_about_crosshair(step).expect("finite zoom");
            let drawn = viewer
                .crosshair_plane_px()
                .expect("a stored point projects");
            assert!(
                (drawn[0] - 200.0).abs() < 1.0e-6 && (drawn[1] - 130.0).abs() < 1.0e-6,
                "the crosshair moved to {drawn:?} after a zoom of {step}"
            );
        }
        // With no point stored the anchor is the screen centre, which is the old behaviour.
        let mut plain = ViewerController::new(960).expect("canonical viewer");
        plain.clear_crosshair();
        let before = plain
            .owner()
            .navigation_centre()
            .expect("configured navigation")
            .to_f64_mirror();
        plain.zoom_about_crosshair(4.0).expect("finite zoom");
        let after = plain
            .owner()
            .navigation_centre()
            .expect("configured navigation")
            .to_f64_mirror();
        assert_eq!(before, after, "a centred zoom does not move the centre");
    }

    /// A translation moves the picture and the crosshair together, because the point does not move.
    #[test]
    fn a_translation_carries_the_crosshair_with_its_feature() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        viewer.set_crosshair([60.0, -20.0]).expect("finite click");
        let before = viewer
            .crosshair_plane_px()
            .expect("a stored point projects");
        let delta = drag_delta_px_down([40.0, 25.0], REFERENCE_RECT, REFERENCE_GRID)
            .expect("a finite drag");
        viewer.pan_px(delta).expect("finite pan");
        let after = viewer
            .crosshair_plane_px()
            .expect("a stored point projects");
        // A drag right and down carries the content with it: `+x` on screen, `-y` up.
        assert!((after[0] - before[0] - delta[0]).abs() < 1.0e-6);
        assert!((after[1] - before[1] + delta[1]).abs() < 1.0e-6);
    }

    #[test]
    fn crosshair_survives_an_in_plane_object_turn_and_clears_on_a_tilt() {
        let mut viewer = ViewerController::new(960).expect("canonical viewer");
        let initial = viewer
            .take_reference_submission()
            .expect("the initial slice requests a reference");
        assert!(viewer.finish_reference_submission(initial.navigation.generation));
        viewer.set_crosshair([60.0, -20.0]).expect("finite click");
        assert!(viewer.crosshair_plane_px().is_some());
        let point = viewer
            .crosshair_centre_f64()
            .expect("stored crosshair has an ambient point");
        let mut object = viewer.requested().object_angles;
        object.rho_34 = 0.3;
        viewer
            .set_object_angles(object)
            .expect("valid in-plane object turn");
        let centre = viewer
            .owner()
            .navigation_centre()
            .expect("navigation has a centre");
        let plane = viewer
            .owner()
            .navigation_plane()
            .expect("navigation has a plane");
        let scale = pixel_scale(viewer.requested().zoom_log2, 960).expect("valid scale");
        let rotated_chart = viewer
            .crosshair
            .as_ref()
            .expect("crosshair survives the in-plane turn")
            .displacement_px(&centre, &plane, scale)
            .expect("stored point has new chart coordinates");
        assert!((rotated_chart[0] - 51.409_785_214_309_565).abs() <= 1.0e-3);
        assert!((rotated_chart[1] + 36.837_942_182_192_49).abs() <= 1.0e-3);
        let projected = viewer
            .crosshair_plane_px()
            .expect("the stored feature projects after an in-plane turn");
        assert!((projected[0] - 60.0).abs() <= 1.0e-3);
        assert!((projected[1] + 20.0).abs() <= 1.0e-3);
        assert_eq!(viewer.crosshair_centre_f64(), Some(point));
        assert_eq!(
            viewer.owner().navigation_plane(),
            Some(viewer.checked_plane())
        );
        assert!(viewer.take_reference_submission().is_none());

        let mut tilted = viewer.requested().object_angles;
        tilted.rho_13 += 0.3;
        viewer.set_object_angles(tilted).expect("valid slice tilt");
        assert!(viewer.crosshair_plane_px().is_none());
        assert!(viewer.take_reference_submission().is_some());
    }

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

    /// A mapped click, pan, and zoom preserve the stored feature and its projected anchor.
    #[test]
    fn tilted_click_pan_and_zoom_keep_the_crosshair_on_its_feature() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        let mut view = ViewControls::MANDELBROT_FLAT;
        view.camera[0] = 0.13;
        view.camera[8] = -0.21;
        view.camera_translation = [0.2, -0.1, 0.3, -0.2, 0.15];
        view.camera_yaw = 0.17;
        view.camera_pitch = -0.12;
        viewer.set_view_controls(view).expect("tilted view");
        viewer.set_crosshair([120.0, -45.0]).expect("mapped click");
        let feature = viewer.crosshair_centre_f64().expect("stored feature facts");
        let clicked = viewer
            .crosshair_plane_px()
            .expect("clicked feature projects");
        assert!((clicked[0] - 120.0).abs() < 1.0e-9);
        assert!((clicked[1] + 45.0).abs() < 1.0e-9);

        let delta = drag_delta_px_down([40.0, 25.0], REFERENCE_RECT, REFERENCE_GRID)
            .expect("a finite drag");
        viewer.pan_px(delta).expect("mapped pan");
        let panned = viewer
            .crosshair_plane_px()
            .expect("panned feature projects");
        viewer.zoom_about_crosshair(3.0).expect("anchored zoom");
        let zoomed = viewer
            .crosshair_plane_px()
            .expect("zoomed feature projects");
        assert!((zoomed[0] - panned[0]).abs() < 1.0e-8);
        assert!((zoomed[1] - panned[1]).abs() < 1.0e-8);
        assert_eq!(viewer.crosshair_centre_f64(), Some(feature));
    }

    /// The scale control is absolute, refuses its own ends, and zooms about the stored target.
    #[test]
    fn the_scale_control_is_absolute_and_bounded() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        let target = viewer
            .crosshair_plane_px()
            .expect("the fresh viewer has its target");
        viewer
            .set_zoom_log2(12.5)
            .expect("a scale inside the range");
        let reached = viewer.requested().zoom_log2;
        assert!(
            (reached - 12.5).abs() <= f64::EPSILON * 12.5,
            "scale reached {reached} rather than 12.5"
        );
        let anchored = viewer
            .crosshair_plane_px()
            .expect("the target remains projected");
        assert!(
            anchored
                .iter()
                .zip(target)
                .all(|(after, before)| (*after - before).abs() < 1.0e-6),
            "the scale moved its target from {target:?} to {anchored:?}"
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
        assert_eq!(hot.pose.view, ViewControls::MANDELBROT_FLAT);
    }

    #[test]
    fn unchanged_refreshes_reuse_one_checked_plane() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        let initial_plane = viewer.checked_plane;
        assert_eq!(viewer.plane_constructions, 1);

        for _ in 0..120 {
            let frame = viewer.drain_hot([960, 540]).expect("unchanged refresh");
            assert_eq!(frame.plane, initial_plane);
        }
        assert_eq!(viewer.plane_constructions, 1);

        let mut angles = viewer.requested().object_angles;
        angles.rho_13 = 0.25;
        viewer.set_object_angles(angles).expect("changed angles");
        let changed_plane = viewer.drain_hot([960, 540]).expect("changed refresh").plane;
        assert_ne!(changed_plane, initial_plane);
        assert_eq!(viewer.plane_constructions, 2);

        viewer.set_object_angles(angles).expect("unchanged angles");
        viewer.drain_hot([960, 540]).expect("unchanged refresh");
        assert_eq!(viewer.plane_constructions, 2);
    }

    #[test]
    fn viewer_refresh_reuses_the_checked_map_until_one_key_field_changes() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        assert_eq!(viewer.map_constructions.get(), 0);

        for _ in 0..120 {
            let frame = viewer.drain_hot(REFERENCE_GRID).expect("unchanged refresh");
            assert_eq!(
                viewer.screen_map(REFERENCE_GRID).expect("refresh stamp"),
                frame.pose.map
            );
        }
        assert_eq!(
            viewer.map_constructions.get(),
            1,
            "120 HOT maps and 120 refresh stamps share one checked construction"
        );

        let mut view = viewer.requested().view;
        view.camera_yaw += 0.125;
        viewer
            .set_view_controls(view)
            .expect("changed VIEW controls");
        viewer.drain_hot(REFERENCE_GRID).expect("changed VIEW map");
        assert_eq!(viewer.map_constructions.get(), 2);

        viewer.wheel_zoom(1.0, [0.0; 2]).expect("changed zoom map");
        viewer
            .drain_hot(REFERENCE_GRID)
            .expect("changed zoom frame");
        assert_eq!(viewer.map_constructions.get(), 3);

        let mut object = viewer.requested().object_angles;
        object.rho_13 = 0.125;
        viewer.set_object_angles(object).expect("changed object");
        viewer
            .drain_hot(REFERENCE_GRID)
            .expect("changed object map");
        assert_eq!(viewer.map_constructions.get(), 4);

        viewer.drain_hot([800, 600]).expect("changed extent map");
        assert_eq!(viewer.map_constructions.get(), 5);

        let mut signed_zero_view = viewer.requested().view;
        signed_zero_view.camera_pitch = -0.0;
        viewer
            .set_view_controls(signed_zero_view)
            .expect("signed-zero VIEW controls");
        viewer.drain_hot([800, 600]).expect("signed-zero VIEW map");
        assert_eq!(viewer.map_constructions.get(), 6);
    }

    #[test]
    fn screen_map_cache_retains_three_extents_and_promotes_hits() {
        let viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        let extents = [REFERENCE_GRID, [480, 270], [120, 68], [800, 600]];

        for extent in extents[..3].iter().copied() {
            viewer.screen_map(extent).expect("initial map");
        }
        assert_eq!(viewer.map_construction_count(), 3);

        viewer.screen_map(extents[0]).expect("promoted hot map");
        viewer.screen_map(extents[3]).expect("fourth map");
        viewer.screen_map(extents[0]).expect("retained hot map");
        assert_eq!(viewer.map_construction_count(), 4);

        viewer.screen_map(extents[1]).expect("evicted cold map");
        assert_eq!(viewer.map_construction_count(), 5);
    }

    #[test]
    fn footprint_cache_is_bit_exact_on_object_view_and_extent() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        set_close_owner_row(&mut viewer);
        assert_eq!(viewer.footprint_construction_count(), 0);
        let first = viewer
            .scene_footprint(REFERENCE_GRID)
            .expect("close footprint");
        assert_eq!(viewer.footprint_construction_count(), 1);
        for _ in 0..8 {
            assert_eq!(
                viewer
                    .scene_footprint(REFERENCE_GRID)
                    .expect("cached footprint"),
                first
            );
            viewer
                .backdrop_map(REFERENCE_GRID)
                .expect("cached backdrop");
        }
        assert_eq!(viewer.footprint_construction_count(), 1);

        viewer
            .wheel_zoom(1.0, [0.0; 2])
            .expect("zoom is outside the footprint key");
        assert_eq!(
            viewer
                .scene_footprint(REFERENCE_GRID)
                .expect("zoom-invariant footprint"),
            first
        );
        assert_eq!(viewer.footprint_construction_count(), 1);

        let mut view = viewer.requested().view;
        view.height_scale = 3.5;
        viewer.set_view_controls(view).expect("changed height");
        viewer
            .scene_footprint(REFERENCE_GRID)
            .expect("changed-view footprint");
        assert_eq!(viewer.footprint_construction_count(), 2);
        viewer
            .scene_footprint([800, 600])
            .expect("changed-extent footprint");
        assert_eq!(viewer.footprint_construction_count(), 3);

        let mut signed_zero = viewer.requested().view;
        signed_zero.camera_translation[4] = -0.0;
        viewer
            .set_view_controls(signed_zero)
            .expect("signed-zero view");
        viewer
            .scene_footprint([800, 600])
            .expect("bit-distinct footprint key");
        assert_eq!(viewer.footprint_construction_count(), 4);
    }

    #[test]
    fn footprint_cache_retains_the_three_refresh_extents() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        set_close_owner_row(&mut viewer);
        let refresh_extents = [REFERENCE_GRID, [480, 270], [120, 68]];

        for extent in refresh_extents {
            viewer.scene_footprint(extent).expect("refresh footprint");
        }
        assert_eq!(viewer.footprint_construction_count(), 3);

        for _ in 0..8 {
            for extent in refresh_extents {
                viewer
                    .scene_footprint(extent)
                    .expect("retained refresh footprint");
            }
        }
        assert_eq!(viewer.footprint_construction_count(), 3);

        viewer
            .scene_footprint(refresh_extents[0])
            .expect("promoted hot footprint");
        viewer
            .scene_footprint([800, 600])
            .expect("fourth footprint");
        viewer
            .scene_footprint(refresh_extents[0])
            .expect("retained hot footprint");
        assert_eq!(viewer.footprint_construction_count(), 4);

        viewer
            .scene_footprint(refresh_extents[1])
            .expect("evicted cold footprint");
        assert_eq!(viewer.footprint_construction_count(), 5);
    }

    #[test]
    fn each_object_and_view_scalar_requires_a_footprint_recompute() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        set_close_owner_row(&mut viewer);
        let base_object = viewer.requested().object_angles;
        let base_view = viewer.requested().view;
        viewer
            .scene_footprint(REFERENCE_GRID)
            .expect("base footprint");
        assert_eq!(viewer.footprint_construction_count(), 1);

        for scalar in 0..26 {
            viewer
                .set_object_angles(base_object)
                .expect("restore base object");
            viewer
                .set_view_controls(base_view)
                .expect("restore base view");
            viewer
                .scene_footprint(REFERENCE_GRID)
                .expect("cached base footprint");
            let before = viewer.footprint_construction_count();

            if scalar < 6 {
                let mut values = base_object.as_array();
                values[scalar] -= 1.0e-6;
                viewer
                    .set_object_angles(object_angles_from_array(values))
                    .expect("changed object scalar");
            } else {
                let mut values = base_view.as_array();
                values[scalar - 6] -= 1.0e-6;
                viewer
                    .set_view_controls(view_controls_from_array(values))
                    .expect("changed view scalar");
            }
            viewer
                .scene_footprint(REFERENCE_GRID)
                .expect("changed-scalar footprint");
            assert_eq!(
                viewer.footprint_construction_count(),
                before + 1,
                "scalar {scalar} reused the base cache entry"
            );
        }
    }

    #[test]
    fn backdrop_map_copies_the_cached_main_rows_and_adds_only_its_apron() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        set_close_owner_row(&mut viewer);
        let PoseMap::Mapped(main) = viewer.screen_map(REFERENCE_GRID).expect("main map") else {
            panic!("owner row is mapped");
        };
        assert_eq!(main.apron_scale.to_bits(), 1.0_f64.to_bits());
        assert_eq!(viewer.map_construction_count(), 1);
        let Some(PoseMap::Mapped(backdrop)) =
            viewer.backdrop_map(REFERENCE_GRID).expect("backdrop map")
        else {
            panic!("close owner row requests a backdrop");
        };
        assert_eq!(backdrop.rows, main.rows);
        assert_eq!(backdrop.inverse, main.inverse);
        assert_eq!(backdrop.condition_number, main.condition_number);
        assert_eq!(backdrop.apron_scale.to_bits(), 2.0_f64.to_bits());
        assert_eq!(viewer.footprint_construction_count(), 1);
        assert_eq!(
            viewer
                .scene_footprint(REFERENCE_GRID)
                .expect("cached footprint")
                .apron_scale
                .to_bits(),
            2.0_f64.to_bits()
        );
        assert_eq!(viewer.footprint_construction_count(), 1);
        assert_eq!(
            viewer.map_construction_count(),
            1,
            "the backdrop copy reuses the extent-keyed main cache slot"
        );
    }

    #[test]
    fn second_owner_row_requests_the_smallest_qualifying_backdrop() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        set_close_owner_row(&mut viewer);
        let Some(PoseMap::Mapped(backdrop)) =
            viewer.backdrop_map(REFERENCE_GRID).expect("backdrop map")
        else {
            panic!("owner row requests a backdrop");
        };
        assert_eq!(backdrop.apron_scale.to_bits(), 2.0_f64.to_bits());
        let PoseMap::Mapped(main) = viewer.screen_map(REFERENCE_GRID).expect("main map") else {
            panic!("owner row is mapped");
        };
        assert_eq!(main.apron_scale.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn first_owner_row_needs_no_backdrop_at_positive_height() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        viewer
            .set_object_angles(ObjectAngles::JULIA)
            .expect("Julia object angles");
        viewer
            .set_view_controls(ViewControls {
                height_scale: 2.165,
                distance_five: 8.0,
                ..ViewControls::NEUTRAL
            })
            .expect("owner relief view");
        assert_eq!(
            viewer
                .backdrop_map(REFERENCE_GRID)
                .expect("owner footprint"),
            None
        );
    }

    #[test]
    fn flat_view_never_constructs_a_distinct_backdrop_map() {
        let viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        let main = viewer.screen_map(REFERENCE_GRID).expect("flat main map");
        assert_eq!(
            viewer.backdrop_map(REFERENCE_GRID).expect("flat footprint"),
            None
        );
        assert_eq!(
            viewer.screen_map(REFERENCE_GRID).expect("cached flat map"),
            main
        );
        assert_eq!(viewer.map_construction_count(), 1);
    }

    #[test]
    fn shipped_presets_refuse_sub_threshold_backdrops() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        let backdrop_extent = [REFERENCE_GRID[0] / 2, REFERENCE_GRID[1] / 2];
        for row in PRESET_ROWS {
            viewer.apply_preset(row).expect("relief preset is valid");
            let footprint = viewer
                .scene_footprint(backdrop_extent)
                .expect("preset footprint");
            assert_eq!(
                footprint.apron_scale.to_bits(),
                1.0_f64.to_bits(),
                "{} footprint",
                row.name
            );
            assert!(
                footprint.uncovered_fraction < 40.0 / 3969.0,
                "{} has a policy-significant uncovered share of {}",
                row.name,
                footprint.uncovered_fraction
            );
            assert!(
                viewer
                    .backdrop_map(backdrop_extent)
                    .expect("preset backdrop")
                    .is_none(),
                "{} backdrop",
                row.name
            );
        }
    }

    #[test]
    fn drained_hot_pose_zoom_matches_the_requested_zoom() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        viewer
            .wheel_zoom(3.25, [11.0, -7.0])
            .expect("finite zoom edit");
        let frame = viewer.drain_hot(REFERENCE_GRID).expect("zoomed HOT frame");
        assert_eq!(
            frame.pose.zoom_log2.to_bits(),
            viewer.requested().zoom_log2.to_bits()
        );
    }

    #[test]
    #[ignore = "measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports construction counts"
    )]
    fn measures_viewer_refresh_map_constructions_before_after() {
        let mut viewer = ViewerController::new(REFERENCE_GRID).expect("canonical viewer");
        let preview_extent = [120, 68];
        for _ in 0..120 {
            viewer.drain_hot(preview_extent).expect("unchanged HOT map");
            viewer
                .screen_map(REFERENCE_GRID)
                .expect("unchanged stamp map");
        }
        let before_constructions = 240;
        let after_constructions = viewer.map_constructions.get();
        assert_eq!(after_constructions, 2);
        eprintln!(
            "PF-L5 viewer_refreshes=120 before_constructions={before_constructions} after_constructions={after_constructions}"
        );
    }

    #[test]
    fn cached_navigation_mirror_matches_the_owner_after_edits_and_acceptance() {
        let mut viewer = ViewerController::new([960, 540]).expect("canonical viewer");
        assert_eq!(
            viewer.navigation_centre_f64(),
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .to_f64_mirror()
        );

        viewer
            .wheel_zoom(1.0, [37.0, -19.0])
            .expect("finite anchored zoom");
        viewer.drag_pan([8.0, -5.0]).expect("finite pan");
        assert_eq!(
            viewer.navigation_centre_f64(),
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .to_f64_mirror()
        );

        let accepted = viewer
            .owner()
            .navigation_centre()
            .expect("configured centre");
        let plane = viewer.checked_plane;
        viewer
            .configure_navigation_context(accepted.clone(), accepted, plane)
            .expect("accepted context");
        assert_eq!(
            viewer.navigation_centre_f64(),
            viewer
                .owner()
                .navigation_centre()
                .expect("configured centre")
                .to_f64_mirror()
        );
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
        assert_eq!(
            viewer.requested().precision_mode,
            PrecisionMode::PictureFast
        );
        assert_eq!(viewer.requested().palette, PaletteId::Ice);
        assert_eq!(viewer.requested().view, relief);
        assert_eq!(second.main.requested_iter_cap, 2_048);
        assert_eq!(
            second.main.precision_mode,
            PrecisionMode::PictureFast as u32
        );
        assert_eq!(second.main.palette_id, PaletteId::Ice as u32);
    }

    #[test]
    fn precision_mode_defaults_fast_and_changes_main_generation() {
        let mut viewer = ViewerController::new(800).expect("canonical viewer");
        assert_eq!(
            viewer.requested().precision_mode,
            PrecisionMode::PictureFast
        );
        let before = viewer.drain_hot([800, 600]).expect("initial frame");
        viewer
            .set_precision_mode(PrecisionMode::Deterministic)
            .expect("mode change");
        let after = viewer.drain_main().expect("mode main");
        assert!(after.main.centre_revision > before.state.main.centre_revision);
        assert_eq!(
            after.main.precision_mode,
            PrecisionMode::Deterministic as u32
        );
        let submission = viewer
            .take_reference_submission()
            .expect("mode change requests a reference");
        assert_eq!(submission.navigation.precision_mode, 0);
        assert_ne!(
            submission.reason.bits() & OrbitReason::PRECISION_MODE_CHANGE.bits(),
            0
        );
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
            assert_eq!(requested.object_angles, row.object_angles);
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

    #[test]
    fn preset_rows_are_exact_identity_maps_and_neutral_mandelbrot_is_edge_on() {
        let mut viewer = ViewerController::new([640, 480]).expect("canonical viewer");
        for row in [PRESET_ROWS[0], PRESET_ROWS[1]] {
            viewer.apply_preset(row).expect("preset is valid");
            assert_eq!(
                viewer.screen_map([640, 480]).expect("preset map"),
                PoseMap::Mapped(ember_julibrot_math::Homography::IDENTITY)
            );
        }
        viewer
            .set_object_angles(ObjectAngles::IDENTITY)
            .expect("identity object");
        viewer
            .set_view_controls(ViewControls::NEUTRAL)
            .expect("neutral camera is physical edge-on");
        assert_eq!(
            viewer.screen_map([640, 480]).expect("edge-on is a state"),
            PoseMap::EdgeOn
        );
        for row in [PRESET_ROWS[2], PRESET_ROWS[3]] {
            assert_eq!(row.view.camera[0], 0.6);
            assert_eq!(row.view.camera[8], 0.97);
        }
    }
}
