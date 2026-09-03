//! Honest requested, delivered, measured, unavailable, and replay-only page facts.

use ember_julibrot_kernels::{KernelMode, RefinementLevel};
use ember_julibrot_math::{precision_for, scaled_pixel_scale};
use ember_julibrot_present::{SampleClass, SubmissionMeasurement};
use serde::Serialize;

use crate::{App, FramePolicy, JULIBROT_ABI_VERSION, LevelTimingRecord};

/// Complete version-three overlay snapshot; absent delivered values serialize as `null`.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct PageFacts {
    pub abi_version: u32,
    pub adapter_name: String,
    pub backend: String,
    pub rgba32f_renderable: bool,
    pub requested_width: u32,
    pub requested_height: u32,
    pub delivered_width: Option<u32>,
    pub delivered_height: Option<u32>,
    pub requested_iteration_cap: u32,
    pub delivered_iteration_cap: Option<u32>,
    pub requested_zoom_log2: f64,
    pub presented_zoom_log2: Option<f64>,
    pub reference_zoom_log2: Option<f64>,
    pub zoom_digits_f64: f64,
    pub depth_digits: u32,
    pub precision_floor_digits: Option<u32>,
    pub precision_working_digits: Option<u32>,
    pub requested_precision_bits: Option<u32>,
    pub delivered_precision_bits: Option<u32>,
    pub orbit_length: Option<u32>,
    pub orbit_generation: Option<u32>,
    pub owner_epoch: u64,
    pub centre_revision: u32,
    pub centre_from_reference_px: [f64; 2],
    pub reference_shift_px: [f64; 2],
    pub scale_mantissa: Option<f32>,
    pub scale_exponent: Option<i32>,
    pub kernel_mode: Option<String>,
    pub refinement_level: Option<String>,
    pub refinement_pending: bool,
    pub scene_mode: &'static str,
    pub scene_update_pending: bool,
    pub draft_skipped_count: u64,
    pub last_draft_skip_reason: Option<&'static str>,
    pub extent_divisor: Option<u32>,
    pub active_pixels: Option<u32>,
    pub worst_case_pixel_iterations: Option<u64>,
    pub kernel_page_passes: Option<u32>,
    pub scratch_copy_commands: Option<u32>,
    pub scratch_copy_bytes: Option<u64>,
    pub logical_heap_bytes: Option<u64>,
    pub reserved_heap_bytes: Option<u64>,
    pub scratch_bytes: Option<u64>,
    pub hot_write_bytes: Option<u32>,
    pub scene_uniform_write_bytes: Option<u32>,
    pub texture_reallocations: Option<u32>,
    pub rebase_count_sum: &'static str,
    pub rebase_count_max: &'static str,
    pub glitch_pixel_count: &'static str,
    pub worker_facts: Option<serde_json::Value>,
    pub worker_compute_us: Option<u32>,
    pub worker_credit_us: Option<u32>,
    pub worker_overfeed_us: Option<u32>,
    pub worker_allocation_events: Option<u32>,
    pub request_buffers_owned_main: Option<u32>,
    pub orbit_buffers_owned_main: Option<u32>,
    pub worker_request_depth: u32,
    pub outstanding_reference_count: u32,
    pub outstanding_reference_generation: Option<u32>,
    pub navigation_pending_depth: u32,
    pub refresh_status: &'static str,
    pub transient_fence_refusals: u32,
    pub last_transient_refusal: Option<String>,
    pub presented_view_stale: bool,
    pub loop_stopped_reason: Option<String>,
    pub palette_id: u32,
    pub object_angles: [f64; 6],
    pub plane_theta_1: f64,
    pub plane_theta_2: f64,
    pub plane_origin: [f64; 4],
    pub target_plane: [f64; 4],
    pub view_theta_1: f64,
    pub view_theta_2: f64,
    pub camera_angles: [f64; 10],
    pub camera_translation: [f64; 5],
    pub camera_yaw: f64,
    pub camera_pitch: f64,
    pub height_scale: f64,
    pub distance_five: f64,
    pub distance_four: f64,
    pub horizon_pixels: u64,
    pub horizon_fraction: f64,
    pub uncertain_pixels: u64,
    pub uncertain_fraction: f64,
    pub edge_on: bool,
    pub map_condition_number: f64,
    pub completed_scene_id: Option<u64>,
    pub in_flight_scene_id: Option<u64>,
    pub warp_source_scene_id: Option<u64>,
    pub reprojected_per_scene: Option<u32>,
    pub refreshes_without_scene: u64,
    pub chart_residual: Option<f64>,
    pub warp_max_error_px: Option<f64>,
    pub warp_p95_error_px: Option<f64>,
    pub warp_exposed_fraction: Option<f64>,
    pub warp_kind: String,
    pub scene_wall_ms: Option<f64>,
    pub scene_fence_wait_ms: Option<f64>,
    pub scene_polls: Option<u32>,
    pub warp_wall_ms: Option<f64>,
    pub warp_fence_wait_ms: Option<f64>,
    pub warp_polls: Option<u32>,
    pub warmup_label: Option<String>,
    pub second_frame_policy: Option<String>,
    pub timer_quantum_ms: Option<f64>,
    pub device_walls: Vec<String>,
    pub app_policies: Vec<String>,
    pub limiting_term: Option<String>,
    pub wasm_bundle_bytes: Option<u64>,
    pub javascript_bundle_bytes: Option<u64>,
    pub wasm_instance_count: u32,
    pub timing_status: &'static str,
    pub precision_mode: &'static str,
    pub scene_precision_mode: Option<&'static str>,
    pub warp_precision_mode: Option<&'static str>,
    pub level_timings: Vec<LevelTimingRecord>,
    pub draft_pixels_discarded: Option<u32>,
    pub draft_iterations_discarded: Option<u64>,
}

impl PageFacts {
    /// Builds a snapshot without inventing any delivered or browser measurement.
    #[must_use]
    pub fn snapshot(app: &App) -> Self {
        let requested = app.viewer().requested();
        let viewer = app.viewer().owner().snapshot();
        let device = app.runtime().facts();
        let precision =
            precision_for(requested.zoom_log2, device.width, requested.iteration_cap).ok();
        let scale = scaled_pixel_scale(requested.zoom_log2, device.width).ok();
        let zoom_digits_f64 = requested.zoom_log2 * core::f64::consts::LOG10_2;
        let depth_digits = ceil_nonnegative_to_u32(zoom_digits_f64);
        let loop_facts = app.frame_loop();
        let present = loop_facts.present_facts();
        let dispatch = loop_facts.dispatch_facts();
        let worker = loop_facts.worker_facts();
        let plan = loop_facts.plan();
        Self {
            abi_version: JULIBROT_ABI_VERSION,
            adapter_name: device.adapter_name.clone(),
            backend: device.backend.clone(),
            rgba32f_renderable: device.rgba32f_renderable,
            requested_width: device.width,
            requested_height: device.height,
            delivered_width: nonzero(present.delivered_width),
            delivered_height: nonzero(present.delivered_height),
            requested_iteration_cap: requested.iteration_cap,
            delivered_iteration_cap: present.iteration_cap,
            requested_zoom_log2: requested.zoom_log2,
            presented_zoom_log2: loop_facts.last_presented_zoom_log2(),
            reference_zoom_log2: loop_facts.accepted_reference_zoom_log2(),
            zoom_digits_f64,
            depth_digits,
            precision_floor_digits: precision.map(|plan| plan.floor_digits),
            precision_working_digits: precision.map(|plan| plan.working_digits),
            requested_precision_bits: precision.map(|plan| plan.requested_bits),
            delivered_precision_bits: nonzero(viewer.main.precision_bits),
            orbit_length: nonzero(viewer.main.orbit_length),
            orbit_generation: nonzero(viewer.main.generation_applied),
            owner_epoch: viewer.epoch,
            centre_revision: viewer.main.centre_revision,
            centre_from_reference_px: viewer.hot.centre_from_reference_px,
            reference_shift_px: viewer.main.reference_shift_px,
            scale_mantissa: scale.map(|value| value.mantissa),
            scale_exponent: scale.map(|value| value.exponent),
            kernel_mode: dispatch.map(|facts| kernel_mode(facts.mode).to_string()),
            refinement_level: present
                .delivered_level
                .map(|level| refinement_level(level).to_string()),
            refinement_pending: loop_facts.refinement_pending(),
            scene_mode: loop_facts.scene_mode().as_str(),
            scene_update_pending: loop_facts.scene_update_pending(),
            draft_skipped_count: loop_facts.draft_skipped_count(),
            last_draft_skip_reason: loop_facts.last_draft_skip_reason(),
            extent_divisor: Some(plan.extent_divisor),
            active_pixels: dispatch.map(|facts| facts.active_pixels),
            worst_case_pixel_iterations: dispatch.map(|facts| facts.worst_case_pixel_iterations),
            kernel_page_passes: dispatch.map(|facts| facts.page_passes),
            scratch_copy_commands: dispatch.map(|facts| facts.copy_commands),
            scratch_copy_bytes: dispatch.map(|facts| facts.gpu_copy_bytes),
            logical_heap_bytes: dispatch.map(|facts| facts.logical_heap_bytes),
            reserved_heap_bytes: dispatch.map(|facts| facts.reserved_heap_bytes),
            scratch_bytes: dispatch.map(|facts| facts.scratch_bytes),
            hot_write_bytes: Some(ember_julibrot_present::HOT_PAYLOAD_BYTES),
            scene_uniform_write_bytes: Some(ember_julibrot_present::SCENE_PAYLOAD_BYTES),
            texture_reallocations: Some(present.texture_reallocations),
            rebase_count_sum: "unavailable",
            rebase_count_max: "unavailable",
            glitch_pixel_count: "unavailable",
            worker_facts: Some(worker_json(worker)),
            worker_compute_us: nonzero(worker.last_compute_us),
            worker_credit_us: Some(worker.credit_us),
            worker_overfeed_us: Some(worker.last_overfeed_us),
            worker_allocation_events: Some(worker.allocation_events),
            request_buffers_owned_main: Some(worker.request_buffers_owned_main),
            orbit_buffers_owned_main: Some(worker.orbit_buffers_owned_main),
            worker_request_depth: loop_facts.worker_request_depth(),
            outstanding_reference_count: loop_facts.outstanding_reference_count(),
            outstanding_reference_generation: loop_facts.outstanding_reference_generation(),
            navigation_pending_depth: app.viewer().owner().navigation_pending_depth(),
            refresh_status: loop_facts.last_status().name(),
            transient_fence_refusals: loop_facts.transient_refusals(),
            last_transient_refusal: loop_facts.last_transient_refusal(),
            presented_view_stale: loop_facts.presented_view_is_stale(app.viewer()),
            loop_stopped_reason: loop_facts.stopped_reason(),
            palette_id: requested.palette as u32,
            object_angles: requested.object_angles.as_array(),
            plane_theta_1: requested.object_angles.rho_13,
            plane_theta_2: requested.object_angles.rho_24,
            plane_origin: requested.plane_origin,
            target_plane: app
                .viewer()
                .owner()
                .navigation_centre()
                .map_or(viewer.main.centre_f64, |centre| centre.to_f64_mirror()),
            view_theta_1: requested.view.camera[0],
            view_theta_2: requested.view.camera[8],
            camera_angles: requested.view.camera,
            camera_translation: requested.view.camera_translation,
            camera_yaw: requested.view.camera_yaw,
            camera_pitch: requested.view.camera_pitch,
            height_scale: requested.view.height_scale,
            distance_five: requested.view.distance_five,
            distance_four: requested.view.distance_four,
            horizon_pixels: loop_facts.horizon_pixels(),
            horizon_fraction: loop_facts.horizon_fraction(),
            uncertain_pixels: loop_facts.uncertain_pixels(),
            uncertain_fraction: loop_facts.uncertain_fraction(),
            edge_on: loop_facts.edge_on(),
            map_condition_number: loop_facts.map_condition_number(),
            completed_scene_id: present.completed_scene_id,
            in_flight_scene_id: present.in_flight_scene_id,
            warp_source_scene_id: loop_facts.last_warp_source(),
            reprojected_per_scene: present.reprojected_per_scene,
            refreshes_without_scene: present.refreshes_without_scene,
            chart_residual: present.chart_residual,
            warp_max_error_px: present.warp_max_error_px,
            warp_p95_error_px: present.warp_p95_error_px,
            warp_exposed_fraction: present.warp_exposed_fraction,
            warp_kind: present.warp_kind.as_str().to_string(),
            scene_wall_ms: present.last_scene.map(|sample| sample.wall_ms),
            scene_fence_wait_ms: present.last_scene.map(|sample| sample.fence_wait_ms),
            scene_polls: present.last_scene.map(|sample| sample.polls),
            warp_wall_ms: present.last_warp.map(|sample| sample.wall_ms),
            warp_fence_wait_ms: present.last_warp.map(|sample| sample.fence_wait_ms),
            warp_polls: present.last_warp.map(|sample| sample.polls),
            warmup_label: newest_measurement(present.last_scene, present.last_warp)
                .map(|sample| sample_class(sample.sample_class).to_string()),
            second_frame_policy: frame_policy(loop_facts.frame_policy()).map(str::to_string),
            timer_quantum_ms: None,
            device_walls: vec!["WebGL2", "EXT_color_buffer_float", "RGBA32F usages"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            app_policies: vec![
                "shallow/deep switch zoom_log2=14",
                "iteration cap=4096",
                "worker credit=250000us/s",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            limiting_term: (plan.extent_divisor > 1)
                .then(|| format!("live heap/header capacity divisor {}", plan.extent_divisor)),
            wasm_bundle_bytes: None,
            javascript_bundle_bytes: None,
            wasm_instance_count: 2,
            timing_status: "requires visible replay",
            precision_mode: requested.precision_mode.as_str(),
            scene_precision_mode: present.last_scene.map(|sample| sample.precision_mode),
            warp_precision_mode: present.last_warp.map(|sample| sample.precision_mode),
            level_timings: loop_facts.level_timings(),
            draft_pixels_discarded: dispatch.map(|facts| facts.draft_pixels_discarded),
            draft_iterations_discarded: dispatch.map(|facts| facts.draft_iterations_discarded),
        }
    }
}

const fn kernel_mode(mode: KernelMode) -> &'static str {
    match mode {
        KernelMode::Shallow => "Shallow",
        KernelMode::Perturbation => "Perturbation",
    }
}

const fn refinement_level(level: RefinementLevel) -> &'static str {
    match level {
        RefinementLevel::Preview => "Preview",
        RefinementLevel::Interactive => "Interactive",
        RefinementLevel::Final => "Final",
    }
}

const fn sample_class(class: SampleClass) -> &'static str {
    match class {
        SampleClass::ColdWarmUp => "ColdWarmUp",
        SampleClass::PolicyProbe => "PolicyProbe",
        SampleClass::Measured => "Measured",
    }
}

fn newest_measurement(
    scene: Option<SubmissionMeasurement>,
    warp: Option<SubmissionMeasurement>,
) -> Option<SubmissionMeasurement> {
    match (scene, warp) {
        (Some(scene), Some(warp)) => Some(if scene.wall_ms >= warp.wall_ms {
            scene
        } else {
            warp
        }),
        (Some(scene), None) => Some(scene),
        (None, Some(warp)) => Some(warp),
        (None, None) => None,
    }
}

fn worker_json(facts: ember_julibrot_worker::WorkerFacts) -> serde_json::Value {
    serde_json::json!({
        "epoch": facts.epoch,
        "last_applied_generation": facts.last_applied_generation,
        "last_ack_generation": facts.last_ack_generation,
        "orbit_queue_depth": facts.orbit_queue_depth,
        "shutdown_queue_depth": facts.shutdown_queue_depth,
        "credit_us": facts.credit_us,
        "last_compute_us": facts.last_compute_us,
        "last_overfeed_us": facts.last_overfeed_us,
        "applied_count": facts.applied_count,
        "stale_count": facts.stale_count,
        "cancelled_count": facts.cancelled_count,
        "allocation_events": facts.allocation_events,
        "request_buffers_owned_main": facts.request_buffers_owned_main,
        "orbit_buffers_owned_main": facts.orbit_buffers_owned_main,
        "mode": facts.mode,
    })
}

const fn frame_policy(policy: FramePolicy) -> Option<&'static str> {
    match policy {
        FramePolicy::Undecided => None,
        FramePolicy::SingleFrameOnDemand => Some("SingleFrameOnDemand"),
        FramePolicy::Continuous => Some("Continuous"),
    }
}

fn nonzero(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ceil_nonnegative_to_u32(value: f64) -> u32 {
    value.max(0.0).ceil().min(f64::from(u32::MAX)) as u32
}
