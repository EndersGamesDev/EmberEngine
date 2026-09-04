/// Keeps the retained DATA grid intact until its relief redraw reaches the surface.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn defer_scene_until_relief_redraw(relief_redraw: bool, view_stale: bool) -> bool {
    relief_redraw && view_stale
}

/// Holds the last exact redraw while Final overwrites DATA and fills its own retained image.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn hold_redraw_during_scene(relief_redraw: bool, scene_in_flight: bool) -> bool {
    relief_redraw && scene_in_flight
}
