# Julibrot navigation calculation inventory

Status: Phase-B audit of the navigation and presentation paths at `225e66bc`. This document records the current implementation; it is not the exact-camera design. `ember-camera` is the selected replacement for the pointer-to-view portion described below.

## Scope and terms

The stored navigation state is currently split between `ViewerController::requested`, the worker-owned `NavigationState`, and the optional bignum crosshair point. A `Pose` does not carry an absolute navigation centre or a reference-centre identity. It carries a binary64 `centre_from_reference_px`, and the warp code assumes that the source and destination values use the same reference coordinate frame.

The precision labels in the inventory are:

- **B64/B128/B1024**: Astro-float values at 64, 128, or 1,024 bits. The Phase-A PictureFast runs use B64 at initial/presented exponents 14/14.5 and B128 at 54/54.5; deterministic promotion uses B1024.
- **F64 LOSS**: binary64 arithmetic or storage, with 53 significand bits.
- **F32 LOSS**: binary32 arithmetic or storage, with 24 significand bits.
- **Integer**: the stated integer operation is exact before a later conversion.

The bignum operation count is source-derived. One `BigCentre::apply_navigation` performs 41 bignum arithmetic operations: one scale subtraction, then four coordinates times two two-multiply/one-add linear combinations, two shift multiplies, one add, and one subtract. It also performs six scalar lifts and four binary64 validation conversions. One `BigCentre::displacement_px` performs 22 bignum arithmetic operations: four coordinate subtractions, then two projections of four multiply-add pairs and one division. Coordinate clones, validation, and ordinary floating-point operations are not counted as bignum operations.

## Cost basis

There is no per-function navigation benchmark. The warmed sokol measurement in [`precision-ledger.md`](precision-ledger.md) is 3.92 seconds for 10,000 1,024-bit mixed owner edits, or **0.392 ms/edit**, including owner publication, HOT drains, and test assertions. The osprey measurement recorded in [`math.md`](math.md) is 3,967.833 ms, or **0.3967833 ms/edit**, for the same deterministic arm. These are conservative envelopes for the centre-edit row, not timings for each subcalculation. The same ledger measures the 1,536-case tumbled planner corpus at **0.197184 ms/case**, including planning and assertions.

Every other cost cell marked **unmeasured** is a finding. In particular, the existing backlog already calls for a browser profile of the per-refresh crosshair projection; no repository result can honestly assign it a wall time.

## Calculation inventory

|ID|Calculation and current code|Inputs and result|Precision and loss|Bignum operations|Measured cost|
|--|----------------------------|-----------------|------------------|----------------:|-------------|
|N1|CSS pointer or drag to centred render-grid pixels: app `anchor_px_up`, `drag_delta_px_down`; the live box midpoint in `app_zoom_box`|CSS pointer/delta, CSS rectangle, integer grid extent -> pixel anchor/delta|**F64 LOSS**. Integer fixture coordinates `[240,135]` bypass this row and lose zero bits; a non-integral CSS/grid ratio is rounded to 53 bits.|0|Unmeasured.|
|N2|Neutral-height screen/plane chart: `ViewerController::screen_map`, math `screen_to_plane`, `construct_plane`|Stored object/view orientation, extent -> forward and inverse homographies and plane basis|Homography is **F64 LOSS**; `Plane` is **F32 LOSS**. The canonical Phase-A view takes the exact identity-map branch and its seed basis is exact. General angles are not exact.|0|Unmeasured; the three-entry map cache avoids most refresh rebuilds.|
|N3|Screen point and drag mapping: math `navigation_delta` and guarded homography evaluation|N1 pixels plus N2 inverse map -> plane-chart anchor and pan in pixels|**F64 LOSS**. It is exact for the Phase-A identity map and integer anchor, but not for a general camera angle.|0|Unmeasured.|
|N4|Box and absolute zoom amount: app `box_zoom_delta_log2`, `ViewerController::set_zoom_log2`|Box sides/viewport or absolute scale -> `delta_log2`|**F64 LOSS** through division, `log2`, and subtraction. A fractional wheel delta is likewise an F64 value. Phase A's `0.5` is exactly binary.|0|Unmeasured.|
|N5|Centre, target, or sampled-reference point update: app `wheel_zoom`, `drag_pan`, `set_crosshair`, `request_reference_for_pixel`; worker `ViewerOwner::navigate_checked`; math `BigCentre::apply_navigation`|N3 pan/anchor, N4 exponent, current centre, plane, width -> new bignum centre or point|Centre arithmetic is B64/B128 in the Phase-A PictureFast runs. **F32 LOSS** enters first: `pixel_scale` reconstructs an F64 value from the 24-bit mantissa made by `scaled_pixel_scale`; pointer values and exponent are **F64 LOSS**, and basis lanes are **F32 LOSS**. No centre coordinate is converted to F64 before the bignum update.|41 arithmetic + 6 lifts + 4 mirror checks per call|The complete 1,024-bit owner edit is within the measured 0.392 ms/edit envelope; this function alone is not timed.|
|N6|Centre-width policy: math `centre_precision_for`, `BigCentre::with_precision`; worker `configure_precision_mode` and the growth branch in `navigate_checked`|Mode, current exponent, width, edit budget, centre/reference -> rounded Astro-float width|Current Phase-A setup narrows B1024 to B64 at 14 and B128 at 54, removing 960 and 896 bits of retained capacity and potentially discarding that many low bits. The fixture centres began as F64, so those low lanes were not populated before the first edit. A later growth restores width, not discarded information. The acceptance-time `old.with_precision` discards zero bits in these runs because old and new references already have equal widths.|Configuration: 8 coordinate precision rounds + 22 displacement operations; growth: 8 rounds before N5|Unmeasured; policy setup is outside the timed edit arm.|
|N7|In-plane orientation update: app `set_object_angles`; worker `reorient_navigation_plane`|F64 angles, centre/reference -> F32 basis and reprojected reference displacement|Angles and matrix construction are **F64 LOSS**; stored plane basis is **F32 LOSS**; displacement is B64/B128 then **F64 LOSS** in pixel space. A slice-changing turn clears the target and submits a zero navigation edit instead.|22 for a plane-preserving turn; 41 for the zero edit on a slice change|Unmeasured.|
|P1|Desired-centre/reference displacement: math `BigCentre::displacement_px`, `centre_from_reference_px`, `reference_shift_px`; worker configuration and edit publication|Two equal-width bignum centres, F32 plane, rounded pixel scale -> two current-grid pixel offsets|Subtraction/projection is B64/B128; scale and basis are **F32 LOSS**; only the final pixel values become **F64 LOSS**. At magnitudes at most 390 px, F64 still retains about 44 fractional pixel bits, so this final conversion cannot explain a 114 px step.|22 per displacement|Included in the 0.392 ms/edit envelope when called by owner navigation; no isolated timing.|
|P2|Reference acceptance and retained/pending pose rebase: browser `reference::apply_response`, worker `accept_orbit`, present `Presenter::set_main`, `SceneLedger::apply_reference_shift`, `rebase_pose`|Old/new references, submitted view centre, centre revision and generation -> new navigation context plus rebased scene poses|Old-reference alignment and both displacements are B64/B128; accepted shift, pose offsets, scale ratio, overlaps, and rebase are **F64 LOSS**, with F32 bases. Precision is known, but the reference identity is not a `Pose` field.|44 arithmetic + 4 precision rounds before the F64 rebase|Unmeasured. This row contains the confirmed sequencing defect below.|
|P3|Frame pose and tier scaling: app `ViewerController::drain_hot`, `main_for_grid`, `install_main`; `SceneFrame` stamping|Stored requested controls, HOT displacement, MAIN identity, refinement extent -> the pose carried by submitted and presented frames|No bignum arithmetic. `Pose` stores zoom, plane origin, maps, controls, and displacement in **F64 LOSS**, with an F32 plane; `main_for_grid` rescales reference shift in F64 for the tier width.|0|Unmeasured.|
|P4|Retained-frame reprojection: math `warp_matrix`; present `Warp::reproject` and `source_to_destination_chart`|Source and destination `Pose` values -> source-to-destination homography and presentation plan|Entirely **F64 LOSS** over F32 bases. It consumes reference-relative pixel offsets as if both poses share one reference. It does not call `to_f64_mirror` or subtract two bignum centres.|0|The full tumbled planning case, including assertions, measured 0.197184 ms/case; raw warp construction is not isolated.|
|P5|Crosshair and selection presentation: app `crosshair_plane_px`, `css_from_anchor_px_up`, plus the DOM rectangle drawn before `app_zoom_box`|Stored target and centre -> plane pixels -> screen pixels -> CSS pixels; drag endpoints -> CSS box|Target and centre align at B64/B128, then displacement is **F64 LOSS** in pixels; map and CSS conversion are **F64 LOSS**, plane basis is **F32 LOSS**. Selection drawing itself never consults the stored view, so its displayed rectangle is only CSS-accurate.|8 coordinate precision rounds + 22 displacement operations for the crosshair; 0 for the CSS box|Unmeasured; this is the backlog's requested browser profile.|
|P6|Shallow mirror and shallow-kernel centre: `BigCentre::to_f64_mirror`/`mirror_centre`, `split_centre`, `shallow_pixel_scale`|Stored centre and exponent -> facts/readout F64 centre, or two-F32 centre plus F32 pixel scale|The mirror is **F64 LOSS**. The shallow kernel receives an approximately 48-bit hi/lo centre and a **F32 LOSS** 24-bit scale. These are downstream today, but the F64 mirror is also stored in MAIN and `Pose::plane_origin` remains F64.|Mirror: 4 big-to-F64 conversions. Split: 4 big subtractions plus 12 conversions.|Unmeasured.|
|P7|Backdrop sampling pose: app `sampling_zoom_log2`, `backdrop_map`, backdrop submission|Requested exponent, F64 apron, coarse extent, P1 displacement -> sampling exponent and backdrop pose|**F64 LOSS** for apron `log2`, exponent subtraction, map, and displacement rescaling; shallow backdrop scale then becomes **F32 LOSS**. This changes sampling coverage, not the requested view centre.|0 beyond P1/P6 inputs|Unmeasured.|

## Phase-A precision loss

The test configures exponent 14 or 54, then applies `+0.5`, so its presented poses are at 14.5 or 54.5. Around the Seahorse Valley coordinate, one binary64 ulp is `2^-53` plane units. With `pixel_scale = 4 / (960 * 2^q)`, that ulp is `240 * 2^(q-53)` pixels:

|Configured/presented exponent|One F64 coordinate ulp|Bits after the binary point needed for one pixel|Needed for quarter pixel|F64 result|
|----------------------------:|---------------------:|-----------------------------------------------:|-----------------------:|----------|
|14 / 14.5|`4.36557456851e-10` / `6.17385476234e-10` px|22 / 23|24 / 25|29 / 28 bits of quarter-pixel headroom|
|54 / 54.5|`480` / `678.822509939` px|62 / 63|64 / 65|11 / 12 bits short of quarter-pixel resolution|

An F64 **coordinate** is therefore unusable as feedback at binary64 depth. An F64 **pixel displacement** is different: P1 performs the bignum subtraction and division before converting, so a roughly 390-pixel result has an F64 ulp near `5.7e-14` px. The large Phase-A movements are semantic reference-frame offsets, not the last pixel conversion's rounding.

`scaled_pixel_scale` is narrower still. It rounds its mantissa to F32 before `pixel_scale` converts that mantissa back to F64 and `apply_navigation` lifts it into a bignum. At both Phase-A exponents the before/after mantissa pairs differ only in their power-of-two exponent, so their ratio is the same `1.414213608179249`. The scale stage loses 29 significand bits relative to F64 and 40 or 104 bits relative to the Phase-A B64/B128 centre widths. For this 276-pixel-radius scenario its error is only about `1.3e-5` px, so it violates exact edit reversibility but does not cause the observed hundreds-of-pixels steps.

## Why the measured distances are 114.059 and 390.104 pixels

The Phase-A grid is 960 by 540. The crosshair is `a = [960/4, 540/4] = [240, 135]` pixels and the zoom is `+0.5`. With the implementation's rounded scale ratio `r = 1.414213608179249`, the anchored zoom moves the desired centre relative to the old reference by

```text
d = (r - 1) a
  = [99.411265963020, 55.918837104199] px
|d| = 114.059265925466 px
```

That is the recurring `114.059265925` table entry. It is the full anchor term, not an ulp. The first draft carries `d` against the old reference. Accepting the requested centre as the new reference changes the requested pose displacement from `d` to zero. If the retained/source pose is not rebased at that transition, reprojection moves the anchor by exactly `|d|`.

The sampled reference uses zero-based sample index `405 * 960 + 720`. Pixel-centre addressing makes its screen offset

```text
s = [720 + 0.5 - 960/2, 405 + 0.5 - 540/2]
  = [240.5, 135.5] px
|s| = 276.044380489805 px
```

The view centre does not move, but its displacement from that new reference becomes `-s`. Missing this second rebase therefore causes the `276.044380490` step. The first presented pose differs from the final pose by the vector sum:

```text
d + s = [339.911265963020, 191.418837104199] px
|d + s| = 390.103627164509 px
```

This rounds to the table's `390.103627165`. The two moves are nearly collinear, but the reported 390.104 is the vector norm, not merely the scalar sum of the other printed norms.

## Causal findings

### Confirmed: the centre revision arrives before its reference shift

`ViewerOwner::navigate_checked` increments and publishes `MainState::centre_revision` immediately, while the old reference remains active. It does not reset `reference_shift_px`, so that field still belongs to the preceding acceptance; it is zero for the first Phase-A hand-off. The refresh loop drains that MAIN state and calls `install_main` before servicing arrivals. Consequently `Presenter::set_main` sees `revision_advanced` and calls `SceneLedger::apply_reference_shift` with the preceding shift rather than the shift for this revision.

When the deep reference arrives, `reference::apply_response` computes the nonzero shift, `configure_navigation_context` changes the reference coordinate frame, and `accept_orbit` publishes the shift under the **same** centre revision. `Presenter::set_main` gates rebasing only on `centre_revision`, now sees no advance, and skips the shift. Source poses remain expressed against the old reference while destination poses from `drain_hot` use the new one. P4 then consistently reprojects inconsistent coordinates. A sampled-reference request repeats the same sequence because `request_reference_for_pixel` spends a centre revision with a zero navigation delta before its new reference is accepted; that early revision may replay the preceding shift, and the later sample shift is still ignored.

The Phase-A event positions corroborate the code ordering. Both corrections occur while the tier is still `Fast` and precision is still `PictureFast`: frame 2 moves by the anchored-centre shift, and sampled frame 6 moves by the sample radius. Deterministic promotion later changes precision without moving the anchor.

### Candidate rulings

|Candidate|Ruling|Deciding evidence|
|---------|------|-----------------|
|Anchor arithmetic computes the new centre in F64 before `BigCentre`|**Eliminated as the Phase-A jump; retained as an exact-camera violation.**|N5 calls `BigCentre::apply_navigation`; it does not form the new centre as F64. The exact repeated distance is the intended anchor term, and the pre-acceptance draft holds it. Rounded F32 scale, F64 pointer/exponent, and F32 plane inputs still break the required reversible edit rule.|
|Acceptance narrows the centre with `with_precision` or replaces it with the accepted reference centre|**Precision explanation eliminated; reference-frame transition confirmed.**|The Phase-A old/new reference widths match, so acceptance-time `old.with_precision` drops zero bits. `submitted.view_centre` remains the navigation centre; only `reference_centre` changes. The resulting nonzero shift is published too late for the revision-only rebase gate.|
|Reprojection computes a raw F64 difference between big centres through `to_f64_mirror`|**Eliminated as stated; confirmed as the exposure point.**|`warp_matrix` sees no bignums and never calls `to_f64_mirror`; it consumes P3's F64 reference-relative pixel values. Those values are accurate enough, but belong to different unlabelled reference frames after the missed rebase.|
|Sampling zoom or coarse backdrop rounds the draft centre|**Eliminated for Phase A.**|The native reproduction does not call `sampling_zoom_log2` or construct a backdrop. P7 changes sampling coverage and exponent only.|
|Each refinement tier carries a different centre revision or centre|**Eliminated as an independent tier defect; revision hand-off confirmed.**|The nonzero moves happen within `Fast`/`PictureFast` at reference-acceptance events, while Preview-to-Final and Fast-to-Deterministic hand-offs have zero movement. The problematic identity is the already-observed revision reused for a later reference shift.|
|The anchor term is dropped at a presentation stage|**Confirmed.**|`114.059265925` is the Euclidean norm of `(pixel_scale_before / pixel_scale_after - 1) * [240,135]`. The term exists in HOT before acceptance, becomes zero when the view centre becomes its own reference, and is not applied to retained poses because `set_main` has already consumed the revision. The sampled `276.044380490` term is missed by the same mechanism.|
|Backlog: recompute the boot crosshair's big displacement each refresh|**Adjacent, not the same defect.**|The backlog item is a cost/cache issue in P5 and explicitly asks for a browser profile. This defect is P2's reference-identity/revision sequencing; caching P5 would leave every pose transition unchanged.|

## `ember-camera` adoption boundary

The app remains responsible for DOM event capture and render-grid extent. Once `ember-camera` lands, the boundary is immediately after integer pointer coordinates and before any F64 chart, scale, angle, or centre calculation:

- `app_set_target`, `app_pan_px`, `app_zoom_box`, `app_set_scale`, the wheel entry used by native tests, angle setters, and saved-view application convert external values to the camera crate's integer coordinate, exponent-quantum, and angle types. They no longer derive navigation deltas.
- The camera record replaces the navigation-bearing parts of `RequestedControls`, the `ViewerController` crosshair `BigCentre`, and the implementations of `wheel_zoom`, `drag_pan`, `set_crosshair`, `crosshair_plane_px`, `zoom_about_crosshair`, `set_zoom_log2`, `request_reference_for_pixel`, and the centre/orientation part of `set_object_angles`, `set_plane_angles`, `set_plane_origin`, `set_centre`, and `apply_saved_view`.
- On the math side it replaces edit-path uses of `NavigationDelta`, `navigation_delta`, `BigCentre::apply_navigation`, `centre_precision_for`, edit-path `pixel_scale`, and edit/target uses of `BigCentre::with_precision` and `displacement_px`. Presentation-only adapters such as `screen_to_plane`, `plane_to_screen`, `warp_matrix`, `to_f64_mirror`, `split_centre`, and kernel scale packing may remain only downstream and must never feed values back into the camera record.
- `ViewerOwner` stops being the arithmetic owner and instead publishes snapshots derived from the camera record. The existing `NavigationSubmission`, `EncodedCentre`, worker codec, and kernel centre encoding are outside this adoption boundary. Adapting the fixed 512-bit record to that unchanged protocol is allowed; changing the protocol requires the separately reported deviation required by the lane ruling.

Phase C must independently repair P2-P4 so every source and destination pose has one coherent reference identity throughout acceptance and tier hand-off. Merely replacing N1-N7 with exact camera arithmetic cannot repair a reference shift the presenter never applies.
