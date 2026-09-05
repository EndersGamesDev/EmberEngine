//! Native source-contract checks for browser facts that need visible replay to observe.

use ember_lab_julibrot::{SavedView, preset_row};

const INDEX: &str = include_str!("../../../../../web/labs/julibrot/index.html");
const MAIN: &str = include_str!("../../../../../web/labs/julibrot/main.js");
const STYLE: &str = include_str!("../../../../../web/labs/julibrot/style.css");
const WORKER: &str = include_str!("../../../../../web/labs/julibrot/worker.js");
const MANIFEST: &str = include_str!("../Cargo.toml");
const LIB: &str = include_str!("../src/lib.rs");
const RUNTIME: &str = include_str!("../src/runtime.rs");
const STATE: &str = include_str!("../src/state.rs");
const FACTS: &str = include_str!("../src/facts.rs");
const MEASUREMENT: &str = include_str!("../src/measurement.rs");
static FRAME: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let mut source = String::from(include_str!("../src/frame/schedule.rs"));
    source.push_str(include_str!("../src/frame/loop.rs"));
    source.push_str(include_str!("../src/frame/loop/browser/backdrop.rs"));
    source.push_str(include_str!("../src/frame/loop/browser/facts.rs"));
    source.push_str(include_str!("../src/frame/loop/browser/reference.rs"));
    source.push_str(include_str!("../src/frame/loop/browser/submit.rs"));
    source
});
const WORKER_BROWSER: &str = include_str!("../../worker/src/browser.rs");
const WORKER_OWNER: &str = include_str!("../../worker/src/browser_owner.rs");
const SAVED: &str = include_str!("../src/saved.rs");
const WIRE: &str = include_str!("../../worker/src/wire.rs");

/// Every field the page facts must carry, in publication order.
const PAGE_FACT_FIELDS: [&str; 119] = [
    "abi_version",
    "adapter_name",
    "backend",
    "rgba32f_renderable",
    "requested_width",
    "requested_height",
    "delivered_width",
    "delivered_height",
    "requested_iteration_cap",
    "delivered_iteration_cap",
    "requested_zoom_log2",
    "presented_zoom_log2",
    "reference_zoom_log2",
    "zoom_digits_f64",
    "depth_digits",
    "precision_floor_digits",
    "precision_working_digits",
    "requested_precision_bits",
    "delivered_precision_bits",
    "orbit_length",
    "orbit_generation",
    "owner_epoch",
    "centre_revision",
    "centre_from_reference_px",
    "reference_shift_px",
    "scale_mantissa",
    "scale_exponent",
    "kernel_mode",
    "refinement_level",
    "refinement_pending",
    "scene_mode",
    "scene_update_pending",
    "draft_skipped_count",
    "sampled_reference_requests",
    "sampled_reference_rounds",
    "sampled_reference_discards",
    "last_draft_skip_reason",
    "extent_divisor",
    "active_pixels",
    "worst_case_pixel_iterations",
    "kernel_page_passes",
    "scratch_copy_commands",
    "scratch_copy_bytes",
    "logical_heap_bytes",
    "reserved_heap_bytes",
    "scratch_bytes",
    "hot_write_bytes",
    "scene_uniform_write_bytes",
    "texture_reallocations",
    "rebase_count_sum",
    "rebase_count_max",
    "glitch_pixel_count",
    "worker_facts",
    "worker_compute_us",
    "worker_credit_us",
    "worker_overfeed_us",
    "worker_allocation_events",
    "request_buffers_owned_main",
    "orbit_buffers_owned_main",
    "palette_id",
    "object_angles",
    "plane_theta_1",
    "plane_theta_2",
    "plane_origin",
    "target_plane",
    "view_theta_1",
    "view_theta_2",
    "camera_angles",
    "camera_translation",
    "camera_yaw",
    "camera_pitch",
    "height_scale",
    "distance_five",
    "distance_four",
    "surface_uncovered_fraction",
    "scene_apron_scale",
    "scene_backdrop_scale",
    "scene_backdrop_extent",
    "relief_clipped_fraction",
    "horizon_pixels",
    "horizon_fraction",
    "uncertain_pixels",
    "uncertain_fraction",
    "edge_on",
    "map_condition_number",
    "completed_scene_id",
    "in_flight_scene_id",
    "warp_source_scene_id",
    "reprojected_per_scene",
    "relief_redraw_count",
    "warp_hold_count",
    "refreshes_without_scene",
    "chart_residual",
    "warp_max_error_px",
    "warp_p95_error_px",
    "warp_exposed_fraction",
    "warp_kind",
    "scene_wall_ms",
    "scene_fence_wait_ms",
    "scene_polls",
    "warp_wall_ms",
    "warp_fence_wait_ms",
    "warp_polls",
    "warmup_label",
    "second_frame_policy",
    "timer_quantum_ms",
    "device_walls",
    "app_policies",
    "limiting_term",
    "wasm_bundle_bytes",
    "javascript_bundle_bytes",
    "wasm_instance_count",
    "timing_status",
    "precision_mode",
    "scene_precision_mode",
    "warp_precision_mode",
    "level_timings",
    "draft_pixels_discarded",
    "draft_iterations_discarded",
];

#[test]
fn loader_version_one_and_abi_three_are_pinned_before_orbit_transfer() {
    for required in [
        "./main.js?v=1",
        "./style.css?v=1",
        "const ABI = 3",
        "./worker.js?v=1",
        "./pkg/ember_lab_julibrot.js?v=1",
        "./pkg/ember_lab_julibrot_bg.wasm?v=1",
        "VersionSkew",
    ] {
        assert!(
            INDEX.contains(required)
                || MAIN.contains(required)
                || WORKER.contains(required)
                || WORKER_OWNER.contains(required),
            "missing version contract: {required}"
        );
    }
    assert!(!LIB.contains("pub fn worker_main(expected_abi: u32)"));
    assert!(WORKER_BROWSER.contains("pub fn worker_main(expected_abi: u32)"));
    assert!(MAIN.contains("const ABI = 3;"));
    assert!(WORKER.contains("const ABI = 3;"));
    assert!(WIRE.contains("pub const JULIBROT_ABI_VERSION: u32 = 3;"));
    assert!(MANIFEST.contains("name = \"ember_lab_julibrot\""));
    assert_eq!(WORKER.matches("ember_lab_julibrot.js?v=1").count(), 1);
    assert_eq!(MAIN.matches("./worker.js?v=1").count(), 1);
    let worker_url = MAIN
        .find("globalThis.JULIBROT_WORKER_URL = \"./worker.js?v=1\"")
        .expect("page publishes the worker URL");
    let module_import = MAIN
        .find("import(\"./pkg/ember_lab_julibrot.js")
        .expect("page imports the main wasm module");
    assert!(worker_url < module_import);
    assert!(FRAME.contains("WorkerChannel::new("));
    assert!(FRAME.contains("WorkerMode::WebWorker"));
    assert!(WORKER_OWNER.contains("const WORKER_URL_GLOBAL: &str = \"JULIBROT_WORKER_URL\""));
    assert!(WORKER_OWNER.contains("const WORKER_URL_FALLBACK: &str = \"./worker.js?v=1\""));
}
#[test]
fn runtime_is_gl_only_and_handlers_precede_the_first_post_device_work() {
    for required in [
        "backends: wgpu::Backends::GL",
        "info.backend != wgpu::Backend::Gl",
        "Limits::downlevel_webgl2_defaults()",
        "EXT_color_buffer_float",
        "TextureFormat::Rgba32Float",
        "TextureUsages::RENDER_ATTACHMENT",
        "TextureUsages::COPY_SRC",
        "TextureUsages::COPY_DST",
        "TextureUsages::TEXTURE_BINDING",
    ] {
        assert!(RUNTIME.contains(required), "missing GL floor: {required}");
    }
    assert!(!RUNTIME.contains("Backends::BROWSER_WEBGPU"));
    let startup = RUNTIME
        .split_once("pub async fn start")
        .expect("runtime startup exists")
        .1;
    let hook = startup
        .find("install_julibrot_panic_hook")
        .expect("panic hook");
    let instance = startup
        .find("wgpu::Instance::new")
        .expect("first wgpu call");
    assert!(hook < instance);
    let after_request = startup
        .split_once(".request_device(")
        .expect("device request exists")
        .1;
    let lost = after_request
        .find("set_device_lost_callback")
        .expect("lost handler");
    let uncaptured = after_request
        .find("install_logging_handler")
        .expect("error handler");
    let scope = after_request
        .find("ValidationScope::begin")
        .expect("init scope");
    assert!(lost < scope && uncaptured < scope);
}
#[test]
fn acquire_path_is_non_panicking_and_initial_frame_is_only_clear_plus_text() {
    let acquire = RUNTIME
        .split_once("fn acquire_surface_texture")
        .expect("acquire helper exists")
        .1
        .split_once("fn canvas_by_id")
        .expect("acquire helper boundary exists")
        .0;
    assert!(!acquire.contains(".unwrap()"));
    assert!(!acquire.contains(".expect("));
    for required in [
        "waiting for first completed scene",
        "Julibrot honest initial clear",
        "frame.present();",
        "SurfaceError::Lost | wgpu::SurfaceError::Outdated",
        "SurfaceError::Timeout",
    ] {
        assert!(INDEX.contains(required) || RUNTIME.contains(required));
    }
    assert!(!INDEX.contains("diagnostic pattern"));
}

/// The page is one viewport-filling stage, and the picture is one of its regions.
///
/// Each region is named, because the arrangement is the feature: the transition bar above the
/// view, a view box in each side column, the render surface between them, the zoom bar below, and
/// every remaining control in a strip that scrolls inside its own row. A layout that quietly went
/// back to one tall column would still pass every control assertion below and would still be the
/// bug this lane was opened for.
#[test]
fn the_page_is_one_stage_with_the_picture_between_the_two_view_boxes() {
    for region in [
        "id=\"masthead\"",
        "id=\"morph-bar\"",
        "id=\"box-a\"",
        "id=\"stage-view\"",
        "id=\"box-b\"",
        "id=\"zoom-bar\"",
        "id=\"dash\"",
        "id=\"facts-panel\"",
    ] {
        assert!(INDEX.contains(region), "missing layout region {region}");
    }
    // Order on the page is the layout: A, then the picture, then B.
    let box_a = INDEX.find("id=\"box-a\"").expect("view box A");
    let view = INDEX.find("id=\"stage-view\"").expect("the render surface");
    let box_b = INDEX.find("id=\"box-b\"").expect("view box B");
    assert!(box_a < view && view < box_b);
    // The transition bar is above the view and the zoom bar below it.
    let morph = INDEX.find("id=\"morph-bar\"").expect("the transition bar");
    let zoom = INDEX.find("id=\"zoom-bar\"").expect("the zoom bar");
    assert!(morph < view && view < zoom);
    // The canvas keeps its delivered grid; only CSS scales it.
    assert!(INDEX.contains(r#"<canvas id="julibrot" width="960" height="540">"#));
    // The stylesheet sizes the viewer to the canvas rather than letting it stretch, because the
    // marker and the rubber band are positioned in the viewer's own coordinates.
    assert!(STYLE.contains("aspect-ratio: 16 / 9"));
    assert!(STYLE.contains("100dvh"));
    assert!(
        !STYLE.contains(".viewer { position: relative; aspect-ratio: 16 / 9; border"),
        "a border on the viewer would take two pixels out of the pointer conversion's aspect"
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive page surface keeps every control under the same contract"
)]
fn page_has_one_canvas_status_overlay_and_every_requested_control() {
    assert_eq!(INDEX.matches("<canvas").count(), 1);
    assert_eq!(INDEX.matches("id=\"status\"").count(), 1);
    assert_eq!(INDEX.matches("id=\"facts-grid\"").count(), 1);
    // The top-level preset selector is retired: its rows moved into the two view boxes.
    assert!(!INDEX.contains("id=\"preset\""));
    // One control per view degree of freedom plus the one precision-policy switch.
    for id in [
        "o12",
        "o13",
        "o14",
        "o23",
        "o24",
        "o34",
        "origin-z-re",
        "origin-z-im",
        "origin-c-re",
        "origin-c-im",
        "q12",
        "q13",
        "q14",
        "q23",
        "q24",
        "q34",
        "q15",
        "q25",
        "q35",
        "q45",
        "t1",
        "t2",
        "t3",
        "t4",
        "t5",
        "camera-yaw",
        "camera-pitch",
        "height",
        "distance-five",
        "distance-four",
        "iteration-cap",
        "precision",
        "palette",
        "auto-scene",
        "update-scene",
        "one-frame",
        "measure",
    ] {
        assert!(
            INDEX.contains(&format!("id=\"{id}\"")),
            "missing control {id}"
        );
        assert!(MAIN.contains(&format!("\"{id}\"")), "unbound control {id}");
    }
    // Each origin slider carries a number box on the same value, bound through one template.
    for id in [
        "origin-z-re-number",
        "origin-z-im-number",
        "origin-c-re-number",
        "origin-c-im-number",
    ] {
        assert!(
            INDEX.contains(&format!("id=\"{id}\"")),
            "missing origin number box {id}"
        );
    }
    // Substrings chosen to avoid a brace pair, which clippy reads as a stray format argument.
    assert!(
        MAIN.contains("-number`, NUMBER(id));"),
        "unpaired number box"
    );
    assert!(MAIN.contains("SET(id, NUMBER(`"), "unpaired origin slider");
    // A row writes the elements and crosses the app boundary as one transaction. The built-in
    // rows are read from the app until it runs out of them, so the page never names one.
    assert!(MAIN.contains("row = JSON.parse(api.app_preset(id));"));
    assert!(MAIN.contains("api.app_apply_saved_view(JSON.stringify(row));"));
    // Every field of a row reaches its control by name, so a row that grows a field reaches a
    // slider named after it with no edit to the loader. This is the whole of the mapping.
    assert!(
        MAIN.contains(
            r#"const ROW_CONTROL_ALIAS = { height_scale: "height", zoom_log2: "scale" };"#
        )
    );
    assert!(MAIN.contains(r#"field.replaceAll("_", "-")"#));
    assert!(MAIN.contains("for (const [field, value] of Object.entries(row)) {"));
    let preset =
        SavedView::from_preset(preset_row(0).expect("first preset")).expect("serializable preset");
    let preset_text = preset.to_page_json().expect("flat page JSON");
    let preset_json: serde_json::Value = serde_json::from_str(&preset_text).expect("JSON object");
    for field in [
        "o12", "o13", "o14", "o23", "o24", "o34", "q12", "q13", "q14", "q23", "q24", "q34", "q15",
        "q25", "q35", "q45", "t1", "t2", "t3", "t4", "t5",
    ] {
        assert!(
            preset_json.get(field).is_some(),
            "row JSON omits affine field {field}"
        );
    }
    for aggregate in ["object", "camera", "camera_translation"] {
        assert!(
            preset_json.get(aggregate).is_none(),
            "page JSON retained aggregate field {aggregate}"
        );
    }
    assert_eq!(
        SavedView::from_page_json(&preset_text).expect("page JSON round trip"),
        preset
    );
    assert_eq!(MAIN.matches("api.app_set_object_angles(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_plane_origin(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_camera_angles(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_camera_translation(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_camera(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_height(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_distances(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_scale(").count(), 1);
    assert_eq!(MAIN.matches("api.app_set_precision_mode(").count(), 1);
    assert!(INDEX.contains(
        r#"<label>precision<select id="precision"><option value="0">Deterministic</option><option value="1" selected>PictureFast</option></select></label>"#
    ));
    assert!(MAIN.contains("event.preventDefault()"));
    assert!(MAIN.contains("bounds.width, bounds.height"));
    assert!(!MAIN.contains("SharedArrayBuffer"));
}

/// Nothing retired is still named, and every entry the page calls is still exported.
///
/// The two halves are one subject from opposite sides: a control the page dropped must not survive
/// in the markup or the boundary, and an entry the loader calls must exist to be called. Split out
/// of the control roster above because that test had grown past the length at which a failure
/// tells you which of two unrelated things broke.
/// A row can be lifted off the page exactly, so a broken state travels as the row that broke.
///
/// The first line is the row's own JSON and the second is the summary the box shows. Nothing here
/// names a field: the row is stringified whole, so a row that grows an angle is reported complete
/// without an edit, which is the only way a copy button stays honest as the row changes.
#[test]
fn a_row_can_be_copied_exactly_with_its_summary_beside_it() {
    for id in ["copy-a", "copy-b", "copy-current", "copy-fallback"] {
        assert!(
            INDEX.contains(&format!("id=\"{id}\"")),
            "missing copy control {id}"
        );
    }
    // The exact row first, the sentence second, and the row is never picked apart to build it.
    assert!(MAIN.contains(
        "const rowText = (label, row) => `${JSON.stringify(row)}\\n${describeRow(label, row)}`;"
    ));
    // The live row comes from the one capture the page already had, not a second path to the app.
    assert!(MAIN.contains(r#"copyText("current", rowText("current", captureRow()))"#));
    assert_eq!(MAIN.matches("api.app_saved_view_json()").count(), 1);
    // The clipboard is a permission: an absent or refusing one falls back to a selected textarea.
    assert!(MAIN.contains(r#"typeof navigator.clipboard.writeText !== "function""#));
    assert!(MAIN.contains(r#"say(target, "row copied");"#));
    assert!(MAIN.contains("() => offerSelection(target, text),"));
    assert!(MAIN.contains("FALLBACK.select();"));
    assert!(INDEX.contains("readonly"));
}

#[test]
fn the_retired_controls_name_nothing_and_the_wasm_boundary_is_complete() {
    // The retired mode selector and its Julia-only number boxes must name nothing.
    for retired in [
        "id=\"view\"",
        "id=\"julia-re\"",
        "id=\"julia-im\"",
        "id=\"preset\"",
        "Tumbled",
        "Flat",
    ] {
        assert!(!INDEX.contains(retired), "the page still carries {retired}");
    }
    assert!(!MAIN.contains("app_set_view("));
    assert!(!MAIN.contains("app_set_preset("));
    assert!(!LIB.contains("pub fn app_set_view("));
    assert!(!LIB.contains("pub fn app_set_preset("));
    for required in [
        "pub fn app_set_object_angles(",
        "pub fn app_set_plane_origin(",
        "pub fn app_set_camera_angles(",
        "pub fn app_set_camera_translation(",
        "pub fn app_set_camera(",
        "pub fn app_set_height(",
        "pub fn app_set_distances(",
        "pub fn app_set_scale(",
        "pub fn app_set_target(",
        "pub fn app_pan_px(",
        "pub fn app_zoom_box(",
        "pub fn app_clear_crosshair(",
        "pub fn app_saved_view_json(",
        "pub fn app_apply_saved_view(",
        "pub fn app_set_centre(",
        "pub fn app_morph_view(",
        "pub fn app_set_precision_mode(",
        "pub fn app_set_scene_mode(",
        "pub fn app_update_scene(",
        "pub fn app_preset(",
    ] {
        assert!(LIB.contains(required), "missing wasm entry {required}");
    }
    // Presets are pure data in one place.
    assert!(STATE.contains("pub const PRESET_ROWS: [PresetRow; 4] = ["));
    assert_eq!(STATE.matches("PresetRow {").count(), 5);
    assert!(STATE.contains("object_angles: ObjectAngles::IDENTITY"));
    assert!(STATE.contains("object_angles: ObjectAngles::JULIA"));
    assert!(STATE.contains("view: ViewControls::MANDELBROT_FLAT"));
    assert!(STATE.contains("view: ViewControls::NEUTRAL"));
}

#[test]
fn the_view_does_not_respond_to_the_wheel_and_says_so_nowhere() {
    // The mechanism is retired, not the word: the page is expected to say the wheel does nothing,
    // and the entry points that made it do something are the things that must be gone.
    for retired in ["app_wheel_zoom", "app_drag_pan", "deltaY"] {
        assert!(!MAIN.contains(retired), "the page still carries {retired}");
        assert!(!LIB.contains(retired), "the app still carries {retired}");
    }
    assert!(!MAIN.contains("addEventListener(\"wheel\""));
    assert!(!LIB.contains("pub fn app_wheel_zoom("));
    assert!(!LIB.contains("pub fn app_drag_pan("));
    // And the page says so, so a reader does not go looking for a handler that is not there.
    assert!(INDEX.contains("There is no wheel."));
}

#[test]
fn the_canvas_navigates_by_crosshair_translation_box_and_scale() {
    // One pointer gesture reaches one entry: the page reports the rectangle it drew and the Rust
    // boundary decides whether that was a box or a click, so the two cannot drift apart here.
    assert!(MAIN.contains(
        "api.app_zoom_box(started.x, started.y, to[0], to[1], bounds.width, bounds.height)"
    ));
    assert_eq!(MAIN.matches("api.app_zoom_box(").count(), 1);
    assert!(MAIN.contains("event.shiftKey"));
    assert!(STATE.contains("pub fn is_box_selection("));
    assert!(STATE.contains("pub fn box_zoom_delta_log2("));
    assert!(STATE.contains("pub const BOX_CLICK_THRESHOLD_PX: f64 = 4.0;"));
    assert!(STATE.contains("pub const SCALE_RANGE_LOG2: [f64; 2] = [-2.0, 120.0];"));
    // The scale control spans exactly the range the app enforces.
    assert!(INDEX.contains("id=\"scale\" type=\"range\" min=\"-2\" max=\"120\""));
    for id in ["t1", "t2", "t3", "t4", "t5"] {
        assert!(INDEX.contains(&format!("id=\"{id}\" type=\"range\" min=\"-8\" max=\"8\"")));
    }
    // The marker and the rubber band are DOM overlays, not scene geometry.
    for element in ["id=\"target\"", "id=\"rubber\""] {
        assert!(INDEX.contains(element), "missing overlay {element}");
    }
    // Shift makes the box; a plain drag is a translation. The page says so in one line.
    assert!(INDEX.contains("shift and drag a box to fill the screen with it"));
    assert!(INDEX.contains("drag to translate"));
    assert!(MAIN.contains("box: event.shiftKey,"));
}

/// A click names a point on the slice and moves nothing; every zoom is taken about that point.
///
/// The three claims are separable and all three used to be false: a click recentred the picture,
/// the marker was drawn at a remembered pixel rather than re-projected, and the `scale` slider
/// zoomed about the screen centre whatever the marker said. Each is pinned here at the boundary it
/// crosses, because a page that keeps its own idea of where the crosshair is will always drift.
#[test]
fn a_click_names_a_point_and_every_zoom_is_taken_about_it() {
    // The click boundary stores the point and does not navigate.
    let set_target = LIB
        .split_once("pub fn app_set_target(")
        .expect("the click entry exists")
        .1
        .split_once("pub fn app_pan_px(")
        .expect("the click entry ends")
        .0;
    assert!(set_target.contains("set_crosshair(anchor)"));
    assert!(
        !set_target.contains("set_target("),
        "a click must not be a navigation edit"
    );
    // The conversion happens once, in the app, and the projection is its exact inverse.
    assert!(STATE.contains("pub fn css_from_anchor_px_up("));
    assert!(STATE.contains("pub fn set_crosshair(&mut self, anchor_px_up: [f64; 2])"));
    assert!(STATE.contains("pub fn crosshair_plane_px(&self) -> Option<[f64; 2]>"));
    assert!(STATE.contains("pub fn zoom_about_crosshair("));
    assert!(STATE.contains("pub fn pan_px("));
    // The slider goes through the crosshair anchor rather than the screen centre.
    assert!(STATE.contains("self.zoom_about_crosshair(zoom_log2 - self.requested.zoom_log2)"));
    // A row load clears the replaced picture's point before the atomic row restores its own.
    assert!(STATE.contains("pub fn clear_crosshair(&mut self)"));
    assert_eq!(MAIN.matches("api.app_clear_crosshair();").count(), 1);
    assert!(SAVED.contains("pub target: Option<SavedCentre>"));
    assert!(STATE.contains("self.crosshair = target;"));
    // The page draws the marker from the projection and never from a pixel it remembered.
    assert!(MAIN.contains("const drawCrosshair = () => {"));
    assert!(MAIN.contains("api.app_crosshair_json(bounds.width, bounds.height)"));
    assert!(MAIN.contains("TARGET.style.left = `${crosshair.crosshair_css[0]}px`;"));
    assert_eq!(MAIN.matches("TARGET.style.left").count(), 1);
    assert!(
        !MAIN.contains("const showTarget"),
        "the page must not place the marker itself"
    );
    // The projected position and the point behind it are published, so the oracle is a number.
    for fact in [
        "crosshair_present",
        "crosshair_plane_px",
        "crosshair_css",
        "crosshair_on_surface",
        "crosshair_precision_bits",
        "crosshair_point_f64",
    ] {
        assert!(
            LIB.contains(fact),
            "missing published crosshair fact {fact}"
        );
    }
}

#[test]
fn boot_draws_the_default_seahorse_valley_target() {
    assert!(STATE.contains("-0.743_643_887_037_151,"));
    assert!(STATE.contains("0.131_825_904_205_33]"));
    assert!(STATE.contains("crosshair: Some(target),"));
    assert!(MAIN.contains("const drawCrosshair = () => {"));
    assert!(MAIN.contains(
        "Object.assign(JSON.parse(api.app_facts_json()), BOOT_FACTS, pageFacts(), drawCrosshair())"
    ));
    assert!(MAIN.contains("TARGET.hidden = !crosshair.crosshair_on_surface;"));
}

#[test]
fn two_view_boxes_share_one_path_from_a_control_value_to_the_worker() {
    for element in [
        "id=\"save-a\"",
        "id=\"save-b\"",
        "id=\"load-a\"",
        "id=\"load-b\"",
        "id=\"readout-a\"",
        "id=\"readout-b\"",
        "id=\"rows-a\"",
        "id=\"rows-b\"",
        "id=\"name-a\"",
        "id=\"name-b\"",
        "id=\"delete-a\"",
        "id=\"delete-b\"",
        "id=\"message-a\"",
        "id=\"message-b\"",
        "id=\"morph\"",
    ] {
        assert!(
            INDEX.contains(element),
            "missing view-box element {element}"
        );
    }
    // The morph slider is inert until both boxes hold a view.
    assert!(INDEX.contains(
        "id=\"morph\" type=\"range\" min=\"0\" max=\"1\" value=\"0\" step=\"any\" disabled"
    ));
    // Loading and morphing both go through one row applier and one atomic app boundary.
    assert_eq!(MAIN.matches("const applyRow = row => {").count(), 1);
    // A morphed row carries the endpoints' precision: the working precision the arithmetic needs
    // is refused downstream, where a centre and its reference must agree bit for bit.
    assert!(SAVED.contains("round_centre("));
    assert!(
        SAVED.contains("let precision_bits = first.precision_bits.max(second.precision_bits);")
    );
    // One applier, reached by a preset row, a saved row, a load and a morph alike. The page no
    // longer replays nine independent setters and then the centre.
    assert_eq!(
        MAIN.matches("api.app_apply_saved_view(JSON.stringify(row));")
            .count(),
        1
    );
    assert!(!MAIN.contains("for (const apply of Object.values(APPLY)) apply();"));
    // Both sides list the same rows, a save needs a name, a taken name is refused, and only a
    // saved row can be deleted — the built-in rows come from the app and are never removable.
    assert!(MAIN.contains("const rowEntries = () => BUILT_IN.concat("));
    assert!(MAIN.contains(r#"say(box, "a saved row needs a name");"#));
    assert!(MAIN.contains("is already taken"));
    assert!(MAIN.contains(r#"say(box, "a built-in row cannot be deleted");"#));
    assert!(MAIN.contains(r#"const ROWS_KEY = "julibrot.rows";"#));
    assert!(MAIN.contains("ROWS.named.filter(entry => entry.name !== name)"));
    assert_eq!(MAIN.matches("api.app_set_centre(").count(), 0);
    assert_eq!(MAIN.matches("api.app_morph_view(").count(), 1);
    assert_eq!(MAIN.matches("api.app_saved_view_json()").count(), 1);
    // Every storage access is wrapped, and the page states what it could not reach.
    assert_eq!(MAIN.matches("localStorage.").count(), 2);
    assert_eq!(MAIN.matches("STORAGE_STATUS = `unavailable:").count(), 2);
    // `t` is page state: it is a fact and never a stored field of the row.
    assert!(MAIN.contains("morph_t: MORPH.disabled ? null : Number(MORPH.value)"));
    assert!(!SAVED.contains("pub morph_t"));
    assert!(!SAVED.contains("pub t:"));
    assert!(SAVED.contains("pub centre: SavedCentre,"));
    assert!(SAVED.contains("pub zoom_log2: f64,"));
    assert!(SAVED.contains("pub object: [f64; 6],"));
    assert!(SAVED.contains("pub camera: [f64; 10],"));
    assert!(SAVED.contains("pub camera_translation: [f64; 5],"));
    assert!(MAIN.contains("affine.every(field => Number.isFinite(row[field]))"));
}

#[test]
fn pointer_input_reaches_the_worker_in_render_grid_pixels() {
    // The loader hands over raw canvas-relative CSS pixels plus the client rectangle; centring, the
    // CSS-to-grid scale, and the y flip are the Rust boundary's work, so they stay testable.
    assert!(MAIN.contains("event.clientX - bounds.left, event.clientY - bounds.top"));
    assert!(STATE.contains("pub fn anchor_px_up("));
    assert!(STATE.contains("pub fn drag_delta_px_down("));
    assert!(STATE.contains("fn css_to_grid_scale("));
    assert_eq!(LIB.matches("let grid = app.grid_extent();").count(), 4);
    assert!(LIB.contains("pointer_css_x: f64"));
    assert!(LIB.contains("rect_css_height: f64"));
    assert!(LIB.contains("end_css_y_down: f64"));
    // The page owns no screen arithmetic at all any more: it neither halves the canvas nor scales
    // a pixel, so CSS scaling of the canvas cannot put the marker and the pointer in two places.
    assert!(!MAIN.contains("bounds.width / 2"));
    assert!(!MAIN.contains("bounds.height / 2"));
    assert!(!MAIN.contains("devicePixelRatio"));
}

#[test]
fn page_facts_carry_every_contract_field_without_fake_aggregate_counts() {
    let fields = PAGE_FACT_FIELDS;
    for field in fields {
        assert!(
            FACTS.contains(&format!("pub {field}:")),
            "missing PageFacts.{field}"
        );
    }
    assert_eq!(FACTS.matches("\"unavailable\"").count(), 2);
    assert!(FACTS.contains("glitch_pixel_count: present.glitch_pixel_count"));
    assert!(FACTS.contains("\"requires visible replay\""));
    assert!(INDEX.contains("approximately 4.5 MB"));
    assert!(MAIN.contains("wasm_bundle_bytes"));
    assert!(MAIN.contains("javascript_bundle_bytes"));
    assert!(MAIN.contains("wasm_instance_count: 2"));
    assert!(FACTS.contains("precision_mode: requested.precision_mode.as_str()"));
    assert!(
        FACTS.contains(
            "scene_precision_mode: present.last_scene.map(|sample| sample.precision_mode)"
        )
    );
    assert!(
        FACTS
            .contains("warp_precision_mode: present.last_warp.map(|sample| sample.precision_mode)")
    );
    assert!(FRAME.contains("precision_mode: self.main.precision_mode"));
    assert!(FACTS.contains("scene_mode: loop_facts.scene_mode().as_str()"));
    assert!(FACTS.contains("scene_update_pending: loop_facts.scene_update_pending()"));
    assert!(FACTS.contains("draft_skipped_count: loop_facts.draft_skipped_count()"));
    assert!(FACTS.contains("sampled_reference_rounds: loop_facts.sampled_reference_rounds()"));
    assert!(FACTS.contains("sampled_reference_discards: loop_facts.sampled_reference_discards()"));
    assert!(FACTS.contains("last_draft_skip_reason: loop_facts.last_draft_skip_reason()"));
    assert!(FACTS.contains("relief_redraw_count: present.relief_redraw_count"));
    assert!(FACTS.contains("warp_hold_count: present.warp_hold_count"));
    assert!(FACTS.contains("warp_exposed_fraction: present.warp_exposed_fraction"));
}

#[test]
fn immutable_boot_facts_are_merged_into_every_refresh() {
    assert!(MAIN.contains("let BOOT_FACTS = Object.freeze({});"));
    assert!(MAIN.contains("BOOT_FACTS = Object.freeze({"));
    assert!(MAIN.contains(
        "Object.assign(JSON.parse(api.app_facts_json()), BOOT_FACTS, pageFacts(), drawCrosshair())"
    ));
    for field in [
        "wasm_bundle_bytes:",
        "javascript_bundle_bytes:",
        "wasm_instance_count: 2",
        "timer_quantum_ms:",
        "timer_probe:",
    ] {
        assert!(MAIN.contains(field), "missing immutable boot fact {field}");
    }
}

#[test]
fn scene_update_controls_bind_the_checkbox_and_button_to_distinct_app_commands() {
    assert!(INDEX.contains(r#"<input id="auto-scene" type="checkbox" checked>"#));
    assert!(INDEX.contains(r#"<button id="update-scene" type="button">Update scene</button>"#));
    assert!(MAIN.contains("api.app_set_scene_mode(event.target.checked ? 1 : 0);"));
    assert!(MAIN.contains("api.app_update_scene();"));
    assert!(MAIN.contains("facts.scene_mode === \"manual\" && facts.scene_update_pending"));
    assert!(MAIN.contains("facts.warp_kind === \"HoldStale\""));
    assert!(MAIN.contains("warp refused, showing scene ${facts.completed_scene_id} unmoved"));
}

#[test]
fn pending_promotion_and_retained_hold_facts_are_explicit() {
    assert!(!FRAME.contains("PREVIOUS_PARTITION_HOLD_ENABLED"));
    assert!(!FRAME.contains("hold_refused_warp_with_partition_capability("));
    assert!(FRAME.contains("pub const SETTLED_DETERMINISTIC_PROMOTION_ENABLED: bool = false;"));
    assert!(FRAME.contains("pub struct SettledPromotion"));
    assert!(FRAME.contains("pub const fn presented_tier(&self) -> PresentedTier"));
    assert!(MAIN.contains("facts.held_since_scene_id !== null"));
    assert!(MAIN.contains("`held_frame_partition` will remain visible in the generic overlay"));
    assert!(MAIN.contains("held from previous partition while the replacement renders"));
}

#[test]
fn timing_contract_keeps_all_bounds_and_labels_visible() {
    for required in [
        "ADAPTIVE_WARM_UPS: u32 = 3",
        "ADAPTIVE_SAMPLES: usize = 15",
        "TARGET_TIMER_QUANTA: u32 = 32",
        "MAX_ADAPTIVE_REPEATS: u32 = 4_096",
        "MAX_BATCH_MS: f64 = 250.0",
        "SUITE_DEADLINE_MS: f64 = 30_000.0",
        "TIMER_READ_LIMIT: u32 = 4_000_000",
        "TIMER_TRANSITION_TARGET: u32 = 32",
        "TIMER_PROBE_DEADLINE_MS: f64 = 500.0",
        "CONTINUOUS_FRAME_THRESHOLD_MS: f64 = 100.0",
        "FrameObservation::WarmUp",
        "FrameObservation::Decision",
    ] {
        assert!(
            MEASUREMENT.contains(required),
            "missing measurement bound: {required}"
        );
    }
    assert!(MAIN.contains("performance.now()"));
    assert!(MAIN.contains("requires visible replay"));
}

#[test]
fn surface_images_are_keyed_by_warp_and_never_presented_by_the_state_model() {
    let surface = include_str!("../src/surface.rs");
    for required in [
        "pub warp_id: u64",
        "pub generation: u32",
        "SurfaceAction::Present",
        "SurfaceAction::Drop",
        "self.pending_warp_id() != Some(warp_id)",
    ] {
        assert!(
            surface.contains(required),
            "missing pending-surface law: {required}"
        );
    }
    assert!(!surface.contains(".present()"));
}

#[test]
fn frame_loop_preserves_cross_slice_order_and_cooperative_polling() {
    let refresh = FRAME
        .split_once("pub fn refresh(")
        .expect("frame refresh exists")
        .1
        .split_once("fn outcome(")
        .expect("refresh boundary exists")
        .0;
    let poll = refresh
        .find("FrameLoop::refresh(&mut self.presenter, now_ms)")
        .expect("opening poll");
    let drain = refresh.find("viewer.drain_hot").expect("HOT drain");
    let write = refresh.find("presenter.write_hot").expect("HOT write");
    let arrivals = refresh.find("service_arrivals").expect("worker arrivals");
    let kernels = refresh.find("submit_due_scene").expect("kernel scene");
    let surface = refresh.find("acquire_for_warp").expect("surface acquire");
    let warp = refresh.find("presenter.frame").expect("warp submit");
    assert!(poll < drain && drain < write && write < arrivals);
    assert!(arrivals < kernels && kernels < surface && surface < warp);
    assert_eq!(refresh.matches("FrameLoop::refresh").count(), 1);
    assert!(!refresh.contains("presenter.poll"));
    assert!(FRAME.contains("KernelMode::for_zoom"));
    assert!(FRAME.contains("presenter.poll_once(now_ms)"));
    assert!(MAIN.contains("requestAnimationFrame"));
    assert!(MAIN.contains("return api.app_needs_refresh();"));
    assert_eq!(
        MAIN.matches("if (stillTurning()) scheduleFrame();").count(),
        4,
        "the loader re-schedules from the completed turn, the caught throw, the wake-up, and automatic scene re-entry"
    );
    assert!(FRAME.contains("runtime.complete_warp"));
}

#[test]
fn the_frame_loop_cannot_latch_on_a_frame_that_never_ran() {
    // The flag is cleared on the way into the turn, not on the way out of a callback that a page
    // the browser has stopped painting for may never receive.
    assert!(MAIN.contains(
        "  const runFrame = (ticket, nowMs, viaFallback) => {\n    if (!RAF_PENDING || ticket !== FRAME_TICKET) return;\n    RAF_PENDING = false;\n"
    ));
    // One low-rate timer stands behind the animation callback, and it is the only timer on the
    // page: a second `setTimeout` would be a second clock rather than a floor under a stopped one.
    assert!(MAIN.contains("const FRAME_FALLBACK_MS = 250;"));
    assert_eq!(MAIN.matches("setTimeout(").count(), 1);
    assert_eq!(MAIN.matches("requestAnimationFrame(").count(), 1);
    assert!(MAIN.contains("requestAnimationFrame(nowMs => runFrame(ticket, nowMs, false));"));
    assert!(MAIN.contains("runFrame(ticket, performance.now(), true);"));
    // The timer runs a turn only when one is due and the callback did not arrive for this
    // schedule, so a healthy animation callback leaves `frames_from_fallback` at zero.
    assert!(MAIN.contains(
        "      if (!RAF_PENDING || ticket !== FRAME_TICKET) return;\n      if (stillTurning()) {"
    ));
    // Returning to a painting page retires the schedule rather than waiting on it.
    for required in [
        "const wakeFrameLoop = () => {",
        "document.addEventListener(\"visibilitychange\", () => { if (!document.hidden) wakeFrameLoop(); });",
        "window.addEventListener(\"pageshow\", wakeFrameLoop);",
        "window.addEventListener(\"focus\", wakeFrameLoop);",
    ] {
        assert!(MAIN.contains(required), "missing wake-up law: {required}");
    }
    // The counters are page facts, so which path drove a turn is a reported number rather than a
    // claim: schedules against turns says the loop is alive, and the split says which clock did it.
    for required in [
        "frame_schedules: FRAME_COUNTS.schedules,",
        "frames_from_raf: FRAME_COUNTS.raf,",
        "frames_from_fallback: FRAME_COUNTS.fallback,",
        "frame_latch_clears: FRAME_COUNTS.latch_clears,",
        "frame_loop_wakeups: FRAME_COUNTS.wakeups,",
    ] {
        assert!(
            MAIN.contains(required),
            "missing frame-loop counter: {required}"
        );
    }
}

#[test]
fn a_transient_fence_refusal_is_survived_rather_than_ending_the_page() {
    for required in [
        "const fn classify_refusal(reason: FenceRefusal) -> RefusalClass {",
        "FenceRefusal::PollLimit | FenceRefusal::Deadline => RefusalClass::Transient,",
        "FenceRefusal::Cancelled => RefusalClass::Cancelled,",
        "FenceRefusal::Device => RefusalClass::Device,",
        "fn record_transient(&mut self, error: AppError) {",
        "self.completed_run = !self.schedule.pending();",
        "view_stale: bool,",
        "|| view_stale",
        "self.loop_state.stop(error.clone());",
        "pub fn presented_view_is_stale(&self, viewer: &ViewerController) -> bool {",
    ] {
        assert!(
            FRAME.contains(required),
            "missing never-hang law: {required}"
        );
    }
    for required in [
        "pub refresh_status: &'static str,",
        "pub transient_fence_refusals: u32,",
        "pub last_transient_refusal: Option<String>,",
        "pub presented_view_stale: bool,",
        "pub loop_stopped_reason: Option<String>,",
    ] {
        assert!(
            FACTS.contains(required),
            "missing published fact: {required}"
        );
    }
    for required in [
        "if (facts.loop_stopped_reason) fail(facts.loop_stopped_reason);",
        "facts.transient_fence_refusals",
        "facts.last_transient_refusal",
    ] {
        assert!(MAIN.contains(required), "missing page display: {required}");
    }
    assert!(
        LIB.contains("if self.frame_loop.stopped_reason().is_some() {"),
        "a stopped loop must answer false to needs_refresh"
    );
}
