# Exact N-dimensional camera

`ember-camera` owns a two-dimensional view embedded in N-dimensional space without owning a renderer, a game, or a laboratory. Its exact record is the authority for centre, logarithmic scale, and image-plane orientation; an observer and rebuilt floating-point basis are presentation products, never alternate camera state.

## Degrees of freedom

An oriented two-plane in N-space needs `2N - 3` independent angles: the first unit vector contributes `N - 1`, and the second contributes `N - 2` after orthogonality removes one direction. Adding N centre coordinates and one scale gives `N + (2N - 3) + 1 = 3N - 2` camera degrees of freedom. That is 10 for `N = 4` and 13 for `N = 5`.

The stored orientation deliberately carries the complete `SO(N)` frame, `N(N - 1)/2` integer turns, rather than only the minimal two-plane coordinates. The additional frame axes make future rotations and point framing deterministic. `Orientation<N>` uses a nested `[[Turn; N]; N]`; only the strict upper triangle is active and every unused slot is zero. Product order is row-major over that triangle, so the five-dimensional order is 12, 13, 14, 15, 23, 24, 25, 34, 35, 45. Mapping Julibrot's older product order belongs to its adoption lane.

## Exact record and width

`Fixed<LIMBS>` is a signed two's-complement fixed-point integer whose least-significant limb comes first. One complete 64-bit limb is reserved for the sign and integer part, giving the standard range from −2⁶³ through the fixed value immediately below 2⁶³; every remaining bit is fractional. The first consumer names `LIMBS = 8`, hence 512 total bits and 448 fractional bits. The type remains generic because width is a consumer budget, not a crate ceiling.

Add, subtract, negate, left shift, integer multiply, and full fixed multiply are checked. An unrepresentable result is a typed refusal and leaves an edited `View` unchanged. Arithmetic right shift drops low bits toward negative infinity. Full fixed-by-fixed multiplication forms the complete double-width product and rounds once to nearest, ties to even; that is the fixed arithmetic's only rounding. Binary64 ingress and egress, scale division by grid width, exponent selection, projection truncation, and midpoint selection are separately named boundary quantisations.

The transport shape avoids unstable generic const expressions. One fixed number is `[[u8; 8]; LIMBS]`, low limb first and every limb little-endian. A `ViewBytes<N, LIMBS>` nests N such coordinates beside a little-endian `i32` exponent and the nested little-endian turn matrix. Every stored view field is `Copy + Eq`, so copying a history entry is a value copy and comparing histories is bit-exact.

## Coordinate and scale law

Pixel origin is the canvas centre. Positive x points right and positive y points up, in render-grid pixels. `Screen` stores a nonzero `u32` grid width and height. Nominal inputs are accepted through a 2³¹-pixel magnitude, which covers every centred `u32` render extent while fixed storage retains headroom for exact differences. `project` and the `click` decoder share an outer limit of three upward binary64 steps beyond that nominal boundary. This tiny readout envelope ensures that an accepted boundary point remains accepted when projection's last rounding step lands just outward; it does not enlarge the consumer's nominal pointer range.

`Exponent` counts 1024 quanta per octave. The quantum is the finest step produced by the first consumer's input devices, so scale can always be reconstructed from an integer. The named navigation range is −2 through 120 octaves, inherited from that consumer's reachable control; larger exponents are deeper. At exponent zero the view spans four plane units across the grid width:

```text
pixel_scale = 4 · 2^(−exponent) / grid_width
```

Whole octaves are fixed shifts. The ten fractional exponent bits select bit-pinned dyadic encodings of `2^(−1/2)` through `2^(−1/1024)`, multiplied in a fixed order; the final division by the `u32` width is nearest-even. The factors carry 53 relative bits because the rebuilt basis has the same boundary precision. Once decoded, they are fixed integers: depth changes their bit position, not their relative error, so coefficient error remains a small fraction of a pixel rather than growing into world-space drift. The same exponent and width always yield the same fixed bits on native and wasm32.

## Orientation and presentation

`Turn` is a `u32` fraction of a complete turn. Turn addition wraps exactly. `Basis<N>` is rebuilt from identity on every request by composing the stored Givens rotations; it is never accumulated frame to frame. Sine and cosine use a fixed polynomial operation sequence rather than a platform math-library call. Rebuilding the same turns therefore rebuilds the same binary64 bits, and adding a turn delta followed by its stored inverse restores the angle record exactly.

`Observer<N>` contains binary64 yaw, pitch, view-space translation, and positive perspective distance. These values never enter `View` and no exact edit reads an observer. Yaw is in radians about the image-plane `v` axis and pitch about `u`, after the plane rotations and before perspective division; translation is in view units. The adopting lane maps its controls onto this convention once.

The only binary64 values admitted by an exact pointer edit are the input pixel and rebuilt basis component bit patterns. Each is decoded directly into `Fixed`; all scale, weighting, anchoring, and centre updates after that boundary are integer operations. Projection takes the opposite route: it performs every large subtraction first, converts the already-small displacement to screen units, and uses binary64 only for the final short dot products and two-by-two Gram solve. The solve compensates the rebuilt basis's bounded polynomial non-orthogonality rather than letting it scale into pixel error at the edge of the accepted range.

## Reversibility

An anchored edit computes the point offset under the old record and the offset under the candidate record, then applies their exact fixed difference to a staged centre. A failed coordinate prevents the staged record from replacing the view. Because every offset is a pure rebuild, any successful anchored edit followed by its exact stored inverse presents identical offset bits in reverse order and cancels bit-exactly at every exponent quantum. Multiplication rounding affects offset construction, not cancellation.

For the first consumer, one lowest bit is `2^-448` plane units. Projection is checked separately because it is a readout boundary: the exhaustive sweep over every exponent quantum, all ten five-dimensional rotations, modest pixels, pixels near two billion, and the nominal maximum measured `PROJECT_PIXEL_TOLERANCE_PIXELS = 7.152557373046875e-7` render pixels. In the reverse direction, points on the integer render-grid pixel lattice rebuild bit-exactly. Any other represented image-plane point rebuilds within `PROJECT_READOUT_ULPS = 3` binary64 steps of each returned pixel coordinate, scaled into plane units by the current pixel scale. The qualification to the image plane is necessary because projection intentionally discards all orthogonal components.

`frame_points` uses an exact midpoint. When the two raw fixed integers have an odd sum, their mathematical midpoint lies halfway between representable values; the function drops that one lowest bit toward negative infinity. It then fits twice the larger absolute projection of the two endpoint-minus-centre vectors on each image axis, so that midpoint asymmetry cannot crop the positive endpoint. It deterministically aligns `u` with the segment, makes the previous `v` orthonormal to it, and quantises the deepest containing exponent. `select_box` likewise chooses the deepest scale that does not crop either axis. One exponent quantum permits at most `(2^(1/1024) - 1) = 0.0006771306930664078` relative inward edge slack, plus `PROJECT_PIXEL_TOLERANCE_PIXELS`.

## Base-plane navigation

Perspective inversion targets the flat base plane at height zero. A ray that is parallel to, or points away from, that plane returns `None`; the named parallel threshold is `1e-12` in the perspective denominator. Navigation does not intersect relief because a relief mesh can be incomplete, delayed, or rebuilt by a different presentation path. A surface snap is allowed as a presentation convenience only: it produces a base-plane screen point and then calls `click`, leaving one closed-form, reproducible navigation authority.

## Calculation inventory

In the table, `N` is ambient dimension, `S ≤ 10` is the number of selected fractional-octave factors in one scale rebuild, `B = 17` is the maximum binary-search count across the named exponent range, `M` is one full fixed-by-fixed multiply, `I` is an exact small-integer multiply, and `A` is an add or subtract. Scale rebuild also performs one whole-octave shift and one small-divisor nearest-even division. Counts are analytic worst cases from the implementation; only the combined worst-case edit and per-frame displacement are timed.

|\#|Calculation|Inputs|Precision boundary|Analytic big-number work|Measured wall|
|-:|-----------|------|------------------|------------------------|-------------|
|1|`click`|view, screen, centred pixel|Pixel and rebuilt basis bits enter fixed precision; result stays fixed.|`(S + 2 + 2N)M + 2N A`, one scale division.|Not separately timed.|
|2|`project`|view, screen, exact point|`N` fixed differences are truncated to screen-scale binary64, then two displacement dots, three basis Gram dots, and a fixed two-by-two solve.|`N A` plus scale rebuild; no point multiply.|Not separately timed.|
|3|`pan`|mutable view, screen, pixel delta|Input bits enter fixed once; staged centre remains fixed.|`(S + 2 + 2N)M + 2N A`, one scale division.|Not separately timed.|
|4|`zoom_about`|mutable view, screen, anchor, exponent delta|Old and new fixed offsets share one exact pixel anchor.|At most `(2S + 4 + 4N)M + 4N A`, two scale divisions.|Combined with row 5: 21,340 ns.|
|5|`rotate_about`|mutable view, screen, anchor, integer turn deltas|Old and new basis bits enter fixed; angle and centre records remain integers.|At most `(2S + 4 + 4N)M + 4N A`, two scale divisions.|Included in the row 4 combined edit wall.|
|6|`frame_points`|mutable view, two exact points, screen|Exact difference and midpoint; fixed CORDIC returns integer turns after basis bits enter fixed precision.|`2N - 3` CORDIC vectorings, each `31 × (2 shifts + 2A) + 1M`; at most `N²(N - 1)M/A` for fallback `v` projection; then `4N M + 11N A + 2I`, `2N` midpoint shifts, and at most `B(SM + 2I)` scale-fit work.|Not separately timed.|
|7|`select_box`|mutable view, two centred pixel corners, screen|Corner bits enter fixed; centre and fit stay fixed; exponent is quantised.|One `click`, `2M`, two pixel differences, two midpoints, plus at most `B(SM + 2I)` scale-fit work.|Not separately timed.|
|8|`invert_perspective`|observer, screen, centred pixel|Presentation-only binary64 ray and closed-form base-plane intersection.|None.|Not separately timed.|
|9|`reference_displacement`|view, exact reference centre, screen|`N` fixed differences are truncated to screen-scale binary64, then two displacement dots, three basis Gram dots, and a fixed two-by-two solve.|`N A` plus scale rebuild.|3,580 ns.|

The release timing contract measures rows 4 and 5 together at `N = 5`, `LIMBS = 8`, with a fractional exponent selecting every table factor and all ten orientation planes active, then measures row 9 separately. The first server release sample measured 21,340 ns for the edit and 3,580 ns for reference displacement, 24,920 ns combined, or about 25 microseconds. A second run on a loaded core measured 55,210 ns and 7,960 ns respectively, 63,170 ns combined; this records the observed load spread, and both samples remain far below the one-millisecond bound. Debug tests report the same quantities without enforcing the performance ceiling.

The gate evidence is 31 unit tests plus the release timing test; no doctests.
