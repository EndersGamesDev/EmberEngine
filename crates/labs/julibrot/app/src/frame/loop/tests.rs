use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use ember_julibrot_kernels::{
    EscapeGrid, GridExtent, KernelError, KernelMode, PerturbUniform, RefinementPlan, SampleStatus,
    perturb_scaled_pixel, plan_refinement,
};
use ember_julibrot_math::{
    BigCentre, EscapeGridRecord, EscapeParams, Homography, MathError, ObjectAngles, OrbitStep,
    Plane, Pose, PoseMap, PrecisionMode, ReferenceOrbitBuilder, ViewControls, pixel_scale,
    precision_for, scale_split, screen_to_plane,
};

use super::super::schedule::{
    PresentedTier, PromotionAction, SETTLED_DETERMINISTIC_PROMOTION_ENABLED,
    STATIC_SETTLE_WINDOW_MS, SettledPromotion,
};
use super::{
    BACKDROP_PRESENT_LEVEL, BrowserRefreshOrder, CoverageTurn, FenceRefusal, FrameLoop, LEVELS,
    PresenterPoll, REFERENCE_RECORD_BYTES, REFERENCE_TEXEL_BYTES, ReferenceLeaseIdentity,
    RefinementLevel, RefinementSchedule, RefusalClass, SceneMode, SubmissionKind,
    accepted_reference_facts, apply_precision_mode, arrival_is_current, backdrop_extent,
    coverage_pre_empts, defer_scene_until_relief_redraw, expand_reference_texels_into, fence_error,
    hold_redraw_during_scene, horizon_facts, main_for_grid, optional_backdrop_plan,
    perturbation_reference_is_current, published_iteration_cap,
    reference_submission_requires_worker, renew_reference_lease_identity, sampling_zoom_log2,
    schedule_exposure_fill, select_reference_candidate, stamp_scene_level, stamped_extent,
    stamped_screen_map, view_projection_changed, warp_submission_due,
};
use crate::{
    AppError, CaptureArming, FramePolicy, LevelTimingLedger, PendingSurface, PictureState,
    SurfaceAction, SurfaceState, ViewerController, anchor_px_up, box_zoom_delta_log2,
};
use ember_julibrot_present::{
    LatticePair, SampleClass, SceneFrame, SubmissionMeasurement, Warp, WarpKind, WarpRefusalReason,
    WarpValidation, relief_redraw_source_covers_destination, renders_same_picture,
};
use ember_julibrot_worker::ReferenceVerification;
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
    let plane = Plane {
        basis_u: [0.0, 0.0, 1.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let current = ReferenceLeaseIdentity {
        main_generation: 14,
        source_generation: 14,
        centre_revision: 7,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: 128,
        orbit_length: EXHAUSTED_AT,
    };
    assert_eq!(KernelMode::for_zoom(12.0), KernelMode::Shallow);
    assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    assert!(!perturbation_reference_is_current(
        12,
        7,
        plane,
        PrecisionMode::PictureFast as u32,
        128,
        CAP,
        Some(current)
    ));
    assert!(perturbation_reference_is_current(
        14,
        7,
        plane,
        PrecisionMode::PictureFast as u32,
        128,
        CAP,
        Some(current)
    ));

    let precision = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
    let view_centre = BigCentre::from_f64(
        [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
        precision.requested_bits,
    )
    .expect("finite seahorse centre");
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
    let widened = sampling_zoom_log2(zoom, 1.25).expect("selected backdrop");
    assert_eq!(widened, zoom - 1.25_f64.log2());
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TraceFenceKind {
    Scene,
    Warp,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TraceFenceResult {
    Pending,
    SceneCompleted {
        generation: u32,
        level: RefinementLevel,
    },
    WarpCompleted {
        kind: Option<WarpKind>,
        source_scene_id: Option<u64>,
    },
    Refused {
        reason: FenceRefusal,
        polls: u32,
        wall_ms: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TraceFenceObservation {
    order: u32,
    kind: TraceFenceKind,
    id: u64,
    result: TraceFenceResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TraceHotWrite {
    slot_order: u32,
    drained_hot_epoch: u64,
    drained_main_epoch: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TraceSurfaceAction {
    #[default]
    None,
    Present {
        warp_id: u64,
    },
    Drop {
        warp_id: u64,
    },
    Ignore {
        warp_id: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TraceCaptureState {
    #[default]
    Idle,
    Armed,
    InFlight,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TracePendingState {
    Idle,
    Pending,
}

impl TracePendingState {
    const fn from_pending(pending: bool) -> Self {
        if pending { Self::Pending } else { Self::Idle }
    }
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
    warp_relief_redraw_count: u64,
    presented_clear_only: u64,
    hot_epoch: u64,
    main_epoch: u64,
    surface: SurfaceState<u64>,
    capture: TraceCaptureState,
    capture_scene: Option<u64>,
    fence_log: Vec<TraceFenceObservation>,
    hot_write_log: Vec<TraceHotWrite>,
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

    fn write_hot_for_slot(&mut self, hold_refused_warp: bool, slot_order: u32) {
        self.hot_writes += 1;
        self.hot_epoch = self.hot_epoch.saturating_add(1);
        self.main_epoch = self.main_epoch.saturating_add(1);
        self.hot_write_log.push(TraceHotWrite {
            slot_order,
            drained_hot_epoch: self.hot_epoch,
            drained_main_epoch: self.main_epoch,
        });
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

    fn submit_warp(&mut self, generation: u32) -> u64 {
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
        self.surface
            .claim(generation)
            .expect("the fake surface has one owner");
        self.surface
            .retain(PendingSurface {
                warp_id: self.next_id,
                generation,
                precision_mode: PrecisionMode::Deterministic.as_str(),
                frame: self.next_id,
            })
            .expect("the fake warp owns its surface");
        let capture = CaptureArming {
            armed: self.capture == TraceCaptureState::Armed,
            route_matches: true,
            readback_in_flight: self.capture == TraceCaptureState::InFlight,
            renderer_already_armed: false,
        };
        if capture.surface_due() {
            self.capture = TraceCaptureState::InFlight;
        }
        self.warp_submissions.push(self.next_id);
        self.next_id
    }

    const fn arm_capture(&mut self) {
        self.capture = TraceCaptureState::Armed;
    }

    const fn drain_frame_capture() {}

    const fn stage_frame_capture() {}

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
        if let Some(pending) = self.pending {
            self.fence_observations += 1;
            let result = match self.callback {
                Some(FakeEvent::Completed(scene)) => TraceFenceResult::SceneCompleted {
                    generation: scene.generation,
                    level: scene.level,
                },
                Some(FakeEvent::Deadline(_)) => TraceFenceResult::Refused {
                    reason: FenceRefusal::Deadline,
                    polls: SCENE_POLLS,
                    wall_ms: SCENE_DEADLINE_MS,
                },
                Some(FakeEvent::Refused {
                    reason,
                    polls,
                    wall_ms,
                    ..
                }) => TraceFenceResult::Refused {
                    reason,
                    polls,
                    wall_ms,
                },
                Some(FakeEvent::WarpCompleted(_)) | None => TraceFenceResult::Pending,
            };
            self.fence_log.push(TraceFenceObservation {
                order: u32::try_from(self.fence_log.len()).unwrap_or(u32::MAX),
                kind: TraceFenceKind::Scene,
                id: pending.id,
                result,
            });
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
        if let Some(pending_warp) = self.pending_warp {
            self.warp_fence_observations += 1;
            let result = match self.warp_callback {
                Some(FakeEvent::WarpCompleted(_)) => TraceFenceResult::WarpCompleted {
                    kind: self.pending_warp_kind,
                    source_scene_id: self.pending_warp_source,
                },
                Some(FakeEvent::Refused {
                    reason,
                    polls,
                    wall_ms,
                    ..
                }) => TraceFenceResult::Refused {
                    reason,
                    polls,
                    wall_ms,
                },
                Some(FakeEvent::Deadline(_)) => TraceFenceResult::Refused {
                    reason: FenceRefusal::Deadline,
                    polls: SCENE_POLLS,
                    wall_ms: SCENE_DEADLINE_MS,
                },
                Some(FakeEvent::Completed(_)) | None => TraceFenceResult::Pending,
            };
            self.fence_log.push(TraceFenceObservation {
                order: u32::try_from(self.fence_log.len()).unwrap_or(u32::MAX),
                kind: TraceFenceKind::Warp,
                id: pending_warp,
                result,
            });
            if let Some(event) = self.warp_callback.take() {
                if matches!(event, FakeEvent::WarpCompleted(_)) {
                    if self.pending_warp_kind == Some(WarpKind::ClearOnly) {
                        self.presented_clear_only = self.presented_clear_only.saturating_add(1);
                    }
                    if self.pending_warp_kind == Some(WarpKind::ReliefRedraw) {
                        self.warp_relief_redraw_count =
                            self.warp_relief_redraw_count.saturating_add(1);
                    }
                    let presented_scene = self.pending_warp_source.take();
                    self.presented_scene = presented_scene;
                    if self.capture == TraceCaptureState::InFlight {
                        self.capture = TraceCaptureState::Ready;
                        self.capture_scene = presented_scene;
                    }
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

/// Characterizes the capability-gated promotion state machine before browser-loop activation.
#[test]
fn settled_picture_fast_promotes_once_and_swaps_only_after_presentation() {
    let mut promotion = SettledPromotion::default();
    let mut clock = FakeClock::default();
    let revision = 7;
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            revision,
            PrecisionMode::PictureFast,
            true,
            true
        ),
        None
    );
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            revision,
            PrecisionMode::PictureFast,
            true,
            true
        ),
        None
    );
    clock.advance(STATIC_SETTLE_WINDOW_MS - 1.0);
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            revision,
            PrecisionMode::PictureFast,
            true,
            true
        ),
        None
    );
    clock.advance(1.0);
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            revision,
            PrecisionMode::PictureFast,
            true,
            true
        ),
        Some(PromotionAction::StartDeterministic {
            requested_revision: revision
        })
    );
    assert_eq!(
        promotion.effective_precision_mode(PrecisionMode::PictureFast),
        PrecisionMode::Deterministic
    );
    assert!(promotion.holds_fast_final());
    assert_eq!(promotion.presented_tier(), PresentedTier::Fast);
    assert!(promotion.deterministic_submitted(41));
    assert!(promotion.deterministic_completed(41));
    assert!(promotion.holds_fast_final());
    assert_eq!(promotion.presented_tier(), PresentedTier::Fast);
    assert!(promotion.deterministic_presented(41));
    assert!(!promotion.holds_fast_final());
    assert_eq!(promotion.presented_tier(), PresentedTier::Deterministic);
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            revision,
            PrecisionMode::PictureFast,
            true,
            true
        ),
        None,
        "one unchanged view promotes only once"
    );
}

/// Characterizes revision cancellation in the not-yet-wired promotion state machine.
#[test]
fn requested_change_cancels_promotion_and_rearms_the_fast_tier() {
    let mut promotion = SettledPromotion::default();
    let mut clock = FakeClock::default();
    assert_eq!(
        promotion.observe(clock.now_ms, 11, PrecisionMode::PictureFast, true, true),
        None
    );
    let _ = promotion.observe(clock.now_ms, 11, PrecisionMode::PictureFast, true, true);
    clock.advance(STATIC_SETTLE_WINDOW_MS);
    assert!(matches!(
        promotion.observe(clock.now_ms, 11, PrecisionMode::PictureFast, true, true),
        Some(PromotionAction::StartDeterministic { .. })
    ));
    assert!(promotion.deterministic_submitted(73));

    clock.advance(1.0);
    assert_eq!(
        promotion.observe(clock.now_ms, 12, PrecisionMode::PictureFast, false, true),
        Some(PromotionAction::CancelDeterministic)
    );
    assert_eq!(
        promotion.effective_precision_mode(PrecisionMode::PictureFast),
        PrecisionMode::PictureFast
    );
    assert_eq!(promotion.presented_tier(), PresentedTier::Fast);
    assert!(!promotion.deterministic_completed(73));

    let _ = promotion.observe(clock.now_ms, 12, PrecisionMode::PictureFast, true, true);
    clock.advance(STATIC_SETTLE_WINDOW_MS);
    assert_eq!(
        promotion.observe(clock.now_ms, 12, PrecisionMode::PictureFast, true, true),
        Some(PromotionAction::StartDeterministic {
            requested_revision: 12
        })
    );
}

/// Characterizes explicit mode and proves the disabled flag has no scheduling effect.
#[test]
fn explicit_deterministic_mode_and_the_disabled_hook_do_not_promote() {
    let mut promotion = SettledPromotion::default();
    let mut clock = FakeClock::default();
    clock.advance(STATIC_SETTLE_WINDOW_MS * 2.0);
    assert_eq!(
        promotion.observe(clock.now_ms, 1, PrecisionMode::Deterministic, true, true),
        None
    );
    assert_eq!(
        promotion.effective_precision_mode(PrecisionMode::Deterministic),
        PrecisionMode::Deterministic
    );
    assert_eq!(
        promotion.observe(
            clock.now_ms,
            2,
            PrecisionMode::PictureFast,
            true,
            SETTLED_DETERMINISTIC_PROMOTION_ENABLED,
        ),
        None
    );
    assert_eq!(promotion.presented_tier(), PresentedTier::Fast);
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TurnOutcome {
    scene_id: Option<u64>,
    warp_id: Option<u64>,
    presented: bool,
    refused: bool,
    completed_scene_id: Option<u64>,
    completed_warp_id: Option<u64>,
    refused_scene_id: Option<u64>,
    refused_warp_id: Option<u64>,
    surface_action: TraceSurfaceAction,
}

/// Drives the same typed stage protocol consumed by the production browser refresh.
fn drive_turn(
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: FakeClock,
    policy: FramePolicy,
    warps: bool,
) -> TurnOutcome {
    let refresh_order = BrowserRefreshOrder::begin();
    let mut outcome = TurnOutcome::default();
    FakePresenter::drain_frame_capture();
    let refresh_order = refresh_order.capture_drained();
    FakePresenter::stage_frame_capture();
    let refresh_order = refresh_order.capture_staged();
    for event in FrameLoop::refresh(presenter, clock.now_ms) {
        match event {
            FakeEvent::Completed(scene) => {
                frame_loop.completed(scene.id, scene.generation, scene.level);
                outcome.completed_scene_id = Some(scene.id);
            }
            FakeEvent::WarpCompleted(id) => {
                outcome.completed_warp_id = Some(id);
                presenter.presented_warps.push(id);
                outcome.presented = true;
                outcome.surface_action = match presenter.surface.complete(id) {
                    SurfaceAction::Present(warp_id) => TraceSurfaceAction::Present { warp_id },
                    SurfaceAction::Drop(warp_id) => TraceSurfaceAction::Drop { warp_id },
                    SurfaceAction::Ignore => TraceSurfaceAction::Ignore { warp_id: id },
                };
            }
            FakeEvent::Deadline(id) => {
                outcome.refused_scene_id = Some(id);
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
                match kind {
                    SubmissionKind::Scene => outcome.refused_scene_id = Some(id),
                    SubmissionKind::Warp => outcome.refused_warp_id = Some(id),
                }
                let refusal = frame_loop.refused(kind, reason, id, polls, wall_ms);
                outcome.refused = refusal.class != RefusalClass::Device;
                if matches!(kind, SubmissionKind::Warp) {
                    outcome.surface_action = match presenter.surface.refuse(id) {
                        SurfaceAction::Present(warp_id) => TraceSurfaceAction::Present { warp_id },
                        SurfaceAction::Drop(warp_id) => TraceSurfaceAction::Drop { warp_id },
                        SurfaceAction::Ignore => TraceSurfaceAction::Ignore { warp_id: id },
                    };
                }
            }
        }
    }
    let refresh_order = refresh_order.fences_observed();
    if frame_loop.stopped().is_some() {
        return outcome;
    }
    let has_retained_scene = presenter.retained_scene.is_some();
    presenter.write_hot_for_slot(has_retained_scene, 0);
    let refresh_order = refresh_order.hot_written();
    if !outcome.refused
        && let Some(level) = frame_loop.due()
    {
        let id = presenter.submit(frame_loop.generation(), level);
        frame_loop.submitted(id, level);
        outcome.scene_id = Some(id);
    }
    let refresh_order = refresh_order.scene_considered();
    if warps && presenter.pending_warp.is_none() && frame_loop.warp_requested(policy) {
        outcome.warp_id = Some(presenter.submit_warp(frame_loop.generation()));
        frame_loop.warp_submitted();
    }
    let _refresh_order = refresh_order.warp_considered();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "the oracle preserves each independent frame, capture, and finished-picture fact"
)]
struct StableFrameFacts {
    generation: u32,
    due: Option<RefinementLevel>,
    refinement: TracePendingState,
    scene_update: TracePendingState,
    scene_flight: TracePendingState,
    warp_flight: TracePendingState,
    retained_scene: Option<u64>,
    presented_scene: Option<u64>,
    capture: TraceCaptureState,
    capture_scene: Option<u64>,
    picture: PictureState,
    picture_finished: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[allow(
    dead_code,
    reason = "every field remains in Debug and equality so a failed trace prints the complete turn"
)]
struct FrameTraceTurn {
    scenario: &'static str,
    turn: u32,
    input_time_ms: f64,
    capture_before: TraceCaptureState,
    hot_writes: Vec<TraceHotWrite>,
    hot_epoch: u64,
    main_epoch: u64,
    submitted_scene_id: Option<u64>,
    submitted_scene_level: Option<RefinementLevel>,
    submitted_warp_id: Option<u64>,
    submitted_warp_kind: Option<WarpKind>,
    submitted_warp_source_scene_id: Option<u64>,
    fence_observations: Vec<TraceFenceObservation>,
    completed_scene_id: Option<u64>,
    completed_warp_id: Option<u64>,
    refused_scene_id: Option<u64>,
    refused_warp_id: Option<u64>,
    surface_action: TraceSurfaceAction,
    facts: StableFrameFacts,
}

fn traced_turn(
    scenario: &'static str,
    turn: u32,
    frame_loop: &mut FrameLoop,
    presenter: &mut FakePresenter,
    clock: FakeClock,
    warps: bool,
) -> FrameTraceTurn {
    let capture_before = presenter.capture;
    let hot_write_start = presenter.hot_write_log.len();
    let fence_start = presenter.fence_log.len();
    let outcome = drive_viewer_harness(frame_loop, presenter, clock, warps);
    let picture = PictureState {
        refinement_pending: frame_loop.refinement_pending(),
        scene_update_pending: frame_loop.scene_update_pending(),
        scene_in_flight: presenter.pending.is_some(),
        presented_view_stale: presenter.retained_scene.is_some()
            && presenter.presented_scene != presenter.retained_scene,
        presented_scene_is_completed: presenter.retained_scene.is_some()
            && presenter.presented_scene == presenter.retained_scene,
        warp_holds_stale: presenter.warp_kind == Some(WarpKind::HoldStale),
    };
    FrameTraceTurn {
        scenario,
        turn,
        input_time_ms: clock.now_ms,
        capture_before,
        hot_writes: presenter.hot_write_log[hot_write_start..].to_vec(),
        hot_epoch: presenter.hot_epoch,
        main_epoch: presenter.main_epoch,
        submitted_scene_id: outcome.scene_id,
        submitted_scene_level: outcome
            .scene_id
            .and_then(|_| presenter.pending.map(|scene| scene.level)),
        submitted_warp_id: outcome.warp_id,
        submitted_warp_kind: outcome.warp_id.and(presenter.pending_warp_kind),
        submitted_warp_source_scene_id: outcome.warp_id.and(presenter.pending_warp_source),
        fence_observations: presenter.fence_log[fence_start..].to_vec(),
        completed_scene_id: outcome.completed_scene_id,
        completed_warp_id: outcome.completed_warp_id,
        refused_scene_id: outcome.refused_scene_id,
        refused_warp_id: outcome.refused_warp_id,
        surface_action: outcome.surface_action,
        facts: StableFrameFacts {
            generation: frame_loop.generation(),
            due: frame_loop.due(),
            refinement: TracePendingState::from_pending(frame_loop.refinement_pending()),
            scene_update: TracePendingState::from_pending(frame_loop.scene_update_pending()),
            scene_flight: TracePendingState::from_pending(presenter.pending.is_some()),
            warp_flight: TracePendingState::from_pending(presenter.pending_warp.is_some()),
            retained_scene: presenter.retained_scene,
            presented_scene: presenter.presented_scene,
            capture: presenter.capture,
            capture_scene: presenter.capture_scene,
            picture,
            picture_finished: picture.finished(),
        },
    }
}

fn short_frame_trace() -> Vec<FrameTraceTurn> {
    let scenario = "short";
    let mut frame_loop = FrameLoop::default();
    frame_loop.restart(7);
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    let mut trace = vec![traced_turn(
        scenario,
        0,
        &mut frame_loop,
        &mut presenter,
        clock,
        false,
    )];
    presenter.fire_completed_callback();
    clock.advance(1.0);
    trace.push(traced_turn(
        scenario,
        1,
        &mut frame_loop,
        &mut presenter,
        clock,
        false,
    ));
    trace
}

fn retained_warp_trace(scenario: &'static str, warp_kind: WarpKind) -> Vec<FrameTraceTurn> {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
    frame_loop.accept_request(37, true);
    frame_loop.scene_selection_changed(37);
    let mut presenter = retained_presenter(false);
    presenter.forced_warp_kind = Some(warp_kind);
    let mut clock = FakeClock::default();
    let mut trace = vec![traced_turn(
        scenario,
        0,
        &mut frame_loop,
        &mut presenter,
        clock,
        true,
    )];
    presenter.fire_warp_completed();
    clock.advance(1.0);
    trace.push(traced_turn(
        scenario,
        1,
        &mut frame_loop,
        &mut presenter,
        clock,
        false,
    ));
    trace
}

fn zoom_frame_trace() -> Vec<FrameTraceTurn> {
    retained_warp_trace("zoom", WarpKind::AnchorHomography)
}

fn height_frame_trace() -> Vec<FrameTraceTurn> {
    retained_warp_trace("height", WarpKind::ReliefRedraw)
}

fn completed_picture_trace(scenario: &'static str, arm_capture: bool) -> Vec<FrameTraceTurn> {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(7, true);
    assert!(frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false))));
    let mut presenter = FakePresenter::default();
    let mut clock = FakeClock::default();
    let mut trace = vec![traced_turn(
        scenario,
        0,
        &mut frame_loop,
        &mut presenter,
        clock,
        true,
    )];
    presenter.fire_completed_callback();
    presenter.fire_warp_completed();
    if arm_capture {
        presenter.arm_capture();
    }
    clock.advance(1.0);
    trace.push(traced_turn(
        scenario,
        1,
        &mut frame_loop,
        &mut presenter,
        clock,
        true,
    ));
    presenter.fire_warp_completed();
    clock.advance(1.0);
    trace.push(traced_turn(
        scenario,
        2,
        &mut frame_loop,
        &mut presenter,
        clock,
        false,
    ));
    trace
}

fn capture_frame_trace() -> Vec<FrameTraceTurn> {
    completed_picture_trace("capture", true)
}

fn finished_picture_trace() -> Vec<FrameTraceTurn> {
    completed_picture_trace("finished-picture", false)
}

const APP_FRAME_TRACE_FIXTURE: &str = "[\n    FrameTraceTurn {\n        scenario: \"short\",\n        turn: 0,\n        input_time_ms: 0.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 1,\n                drained_main_epoch: 1,\n            },\n        ],\n        hot_epoch: 1,\n        main_epoch: 1,\n        submitted_scene_id: Some(\n            1,\n        ),\n        submitted_scene_level: Some(\n            Preview,\n        ),\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [],\n        completed_scene_id: None,\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Pending,\n            scene_update: Idle,\n            scene_flight: Pending,\n            warp_flight: Idle,\n            retained_scene: None,\n            presented_scene: None,\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: true,\n                scene_update_pending: false,\n                scene_in_flight: true,\n                presented_view_stale: false,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"short\",\n        turn: 1,\n        input_time_ms: 1.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 2,\n                drained_main_epoch: 2,\n            },\n        ],\n        hot_epoch: 2,\n        main_epoch: 2,\n        submitted_scene_id: Some(\n            2,\n        ),\n        submitted_scene_level: Some(\n            Interactive,\n        ),\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [\n            TraceFenceObservation {\n                order: 0,\n                kind: Scene,\n                id: 1,\n                result: SceneCompleted {\n                    generation: 7,\n                    level: Preview,\n                },\n            },\n        ],\n        completed_scene_id: Some(\n            1,\n        ),\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Pending,\n            scene_update: Idle,\n            scene_flight: Pending,\n            warp_flight: Idle,\n            retained_scene: Some(\n                1,\n            ),\n            presented_scene: None,\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: true,\n                scene_update_pending: false,\n                scene_in_flight: true,\n                presented_view_stale: true,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"zoom\",\n        turn: 0,\n        input_time_ms: 0.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 1,\n                drained_main_epoch: 1,\n            },\n        ],\n        hot_epoch: 1,\n        main_epoch: 1,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: Some(\n            38,\n        ),\n        submitted_warp_kind: Some(\n            AnchorHomography,\n        ),\n        submitted_warp_source_scene_id: Some(\n            37,\n        ),\n        fence_observations: [],\n        completed_scene_id: None,\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 0,\n            due: None,\n            refinement: Idle,\n            scene_update: Pending,\n            scene_flight: Idle,\n            warp_flight: Pending,\n            retained_scene: Some(\n                37,\n            ),\n            presented_scene: Some(\n                37,\n            ),\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: true,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"zoom\",\n        turn: 1,\n        input_time_ms: 1.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 2,\n                drained_main_epoch: 2,\n            },\n        ],\n        hot_epoch: 2,\n        main_epoch: 2,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [\n            TraceFenceObservation {\n                order: 0,\n                kind: Warp,\n                id: 38,\n                result: WarpCompleted {\n                    kind: Some(\n                        AnchorHomography,\n                    ),\n                    source_scene_id: Some(\n                        37,\n                    ),\n                },\n            },\n        ],\n        completed_scene_id: None,\n        completed_warp_id: Some(\n            38,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 38,\n        },\n        facts: StableFrameFacts {\n            generation: 0,\n            due: None,\n            refinement: Idle,\n            scene_update: Pending,\n            scene_flight: Idle,\n            warp_flight: Idle,\n            retained_scene: Some(\n                37,\n            ),\n            presented_scene: Some(\n                37,\n            ),\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: true,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"height\",\n        turn: 0,\n        input_time_ms: 0.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 1,\n                drained_main_epoch: 1,\n            },\n        ],\n        hot_epoch: 1,\n        main_epoch: 1,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: Some(\n            38,\n        ),\n        submitted_warp_kind: Some(\n            ReliefRedraw,\n        ),\n        submitted_warp_source_scene_id: Some(\n            37,\n        ),\n        fence_observations: [],\n        completed_scene_id: None,\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 0,\n            due: None,\n            refinement: Idle,\n            scene_update: Pending,\n            scene_flight: Idle,\n            warp_flight: Pending,\n            retained_scene: Some(\n                37,\n            ),\n            presented_scene: Some(\n                37,\n            ),\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: true,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"height\",\n        turn: 1,\n        input_time_ms: 1.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 2,\n                drained_main_epoch: 2,\n            },\n        ],\n        hot_epoch: 2,\n        main_epoch: 2,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [\n            TraceFenceObservation {\n                order: 0,\n                kind: Warp,\n                id: 38,\n                result: WarpCompleted {\n                    kind: Some(\n                        ReliefRedraw,\n                    ),\n                    source_scene_id: Some(\n                        37,\n                    ),\n                },\n            },\n        ],\n        completed_scene_id: None,\n        completed_warp_id: Some(\n            38,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 38,\n        },\n        facts: StableFrameFacts {\n            generation: 0,\n            due: None,\n            refinement: Idle,\n            scene_update: Pending,\n            scene_flight: Idle,\n            warp_flight: Idle,\n            retained_scene: Some(\n                37,\n            ),\n            presented_scene: Some(\n                37,\n            ),\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: true,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"capture\",\n        turn: 0,\n        input_time_ms: 0.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 1,\n                drained_main_epoch: 1,\n            },\n        ],\n        hot_epoch: 1,\n        main_epoch: 1,\n        submitted_scene_id: Some(\n            1,\n        ),\n        submitted_scene_level: Some(\n            Final,\n        ),\n        submitted_warp_id: Some(\n            2,\n        ),\n        submitted_warp_kind: Some(\n            ClearOnly,\n        ),\n        submitted_warp_source_scene_id: None,\n        fence_observations: [],\n        completed_scene_id: None,\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Pending,\n            scene_update: Idle,\n            scene_flight: Pending,\n            warp_flight: Pending,\n            retained_scene: None,\n            presented_scene: None,\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: true,\n                scene_update_pending: false,\n                scene_in_flight: true,\n                presented_view_stale: false,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"capture\",\n        turn: 1,\n        input_time_ms: 1.0,\n        capture_before: Armed,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 2,\n                drained_main_epoch: 2,\n            },\n        ],\n        hot_epoch: 2,\n        main_epoch: 2,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: Some(\n            3,\n        ),\n        submitted_warp_kind: Some(\n            AnchorHomography,\n        ),\n        submitted_warp_source_scene_id: Some(\n            1,\n        ),\n        fence_observations: [\n            TraceFenceObservation {\n                order: 0,\n                kind: Scene,\n                id: 1,\n                result: SceneCompleted {\n                    generation: 7,\n                    level: Final,\n                },\n            },\n            TraceFenceObservation {\n                order: 1,\n                kind: Warp,\n                id: 2,\n                result: WarpCompleted {\n                    kind: Some(\n                        ClearOnly,\n                    ),\n                    source_scene_id: None,\n                },\n            },\n        ],\n        completed_scene_id: Some(\n            1,\n        ),\n        completed_warp_id: Some(\n            2,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 2,\n        },\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Idle,\n            scene_update: Idle,\n            scene_flight: Idle,\n            warp_flight: Pending,\n            retained_scene: Some(\n                1,\n            ),\n            presented_scene: None,\n            capture: InFlight,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: false,\n                scene_in_flight: false,\n                presented_view_stale: true,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"capture\",\n        turn: 2,\n        input_time_ms: 2.0,\n        capture_before: InFlight,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 3,\n                drained_main_epoch: 3,\n            },\n        ],\n        hot_epoch: 3,\n        main_epoch: 3,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [\n            TraceFenceObservation {\n                order: 2,\n                kind: Warp,\n                id: 3,\n                result: WarpCompleted {\n                    kind: Some(\n                        AnchorHomography,\n                    ),\n                    source_scene_id: Some(\n                        1,\n                    ),\n                },\n            },\n        ],\n        completed_scene_id: None,\n        completed_warp_id: Some(\n            3,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 3,\n        },\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Idle,\n            scene_update: Idle,\n            scene_flight: Idle,\n            warp_flight: Idle,\n            retained_scene: Some(\n                1,\n            ),\n            presented_scene: Some(\n                1,\n            ),\n            capture: Ready,\n            capture_scene: Some(\n                1,\n            ),\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: false,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: true,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"finished-picture\",\n        turn: 0,\n        input_time_ms: 0.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 1,\n                drained_main_epoch: 1,\n            },\n        ],\n        hot_epoch: 1,\n        main_epoch: 1,\n        submitted_scene_id: Some(\n            1,\n        ),\n        submitted_scene_level: Some(\n            Final,\n        ),\n        submitted_warp_id: Some(\n            2,\n        ),\n        submitted_warp_kind: Some(\n            ClearOnly,\n        ),\n        submitted_warp_source_scene_id: None,\n        fence_observations: [],\n        completed_scene_id: None,\n        completed_warp_id: None,\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: None,\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Pending,\n            scene_update: Idle,\n            scene_flight: Pending,\n            warp_flight: Pending,\n            retained_scene: None,\n            presented_scene: None,\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: true,\n                scene_update_pending: false,\n                scene_in_flight: true,\n                presented_view_stale: false,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"finished-picture\",\n        turn: 1,\n        input_time_ms: 1.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 2,\n                drained_main_epoch: 2,\n            },\n        ],\n        hot_epoch: 2,\n        main_epoch: 2,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: Some(\n            3,\n        ),\n        submitted_warp_kind: Some(\n            AnchorHomography,\n        ),\n        submitted_warp_source_scene_id: Some(\n            1,\n        ),\n        fence_observations: [\n            TraceFenceObservation {\n                order: 0,\n                kind: Scene,\n                id: 1,\n                result: SceneCompleted {\n                    generation: 7,\n                    level: Final,\n                },\n            },\n            TraceFenceObservation {\n                order: 1,\n                kind: Warp,\n                id: 2,\n                result: WarpCompleted {\n                    kind: Some(\n                        ClearOnly,\n                    ),\n                    source_scene_id: None,\n                },\n            },\n        ],\n        completed_scene_id: Some(\n            1,\n        ),\n        completed_warp_id: Some(\n            2,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 2,\n        },\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Idle,\n            scene_update: Idle,\n            scene_flight: Idle,\n            warp_flight: Pending,\n            retained_scene: Some(\n                1,\n            ),\n            presented_scene: None,\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: false,\n                scene_in_flight: false,\n                presented_view_stale: true,\n                presented_scene_is_completed: false,\n                warp_holds_stale: false,\n            },\n            picture_finished: false,\n        },\n    },\n    FrameTraceTurn {\n        scenario: \"finished-picture\",\n        turn: 2,\n        input_time_ms: 2.0,\n        capture_before: Idle,\n        hot_writes: [\n            TraceHotWrite {\n                slot_order: 0,\n                drained_hot_epoch: 3,\n                drained_main_epoch: 3,\n            },\n        ],\n        hot_epoch: 3,\n        main_epoch: 3,\n        submitted_scene_id: None,\n        submitted_scene_level: None,\n        submitted_warp_id: None,\n        submitted_warp_kind: None,\n        submitted_warp_source_scene_id: None,\n        fence_observations: [\n            TraceFenceObservation {\n                order: 2,\n                kind: Warp,\n                id: 3,\n                result: WarpCompleted {\n                    kind: Some(\n                        AnchorHomography,\n                    ),\n                    source_scene_id: Some(\n                        1,\n                    ),\n                },\n            },\n        ],\n        completed_scene_id: None,\n        completed_warp_id: Some(\n            3,\n        ),\n        refused_scene_id: None,\n        refused_warp_id: None,\n        surface_action: Present {\n            warp_id: 3,\n        },\n        facts: StableFrameFacts {\n            generation: 7,\n            due: None,\n            refinement: Idle,\n            scene_update: Idle,\n            scene_flight: Idle,\n            warp_flight: Idle,\n            retained_scene: Some(\n                1,\n            ),\n            presented_scene: Some(\n                1,\n            ),\n            capture: Idle,\n            capture_scene: None,\n            picture: PictureState {\n                refinement_pending: false,\n                scene_update_pending: false,\n                scene_in_flight: false,\n                presented_view_stale: false,\n                presented_scene_is_completed: true,\n                warp_holds_stale: false,\n            },\n            picture_finished: true,\n        },\n    },\n]";

fn named_frame_traces() -> Vec<FrameTraceTurn> {
    [
        short_frame_trace(),
        zoom_frame_trace(),
        height_frame_trace(),
        capture_frame_trace(),
        finished_picture_trace(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[test]
fn named_frame_scenarios_match_frozen_complete_turn_records() {
    let scenarios = [
        ("short", short_frame_trace as fn() -> Vec<FrameTraceTurn>),
        ("zoom", zoom_frame_trace),
        ("height", height_frame_trace),
        ("capture", capture_frame_trace),
        ("finished-picture", finished_picture_trace),
    ];
    for (scenario, run) in scenarios {
        let baseline = run();
        assert!(!baseline.is_empty(), "{scenario} trace");
        assert!(
            baseline.iter().all(|turn| turn.scenario == scenario),
            "{scenario} trace names every turn"
        );
        assert!(
            baseline
                .windows(2)
                .all(|turns| turns[0].hot_epoch < turns[1].hot_epoch
                    && turns[0].main_epoch <= turns[1].main_epoch),
            "{scenario} trace epochs are monotonic"
        );
    }

    let zoom = zoom_frame_trace();
    assert_eq!(
        zoom[0].submitted_warp_kind,
        Some(WarpKind::AnchorHomography)
    );
    let height = height_frame_trace();
    assert_eq!(height[0].submitted_warp_kind, Some(WarpKind::ReliefRedraw));
    let capture = capture_frame_trace();
    assert_eq!(capture[1].facts.capture, TraceCaptureState::InFlight);
    let captured = capture.last().expect("capture completion turn");
    assert_eq!(captured.facts.capture, TraceCaptureState::Ready);
    assert_eq!(captured.facts.capture_scene, captured.facts.retained_scene);
    assert!(matches!(
        captured.surface_action,
        TraceSurfaceAction::Present { .. }
    ));
    let finished = finished_picture_trace();
    assert!(!finished[1].facts.picture_finished);
    assert!(
        finished
            .last()
            .expect("finished-picture presentation turn")
            .facts
            .picture_finished
    );

    assert_eq!(
        format!("{:#?}", named_frame_traces()),
        APP_FRAME_TRACE_FIXTURE
    );
}

#[test]
#[ignore = "prints the immutable frame trace for review before it is committed"]
#[allow(
    clippy::print_stdout,
    reason = "the ignored fixture generator must return the reviewed bytes to the orchestrator"
)]
fn print_app_frame_trace_fixture() {
    let fixture = format!("{:#?}", named_frame_traces());
    println!("const APP_FRAME_TRACE_FIXTURE: &str = {fixture:?};");
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
fn viewer_harness_holds_an_auto_refusal_while_the_final_fill_is_pending() {
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
fn viewer_harness_keeps_the_picture_when_a_non_final_round_is_retired_and_resumed() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.accept_request(37, true);
    let mut presenter = retained_presenter(true);
    let mut clock = FakeClock::default();

    let first = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
    let retired_scene = first.scene_id.expect("the first round submits Preview");
    assert_eq!(presenter.warp_kind, Some(WarpKind::HoldStale));
    presenter.fire_warp_completed();
    clock.advance(1.0);
    assert!(drive_viewer_harness(&mut frame_loop, &mut presenter, clock, false).presented);
    assert_eq!(presenter.presented_scene, Some(37));

    assert!(frame_loop.retired(retired_scene));
    presenter.pending = None;
    frame_loop.scene_input_resumed(38, RefinementLevel::Interactive);
    frame_loop.accept_request(38, false);
    let resumed = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
    assert!(resumed.scene_id.is_some());
    assert!(resumed.warp_id.is_some());
    assert_eq!(presenter.warp_kind, Some(WarpKind::HoldStale));
    assert_eq!(presenter.pending_warp_source, Some(37));
    assert_eq!(presenter.warp_hold_count, 2);
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
    const DRAG_FRAMES: usize = 90;
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

fn measured_relief_zoom_viewer() -> ViewerController {
    let object = ObjectAngles {
        rho_13: -0.163_226_878_883_618_53,
        rho_24: -0.163_226_878_883_618_53,
        ..ObjectAngles::IDENTITY
    };
    let view = ViewControls {
        camera: [
            0.387_171_329_215_743,
            0.945_997_639_356_507,
            -1.185_339_915_598_97,
            -0.756_473_212_467_682,
            -0.236_634_784_429_762,
            -1.770_158_147_141_63,
            2.011_666_416_834_24,
            0.504_134_975_524_275,
            -0.665_501_487_561_046,
            0.374_175_368_514_795,
        ],
        camera_translation: [-0.04, 0.258, 0.0, 0.0, 0.0],
        height_scale: 3.565,
        distance_five: 8.0,
        distance_four: 8.0,
        ..ViewControls::NEUTRAL
    };
    let origin = [-0.629, 0.0, -0.083, 0.016];
    let mut viewer = ViewerController::new([960, 540]).expect("measured relief viewer");
    viewer
        .set_object_angles(object)
        .expect("measured relief object");
    viewer
        .set_plane_origin(origin)
        .expect("measured relief origin");
    viewer
        .set_view_controls(view)
        .expect("measured relief view");
    viewer
        .set_zoom_log2(1.259_194_831_013_92)
        .expect("measured relief scale");
    viewer
        .set_centre(BigCentre::from_f64(origin, 1_024).expect("finite measured centre"))
        .expect("measured relief centre");
    viewer
}

const MEASURED_FINAL_EXTENT: [u32; 2] = [960, 540];
const MEASURED_PREVIEW_EXTENT: [u32; 2] = [120, 68];

fn measured_relief_scene(scene_id: u64, level: RefinementLevel, pose: &Pose) -> SceneFrame {
    SceneFrame {
        scene_id,
        pose: *pose,
        iteration_cap: 128,
        level,
        extent: [pose.grid_width, pose.grid_height],
        texture_index: 0,
        centre_revision: 1,
        plane_origin_f64: pose.plane_origin,
        precision_mode: PrecisionMode::PictureFast.as_str(),
        measurement: SubmissionMeasurement {
            kind: SubmissionKind::Scene,
            id: scene_id,
            source_scene_id: None,
            sample_class: SampleClass::Measured,
            precision_mode: PrecisionMode::PictureFast.as_str(),
            wall_ms: 1.0,
            fence_wait_ms: 0.5,
            polls: 1,
        },
    }
}

#[derive(Clone, Copy, Debug)]
struct PendingZoomWarp {
    completes_at_turn: u32,
    kind: WarpKind,
    predicted_exposed_fraction: Option<f64>,
    requested_revision: u32,
}

#[derive(Clone, Copy, Debug)]
struct PendingZoomScene {
    completes_at_turn: u32,
    pose: Pose,
    requested_revision: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomInitialScene {
    SettledFinal,
    FinalInFlight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomRefinementOnEdit {
    Restart,
    StayIdle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomEditTrigger {
    RequestedFrame,
    StaleViewRecovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomLatticeProbe {
    Skip,
    Enforce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomEditState {
    None,
    Applied,
    CrosshairRefused,
    ZoomRefused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomRecordState {
    Ready,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomRefinementState {
    Pending,
    Idle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomHoldState {
    Disarmed,
    Armed,
    Selected,
    RefusedByLattice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ZoomSourceCoverage {
    NotCovering,
    CoversDestination,
}

#[derive(Clone, Copy, Debug)]
struct ZoomScript<'a> {
    scenario: &'static str,
    retained_level: RefinementLevel,
    edits: &'a [(u32, f64, Option<[f64; 2]>)],
    initial_scene: ZoomInitialScene,
    refinement_on_edit: ZoomRefinementOnEdit,
    edit_trigger: ZoomEditTrigger,
    forget_records_before_selection_at_turn: Option<u32>,
    destination_extent: [u32; 2],
    lattice_probe: ZoomLatticeProbe,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "all fields are retained so a failing assertion prints the complete per-turn plan record"
)]
struct ZoomTurnRecord {
    scenario: &'static str,
    turn: u32,
    planned: WarpKind,
    selected: WarpKind,
    presented: Option<WarpKind>,
    visible_kind: Option<WarpKind>,
    presented_exposed_fraction: Option<f64>,
    refusal_reason: Option<WarpRefusalReason>,
    edit_state: ZoomEditState,
    attempted_crosshair: Option<[f64; 2]>,
    attempted_zoom_delta: Option<f64>,
    records: ZoomRecordState,
    retained_level: RefinementLevel,
    refinement: ZoomRefinementState,
    hold: ZoomHoldState,
    source_coverage: ZoomSourceCoverage,
    requested_revision: u32,
    warp_in_flight_kind: Option<WarpKind>,
    scene_in_flight: bool,
    completed_requested_final: bool,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "a failing ruling assertion prints the named violation and its complete turn record"
)]
struct ZoomRuleViolation {
    reason: &'static str,
    record: ZoomTurnRecord,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    dead_code,
    reason = "a failing ruling assertion prints each scenario's grey outcome and first grey turn"
)]
struct ZoomScenarioOutcome {
    scenario: &'static str,
    captured_grey: bool,
    first_grey_turn: Option<ZoomTurnRecord>,
}

fn measured_relief_plan(
    retained: &SceneFrame,
    requested: &Pose,
) -> ember_julibrot_present::WarpPlan {
    Warp::reproject(
        retained,
        &retained.pose,
        requested,
        PrecisionMode::PictureFast,
        WarpValidation::Ordinary,
    )
}

fn measured_relief_source_covers_destination(
    plan: &ember_julibrot_present::WarpPlan,
    source: &SceneFrame,
    requested: &Pose,
) -> bool {
    if plan.kind == WarpKind::ReliefRedraw {
        return plan.source_valid && relief_redraw_source_covers_destination(source, requested);
    }
    if !matches!(
        plan.refusal_reason,
        Some(
            WarpRefusalReason::ErrorCeiling { .. }
                | WarpRefusalReason::ErrorCorpus { .. }
                | WarpRefusalReason::ReliefExposure { .. }
        )
    ) {
        return false;
    }
    relief_redraw_source_covers_destination(source, requested)
}

fn measured_relief_exposure_zoom_edit() -> Option<(u32, f64, Option<[f64; 2]>)> {
    let crosshairs = [
        [40.0, 30.0],
        [40.0, -30.0],
        [-40.0, 30.0],
        [-40.0, -30.0],
        [180.0, 90.0],
        [180.0, -90.0],
        [-180.0, 90.0],
        [-180.0, -90.0],
    ];
    for delta_log2 in [1.43, 3.0, 0.5, 0.1, 6.0] {
        for crosshair in crosshairs {
            let mut viewer = measured_relief_zoom_viewer();
            let retained_pose = viewer
                .drain_hot(MEASURED_FINAL_EXTENT)
                .expect("retained measured pose")
                .pose;
            let retained = measured_relief_scene(37, RefinementLevel::Final, &retained_pose);
            if viewer.set_crosshair(crosshair).is_err()
                || viewer.zoom_about_crosshair(delta_log2).is_err()
            {
                continue;
            }
            let Ok(requested) = viewer.drain_hot(MEASURED_FINAL_EXTENT) else {
                continue;
            };
            if matches!(
                measured_relief_plan(&retained, &requested.pose).refusal_reason,
                Some(WarpRefusalReason::ReliefExposure { .. })
            ) {
                return Some((0, delta_log2, Some(crosshair)));
            }
        }
    }
    None
}

#[allow(
    clippy::too_many_lines,
    reason = "the visible-turn reproduction keeps its edit, render, and presentation order in one trace"
)]
fn drive_measured_zoom_trace(script: ZoomScript<'_>) -> Vec<ZoomTurnRecord> {
    const WARP_FLIGHT_TURNS: u32 = 12;
    const SCENE_FLIGHT_TURNS: u32 = 30;
    const TOTAL_TURNS: u32 = 120;

    let mut viewer = measured_relief_zoom_viewer();
    let source_extent = if script.retained_level == RefinementLevel::Preview {
        MEASURED_PREVIEW_EXTENT
    } else {
        MEASURED_FINAL_EXTENT
    };
    let retained_pose = viewer
        .drain_hot(source_extent)
        .expect("retained measured pose")
        .pose;
    let settled_final_pose = viewer
        .drain_hot(MEASURED_FINAL_EXTENT)
        .expect("settled Final pose")
        .pose;
    let mut retained = measured_relief_scene(37, script.retained_level, &retained_pose);
    let mut frame_loop = FrameLoop::default();
    let mut requested_revision = 0_u32;
    let mut presented_revision = 0_u32;
    let mut last_presented_kind = Some(WarpKind::AnchorHomography);
    let mut pending_warp: Option<PendingZoomWarp> = None;
    let final_already_in_flight = script.initial_scene == ZoomInitialScene::FinalInFlight;
    let mut pending_scene = final_already_in_flight.then_some(PendingZoomScene {
        completes_at_turn: 18,
        pose: settled_final_pose,
        requested_revision: 0,
    });
    let mut records_ready = !final_already_in_flight;
    let mut next_scene_id = 38_u64;
    let mut trace = Vec::new();

    for turn in 0..=TOTAL_TURNS {
        let mut presented = None;
        let mut presented_exposed_fraction = None;
        let mut edit_state = ZoomEditState::None;
        let mut attempted_crosshair = None;
        let mut attempted_zoom_delta = None;
        if pending_warp.is_some_and(|warp| warp.completes_at_turn == turn) {
            let warp = pending_warp.take().expect("due warp is pending");
            presented = Some(warp.kind);
            presented_exposed_fraction = warp.predicted_exposed_fraction;
            last_presented_kind = Some(warp.kind);
            if warp.kind != WarpKind::HoldStale {
                presented_revision = warp.requested_revision;
            }
        }
        if pending_scene.is_some_and(|scene| scene.completes_at_turn == turn) {
            let scene = pending_scene.take().expect("due scene is pending");
            retained = measured_relief_scene(next_scene_id, RefinementLevel::Final, &scene.pose);
            next_scene_id = next_scene_id.saturating_add(1);
            records_ready = true;
            if scene.requested_revision == requested_revision {
                frame_loop.schedule.pause();
                frame_loop.accept_request(37, false);
            }
        }

        for &(_, delta_log2, crosshair) in script.edits.iter().filter(|edit| edit.0 == turn) {
            attempted_crosshair = crosshair;
            attempted_zoom_delta = Some(delta_log2);
            if let Some(crosshair) = crosshair {
                let Ok(()) = viewer.set_crosshair(crosshair) else {
                    edit_state = ZoomEditState::CrosshairRefused;
                    continue;
                };
            }
            if viewer.zoom_about_crosshair(delta_log2).is_err() {
                edit_state = ZoomEditState::ZoomRefused;
                continue;
            }
            edit_state = ZoomEditState::Applied;
            requested_revision = requested_revision.saturating_add(1);
            let restart_scene = script.refinement_on_edit == ZoomRefinementOnEdit::Restart;
            if script.edit_trigger == ZoomEditTrigger::RequestedFrame {
                frame_loop.accept_request(37, restart_scene);
            } else if restart_scene {
                frame_loop.scene_changed(37);
            }
        }

        let destination_extent = script.destination_extent;
        let requested = viewer
            .drain_hot(destination_extent)
            .expect("scripted requested pose")
            .pose;
        let completed_requested_final = requested_revision != 0
            && retained.level == RefinementLevel::Final
            && renders_same_picture(&retained.pose, &requested);
        let view_is_stale = presented_revision != requested_revision;
        if script.edit_trigger == ZoomEditTrigger::StaleViewRecovery
            && super::stale_view_needs_a_new_scene(
                SceneMode::Auto,
                frame_loop.refinement_pending(),
                view_is_stale,
                completed_requested_final,
            )
        {
            frame_loop.request_missing_final(37);
        }
        let plan = measured_relief_plan(&retained, &requested);
        let source_covers_destination = requested_revision != 0
            && measured_relief_source_covers_destination(&plan, &retained, &requested);
        if script.forget_records_before_selection_at_turn == Some(turn) {
            records_ready = false;
        }
        let retain_visible_redraw = plan.kind == WarpKind::ReliefRedraw
            && !records_ready
            && pending_scene.is_some()
            && last_presented_kind == Some(WarpKind::ReliefRedraw);
        let mut selected =
            if plan.kind == WarpKind::ReliefRedraw && !records_ready && !retain_visible_redraw {
                WarpKind::ClearOnly
            } else {
                plan.kind
            };
        if selected == WarpKind::ClearOnly && source_covers_destination {
            selected = WarpKind::HoldStale;
        }
        let hold_armed = source_covers_destination;
        let hold_selected_before_lattice = selected == WarpKind::HoldStale;
        let lattice_refused = script.lattice_probe == ZoomLatticeProbe::Enforce
            && hold_selected_before_lattice
            && LatticePair::new(retained.extent, destination_extent).is_none_or(|lattice| {
                lattice.destination() != destination_extent
                    || lattice
                        .covering_rows()
                        .is_none_or(|rows| !lattice.covers_destination(rows))
            });
        if lattice_refused {
            selected = WarpKind::ClearOnly;
        }
        let hold = if lattice_refused {
            ZoomHoldState::RefusedByLattice
        } else if hold_selected_before_lattice {
            ZoomHoldState::Selected
        } else if hold_armed {
            ZoomHoldState::Armed
        } else {
            ZoomHoldState::Disarmed
        };

        let defer_scene_for_redraw =
            defer_scene_until_relief_redraw(selected == WarpKind::ReliefRedraw, view_is_stale);
        if pending_scene.is_none()
            && requested_revision != 0
            && frame_loop.refinement_pending()
            && !defer_scene_for_redraw
            && !renders_same_picture(&retained.pose, &requested)
        {
            pending_scene = Some(PendingZoomScene {
                completes_at_turn: turn.saturating_add(SCENE_FLIGHT_TURNS),
                pose: requested,
                requested_revision,
            });
        }
        let hold_redraw = selected == WarpKind::ReliefRedraw && pending_scene.is_some();
        let warp_requested = warp_submission_due(
            frame_loop.warp_requested(FramePolicy::SingleFrameOnDemand),
            defer_scene_for_redraw,
        );
        if warp_requested && pending_warp.is_none() && !hold_redraw {
            pending_warp = Some(PendingZoomWarp {
                completes_at_turn: turn.saturating_add(WARP_FLIGHT_TURNS),
                kind: selected,
                predicted_exposed_fraction: if selected == WarpKind::ReliefRedraw {
                    plan.predicted_exposed_fraction
                } else {
                    None
                },
                requested_revision,
            });
            frame_loop.warp_submitted();
        }
        trace.push(ZoomTurnRecord {
            scenario: script.scenario,
            turn,
            planned: plan.kind,
            selected,
            presented,
            visible_kind: last_presented_kind,
            presented_exposed_fraction,
            refusal_reason: plan.refusal_reason,
            edit_state,
            attempted_crosshair,
            attempted_zoom_delta,
            records: if records_ready {
                ZoomRecordState::Ready
            } else {
                ZoomRecordState::Unavailable
            },
            retained_level: retained.level,
            refinement: if frame_loop.refinement_pending() {
                ZoomRefinementState::Pending
            } else {
                ZoomRefinementState::Idle
            },
            hold,
            source_coverage: if source_covers_destination {
                ZoomSourceCoverage::CoversDestination
            } else {
                ZoomSourceCoverage::NotCovering
            },
            requested_revision,
            warp_in_flight_kind: pending_warp.map(|warp| warp.kind),
            scene_in_flight: pending_scene.is_some(),
            completed_requested_final,
        });
    }
    trace
}

/// Collects the planner's nonzero zoom-in exposure publications without making them grey failures.
///
/// The relief-warp planner round owns this follow-up. Keeping the values beside the seven native
/// turn traces preserves the evidence while this lane enforces only the retained-picture ruling.
fn measured_zoom_exposure_report(traces: &[Vec<ZoomTurnRecord>]) -> Vec<ZoomTurnRecord> {
    traces
        .iter()
        .flatten()
        .filter(|turn| {
            turn.source_coverage == ZoomSourceCoverage::CoversDestination
                && turn.presented.is_some()
                && turn.presented_exposed_fraction.unwrap_or(0.0) > f64::EPSILON
        })
        .copied()
        .collect()
}

#[test]
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    reason = "all scenarios run before one assertion, while fenced planner exposure stays visible as telemetry"
)]
fn two_second_visible_zoom_scripts_never_clear_a_covering_retained_scene() {
    let idle_refused_edit = measured_relief_exposure_zoom_edit();
    let found_idle_relief_exposure = idle_refused_edit.is_some();
    let idle_refused_edit = idle_refused_edit.unwrap_or((0, 3.0, Some([40.0, 30.0])));
    let traces = vec![
        drive_measured_zoom_trace(ZoomScript {
            scenario: "off-centre box +1.43",
            retained_level: RefinementLevel::Final,
            edits: &[(0, 1.43, Some([40.0, 30.0]))],
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "slider +0.5 at 100 ms",
            retained_level: RefinementLevel::Final,
            edits: &[(0, 0.1, None), (6, 0.5, None)],
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "retained Preview before Final",
            retained_level: RefinementLevel::Preview,
            edits: &[(0, 0.5, None)],
            initial_scene: ZoomInitialScene::FinalInFlight,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "idle Final far off-centre refusal",
            retained_level: RefinementLevel::Final,
            edits: &[idle_refused_edit],
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::StayIdle,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "record lease forgotten before redraw submission",
            retained_level: RefinementLevel::Final,
            edits: &[(0, 0.5, Some([40.0, 30.0]))],
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: Some(0),
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "idle Final record-lease gap",
            retained_level: RefinementLevel::Final,
            edits: &[(0, 0.5, Some([40.0, 30.0]))],
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::StayIdle,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: Some(0),
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        }),
        drive_measured_zoom_trace(ZoomScript {
            scenario: "Preview hold lattice enforcement",
            retained_level: RefinementLevel::Preview,
            edits: &[(0, 0.5, Some([40.0, 30.0]))],
            initial_scene: ZoomInitialScene::FinalInFlight,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::RequestedFrame,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Enforce,
        }),
    ];

    let outcomes: Vec<_> = traces
        .iter()
        .map(|trace| {
            let first_grey_turn = trace.iter().copied().find(|turn| {
                turn.presented == Some(WarpKind::ClearOnly)
                    || turn
                        .presented_exposed_fraction
                        .is_some_and(|fraction| fraction > 0.5)
            });
            ZoomScenarioOutcome {
                scenario: trace[0].scenario,
                captured_grey: first_grey_turn.is_some(),
                first_grey_turn,
            }
        })
        .collect();
    let exposure_report = measured_zoom_exposure_report(&traces);
    eprintln!("zoom-in exposure follow-up for the relief-warp planner: {exposure_report:#?}");
    let mut violations = Vec::new();
    for turn in traces.iter().flatten() {
        if turn.edit_state == ZoomEditState::CrosshairRefused {
            violations.push(ZoomRuleViolation {
                reason: "visible-frame crosshair was refused",
                record: *turn,
            });
        } else if turn.edit_state == ZoomEditState::ZoomRefused {
            violations.push(ZoomRuleViolation {
                reason: "positive scripted zoom was refused",
                record: *turn,
            });
        }
        if turn.source_coverage != ZoomSourceCoverage::CoversDestination || turn.presented.is_none()
        {
            continue;
        }
        if turn.presented == Some(WarpKind::ClearOnly) {
            violations.push(ZoomRuleViolation {
                reason: "covered zoom-in presented ClearOnly",
                record: *turn,
            });
        }
    }
    assert!(
        violations.is_empty(),
        "far off-centre ReliefExposure candidate found: {found_idle_relief_exposure}; covered zoom-in ruling violations: {violations:#?}; planner exposure follow-up: {exposure_report:#?}; scenario outcomes: {outcomes:#?}; per-turn traces: {traces:#?}"
    );
}

#[test]
fn corner_box_redraw_and_final_make_bounded_progress_after_stale_recovery() {
    let trace_box = |scenario, centre, size| {
        let anchor = anchor_px_up(
            centre,
            MEASURED_FINAL_EXTENT.map(f64::from),
            MEASURED_FINAL_EXTENT,
        )
        .expect("box centre maps into the measured frame");
        let delta_log2 = box_zoom_delta_log2(size, MEASURED_FINAL_EXTENT.map(f64::from))
            .expect("box has a finite zoom");
        let edits = [(0, delta_log2, Some(anchor))];
        drive_measured_zoom_trace(ZoomScript {
            scenario,
            retained_level: RefinementLevel::Final,
            edits: &edits,
            initial_scene: ZoomInitialScene::SettledFinal,
            refinement_on_edit: ZoomRefinementOnEdit::Restart,
            edit_trigger: ZoomEditTrigger::StaleViewRecovery,
            forget_records_before_selection_at_turn: None,
            destination_extent: MEASURED_FINAL_EXTENT,
            lattice_probe: ZoomLatticeProbe::Skip,
        })
    };
    let corner = trace_box(
        "corner box after stale-view recovery",
        [200.0, 130.0],
        [320.0 - 80.0, 200.0 - 60.0],
    );
    let centred = trace_box(
        "centred box after stale-view recovery",
        [450.0, 250.0],
        [600.0 - 300.0, 350.0 - 150.0],
    );
    let edited = corner
        .iter()
        .find(|turn| turn.edit_state == ZoomEditState::Applied)
        .expect("corner box edit appears in its turn trace");
    assert_eq!(edited.planned, WarpKind::ReliefRedraw, "{corner:#?}");
    assert_eq!(
        edited.source_coverage,
        ZoomSourceCoverage::NotCovering,
        "the corner box must exercise honest outside-source margins: {corner:#?}"
    );
    assert_eq!(
        centred
            .iter()
            .find(|turn| turn.edit_state == ZoomEditState::Applied)
            .map(|turn| turn.source_coverage),
        Some(ZoomSourceCoverage::CoversDestination),
        "the centred comparison must exercise retained-source coverage: {centred:#?}"
    );

    let first_uncovered = corner.iter().find(|turn| {
        turn.requested_revision != 0
            && !turn.completed_requested_final
            && !turn.scene_in_flight
            && !matches!(
                turn.visible_kind,
                Some(WarpKind::ReliefRedraw | WarpKind::HoldStale)
            )
            && !matches!(
                turn.warp_in_flight_kind,
                Some(WarpKind::ReliefRedraw | WarpKind::HoldStale)
            )
    });
    let corner_final = corner.iter().find(|turn| turn.completed_requested_final);
    let centred_final = centred.iter().find(|turn| turn.completed_requested_final);
    let final_presentation = corner_final.and_then(|completed| {
        corner.iter().find(|turn| {
            turn.turn > completed.turn && turn.presented == Some(WarpKind::AnchorHomography)
        })
    });
    assert!(
        first_uncovered.is_none() && corner_final.is_some(),
        "a visible or in-flight redraw, visible or in-flight hold, or in-flight scene must cover every turn until the requested Final arrives; first uncovered: {first_uncovered:#?}; Final: {corner_final:#?}; trace: {corner:#?}"
    );
    assert_eq!(
        corner_final.map(|turn| turn.turn),
        centred_final.map(|turn| turn.turn),
        "a translated box must not retire the requested main scene or add a replacement flight; corner: {corner:#?}; centred: {centred:#?}"
    );
    assert!(
        final_presentation.is_some(),
        "the request must survive until the requested Final's anchor presentation: {corner:#?}"
    );
}

fn measured_height_drag_pose(row: HeightDragRow, height_scale: f64) -> Pose {
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

fn measured_height_drag_plan(
    row: HeightDragRow,
    retained_height_scale: f64,
    requested_height_scale: f64,
) -> WarpKind {
    let retained = measured_height_drag_pose(row, retained_height_scale);
    let frame = SceneFrame {
        scene_id: 37,
        pose: retained,
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
        &measured_height_drag_pose(row, requested_height_scale),
        PrecisionMode::PictureFast,
        WarpValidation::Ordinary,
    )
    .kind
}

#[test]
fn manual_final_height_change_keeps_the_accepted_relief_redraw_live() {
    let mut frame_loop = FrameLoop::default();
    frame_loop.set_scene_mode(SceneMode::Manual, 37, true);
    assert_eq!(frame_loop.due(), None);
    assert_eq!(
        measured_height_drag_plan(
            HeightDragRow {
                name: "close-d5-2",
                distance_five: 2.0,
            },
            0.0,
            0.4,
        ),
        WarpKind::ReliefRedraw
    );
    assert!(frame_loop.stopped().is_none());
}

#[test]
fn non_dispatching_manual_and_deep_reference_waits_keep_retained_records() {
    let source = include_str!("browser/submit.rs");
    let submit = source
        .find("fn submit_due_scene(")
        .expect("the main submit path exists");
    let body = &source[submit..];
    let reference_wait = body
        .find("if matches!(map, PoseMap::Mapped(_))")
        .expect("the deep-reference wait exists");
    let due = body
        .find("let Some(level) = self.loop_state.due()")
        .expect("the paused-schedule wait exists");
    let overwrite = body
        .find("self.presenter.forget_retained_records(&self.grid);")
        .expect("record invalidation marks the actual overwrite");
    assert!(reference_wait < overwrite && due < overwrite);
}

#[derive(Clone, Copy, Debug, Default)]
struct HeightDragStats {
    clear_only_before: u64,
    clear_only_presentations: u64,
    hold_presentations: u64,
    relief_redraw_presentations: u64,
    final_after_drag_ms: f64,
}

/// Drives a moving retained source while the `clear_only_before` classification is also measured
/// from one fixed flat source; both must remain painted throughout either height-drag fixture.
#[allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    reason = "the timed two-row drag harness keeps its scheduler, presenter, and reported counts in one visible trace"
)]
fn drive_height_drag(row: HeightDragRow) -> HeightDragStats {
    const DRAG_FRAMES: u32 = 90;
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
    let mut retained_height_scale = 0.0;
    let mut pending_height_scale = None;
    let mut clear_only_before = 0_u64;

    for input in 1..=DRAG_FRAMES {
        let requested_height_scale = 4.0 * f64::from(input) / f64::from(DRAG_FRAMES);
        let flat_source_plan = measured_height_drag_plan(row, 0.0, requested_height_scale);
        if flat_source_plan == WarpKind::ClearOnly {
            clear_only_before = clear_only_before.saturating_add(1);
        }
        if presenter.pending.is_some() {
            flight_frames += 1;
            if flight_frames == FLIGHT_FRAMES {
                presenter.fire_completed_callback();
                flight_frames = 0;
                retained_height_scale = pending_height_scale
                    .take()
                    .expect("an in-flight scene carries its requested height");
                let events = FrameLoop::refresh(&mut presenter, clock.now_ms);
                assert_eq!(events.len(), 1, "{} completed scene event", row.name);
                let FakeEvent::Completed(scene) = events[0] else {
                    panic!("{} completed scene event", row.name);
                };
                assert!(frame_loop.completed(scene.id, scene.generation, scene.level));
                frame_loop.restart(37);
            }
        }
        if presenter.pending_warp.is_some() {
            presenter.fire_warp_completed();
        }
        frame_loop.accept_request(37, true);
        frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false)));
        let moving_source_plan =
            measured_height_drag_plan(row, retained_height_scale, requested_height_scale);
        assert_eq!(
            moving_source_plan == WarpKind::ClearOnly,
            flat_source_plan == WarpKind::ClearOnly,
            "{} input {input}: refusal must remain destination-driven",
            row.name
        );
        presenter.forced_warp_kind = Some(moving_source_plan);
        let turn = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        if let Some(scene_id) = turn.scene_id {
            assert!(scene_id > last_scene_id, "{} scene ids", row.name);
            last_scene_id = scene_id;
            pending_height_scale = Some(requested_height_scale);
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
    let settled_scene = loop {
        if presenter.pending_warp.is_some() {
            presenter.fire_warp_completed();
        }
        if presenter.pending.is_some() {
            flight_frames += 1;
            if flight_frames == FLIGHT_FRAMES {
                presenter.fire_completed_callback();
                flight_frames = 0;
                retained_height_scale = pending_height_scale
                    .take()
                    .expect("an in-flight scene carries its requested height");
            }
        } else {
            frame_loop.restart(37);
            frame_loop.skip_drafts_for_accepted_warp(Some((RefinementLevel::Final, false)));
        }
        presenter.forced_warp_kind =
            Some(measured_height_drag_plan(row, retained_height_scale, 4.0));
        let turn = drive_viewer_harness(&mut frame_loop, &mut presenter, clock, true);
        if let Some(scene_id) = turn.scene_id {
            assert!(scene_id > last_scene_id, "{} settled scene id", row.name);
            last_scene_id = scene_id;
            pending_height_scale = Some(4.0);
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
        clear_only_before,
        clear_only_presentations: presenter.presented_clear_only,
        hold_presentations: presenter.warp_hold_count,
        relief_redraw_presentations: presenter.warp_relief_redraw_count,
        final_after_drag_ms: clock.now_ms - drag_ended_ms,
    };
    eprintln!(
        "height_drag row={} clear_only_plans={} clear_only_presented={} holds={} relief_redraws={} final_ms={:.3}",
        row.name,
        stats.clear_only_before,
        stats.clear_only_presentations,
        stats.hold_presentations,
        stats.relief_redraw_presentations,
        stats.final_after_drag_ms,
    );
    stats
}

#[test]
fn three_second_height_drag_keeps_both_measured_rows_painted_and_settles_in_one_round() {
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
        assert_eq!(stats.clear_only_before, 0, "{}", row.name);
        assert_eq!(stats.clear_only_presentations, 0, "{}", row.name);
        assert_eq!(stats.hold_presentations, 0, "{}", row.name);
        assert!(stats.relief_redraw_presentations > 0, "{}", row.name);
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
    assert!(source.contains("JulibrotKernels::plan_grid_pair"));
    assert!(source.contains("allocate_grid_pair(&mut executor, &plan)"));
    assert!(submit.contains("std::mem::swap(&mut self.grid, spare);"));
    assert!(submit.contains("self.grid_round != self.loop_state.ladder_round()"));
}

#[test]
fn heap_exhaustion_degrades_to_a_main_only_frame() {
    assert_eq!(optional_backdrop_plan(Err(KernelError::Heap)), Ok(None));
}

#[test]
fn no_backdrop_and_main_only_fallbacks_release_retained_backdrop_records() {
    let backdrop = include_str!("browser/backdrop.rs");
    let release_branch =
        "else {\n            self.release_backdrop()?;\n            return Ok(false);\n        };";
    assert_eq!(backdrop.matches(release_branch).count(), 2);

    let ensure = backdrop
        .find("fn ensure_backdrop_grid(")
        .expect("the optional backdrop allocator exists");
    let ensure_body = &backdrop[ensure..];
    let release = ensure_body
        .find("self.release_backdrop()?;")
        .expect("an old backdrop is released first");
    let optional = ensure_body
        .find("optional_backdrop_plan(")
        .expect("heap exhaustion can select the main-only fallback");
    let allocation = ensure_body
        .find("allocate_grid(&mut self.executor, &plan)")
        .expect("allocation can select the main-only fallback");
    assert!(release < optional && optional < allocation);

    let release_backdrop = ensure_body
        .find("pub(super) fn release_backdrop(")
        .expect("the backdrop release hook exists");
    let release_body = &ensure_body[release_backdrop..];
    assert!(release_body.contains("self.free_grid(&backdrop.grid)"));

    let submit = include_str!("browser/submit.rs");
    let free = submit
        .find("pub(super) fn free_grid(")
        .expect("the shared grid-free hook exists");
    assert!(submit[free..].contains("self.presenter.forget_retained_grid(grid);"));
}

#[test]
fn precision_replacement_installs_the_plan_that_sized_the_new_pair() {
    let source = include_str!("browser/submit.rs");
    let synchronize = source
        .find("fn synchronize_precision_mode(")
        .expect("the precision synchronizer exists");
    let body = &source[synchronize..];
    let allocation = body
        .find("allocate_grid_pair(&mut self.executor, &next_plan)")
        .expect("the replacement pair uses the new plan");
    let install = body
        .find("self.plan = next_plan;")
        .expect("the new plan is installed");
    let grid = body
        .find("self.grid = next_grid;")
        .expect("the new grid is installed");
    assert!(allocation < install && install < grid);
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

// Compatible-reference lease regressions for lane jb-slide-data.

#[test]
fn one_ulp_deep_zoom_is_lease_compatible_and_a_threshold_pan_is_not() {
    const WIDTH: u32 = 960;
    const HEIGHT: u32 = 540;
    const CAP: u32 = 512;
    let mut viewer = ViewerController::new([WIDTH, HEIGHT]).expect("canonical viewer");
    let initial = viewer
        .take_reference_submission()
        .expect("startup navigation");
    assert!(viewer.owner_mut().accept_navigation_without_orbit(
        initial.navigation.generation,
        initial.navigation.centre_revision,
    ));

    viewer.set_zoom_log2(14.0).expect("deep zoom");
    let accepted = viewer
        .take_reference_submission()
        .expect("deep view requests its first orbit");
    let accepted_centre = accepted.navigation.centre.clone();
    let plane = viewer.checked_plane();
    let precision = precision_for(14.0, WIDTH, CAP).expect("deep precision");
    assert!(viewer.owner_mut().accept_navigation_with_orbit(
        accepted.navigation.generation,
        accepted.navigation.centre_revision,
        7,
        CAP,
        precision.requested_bits,
    ));
    let lease = ReferenceLeaseIdentity {
        main_generation: accepted.navigation.generation,
        source_generation: accepted.navigation.generation,
        centre_revision: accepted.navigation.centre_revision,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: precision.requested_bits,
        orbit_length: CAP,
    };

    let nudged_zoom = f64::from_bits(14.0_f64.to_bits() + 1);
    viewer.set_zoom_log2(nudged_zoom).expect("one-ulp zoom");
    let nudged = viewer
        .take_reference_submission()
        .expect("the owner exposes the coalesced navigation");
    let nudged_precision = precision_for(nudged_zoom, WIDTH, CAP).expect("nudged precision");
    assert_eq!(nudged.navigation.centre, accepted_centre);
    assert!(
        !reference_submission_requires_worker(
            false,
            true,
            plane,
            nudged.navigation.precision_mode,
            nudged_precision.requested_bits,
            CAP,
            Some(lease),
        ),
        "zoom is dispatch scale, not a reason to issue another orbit request"
    );
    assert!(viewer.owner_mut().accept_navigation_with_orbit(
        nudged.navigation.generation,
        nudged.navigation.centre_revision,
        7,
        CAP,
        precision.requested_bits,
    ));
    let mut renewed = lease;
    renew_reference_lease_identity(
        &mut renewed,
        nudged.navigation.generation,
        nudged.navigation.centre_revision,
        nudged.navigation.precision_mode,
    );
    assert!(perturbation_reference_is_current(
        nudged.navigation.generation,
        nudged.navigation.centre_revision,
        plane,
        nudged.navigation.precision_mode,
        nudged_precision.requested_bits,
        CAP,
        Some(renewed),
    ));

    viewer.pan_px([300.0, 0.0]).expect("past-threshold pan");
    let moved = viewer
        .take_reference_submission()
        .expect("the moved centre exposes a worker submission");
    let displacement = viewer.owner().drain_hot().hot.centre_from_reference_px;
    assert!(displacement[0].hypot(displacement[1]) > f64::from(WIDTH) / 4.0);
    assert!(reference_submission_requires_worker(
        false,
        moved.navigation.centre == accepted_centre,
        plane,
        moved.navigation.precision_mode,
        nudged_precision.requested_bits,
        CAP,
        Some(renewed),
    ));
}

#[test]
fn a_different_plane_invalidates_a_lease_at_the_identical_generation() {
    let plane = Plane {
        basis_u: [0.0, 0.0, 1.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let different_plane = Plane {
        basis_u: [1.0, 0.0, 0.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let lease = ReferenceLeaseIdentity {
        main_generation: 11,
        source_generation: 11,
        centre_revision: 6,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: 128,
        orbit_length: 512,
    };
    assert!(!perturbation_reference_is_current(
        11,
        6,
        different_plane,
        PrecisionMode::PictureFast as u32,
        128,
        512,
        Some(lease),
    ));
}

#[test]
fn lease_renewal_refreshes_generation_revision_and_precision_from_navigation() {
    let plane = Plane {
        basis_u: [0.0, 0.0, 1.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let mut lease = ReferenceLeaseIdentity {
        main_generation: 4,
        source_generation: 4,
        centre_revision: 2,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: 128,
        orbit_length: 512,
    };
    renew_reference_lease_identity(&mut lease, 5, 3, PrecisionMode::Deterministic as u32);
    assert!(perturbation_reference_is_current(
        5,
        3,
        plane,
        PrecisionMode::Deterministic as u32,
        128,
        512,
        Some(lease),
    ));
    assert!(!perturbation_reference_is_current(
        5,
        3,
        plane,
        PrecisionMode::PictureFast as u32,
        128,
        512,
        Some(lease),
    ));
}

#[test]
fn a_longer_reference_span_is_not_leased_to_a_lower_cap() {
    let plane = Plane {
        basis_u: [0.0, 0.0, 1.0, 0.0],
        basis_v: [0.0, 0.0, 0.0, 1.0],
    };
    let lease = ReferenceLeaseIdentity {
        main_generation: 8,
        source_generation: 7,
        centre_revision: 4,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: 128,
        orbit_length: 512,
    };
    assert!(reference_submission_requires_worker(
        false,
        true,
        plane,
        PrecisionMode::PictureFast as u32,
        128,
        256,
        Some(lease),
    ));
}

#[test]
fn accepted_reference_facts_keep_verification_separate_from_escalations() {
    let deferred = accepted_reference_facts(ReferenceVerification::Deferred, None, 0);
    assert_eq!(deferred.reference_verification, "Deferred");
    assert_eq!(deferred.consumed_word_error_ulps, None);
    assert_eq!(deferred.precision_escalations, 0);

    let stable = accepted_reference_facts(ReferenceVerification::Stable, Some(2), 0);
    assert_eq!(stable.reference_verification, "Stable");
    assert_eq!(stable.consumed_word_error_ulps, Some(2));
    assert_eq!(stable.precision_escalations, 0);

    let stable_after_escalation =
        accepted_reference_facts(ReferenceVerification::Stable, Some(1), 3);
    assert_eq!(stable_after_escalation.reference_verification, "Stable");
    assert_eq!(stable_after_escalation.consumed_word_error_ulps, Some(1));
    assert_eq!(stable_after_escalation.precision_escalations, 3);

    let deferred_after_escalation =
        accepted_reference_facts(ReferenceVerification::Deferred, None, 2);
    assert_eq!(deferred_after_escalation.reference_verification, "Deferred");
    assert_eq!(deferred_after_escalation.consumed_word_error_ulps, None);
    assert_eq!(deferred_after_escalation.precision_escalations, 2);
}

/// Reproduces the escaped-reference deadlock: a discarded census correction leaves the loop with
/// no reference any later dispatch will accept.
///
/// At a centre outside the set the reference orbit escapes after a handful of iterations, so the
/// census asks for a correction at another orbit point. That request is a navigation: it moves
/// only the orbit point, and its delta is zero, but the owner's exact-edit path still advances
/// both the requested generation and the staged centre revision. When the arrival does not
/// outlive the accepted orbit the loop discards it and returns, leaving the accepted lease naming
/// the generation and centre revision from before the correction. Every later dispatch tests that
/// lease against the correction's own navigation and reads it as out of date, so the scene gate
/// refuses for ever: the schedule keeps its due level with nothing in flight, refinement stays
/// pending, and the presenter holds the previous picture under the pending-work hold.
#[test]
fn a_discarded_census_correction_leaves_a_reference_the_next_dispatch_accepts() {
    const CAP: u32 = 512;
    /// Orbit length the measured row at a centre outside the set delivered at zoom sixty.
    const ESCAPED_AT: u32 = 4;
    const ACCEPTED_GENERATION: u32 = 42;
    const ACCEPTED_CENTRE_REVISION: u32 = 316;
    const PRECISION_BITS: u32 = 110;

    let plane = Plane {
        basis_u: [1.0, 0.0, 0.0, 0.0],
        basis_v: [0.0, 1.0, 0.0, 0.0],
    };
    let accepted = ReferenceLeaseIdentity {
        main_generation: ACCEPTED_GENERATION,
        source_generation: ACCEPTED_GENERATION,
        centre_revision: ACCEPTED_CENTRE_REVISION,
        plane,
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: PRECISION_BITS,
        orbit_length: ESCAPED_AT,
    };
    // A short orbit does serve the generation it was fetched for, which is why exactly one level
    // is drawn before the ladder stops, and that level's census then asks for the correction.
    assert!(perturbation_reference_is_current(
        ACCEPTED_GENERATION,
        ACCEPTED_CENTRE_REVISION,
        plane,
        PrecisionMode::PictureFast as u32,
        PRECISION_BITS,
        CAP,
        Some(accepted)
    ));
    assert!(
        super::sampled_reference_due(true, CAP, ESCAPED_AT, 0, None),
        "a cap that outlasts the escaped orbit asks the census for a longer reference"
    );

    // The correction's navigation: a zero delta that still advances both counters by one.
    let correction_generation = ACCEPTED_GENERATION + 1;
    let correction_centre_revision = ACCEPTED_CENTRE_REVISION + 1;

    // The discard hands the orbit already held to the navigation the correction created.
    let mut after_discard = accepted;
    super::adopt_reference_lease_for_correction(
        &mut after_discard,
        correction_generation,
        correction_centre_revision,
    );
    assert!(
        perturbation_reference_is_current(
            correction_generation,
            correction_centre_revision,
            plane,
            PrecisionMode::PictureFast as u32,
            PRECISION_BITS,
            CAP,
            Some(after_discard)
        ),
        "a discarded correction must leave the accepted orbit serving the navigation the \
         correction itself created; that navigation is the same view, so refusing it strands \
         every later dispatch"
    );

    // The orbit that was already too short is still too short, so the levels above it draw
    // reference-exhausted records rather than nothing: the lease keeps its escaped length.
    assert_eq!(after_discard.orbit_length, ESCAPED_AT);
    // The navigation before the correction is not served by the adopted lease: the orbit answers
    // one navigation, and moving it forward is not the same as renewing it across a view change.
    assert!(!perturbation_reference_is_current(
        ACCEPTED_GENERATION,
        ACCEPTED_CENTRE_REVISION,
        plane,
        PrecisionMode::PictureFast as u32,
        PRECISION_BITS,
        CAP,
        Some(after_discard)
    ));

    // The schedule the discard leaves behind: the level whose census asked is due again under the
    // correction's generation, so the round costs one resumed level rather than a repaint from
    // Preview, and no state is reachable in which a level is due that no dispatch may serve.
    let mut ladder = FrameLoop::default();
    ladder.restart(ACCEPTED_GENERATION);
    ladder.submitted(7, RefinementLevel::Preview);
    assert!(ladder.completed(7, ACCEPTED_GENERATION, RefinementLevel::Preview));
    assert_eq!(ladder.due(), Some(RefinementLevel::Interactive));
    ladder.scene_input_resumed(correction_generation, RefinementLevel::Interactive);
    assert_eq!(ladder.due(), Some(RefinementLevel::Interactive));
    assert_eq!(ladder.generation(), correction_generation);
    assert!(ladder.refinement_pending());
}

/// Pins when an idle automatic ladder starts another Final.
///
/// A redraw may stamp the requested view before its replacement scene lands. If that scene is
/// retired, the surface no longer reads stale, so the retained completed Final is the authority.
#[test]
fn automatic_recovery_stops_only_when_the_requested_final_exists() {
    use super::SceneMode::Auto;
    assert!(
        !super::stale_view_needs_a_new_scene(Auto, false, true, true),
        "a completed requested Final only needs its presentation warp"
    );
    assert!(
        !super::stale_view_needs_a_new_scene(Auto, false, false, true),
        "a completed requested Final needs no replacement scene"
    );
    assert!(
        super::stale_view_needs_a_new_scene(Auto, false, true, false),
        "a stale view with no requested Final starts the ladder"
    );
    assert!(
        super::stale_view_needs_a_new_scene(Auto, false, false, false),
        "an accepted retained warp cannot conceal a missing requested Final"
    );
    assert!(!super::stale_view_needs_a_new_scene(
        Auto, true, true, false
    ));

    let mut recovering = FrameLoop::default();
    recovering.request_missing_final(7);
    assert_eq!(recovering.due(), Some(RefinementLevel::Preview));
    assert!(recovering.warp_requested(FramePolicy::SingleFrameOnDemand));
    recovering.warp_submitted();
    assert!(
        recovering.warp_requested(FramePolicy::SingleFrameOnDemand),
        "the redraw must not consume the request that will present the eventual Final"
    );
}

/// Pins that manual refinement still records every stale turn.
///
/// Coverage may select a hold in manual mode, but the pending-update flag must not depend on that
/// presentation decision. Manual refinement starts no work in this rule, so the observation is
/// bookkeeping: manual mode marks the update pending and waits, while auto mode restarts.
#[test]
fn manual_refinement_records_a_stale_view_even_while_a_hold_persists() {
    use super::SceneMode::{Auto, Manual};
    assert!(
        super::stale_view_needs_a_new_scene(Manual, false, true, false),
        "a manual page records the moved pose in the very gap auto refinement waits through"
    );
    assert!(super::stale_view_needs_a_new_scene(
        Manual, false, true, true
    ));
    assert!(
        !super::stale_view_needs_a_new_scene(Manual, true, true, false),
        "a pending ladder is already the record that work is due"
    );
    assert!(!super::stale_view_needs_a_new_scene(
        Manual, false, false, false
    ));

    // What the observation does in each mode: manual marks the update and stays paused.
    let mut manual = FrameLoop::default();
    manual.set_scene_mode(SceneMode::Manual, 7, true);
    manual.scene_changed(7);
    assert!(manual.scene_update_pending());
    assert!(!manual.refinement_pending());
    let mut auto = FrameLoop::default();
    auto.set_scene_mode(Auto, 7, true);
    auto.scene_changed(7);
    assert_eq!(auto.due(), Some(RefinementLevel::Preview));
}

/// Pins the stale reading a hold must not clear.
///
/// The loop stamps the view it expects the next presented image to reproduce when it submits the
/// warp that will draw it. A held warp draws the last completed picture unmoved, so the image that
/// reaches the canvas belongs to the view that was already there; stamping the requested view
/// against it makes the loop report a picture of another zoom as the current view, and every
/// caller that asks whether the picture is finished is told yes about the wrong picture.
#[test]
fn a_held_warp_does_not_stamp_the_requested_view_as_presented() {
    assert!(!super::warp_presents_requested_view(WarpKind::HoldStale));
    for kind in [
        WarpKind::AnchorHomography,
        WarpKind::ClearOnly,
        WarpKind::ReliefRedraw,
    ] {
        assert!(
            super::warp_presents_requested_view(kind),
            "{kind:?} draws for the requested view and stamps it"
        );
    }
}

/// Builds the viewer state the measured zoom row reaches: a burst of navigations, an accepted
/// reference whose orbit escaped after four iterations, and the census correction in flight.
fn burst_to_an_escaped_reference() -> (ViewerController, u32, u32, ReferenceLeaseIdentity) {
    const WIDTH: u32 = 960;
    const HEIGHT: u32 = 540;
    const CAP: u32 = 512;
    /// Orbit length the measured row at a centre outside the set delivered at zoom sixty.
    const ESCAPED_AT: u32 = 4;
    const ORBIT_ID: u32 = 7;

    let mut viewer = ViewerController::new([WIDTH, HEIGHT]).expect("canonical viewer");
    let initial = viewer
        .take_reference_submission()
        .expect("startup navigation");
    assert!(viewer.owner_mut().accept_navigation_without_orbit(
        initial.navigation.generation,
        initial.navigation.centre_revision,
    ));

    // The gesture: forty slider inputs from 1.259 to 60 in 1.2 s, each one a navigation.
    for step in 1..=40 {
        let zoom = 1.259 + (60.0 - 1.259) * f64::from(step) / 40.0;
        viewer.set_zoom_log2(zoom).expect("burst step");
    }
    let accepted = viewer
        .take_reference_submission()
        .expect("the burst releases its coalesced navigation");
    let precision = precision_for(60.0, WIDTH, CAP).expect("zoom sixty precision");
    assert!(viewer.owner_mut().accept_navigation_with_orbit(
        accepted.navigation.generation,
        accepted.navigation.centre_revision,
        ORBIT_ID,
        ESCAPED_AT,
        precision.requested_bits,
    ));
    let lease = ReferenceLeaseIdentity {
        main_generation: accepted.navigation.generation,
        source_generation: accepted.navigation.generation,
        centre_revision: accepted.navigation.centre_revision,
        plane: viewer.checked_plane(),
        precision_mode: PrecisionMode::PictureFast as u32,
        precision_bits: precision.requested_bits,
        orbit_length: ESCAPED_AT,
    };

    // The census correction: another orbit point on the same view, which still spends a
    // generation and a centre revision.
    let _generation = viewer
        .request_reference_for_pixel(0, [WIDTH, HEIGHT])
        .expect("the census candidate names a reference point");
    let correction = viewer
        .take_reference_submission()
        .expect("the correction is released as its own submission");
    assert_ne!(
        correction.reference_centre, correction.navigation.centre,
        "the correction moves the orbit point, not the view"
    );
    assert_ne!(
        correction.navigation.generation,
        accepted.navigation.generation
    );
    assert_ne!(
        correction.navigation.centre_revision,
        accepted.navigation.centre_revision
    );
    (
        viewer,
        correction.navigation.generation,
        correction.navigation.centre_revision,
        lease,
    )
}

/// Reproduces the escaped-reference deadlock over the owner the browser loop actually drives.
///
/// The owner answers a navigation only through the submission it named: `finish_navigation_submission`
/// clears the in-flight generation, and every acceptance entry that takes a navigation requires it.
/// A discard handled after its submission has been finished therefore has nothing it can hand the
/// accepted orbit to, however the discard path is written, and the accepted lease goes on naming the
/// centre revision from before the correction. The perturbation gate reads that lease against the
/// correction's own navigation, refuses, and the ladder keeps a level due that no dispatch may serve.
#[test]
fn a_discarded_correction_can_only_adopt_while_its_submission_is_in_flight() {
    const CAP: u32 = 512;

    // Finishing the submission first, which is the order the deadlock was measured under.
    let (mut finished_first, generation, centre_revision, lease) = burst_to_an_escaped_reference();
    assert!(finished_first.finish_reference_submission(generation));
    assert!(
        !finished_first.owner_mut().accept_navigation_with_orbit(
            generation,
            centre_revision,
            7,
            lease.orbit_length,
            lease.precision_bits,
        ),
        "a finished submission is a navigation the owner will not answer"
    );
    let stranded = finished_first.owner().drain_main().main;
    assert!(
        !perturbation_reference_is_current(
            stranded.generation_applied,
            stranded.centre_revision,
            lease.plane,
            lease.precision_mode,
            lease.precision_bits,
            CAP,
            Some(lease)
        ),
        "the lease left behind serves no navigation the loop will ask about again"
    );

    // Adopting while the submission is still in flight, which is what the loop must do.
    let (mut adopted, generation, centre_revision, lease) = burst_to_an_escaped_reference();
    assert!(
        adopted.owner_mut().accept_navigation_with_orbit(
            generation,
            centre_revision,
            7,
            lease.orbit_length,
            lease.precision_bits,
        ),
        "the orbit already held answers the navigation the correction created"
    );
    let mut adopted_lease = lease;
    super::adopt_reference_lease_for_correction(&mut adopted_lease, generation, centre_revision);
    let main = adopted.owner().drain_main().main;
    assert_eq!(main.generation_applied, generation);
    assert_eq!(main.centre_revision, centre_revision);
    assert_eq!(
        main.orbit_length, lease.orbit_length,
        "the reference stays as short as it escaped"
    );
    assert!(
        perturbation_reference_is_current(
            main.generation_applied,
            main.centre_revision,
            adopted_lease.plane,
            adopted_lease.precision_mode,
            adopted_lease.precision_bits,
            CAP,
            Some(adopted_lease)
        ),
        "the adopted lease serves the navigation the correction created, so a level can dispatch"
    );
}

/// Pins the refusal end state: a ladder that cannot be served stops saying replacement work is due.
///
/// A due level with nothing in flight is what the presenter's hold of the previous picture rests
/// on. When the reference that level needs cannot be made current and no newer navigation is on the
/// way, the claim is false, so the ladder stands down, the reason is published, and the stale-view
/// rule does not re-arm the same level on the next turn. New scene input clears the verdict.
#[test]
fn a_ladder_that_cannot_be_served_stands_down_and_says_why() {
    let mut refused = FrameLoop::default();
    refused.restart(42);
    assert!(refused.refinement_pending());
    refused.refuse_scene(super::STRANDED_CORRECTION_REASON);
    assert!(
        !refused.refinement_pending(),
        "the hold has nothing to wait for"
    );
    assert_eq!(
        refused.scene_refusal(),
        Some(super::STRANDED_CORRECTION_REASON)
    );
    refused.scene_changed(42);
    assert!(
        !refused.refinement_pending(),
        "a stale view does not re-arm a level the loop just refused"
    );
    refused.scene_input_ready(43);
    assert_eq!(refused.due(), Some(RefinementLevel::Preview));
    assert_eq!(
        refused.scene_refusal(),
        None,
        "new scene input retires the verdict"
    );
}
