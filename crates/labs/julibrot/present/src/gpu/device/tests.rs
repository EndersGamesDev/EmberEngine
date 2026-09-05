use ember_julibrot_kernels::{EscapeGrid, RefinementLevel};
use ember_julibrot_math::{ObjectAngles, PrecisionMode, ViewControls, construct_plane};
use ember_julibrot_worker::MainState;

use super::census::{census_if_ready, observe_fence, take_glitch_readback_result};
use super::*;
use crate::fence::FenceDecision;
use crate::state::{PendingScene, SceneCompletion};
use crate::{PresentFacts, SubmissionKind, SubmissionMeasurement};

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

        let FenceDecision::Complete(measurement) = observe_fence(&mut pending, 101.0) else {
            panic!("a successful scene fence must deliver independently of its census");
        };
        assert_eq!(measurement.id, 29);
        let census = census_if_ready(take_glitch_readback_result(&mut pending), || {
            panic!("an unavailable census must not be read")
        });
        assert_eq!(census, None);
    }
}

fn binding_pose() -> Pose {
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
        grid_width: 64,
        grid_height: 36,
        map: PoseMap::Mapped(ember_julibrot_math::Homography::IDENTITY),
        centre_from_reference_px: [0.0; 2],
    }
}

fn binding_measurement(id: u64) -> SubmissionMeasurement {
    SubmissionMeasurement {
        kind: SubmissionKind::Scene,
        id,
        source_scene_id: None,
        sample_class: SampleClass::Measured,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        wall_ms: 1.0,
        fence_wait_ms: 0.5,
        polls: 1,
    }
}

fn promote_binding_scene(ledger: &mut SceneLedger, scene_id: u64) -> crate::SceneFrame {
    ledger
        .begin(|texture_index| {
            Ok(PendingScene {
                scene_id,
                pose: binding_pose(),
                palette: PaletteId::Classic,
                iteration_cap: 64,
                level: RefinementLevel::Final,
                extent: [64, 36],
                grid: binding_main().grid,
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
    let mut arena = ember_lab_heap::SpanArena::new(64, 1, 64, 4_096, 64)
        .expect("relief fixture arena is valid");
    let span = arena
        .allocate_span(64 * 36, 64)
        .expect("relief fixture grid fits");
    PresentMain {
        epoch: 1,
        state: MainState {
            delivered_iter_cap: 64,
            ..MainState::default()
        },
        grid: EscapeGrid {
            span,
            width: 64,
            height: 36,
            level: RefinementLevel::Final,
        },
        object: ember_julibrot_math::ObjectAngles::JULIA,
        plane: binding_pose().plane,
        map: PoseMap::EdgeOn,
        backdrop: None,
    }
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
    let held = apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), true);
    assert_eq!(held.kind, WarpKind::HoldStale);
    assert_eq!(held.source_scene_id, Some(sampled.scene_id));
    assert_eq!(held.source_texture_index, Some(sampled.texture_index));
    assert!(held.source_valid);
    assert!(!held.exposed);
    assert_eq!(held.rows, identity_rows());

    let mut facts = PresentFacts::default();
    facts.record_warp_plan(&held, Some(0.0));
    assert_eq!(facts.warp_kind, WarpKind::HoldStale);
    assert_eq!(facts.warp_kind.as_str(), "HoldStale");

    let mut hot = WarpSourceSlot::default();
    hot.write_hot(&held, false);
    assert_eq!(
        hot.frame(ledger.retained(), ledger.held())
            .map(|frame| frame.scene_id),
        Some(37)
    );
    assert_eq!(hot.accepted_frame(ledger.retained(), ledger.held()), None);
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
        apply_hold_policy(refused, Some(&held.frame), false).kind,
        WarpKind::ClearOnly
    );
    let plan = apply_hold_policy(refused, Some(&held.frame), true);
    assert_eq!(plan.kind, WarpKind::HoldStale);
    assert_eq!(plan.rows, identity_rows());
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
    let cleared = apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), false);
    assert_eq!(cleared.kind, WarpKind::ClearOnly);
    assert!(!cleared.source_valid);

    let mut bounded = clear_warp_plan(false, false);
    bounded.kind = WarpKind::AnchorHomography;
    bounded.source_scene_id = Some(sampled.scene_id);
    bounded.source_texture_index = Some(sampled.texture_index);
    bounded.source_valid = true;
    let accepted = apply_hold_policy(bounded, ledger.retained(), true);
    assert_eq!(accepted, bounded);

    let mut facts = PresentFacts::default();
    facts.record_warp_plan(
        &apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), true),
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
    let uniform = relief_scene_uniform(retained_grid, &sampled, crate::CLASSIC_PALETTE)
        .expect("compatible records form a scene uniform");
    assert_eq!(uniform.grid, [64, 36, RefinementLevel::Final as u32, 64]);
    assert_eq!(uniform.span[0], retained_grid.span.directory_index);
    assert_eq!(uniform.span[1], 64 * 36);
    assert_eq!(uniform.basis_u, sampled.pose.plane.basis_u);
    assert_eq!(uniform.screen_to_plane_row_0, [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(uniform.screen_to_plane_row_2, [0.0, 0.0, 1.0, 1.0]);
    let load = scene_load_color(crate::CLASSIC_PALETTE);
    let sky = crate::exterior_zero(crate::CLASSIC_PALETTE);
    assert_eq!([load.r, load.g, load.b, load.a], sky.map(f64::from));
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
    let held = apply_hold_policy(clear_warp_plan(false, true), ledger.retained(), true);
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
    assert_eq!(
        body.matches("backdrop").count(),
        2,
        "the backdrop may reach set_main only as the selection comparison"
    );
    assert!(body.contains("previous.backdrop != main.backdrop"));
    assert!(
        body.find("previous.backdrop != main.backdrop")
            < body.find("self.ledger.invalidate_incompatible("),
        "the backdrop comparison belongs to the selection test, never to the clear"
    );
}

/// The stamp needs a stencil aspect, and the engine's floor has to admit the format.
#[test]
fn the_scene_depth_target_carries_a_stencil_aspect() {
    assert_eq!(DEPTH_FORMAT, wgpu::TextureFormat::Depth24PlusStencil8);
    assert!(DEPTH_FORMAT.has_stencil_aspect());
    assert!(DEPTH_FORMAT.has_depth_aspect());
}

#[test]
fn relief_redraw_disocclusion_is_clear_and_distinct_from_exterior() {
    let disocclusion = warp_load_color(crate::CLASSIC_PALETTE);
    let clear = crate::CLASSIC_PALETTE.clear_rgba.map(f64::from);
    let exterior = crate::exterior_zero(crate::CLASSIC_PALETTE).map(f64::from);
    assert_eq!(
        [
            disocclusion.r,
            disocclusion.g,
            disocclusion.b,
            disocclusion.a
        ],
        clear
    );
    assert_ne!(clear, exterior);
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
    assert!(relief_scene_uniform(&retained_grid, &sampled, crate::CLASSIC_PALETTE).is_err());
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
    assert!(relief_scene_uniform(&main.grid, &sampled, crate::CLASSIC_PALETTE).is_ok());
}

#[test]
fn refused_backdrop_coverage_is_absent_until_main_only_submit_measurement() {
    let angle = -1.316_653_720_171_549_4;
    let object = ObjectAngles {
        rho_13: angle,
        rho_24: angle,
        ..ObjectAngles::IDENTITY
    };
    let camera_angle = -0.254_142_606_623_347_1;
    let mut camera = [0.0; 10];
    camera[1] = camera_angle;
    camera[4] = camera_angle;
    let view = ViewControls {
        camera,
        camera_yaw: 0.960_422_302_787_256,
        camera_pitch: core::f64::consts::PI,
        height_scale: 4.0,
        distance_five: 2.0,
        distance_four: 2.0,
        ..ViewControls::NEUTRAL
    };
    let mut pose = binding_pose();
    pose.object = object;
    pose.plane = construct_plane(object).expect("coverage fixture plane constructs");
    pose.view = view;
    pose.grid_width = 960;
    pose.grid_height = 540;
    let mut main = binding_main();
    let mut invalid_grid = main.grid.clone();
    invalid_grid.width = 0;
    let backdrop_map = ember_julibrot_math::Homography {
        apron_scale: 2.0,
        ..ember_julibrot_math::Homography::IDENTITY
    };
    main.backdrop = Some(crate::PresentBackdrop {
        grid: invalid_grid,
        iteration_cap: 64,
        plane: pose.plane,
        map: PoseMap::Mapped(backdrop_map),
    });
    assert!(
        validate_backdrop(
            main.backdrop.as_ref().expect("candidate backdrop exists"),
            ember_lab_heap::DialectLimits {
                descriptor_capacity: u32::MAX,
                span_capacity: u32::MAX,
                handle_capacity: u32::MAX,
            },
        )
        .is_err(),
        "the submit path refuses this candidate before publishing its coverage"
    );
    let relief = crate::WarpPlan {
        kind: WarpKind::ReliefRedraw,
        source_valid: true,
        exposed: true,
        ..clear_warp_plan(false, true)
    };
    assert_eq!(
        planned_exposed_fraction(&relief, Some(&pose), None),
        None,
        "HOT publication cannot assume the candidate backdrop will validate"
    );
    let main_only = relief_redraw_clear_fraction(&pose, Some(&main), false)
        .expect("the main-only coverage mirror is finite");
    let candidate_backdrop = relief_redraw_clear_fraction(&pose, Some(&main), true)
        .expect("the unvalidated candidate coverage mirror is finite");
    assert!(main_only > candidate_backdrop);
    let mut facts = PresentFacts::default();
    facts.record_warp_plan(&relief, None);
    assert_eq!(facts.warp_exposed_fraction, None);
    facts.record_relief_coverage(Some(main_only));
    assert_eq!(facts.warp_exposed_fraction, Some(main_only));
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
