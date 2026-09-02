//! Native source-contract checks for browser facts that need visible replay to observe.

const INDEX: &str = include_str!("../../../../../web/labs/julibrot/index.html");
const MAIN: &str = include_str!("../../../../../web/labs/julibrot/main.js");
const WORKER: &str = include_str!("../../../../../web/labs/julibrot/worker.js");
const LIB: &str = include_str!("../src/lib.rs");
const RUNTIME: &str = include_str!("../src/runtime.rs");
const FACTS: &str = include_str!("../src/facts.rs");
const MEASUREMENT: &str = include_str!("../src/measurement.rs");
const FRAME: &str = include_str!("../src/frame.rs");
const WORKER_BROWSER: &str = include_str!("../../worker/src/browser.rs");
const WORKER_OWNER: &str = include_str!("../../worker/src/browser_owner.rs");

#[test]
fn loader_worker_and_wasm_share_version_one_before_orbit_transfer() {
    for required in [
        "./main.js?v=1",
        "./style.css?v=1",
        "const ABI = 1",
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
    assert_eq!(WORKER.matches("ember_lab_julibrot.js?v=1").count(), 1);
    assert!(FRAME.contains("WorkerChannel::new("));
    assert!(FRAME.contains("WorkerMode::WebWorker"));
    assert!(WORKER_OWNER.contains("const WORKER_URL: &str = \"./worker.js?v=1\""));
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

#[test]
fn page_has_one_canvas_status_overlay_and_every_requested_control() {
    assert_eq!(INDEX.matches("<canvas").count(), 1);
    assert_eq!(INDEX.matches("id=\"status\"").count(), 1);
    assert_eq!(INDEX.matches("id=\"facts-grid\"").count(), 1);
    for id in [
        "view",
        "preset",
        "julia-re",
        "julia-im",
        "theta-1",
        "theta-2",
        "iteration-cap",
        "palette",
        "one-frame",
        "measure",
    ] {
        assert!(
            INDEX.contains(&format!("id=\"{id}\"")),
            "missing control {id}"
        );
        assert!(MAIN.contains(&format!("\"{id}\"")), "unbound control {id}");
    }
    assert!(MAIN.contains("event.preventDefault()"));
    assert!(MAIN.contains("bounds.height / 2 -"));
    assert!(!MAIN.contains("SharedArrayBuffer"));
}

#[test]
fn page_facts_carry_every_contract_field_without_fake_aggregate_counts() {
    let fields = [
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
        "view_mode",
        "completed_scene_id",
        "in_flight_scene_id",
        "warp_source_scene_id",
        "reprojected_per_scene",
        "refreshes_without_scene",
        "chart_residual",
        "tumbled_max_error_px",
        "tumbled_p95_error_px",
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
    ];
    for field in fields {
        assert!(
            FACTS.contains(&format!("pub {field}:")),
            "missing PageFacts.{field}"
        );
    }
    assert_eq!(FACTS.matches("\"unavailable\"").count(), 3);
    assert!(FACTS.contains("\"requires visible replay\""));
    assert!(INDEX.contains("approximately 4.5 MB"));
    assert!(MAIN.contains("wasm_bundle_bytes"));
    assert!(MAIN.contains("javascript_bundle_bytes"));
    assert!(MAIN.contains("wasm_instance_count = 2"));
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
        .find("presenter.poll(now_ms)")
        .expect("opening poll");
    let drain = refresh.find("viewer.drain_hot").expect("HOT drain");
    let write = refresh.find("presenter.write_hot").expect("HOT write");
    let arrivals = refresh.find("service_arrivals").expect("worker arrivals");
    let kernels = refresh.find("submit_due_scene").expect("kernel scene");
    let surface = refresh.find("acquire_for_warp").expect("surface acquire");
    let warp = refresh.find("presenter.frame").expect("warp submit");
    assert!(poll < drain && drain < write && write < arrivals);
    assert!(arrivals < kernels && kernels < surface && surface < warp);
    assert!(FRAME.contains("KernelMode::for_zoom"));
    assert!(FRAME.contains("self.presenter.poll(now_ms)"));
    assert!(MAIN.contains("requestAnimationFrame"));
    assert!(FRAME.contains("runtime.complete_warp"));
}
