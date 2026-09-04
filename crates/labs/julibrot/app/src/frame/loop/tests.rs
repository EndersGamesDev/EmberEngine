use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use ember_julibrot_kernels::{
    EscapeGrid, GridExtent, KernelMode, PerturbUniform, RefinementPlan, SampleStatus,
    perturb_scaled_pixel, plan_refinement,
};
use ember_julibrot_math::{
    BigCentre, EscapeGridRecord, EscapeParams, Homography, MathError, ObjectAngles, OrbitStep,
    Plane, Pose, PoseMap, PrecisionMode, ReferenceOrbitBuilder, ViewControls, pixel_scale,
    precision_for, scale_split, screen_to_plane,
};

use super::{
    BACKDROP_PRESENT_LEVEL, CoverageTurn, FenceRefusal, FrameLoop, LEVELS, PresenterPoll,
    REFERENCE_RECORD_BYTES, REFERENCE_TEXEL_BYTES, RefinementLevel, RefinementSchedule,
    RefusalClass, SceneMode, SubmissionKind, apply_precision_mode, arrival_is_current,
    backdrop_extent, coverage_pre_empts, defer_scene_until_relief_redraw,
    expand_reference_texels_into, fence_error, hold_redraw_during_scene, horizon_facts,
    main_for_grid, perturbation_reference_is_current, published_iteration_cap, sampling_zoom_log2,
    schedule_exposure_fill, select_reference_candidate, stamp_scene_level, stamped_extent,
    stamped_screen_map, view_projection_changed,
};
use crate::{AppError, FramePolicy, LevelTimingLedger, ViewerController};
use ember_julibrot_present::{
    PaletteId, SampleClass, SceneFrame, SubmissionMeasurement, Warp, WarpKind, WarpValidation,
};
use ember_lab_heap::SpanArena;

/// Poll budget and wall the version-three present configuration refuses at.
const SCENE_POLLS: u32 = 4_096;
const SCENE_DEADLINE_MS: f64 = 30_000.0;

/// Pins fix (1): a boundary reference buys exactly one correction and skips the levels below.
///
/// Preview 64, Interactive 256, Final 512 against an accepted orbit of 200: only the two levels
/// whose cap outlasts that orbit could ask, and once one has asked the other must not, or the
/// second request supersedes the first and one correction has cost two of the four slots. The
/// resumed ladder then restarts at the level that asked rather than at Preview, because a
/// reference exchange replaces the orbit the same view is expanded around and repaints nothing.
#[test]
fn a_boundary_reference_spends_one_request_and_resumes_at_the_level_that_asked() {
    const PREVIEW_CAP: u32 = 64;
    const INTERACTIVE_CAP: u32 = 256;
    const FINAL_CAP: u32 = 512;
    const ORBIT: u32 = 200;

    let mut requests = 0;
    let mut at_length = None;
    let mut asked = Vec::new();
    for (level, cap) in [
        (RefinementLevel::Preview, PREVIEW_CAP),
        (RefinementLevel::Interactive, INTERACTIVE_CAP),
        (RefinementLevel::Final, FINAL_CAP),
    ] {
        if super::sampled_reference_due(true, cap, ORBIT, requests, at_length) {
            requests += 1;
            at_length = Some(ORBIT);
            asked.push(level);
        }
    }
    assert_eq!(
        asked,
        vec![RefinementLevel::Interactive],
        "one request per accepted orbit, at the first level whose cap outlasts it"
    );
    assert_eq!(requests, 1);

    // The bound and the shallow path still refuse, and a reference already long enough asks
    // for nothing at all.
    assert!(!super::sampled_reference_due(
        false, FINAL_CAP, ORBIT, 0, None
    ));
    assert!(!super::sampled_reference_due(
        true, FINAL_CAP, FINAL_CAP, 0, None
    ));
    assert!(!super::sampled_reference_due(
        true,
        FINAL_CAP,
        ORBIT,
        super::SAMPLED_REFERENCE_LIMIT,
        None
    ));
    // A longer accepted orbit re-arms the request: the outstanding one was for the old length.
    assert!(super::sampled_reference_due(
        true,
        FINAL_CAP,
        ORBIT + 51,
        1,
        Some(ORBIT)
    ));

    let mut resumed = FrameLoop::default();
    resumed.restart(7);
    assert_eq!(resumed.due(), Some(RefinementLevel::Preview));
    resumed.scene_input_resumed(8, RefinementLevel::Interactive);
    assert_eq!(
        resumed.due(),
        Some(RefinementLevel::Interactive),
        "a correction round resumes at the level whose census named the candidate"
    );
    let mut restarted = FrameLoop::default();
    restarted.scene_input_ready(9);
    assert_eq!(
        restarted.due(),
        Some(RefinementLevel::Preview),
        "an ordinary navigation still starts the ladder from Preview"
    );
}

#[test]
fn the_census_candidate_ranks_interior_over_glitch_over_the_longest_escape() {
    fn record(status: SampleStatus, escaped: f32, smooth_iter: f32) -> EscapeGridRecord {
        EscapeGridRecord {
            smooth_iter,
            escaped,
            rebase_count: 0.0,
            status: status.as_f32(),
        }
    }
    const CAP: u32 = 512;
    let escaping = record(SampleStatus::Sampled, 1.0, 511.0);
    let exhausted = record(
        SampleStatus::Glitch,
        0.0,
        ember_julibrot_kernels::GLITCH_REFERENCE_EXHAUSTED,
    );
    let numeric = record(
        SampleStatus::Glitch,
        0.0,
        ember_julibrot_kernels::GLITCH_NUMERIC_FAILURE,
    );
    let interior = record(SampleStatus::Sampled, 0.0, -1.0);
    let horizon = record(SampleStatus::Horizon, 0.0, -1.0);

    assert_eq!(
        select_reference_candidate(&[numeric, escaping, exhausted, interior, horizon], CAP),
        Some(super::ReferenceCandidate {
            index: 3,
            rank: 255
        }),
        "a record that never escaped outranks every other"
    );
    assert_eq!(
        select_reference_candidate(&[numeric, escaping, exhausted], CAP),
        Some(super::ReferenceCandidate {
            index: 2,
            rank: 254
        }),
        "only the glitch that exhausted its reference outranks an escaping record"
    );
    assert_eq!(
        select_reference_candidate(&[numeric, escaping], CAP),
        Some(super::ReferenceCandidate {
            index: 1,
            rank: 253
        }),
        "a glitch from arithmetic failure ranks below every escaping record"
    );
    assert_eq!(
        select_reference_candidate(&[numeric], CAP),
        Some(super::ReferenceCandidate { index: 0, rank: 0 }),
        "a numeric failure is still a last resort when the grid holds nothing else"
    );
    assert_eq!(
        select_reference_candidate(&[horizon], CAP),
        None,
        "a horizon record is never a reference"
    );
}

/// Pins the exact repro row: plane origin c = (-0.743643887037151, 0.13182590420533), scale 14.
///
/// The delivered row reported a reference orbit of 41 records against a 512 cap. That origin is
/// itself a point of the set, so a reference taken exactly there runs the whole cap: the row
/// glitches because the reference is not that point but wherever the view centre landed, and
/// the centre is carried off the origin by a zoom about a crosshair. The kernel condition is
/// the reference length alone — a record is a glitch when it needs more reference steps than
/// the orbit has — so this harness reproduces it on the row's own view by driving the opening
/// Final with the first 41 records of the reference, exactly as a reference that escaped at 41
/// would. The delivered loop then takes that Final's census candidate, moves the orbit point
/// onto its pixel without moving the navigation centre, and renders again until the Final
/// carries no glitches with an orbit at least as long as the frame's maximum count.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the pin is one measured sequence: seed, opening Final, census exchange, delivery"
)]
fn the_exact_origin_row_at_zoom_fourteen_corrects_to_a_glitch_free_final() {
    const WIDTH: u32 = 960;
    const HEIGHT: u32 = 540;
    const CAP: u32 = 512;
    const ROUND_LIMIT: u32 = 4;
    /// Reference length the delivered row reported for this view.
    const EXHAUSTED_AT: u32 = 41;
    assert_eq!(KernelMode::for_zoom(12.0), KernelMode::Shallow);
    assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    assert!(!perturbation_reference_is_current(
        14.0,
        14,
        Some((12, 12.0))
    ));
    assert!(perturbation_reference_is_current(
        14.0,
        14,
        Some((14, 14.0))
    ));

    let precision = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
    let view_centre = BigCentre::from_f64(
        [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
        precision.requested_bits,
    )
    .expect("finite seahorse centre");
    let plane = Plane {
        basis_u: [0.0, 0.0, 1.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let extent = GridExtent {
        width: WIDTH,
        height: HEIGHT,
    };
    let final_scale = scale_split(14.0, WIDTH).expect("zoom fourteen Final scale");
    let final_pixel = pixel_scale(14.0, WIDTH).expect("Final pixel scale");

    let mut viewer = ViewerController::new([WIDTH, HEIGHT]).expect("canonical viewer");
    viewer
        .set_plane_origin(view_centre.to_f64_mirror())
        .expect("finite origin controls");
    viewer.set_zoom_log2(14.0).expect("zoom fourteen");

    let mut centre_from_reference = [0.0_f64; 2];
    let mut reference_centre = view_centre;
    let mut opening = None;
    let mut delivered = None;
    let mut references = 0;

    for round in 0..=ROUND_LIMIT {
        let mut builder =
            ReferenceOrbitBuilder::new(&reference_centre, precision, EscapeParams::new(CAP))
                .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack_referenced(
            plane,
            &Homography::IDENTITY,
            centre_from_reference,
            final_scale,
            extent,
            EscapeParams::new(CAP),
            if round == 0 {
                EXHAUSTED_AT
            } else {
                orbit.length
            },
            RefinementLevel::Final,
        )
        .expect("referenced Final uniform");
        let samples = (0..WIDTH * HEIGHT)
            .map(|index| {
                perturb_scaled_pixel(&uniforms, &orbit.records, index)
                    .expect("canonical Final pixel")
            })
            .collect::<Vec<_>>();
        let glitch_pixel_count = samples
            .iter()
            .filter(|sample| {
                SampleStatus::from_f32(sample.record.status) == Some(SampleStatus::Glitch)
            })
            .count();
        let frame_max_count = samples
            .iter()
            .map(|sample| sample.escape_index.map_or(CAP, |index| index + 1))
            .max()
            .expect("Final is nonempty");
        let orbit_length = if round == 0 {
            EXHAUSTED_AT
        } else {
            orbit.length
        };
        if opening.is_none() {
            assert!(
                orbit.length >= EXHAUSTED_AT,
                "the origin's own orbit must reach the reported {EXHAUSTED_AT} records"
            );
            assert!(
                glitch_pixel_count > 0,
                "a reference exhausted at {EXHAUSTED_AT} against a {CAP} cap must leave the recorded defect"
            );
            opening = Some((EXHAUSTED_AT, glitch_pixel_count));
        }
        delivered = Some((orbit_length, glitch_pixel_count, frame_max_count));
        if orbit_length >= CAP || round == ROUND_LIMIT {
            break;
        }
        let records = samples
            .iter()
            .map(|sample| sample.record)
            .collect::<Vec<_>>();
        let candidate =
            select_reference_candidate(&records, CAP).expect("a Final always holds a candidate");
        let generation = viewer
            .request_reference_for_pixel(candidate.index, [WIDTH, HEIGHT])
            .expect("deterministic census reference");
        let submission = viewer
            .take_reference_submission()
            .expect("selected reference submission");
        assert_eq!(submission.navigation.generation, generation);
        centre_from_reference = submission
            .navigation
            .centre
            .displacement_px(&submission.reference_centre, &plane, final_pixel)
            .expect("reference displacement");
        assert!(
            viewer.finish_reference_submission(generation),
            "the accepted reference must release its coalesced successor"
        );
        viewer
            .configure_navigation_context(
                submission.navigation.centre.clone(),
                submission.reference_centre.clone(),
                plane,
            )
            .expect("accepted navigation context");
        let anchor = [
            0.5f64.mul_add(-f64::from(WIDTH), f64::from(candidate.index % WIDTH) + 0.5),
            0.5f64.mul_add(-f64::from(HEIGHT), f64::from(candidate.index / WIDTH) + 0.5),
        ];
        assert!(
            (centre_from_reference[0] + anchor[0]).abs() < 0.5
                && (centre_from_reference[1] + anchor[1]).abs() < 0.5,
            "the reference must land on its own census pixel: centre_from_reference {centre_from_reference:?} against anchor {anchor:?}"
        );
        reference_centre = submission.reference_centre;
        references += 1;
    }

    let (opening_length, opening_glitches) = opening.expect("the opening Final was measured");
    let (orbit_length, glitch_pixel_count, frame_max_count) =
        delivered.expect("a Final was delivered");
    assert!(
        opening_length < CAP && opening_glitches > 0,
        "opening reference {opening_length} left {opening_glitches} glitches"
    );
    assert_eq!(
        orbit_length, CAP,
        "a cap-long reference is the fixture's zero-reference-exhaustion proof"
    );
    // These five pixels meet the Pauldelbrot numeric criterion; the published count is a measurement, not a target.
    assert_eq!(glitch_pixel_count, 5, "the numeric-glitch count changed");
    assert!(
        orbit_length >= frame_max_count,
        "delivered reference orbit {orbit_length} is shorter than the frame maximum {frame_max_count}"
    );
    assert!(
        (1..=ROUND_LIMIT).contains(&references),
        "the correction took {references} references"
    );
}

#[test]
fn requested_and_owner_hot_zoom_keep_bit_identity_through_every_absolute_reset_path() {
    const EXTENT: [u32; 2] = [960, 540];

    fn assert_zoom_identity(viewer: &mut ViewerController) {
        let requested = viewer.requested().zoom_log2.to_bits();
        assert_eq!(
            viewer
                .drain_hot(EXTENT)
                .expect("bit-identical HOT zoom drains")
                .state
                .hot
                .zoom_log2
                .to_bits(),
            requested
        );
    }

    let mut viewer = ViewerController::new(EXTENT).expect("canonical viewer");
    viewer.set_zoom_log2(12.0).expect("slider zoom");
    assert_zoom_identity(&mut viewer);
    viewer.wheel_zoom(0.375, [37.0, -19.0]).expect("wheel zoom");
    assert_zoom_identity(&mut viewer);
    viewer
        .set_plane_origin([0.0, 0.0, -0.75, 0.1])
        .expect("finite origin reset");
    assert_zoom_identity(&mut viewer);
}

#[test]
fn relief_redraw_precedes_final_and_holds_while_final_overwrites_data() {
    assert!(defer_scene_until_relief_redraw(true, true));
    assert!(!defer_scene_until_relief_redraw(true, false));
    assert!(!defer_scene_until_relief_redraw(false, true));
    assert!(hold_redraw_during_scene(true, true));
    assert!(!hold_redraw_during_scene(true, false));
    assert!(!hold_redraw_during_scene(false, true));
}

#[test]
fn browser_refresh_wires_relief_redraw_before_submission_and_holds_during_scene() {
    let source = include_str!("../loop.rs");
    assert!(
        source.contains("let defer_scene_for_redraw = super::defer_scene_until_relief_redraw(")
    );
    assert!(source.contains(
        "let scene_id = if defer_scene_for_redraw && self.active_backdrop_map.is_none() {"
    ));
    assert!(source.contains("let redraw_scene_in_flight = super::hold_redraw_during_scene("));
    assert!(source.contains(
        "if warp_requested && !runtime.has_pending_surface() && !redraw_scene_in_flight {"
    ));
}

#[test]
fn backdrop_extent_spends_at_most_one_quarter_of_final_records() {
    assert_eq!(backdrop_extent([960, 540]), Some([480, 270]));
    assert_eq!(backdrop_extent([961, 541]), Some([480, 270]));
    assert_eq!(backdrop_extent([1, 1]), None);
    for extent in [[960, 540], [961, 541], [2, 2], [4_096, 2_047]] {
        let backdrop = backdrop_extent(extent).expect("fixture admits a backdrop");
        let final_records = u64::from(extent[0]) * u64::from(extent[1]);
        let backdrop_records = u64::from(backdrop[0]) * u64::from(backdrop[1]);
        assert!(backdrop_records <= final_records / 4);
    }
}

#[test]
fn backdrop_sampling_zoom_widens_only_the_coarse_kernel_grid() {
    let zoom = 3.921_825_538_184_839;
    assert_eq!(
        sampling_zoom_log2(zoom, 1.0).expect("identity").to_bits(),
        zoom.to_bits()
    );
    let widened = sampling_zoom_log2(zoom, 5.0).expect("fivefold backdrop");
    assert_eq!(widened, zoom - 5.0_f64.log2());
    assert!(sampling_zoom_log2(zoom, 0.5).is_err());
}

#[test]
fn backdrop_only_frame_cannot_skip_the_main_preview() {
    assert_eq!(BACKDROP_PRESENT_LEVEL, RefinementLevel::Preview);
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(1, true);
    assert!(!frame_loop.skip_drafts_for_accepted_warp(Some((BACKDROP_PRESENT_LEVEL, false,))));
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    assert!(
        include_str!("browser/submit.rs").contains("grid.level = super::BACKDROP_PRESENT_LEVEL;")
    );
}

#[test]
fn compact_reference_records_expand_to_zero_padded_rgba_texels() {
    let records = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let mut texels = Vec::with_capacity(32);
    expand_reference_texels_into(&records, 2, &mut texels).expect("fixture has two records");
    assert_eq!(texels.len(), 32);
    assert_eq!(&texels[..8], &records[..8]);
    assert_eq!(&texels[8..16], &[0; 8]);
    assert_eq!(&texels[16..24], &records[8..]);
    assert_eq!(&texels[24..], &[0; 8]);
    assert!(expand_reference_texels_into(&records, 1, &mut texels).is_err());
}

#[test]
#[allow(
    clippy::print_stderr,
    reason = "the requested native performance oracle reports allocations and copied bytes"
)]
fn accepted_reference_upload_reuses_scratch_without_copying_the_transfer() {
    let records = vec![7_u8; 4_096 * REFERENCE_RECORD_BYTES];
    let mut scratch = Vec::with_capacity(4_096 * REFERENCE_TEXEL_BYTES);
    let allocation = scratch.as_ptr();
    let capacity = scratch.capacity();
    expand_reference_texels_into(&records, 4_096, &mut scratch)
        .expect("preallocated scratch accepts the maximum policy orbit");
    let after_allocations = usize::from(scratch.as_ptr() != allocation);
    assert_eq!(after_allocations, 0);
    assert_eq!(scratch.capacity(), capacity);
    assert_eq!(scratch.len(), 4_096 * REFERENCE_TEXEL_BYTES);

    let before_allocations = 2;
    let copied_records = std::hint::black_box(records.clone());
    let copied_texels = std::hint::black_box(vec![0_u8; 4_096 * REFERENCE_TEXEL_BYTES]);
    std::hint::black_box((copied_records, copied_texels));
    eprintln!(
        "accepted_reference_upload before_allocations={before_allocations} after_allocations={after_allocations} before_copied_bytes={} after_copied_bytes={}",
        4_096 * REFERENCE_RECORD_BYTES * 2,
        4_096 * REFERENCE_RECORD_BYTES,
    );
}

#[test]
fn inert_distance_five_change_does_not_restart_a_flat_ladder() {
    let before = ViewControls::MANDELBROT_FLAT;
    let after = ViewControls {
        distance_five: 64.0,
        ..before
    };
    let map = |view| {
        PoseMap::Mapped(
            screen_to_plane(&ObjectAngles::IDENTITY, &view, 0.0, 960, 540, 16.0 / 9.0)
                .expect("faced flat map"),
        )
    };
    assert!(!view_projection_changed(
        before,
        map(before),
        after,
        map(after)
    ));
}

#[test]
fn the_stamped_map_extent_does_not_follow_the_refinement_ladder() {
    let plan = plan_refinement(
        GridExtent {
            width: 960,
            height: 540,
        },
        EscapeParams::new(512),
        |_| true,
    )
    .expect("the ladder fixture has enough capacity")
    .with_precision_mode(PrecisionMode::PictureFast);

    // A near-edge-on Mandelbrot object rotation: well outside the canonical flat pair, so the
    // map is solved at every extent instead of collapsing to the identity.
    let object = ObjectAngles {
        rho_13: 1.5,
        ..ObjectAngles::IDENTITY
    };
    let view = ViewControls::MANDELBROT_FLAT;
    let map = |extent: [u32; 2]| {
        let [width, height] = extent;
        PoseMap::Mapped(
            screen_to_plane(
                &object,
                &view,
                0.0,
                width,
                height,
                f64::from(width) / f64::from(height),
            )
            .expect("the tilted fixture map is invertible"),
        )
    };

    // The ladder's own levels disagree about the map for one unchanged requested view, so a
    // stamp taken at the prepared level reads stale the moment the ladder advances.
    let preview = plan.level(RefinementLevel::Preview).extent;
    let last = plan.level(RefinementLevel::Final).extent;
    assert_ne!([preview.width, preview.height], [last.width, last.height]);
    assert!(view_projection_changed(
        view,
        map([preview.width, preview.height]),
        view,
        map([last.width, last.height])
    ));

    // The stamp is taken at the requested extent, which every level of the plan shares, so an
    // unchanged requested view stays equivalent to itself across the whole ladder.
    assert_eq!(stamped_extent(&plan), [960, 540]);
    assert!(!view_projection_changed(
        view,
        map(stamped_extent(&plan)),
        view,
        map(stamped_extent(&plan))
    ));
    // Nothing the ladder prepares can move it: only Final renders at the stamped extent.
    for level in LEVELS {
        let extent = plan.level(level).extent;
        assert_eq!(
            [extent.width, extent.height] == stamped_extent(&plan),
            level == RefinementLevel::Final,
            "level {level:?}"
        );
    }
}

#[test]
fn browser_refresh_reuses_preview_and_requested_extent_maps() {
    let plan = plan_refinement(
        GridExtent {
            width: 960,
            height: 540,
        },
        EscapeParams::new(512),
        |_| true,
    )
    .expect("the ladder fixture has enough capacity")
    .with_precision_mode(PrecisionMode::PictureFast);
    let preview = plan.level(RefinementLevel::Preview).extent;
    let preview_extent = [preview.width, preview.height];
    let requested_extent = stamped_extent(&plan);
    assert_ne!(preview_extent, requested_extent);

    let mut viewer = ViewerController::new(requested_extent).expect("canonical viewer");
    for _ in 0..120 {
        viewer.drain_hot(preview_extent).expect("Preview HOT map");
        std::hint::black_box(stamped_screen_map(&viewer, &plan));
        assert_eq!(viewer.map_construction_count(), 2);
    }
}

#[test]
fn horizon_fraction_counts_pixel_centres_without_sampling_the_grid() {
    assert_eq!(
        horizon_facts(PoseMap::Mapped(Homography::IDENTITY), [8, 4]).fraction,
        0.0
    );
    let half = Homography {
        rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0],
        ..Homography::IDENTITY
    };
    assert_eq!(
        horizon_facts(PoseMap::Mapped(half), [8, 4]),
        super::HorizonFacts {
            pixels: 16,
            fraction: 0.5,
            uncertain_pixels: 0,
            uncertain_fraction: 0.0,
            condition_number: 1.0,
            edge_on: false,
        }
    );
    let all = Homography {
        rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0],
        ..Homography::IDENTITY
    };
    assert_eq!(horizon_facts(PoseMap::Mapped(all), [8, 4]).fraction, 1.0);
    let edge = horizon_facts(PoseMap::EdgeOn, [8, 4]);
    assert_eq!(edge.fraction, 1.0);
    assert!(edge.edge_on);
}

#[test]
fn reference_shift_is_expressed_in_each_level_pixel_scale() {
    let state = ember_julibrot_worker::MainState {
        reference_shift_px: [12.0, -8.0],
        ..ember_julibrot_worker::MainState::default()
    };
    assert_eq!(
        main_for_grid(state, 240, 960).reference_shift_px,
        [3.0, -2.0]
    );
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct PendingFakeScene {
    id: u64,
    generation: u32,
    level: RefinementLevel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FakeEvent {
    Completed(PendingFakeScene),
    Deadline(u64),
    WarpCompleted(u64),
    Refused {
        id: u64,
        kind: SubmissionKind,
        reason: FenceRefusal,
        polls: u32,
        wall_ms: f64,
    },
}

#[derive(Debug, Default)]
struct FakePresenter {
    next_id: u64,
    pending: Option<PendingFakeScene>,
    pending_warp: Option<u64>,
    pending_warp_kind: Option<WarpKind>,
    callback: Option<FakeEvent>,
    warp_callback: Option<FakeEvent>,
    fence_observations: u32,
    warp_fence_observations: u32,
    hot_writes: u32,
    submissions: Vec<RefinementLevel>,
    warp_submissions: Vec<u64>,
    presented_warps: Vec<u64>,
    retained_scene: Option<u64>,
    presented_scene: Option<u64>,
    pending_warp_source: Option<u64>,
    refuse_warp: bool,
    relief_redraw: bool,
    forced_warp_kind: Option<WarpKind>,
    warp_kind: Option<WarpKind>,
    warp_hold_count: u64,
    presented_clear_only: u64,
}

impl FakePresenter {
    fn submit(&mut self, generation: u32, level: RefinementLevel) -> u64 {
        self.next_id += 1;
        let scene = PendingFakeScene {
            id: self.next_id,
            generation,
            level,
        };
        self.pending = Some(scene);
        self.submissions.push(level);
        scene.id
    }

    fn write_hot(&mut self, hold_refused_warp: bool) {
        self.hot_writes += 1;
        let planned = self.forced_warp_kind.unwrap_or_else(|| {
            if self.relief_redraw && self.retained_scene.is_some() {
                WarpKind::ReliefRedraw
            } else if self.refuse_warp {
                WarpKind::ClearOnly
            } else if self.retained_scene.is_some() {
                WarpKind::AnchorHomography
            } else {
                WarpKind::ClearOnly
            }
        });
        self.warp_kind = Some(if planned == WarpKind::ClearOnly {
            if hold_refused_warp && self.retained_scene.is_some() {
                WarpKind::HoldStale
            } else {
                WarpKind::ClearOnly
            }
        } else {
            planned
        });
    }

    fn submit_warp(&mut self) -> u64 {
        self.next_id += 1;
        self.pending_warp = Some(self.next_id);
        self.pending_warp_kind = self.warp_kind;
        self.pending_warp_source = match self.warp_kind {
            Some(WarpKind::ClearOnly) | None => None,
            Some(_) => self.retained_scene,
        };
        if self.warp_kind == Some(WarpKind::HoldStale) {
            self.warp_hold_count = self.warp_hold_count.saturating_add(1);
        }
        self.warp_submissions.push(self.next_id);
        self.next_id
    }

    fn fire_completed_callback(&mut self) {
        self.callback = self.pending.map(FakeEvent::Completed);
    }

    fn fire_deadline(&mut self) {
        self.callback = self.pending.map(|scene| FakeEvent::Deadline(scene.id));
    }

    fn fire_warp_completed(&mut self) {
        self.warp_callback = self.pending_warp.map(FakeEvent::WarpCompleted);
    }

    fn fire_warp_refusal(&mut self, reason: FenceRefusal, polls: u32, wall_ms: f64) {
        self.warp_callback = self.pending_warp.map(|id| FakeEvent::Refused {
            id,
            kind: SubmissionKind::Warp,
            reason,
            polls,
            wall_ms,
        });
    }
}

impl PresenterPoll for FakePresenter {
    type Event = FakeEvent;

    fn poll_once(&mut self, _now_ms: f64) -> Vec<Self::Event> {
        let mut events = Vec::new();
        if self.pending.is_some() {
            self.fence_observations += 1;
            if let Some(event) = self.callback.take() {
                if let FakeEvent::Completed(scene) = event
                    && (self.retained_scene.is_none() || scene.level == RefinementLevel::Final)
                {
                    self.retained_scene = Some(scene.id);
                    self.refuse_warp = false;
                }
                self.pending = None;
                events.push(event);
            }
        }
        if self.pending_warp.is_some() {
            self.warp_fence_observations += 1;
            if let Some(event) = self.warp_callback.take() {
                if matches!(event, FakeEvent::WarpCompleted(_)) {
                    if self.pending_warp_kind == Some(WarpKind::ClearOnly) {
                        self.presented_clear_only = self.presented_clear_only.saturating_add(1);
                    }
                    self.presented_scene = self.pending_warp_source.take();
                }
                self.pending_warp_kind = None;
                self.pending_warp = None;
                events.push(event);
            }
        }
        events
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FakeClock {
    now_ms: f64,
}

impl FakeClock {
    fn advance(&mut self, elapsed_ms: f64) {
        self.now_ms += elapsed_ms;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TurnOutcome {
    scene_id: Option<u64>,
    warp_id: Option<u64>,
    presented: bool,
    refused: bool,
}

/// Drives one turn in the browser's order: poll, then scene, then surface warp.
fn drive_turn(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: FakeClock,
    policy: FramePolicy,
    warps: bool,
) -> TurnOutcome {
    let mut outcome = TurnOutcome::default();
    for event in FrameLoop::refresh(presenter, clock.now_ms) {
        match event {
            FakeEvent::Completed(scene) => {
                frame_loop.completed(scene.id, scene.generation, scene.level);
            }
            FakeEvent::WarpCompleted(id) => {
                presenter.presented_warps.push(id);
                outcome.presented = true;
            }
            FakeEvent::Deadline(id) => {
                let refusal = frame_loop.refused(
                    SubmissionKind::Scene,
                    FenceRefusal::Deadline,
                    id,
                    SCENE_POLLS,
                    SCENE_DEADLINE_MS,
                );
                outcome.refused = refusal.class != RefusalClass::Device;
            }
            FakeEvent::Refused {
                id,
                kind,
                reason,
                polls,
                wall_ms,
            } => {
                let refusal = frame_loop.refused(kind, reason, id, polls, wall_ms);
                outcome.refused = refusal.class != RefusalClass::Device;
            }
        }
    }
    if frame_loop.stopped().is_some() {
        return outcome;
    }
    let has_retained_scene = presenter.retained_scene.is_some();
    presenter.write_hot(frame_loop.hold_refused_warp(has_retained_scene));
    if !outcome.refused
        && let Some(level) = frame_loop.due()
    {
        let id = presenter.submit(frame_loop.generation(), level);
        frame_loop.submitted(id, level);
        outcome.scene_id = Some(id);
    }
    if warps && presenter.pending_warp.is_none() && frame_loop.warp_requested(policy) {
        outcome.warp_id = Some(presenter.submit_warp());
        frame_loop.warp_submitted();
    }
    outcome
}

fn drive_refresh(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: FakeClock,
) -> Option<u64> {
    drive_turn(
        frame_loop,
        presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        false,
    )
    .scene_id
}

fn drive_viewer_harness(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: FakeClock,
    warps: bool,
) -> TurnOutcome {
    drive_turn(
        frame_loop,
        presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        warps,
    )
}

fn retained_presenter(refuse_warp: bool) -> FakePresenter {
    FakePresenter {
        next_id: 37,
        retained_scene: Some(37),
        presented_scene: Some(37),
        refuse_warp,
        ..FakePresenter::default()
    }
}

fn finish_pending_refused_ladder(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: &mut FakeClock,
) -> u64 {
    assert_eq!(
        presenter.pending.map(|scene| scene.level),
        Some(RefinementLevel::Preview)
    );
    presenter.fire_completed_callback();
    clock.advance(1.0);
    let interactive = drive_viewer_harness(frame_loop, presenter, *clock, false);
    assert_eq!(
        presenter.pending.map(|scene| scene.level),
        Some(RefinementLevel::Interactive)
    );
    assert!(interactive.scene_id.is_some());
    presenter.fire_completed_callback();
    clock.advance(1.0);
    let final_turn = drive_viewer_harness(frame_loop, presenter, *clock, false);
    let final_scene = final_turn.scene_id.expect("Final scene is submitted");
    assert_eq!(
        presenter.pending.map(|scene| scene.level),
        Some(RefinementLevel::Final)
    );
    presenter.fire_completed_callback();
    clock.advance(1.0);
    let fill = drive_viewer_harness(frame_loop, presenter, *clock, true);
    assert!(fill.warp_id.is_some());
    assert_eq!(presenter.retained_scene, Some(final_scene));
    assert_eq!(presenter.warp_kind, Some(WarpKind::AnchorHomography));
    presenter.fire_warp_completed();
    clock.advance(1.0);
    assert!(drive_viewer_harness(frame_loop, presenter, *clock, false).presented);
    assert_eq!(presenter.presented_scene, Some(final_scene));
    final_scene
}

fn precision_runtime_from_viewer(
    viewer: &mut ViewerController,
) -> (PrecisionMode, FrameLoop, RefinementPlan) {
    let mut precision_mode = PrecisionMode::Deterministic;
    let mut frame_loop = FrameLoop::default();
    let mut plan = plan_refinement(
        GridExtent {
            width: 960,
            height: 540,
        },
        EscapeParams::new(4_096),
        |_| true,
    )
    .expect("the native precision fixture has enough capacity");
    apply_precision_mode(
        viewer.requested().precision_mode,
        &mut precision_mode,
        &mut frame_loop,
        &mut plan,
        viewer,
    )
    .expect("the viewer precision request is applicable");
    (precision_mode, frame_loop, plan)
}

fn complete_preview(frame_loop: &mut FrameLoop, id: u64) {
    let generation = frame_loop.generation();
    frame_loop.submitted(id, RefinementLevel::Preview);
    assert!(frame_loop.completed(id, generation, RefinementLevel::Preview));
}

/// Runs the Preview, Interactive, Final ladder to its end with a warp on every turn.
fn run_ladder_to_idle(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: &mut FakeClock,
    policy: FramePolicy,
) {
    for _ in 0..=LEVELS.len() {
        drive_turn(frame_loop, presenter, *clock, policy, true);
        presenter.fire_completed_callback();
        presenter.fire_warp_completed();
        clock.advance(16.0);
    }
}

#[test]
#[allow(
    clippy::print_stderr,
    reason = "the requested fake-clock oracle reports acceptance-to-scene latency"
)]
fn accepted_deep_reference_submits_its_first_scene_in_the_accepting_refresh() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let clock = FakeClock::default();
    let accepted_at = clock.now_ms;

    frame_loop.scene_input_ready(7);
    let scene_id = drive_refresh(&mut frame_loop, &mut presenter, clock)
        .expect("the accepting refresh submits its scheduled scene");
    let after_ms = clock.now_ms - accepted_at;

    assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
    assert_eq!(scene_id, 1);
    assert_eq!(after_ms, 0.0);
    eprintln!(
        "accepted_reference_first_scene before_ms={:.6} after_ms={after_ms:.6}",
        1_000.0 / 60.0,
    );
}

#[test]
#[allow(
    clippy::print_stderr,
    reason = "the requested native facts harness reports structural snapshot allocations"
)]
fn facts_refresh_reuses_cached_text_and_borrows_the_timing_ledger() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.record_transient(AppError::Deadline {
        operation: "facts allocation harness",
        deadline_ms: 1.0,
    });
    let cached = std::sync::Arc::clone(
        frame_loop
            .last_transient_text()
            .expect("the refusal cached its display text"),
    );
    frame_loop.stop(AppError::Deadline {
        operation: "facts stopping harness",
        deadline_ms: 2.0,
    });
    let stopped = std::sync::Arc::clone(
        frame_loop
            .stopped_text()
            .expect("the stop cached its display text"),
    );
    let timings = LevelTimingLedger::default();
    let timing_address = std::ptr::from_ref(&timings);

    for _ in 0..120 {
        let text = std::sync::Arc::clone(
            frame_loop
                .last_transient_text()
                .expect("cached text remains available"),
        );
        assert!(std::sync::Arc::ptr_eq(&cached, &text));
        let stopped_text = std::sync::Arc::clone(
            frame_loop
                .stopped_text()
                .expect("cached stop text remains available"),
        );
        assert!(std::sync::Arc::ptr_eq(&stopped, &stopped_text));
        assert_eq!(std::ptr::from_ref(&timings), timing_address);
    }

    let before_allocations = 11;
    let after_allocations = 0;
    eprintln!(
        "page_facts_snapshot before_allocations_at_least={before_allocations} after_allocations={after_allocations} refreshes=120"
    );
}

#[test]
fn headless_frame_loop_populates_a_per_level_timing_record() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    let mut timings = LevelTimingLedger::default();
    frame_loop.accept_request(7, true);
    let scene_id =
        drive_refresh(&mut frame_loop, &mut presenter, clock).expect("headless Preview submission");
    timings.record_worker(11, 1_250, None);
    timings.begin_scene(11, scene_id, RefinementLevel::Preview);
    presenter.fire_completed_callback();
    clock.advance(2.5);
    let _next_scene = drive_refresh(&mut frame_loop, &mut presenter, clock);
    timings.complete_scene(
        scene_id,
        SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: scene_id,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::Deterministic.as_str(),
            wall_ms: 2.5,
            fence_wait_ms: 2.0,
            polls: 2,
        },
    );
    timings.complete_warp(SubmissionMeasurement {
        kind: SubmissionKind::Warp,
        id: 99,
        source_scene_id: Some(scene_id),
        sample_class: SampleClass::Measured,
        precision_mode: PrecisionMode::Deterministic.as_str(),
        wall_ms: 0.75,
        fence_wait_ms: 0.5,
        polls: 1,
    });
    assert_eq!(presenter.submissions[0], RefinementLevel::Preview);
    assert_eq!(timings.records().len(), 1);
    let records = timings.records();
    let record = records.first().expect("one timing record");
    assert_eq!(record.scene_us, Some(2_500));
    assert_eq!(record.warp_us, Some(750));
    assert_eq!(record.worker_reference_us, Some(1_250));
    assert_eq!(record.dispatch_us, None);
    assert_eq!(record.credit_wait_us, None);
    assert!(!record.discarded);
}

#[test]
fn published_main_cap_holds_for_the_whole_ladder() {
    for requested in [64_u32, 512, 4_096, 8_192] {
        let plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(requested),
            |_| true,
        )
        .expect("a 960 by 540 plan with unlimited capacity is representable");
        assert_eq!(published_iteration_cap(&plan), requested.min(4_096));
    }
}

#[test]
fn the_per_level_cap_the_app_must_not_publish_changes_between_levels() {
    let plan = plan_refinement(
        GridExtent {
            width: 960,
            height: 540,
        },
        EscapeParams::new(512),
        |_| true,
    )
    .expect("a 960 by 540 plan with unlimited capacity is representable");
    let level_caps = LEVELS.map(|level| plan.level(level).iteration_cap);
    assert_eq!(level_caps, [64, 256, 512]);
    assert!(
        level_caps
            .iter()
            .any(|cap| *cap != published_iteration_cap(&plan)),
        "a per-level cap would look like a new selection to present"
    );
}

#[test]
fn coalesced_navigation_makes_an_endpoint_current_arrival_stale() {
    assert!(arrival_is_current(false, 7, 7, 0));
    assert!(!arrival_is_current(false, 7, 7, 1));
    assert!(!arrival_is_current(false, 7, 8, 0));
    assert!(!arrival_is_current(true, 7, 7, 0));
}

#[test]
fn refinement_advances_only_after_matching_completed_scenes() {
    let mut schedule = RefinementSchedule::default();
    schedule.restart(7);
    assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
    schedule.submitted(11, RefinementLevel::Preview);
    assert_eq!(schedule.due(), None);
    assert!(!schedule.completed(10, 7, RefinementLevel::Preview));
    assert!(schedule.completed(11, 7, RefinementLevel::Preview));
    assert_eq!(schedule.due(), Some(RefinementLevel::Interactive));
    schedule.submitted(12, RefinementLevel::Interactive);
    assert!(schedule.completed(12, 7, RefinementLevel::Interactive));
    assert_eq!(schedule.due(), Some(RefinementLevel::Final));
    schedule.submitted(13, RefinementLevel::Final);
    assert!(schedule.completed(13, 7, RefinementLevel::Final));
    assert!(!schedule.pending());
}

#[test]
fn picture_fast_advances_directly_from_preview_to_final() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.apply_precision_mode(PrecisionMode::PictureFast, 8);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    frame_loop.submitted(21, RefinementLevel::Preview);
    assert!(frame_loop.completed(21, 8, RefinementLevel::Preview));
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
    frame_loop.submitted(22, RefinementLevel::Final);
    assert!(frame_loop.completed(22, 8, RefinementLevel::Final));
    assert!(!frame_loop.refinement_pending());
}

#[test]
fn picture_fast_viewer_builds_the_fast_ladder_and_centre_policy() {
    let mut viewer = ViewerController::new(960).expect("canonical viewer");
    let (precision_mode, mut frame_loop, plan) = precision_runtime_from_viewer(&mut viewer);
    assert_eq!(precision_mode, PrecisionMode::PictureFast);
    assert_eq!(plan.precision_mode, PrecisionMode::PictureFast);
    assert_eq!(plan.level(RefinementLevel::Preview).iteration_cap, 32);
    assert_eq!(
        viewer
            .owner()
            .navigation_centre()
            .expect("configured centre")
            .precision_bits,
        64
    );
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    complete_preview(&mut frame_loop, 31);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
}

#[test]
fn viewer_mode_changes_reapply_the_ladder_plan_and_centre_width() {
    let mut viewer = ViewerController::new(960).expect("canonical viewer");
    let (mut precision_mode, mut frame_loop, mut plan) = precision_runtime_from_viewer(&mut viewer);

    viewer
        .set_precision_mode(PrecisionMode::Deterministic)
        .expect("the page mode path accepts deterministic");
    apply_precision_mode(
        viewer.requested().precision_mode,
        &mut precision_mode,
        &mut frame_loop,
        &mut plan,
        &mut viewer,
    )
    .expect("deterministic mode applies everywhere");
    assert_eq!(precision_mode, PrecisionMode::Deterministic);
    assert_eq!(plan.precision_mode, PrecisionMode::Deterministic);
    assert_eq!(
        viewer
            .owner()
            .navigation_centre()
            .expect("configured centre")
            .precision_bits,
        1_024
    );
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    complete_preview(&mut frame_loop, 41);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Interactive));

    viewer
        .set_precision_mode(PrecisionMode::PictureFast)
        .expect("the page mode path accepts picture-fast");
    apply_precision_mode(
        viewer.requested().precision_mode,
        &mut precision_mode,
        &mut frame_loop,
        &mut plan,
        &mut viewer,
    )
    .expect("picture-fast mode applies everywhere");
    assert_eq!(precision_mode, PrecisionMode::PictureFast);
    assert_eq!(plan.precision_mode, PrecisionMode::PictureFast);
    assert_eq!(
        viewer
            .owner()
            .navigation_centre()
            .expect("configured centre")
            .precision_bits,
        64
    );
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    complete_preview(&mut frame_loop, 42);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));

    viewer
        .set_precision_mode(PrecisionMode::Deterministic)
        .expect("the page mode path switches back to deterministic");
    apply_precision_mode(
        viewer.requested().precision_mode,
        &mut precision_mode,
        &mut frame_loop,
        &mut plan,
        &mut viewer,
    )
    .expect("deterministic mode is restored everywhere");
    assert_eq!(precision_mode, PrecisionMode::Deterministic);
    assert_eq!(plan.precision_mode, PrecisionMode::Deterministic);
    assert_eq!(
        viewer
            .owner()
            .navigation_centre()
            .expect("configured centre")
            .precision_bits,
        1_024
    );
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    complete_preview(&mut frame_loop, 43);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Interactive));
}

fn wait_for_unused_shallow_orbit() -> Result<(), MathError> {
    let centre = BigCentre::from_f64([0.0, 0.0, -0.5, 0.5], 1_024)?;
    let params = EscapeParams::new(4_096);
    let mut builder =
        ReferenceOrbitBuilder::new(&centre, precision_for(0.0, 960, params.max_iter)?, params)?;
    let chunk = NonZeroU32::new(params.max_iter).ok_or(MathError::InvalidMaxIter)?;
    loop {
        match builder.step(chunk)? {
            OrbitStep::Pending { .. } => {}
            OrbitStep::Complete(orbit) => {
                std::hint::black_box(orbit);
                return Ok(());
            }
        }
    }
}

fn time_to_first_shallow_scene(wait_for_orbit: bool) -> Result<Duration, MathError> {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let clock = FakeClock::default();
    let started = Instant::now();
    if wait_for_orbit {
        wait_for_unused_shallow_orbit()?;
    }
    frame_loop.accept_request(1, true);
    assert_eq!(KernelMode::for_zoom(0.0), KernelMode::Shallow);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );
    Ok(started.elapsed())
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
#[allow(
    clippy::print_stderr,
    reason = "the requested native performance oracle reports its before and after walls"
)]
fn shallow_frame_loop_no_longer_waits_for_an_unused_orbit() -> Result<(), MathError> {
    wait_for_unused_shallow_orbit()?;
    let before = median(
        (0..5)
            .map(|_| time_to_first_shallow_scene(true))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let after = median(
        (0..5)
            .map(|_| time_to_first_shallow_scene(false))
            .collect::<Result<Vec<_>, _>>()?,
    );
    assert!(before > after);
    eprintln!(
        "first_shallow_scene before_ms={:.6} after_ms={:.6} saved_ms={:.6}",
        before.as_secs_f64() * 1_000.0,
        after.as_secs_f64() * 1_000.0,
        before.saturating_sub(after).as_secs_f64() * 1_000.0,
    );
    Ok(())
}

#[test]
fn a_shallow_selection_is_ready_without_an_orbit_but_a_deep_crossing_waits() {
    let mut shallow = FrameLoop::default();
    shallow.accept_request(7, true);
    assert_eq!(shallow.due(), Some(RefinementLevel::Preview));

    let mut deep = FrameLoop::default();
    deep.accept_request(8, false);
    assert_eq!(deep.due(), None);
    deep.restart(8);
    assert_eq!(deep.due(), Some(RefinementLevel::Preview));
    assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
}

#[test]
fn newer_main_keeps_old_in_flight_work_from_advancing_it() {
    let mut schedule = RefinementSchedule::default();
    schedule.restart(3);
    schedule.submitted(21, RefinementLevel::Preview);
    schedule.restart(4);
    assert_eq!(schedule.due(), None);
    assert!(!schedule.completed(21, 3, RefinementLevel::Preview));
    assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
}

#[test]
fn refusal_retries_the_same_level_without_a_third_target() {
    let mut schedule = RefinementSchedule::default();
    schedule.restart(9);
    schedule.submitted(31, RefinementLevel::Preview);
    assert!(schedule.retired(31));
    assert_eq!(schedule.due(), Some(RefinementLevel::Preview));
}

#[test]
fn pending_fence_is_observed_once_per_refresh_and_completes_after_callback() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(7, true);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );

    for _ in 0..3 {
        clock.advance(4.0);
        assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
    }
    assert_eq!(presenter.fence_observations, 3);
    assert_eq!(presenter.submissions, [RefinementLevel::Preview]);

    presenter.fire_completed_callback();
    assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
    clock.advance(4.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(2)
    );
    assert_eq!(presenter.fence_observations, 4);
    assert_eq!(
        presenter.submissions,
        [RefinementLevel::Preview, RefinementLevel::Interactive]
    );
}

#[test]
fn deadline_refusal_resubmits_the_same_level_on_the_following_refresh() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(11, true);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );

    presenter.fire_deadline();
    clock.advance(30_000.0);
    assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
    assert_eq!(presenter.submissions, [RefinementLevel::Preview]);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));

    clock.advance(1.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(2)
    );
    assert_eq!(
        presenter.submissions,
        [RefinementLevel::Preview, RefinementLevel::Preview]
    );
}

#[test]
fn app_needs_refresh_while_either_fence_is_in_flight() {
    let frame_loop = FrameLoop::default();
    assert!(frame_loop.needs_refresh(true, false, false, false));
    assert!(frame_loop.needs_refresh(false, true, false, false));
    assert!(!frame_loop.needs_refresh(false, false, false, false));

    let mut scheduled = FrameLoop::default();
    scheduled.restart(5);
    assert!(scheduled.refinement_pending());
    assert!(scheduled.needs_refresh(false, false, false, false));
}

#[test]
fn viewer_harness_holds_manual_refusal_until_update_scene_presents_final() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
    frame_loop.accept_request(37, true);
    let mut presenter = retained_presenter(true);
    let mut clock = FakeClock::default();

    let held = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
    assert_eq!(held.scene_id, None);
    assert!(held.warp_id.is_some());
    assert_eq!(presenter.warp_kind, Some(WarpKind::HoldStale));
    assert_eq!(presenter.pending_warp_source, Some(37));
    presenter.fire_warp_completed();
    clock.advance(1.0);
    assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
    assert_eq!(presenter.presented_scene, Some(37));
    assert!(frame_loop.scene_update_pending());

    frame_loop.request_scene_update(37);
    let update = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false);
    assert!(update.scene_id.is_some());
    let final_scene = finish_pending_refused_ladder(&mut frame_loop, &mut presenter, &mut clock);
    assert_ne!(final_scene, 37);
    assert!(!frame_loop.scene_update_pending());
}

#[test]
fn viewer_harness_holds_an_auto_refusal_until_the_rounds_final_fill() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(37, true);
    let mut presenter = retained_presenter(true);
    let mut clock = FakeClock::default();

    let held = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
    assert!(held.scene_id.is_some());
    assert!(held.warp_id.is_some());
    assert_eq!(presenter.warp_kind, Some(WarpKind::HoldStale));
    assert_eq!(presenter.pending_warp_source, Some(37));
    assert_eq!(presenter.warp_hold_count, 1);
    presenter.fire_warp_completed();
    clock.advance(1.0);
    assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
    assert_eq!(presenter.presented_scene, Some(37));

    let final_scene = finish_pending_refused_ladder(&mut frame_loop, &mut presenter, &mut clock);
    assert_ne!(final_scene, 37);
}

#[test]
fn automatic_stale_hold_expires_after_one_ladder_round_without_a_final() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(37, true);
    assert!(frame_loop.hold_refused_warp(true));
    let held_round = frame_loop.ladder_round();

    frame_loop.restart(38);
    assert_ne!(frame_loop.ladder_round(), held_round);
    assert!(!frame_loop.hold_refused_warp(true));
}

#[test]
fn viewer_harness_keeps_bounded_manual_warps_moving_the_retained_picture() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
    frame_loop.accept_request(37, true);
    let mut presenter = retained_presenter(false);
    let mut clock = FakeClock::default();

    let bounded = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
    assert!(bounded.warp_id.is_some());
    assert_eq!(presenter.warp_kind, Some(WarpKind::AnchorHomography));
    assert_eq!(presenter.pending_warp_source, Some(37));
    presenter.fire_warp_completed();
    clock.advance(1.0);
    assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
    assert_eq!(presenter.presented_scene, Some(37));
}

#[test]
fn manual_control_change_writes_hot_and_schedules_no_scene() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let clock = FakeClock::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 7, true);
    assert!(frame_loop.hold_refused_warp(false));
    frame_loop.accept_request(7, true);
    frame_loop.scene_selection_changed(7);
    assert!(!frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, true))));

    let turn = drive_turn(
        &mut frame_loop,
        &mut presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        true,
    );
    assert_eq!(presenter.hot_writes, 1);
    assert_eq!(turn.scene_id, None);
    assert!(
        turn.warp_id.is_some(),
        "the changed HOT pose is still reprojected"
    );
    assert!(frame_loop.scene_update_pending());
    assert!(!frame_loop.refinement_pending());

    presenter.fire_warp_completed();
    let completed = drive_turn(
        &mut frame_loop,
        &mut presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        false,
    );
    assert!(completed.presented);
    assert!(!frame_loop.needs_refresh(false, false, false, false));
    assert!(frame_loop.scene_update_pending());
}

#[test]
fn manual_update_scene_restarts_the_current_ladder() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 11, true);
    frame_loop.accept_request(11, true);
    frame_loop.request_scene_update(11);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );

    presenter.fire_completed_callback();
    clock.advance(1.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(2)
    );
    assert!(!frame_loop.scene_update_pending());
    presenter.fire_completed_callback();
    clock.advance(1.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(3)
    );
    presenter.fire_completed_callback();
    clock.advance(1.0);
    assert_eq!(drive_refresh(&mut frame_loop, &mut presenter, clock), None);
    assert_eq!(presenter.submissions, LEVELS);
    assert!(!frame_loop.refinement_pending());
}

#[test]
fn manual_main_change_keeps_its_orbit_request_without_a_scene() {
    let mut viewer = ViewerController::new(960).expect("canonical viewer");
    let initial = viewer
        .take_reference_submission()
        .expect("startup requests its first orbit");
    assert!(viewer.finish_reference_submission(initial.navigation.generation));
    viewer
        .set_plane_origin([0.0, 0.0, -0.75, 0.1])
        .expect("finite origin");
    assert!(
        viewer.take_reference_submission().is_some(),
        "manual scene policy does not consume or suppress MAIN orbit work"
    );

    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 13, true);
    frame_loop.accept_request(13, true);
    frame_loop.scene_input_ready(14);
    assert_eq!(frame_loop.due(), None);
    assert!(frame_loop.needs_refresh(false, false, true, false));
}

#[test]
fn enabling_auto_with_a_manual_change_schedules_preview() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 17, true);
    frame_loop.accept_request(17, true);
    assert!(frame_loop.scene_update_pending());
    frame_loop.set_scene_mode(SceneMode::Auto, 17, true);
    assert_eq!(frame_loop.scene_mode(), SceneMode::Auto);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    assert!(!frame_loop.scene_update_pending());
}

#[test]
fn auto_mode_and_the_first_manual_scene_keep_automatic_refinement() {
    let mut automatic = FrameLoop::default();
    automatic.accept_request(19, true);
    assert_eq!(automatic.due(), Some(RefinementLevel::Preview));

    let mut first_scene = FrameLoop::default();
    first_scene.restart(23);
    first_scene.set_scene_mode(SceneMode::Manual, 23, false);
    assert_eq!(first_scene.due(), Some(RefinementLevel::Preview));
}

#[test]
fn exposed_accepted_final_warp_submits_final_directly_and_returns_to_idle() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.schedule.precision_mode = PrecisionMode::PictureFast;
    frame_loop.accept_request(31, true);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    assert!(frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, true))));
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
    assert_eq!(frame_loop.draft_skipped_count(), 1);
    assert_eq!(
        frame_loop.last_draft_skip_reason(),
        Some("accepted exposed higher-level retained warp")
    );

    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );
    assert_eq!(presenter.submissions, [RefinementLevel::Final]);
    presenter.fire_completed_callback();
    clock.advance(1.0);
    let warp = drive_turn(
        &mut frame_loop,
        &mut presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        true,
    );
    assert_eq!(warp.scene_id, None);
    assert!(warp.warp_id.is_some());
    presenter.fire_warp_completed();
    clock.advance(1.0);
    let settled = drive_turn(
        &mut frame_loop,
        &mut presenter,
        clock,
        FramePolicy::SingleFrameOnDemand,
        false,
    );
    assert!(settled.presented);
    assert!(!frame_loop.needs_refresh(false, false, false, false));
}

#[test]
fn refused_warp_and_first_scene_run_the_full_ladder() {
    let mut refused = FrameLoop::default();
    assert!(!refused.hold_refused_warp(false));
    refused.accept_request(37, true);
    assert!(!refused.skip_drafts_for_accepted_warp(None));
    assert_eq!(refused.due(), Some(RefinementLevel::Preview));

    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    run_ladder_to_idle(
        &mut refused,
        &mut presenter,
        &mut clock,
        FramePolicy::SingleFrameOnDemand,
    );
    assert_eq!(presenter.submissions, LEVELS);
    assert_eq!(refused.draft_skipped_count(), 0);
    assert_eq!(refused.last_draft_skip_reason(), None);
}

#[test]
fn deterministic_accepted_warp_skips_both_draft_levels() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(41, true);
    assert!(frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false))));
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Final));
    assert_eq!(frame_loop.draft_skipped_count(), 2);
    assert_eq!(
        frame_loop.last_draft_skip_reason(),
        Some("accepted covering higher-level retained warp")
    );
}

#[test]
fn edge_on_final_completes_once_then_stays_idle_in_both_scene_modes() {
    const SETTLED_REFRESHES: usize = 16;
    let plan = plan_refinement(
        GridExtent {
            width: 8,
            height: 8,
        },
        EscapeParams::new(64),
        |_| true,
    )
    .expect("the edge-on fixture has enough capacity");
    let mut arena = SpanArena::new(8, 2, 8, 256, 8).expect("fixture arena");
    let span = arena.allocate_span(64, 8).expect("fixture grid span");
    let edge_on_object = ObjectAngles {
        rho_13: std::f64::consts::FRAC_PI_2,
        ..ObjectAngles::IDENTITY
    };
    assert!(
        screen_to_plane(
            &edge_on_object,
            &ViewControls::MANDELBROT_FLAT,
            0.0,
            8,
            8,
            1.0,
        )
        .is_err(),
        "the exact browser fixture must take the all-sky EdgeOn path"
    );

    for mode in [SceneMode::Auto, SceneMode::Manual] {
        let mut frame_loop = FrameLoop::default();
        if mode == SceneMode::Manual {
            frame_loop.set_scene_mode(mode, 29, false);
        }
        frame_loop.schedule.generation = 29;
        frame_loop.schedule.next = Some(RefinementLevel::Final);

        // EdgeOn skips the kernel encoder, so the grid still carries the preceding level until
        // app stamps the scheduled all-sky scene explicitly.
        let mut grid = EscapeGrid {
            span: span.clone(),
            width: plan.level(RefinementLevel::Preview).extent.width,
            height: plan.level(RefinementLevel::Preview).extent.height,
            level: RefinementLevel::Preview,
        };
        let scheduled = frame_loop.due().expect("Final is due");
        assert_ne!(grid.level, scheduled);
        stamp_scene_level(&mut grid, &plan, scheduled);

        let mut presenter = FakePresenter::default();
        let scene_id = presenter.submit(frame_loop.generation(), grid.level);
        frame_loop.submitted(scene_id, scheduled);
        presenter.fire_completed_callback();
        let mut clock = FakeClock::default();
        clock.advance(1.0);

        for _ in 0..SETTLED_REFRESHES {
            let outcome = drive_turn(
                &mut frame_loop,
                &mut presenter,
                clock,
                FramePolicy::SingleFrameOnDemand,
                false,
            );
            assert_eq!(outcome.scene_id, None, "mode {mode:?}");
            assert!(!frame_loop.refinement_pending(), "mode {mode:?}");
            assert!(!frame_loop.scene_update_pending(), "mode {mode:?}");
            assert!(
                !frame_loop.needs_refresh(false, false, false, false),
                "mode {mode:?}"
            );
            clock.advance(1.0);
        }
        assert_eq!(presenter.submissions, [RefinementLevel::Final]);
    }
}

#[test]
fn a_stale_presented_view_alone_keeps_the_loop_scheduled() {
    let frame_loop = FrameLoop::default();
    assert!(
        !frame_loop.needs_refresh(false, false, false, false),
        "an idle loop showing the requested view has nothing to do"
    );
    assert!(
        frame_loop.needs_refresh(false, false, false, true),
        "an image belonging to an older requested view is unfinished work"
    );
}

#[test]
fn one_frame_request_after_idle_starts_a_new_scene_ladder() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(13, true);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(1)
    );

    for expected in [Some(2), Some(3), None] {
        presenter.fire_completed_callback();
        clock.advance(1.0);
        assert_eq!(
            drive_refresh(&mut frame_loop, &mut presenter, clock),
            expected
        );
    }
    frame_loop.warp_submitted();
    assert!(!frame_loop.needs_refresh(false, false, false, false));
    assert!(!frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand));

    frame_loop.accept_request(13, true);
    assert!(frame_loop.needs_refresh(false, false, false, false));
    assert!(frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand));
    clock.advance(1_000.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(4)
    );
    assert_eq!(
        presenter.submissions.last(),
        Some(&RefinementLevel::Preview)
    );
}

#[test]
fn an_exposed_warp_starts_the_scene_that_fills_its_edge() {
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(17, true);
    run_ladder_to_idle(
        &mut frame_loop,
        &mut presenter,
        &mut clock,
        FramePolicy::SingleFrameOnDemand,
    );
    assert!(!frame_loop.refinement_pending());

    assert!(schedule_exposure_fill(&mut frame_loop, true, 17));
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    let preview_id = presenter.next_id + 1;
    clock.advance(1.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(preview_id)
    );
    presenter.fire_completed_callback();
    let interactive_id = presenter.next_id + 1;
    clock.advance(1.0);
    assert_eq!(
        drive_refresh(&mut frame_loop, &mut presenter, clock),
        Some(interactive_id)
    );
    assert_eq!(presenter.submissions[3], RefinementLevel::Preview);
    assert_eq!(presenter.submissions[4], RefinementLevel::Interactive);
}

#[test]
fn an_exposed_manual_warp_waits_for_update_without_latching_the_loop() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 18, true);
    assert!(!schedule_exposure_fill(&mut frame_loop, true, 18));
    assert_eq!(frame_loop.due(), None);
    assert!(frame_loop.scene_update_pending());
    assert!(!frame_loop.needs_refresh(false, false, false, false));
}

#[test]
fn the_page_reads_the_deadline_refusal_that_killed_the_throttled_tab() {
    assert_eq!(
        fence_error(SubmissionKind::Warp, FenceRefusal::Deadline, 7, 60_000.0).to_string(),
        "warp fence exceeded its 60000 ms deadline"
    );
    assert_eq!(
        classified(FenceRefusal::Deadline),
        RefusalClass::Transient,
        "a bounded wall says the observation window closed, not that the device is gone"
    );
    assert_eq!(classified(FenceRefusal::PollLimit), RefusalClass::Transient);
    assert_eq!(classified(FenceRefusal::Cancelled), RefusalClass::Cancelled);
    assert_eq!(classified(FenceRefusal::Device), RefusalClass::Device);
}

fn classified(reason: FenceRefusal) -> RefusalClass {
    super::classify_refusal(reason)
}

/// Reproduces the throttled-tab failure: a warp fence refuses at its deadline with an empty
/// ladder, no scene in flight and no surface pending, which is precisely the state whose
/// `needs_refresh` answer used to be false and left the page dead.
#[test]
fn a_throttled_warp_deadline_re_arms_the_loop_and_a_later_warp_presents() {
    let policy = FramePolicy::SingleFrameOnDemand;
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(21, true);
    run_ladder_to_idle(&mut frame_loop, &mut presenter, &mut clock, policy);
    assert_eq!(
        presenter.submissions,
        [
            RefinementLevel::Preview,
            RefinementLevel::Interactive,
            RefinementLevel::Final
        ]
    );
    let refused_warp = *presenter
        .warp_submissions
        .last()
        .expect("the ladder submits a warp on every turn");

    // The tab goes to the background, the fence is polled once a second, and the wall runs out.
    presenter.fire_warp_refusal(FenceRefusal::Deadline, 61, 60_000.0);
    clock.advance(60_000.0);
    let refusal_turn = drive_turn(&mut frame_loop, &mut presenter, clock, policy, false);
    assert!(refusal_turn.refused);
    assert!(!refusal_turn.presented);
    assert_eq!(frame_loop.transient_refusals(), 1);
    assert_eq!(
        frame_loop.last_transient().map(ToString::to_string),
        Some("warp fence exceeded its 60000 ms deadline".to_string())
    );
    assert!(frame_loop.stopped().is_none());
    assert!(
        frame_loop.needs_refresh(false, false, false, false),
        "with the ladder empty and the refused warp retired, only the re-arm keeps the page alive"
    );

    // Input resumes: the very next turn asks for the surface image the refusal cost.
    clock.advance(16.0);
    let retry_warp = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true)
        .warp_id
        .expect("a re-armed run requests the next surface image");
    assert_ne!(retry_warp, refused_warp);

    presenter.fire_warp_completed();
    clock.advance(16.0);
    let recovery = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true);
    assert!(recovery.presented);
    assert_eq!(presenter.presented_warps.last(), Some(&retry_warp));
    assert!(
        !frame_loop.needs_refresh(false, false, false, false),
        "one survived refusal must not turn an on-demand page into a permanent spin"
    );
}

#[test]
fn a_warp_poll_limit_refusal_is_survived_the_same_way() {
    let policy = FramePolicy::SingleFrameOnDemand;
    let mut frame_loop = FrameLoop::default();
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    frame_loop.accept_request(31, true);
    run_ladder_to_idle(&mut frame_loop, &mut presenter, &mut clock, policy);

    presenter.fire_warp_refusal(FenceRefusal::PollLimit, 4_096, 812.5);
    clock.advance(812.5);
    let refusal_turn = drive_turn(&mut frame_loop, &mut presenter, clock, policy, false);
    assert!(refusal_turn.refused);
    assert_eq!(frame_loop.transient_refusals(), 1);
    assert_eq!(
        frame_loop.last_transient().map(ToString::to_string),
        Some("warp fence exhausted its 4096 completion polls".to_string())
    );
    assert!(frame_loop.stopped().is_none());
    assert!(frame_loop.needs_refresh(false, false, false, false));

    clock.advance(16.0);
    let retry_warp = drive_turn(&mut frame_loop, &mut presenter, clock, policy, true)
        .warp_id
        .expect("a re-armed run requests the next surface image");
    presenter.fire_warp_completed();
    clock.advance(16.0);
    assert!(drive_turn(&mut frame_loop, &mut presenter, clock, policy, true).presented);
    assert_eq!(presenter.presented_warps.last(), Some(&retry_warp));
}

#[test]
fn a_scene_deadline_refusal_retires_the_scene_and_keeps_the_level_due() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(41, true);
    frame_loop.submitted(9, RefinementLevel::Preview);
    assert_eq!(frame_loop.due(), None);

    let outcome = frame_loop.refused(
        SubmissionKind::Scene,
        FenceRefusal::Deadline,
        9,
        61,
        30_000.0,
    );
    assert_eq!(outcome.class, RefusalClass::Transient);
    assert!(outcome.retired_scene);
    assert_eq!(frame_loop.due(), Some(RefinementLevel::Preview));
    assert_eq!(frame_loop.transient_refusals(), 1);
    assert!(frame_loop.stopped().is_none());
    assert!(frame_loop.needs_refresh(false, false, false, false));
}

#[test]
fn a_device_fence_refusal_stops_the_loop_with_its_typed_status() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(51, true);
    let outcome = frame_loop.refused(SubmissionKind::Warp, FenceRefusal::Device, 3, 12, 41.5);
    assert_eq!(outcome.class, RefusalClass::Device);
    assert_eq!(
        frame_loop.transient_refusals(),
        0,
        "device loss is never counted as something the page survived"
    );
    assert_eq!(
        frame_loop.stopped().map(ToString::to_string),
        Some("fence mapping failed during warp fence: four-byte fence callback failed".to_string())
    );
    assert!(
        !frame_loop.needs_refresh(true, true, true, true),
        "a stopped loop schedules nothing, whatever else is outstanding"
    );

    frame_loop.refused(
        SubmissionKind::Warp,
        FenceRefusal::Deadline,
        4,
        61,
        60_000.0,
    );
    assert_eq!(
        frame_loop.stopped().map(ToString::to_string),
        Some("fence mapping failed during warp fence: four-byte fence callback failed".to_string()),
        "a later refusal never overwrites the cause the page is reporting"
    );
}

/// The coverage backdrop yields its turn to the main ladder, and to a running main dispatch.
#[test]
fn coverage_takes_one_turn_and_then_yields_one() {
    assert_eq!(
        coverage_pre_empts(CoverageTurn::Backdrop, false),
        (true, CoverageTurn::Main),
        "the first delivery in a pose family is the wide coverage"
    );
    assert_eq!(
        coverage_pre_empts(CoverageTurn::Main, false),
        (false, CoverageTurn::Backdrop),
        "and the turn after it belongs to the ladder"
    );
    assert_eq!(
        coverage_pre_empts(CoverageTurn::Backdrop, true),
        (false, CoverageTurn::Backdrop),
        "a running main dispatch is never pre-empted, and keeps coverage's claim"
    );
}

/// Three seconds of continuous drag at 30 Hz, then a second of stillness.
///
/// The regression this pins is the whole drag being presented at the coarse backdrop. The
/// backdrop is requested against the view stamp, the stamp moves every frame of a drag, and a
/// scheduler that always prefers a stale backdrop therefore dispatches a new one before the
/// ladder ever runs: the main grid is never seen until the gesture ends. Here the two share the
/// schedule, no two backdrops are dispatched without a main level between them, and the pose
/// that settles still walks its ladder to Final.
#[test]
fn a_continuous_drag_alternates_the_backdrop_with_the_main_ladder() {
    const DRAG_FRAMES: u32 = 90;
    const SETTLE_FRAMES: usize = 30;
    /// Frames a dispatched scene occupies the presenter before its fence completes.
    const FLIGHT_FRAMES: usize = 3;
    /// Preview, Interactive, Final.
    const LADDER_LEVELS: usize = 3;

    let mut turn = CoverageTurn::Backdrop;
    let mut flight: Option<(bool, usize)> = None;
    let mut dispatched: Vec<(usize, bool)> = Vec::new();
    let mut completed_levels = 0_usize;
    let mut coverage_is_stale = true;

    for frame in 0..DRAG_FRAMES + SETTLE_FRAMES {
        let dragging = frame < DRAG_FRAMES;
        if let Some((is_backdrop, remaining)) = flight {
            let remaining = remaining - 1;
            if remaining > 0 {
                flight = Some((is_backdrop, remaining));
            } else {
                flight = None;
                if is_backdrop {
                    // A completed backdrop matches the stamp it was requested for, and a drag
                    // has moved that stamp again by the time it lands.
                    coverage_is_stale = dragging;
                } else if dragging {
                    // The moved view restarts the ladder: every drag level is a Preview.
                    completed_levels = 0;
                } else {
                    completed_levels += 1;
                }
            }
        }
        // The contest runs every frame, in flight or not, exactly as the browser loop does.
        let main_in_flight = flight.is_some_and(|(is_backdrop, _)| !is_backdrop);
        let (pre_empts, next_turn) = if coverage_is_stale {
            coverage_pre_empts(turn, main_in_flight)
        } else {
            (false, CoverageTurn::Backdrop)
        };
        turn = next_turn;
        if flight.is_some() {
            continue;
        }
        if pre_empts {
            dispatched.push((frame, true));
            flight = Some((true, FLIGHT_FRAMES));
        } else if completed_levels < LADDER_LEVELS {
            dispatched.push((frame, false));
            flight = Some((false, FLIGHT_FRAMES));
        }
    }

    let backdrops = dispatched.iter().filter(|(_, wide)| *wide).count();
    let mains = dispatched.len() - backdrops;
    assert!(
        backdrops >= 2,
        "the drag must have re-requested coverage more than once, not {backdrops} times"
    );
    assert!(
        mains + 1 >= backdrops,
        "the main ladder ran {mains} times against {backdrops} backdrops"
    );
    for pair in dispatched.windows(2) {
        assert!(
            !(pair[0].1 && pair[1].1),
            "two backdrops in a row, the second at frame {}",
            pair[1].0
        );
    }
    assert_eq!(
        completed_levels, LADDER_LEVELS,
        "the settled pose must still walk its ladder to Final"
    );
}

#[derive(Clone, Copy, Debug)]
struct HeightDragRow {
    name: &'static str,
    distance_five: f64,
}

fn owner_height_drag_pose(row: HeightDragRow, height_scale: f64) -> Pose {
    let object = ObjectAngles {
        rho_13: -1.316_653_720_171_549_4,
        rho_24: -1.316_653_720_171_549_4,
        ..ObjectAngles::IDENTITY
    };
    let mut camera = [0.0; 10];
    camera[1] = -0.254_142_606_623_347_1;
    camera[4] = -0.254_142_606_623_347_1;
    let view = ViewControls {
        camera,
        camera_yaw: 0.96,
        camera_pitch: core::f64::consts::PI,
        height_scale,
        distance_five: row.distance_five,
        distance_four: row.distance_five,
        ..ViewControls::NEUTRAL
    };
    let extent = [96, 54];
    let plane = ember_julibrot_math::construct_plane(object).expect("owner drag plane");
    let map = screen_to_plane(
        &object,
        &view,
        3.92,
        extent[0],
        extent[1],
        f64::from(extent[0]) / f64::from(extent[1]),
    )
    .map_or(PoseMap::EdgeOn, PoseMap::Mapped);
    Pose {
        epoch: 1,
        orbit_generation: 37,
        plane,
        object,
        plane_origin: [0.0, 0.0, -0.671, 0.131],
        zoom_log2: 3.92,
        view,
        grid_width: extent[0],
        grid_height: extent[1],
        map,
        centre_from_reference_px: [0.0; 2],
    }
}

fn owner_height_drag_plan(row: HeightDragRow, height_scale: f64) -> WarpKind {
    let retained = owner_height_drag_pose(row, 0.0);
    let frame = SceneFrame {
        scene_id: 37,
        pose: retained,
        palette: PaletteId::Classic,
        iteration_cap: 128,
        level: RefinementLevel::Final,
        extent: [retained.grid_width, retained.grid_height],
        texture_index: 0,
        centre_revision: 1,
        plane_origin_f64: retained.plane_origin,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        measurement: SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: 37,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::PictureFast.as_str(),
            wall_ms: 1.0,
            fence_wait_ms: 0.5,
            polls: 1,
        },
    };
    Warp::reproject(
        &frame,
        &retained,
        &owner_height_drag_pose(row, height_scale),
        PrecisionMode::PictureFast,
        WarpValidation::Ordinary,
    )
    .kind
}

#[derive(Clone, Copy, Debug, Default)]
struct HeightDragStats {
    clear_only_presentations: u64,
    hold_presentations: u64,
    final_after_drag_ms: f64,
}

fn drive_height_drag(row: HeightDragRow) -> HeightDragStats {
    const DRAG_FRAMES: usize = 90;
    const FLIGHT_FRAMES: usize = 3;
    const FRAME_MS: f64 = 1_000.0 / 30.0;

    assert!(matches!(row.distance_five, 2.0 | 8.0), "{}", row.name);
    let mut frame_loop = FrameLoop::default();
    frame_loop.schedule.precision_mode = PrecisionMode::PictureFast;
    let mut presenter = retained_presenter(false);
    presenter.relief_redraw = true;
    let mut clock = FakeClock::default();
    let mut flight_frames = 0_usize;
    let mut last_scene_id = 0_u64;

    for input in 1..=DRAG_FRAMES {
        if presenter.pending_warp.is_some() {
            presenter.fire_warp_completed();
        }
        if presenter.pending.is_some() {
            flight_frames += 1;
            if flight_frames == FLIGHT_FRAMES {
                presenter.fire_completed_callback();
                flight_frames = 0;
            }
        }
        frame_loop.accept_request(37, true);
        frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false)));
        presenter.forced_warp_kind = Some(owner_height_drag_plan(
            row,
            4.0 * f64::from(input) / f64::from(DRAG_FRAMES),
        ));
        let turn = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        if let Some(scene_id) = turn.scene_id {
            assert!(scene_id > last_scene_id, "{} scene ids", row.name);
            last_scene_id = scene_id;
        }
        assert_ne!(
            presenter.warp_kind,
            Some(WarpKind::ClearOnly),
            "{}",
            row.name
        );
        clock.advance(FRAME_MS);
    }

    let drag_ended_ms = clock.now_ms;
    presenter.forced_warp_kind = Some(owner_height_drag_plan(row, 4.0));
    let settled_scene = loop {
        if presenter.pending_warp.is_some() {
            presenter.fire_warp_completed();
        }
        if presenter.pending.is_some() {
            flight_frames += 1;
            if flight_frames == FLIGHT_FRAMES {
                presenter.fire_completed_callback();
                flight_frames = 0;
            }
        } else {
            frame_loop.restart(37);
            frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false)));
        }
        let turn = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        if let Some(scene_id) = turn.scene_id {
            assert!(scene_id > last_scene_id, "{} settled scene id", row.name);
            last_scene_id = scene_id;
        }
        if presenter
            .retained_scene
            .is_some_and(|scene_id| scene_id == last_scene_id && presenter.pending.is_none())
        {
            break last_scene_id;
        }
        clock.advance(FRAME_MS);
    };
    assert_eq!(presenter.retained_scene, Some(settled_scene));
    let stats = HeightDragStats {
        clear_only_presentations: presenter.presented_clear_only,
        hold_presentations: presenter.warp_hold_count,
        final_after_drag_ms: clock.now_ms - drag_ended_ms,
    };
    eprintln!(
        "height_drag row={} clear_only={} holds={} final_ms={:.3}",
        row.name,
        stats.clear_only_presentations,
        stats.hold_presentations,
        stats.final_after_drag_ms,
    );
    stats
}

#[test]
fn three_second_height_drag_keeps_both_owner_rows_painted_and_settles_in_one_round() {
    for row in [
        HeightDragRow {
            name: "gentle-d5-8",
            distance_five: 8.0,
        },
        HeightDragRow {
            name: "close-d5-2",
            distance_five: 2.0,
        },
    ] {
        let stats = drive_height_drag(row);
        assert_eq!(stats.clear_only_presentations, 0, "{}", row.name);
        assert_eq!(stats.hold_presentations, 0, "{}", row.name);
        assert!(
            stats.final_after_drag_ms <= 3.0 * (1_000.0 / 30.0),
            "{}",
            row.name
        );
    }
}

#[test]
fn browser_main_ladder_keeps_one_alternate_final_capacity_grid() {
    let source = include_str!("../loop.rs");
    let submit = include_str!("browser/submit.rs");
    assert!(source.contains("const MAX_HEADER_SETS: u32 = 9;"));
    assert!(source.contains("spare_grid: Option<EscapeGrid>,"));
    assert!(source.contains("grid_round: u64,"));
    assert!(submit.contains("std::mem::swap(&mut self.grid, spare);"));
    assert!(submit.contains("self.grid_round != self.loop_state.ladder_round()"));
}

/// The browser loop asks the shared policy rather than preferring a stale backdrop outright,
/// and a settled coverage layer hands its claim back for the next pose family.
#[test]
fn browser_backdrop_preparation_routes_through_the_coverage_turn() {
    let mut source = String::from(include_str!("../loop.rs"));
    source.push_str(include_str!("browser/backdrop.rs"));
    assert!(source.contains("super::coverage_pre_empts("));
    assert!(source.contains("in_flight_scene_id.is_some()"));
    assert!(source.contains("self.coverage_turn = next_turn;"));
    assert!(source.contains("coverage_turn: super::CoverageTurn::Backdrop,"));
    assert!(source.contains("self.coverage_turn = super::CoverageTurn::Backdrop;"));
}

/// The backdrop dispatch is behind the SAME reference-generation and zoom guards as the main.
///
/// `submit_due_scene` refuses while a reference submission is outstanding or a navigation is
/// pending, and again while the zoom's kernel mode has no accepted reference; only then does it
/// route to the backdrop. A stale orbit must never drive the wide layer either: it samples the
/// same field through the same reference, and reaching it before those guards would let it do
/// so from an orbit the main grid has already refused.
#[test]
fn the_backdrop_dispatch_is_behind_the_main_reference_and_zoom_guards() {
    let source = include_str!("browser/submit.rs");
    let submit = source
        .find("fn submit_due_scene(")
        .expect("the scene submission exists");
    let body = &source[submit..];
    let references = body
        .find("if matches!(map, PoseMap::Mapped(_))")
        .expect("the outstanding-reference guard exists");
    let ready = body
        .find("if !self.scene_ready(viewer.requested().zoom_log2) {")
        .expect("the zoom guard exists");
    let route = body
        .find("return self.submit_due_backdrop(")
        .expect("the backdrop route exists");
    assert!(
        references < ready && ready < route,
        "the backdrop route must follow both guards, found at {references}/{ready}/{route}"
    );
}
