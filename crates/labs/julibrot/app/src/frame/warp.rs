/// Keeps the retained DATA grid intact until its relief redraw reaches the surface.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn defer_scene_until_relief_redraw(relief_redraw: bool, view_stale: bool) -> bool {
    relief_redraw && view_stale
}

/// Leaves the last redraw visible while Final overwrites its retained records.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn hold_redraw_during_scene(relief_redraw: bool, scene_in_flight: bool) -> bool {
    relief_redraw && scene_in_flight
}

/// Treats a redraw that deferred its replacement scene as required presentation work.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) const fn warp_submission_due(warp_requested: bool, scene_deferred: bool) -> bool {
    warp_requested || scene_deferred
}
