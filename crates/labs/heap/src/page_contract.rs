//! Native source-level guards for the self-contained browser page contract.

const PAGE: &str = include_str!("../../../../web/labs/heap/index.html");
const RUNTIME: &str = include_str!("lattice_gpu.rs");

#[test]
fn loader_is_v5_and_runtime_is_explicitly_gl_only() {
    assert!(PAGE.contains("ember_lab_heap.js?v=5"));
    assert!(PAGE.contains("ember_lab_heap_bg.wasm?v=5"));
    assert!(PAGE.contains("heap-lattice-v5"));
    assert!(!PAGE.contains("heap-lattice-v4"));
    assert!(RUNTIME.contains("backends: wgpu::Backends::GL"));
    assert!(RUNTIME.contains("info.backend != wgpu::Backend::Gl"));
}

#[test]
fn selection_uses_two_labelled_frames_and_only_the_second_decides_animation() {
    for required in [
        "first fenced warm-up",
        "second fenced decision",
        "first.normalized_ms",
        "second.normalized_ms <= ANIMATE_THRESHOLD_MS",
        "selectionWarmup = { first, second, animates }",
    ] {
        assert!(PAGE.contains(required), "missing page contract: {required}");
    }
    assert!(!PAGE.contains("setAnimation(measured.normalized_ms <= ANIMATE_THRESHOLD_MS)"));
}

#[test]
fn equality_gate_precedes_timing_and_reports_both_oracles() {
    for required in [
        "conform_heap_lattice_json",
        "GPU record samples and 64×36 image",
        "numeric ${numeric.pass ? \"PASS\" : \"FAIL\"}",
        "image ${image.pass ? \"PASS\" : \"FAIL\"}",
        "Mode C/layer timing is disqualified until the live equality gate passes",
        "${numeric.sampled_indices.length} deterministic indices",
        "${integer(image.compared_pixels)} pixels",
    ] {
        assert!(PAGE.contains(required), "missing page contract: {required}");
    }
}

#[test]
fn honesty_measurement_and_byte_laws_remain_visible() {
    for required in [
        "select_heap_lattice_json(selectedMode, selectedStep, policy)",
        "rendered immediately",
        "cancelled by a newer selection",
        "TARGET_QUANTA = 32",
        "SAMPLE_COUNT = 15",
        "Math.ceil(ordered.length * 0.95) - 1",
        "per_frame_cpu_to_gpu_bytes",
        "requires visible replay",
    ] {
        assert!(PAGE.contains(required), "missing page contract: {required}");
    }
    assert!(RUNTIME.contains("per_frame_cpu_to_gpu_bytes: 192"));
    assert!(RUNTIME.contains("borrowed.device.poll(wgpu::Maintain::Poll)"));
    assert!(RUNTIME.contains("COMPLETION_DEADLINE_MS"));
}
