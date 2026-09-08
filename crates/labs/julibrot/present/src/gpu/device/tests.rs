use ember_julibrot_kernels::{EscapeGrid, RefinementLevel};
use ember_julibrot_math::{ObjectAngles, PrecisionMode, ViewControls, construct_plane};
use ember_julibrot_worker::{HotState, MainState};

use super::census::{census_if_ready, observe_fence, take_glitch_readback_result};
use super::ledger::{LatticeRefusal, presentation_ledger_entry, redraw_source_covers_destination};
use super::readback::{FrameReadback, FrameReadbackRoute};
use super::*;
use crate::fence::FenceDecision;
use crate::state::{PendingScene, SceneCompletion};
use crate::{
    FrameReceipt, FrameState, PresentFacts, PresentHot, SubmissionKind, SubmissionMeasurement,
};

#[test]
fn glitch_census_sums_red_counts_and_ignores_row_padding() {
    let mut bytes = vec![99_u8; 32];
    bytes[..8].copy_from_slice(&[7, 10, 3, 255, 11, 40, 5, 255]);
    bytes[16..24].copy_from_slice(&[13, 40, 9, 255, 17, 0, 0, 0]);
    let census = census_bytes(&bytes, [2, 2], 16);
    assert_eq!(census.glitch_pixel_count, 48);
    assert_eq!(census.reference_sample, Some(GLITCH_RECORDS_PER_TEXEL + 5));
}

/// Packs one grid of escape records the way the census fragment shader would.
///
/// This mirrors the shader's arithmetic on the CPU rather than executing it: the same 255-record
/// groups, the same four-tier rank, the same unorm quantisation to a byte, and the same
/// row-padded readback layout. It exists so the decode is exercised against shader-shaped bytes
/// instead of hand-written ones.
#[allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the mirror reproduces the census fragment's exact comparisons and unorm rounding"
)]
fn census_texels(records: &[[f32; 4]], cap: f32, grid_width: u32) -> (Vec<u8>, [u32; 2], u32) {
    let groups = u32::try_from(records.len().div_ceil(GLITCH_RECORDS_PER_TEXEL as usize))
        .expect("group count fits");
    let extent = [grid_width, groups.div_ceil(grid_width).max(1)];
    let bytes_per_row = (extent[0] * RGBA8_BYTES_PER_TEXEL)
        .div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let mut bytes = vec![0xAB_u8; (bytes_per_row * extent[1]) as usize];
    for row in 0..extent[1] {
        for column in 0..extent[0] {
            let group = row * extent[0] + column;
            let start = (group * GLITCH_RECORDS_PER_TEXEL) as usize;
            let mut count = 0_u32;
            let mut best_rank = 0.0_f32;
            let mut best_offset = 0_u32;
            let mut located = 0.0_f32;
            for offset in 0..GLITCH_RECORDS_PER_TEXEL {
                let Some(record) = records.get(start + offset as usize) else {
                    break;
                };
                if record[3] == 1.0 {
                    count += 1;
                }
                if record[3] == 2.0 || record[3] == 3.0 {
                    continue;
                }
                let rank = if record[3] == 1.0 {
                    if record[0] == -1.0 { 254.0 } else { 0.0 }
                } else if record[1] == 1.0 {
                    (252.0 * record[0].ceil().clamp(0.0, cap) / cap + 0.5).floor() + 1.0
                } else {
                    255.0
                };
                if located == 0.0 || rank > best_rank {
                    best_rank = rank;
                    best_offset = offset;
                    located = 1.0;
                }
            }
            let texel = (row * bytes_per_row + column * RGBA8_BYTES_PER_TEXEL) as usize;
            bytes[texel] = u8::try_from(count).expect("group count is at most 255");
            bytes[texel + 1] = best_rank as u8;
            bytes[texel + 2] = u8::try_from(best_offset).expect("offset is at most 254");
            bytes[texel + 3] = if located == 0.0 { 0 } else { 255 };
        }
    }
    (bytes, extent, bytes_per_row)
}

/// Pins the decode against shader-shaped bytes, including what the byte rank cannot separate.
#[test]
fn the_census_decodes_shader_shaped_bytes_and_keeps_the_lowest_index_on_a_quantised_tie() {
    const CAP: f32 = 512.0;
    const GRID_WIDTH: u32 = 4;
    let escaped = |count: f32| [count, 1.0, 0.0, 0.0];
    let interior = [-1.0, 0.0, 0.0, 0.0];
    let exhausted = [-1.0, 0.0, 0.0, 1.0];
    let numeric = [-2.0, 0.0, 0.0, 1.0];

    // Two escaping records one iteration apart share a byte rank; the lower index must win, and
    // the higher exact count loses. The mirror over exact counts would name the other one.
    let mut records = vec![escaped(10.0); 3 * GLITCH_RECORDS_PER_TEXEL as usize];
    records[7] = escaped(400.0);
    records[GLITCH_RECORDS_PER_TEXEL as usize + 3] = escaped(401.0);
    let (bytes, extent, bytes_per_row) = census_texels(&records, CAP, GRID_WIDTH);
    assert_eq!(
        census_bytes(&bytes, extent, bytes_per_row).reference_sample,
        Some(7),
        "a byte rank cannot separate 400 from 401, and the lowest index keeps the tie"
    );

    // The four tiers, decoded end to end: numeric failure below every escape, an exhaustion
    // glitch above them, and a record that never escaped above that.
    records[GLITCH_RECORDS_PER_TEXEL as usize + 4] = numeric;
    records[2 * GLITCH_RECORDS_PER_TEXEL as usize + 9] = exhausted;
    let (bytes, extent, bytes_per_row) = census_texels(&records, CAP, GRID_WIDTH);
    let census = census_bytes(&bytes, extent, bytes_per_row);
    assert_eq!(census.glitch_pixel_count, 2);
    assert_eq!(
        census.reference_sample,
        Some(2 * GLITCH_RECORDS_PER_TEXEL + 9)
    );
    records[5] = interior;
    let (bytes, extent, bytes_per_row) = census_texels(&records, CAP, GRID_WIDTH);
    assert_eq!(
        census_bytes(&bytes, extent, bytes_per_row).reference_sample,
        Some(5)
    );
}

#[test]
fn the_census_reference_candidate_is_the_lowest_index_of_the_highest_rank() {
    let mut bytes = vec![0_u8; 32];
    bytes[..8].copy_from_slice(&[0, 200, 4, 255, 0, 200, 1, 255]);
    bytes[16..24].copy_from_slice(&[0, 201, 2, 255, 0, 255, 0, 0]);
    let census = census_bytes(&bytes, [2, 2], 16);
    assert_eq!(census.glitch_pixel_count, 0);
    assert_eq!(
        census.reference_sample,
        Some(2 * GLITCH_RECORDS_PER_TEXEL + 2)
    );
    let empty = census_bytes(&[0_u8; 32], [2, 2], 16);
    assert_eq!(empty, SceneCensus::EMPTY);
}

#[test]
fn census_failure_or_delay_never_refuses_or_delays_the_scene() {
    for census_result in [None, Some(Err(()))] {
        let mut pending = PendingFence {
            ledger: FenceLedger::new(
                SubmissionKind::Scene,
                29,
                None,
                PrecisionMode::PictureFast.as_str(),
                SampleClass::Measured,
                100.0,
                30_000.0,
                4_096,
            ),
            signal: Arc::new(Mutex::new(Some(Ok(())))),
            signal_result: None,
            glitch_readback: Some(PendingGlitchReadback {
                signal: Arc::new(Mutex::new(census_result)),
            }),
        };

        let FenceDecision::Complete(measurement) = observe_fence(&mut pending, 101.0, 31) else {
            panic!("a successful scene fence must deliver independently of its census");
        };
        assert_eq!(measurement.id, 29);
        assert_eq!(measurement.completion_sequence, 31);
        let census = census_if_ready(take_glitch_readback_result(&mut pending), || {
            panic!("an unavailable census must not be read")
        });
        assert_eq!(census, None);
    }
}

/// The pixel lattice every binding fixture samples and presents on.
const BINDING_EXTENT: [u32; 2] = [64, 36];
const NON_UNIFORM_CAPTURE_EXTENT: [u32; 2] = [256, 144];

fn binding_pose() -> Pose {
    binding_pose_on(BINDING_EXTENT)
}

fn binding_pose_on(extent: [u32; 2]) -> Pose {
    Pose {
        epoch: 1,
        orbit_generation: 1,
        plane: ember_julibrot_math::Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        },
        object: ember_julibrot_math::ObjectAngles::JULIA,
        plane_origin: [0.0; 4],
        zoom_log2: 0.0,
        view: ViewControls::NEUTRAL,
        grid_width: extent[0],
        grid_height: extent[1],
        map: PoseMap::Mapped(ember_julibrot_math::Homography::IDENTITY),
        centre_from_reference_px: [0.0; 2],
    }
}

fn binding_measurement(id: u64) -> SubmissionMeasurement {
    SubmissionMeasurement {
        kind: SubmissionKind::Scene,
        id,
        completion_sequence: id,
        source_scene_id: None,
        sample_class: SampleClass::Measured,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        wall_ms: 1.0,
        fence_wait_ms: 0.5,
        polls: 1,
    }
}

fn promote_binding_scene(ledger: &mut SceneLedger, scene_id: u64) -> crate::SceneFrame {
    promote_binding_scene_on(ledger, scene_id, BINDING_EXTENT)
}

fn promote_binding_scene_on(
    ledger: &mut SceneLedger,
    scene_id: u64,
    extent: [u32; 2],
) -> crate::SceneFrame {
    ledger
        .begin(|texture_index| {
            Ok(PendingScene {
                scene_id,
                pose: binding_pose_on(extent),
                iteration_cap: 64,
                level: RefinementLevel::Final,
                extent,
                grid: binding_main_on(extent).grid,
                texture_index,
                centre_revision: 1,
                plane_origin_f64: [0.0; 4],
                precision_mode: PrecisionMode::PictureFast.as_str(),
                drop_reason: None,
            })
        })
        .expect("binding scene begins");
    match ledger.complete(binding_measurement(scene_id)) {
        Some(SceneCompletion::Promoted(frame)) => frame,
        other => panic!("binding scene did not promote: {other:?}"),
    }
}

fn binding_main() -> PresentMain {
    binding_main_on(BINDING_EXTENT)
}

fn binding_main_on(extent: [u32; 2]) -> PresentMain {
    let side = u16::try_from(extent[0].max(extent[1]).next_power_of_two())
        .expect("the native fixture extent fits the heap side");
    let mut arena = ember_lab_heap::SpanArena::new(side, 1, 64, 4_096, 64)
        .expect("relief fixture arena is valid");
    let span = arena
        .allocate_span(extent[0] * extent[1], 64)
        .expect("relief fixture grid fits");
    PresentMain {
        epoch: 1,
        state: MainState {
            delivered_iter_cap: 64,
            ..MainState::default()
        },
        grid: EscapeGrid {
            span,
            width: extent[0],
            height: extent[1],
            level: RefinementLevel::Final,
        },
        object: ember_julibrot_math::ObjectAngles::JULIA,
        plane: binding_pose_on(extent).plane,
        map: PoseMap::EdgeOn,
        backdrop: None,
    }
}

#[test]
fn reference_acceptance_identity_is_independent_of_centre_revision() {
    let accepted = binding_main();
    let mut edited = binding_main();
    edited.state.centre_revision = accepted.state.centre_revision + 1;
    assert!(!accepted_reference_advanced(Some(&accepted), &edited));

    let mut next_acceptance = edited.clone();
    next_acceptance.state.generation_applied = accepted.state.generation_applied + 1;
    assert!(accepted_reference_advanced(Some(&edited), &next_acceptance));
}

fn native_test_device() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let request = |force_fallback_adapter| wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter,
    };
    let adapter = pollster::block_on(instance.request_adapter(&request(true)))
        .or_else(|| pollster::block_on(instance.request_adapter(&request(false))))
        .expect("a native GPU or software adapter is available for the presentation test");
    let required = wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT;
    assert!(
        adapter
            .get_texture_format_features(wgpu::TextureFormat::Rgba32Float)
            .allowed_usages
            .contains(required),
        "the native test adapter supports the WebGL2 value-target contract"
    );
    let limits = wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("Julibrot native presentation test device"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::MemoryUsage,
        },
        None,
    ))
    .expect("the native presentation test device is created");
    // `Arc::new` makes Clippy's `arc_with_non_send_sync` probe recurse through wgpu's backend
    // dispatch graph on this nightly. `Presenter` requires these shared handles, and conversion
    // constructs the same concrete Arc without asking that unrelated lint to solve the graph.
    (Arc::from(device), Arc::from(queue))
}

fn native_test_heap(device: &wgpu::Device) -> HeapPresentResources {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot native presentation test heap"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let data_view = Arc::from(texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    }));
    let buffer = |label, size| {
        Arc::from(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        }))
    };
    HeapPresentResources {
        data_view,
        descriptor_buffer: buffer("Julibrot native test heap descriptors", 16),
        span_directory_buffer: buffer("Julibrot native test heap directory", 32),
        descriptor_capacity: 1,
        span_capacity: 1,
        handle_capacity: 4,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeValuePattern {
    Uniform,
    Split,
}

fn native_split_value_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    const SOURCE: &str = r"
        @vertex
        fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
            var positions = array<vec2<f32>, 3>(
                vec2<f32>(-1.0, -1.0),
                vec2<f32>(3.0, -1.0),
                vec2<f32>(-1.0, 3.0),
            );
            return vec4<f32>(positions[index], 0.0, 1.0);
        }

        @fragment
        fn fragment() -> @location(0) vec4<f32> {
            return vec4<f32>(3.0, 1.0, 0.0, 0.2);
        }
    ";
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Julibrot native split-value shader"),
        source: wgpu::ShaderSource::Wgsl(SOURCE.into()),
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Julibrot native split-value pipeline"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba32Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn native_palette_presenter() -> (Presenter, Arc<wgpu::Device>) {
    native_palette_presenter_on(BINDING_EXTENT, NativeValuePattern::Uniform)
}

fn native_palette_presenter_on(
    extent: [u32; 2],
    pattern: NativeValuePattern,
) -> (Presenter, Arc<wgpu::Device>) {
    let (device, queue) = native_test_device();
    let config = PresentConfig {
        surface_format: wgpu::TextureFormat::Rgba8Unorm,
        min_uniform_buffer_offset_alignment: device.limits().min_uniform_buffer_offset_alignment,
        fence_deadline_ms: PresentConfig::V1_FENCE_DEADLINE_MS,
        max_fence_polls: PresentConfig::V1_MAX_FENCE_POLLS,
    };
    let mut presenter = Presenter::new(
        Arc::clone(&device),
        Arc::clone(&queue),
        native_test_heap(&device),
        config,
    )
    .expect("the native palette presenter is created");
    let mut main = binding_main_on(extent);
    main.state.generation_applied = 1;
    main.state.centre_revision = 1;
    main.state.precision_mode = PrecisionMode::PictureFast as u32;
    presenter.set_main(main);
    let completed = promote_binding_scene_on(&mut presenter.ledger, 37, extent);
    presenter.facts.completed_scene_id = Some(completed.scene_id);
    let texture_index = usize::try_from(completed.texture_index)
        .expect("the retained texture index fits this process");
    ensure_scene_texture(&device, &mut presenter.gpu, texture_index, completed.extent)
        .expect("the retained value texture is allocated");
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Julibrot native retained-value encoder"),
    });
    let split_pipeline =
        (pattern == NativeValuePattern::Split).then(|| native_split_value_pipeline(&device));
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Julibrot native retained-value clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &presenter.gpu.scene_textures[texture_index].view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 12.0,
                        g: 1.0,
                        b: 0.0,
                        a: 0.7,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        if let Some(pipeline) = split_pipeline.as_ref() {
            let split = extent[0] / 2;
            pass.set_pipeline(pipeline);
            pass.set_scissor_rect(split, 0, extent[0] - split, extent[1]);
            pass.draw(0..3, 0..1);
        }
    }
    queue.submit([encoder.finish()]);
    (presenter, device)
}

fn capture_native_palette(
    presenter: &mut Presenter,
    device: &wgpu::Device,
    palette_id: PaletteId,
    refresh_id: u64,
) -> (FrameReceipt, FrameReadback) {
    capture_native_palette_on(presenter, device, palette_id, refresh_id, BINDING_EXTENT)
}

fn capture_native_palette_on(
    presenter: &mut Presenter,
    device: &wgpu::Device,
    palette_id: PaletteId,
    refresh_id: u64,
    extent: [u32; 2],
) -> (FrameReceipt, FrameReadback) {
    let mut main = presenter.main.clone().expect("the test MAIN is installed");
    main.epoch = refresh_id;
    main.state.palette_id = palette_id as u32;
    presenter.set_main(main);
    let pose = binding_pose_on(extent);
    let hot = PresentHot {
        epoch: refresh_id,
        state: HotState::default(),
        plane: pose.plane,
        object: pose.object,
        view: pose.view,
        map: pose.map,
    };
    let stride = crate::hot_stride(device.limits().min_uniform_buffer_offset_alignment)
        .expect("the device admits the HOT stride");
    let slot = HotSlot::for_refresh(refresh_id, stride, hot.epoch)
        .expect("the refresh selects a HOT slot");
    presenter.write_hot(slot, hot, false);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot native palette presentation target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    presenter.arm_offscreen_frame_readback();
    let now_ms = f64::from(
        u32::try_from(refresh_id).expect("the native test refresh fits the exact f64 range"),
    );
    let receipt = presenter
        .frame(
            FrameState {
                surface_view: &view,
                canvas_width: extent[0],
                canvas_height: extent[1],
                refresh_id,
                now_ms,
            },
            slot,
        )
        .expect("the palette presentation is submitted");
    device.poll(wgpu::Maintain::Wait);
    assert!(presenter.poll_fixed(now_ms + 1.0).into_iter().any(
        |event| matches!(event, crate::PresentEvent::WarpCompleted { measurement } if measurement.id == receipt.warp_id)
    ));
    presenter.record_presented(receipt.warp_id);
    device.poll(wgpu::Maintain::Wait);
    let readback = presenter
        .take_frame_readback()
        .expect("the offscreen palette copy maps")
        .expect("the offscreen palette copy is ready");
    (receipt, readback)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RgbaRun {
    length: u32,
    rgba: [u8; 4],
}

#[derive(Debug, PartialEq)]
struct NativeWholeGridCapture {
    route: FrameReadbackRoute,
    extent: [u32; 2],
    copied_scene_id: Option<u64>,
    completed_scene_id: Option<u64>,
    receipt: FrameReceipt,
    stable_facts: PresentFacts,
    rgba_fnv1a64: String,
    rgba_runs: Vec<RgbaRun>,
}

fn clear_submission_wall(measurement: &mut Option<SubmissionMeasurement>) {
    if let Some(measurement) = measurement {
        measurement.wall_ms = 0.0;
        measurement.fence_wait_ms = 0.0;
    }
}

fn stable_present_facts(facts: &PresentFacts) -> PresentFacts {
    let mut stable = facts.clone();
    clear_submission_wall(&mut stable.last_scene);
    clear_submission_wall(&mut stable.last_warp);
    stable
}

fn rgba_fnv1a64(rgba: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    rgba.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

fn rgba_runs(rgba: &[u8]) -> Vec<RgbaRun> {
    let (pixels, remainder) = rgba.as_chunks::<4>();
    assert_eq!(remainder, &[], "the whole-grid copy is packed RGBA");
    let mut runs: Vec<RgbaRun> = Vec::new();
    for &rgba in pixels {
        if let Some(run) = runs.last_mut()
            && run.rgba == rgba
        {
            run.length += 1;
        } else {
            runs.push(RgbaRun { length: 1, rgba });
        }
    }
    runs
}

fn native_whole_grid_capture() -> NativeWholeGridCapture {
    native_whole_grid_capture_on(BINDING_EXTENT, NativeValuePattern::Uniform)
}

fn non_uniform_native_whole_grid_capture() -> NativeWholeGridCapture {
    native_whole_grid_capture_on(NON_UNIFORM_CAPTURE_EXTENT, NativeValuePattern::Split)
}

fn native_whole_grid_capture_on(
    extent: [u32; 2],
    pattern: NativeValuePattern,
) -> NativeWholeGridCapture {
    let (mut presenter, device) = native_palette_presenter_on(extent, pattern);
    let (receipt, readback) =
        capture_native_palette_on(&mut presenter, &device, PaletteId::Classic, 101, extent);
    let stable_facts = stable_present_facts(presenter.facts_ref());
    let extent = [readback.width, readback.height];
    let copied_scene_id = readback.scene_id;
    let expected_bytes = usize::try_from(readback.width * readback.height * 4)
        .expect("the native whole-grid byte count fits this process");
    assert_eq!(readback.rgba.len(), expected_bytes);
    assert_eq!(copied_scene_id, stable_facts.completed_scene_id);
    let rgba_runs = rgba_runs(&readback.rgba);
    if pattern == NativeValuePattern::Split {
        assert!(
            rgba_runs.len() > 1,
            "the larger native oracle must retain spatial variation"
        );
    }
    NativeWholeGridCapture {
        route: readback.route,
        extent,
        copied_scene_id,
        completed_scene_id: stable_facts.completed_scene_id,
        receipt,
        rgba_fnv1a64: format!("{:016x}", rgba_fnv1a64(&readback.rgba)),
        rgba_runs,
        stable_facts,
    }
}

#[test]
#[ignore = "prints the server-rendered fixture for review and verbatim commit"]
#[allow(
    clippy::print_stdout,
    reason = "the ignored generator emits its reviewed fixture"
)]
fn print_native_whole_grid_capture_fixture() {
    let capture = native_whole_grid_capture();
    let fixture = format!("{capture:#?}");
    println!("const NATIVE_WHOLE_GRID_FIXTURE: &str = {fixture:?};");
}

#[test]
fn native_whole_grid_capture_matches_fixture() {
    let capture = native_whole_grid_capture();
    assert_eq!(format!("{capture:#?}"), NATIVE_WHOLE_GRID_FIXTURE);
}

const NON_UNIFORM_NATIVE_WHOLE_GRID_FIXTURE: &str = "NativeWholeGridCapture {\n    route: OffscreenRerender,\n    extent: [\n        256,\n        144,\n    ],\n    copied_scene_id: Some(\n        37,\n    ),\n    completed_scene_id: Some(\n        37,\n    ),\n    receipt: FrameReceipt {\n        refresh_id: 101,\n        warp_id: 1,\n        source_scene_id: Some(\n            37,\n        ),\n        precision_mode: \"PictureFast\",\n        exposed: false,\n        status: ShowingCompletedScene,\n    },\n    stable_facts: PresentFacts {\n        completed_scene_id: Some(\n            37,\n        ),\n        in_flight_scene_id: None,\n        source_generation: None,\n        held_frame_partition: None,\n        held_since_scene_id: None,\n        precision_mode: \"PictureFast\",\n        delivered_width: 0,\n        delivered_height: 0,\n        destination_width: 256,\n        destination_height: 144,\n        warp_source_width: 256,\n        warp_source_height: 144,\n        warp_lattice_refusal: None,\n        delivered_level: None,\n        iteration_cap: None,\n        glitch_pixel_count: None,\n        palette: Classic,\n        view: ViewControls {\n            camera: [\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n            ],\n            camera_translation: [\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n            ],\n            camera_yaw: 0.0,\n            camera_pitch: 0.0,\n            height_scale: 0.0,\n            distance_five: 8.0,\n            distance_four: 8.0,\n        },\n        centre_from_reference_px: [\n            0.0,\n            0.0,\n        ],\n        reference_shift_px: [\n            0.0,\n            0.0,\n        ],\n        last_scene: None,\n        last_warp: Some(\n            SubmissionMeasurement {\n                kind: Warp,\n                id: 1,\n                completion_sequence: 1,\n                source_scene_id: Some(\n                    37,\n                ),\n                sample_class: ColdWarmUp,\n                precision_mode: \"PictureFast\",\n                wall_ms: 0.0,\n                fence_wait_ms: 0.0,\n                polls: 1,\n            },\n        ),\n        reprojected_per_scene: None,\n        relief_redraw_count: 0,\n        warp_hold_count: 0,\n        refreshes_without_scene: 0,\n        texture_reallocations: 0,\n        warp_exposed: false,\n        warp_exposed_fraction: Some(\n            0.0,\n        ),\n        scene_fill_due: false,\n        chart_residual: Some(\n            0.0,\n        ),\n        warp_max_error_px: Some(\n            0.0,\n        ),\n        warp_p95_error_px: Some(\n            0.0,\n        ),\n        warp_kind: AnchorHomography,\n        warp_refusal_reason: None,\n        presentation_ledger: PresentationLedger {\n            entries: [\n                Some(\n                    PresentationLedgerEntry {\n                        scene_id: Some(\n                            37,\n                        ),\n                        level: Some(\n                            Final,\n                        ),\n                        warp_kind: AnchorHomography,\n                        requested_centre_px: Some(\n                            [\n                                128.0,\n                                72.0,\n                            ],\n                        ),\n                        anchor_px: Some(\n                            [\n                                0.0,\n                                0.0,\n                            ],\n                        ),\n                    },\n                ),\n                None,\n                None,\n                None,\n                None,\n                None,\n                None,\n                None,\n            ],\n            next: 1,\n            len: 1,\n        },\n        status: ShowingCompletedScene,\n    },\n    rgba_fnv1a64: \"d54107bc96969325\",\n    rgba_runs: [\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n        RgbaRun {\n            length: 128,\n            rgba: [\n                255,\n                0,\n                255,\n                255,\n            ],\n        },\n    ],\n}";

#[test]
fn non_uniform_native_whole_grid_capture_matches_fixture() {
    let capture = non_uniform_native_whole_grid_capture();
    assert_eq!(
        format!("{capture:#?}"),
        NON_UNIFORM_NATIVE_WHOLE_GRID_FIXTURE
    );
}

#[test]
#[ignore = "prints the non-uniform server-rendered fixture for review and verbatim commit"]
#[allow(
    clippy::print_stdout,
    reason = "the ignored generator emits its reviewed fixture"
)]
fn print_non_uniform_native_whole_grid_capture_fixture() {
    let capture = non_uniform_native_whole_grid_capture();
    let fixture = format!("{capture:#?}");
    println!("const NON_UNIFORM_NATIVE_WHOLE_GRID_FIXTURE: &str = {fixture:?};");
}

const NATIVE_WHOLE_GRID_FIXTURE: &str = "NativeWholeGridCapture {\n    route: OffscreenRerender,\n    extent: [\n        64,\n        36,\n    ],\n    copied_scene_id: Some(\n        37,\n    ),\n    completed_scene_id: Some(\n        37,\n    ),\n    receipt: FrameReceipt {\n        refresh_id: 101,\n        warp_id: 1,\n        source_scene_id: Some(\n            37,\n        ),\n        precision_mode: \"PictureFast\",\n        exposed: false,\n        status: ShowingCompletedScene,\n    },\n    stable_facts: PresentFacts {\n        completed_scene_id: Some(\n            37,\n        ),\n        in_flight_scene_id: None,\n        source_generation: None,\n        held_frame_partition: None,\n        held_since_scene_id: None,\n        precision_mode: \"PictureFast\",\n        delivered_width: 0,\n        delivered_height: 0,\n        destination_width: 64,\n        destination_height: 36,\n        warp_source_width: 64,\n        warp_source_height: 36,\n        warp_lattice_refusal: None,\n        delivered_level: None,\n        iteration_cap: None,\n        glitch_pixel_count: None,\n        palette: Classic,\n        view: ViewControls {\n            camera: [\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n            ],\n            camera_translation: [\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n                0.0,\n            ],\n            camera_yaw: 0.0,\n            camera_pitch: 0.0,\n            height_scale: 0.0,\n            distance_five: 8.0,\n            distance_four: 8.0,\n        },\n        centre_from_reference_px: [\n            0.0,\n            0.0,\n        ],\n        reference_shift_px: [\n            0.0,\n            0.0,\n        ],\n        last_scene: None,\n        last_warp: Some(\n            SubmissionMeasurement {\n                kind: Warp,\n                id: 1,\n                completion_sequence: 1,\n                source_scene_id: Some(\n                    37,\n                ),\n                sample_class: ColdWarmUp,\n                precision_mode: \"PictureFast\",\n                wall_ms: 0.0,\n                fence_wait_ms: 0.0,\n                polls: 1,\n            },\n        ),\n        reprojected_per_scene: None,\n        relief_redraw_count: 0,\n        warp_hold_count: 0,\n        refreshes_without_scene: 0,\n        texture_reallocations: 0,\n        warp_exposed: false,\n        warp_exposed_fraction: Some(\n            0.0,\n        ),\n        scene_fill_due: false,\n        chart_residual: Some(\n            0.0,\n        ),\n        warp_max_error_px: Some(\n            0.0,\n        ),\n        warp_p95_error_px: Some(\n            0.0,\n        ),\n        warp_kind: AnchorHomography,\n        warp_refusal_reason: None,\n        presentation_ledger: PresentationLedger {\n            entries: [\n                Some(\n                    PresentationLedgerEntry {\n                        scene_id: Some(\n                            37,\n                        ),\n                        level: Some(\n                            Final,\n                        ),\n                        warp_kind: AnchorHomography,\n                        requested_centre_px: Some(\n                            [\n                                32.0,\n                                18.0,\n                            ],\n                        ),\n                        anchor_px: Some(\n                            [\n                                0.0,\n                                0.0,\n                            ],\n                        ),\n                    },\n                ),\n                None,\n                None,\n                None,\n                None,\n                None,\n                None,\n                None,\n            ],\n            next: 1,\n            len: 1,\n        },\n        status: ShowingCompletedScene,\n    },\n    rgba_fnv1a64: \"27e4f088d4b3c925\",\n    rgba_runs: [\n        RgbaRun {\n            length: 2304,\n            rgba: [\n                161,\n                178,\n                39,\n                255,\n            ],\n        },\n    ],\n}";

#[test]
fn two_palettes_recolour_one_completed_scene_through_the_offscreen_route() {
    let (mut presenter, device) = native_palette_presenter();
    let next_scene_id = presenter.next_scene_id;
    let (classic_receipt, classic) =
        capture_native_palette(&mut presenter, &device, PaletteId::Classic, 101);
    let (ice_receipt, ice) = capture_native_palette(&mut presenter, &device, PaletteId::Ice, 102);

    assert_eq!(classic_receipt.source_scene_id, Some(37));
    assert_eq!(ice_receipt.source_scene_id, classic_receipt.source_scene_id);
    assert_eq!(classic.scene_id, Some(37));
    assert_eq!(ice.scene_id, classic.scene_id);
    assert_eq!(presenter.facts().completed_scene_id, Some(37));
    assert_eq!(presenter.next_scene_id, next_scene_id);
    assert_eq!(classic.route, FrameReadbackRoute::OffscreenRerender);
    assert_eq!(ice.route, FrameReadbackRoute::OffscreenRerender);
    assert_eq!(classic.rgba.len(), ice.rgba.len());
    let (classic_pixels, classic_remainder) = classic.rgba.as_chunks::<4>();
    let (ice_pixels, ice_remainder) = ice.rgba.as_chunks::<4>();
    assert_eq!(classic_remainder, &[]);
    assert_eq!(ice_remainder, &[]);
    assert!(
        classic_pixels
            .iter()
            .zip(ice_pixels)
            .any(|(classic, ice)| classic != ice),
        "the present-time palette changes pixels without a new scene"
    );
}

#[test]
fn sample_classes_reset_and_advance_without_hiding_warmup() {
    let mut tracker = SampleTracker::default();
    assert_eq!(tracker.next(), SampleClass::ColdWarmUp);
    tracker.completed();
    assert_eq!(tracker.next(), SampleClass::PolicyProbe);
    tracker.completed();
    assert_eq!(tracker.next(), SampleClass::Measured);
    tracker.reset();
    assert_eq!(tracker.next(), SampleClass::ColdWarmUp);
}

#[test]
fn clear_plan_is_identity_but_never_samples() {
    let plan = clear_warp_plan(false, true);
    assert_eq!(plan.kind, WarpKind::ClearOnly);
    assert!(!plan.source_valid);
    assert!(plan.exposed);
    assert_eq!(plan.rows[2], [0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn manual_hold_keeps_a_refused_warp_on_the_retained_picture() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 37);
    let refused = crate::WarpPlan {
        refusal_reason: Some(crate::WarpRefusalReason::Matrix),
        ..clear_warp_plan(false, true)
    };
    let held = apply_hold_policy(refused, ledger.retained(), true, BINDING_EXTENT);
    assert_eq!(held.kind, WarpKind::HoldStale);
    assert_eq!(held.source_scene_id, Some(sampled.scene_id));
    assert_eq!(held.source_texture_index, Some(sampled.texture_index));
    assert!(held.source_valid);
    assert!(!held.exposed);
    assert_eq!(held.rows, crate::identity_warp_rows());
    assert_eq!(held.refusal_reason, refused.refusal_reason);

    let mut facts = PresentFacts::default();
    facts.record_warp_plan(&held, Some(0.0));
    assert_eq!(facts.warp_kind, WarpKind::HoldStale);
    assert_eq!(facts.warp_kind.as_str(), "HoldStale");
    assert_eq!(facts.warp_refusal_reason, refused.refusal_reason);

    let mut hot = WarpSourceSlot::default();
    hot.write_hot(&held, false);
    assert_eq!(
        hot.frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(37)
    );
    assert_eq!(hot.accepted_frame(ledger.retained(), ledger.held()), None);
}

#[derive(Clone, Debug)]
struct HoldPolicyCorpusCase {
    name: &'static str,
    plan: crate::WarpPlan,
    retained: Option<crate::SceneFrame>,
    enabled: bool,
    extent: [u32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct HoldPolicyCorpusResult {
    name: &'static str,
    plan: crate::WarpPlan,
}

type HoldPolicyEntry =
    fn(&crate::WarpPlan, Option<&crate::SceneFrame>, bool, [u32; 2]) -> crate::WarpPlan;

fn current_hold_policy_entry(
    plan: &crate::WarpPlan,
    retained: Option<&crate::SceneFrame>,
    enabled: bool,
    extent: [u32; 2],
) -> crate::WarpPlan {
    apply_hold_policy(*plan, retained, enabled, extent)
}

fn extracted_hold_policy_entry(
    plan: &crate::WarpPlan,
    retained: Option<&crate::SceneFrame>,
    enabled: bool,
    extent: [u32; 2],
) -> crate::WarpPlan {
    apply_hold_policy(*plan, retained, enabled, extent)
}

fn named_hold_policy_corpus() -> Vec<HoldPolicyCorpusCase> {
    let mut ledger = SceneLedger::default();
    let retained = promote_binding_scene(&mut ledger, 37);
    let refused = crate::WarpPlan {
        refusal_reason: Some(crate::WarpRefusalReason::ErrorCeiling {
            max_px: 2.0,
            p95_px: 1.5,
        }),
        ..clear_warp_plan(false, true)
    };
    let mut accepted = clear_warp_plan(false, false);
    accepted.kind = WarpKind::AnchorHomography;
    accepted.source_scene_id = Some(retained.scene_id);
    accepted.source_texture_index = Some(retained.texture_index);
    accepted.source_valid = true;
    vec![
        HoldPolicyCorpusCase {
            name: "disabled-refusal",
            plan: refused,
            retained: Some(retained.clone()),
            enabled: false,
            extent: BINDING_EXTENT,
        },
        HoldPolicyCorpusCase {
            name: "missing-retained-frame",
            plan: refused,
            retained: None,
            enabled: true,
            extent: BINDING_EXTENT,
        },
        HoldPolicyCorpusCase {
            name: "accepted-geometric-plan",
            plan: accepted,
            retained: Some(retained.clone()),
            enabled: true,
            extent: BINDING_EXTENT,
        },
        HoldPolicyCorpusCase {
            name: "eligible-refusal-held",
            plan: refused,
            retained: Some(retained.clone()),
            enabled: true,
            extent: BINDING_EXTENT,
        },
        HoldPolicyCorpusCase {
            name: "invalid-destination-extent",
            plan: refused,
            retained: Some(retained),
            enabled: true,
            extent: [0, BINDING_EXTENT[1]],
        },
    ]
}

fn run_hold_policy_corpus(entry: HoldPolicyEntry) -> Vec<HoldPolicyCorpusResult> {
    let corpus = named_hold_policy_corpus();
    corpus
        .iter()
        .map(|case| HoldPolicyCorpusResult {
            name: case.name,
            plan: entry(
                &case.plan,
                case.retained.as_ref(),
                case.enabled,
                case.extent,
            ),
        })
        .collect()
}

#[test]
fn named_paired_hold_policy_corpus_matches_complete_warp_plans() {
    let current = run_hold_policy_corpus(current_hold_policy_entry);
    let extracted = run_hold_policy_corpus(extracted_hold_policy_entry);
    assert_eq!(current, extracted);
    assert_eq!(
        current.iter().map(|result| result.name).collect::<Vec<_>>(),
        [
            "disabled-refusal",
            "missing-retained-frame",
            "accepted-geometric-plan",
            "eligible-refusal-held",
            "invalid-destination-extent",
        ]
    );
    assert_eq!(current[3].plan.kind, WarpKind::HoldStale);
    assert_eq!(current[4].plan.kind, WarpKind::ClearOnly);
}

#[test]
fn incompatible_slice_admits_only_an_unchanged_held_plan_until_replacement() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 38);
    let tilted = construct_plane(ObjectAngles {
        rho_13: ObjectAngles::JULIA.rho_13 + 0.25,
        ..ObjectAngles::JULIA
    })
    .expect("tilted slice constructs");
    assert!(ledger.invalidate_incompatible(
        sampled.iteration_cap,
        sampled.plane_origin_f64,
        tilted,
        sampled.precision_mode,
    ));
    assert!(ledger.retained().is_none());
    let held = ledger
        .held()
        .expect("the incompatible transition keeps one image");
    assert_eq!(held.frame.scene_id, sampled.scene_id);
    assert_eq!(held.partition.plane, sampled.pose.plane);

    let refused = clear_warp_plan(false, true);
    assert_eq!(
        apply_hold_policy(refused, Some(&held.frame), false, BINDING_EXTENT).kind,
        WarpKind::ClearOnly
    );
    let plan = apply_hold_policy(refused, Some(&held.frame), true, BINDING_EXTENT);
    assert_eq!(plan.kind, WarpKind::HoldStale);
    assert_eq!(plan.rows, crate::identity_warp_rows());
    assert!(!plan.exposed);

    let mut geometric = plan;
    geometric.kind = WarpKind::AnchorHomography;
    let mut former_source = WarpSourceSlot::default();
    former_source.write_hot(&geometric, false);
    assert!(
        former_source
            .frame(ledger.retained(), ledger.held())
            .is_none(),
        "a pre-transition geometric slot cannot discover the held frame"
    );

    let mut source = WarpSourceSlot::default();
    source.write_hot(&plan, false);
    assert_eq!(
        source
            .frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(sampled.scene_id)
    );
    assert!(
        source
            .accepted_frame(ledger.retained(), ledger.held())
            .is_none(),
        "a held partition is not an accepted geometric warp source"
    );
    assert!(
        source
            .relief_frame(ledger.retained(), ledger.held())
            .is_none(),
        "a held partition is not a relief-redraw source"
    );

    assert_eq!(
        warp_exposed_fraction(&plan, &held.frame.pose, Some(&held.frame)),
        Some(0.0),
        "the held image covers the synthetic transition without a clear-only region"
    );

    let replacement = promote_binding_scene(&mut ledger, 39);
    assert_ne!(replacement.texture_index, sampled.texture_index);
    assert!(ledger.held().is_none());
    assert_eq!(ledger.retained().map(|frame| frame.scene_id), Some(39));
}

#[test]
fn auto_refusal_still_clears_and_manual_bounded_warp_stays_accepted() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 41);
    let cleared = apply_hold_policy(
        clear_warp_plan(false, true),
        ledger.retained(),
        false,
        BINDING_EXTENT,
    );
    assert_eq!(cleared.kind, WarpKind::ClearOnly);
    assert!(!cleared.source_valid);

    let mut bounded = clear_warp_plan(false, false);
    bounded.kind = WarpKind::AnchorHomography;
    bounded.source_scene_id = Some(sampled.scene_id);
    bounded.source_texture_index = Some(sampled.texture_index);
    bounded.source_valid = true;
    let accepted = apply_hold_policy(bounded, ledger.retained(), true, BINDING_EXTENT);
    assert_eq!(accepted, bounded);

    let mut facts = PresentFacts::default();
    facts.record_warp_plan(
        &apply_hold_policy(
            clear_warp_plan(false, true),
            ledger.retained(),
            true,
            BINDING_EXTENT,
        ),
        Some(0.0),
    );
    assert_eq!(facts.warp_kind, WarpKind::HoldStale);
    facts.record_warp_plan(&accepted, Some(0.0));
    assert_eq!(facts.warp_kind, WarpKind::AnchorHomography);
}

#[test]
fn browser_order_clears_a_hot_plan_after_scene_promotion() {
    let mut ledger = SceneLedger::default();
    assert!(!ledger.invalidate_incompatible(
        64,
        [0.0; 4],
        binding_pose().plane,
        PrecisionMode::PictureFast.as_str(),
    ));
    let sampled = promote_binding_scene(&mut ledger, 41);
    let mut plan = clear_warp_plan(false, false);
    plan.kind = WarpKind::AnchorHomography;
    plan.source_scene_id = Some(sampled.scene_id);
    plan.source_texture_index = Some(sampled.texture_index);
    plan.source_valid = true;
    let mut hot = WarpSourceSlot::default();
    hot.write_hot(&plan, false);
    assert_eq!(
        hot.frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(41)
    );

    let promoted = promote_binding_scene(&mut ledger, 42);
    assert_eq!(promoted.scene_id, 42);
    assert_eq!(
        hot.frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        None
    );
    assert_eq!(HOT_SOURCE_VALID_BYTE_OFFSET, 280);
}

#[test]
fn accepted_exposed_plan_remains_a_source_and_reports_its_clear_share() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 51);
    let mut plan = clear_warp_plan(false, true);
    plan.kind = WarpKind::AnchorHomography;
    plan.source_scene_id = Some(sampled.scene_id);
    plan.source_texture_index = Some(sampled.texture_index);
    plan.source_valid = true;
    plan.rows[0][2] = 16.0;
    let mut hot = WarpSourceSlot::default();
    hot.write_hot(&plan, false);

    assert_eq!(
        hot.frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(51),
        "exposure does not invalidate the accepted source"
    );
    let fraction = warp_exposed_fraction(&plan, &binding_pose(), ledger.retained())
        .expect("the accepted source has an exposure census");
    assert!((fraction - 2.0 / 9.0).abs() <= f64::EPSILON);
}

#[test]
fn relief_redraw_reuses_the_retained_grid_and_scene_uniform_contract() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 61);
    let mut plan = clear_warp_plan(false, true);
    plan.kind = WarpKind::ReliefRedraw;
    plan.source_scene_id = Some(sampled.scene_id);
    plan.source_texture_index = Some(sampled.texture_index);
    plan.source_valid = true;
    let mut hot = WarpSourceSlot::default();
    hot.write_hot(&plan, false);
    assert_eq!(
        hot.relief_frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(61)
    );

    let retained_grid = ledger
        .retained_grid()
        .expect("retained frame owns its record grid");
    let uniform = relief_scene_uniform(retained_grid, &sampled, &sampled.pose, sampled.extent)
        .expect("compatible records form a scene uniform");
    assert_eq!(uniform.grid, [64, 36, RefinementLevel::Final as u32, 64]);
    assert_eq!(uniform.span[0], retained_grid.span.directory_index);
    assert_eq!(uniform.span[1], 64 * 36);
    assert_eq!(uniform.basis_u, sampled.pose.plane.basis_u);
    assert_eq!(uniform.screen_to_plane_row_0, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(uniform.screen_to_plane_row_2, [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(uniform.reserved_0, [0.0; 4]);
    let load = scene_load_color();
    assert_eq!([load.r, load.g, load.b, load.a], [0.0, 0.0, 4.0, 1.0]);
}

#[test]
fn scene_encoder_clear_calls_scene_load_color_at_the_render_pass() {
    const CALL_SITE: &str = "load: wgpu::LoadOp::Clear(scene_load_color()),";
    let source = include_str!("scene.rs");
    let encode_scene = source
        .split_once("pub(super) fn encode_scene(")
        .expect("the scene encoder entry exists")
        .1;
    let render_pass = encode_scene
        .split_once("pass.set_bind_group")
        .expect("the scene render pass ends before its bindings")
        .0;
    assert_eq!(
        render_pass.matches(CALL_SITE).count(),
        1,
        "the scene attachment clear names the shared load colour at its call site"
    );
}

/// What one pixel of the scene attachment holds when the pass is over.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComposedPixel {
    Sky,
    Backdrop,
    Main,
}

/// Runs the pass's real depth-range and stencil state over one pixel, in draw order.
///
/// Nothing here knows the intended answer: it reads `scene_draw_order`, `scene_stencil`,
/// `stencil_reference` and `SCENE_DEPTH_COMPARE` — the same values the pipelines and the
/// encoder are built from — and applies the fixed-function rules to them.
fn compose_pixel(main: Option<f32>, backdrop: Option<f32>) -> ComposedPixel {
    let mut colour = ComposedPixel::Sky;
    let mut depth = 1.0_f32;
    let mut stencil = BACKDROP_STENCIL;
    for layer in scene_draw_order(backdrop.is_some()) {
        let (fragment, drawn) = match layer {
            SceneLayer::Main => (main, ComposedPixel::Main),
            SceneLayer::Backdrop => (backdrop, ComposedPixel::Backdrop),
        };
        let Some(fragment) = fragment else {
            continue;
        };
        let state = scene_stencil(*layer);
        let reference = stencil_reference(*layer);
        let face = state.front;
        let [minimum_depth, maximum_depth] = scene_depth_range(*layer, backdrop.is_some());
        let fragment = (maximum_depth - minimum_depth).mul_add(fragment, minimum_depth);
        let operation = if !compares(
            face.compare,
            f64::from(reference & state.read_mask),
            f64::from(stencil & state.read_mask),
        ) {
            face.fail_op
        } else if !compares(SCENE_DEPTH_COMPARE, fragment.into(), depth.into()) {
            face.depth_fail_op
        } else {
            depth = fragment;
            colour = drawn;
            face.pass_op
        };
        let written = stencil_write(operation, reference, stencil);
        stencil = (stencil & !state.write_mask) | (written & state.write_mask);
    }
    colour
}

/// The WebGPU comparison functions, over a new value and the stored one.
#[allow(
    clippy::float_cmp,
    reason = "the fixed-function equality test is exact by definition"
)]
fn compares(compare: wgpu::CompareFunction, new: f64, stored: f64) -> bool {
    match compare {
        wgpu::CompareFunction::Never => false,
        wgpu::CompareFunction::Less => new < stored,
        wgpu::CompareFunction::Equal => new == stored,
        wgpu::CompareFunction::LessEqual => new <= stored,
        wgpu::CompareFunction::Greater => new > stored,
        wgpu::CompareFunction::NotEqual => new != stored,
        wgpu::CompareFunction::GreaterEqual => new >= stored,
        wgpu::CompareFunction::Always => true,
    }
}

/// The WebGPU stencil operations, before the write mask is applied.
fn stencil_write(operation: wgpu::StencilOperation, reference: u32, stored: u32) -> u32 {
    match operation {
        wgpu::StencilOperation::Keep => stored,
        wgpu::StencilOperation::Zero => 0,
        wgpu::StencilOperation::Replace => reference,
        wgpu::StencilOperation::Invert => !stored & 0xff,
        wgpu::StencilOperation::IncrementClamp => stored.saturating_add(1).min(0xff),
        wgpu::StencilOperation::DecrementClamp => stored.saturating_sub(1),
        wgpu::StencilOperation::IncrementWrap => (stored + 1) & 0xff,
        wgpu::StencilOperation::DecrementWrap => stored.wrapping_sub(1) & 0xff,
    }
}

/// The composition rule, driven through the state the pass actually carries.
///
/// The two grids are independent samplings of the same field, so the coarse backdrop is assigned
/// the farther half of the viewport depth range and the main grid the nearer half. Each grid still
/// depth-orders its own folds, while the main owns every pixel it reaches.
#[test]
fn the_backdrop_shows_only_where_the_main_grid_has_no_fragment() {
    for main in [0.1_f32, 0.5, 0.9] {
        for backdrop in [0.05_f32, 0.5, 0.95] {
            assert_eq!(
                compose_pixel(Some(main), Some(backdrop)),
                ComposedPixel::Main,
                "main at {main} lost to a backdrop chord at {backdrop}"
            );
        }
    }
    assert_eq!(
        compose_pixel(None, Some(0.5)),
        ComposedPixel::Backdrop,
        "a pixel the main grid misses must show the backdrop"
    );
    assert_eq!(
        compose_pixel(Some(0.5), None),
        ComposedPixel::Main,
        "a pose with no backdrop still draws its main grid"
    );
    assert_eq!(
        compose_pixel(None, None),
        ComposedPixel::Sky,
        "neither grid reaching the pixel leaves the distinct sky"
    );
}

/// The backdrop keeps its own internal ordering: it is depth-tested against itself.
#[test]
fn the_backdrop_is_ordered_against_itself_by_depth() {
    assert_eq!(SCENE_DEPTH_COMPARE, wgpu::CompareFunction::LessEqual);
    assert_eq!(scene_stencil(SceneLayer::Backdrop).write_mask, 0);
    assert!(compares(SCENE_DEPTH_COMPARE, 0.25, 0.75));
    assert!(!compares(SCENE_DEPTH_COMPARE, 0.75, 0.25));
}

#[test]
fn backdrop_then_main_uses_disjoint_depth_ranges() {
    assert_eq!(
        scene_draw_order(true),
        [SceneLayer::Backdrop, SceneLayer::Main]
    );
    assert_eq!(scene_depth_range(SceneLayer::Backdrop, true), [0.5, 1.0]);
    assert_eq!(scene_depth_range(SceneLayer::Main, true), [0.0, 0.5]);
    assert_eq!(scene_depth_range(SceneLayer::Main, false), [0.0, 1.0]);
}

/// The status-one census counts the MAIN grid's records, with or without a backdrop attached.
///
/// The census pass binds one scene group and draws one full-screen triangle over it. The
/// backdrop split that binding into a two-slot array, so the census would silently have started
/// counting whichever slot the rebase left in place. It binds slot zero and names nothing of
/// the backdrop: a Final main therefore publishes the same glitch count either way.
#[test]
fn the_glitch_census_reads_the_main_grid_alone() {
    let source = include_str!("census.rs");
    let start = source
        .find("fn encode_glitch_count(")
        .expect("the census encoder exists");
    let body = &source[start..];
    let end = body
        .find("\npub(super) fn ")
        .expect("the census encoder ends");
    let body = &body[..end];
    assert!(body.contains("pass.set_bind_group(1, &gpu.scene_groups[0], &[hot_offset]);"));
    assert!(
        !body.contains("scene_groups[1]") && !body.to_lowercase().contains("backdrop_indices"),
        "the census must never draw or bind the backdrop layer"
    );
}

/// A backdrop attaching or expiring drops the scene in flight and never the held picture.
///
/// `set_main` treats a changed backdrop as a replaced selection, which is right: an in-flight
/// scene was composed for the other backdrop and is stale. But manual mode holds the retained
/// picture across a refused warp, and a coverage layer arriving or going stale is not a reason
/// to take that picture away — during a drag it happens repeatedly.
#[test]
fn a_changed_backdrop_never_clears_a_held_picture() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 53);
    // The only thing a backdrop attach or expiry reaches in `set_main`.
    ledger.mark_replaced();
    assert_eq!(
        ledger.retained().map(|frame| frame.scene_id),
        Some(sampled.scene_id),
        "the retained picture survives a replaced selection"
    );
    let held = apply_hold_policy(
        clear_warp_plan(false, true),
        ledger.retained(),
        true,
        BINDING_EXTENT,
    );
    assert_eq!(held.kind, WarpKind::HoldStale);
    assert_eq!(held.source_scene_id, Some(sampled.scene_id));
    assert!(held.source_valid);

    let source = include_str!("../device.rs");
    let start = source
        .find("pub fn set_main(")
        .expect("the main publication exists");
    let body = &source[start..];
    let end = body
        .find("self.main = Some(main);")
        .expect("the main publication ends");
    let body = &body[..end];
    assert!(body.contains("scene_selection_replaced(previous, &main)"));
    assert!(
        body.find("scene_selection_replaced(previous, &main)")
            < body.find("self.ledger.invalidate_incompatible("),
        "the backdrop comparison belongs to the selection test, never to the clear"
    );
    assert!(source.contains("previous.backdrop != current.backdrop"));
}

#[test]
fn palette_is_not_a_scene_selection_key_and_is_uploaded_before_shade() {
    let previous = binding_main();
    let mut current = previous.clone();
    current.state.palette_id = PaletteId::Ice as u32;
    assert!(!scene_selection_replaced(&previous, &current));

    let warp = include_str!("warp.rs");
    let palette_write = warp
        .find("write_palette(&self.queue, &self.gpu, selected.1);")
        .expect("the current palette is uploaded for each present");
    let shade = warp
        .find("encode_shade(&mut encoder, &self.gpu, state.surface_view);")
        .expect("every present runs the sole shade pass");
    let submit = warp
        .find("self.queue.submit(")
        .expect("the presentation submission exists");
    assert!(palette_write < shade && shade < submit);

    let scene = crate::scene_shader(ember_lab_heap::DialectLimits {
        descriptor_capacity: 2,
        span_capacity: 2,
        handle_capacity: 4,
    });
    for source in [scene.as_str(), crate::warp_shader()] {
        assert!(!source.contains("PaletteUniform"));
        assert!(!source.contains("palette."));
    }
    assert!(
        crate::shade_shader()
            .expect("shade template renders")
            .source()
            .contains("palette.")
    );
    assert!(!include_str!("scene/submit.rs").contains("selected_palette"));
}

/// The stamp needs a stencil aspect, and the engine's floor has to admit the format.
#[test]
fn the_scene_depth_target_carries_a_stencil_aspect() {
    assert_eq!(DEPTH_FORMAT, wgpu::TextureFormat::Depth24PlusStencil8);
    assert!(DEPTH_FORMAT.has_stencil_aspect());
    assert!(DEPTH_FORMAT.has_depth_aspect());
}

#[test]
fn relief_redraw_disocclusion_is_a_clear_value() {
    let disocclusion = warp_load_color();
    assert_eq!(
        [
            disocclusion.r,
            disocclusion.g,
            disocclusion.b,
            disocclusion.a
        ],
        [0.0, 0.0, 4.0, 1.0]
    );
}

#[test]
fn a_presented_relief_plan_survives_only_while_final_leases_its_records() {
    use super::warp::retain_relief_plan_during_scene;

    assert!(retain_relief_plan_during_scene(
        WarpKind::ReliefRedraw,
        false,
        true,
        Some(WarpKind::ReliefRedraw),
    ));
    assert!(!retain_relief_plan_during_scene(
        WarpKind::ReliefRedraw,
        true,
        true,
        Some(WarpKind::ReliefRedraw),
    ));
    assert!(!retain_relief_plan_during_scene(
        WarpKind::ReliefRedraw,
        false,
        false,
        Some(WarpKind::ReliefRedraw),
    ));
    assert!(!retain_relief_plan_during_scene(
        WarpKind::ReliefRedraw,
        false,
        true,
        Some(WarpKind::AnchorHomography),
    ));
}

#[test]
fn relief_redraw_refuses_a_retained_grid_whose_extent_no_longer_matches_its_frame() {
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 62);
    let mut retained_grid = ledger
        .retained_grid()
        .expect("retained frame owns its record grid")
        .clone();
    retained_grid.width /= 2;
    retained_grid.height /= 2;
    assert!(relief_scene_uniform(&retained_grid, &sampled, &sampled.pose, sampled.extent).is_err());
}

#[test]
fn relief_redraw_accepts_records_in_the_idle_live_main_grid() {
    let main = binding_main();
    let mut ledger = SceneLedger::default();
    let sampled = promote_binding_scene(&mut ledger, 63);
    assert_eq!(
        ledger
            .retained_grid()
            .expect("the promoted Final keeps its records")
            .span
            .directory_index,
        main.grid.span.directory_index
    );
    assert!(relief_scene_uniform(&main.grid, &sampled, &sampled.pose, sampled.extent).is_ok());
}

#[test]
fn relief_redraw_publishes_the_planned_fraction_without_filling_from_a_backdrop() {
    let pose = binding_pose();
    let relief = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        source_valid: true,
        exposed: true,
        predicted_exposed_fraction: Some(0.071_952_160_494),
        ..clear_warp_plan(false, true)
    };
    assert_eq!(
        planned_exposed_fraction(&relief, Some(&pose), None),
        relief.predicted_exposed_fraction
    );
    let mut facts = PresentFacts::default();
    facts.record_warp_plan(&relief, relief.predicted_exposed_fraction);
    assert_eq!(
        facts.warp_exposed_fraction,
        relief.predicted_exposed_fraction
    );
    assert_eq!(facts.warp_max_error_px, None);
    assert_eq!(facts.warp_p95_error_px, None);
}

#[test]
fn every_gpu_dynamic_offset_comes_from_the_opaque_slot() {
    let mut source = String::from(include_str!("../device.rs"));
    source.push_str(include_str!("census.rs"));
    source.push_str(include_str!("redraw.rs"));
    source.push_str(include_str!("scene.rs"));
    source.push_str(include_str!("scene/submit.rs"));
    source.push_str(include_str!("warp.rs"));
    let accessor = [".dynamic_", "offset()"].concat();
    let bypass = ["index()", " * self.gpu.hot_stride"].concat();
    assert_eq!(source.matches(&accessor).count(), 7);
    assert!(!source.contains(&bypass));
}

const HEAP_DESCRIPTOR_LAYOUT: &str = "create_heap_layout descriptor/static_uniform_entry";
const HEAP_DIRECTORY_LAYOUT: &str = "create_heap_layout directory/static_uniform_entry";
const SCENE_UNIFORM_LAYOUT: &str = "create_scene_layout scene/static_uniform_entry";
const SCENE_HOT_LAYOUT: &str = "create_scene_layout HOT/hot_uniform_entry";
const WARP_HOT_LAYOUT: &str = "create_warp_hot_layout HOT/hot_uniform_entry";
const WARP_SCENE_LAYOUT: &str = "create_warp_hot_layout scene/static_uniform_entry";
const PALETTE_LAYOUT: &str = "create_palette_layout/static_uniform_entry";

type UniformLayoutFact = (&'static str, &'static str, u32, bool, u64);

fn uniform_layout_fact(
    constructor: &'static str,
    class: &'static str,
    entry: &wgpu::BindGroupLayoutEntry,
) -> UniformLayoutFact {
    assert_eq!(
        entry.visibility,
        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        "{constructor} visibility"
    );
    assert!(entry.count.is_none(), "{constructor} is not an array");
    let wgpu::BindingType::Buffer {
        ty,
        has_dynamic_offset,
        min_binding_size,
    } = &entry.ty
    else {
        panic!("{constructor} must construct a uniform buffer entry");
    };
    assert!(matches!(ty, wgpu::BufferBindingType::Uniform));
    assert_eq!(
        *has_dynamic_offset,
        class == "dynamic",
        "{constructor} structural class"
    );
    (
        constructor,
        class,
        entry.binding,
        *has_dynamic_offset,
        min_binding_size.as_ref().map_or(0, |size| size.get()),
    )
}

#[test]
fn every_uniform_layout_lists_its_structural_constructor() {
    let source = include_str!("uniforms.rs");
    assert_eq!(
        source.matches("static_uniform_entry(").count(),
        6,
        "five static bindings plus the constructor definition"
    );
    assert_eq!(
        source.matches("hot_uniform_entry(").count(),
        3,
        "two HOT bindings plus the constructor definition"
    );
    assert_eq!(
        source.matches("has_dynamic_offset:").count(),
        2,
        "only the static and HOT constructors set the field"
    );

    let heap = super::uniforms::heap_layout_entries(ember_lab_heap::DialectLimits {
        descriptor_capacity: 8,
        span_capacity: 4,
        handle_capacity: 4,
    })
    .expect("the enumerated heap layout sizes fit");
    let scene = super::uniforms::scene_layout_entries();
    let warp = super::uniforms::warp_hot_layout_entries();
    let palette = super::uniforms::palette_layout_entries();
    assert!(matches!(&heap[0].ty, wgpu::BindingType::Texture { .. }));

    let actual = [
        uniform_layout_fact(HEAP_DESCRIPTOR_LAYOUT, "static", &heap[1]),
        uniform_layout_fact(HEAP_DIRECTORY_LAYOUT, "static", &heap[2]),
        uniform_layout_fact(SCENE_UNIFORM_LAYOUT, "static", &scene[0]),
        uniform_layout_fact(SCENE_HOT_LAYOUT, "dynamic", &scene[1]),
        uniform_layout_fact(WARP_HOT_LAYOUT, "dynamic", &warp[0]),
        uniform_layout_fact(WARP_SCENE_LAYOUT, "static", &warp[1]),
        uniform_layout_fact(PALETTE_LAYOUT, "static", &palette[0]),
    ];
    assert_eq!(
        actual,
        [
            (HEAP_DESCRIPTOR_LAYOUT, "static", 1, false, 128),
            (HEAP_DIRECTORY_LAYOUT, "static", 2, false, 80),
            (
                SCENE_UNIFORM_LAYOUT,
                "static",
                0,
                false,
                u64::from(SCENE_PAYLOAD_BYTES),
            ),
            (
                SCENE_HOT_LAYOUT,
                "dynamic",
                1,
                true,
                u64::from(HOT_PAYLOAD_BYTES),
            ),
            (
                WARP_HOT_LAYOUT,
                "dynamic",
                0,
                true,
                u64::from(HOT_PAYLOAD_BYTES),
            ),
            (
                WARP_SCENE_LAYOUT,
                "static",
                1,
                false,
                u64::from(SCENE_PAYLOAD_BYTES),
            ),
            (PALETTE_LAYOUT, "static", 0, false, 48),
        ]
    );
}

/// One completed picture at `extent`, shaped as the ledger promotes it.
fn frame_at_extent(scene_id: u64, extent: [u32; 2]) -> crate::SceneFrame {
    let mut pose = binding_pose();
    pose.grid_width = extent[0];
    pose.grid_height = extent[1];
    crate::SceneFrame {
        scene_id,
        pose,
        iteration_cap: 64,
        level: RefinementLevel::Preview,
        extent,
        texture_index: 0,
        centre_revision: 1,
        plane_origin_f64: [0.0; 4],
        precision_mode: PrecisionMode::PictureFast.as_str(),
        measurement: binding_measurement(scene_id),
    }
}

/// The seam's mirror of the warp fragment's source lookup, named the way these tests read.
fn warp_source_uv(
    rows: [[f32; 4]; 3],
    destination_extent: [u32; 2],
    source_extent: [u32; 2],
    chart: [f64; 2],
) -> Option<[f64; 2]> {
    crate::LatticePair::new(source_extent, destination_extent)?.source_uv(rows, chart)
}

/// A held picture drawn at a reduced extent must fill the destination, not sit in its centre.
///
/// A morph across slices sends the completed picture to the held slot and the app asks for a hold
/// while the next scene is in flight. The held picture was drawn at the `PictureFast` Preview
/// extent (a 960 by 540 surface at divisor 8), and the destination lattice is the full surface.
/// The fragment normalises the mapped source pixel by the source texture's dimensions, so rows
/// that carry no extent ratio place the whole picture inside the central 120 by 68 pixels and
/// paint the rest clear: a thumbnail at a scale the geometry does not have. Under the rendering
/// rule a moving frame may be very inaccurate but never wrong, and a picture at the wrong scale
/// in the wrong place asserts geometry that does not exist.
#[test]
fn a_held_reduced_extent_picture_fills_the_destination_rather_than_a_centred_thumbnail() {
    let source_extent = [120, 68];
    let destination_extent = [960, 540];
    let held = frame_at_extent(57, source_extent);
    let plan = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&held),
        true,
        destination_extent,
    );
    assert_eq!(plan.kind, WarpKind::HoldStale);
    assert!(plan.source_valid);
    for chart in [
        [-1.0, -1.0],
        [1.0, -1.0],
        [-1.0, 1.0],
        [1.0, 1.0],
        [0.0, 0.0],
    ] {
        let uv = warp_source_uv(plan.rows, destination_extent, source_extent, chart)
            .expect("a hold keeps every destination point in front of the source");
        assert!(
            uv.iter().all(|value| (-1.0e-4..=1.000_1).contains(value)),
            "chart {chart:?} samples {uv:?}, outside the held picture: the picture is shown at \
             its own texel size in the centre of the destination"
        );
    }
}

/// The reverse mismatch magnifies a centre crop, which is the same wrongness the other way.
#[test]
fn a_held_full_extent_picture_is_not_magnified_into_a_centre_crop() {
    let source_extent = [960, 540];
    let destination_extent = [120, 68];
    let held = frame_at_extent(58, source_extent);
    let plan = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&held),
        true,
        destination_extent,
    );
    assert_eq!(plan.kind, WarpKind::HoldStale);
    let corner = warp_source_uv(plan.rows, destination_extent, source_extent, [1.0, 1.0])
        .expect("a hold keeps the destination corner in front of the source");
    assert!(
        corner[0] > 0.999 && corner[1] < 0.001,
        "the destination corner samples {corner:?} rather than the source corner: the hold shows \
         a magnified centre crop instead of the whole picture"
    );
}

/// Equal extents keep the hold exactly identity, which is what every earlier hold measured.
#[test]
fn a_held_picture_at_the_destination_extent_holds_by_identity() {
    let extent = [960, 540];
    let held = frame_at_extent(59, extent);
    let plan = apply_hold_policy(clear_warp_plan(false, true), Some(&held), true, extent);
    assert_eq!(plan.kind, WarpKind::HoldStale);
    assert_eq!(plan.rows, crate::identity_warp_rows());
}

/// A hold whose scale cannot be stated is refused rather than placed somewhere.
#[test]
fn a_hold_with_an_unusable_extent_stays_a_clear_plan() {
    assert_eq!(crate::LatticePair::new([0, 68], [960, 540]), None);
    assert_eq!(crate::LatticePair::new([120, 68], [960, 0]), None);
    assert_eq!(
        crate::LatticePair::new([960, 540], [960, 540]).and_then(crate::LatticePair::covering_rows),
        Some(crate::identity_warp_rows())
    );

    let mut degenerate = frame_at_extent(60, [120, 68]);
    degenerate.extent = [120, 0];
    let plan = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&degenerate),
        true,
        [960, 540],
    );
    assert_eq!(plan.kind, WarpKind::ClearOnly);
    assert!(!plan.source_valid);
}

/// The warp names its own destination lattice instead of inheriting the last scene's.
///
/// The fragment divides the chart by `scene.grid.xy`, and that buffer is last written by a scene
/// submission at MAIN's extent or by a relief redraw at the retained records' extent. Either can
/// disagree with the pose a plan was built against, and the plan rows would then be read on a
/// lattice they were not written for.
#[test]
fn the_image_warp_states_the_destination_lattice_it_planned_against() {
    let source = include_str!("warp.rs");
    let find = |needle: &str| {
        source
            .find(needle)
            .unwrap_or_else(|| panic!("the warp submission contains {needle}"))
    };
    let stated = find("self.write_warp_destination_extent(warp_destination_extent);");
    let encoded = find("encode_image_warp(");
    let submitted = find("self.queue.submit(");
    assert!(
        stated < encoded,
        "the destination lattice is stated before the pass that reads it is encoded"
    );
    assert!(
        encoded < submitted,
        "the image warp is encoded before the submission that carries both"
    );
    assert!(
        source.contains("SCENE_GRID_BYTE_OFFSET"),
        "the destination lattice is written at the named scene-grid offset"
    );
    assert_eq!(SCENE_GRID_BYTE_OFFSET, 0);
    assert_eq!(
        destination_extent(Some(&binding_pose()), None),
        BINDING_EXTENT
    );
    assert_eq!(
        destination_extent(None, Some(&binding_main())),
        BINDING_EXTENT
    );
    assert_eq!(destination_extent(None, None), [0, 0]);
}

/// The lattice pair a plan maps between is a property of every plan kind, not of the hold alone.
///
/// A plan's rows are read by the warp fragment as destination pixels in, source pixels out, and
/// the fragment then normalises the source pixel by the source texture's own dimensions. Two
/// extents therefore decide where a plan puts the picture: the destination lattice written to
/// `scene.grid.xy`, and the source texture's extent, which is the delivered extent of the scene
/// drawn into it. A plan built from one extent, or from identity by default, asserts that the two
/// are equal; nothing in the presenter makes them equal.
///
/// A scene is submitted at `main.grid`'s extent while its pose carries the surface lattice, and no
/// check relates the two. So a completed picture can be retained at the `PictureFast` divisor
/// extent with a pose whose grid is the full surface, and `renders_same_picture` — which compares
/// the two poses' grids and nothing else — then admits the identity `exact_self` plan. Identity on
/// that pair is the morph thumbnail again, reached through the planner rather than through the
/// hold: the whole picture inside the central 120 by 68 pixels of a 960 by 540 destination, clear
/// around it. Under the rendering rule a moving frame may be very inaccurate but never wrong, and
/// a frame at the wrong scale is wrong.
#[test]
fn a_plan_states_the_lattice_pair_it_maps_between_whatever_its_kind() {
    let destination_extent = [960, 540];
    let source_extent = [120, 68];
    let mut pose = binding_pose();
    pose.grid_width = destination_extent[0];
    pose.grid_height = destination_extent[1];
    let mut frame = frame_at_extent(61, source_extent);
    frame.pose = pose;
    let plan = crate::Warp::reproject(&frame, &pose, &pose);
    assert_eq!(plan.kind, WarpKind::AnchorHomography);
    assert!(plan.source_valid);
    for chart in [
        [-1.0, -1.0],
        [1.0, -1.0],
        [-1.0, 1.0],
        [1.0, 1.0],
        [0.0, 0.0],
    ] {
        let uv = warp_source_uv(plan.rows, destination_extent, source_extent, chart)
            .expect("a plan keeps every destination point in front of the source");
        assert!(
            uv.iter().all(|value| (-1.0e-4..=1.000_1).contains(value)),
            "chart {chart:?} samples {uv:?}, outside the source picture: the plan was built from \
             the pose lattice alone and ignores the extent the picture was delivered at"
        );
    }
}

/// The extent pairings the refinement ladder and the surface actually produce.
///
/// `960x540` is the surface, and the `PictureFast` divisors take it down the ladder: 2 gives
/// 480x270, 4 gives 240x135, 8 gives 120x68. `1920x1080` is a device-pixel-ratio-2 surface, and
/// `121x68` is there because a reduced extent rounds up per axis and an odd width is what the
/// rounding produces; every pairing is checked in both directions because a hold from a coarse
/// picture onto a fine lattice and a hold from a fine picture onto a coarse one are different
/// arithmetic with the same wrongness available at each end.
const LADDER_EXTENTS: [[u32; 2]; 6] = [
    [960, 540],
    [480, 270],
    [240, 135],
    [120, 68],
    [1_920, 1_080],
    [121, 68],
];

/// Every ordered pair of ladder extents, the equal ones included.
fn ladder_pairs() -> impl Iterator<Item = ([u32; 2], [u32; 2])> {
    LADDER_EXTENTS
        .into_iter()
        .flat_map(|source| LADDER_EXTENTS.map(move |destination| (source, destination)))
}

/// A pose on a named lattice, otherwise the binding fixture.
fn pose_on(extent: [u32; 2]) -> Pose {
    let mut pose = binding_pose();
    pose.grid_width = extent[0];
    pose.grid_height = extent[1];
    pose
}

/// A completed picture delivered at `source_extent` whose pose sits on `pose_extent`.
fn frame_on(scene_id: u64, source_extent: [u32; 2], pose_extent: [u32; 2]) -> crate::SceneFrame {
    let mut frame = frame_at_extent(scene_id, source_extent);
    frame.pose = pose_on(pose_extent);
    frame
}

#[test]
fn preview_relief_redraw_maps_the_delivery_lattice_into_the_destination_chart() {
    let mut grid = binding_main().grid;
    grid.width = 8;
    grid.height = 5;
    grid.level = RefinementLevel::Preview;
    let source = frame_on(90, [8, 5], [64, 36]);
    let mut destination = pose_on([64, 36]);
    destination.zoom_log2 = 1.0;
    destination.centre_from_reference_px = [4.0, -2.0];
    destination.view.height_scale = 2.0;
    let uniform = relief_scene_uniform(&grid, &source, &destination, [960, 540])
        .expect("the reduced retained grid composes into the destination chart");

    assert_eq!(uniform.grid[..2], [8, 5]);
    let rows = [
        f64::from(uniform.screen_to_plane_row_0[0]),
        f64::from(uniform.screen_to_plane_row_0[1]),
        f64::from(uniform.screen_to_plane_row_0[2]),
        f64::from(uniform.screen_to_plane_row_1[0]),
        f64::from(uniform.screen_to_plane_row_1[1]),
        f64::from(uniform.screen_to_plane_row_1[2]),
        f64::from(uniform.screen_to_plane_row_2[0]),
        f64::from(uniform.screen_to_plane_row_2[1]),
        f64::from(uniform.screen_to_plane_row_2[2]),
    ];
    let uniform_chart = crate::apply_homography(rows, [1.0, -1.0])
        .expect("the known source vertex reaches the destination chart");
    assert!((uniform_chart[0] - 1.5).abs() < 1.0e-6);
    assert!((uniform_chart[1] + 1.55).abs() < 1.0e-6);
    let chart_scale =
        4.0 * f64::from(uniform.screen_to_plane_row_2[3]) / f64::from(uniform.grid[0]);
    let display = uniform_chart.map(|coordinate| chart_scale * coordinate);
    assert!((display[0] - 0.75).abs() < 1.0e-6);
    assert!((display[1] + 0.775).abs() < 1.0e-6);
    assert_eq!(super::redraw::RELIEF_STRETCH_GUARD, None);
    assert_eq!(uniform.reserved_0, [0.0; 4]);
    assert_eq!(super::redraw::RELIEF_STRETCH_GUARD_CANDIDATE, 1.0);
    let guarded = relief_scene_uniform_with_guard(
        &grid,
        &source,
        &destination,
        [960, 540],
        Some(super::redraw::RELIEF_STRETCH_GUARD_CANDIDATE),
    )
    .expect("the measured candidate packs its enabled guard lane");
    assert_eq!(guarded.reserved_0, [1.0, 960.0, 540.0, 1.0]);

    let redraw = crate::relief_redraw_source_pose(&source.pose, source.extent, &destination)
        .expect("the source delivery lattice composes into the destination pose");
    let record = [12.0, 1.0, 0.0, 0.0];
    let actual = crate::project_scene_record_vertex(
        &redraw,
        [1.0, -1.0],
        record,
        source.iteration_cap,
        crate::CLASSIC_PALETTE,
    )
    .expect("the record height is valid")
    .expect("the redraw vertex projects");
    let destination_pixel_scale = f64::from(destination.grid_width) / f64::from(source.extent[0]);
    let actual_destination_px = actual
        .0
        .map(|coordinate| coordinate * destination_pixel_scale);
    let expected = crate::project_scene_record_vertex(
        &destination,
        [12.0, -12.4],
        record,
        source.iteration_cap,
        crate::CLASSIC_PALETTE,
    )
    .expect("the record height is valid")
    .expect("the destination vertex projects");
    assert!((actual_destination_px[0] - expected.0[0]).abs() < 1.0e-9);
    assert!((actual_destination_px[1] - expected.0[1]).abs() < 1.0e-9);
    assert!((actual.1 - expected.1).abs() < 1.0e-9);
}

/// Reprojects `frame` onto `to_pose` through the same planner entry as the presenter.
fn reproject_onto(frame: &crate::SceneFrame, to_pose: &Pose) -> crate::WarpPlan {
    crate::Warp::reproject(frame, &frame.pose, to_pose)
}

/// Independently reproduces record-chart containment so fixture decisions cannot drift silently.
fn reproduced_source_covers_destination(
    plan: &crate::WarpPlan,
    source: &crate::SceneFrame,
    requested: &Pose,
) -> bool {
    if plan.kind == WarpKind::ReliefRedraw {
        return plan.source_valid && reproduced_chart_covers_destination(source, requested);
    }
    if !matches!(
        plan.refusal_reason,
        Some(
            crate::WarpRefusalReason::ErrorCeiling { .. }
                | crate::WarpRefusalReason::ErrorCorpus { .. }
                | crate::WarpRefusalReason::ReliefExposure { .. }
        )
    ) {
        return false;
    }
    reproduced_chart_covers_destination(source, requested)
}

fn reproduced_chart_covers_destination(source: &crate::SceneFrame, requested: &Pose) -> bool {
    if source.extent.contains(&0) {
        return false;
    }
    let Some(chart) = crate::planner::source_to_destination_chart(&source.pose, requested) else {
        return false;
    };
    let determinant = chart[0].mul_add(chart[4], -chart[1] * chart[3]);
    if !determinant.is_finite() || determinant.abs() <= 1.0e-12 {
        return false;
    }
    let source_width = f64::from(source.pose.grid_width);
    let source_height = f64::from(source.pose.grid_height);
    let source_half = [
        crate::SOURCE_TEXEL_REACH_PX.mul_add(
            source_width / f64::from(source.extent[0]),
            source_width * 0.5,
        ),
        crate::SOURCE_TEXEL_REACH_PX.mul_add(
            source_height / f64::from(source.extent[1]),
            source_height * 0.5,
        ),
    ];
    let destination_half = [
        f64::from(requested.grid_width) * 0.5,
        f64::from(requested.grid_height) * 0.5,
    ];
    [
        [-destination_half[0], -destination_half[1]],
        [destination_half[0], -destination_half[1]],
        [-destination_half[0], destination_half[1]],
        [destination_half[0], destination_half[1]],
    ]
    .into_iter()
    .all(|destination| {
        let translated = [destination[0] - chart[2], destination[1] - chart[5]];
        let retained = [
            chart[4].mul_add(translated[0], -chart[1] * translated[1]) / determinant,
            (-chart[3]).mul_add(translated[0], chart[0] * translated[1]) / determinant,
        ];
        retained
            .iter()
            .zip(source_half)
            .all(|(value, half)| value.is_finite() && (-half..=half).contains(value))
    })
}

fn assert_coverage_decision(
    name: &str,
    plan: &crate::WarpPlan,
    source: &crate::SceneFrame,
    requested: &Pose,
    expected: bool,
) {
    let reproduced = reproduced_source_covers_destination(plan, source, requested);
    assert_eq!(
        redraw_source_covers_destination(plan, source, requested),
        reproduced,
        "production coverage drifted from the app reproduction for {name}"
    );
    assert_eq!(reproduced, expected, "the reproduced {name} decision moved");
}

#[test]
fn production_redraw_coverage_matches_the_reproduction_oracle() {
    let extent = [960, 540];
    let final_source = frame_on(93, extent, extent);
    let mut preview_source = frame_on(94, [120, 68], extent);
    preview_source.level = RefinementLevel::Preview;

    let mut covered = pose_on(extent);
    covered.zoom_log2 = 1.0;
    let mut in_bounds = covered;
    // At this two-times zoom the inverse map sends the frame to x=0..480 and y=-270..0: its right
    // and lower edges reach the retained boundary without crossing it.
    in_bounds.centre_from_reference_px = [480.0, -270.0];
    let mut outside = covered;
    // Sixty more source pixels of translation cross both retained boundaries.
    outside.centre_from_reference_px = [600.0, -360.0];

    let admitted_final_covered = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        destination_pose: Some(covered),
        ..reproject_onto(&final_source, &covered)
    };
    let admitted_final_outside = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        destination_pose: Some(outside),
        ..reproject_onto(&final_source, &outside)
    };
    let admitted_final_in_bounds = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        destination_pose: Some(in_bounds),
        ..reproject_onto(&final_source, &in_bounds)
    };
    let admitted_preview_covered = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        destination_pose: Some(covered),
        ..reproject_onto(&preview_source, &covered)
    };
    let refused_covered = crate::WarpPlan {
        refusal_reason: Some(crate::WarpRefusalReason::ErrorCeiling {
            max_px: 2.0,
            p95_px: 1.5,
        }),
        ..clear_warp_plan(false, true)
    };
    let refused_outside = crate::WarpPlan {
        refusal_reason: Some(crate::WarpRefusalReason::ReliefExposure {
            predicted_fraction: 0.09,
            limit: 0.08,
        }),
        ..clear_warp_plan(false, true)
    };
    let hard_refusal = crate::WarpPlan {
        refusal_reason: Some(crate::WarpRefusalReason::Matrix),
        ..clear_warp_plan(false, true)
    };

    assert_coverage_decision(
        "Final admitted covered zoom",
        &admitted_final_covered,
        &final_source,
        &covered,
        true,
    );
    assert_coverage_decision(
        "Final admitted in-bounds translation",
        &admitted_final_in_bounds,
        &final_source,
        &in_bounds,
        true,
    );
    assert_coverage_decision(
        "Final admitted exposed translation",
        &admitted_final_outside,
        &final_source,
        &outside,
        false,
    );
    assert_coverage_decision(
        "Preview admitted covered zoom",
        &admitted_preview_covered,
        &preview_source,
        &covered,
        true,
    );
    assert_coverage_decision(
        "Final corpus refusal covered zoom",
        &refused_covered,
        &final_source,
        &covered,
        true,
    );
    assert_coverage_decision(
        "Preview exposure refusal exposed translation",
        &refused_outside,
        &preview_source,
        &outside,
        false,
    );
    assert_coverage_decision(
        "hard matrix refusal",
        &hard_refusal,
        &final_source,
        &covered,
        false,
    );
}

#[test]
fn preview_and_final_put_one_requested_pose_at_the_same_presented_pixels() {
    let presented_extent = [960, 540];
    let requested = pose_on(presented_extent);
    let mut preview = frame_on(81, [120, 68], presented_extent);
    preview.level = RefinementLevel::Preview;
    let preview_plan = reproject_onto(&preview, &requested);
    let preview_lattice = preview_plan
        .lattice
        .expect("the Preview plan names its delivery pair");
    assert_eq!(preview_lattice.source(), [120, 68]);
    assert_eq!(preview_lattice.destination(), presented_extent);
    let preview_entry =
        presentation_ledger_entry(&preview_plan, &requested, Some(&preview), presented_extent);

    let mut final_frame = frame_on(82, presented_extent, presented_extent);
    final_frame.level = RefinementLevel::Final;
    let final_plan = reproject_onto(&final_frame, &requested);
    let final_entry = presentation_ledger_entry(
        &final_plan,
        &requested,
        Some(&final_frame),
        presented_extent,
    );

    assert_eq!(preview_entry.requested_centre_px, Some([480.0, 270.0]));
    assert_eq!(preview_entry.anchor_px, Some([0.0, 0.0]));
    assert_eq!(
        final_entry.requested_centre_px,
        preview_entry.requested_centre_px
    );
    assert_eq!(final_entry.anchor_px, preview_entry.anchor_px);
    assert_eq!(preview_entry.level, Some(RefinementLevel::Preview));
    assert_eq!(final_entry.level, Some(RefinementLevel::Final));
}

#[test]
fn relief_ledger_maps_preview_records_instead_of_asserting_requested_positions() {
    let presented_extent = [960, 540];
    let mut source = frame_on(91, [120, 68], presented_extent);
    source.level = RefinementLevel::Preview;
    let mut requested = pose_on(presented_extent);
    requested.zoom_log2 = 0.1;
    let plan = crate::WarpPlan {
        lattice: crate::LatticePair::new(source.extent, presented_extent),
        source_scene_id: Some(source.scene_id),
        source_texture_index: Some(source.texture_index),
        destination_pose: Some(requested),
        source_valid: true,
        exposed: true,
        predicted_exposed_fraction: Some(0.071_952_160_494),
        kind: WarpKind::ReliefRedraw,
        ..clear_warp_plan(false, true)
    };
    let entry = presentation_ledger_entry(&plan, &requested, Some(&source), presented_extent);
    assert_eq!(entry.requested_centre_px, Some([480.0, 270.0]));
    assert_eq!(entry.anchor_px, Some([0.0, 0.0]));

    let unstated_destination = crate::WarpPlan {
        destination_pose: None,
        ..plan
    };
    let entry = presentation_ledger_entry(
        &unstated_destination,
        &requested,
        Some(&source),
        presented_extent,
    );
    assert_eq!(entry.requested_centre_px, None);
    assert_eq!(entry.anchor_px, None);
}

#[test]
fn a_flat_centred_zoom_hold_keeps_the_centre_but_exposes_the_corner_correction() {
    let extent = [960, 540];
    let source = frame_on(83, extent, extent);
    let mut requested = pose_on(extent);
    requested.zoom_log2 += 0.1;
    let held = apply_hold_policy(clear_warp_plan(false, true), Some(&source), true, extent);
    let held_entry = presentation_ledger_entry(&held, &requested, Some(&source), extent);
    let moved = reproject_onto(&source, &requested);
    let moved_entry = presentation_ledger_entry(&moved, &requested, Some(&source), extent);

    assert_eq!(held_entry.requested_centre_px, Some([480.0, 270.0]));
    assert_eq!(
        moved_entry.requested_centre_px,
        held_entry.requested_centre_px
    );
    assert_eq!(held_entry.anchor_px, Some([32.14, 18.08]));
    assert_eq!(moved_entry.anchor_px, Some([0.0, 0.0]));
}

/// Asserts a plan names exactly this pairing and returns the pair it named.
fn named_pair(
    plan: &crate::WarpPlan,
    source_extent: [u32; 2],
    destination_extent: [u32; 2],
    route: &str,
) -> crate::LatticePair {
    let lattice = plan
        .lattice
        .unwrap_or_else(|| panic!("the {route} plan names no lattice pair"));
    assert_eq!(
        [lattice.source(), lattice.destination()],
        [source_extent, destination_extent],
        "the {route} plan on {source_extent:?} to {destination_extent:?} names another pairing"
    );
    lattice
}

/// The retained scene at its own pose: identity in the picture, not in pixels.
fn self_plan_covers(source_extent: [u32; 2], destination_extent: [u32; 2]) -> crate::WarpPlan {
    let frame = frame_on(71, source_extent, destination_extent);
    let plan = reproject_onto(&frame, &pose_on(destination_extent));
    assert_eq!(plan.kind, WarpKind::AnchorHomography);
    assert!(plan.source_valid);
    let lattice = named_pair(&plan, source_extent, destination_extent, "self");
    assert!(
        lattice.covers_destination(plan.rows),
        "the self plan on {source_extent:?} to {destination_extent:?} leaves the source"
    );
    plan
}

/// A pan moves ground off the source on purpose, so it declares exposure instead of coverage.
///
/// What must hold either way is that the centre of the destination, moved by three pixels of a
/// lattice hundreds wide, still reads near the centre of the source picture rather than at some
/// multiple of the extent ratio away from it.
fn pan_plan_declares_exposure(
    source_extent: [u32; 2],
    destination_extent: [u32; 2],
) -> crate::WarpPlan {
    let frame = frame_on(71, source_extent, destination_extent);
    let mut moved = pose_on(destination_extent);
    moved.centre_from_reference_px = [3.0, -2.0];
    let plan = reproject_onto(&frame, &moved);
    assert_eq!(plan.kind, WarpKind::AnchorHomography);
    assert!(plan.exposed);
    let lattice = named_pair(&plan, source_extent, destination_extent, "pan");
    let centre = lattice
        .source_uv(plan.rows, [0.0, 0.0])
        .expect("the destination centre is in front of the source");
    assert!(
        centre.iter().all(|value| (0.4..=0.6).contains(value)),
        "a three-pixel pan on {source_extent:?} to {destination_extent:?} reads {centre:?}"
    );
    plan
}

/// One zoom step in shows half the chart, whose corners land a quarter in from the source corners.
///
/// This is the reprojection the delivery composition exists for, and the one that asks the
/// coverage question of a solved map rather than of a covering one: nothing is exposed, so the
/// corner position is checkable. Without the composition the solved map's output stays in the
/// source pose's pixels while the fragment divides by the source texture's extent, and on the
/// 120x68-into-960x540 pairing the corner that should read a quarter out reads twice the picture
/// out instead.
fn zoom_plan_covers(source_extent: [u32; 2], destination_extent: [u32; 2]) -> crate::WarpPlan {
    let frame = frame_on(71, source_extent, destination_extent);
    let mut closer = pose_on(destination_extent);
    closer.zoom_log2 = 1.0;
    let plan = reproject_onto(&frame, &closer);
    assert_eq!(plan.kind, WarpKind::AnchorHomography);
    assert!(
        !plan.exposed,
        "a zoom into the middle of the source exposes nothing on {source_extent:?} to \
         {destination_extent:?}"
    );
    let lattice = named_pair(&plan, source_extent, destination_extent, "zoom");
    assert!(
        lattice.covers_destination(plan.rows),
        "the zoom plan on {source_extent:?} to {destination_extent:?} leaves the source"
    );
    let corner = lattice
        .source_uv(plan.rows, [1.0, 1.0])
        .expect("the destination corner is in front of the source");
    assert!(
        (corner[0] - 0.75).abs() < 1.0e-3 && (corner[1] - 0.25).abs() < 1.0e-3,
        "one zoom step on {source_extent:?} to {destination_extent:?} reads {corner:?} rather \
         than a quarter in from the source corner"
    );
    plan
}

/// The hold shows the last completed picture unchanged on the lattice it is presented on.
fn hold_plan_covers(source_extent: [u32; 2], destination_extent: [u32; 2]) -> crate::WarpPlan {
    let frame = frame_on(71, source_extent, destination_extent);
    let plan = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&frame),
        true,
        destination_extent,
    );
    assert_eq!(plan.kind, WarpKind::HoldStale);
    let lattice = named_pair(&plan, source_extent, destination_extent, "hold");
    assert!(
        lattice.covers_destination(plan.rows),
        "the hold on {source_extent:?} to {destination_extent:?} leaves the source"
    );
    plan
}

/// Every plan kind that samples a source covers its destination, on every ladder pairing.
///
/// The four sampling routes reach the fragment differently — the planner's self-identity, its
/// solved reprojection under a pan and under a zoom, and the hold policy — and before the seam
/// only the last of them knew that two extents exist. The table is the check that the seam is the
/// one place the question is answered: for each pairing the plan's own lattice is asked whether
/// the four destination corners and the centre land inside the source picture, which is the same
/// arithmetic the fragment does before it decides to sample or to paint clear.
#[test]
fn every_sampling_plan_kind_covers_its_destination_on_every_ladder_pairing() {
    for (source_extent, destination_extent) in ladder_pairs() {
        let plans = [
            self_plan_covers(source_extent, destination_extent),
            pan_plan_declares_exposure(source_extent, destination_extent),
            zoom_plan_covers(source_extent, destination_extent),
            hold_plan_covers(source_extent, destination_extent),
        ];
        for plan in plans {
            assert_eq!(
                enforce_lattice(&plan, destination_extent).1,
                None,
                "a plan that states its pair is refused on {source_extent:?} to \
                 {destination_extent:?}"
            );
        }
    }
}

/// A clear plan samples nothing, so it names no pair and the invariant has nothing to ask it.
#[test]
fn a_clear_plan_names_no_lattice_pair_and_is_never_refused_for_one() {
    for (_, destination_extent) in ladder_pairs() {
        let plan = clear_warp_plan(false, true);
        assert_eq!(plan.lattice, None);
        assert_eq!(enforce_lattice(&plan, destination_extent).1, None);
        let edge_on = clear_warp_plan(true, false);
        assert_eq!(edge_on.lattice, None);
        assert_eq!(enforce_lattice(&edge_on, destination_extent).1, None);
    }
}

/// The three refusals: an unstated pair, a pair naming another destination, corners off the source.
///
/// Each becomes an honest clear with a published reason. Clearing restarts the refinement ladder,
/// which is a real cost, but it asserts nothing about geometry; a picture at a scale the geometry
/// does not have asserts geometry that does not exist, and under the rendering rule a moving frame
/// may be very inaccurate but never wrong.
#[test]
fn a_plan_that_cannot_state_where_it_puts_the_picture_is_refused_into_a_clear() {
    let destination_extent = [960, 540];
    let source_extent = [120, 68];
    let frame = frame_on(72, source_extent, destination_extent);
    let held = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&frame),
        true,
        destination_extent,
    );

    let unstated = crate::WarpPlan {
        lattice: None,
        ..held
    };
    let (refused, reason) = enforce_lattice(&unstated, destination_extent);
    assert_eq!(refused.kind, WarpKind::ClearOnly);
    assert!(!refused.source_valid);
    assert!(
        refused.exposed,
        "a refusal is ground the next scene must fill"
    );
    assert_eq!(
        reason.map(LatticeRefusal::as_str),
        Some("plan named no lattice pair")
    );

    let elsewhere = crate::WarpPlan {
        lattice: crate::LatticePair::new(source_extent, [480, 270]),
        ..held
    };
    let (refused, reason) = enforce_lattice(&elsewhere, destination_extent);
    assert_eq!(refused.kind, WarpKind::ClearOnly);
    assert_eq!(
        reason.map(LatticeRefusal::as_str),
        Some("plan named another destination lattice")
    );

    // The morph thumbnail itself, constructed as a plan rather than reached through a path: the
    // pair is named and correct, and the rows are the identity that ignores it.
    let thumbnail = crate::WarpPlan {
        rows: crate::identity_warp_rows(),
        ..held
    };
    let (refused, reason) = enforce_lattice(&thumbnail, destination_extent);
    assert_eq!(refused.kind, WarpKind::ClearOnly);
    assert_eq!(
        reason.map(LatticeRefusal::as_str),
        Some("plan maps the destination outside the source")
    );

    // The same rows with exposure declared are a different claim and are not refused: the plan is
    // telling the exposure machinery that part of the destination has no source.
    let declared = crate::WarpPlan {
        rows: crate::identity_warp_rows(),
        exposed: true,
        ..held
    };
    assert_eq!(enforce_lattice(&declared, destination_extent).1, None);
}

/// A hold whose scale cannot be stated stays a clear plan rather than placing the picture.
#[test]
fn a_zero_source_extent_refuses_the_hold_before_any_rows_exist() {
    let mut degenerate = frame_on(73, [120, 68], [960, 540]);
    degenerate.extent = [120, 0];
    let plan = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&degenerate),
        true,
        [960, 540],
    );
    assert_eq!(plan.kind, WarpKind::ClearOnly);
    assert_eq!(plan.lattice, None);
    assert!(!plan.source_valid);
}

/// Every other path that draws against an extent either agrees by construction or is checked.
///
/// The warp fragment is not the only pass whose destination lattice can disagree with what it
/// draws, so the same question is put to the three remaining ones. The answers differ in kind, and
/// that difference is the point: two of them are structural, one is an explicit refusal, and none
/// of them needs the plan seam.
///
/// The relief redraw draws the retained records as a mesh rather than sampling a texture, so it
/// has no source uv and no scale to get wrong. What it can get wrong is the grid it indexes, and
/// `relief_scene_uniform` refuses outright when the retained grid's dimensions are not the source
/// frame's extent. The redraw writes that grid into the scene uniform itself, which is exactly why
/// `write_warp_destination_extent` runs on the image-warp branch alone: writing the pose's grid
/// over the redraw's would leave the mesh indexing a lattice it was not built for. The
/// backdrop-first composition is the same shape, with `backdrop_scene_uniform` and
/// `ensure_backdrop_indices` taking the extent from the one backdrop grid.
///
/// The edge-on plan's identity rows are the screen-to-plane map, not plan rows, and the fragment
/// returns exterior sky on the edge-on flag before it reads either map. There is no lattice
/// question because there is no sample.
///
/// The ledger's two-index rotation hands out the index the retained frame is not on, and the
/// scene texture at that index is reallocated to the submitted extent before anything is drawn
/// into it. A frame's recorded extent is therefore the extent of its own texture by construction:
/// the index holding the retained picture is never passed to `ensure_scene_texture`.
#[test]
fn the_remaining_extent_paths_agree_by_construction_or_refuse() {
    let redraw = include_str!("redraw.rs");
    assert!(
        redraw.contains("if [grid.width, grid.height] != source.extent {"),
        "the relief redraw refuses a retained grid that is not the source frame's extent"
    );
    let warp = include_str!("warp.rs");
    let relief_branch = warp
        .find("if relief_redraw {")
        .expect("the warp submission branches on the relief redraw");
    let destination_write = warp
        .find("self.write_warp_destination_extent(warp_destination_extent);")
        .expect("the image warp states its destination lattice");
    let image_branch = warp
        .find("encode_image_warp(")
        .expect("the warp submission encodes an image warp");
    assert!(
        relief_branch < destination_write && destination_write < image_branch,
        "the destination lattice is written on the image-warp branch alone, so a relief redraw \
         keeps the record grid it indexes its mesh from"
    );

    let shader = crate::warp_shader();
    let edge_on = shader
        .find("if (hot.flags.w != 0u)")
        .expect("the fragment tests the edge-on flag");
    let screen_map = shader
        .find("hot.screen_to_plane_row_0.xyz")
        .expect("the fragment reads the screen map");
    let plan_rows = shader
        .find("hot.homography_row_0.xyz")
        .expect("the fragment reads the plan rows");
    assert!(
        edge_on < screen_map && edge_on < plan_rows,
        "an edge-on payload returns sky before either map is read, so its identity screen rows \
         are never a lattice claim"
    );

    let submit = include_str!("scene/submit.rs");
    assert!(
        submit.contains("self.ledger.begin(|texture_index| {")
            && submit
                .contains("ensure_scene_texture(device, gpu, texture_index as usize, extent)?"),
        "only the index the ledger hands out is reallocated, so the retained frame's recorded \
         extent stays the extent of its own texture"
    );

    let mut ledger = SceneLedger::default();
    let first = promote_binding_scene(&mut ledger, 81);
    let second = promote_binding_scene(&mut ledger, 82);
    assert_ne!(
        first.texture_index, second.texture_index,
        "consecutive scenes take different texture indices"
    );
    assert_eq!(
        ledger
            .available_texture_index()
            .expect("no pending scene occupies the ledger"),
        first.texture_index,
        "the index handed out next is the one the retained picture is not on"
    );
}

/// A captured frame's facts row says what scale the picture was presented at.
#[test]
fn the_facts_publish_both_lattices_of_the_plan_they_recorded() {
    let mut facts = PresentFacts::default();
    assert_eq!([facts.destination_width, facts.destination_height], [0, 0]);
    assert_eq!([facts.warp_source_width, facts.warp_source_height], [0, 0]);
    assert_eq!(facts.warp_lattice_refusal, None);

    let destination_extent = [960, 540];
    let frame = frame_on(83, [120, 68], destination_extent);
    let held = apply_hold_policy(
        clear_warp_plan(false, true),
        Some(&frame),
        true,
        destination_extent,
    );
    facts.record_warp_plan(&held, Some(0.0));
    assert_eq!(
        [facts.warp_source_width, facts.warp_source_height],
        [120, 68]
    );
    assert_eq!(facts.warp_kind, WarpKind::HoldStale);

    facts.record_warp_plan(&clear_warp_plan(false, true), None);
    assert_eq!(
        [facts.warp_source_width, facts.warp_source_height],
        [0, 0],
        "a plan that samples nothing publishes no source extent"
    );
}
