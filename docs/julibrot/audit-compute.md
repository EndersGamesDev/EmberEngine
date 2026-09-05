# Julibrot compute audit

All source citations in this audit name the immutable baseline eb5ce23e; later commits are named only when establishing the disposition of an open line, and every timing identified as measured was collected on sokol through the required run-report wrapper.

Every size, count, bound, and policy constant described from a source citation is code-derived and not wall-measured unless its paragraph explicitly labels a sokol measurement; every browser or GPU wall without such a receipt is labelled unmeasured.

## 1. Latency ledger

The control-to-scene path is control mutation and navigation state, worker request construction, transfer into the worker, reference-orbit construction when deep precision requires it, credit-controlled response publication, response transfer and decoding, reference upload, refinement planning and heap dispatch, and scratch landing before presentation; the handoffs are visible at `crates/labs/julibrot/app/src/state.rs:542`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:281`, `crates/labs/julibrot/worker/src/browser.rs:59`, `crates/labs/julibrot/worker/src/compute.rs:115`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:111`, `crates/labs/julibrot/kernels/src/gpu.rs:267`, and `crates/labs/heap/src/executor.rs:1053`.

Request encoding is fixed-cost metadata plus centre limbs proportional to precision: the wire layout is 116 bytes plus two little-endian limb vectors, each encode owns two `Vec<u8>` allocations, and decode owns two more vectors (`crates/labs/julibrot/worker/src/codec.rs:16`, `crates/labs/julibrot/worker/src/codec.rs:44`, `crates/labs/julibrot/worker/src/codec.rs:97`).

The browser bridge allocates four maximum-size buffers at startup and a request uses only its compact prefix, while native decode from a 628-byte prefix measured 675 us for 20,000 copies versus 14,290 us for copying the whole 32,832-byte buffer; this is 0.03375 us versus 0.7145 us per copy, with `RUN-REPORT exit=0 wall=0.2s` (`crates/labs/julibrot/worker/src/browser.rs:132`, `crates/labs/julibrot/worker/src/browser.rs:308`, `crates/labs/julibrot/worker/src/wire.rs:13`).

The reference centre is maintained at 1,024 bits and PictureFast navigation derives lower-width centres under a 10,000-edit error budget (`crates/labs/julibrot/math/src/big.rs:5`, `crates/labs/julibrot/math/src/big.rs:22`, `crates/labs/julibrot/math/src/big.rs:31`, `crates/labs/julibrot/worker/src/owner.rs:304`).

Reference-orbit construction is proportional to the iteration cap and currently grows each returned record vector by repeated pushes from an empty `Vec`; deterministic construction produces one orbit, while PictureFast Final is capable of producing a primary and verification orbit (`crates/labs/julibrot/math/src/orbit.rs:44`, `crates/labs/julibrot/math/src/orbit.rs:192`, `crates/labs/julibrot/math/src/orbit.rs:239`, `crates/labs/julibrot/math/src/orbit.rs:327`).

The shallow f32 path needs no reference construction or worker round trip: it accepts a reference-free configuration immediately, whereas the deep path submits a request and waits for an accepted orbit (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:281`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:297`).

A native read-only app fixture measured the former deep-style work avoided by the shallow branch as 751.947279 ms before and 0.000100 ms after, saving 751.947179 ms, with `RUN-REPORT exit=0 wall=15.4s`; the browser scheduling and transfer portions remain unmeasured (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:281`).

At cap 512, single-orbit generation measured 307 us, its logical payload was 4,096 bytes, packing averaged 6,534 ns, and a deliberately depleted credit balance waited 1,228 us; at cap 4,096 those values were 2,568 us, 32,768 bytes, 57,471 ns, and 10,272 us, with `RUN-REPORT exit=0 wall=11.2s` (`crates/labs/julibrot/math/src/orbit.rs:239`, `crates/labs/julibrot/worker/src/credit.rs:153`).

The paired 16-byte-per-iteration reference baseline measured 634 us, 8,192 bytes, 11,936 ns packing, and 2,536 us depleted-credit wait at cap 512, then 5,097 us, 65,536 bytes, 95,386 ns packing, and 20,388 us depleted-credit wait at cap 4,096, with `RUN-REPORT exit=0 wall=0.3s` (`crates/labs/julibrot/math/src/orbit.rs:327`, `crates/labs/julibrot/worker/src/credit.rs:153`).

Credit price is proportional to measured orbit-generation time but clamped to the interval from one microsecond to 250,000 us, and admission waits are individually bounded by one second (`crates/labs/julibrot/worker/src/credit.rs:16`, `crates/labs/julibrot/worker/src/credit.rs:153`).

Orbit response packing is proportional to cap and writes eight indexed bytes per record into the JavaScript-owned array, so cap 4,096 performs 32,768 indexed setter calls; browser wall time for those calls is unmeasured (`crates/labs/julibrot/worker/src/browser.rs:366`, `crates/labs/julibrot/worker/src/browser.rs:981`).

Request decode is bounded independently of maximum orbit capacity because only the used request prefix and trailer are copied before decoding, with the 628-byte measured upper case above (`crates/labs/julibrot/worker/src/browser.rs:308`, `crates/labs/julibrot/worker/src/codec.rs:405`).

Preview, Interactive, and Final use scale divisors 4, 2, and 1 and cap divisors 64, 16, and 1; PictureFast uses an additional Preview scale divisor of 2 and skips Interactive (`crates/labs/julibrot/kernels/src/refinement.rs:8`, `crates/labs/julibrot/kernels/src/refinement.rs:64`, `crates/labs/julibrot/kernels/src/refinement.rs:114`).

For a 960 by 540 deterministic view, code-derived rather than wall-measured work is Preview 240 by 135 at cap 64, or 2,073,600 pixel-iterations and 518,400 output bytes; Interactive 480 by 270 at cap 256, or 33,177,600 pixel-iterations and 2,073,600 bytes; and Final 960 by 540 at cap 4,096, or 2,123,366,400 pixel-iterations and 8,294,400 bytes (`crates/labs/julibrot/kernels/src/refinement.rs:236`, `crates/labs/julibrot/kernels/src/refinement.rs:261`).

For the same view, PictureFast Preview is code-derived as 120 by 68 at cap 32, or 261,120 pixel-iterations and 130,560 bytes, followed directly by the same Final workload; GPU execution wall time for every level is unmeasured and requires the browser (`crates/labs/julibrot/kernels/src/refinement.rs:64`, `crates/labs/julibrot/kernels/src/refinement.rs:261`).

Native planning over 10,000 calls per level measured 5,815 us, 4,538 us, and 9,786 us before allocation removal for Preview, Interactive, and Final, versus 3 us for each allocation-free form; the former created at least five allocations per plan and the latter zero, with `RUN-REPORT exit=0 wall=0.3s` (`crates/labs/julibrot/kernels/src/refinement.rs:236`).

Each level reserves one whole 8,388,608-byte final-size output span even when it writes only a Preview or Interactive prefix, and a paired grid reserves 16,777,216 bytes; these are code-derived capacities, not wall measurements (`crates/labs/julibrot/kernels/src/gpu.rs:145`, `crates/labs/julibrot/kernels/src/gpu.rs:224`, `crates/labs/julibrot/kernels/src/refinement.rs:261`).

At 960 by 540, page splitting is code-derived as one dispatch page and two copy commands for Preview, two pages and three copy commands for Interactive, and eight pages and eight copy commands for Final (`crates/labs/julibrot/kernels/src/refinement.rs:261`, `crates/labs/julibrot/kernels/src/gpu.rs:681`).

Each page resolves and compares the entire static header block before encoding, so the 2,048-byte Julibrot header is compared eight times during the code-derived Final plan; header reserve upload occurs only on a difference (`crates/labs/heap/src/executor.rs:802`, `crates/labs/heap/src/executor.rs:957`).

Descriptor-table and span-directory synchronization each call a packing routine that allocates a fresh word vector when metadata is dirty; the app config makes their code-derived uploads 1,024 bytes and 768 bytes respectively, and browser wall time is unmeasured (`crates/labs/heap/src/heap.rs:485`, `crates/labs/heap/src/span.rs:287`, `crates/labs/heap/src/executor.rs:1009`, `crates/labs/julibrot/app/src/frame/loop.rs:418`).

Every encoded page allocates a texture-view vector and an optional-attachment vector and creates a scratch texture view, making the code-derived Final overhead 16 heap allocations and eight scratch views before command submission; browser wall time is unmeasured (`crates/labs/heap/src/executor.rs:1053`, `crates/labs/heap/src/executor.rs:1155`).

Scratch landing copies the logical row range into a 512 by 512 by four-layer, 16-byte-pixel scratch texture whose configured capacity is code-derived as 16,777,216 bytes; copy count scales with page count, while GPU copy and fence wall times are unmeasured (`crates/labs/heap/src/executor.rs:1053`, `crates/labs/julibrot/app/src/frame/loop.rs:418`).

## 2. Fast-slide behavior

Zoom, pan, origin, centre, iteration-cap, and precision changes navigate and release the current reference; object-angle edits navigate only when they change the active plane, while an in-plane reorientation does not; palette and view-only changes do not navigate (`crates/labs/julibrot/app/src/state.rs:542`, `crates/labs/julibrot/app/src/state.rs:577`, `crates/labs/julibrot/app/src/state.rs:796`, `crates/labs/julibrot/app/src/state.rs:866`, `crates/labs/julibrot/app/src/state.rs:926`, `crates/labs/julibrot/app/src/state.rs:952`, `crates/labs/julibrot/app/src/state.rs:977`, `crates/labs/julibrot/app/src/state.rs:1000`, `crates/labs/julibrot/app/src/state.rs:1015`).

The owner admits at most one active request and retains only the latest pending request, so an arbitrarily long burst produces one in-flight request plus one coalesced successor rather than an unbounded request queue (`crates/labs/julibrot/worker/src/owner.rs:372`, `crates/labs/julibrot/worker/src/owner.rs:461`).

The worker-side pending slot is also latest-wins, the arrival endpoint has two FIFO slots, and the application drains at most those two arrivals on a service pass; the four-buffer pool therefore bounds owned transfer memory (`crates/labs/julibrot/worker/src/browser.rs:59`, `crates/labs/julibrot/worker/src/endpoint.rs:261`, `crates/labs/julibrot/worker/src/endpoint.rs:318`, `crates/labs/julibrot/worker/src/endpoint.rs:536`, `crates/labs/julibrot/worker/src/endpoint.rs:583`).

Generation checks occur before each orbit chunk, the chunk is at most 64 iterations, and the elapsed-time check runs every eight iterations against a 2,000 us target, so ordinary stale work yields cooperatively; the browser distribution of individual high-precision steps and actual yield intervals is unmeasured (`crates/labs/julibrot/worker/src/compute.rs:11`, `crates/labs/julibrot/worker/src/compute.rs:148`).

Cancelled or stale responses are still published or drained so their buffers and credits return, and the application accepts only the exact current generation and pending identity before upload (`crates/labs/julibrot/worker/src/browser.rs:738`, `crates/labs/julibrot/worker/src/endpoint.rs:627`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:149`).

Every wait on this side has a guard: worker yielding uses a zero-delay timer rather than a blocking wait, admission expires in at most one second per attempt, endpoint resize and shutdown drains expire after four seconds, and application service uses nonblocking pumps; these guards prevent an internal unbounded wait by code review, while browser event-loop and device-completion behavior remain unmeasured (`crates/labs/julibrot/worker/src/browser.rs:964`, `crates/labs/julibrot/worker/src/credit.rs:153`, `crates/labs/julibrot/worker/src/endpoint.rs:449`, `crates/labs/julibrot/worker/src/endpoint.rs:595`).

FIFO draining and exact-generation acceptance prevent an older orbit from being installed after a newer orbit by code review, but a fast navigation resets census progress and no candidate accompanies the new navigation, so repeated motion can indefinitely postpone secondary glitch correction without hanging the main request path (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:234`, `docs/plans/backlog.md:332`).

Draining 20,000,000 synthetic arrivals measured 81,690 us with shifting-vector removal and 42,863 us with a fixed queue, with `RUN-REPORT exit=0 wall=0.4s`; current production uses the fixed bounded endpoint storage, so this result confirms the bounded shape rather than identifying a current unbounded backlog (`crates/labs/julibrot/worker/src/endpoint.rs:261`).

A 10,000-edit native navigation sequence at zoom 100 and width 1,024 measured 4,783.333 ms for deterministic recomputation and 4,644.672 ms for PictureFast, saving 138.661 ms while accumulating 0.001557775202 pixel final error, or 0.0000001557775202 pixel per edit, with `RUN-REPORT exit=0 wall=9.6s` (`crates/labs/julibrot/math/src/big.rs:22`, `crates/labs/julibrot/worker/src/owner.rs:304`).

## 3. Open-line disposition

PF-R4 remains real: the explicit boundary measurement constructs one synthetic record at max iteration one, while the four-case mixed corpus reports no boundary samples, so no boundary corpus yet spans production orbit records, later escapes, rebases, and both rescale directions (`docs/plans/backlog.md:270`, `crates/labs/julibrot/kernels/src/perturb.rs:719`, `crates/labs/julibrot/kernels/src/perturb.rs:781`).

PF-R6 remains real and browser-dependent: the app expands eight-byte orbit records into 16-byte heap texels and WGSL reads one point per texel, but no GPU-completion measurement compares this with the proposed two-points-per-texel accessor (`docs/plans/backlog.md:272`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:169`, `crates/labs/julibrot/kernels/src/perturb.wgsl:128`).

PF-R8 remains real: non-orbit headers clear the entire maximum message and returned request validation scans the unused suffix, making the return path proportional to maximum capacity; browser wall time is unmeasured (`docs/plans/backlog.md:274`, `crates/labs/julibrot/worker/src/browser.rs:228`, `crates/labs/julibrot/worker/src/browser.rs:428`, `crates/labs/julibrot/worker/src/wire.rs:676`).

PF-R9 remains real: orbit payload packing uses eight per-byte indexed calls per iteration instead of a bulk copy, with 32,768 calls at cap 4,096 and browser cost unmeasured (`docs/plans/backlog.md:275`, `crates/labs/julibrot/worker/src/browser.rs:366`, `crates/labs/julibrot/worker/src/browser.rs:981`).

PF-V6 remains real: the executor resolves and compares the complete resident and dispatch header set for every page instead of validating it once per selected dispatch (`docs/plans/backlog.md:304`, `crates/labs/heap/src/executor.rs:957`).

PF-V7 remains real: resource synchronization repacks fresh descriptor and span vectors and uploads both complete metadata tables whenever reference resource words change (`docs/plans/backlog.md:305`, `crates/labs/heap/src/executor.rs:885`, `crates/labs/heap/src/executor.rs:1009`).

PL-02 is fixed by implementation commit 3786bd77 and merge commit 9d76ea2f: owner policy selects lower working widths under a bounded edit budget, and tests pin the 64-, 128-, and 576-bit choices (`docs/plans/backlog.md:263`, `crates/labs/julibrot/math/src/big.rs:554`, `crates/labs/julibrot/worker/src/owner.rs:790`).

PL-06 is fixed by implementation commit 3786bd77 and merge commit 9d76ea2f: PictureFast has a distinct low-resolution Preview and bypasses Interactive (`docs/plans/backlog.md:276`, `crates/labs/julibrot/kernels/src/refinement.rs:64`, `crates/labs/julibrot/kernels/src/refinement.rs:114`).

PL-08 is not fixed despite implementation commit c886716e and merge commit 040dfb7e: the purported PictureFast conformance renderer calls the deterministic renderer directly, so its green comparison cannot exercise the fast policy, and the special-value counterpart does the same (`docs/plans/backlog.md:278`, `crates/labs/julibrot/app/tests/final_conformance.rs:384`, `crates/labs/julibrot/app/tests/final_conformance.rs:489`).

PL-10 remains real and needs the browser: no stable in-browser receipt decomposes a representative slide into reference work, transfer, packing, upload, dispatch, and fence walls (`docs/plans/backlog.md:280`, `crates/labs/julibrot/app/src/timing.rs:92`).

PL-11 is fixed by implementation commit 3786bd77 and merge commit 9d76ea2f: the shallow branch neither requests nor transfers an orbit and does not enter orbit-response credit gating (`docs/plans/backlog.md:281`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:281`).

Requested backlog 253, baseline backlog line 255, is fixed as a safety bound but remains a possible visible delay: pricing is clamped and admission waits are timed, while depleted waits measured 1,228 us at cap 512 and 10,272 us at cap 4,096 with `RUN-REPORT exit=0 wall=11.2s` (`docs/plans/backlog.md:255`, `crates/labs/julibrot/worker/src/credit.rs:153`).

Requested backlog 256, baseline backlog line 258, remains real and browser-dependent: the exact 2,000 ms hold with one poll before and one after has no reproducible browser receipt (`docs/plans/backlog.md:258`, `crates/labs/julibrot/app/src/timing.rs:114`).

Requested backlog 258, baseline backlog line 260, remains undecided without the browser build environment: bundle reproducibility cannot be tested because this lane neither rebuilds nor changes the WebAssembly bundle (`docs/plans/backlog.md:260`).

Requested backlog 286, baseline backlog line 288, remains real: the shared plane-axis constants are still part of request state rather than a separately versioned immutable contract, and changing that layout is frozen in this lane (`docs/plans/backlog.md:288`, `crates/labs/julibrot/worker/src/owner.rs:26`, `crates/labs/julibrot/worker/src/codec.rs:16`).

Requested backlog 304, baseline backlog line 306, is fixed by implementation commits f1963e64 and e6facb72 and merge commit bcdfe38c: native request encoding and the browser bridge use the same visitor rather than duplicating offsets (`docs/plans/backlog.md:306`, `crates/labs/julibrot/worker/src/codec.rs:405`, `crates/labs/julibrot/worker/src/browser.rs:523`).

Requested backlog 308, baseline backlog line 310, remains real outside the writable scope: backdrop reuse keys requested extent, cap, and mode but not the derived delivered extent, allowing stale reuse after a derivation change (`docs/plans/backlog.md:310`, `crates/labs/julibrot/app/src/frame/loop/browser/backdrop.rs:64`).

Requested backlog 309, baseline backlog line 311, is structurally real but fail-stopped rather than silently visible: a retired transient grid can remain referenced, while the typed error terminates the frame path instead of presenting it (`docs/plans/backlog.md:311`, `crates/labs/julibrot/app/src/frame/loop/browser/submit.rs:260`, `crates/labs/julibrot/app/src/frame/loop/browser/submit.rs:294`).

Requested backlog 331, baseline backlog line 333, remains real: accumulated cancellation error is measured in test-only code but production detection uses only the Pauldelbrot single-step test (`docs/plans/backlog.md:333`, `crates/labs/julibrot/kernels/src/perturb.rs:298`, `crates/labs/julibrot/kernels/src/perturb.rs:330`, `crates/labs/julibrot/kernels/src/perturb.wgsl:247`).

Requested backlog 329, baseline backlog line 331, remains real: the shipped rebase path records glitches and census candidates but does not perform regional secondary-reference correction (`docs/plans/backlog.md:331`, `crates/labs/julibrot/kernels/src/perturb.wgsl:247`, `crates/labs/julibrot/kernels/src/perturb.rs:254`).

Requested backlog 330, baseline backlog line 332, remains real: every navigation releases the accepted reference and resets the search, and no compatible census candidate is reprojected into the next request (`docs/plans/backlog.md:332`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:234`).

## 4. Correctness under speed

The CPU oracle propagates an explicit f64 error envelope through perturbation, rescale, and rebase, whereas production WGSL evaluates the scaled perturbation and the Pauldelbrot single-step glitch predicate without carrying that envelope (`crates/labs/julibrot/math/src/perturb.rs:84`, `crates/labs/julibrot/math/src/perturb.rs:160`, `crates/labs/julibrot/kernels/src/perturb.wgsl:191`, `crates/labs/julibrot/kernels/src/perturb.wgsl:247`).

The envelope boundary fixture measured maximum norm error 0.000015258788835 against an envelope of 0.000045776369689, a tightness ratio of 3.0000002086, with `RUN-REPORT exit=0 wall=0.2s`; a mixed rescale-and-rebase corpus measured four cases, zero boundary failures, and zero violations with `RUN-REPORT exit=0 wall=0.3s` (`crates/labs/julibrot/math/src/perturb.rs:84`).

The WGSL rebase switches the perturbation origin to the current reference sample when the single-step condition fires, and records numeric or exhausted-reference glitches as distinct negative codes; a changed main reference cannot splice into an already dispatched grid because generation and accepted-reference guards reject mismatches (`crates/labs/julibrot/kernels/src/perturb.wgsl:247`, `crates/labs/julibrot/kernels/src/perturb.wgsl:254`, `crates/labs/julibrot/kernels/src/perturb.rs:254`, `crates/labs/julibrot/kernels/src/gpu.rs:476`).

The f32/deep switch is explicit: the shallow configuration carries no orbit, while deep configuration is blocked until the exact generation and precision policy are accepted (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:281`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:297`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:149`).

ABI fixtures measured 16 cases, 20,464 records, 40,928 coordinates, and zero changes to consumed high words with `RUN-REPORT exit=0 wall=2.6s`, supporting the current low-word WGSL consumption rather than proving future high-word use (`crates/labs/julibrot/kernels/src/perturb.wgsl:128`).

The accumulated-error experiment measured 518,400 samples: at threshold 0.0001 it flagged 3,871, including 2,547 wrong escapes, 408 verified-correct escapes, and 916 no-escape cases; at 0.001 it flagged 471, including 347 wrong, two verified-correct, and 122 no-escape cases; at 0.01 it flagged 58, including 23 wrong and 35 no-escape cases, with `RUN-REPORT exit=0 wall=33.5s` (`crates/labs/julibrot/kernels/src/perturb.rs:298`).

At threshold 0.001 the corrected-final experiment flagged 3,071 samples with 2,140 old-result mismatches, 926 matches, and five glitches, while 57 of 1,024 sampled unflagged points were wrong; the measured true-positive fraction was 0.697977821 and sampled false-negative fraction 0.055664062, so this detector is not yet a safe value-preserving production change (`crates/labs/julibrot/kernels/src/perturb.rs:298`, measurement `RUN-REPORT exit=0 wall=33.5s`).

The same native run measured 2,115,025 us for the baseline detector and 2,075,724 us for accumulated detection over 100,000 rounds with 27 rebases per round, a noisy negative delta of 14.556 ns per rebase, and measured the Pauldelbrot path at 3,456,316 us for 102.4 million iterations, or 33,753 ps per iteration; browser GPU cost is unmeasured (`crates/labs/julibrot/kernels/src/perturb.rs:298`, `RUN-REPORT exit=0 wall=33.5s`).

The production receipt records generation, precision bits, precision mode, orbit, and axis constants, but application acceptance does not retain `reference_verification`, maximum consumed-word error, or precision escalation facts exposed by the response (`crates/labs/julibrot/worker/src/owner.rs:26`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:182`, `crates/labs/julibrot/worker/src/channel.rs:723`).

More seriously, ordinary PictureFast construction assigns `ReferencePass::Preview`, and a baseline source search found the only `with_precision_policy` call sites in a measurement harness and a codec test rather than application scheduling; with no application consumer of the response getters, PictureFast skips to Final with a deferred, unverified reference while state identifies only mode and bit width (`crates/labs/julibrot/worker/src/codec.rs:287`, `crates/labs/julibrot/worker/src/compute.rs:399`, `crates/labs/julibrot/worker/src/codec.rs:673`, `crates/labs/julibrot/worker/src/channel.rs:723`, `crates/labs/julibrot/kernels/src/refinement.rs:64`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:182`).

Three native Final conformance tests reported green with `RUN-REPORT exit=0 wall=0.4s`, but review establishes that the fast renderer delegates to the exact renderer, so the result proves the deterministic oracle is self-consistent and does not prove PictureFast Final conformance (`crates/labs/julibrot/app/tests/final_conformance.rs:384`, `crates/labs/julibrot/app/tests/final_conformance.rs:489`).

Apart from that precision-receipt gap, exact generation checks at arrival, GPU reference acceptance, scene readiness, and the view stamp prevent a completed grid for an old plane, reference, centre revision, cap, or precision mode from being submitted as the current scene by code review; the complete rapid-slider browser sequence remains unmeasured (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:149`, `crates/labs/julibrot/kernels/src/gpu.rs:476`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:347`, `crates/labs/julibrot/app/src/frame/loop.rs:446`).

## 5. Tile readiness

The tile design requires per-tile source-screen regions as jobs, one shared and pinned reference orbit per MAIN generation, paired value and reconstruction records, transactional refinement in which value and depth complete together, a demand-ordered queue, and bounded resident budgets (`docs/julibrot/tiled-reprojection.md:71`, `docs/julibrot/tiled-reprojection.md:141`, `docs/julibrot/tiled-reprojection.md:227`, `docs/julibrot/tiled-reprojection.md:233`, `docs/julibrot/tiled-reprojection.md:241`, `docs/julibrot/tiled-reprojection.md:249`).

Useful foundations already exist in the heap's bounded descriptor and span directories, paired contiguous span allocation, logical-page dispatch, explicit reference generation identity, and latest-wins worker admission (`crates/labs/heap/src/heap.rs:485`, `crates/labs/heap/src/span.rs:287`, `crates/labs/julibrot/kernels/src/gpu.rs:224`, `crates/labs/julibrot/kernels/src/gpu.rs:681`, `crates/labs/julibrot/worker/src/owner.rs:372`).

The present compute shape is wrong for tiles because a refinement owns a whole-screen final-size reservation, the kernel emits one value record rather than a value-and-reconstruction pair, refinement is a serial global ladder, worker admission describes reference-orbit requests rather than tile jobs, and the two-entry reference-template cache is not a pinned generation lease (`crates/labs/julibrot/kernels/src/gpu.rs:145`, `crates/labs/julibrot/kernels/src/refinement.rs:114`, `crates/labs/julibrot/worker/src/codec.rs:16`, `crates/labs/julibrot/kernels/src/gpu.rs:606`).

The smallest stage-0 foundation is a wire-free, value-preserving job model in math and kernels containing content, MAIN, and reference identities, a source-screen rectangle, refinement rung, demand key, paired output-span plan, and a reference lease, plus pure validation and budget arithmetic; this matches the design's policy-and-math stage without changing request layouts, kernels, or delivered values (`docs/julibrot/tiled-reprojection.md:227`, `docs/julibrot/tiled-reprojection.md:349`).

That foundation is estimated at two to three engineering days with low runtime risk and medium API-shape risk; implementation is intentionally absent from this audit because the tile design belongs to the next work (`docs/julibrot/tiled-reprojection.md:349`).

## 6. Ranked findings and recommendations

### Critical

- C1 — PictureFast Final lacks enforced verification (`crates/labs/julibrot/worker/src/codec.rs:287`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:182`): a deep PictureFast slide accepts a Preview-pass orbit, proceeds to Final, and exposes no verification receipt, so the user can see an incorrectly certified Final grid or a deep-zoom artifact; the fix shape is an explicit Final reference pass before Final dispatch plus retention and validation of the existing verification facts, with medium correctness and scheduling risk, tile-prep relevance through generation leases, and an estimate of one to two days.

- C2 — Final conformance gives a false green (`crates/labs/julibrot/app/tests/final_conformance.rs:384`, `crates/labs/julibrot/app/tests/final_conformance.rs:489`): an implementation regression in PictureFast can ship because the test's fast side invokes the deterministic renderer, leaving the user exposed to wrong Final values; the fix shape is independently executing the real Preview and Final precision policies against the oracle, with low product risk, direct support for C1 rather than tile prep, and an estimate of one day.

### High

- H1 — Orbit transfer uses per-byte JavaScript setters (`crates/labs/julibrot/worker/src/browser.rs:366`, `crates/labs/julibrot/worker/src/browser.rs:981`): cap 4,096 produces 32,768 setter calls per orbit, so rapid deep changes can make completed references arrive late and leave the scene on stale reprojection longer; the fix shape is a validated bulk typed-array copy preserving the wire bytes, with medium browser interoperability risk, tile-prep relevance for shared-reference delivery, and an estimate of one day plus browser measurements.

- H2 — Request return performs maximum-capacity clearing and validation (`crates/labs/julibrot/worker/src/browser.rs:228`, `crates/labs/julibrot/worker/src/browser.rs:428`, `crates/labs/julibrot/worker/src/wire.rs:676`): every small request returned from a cap-4,096 buffer can clear and scan roughly 32 KiB during a burst, producing avoidable worker-main-thread pressure and slider lag; the fix shape is message-kind-aware used-range clearing and validation with retained-tail invariants, with medium stale-byte safety risk, tile-prep relevance to bounded queues, and an estimate of one to two days plus browser tests.

- H3 — Reference upload expands into a square heap page (`crates/labs/heap/src/executor.rs:726`, `crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:169`): a cap-4,096 orbit has 65,536 logical bytes after expansion but allocates and uploads a 1,048,576-byte 256-square staging vector, a code-derived 16-fold amplification that can delay each accepted deep reference; the fix shape is a row-exact or padded-row upload path proven against WebGPU layout constraints, with medium backend risk, strong tile-prep relevance, and an estimate of two days including browser validation.

- H4 — Census work is discarded across navigation (`crates/labs/julibrot/app/src/frame/loop/browser/reference.rs:234`, `docs/plans/backlog.md:332`): repeated motion resets candidate progress before a secondary reference can be selected, so a persistent glitch can remain visible throughout a slide; the fix shape is carrying a generation-independent candidate observation into the next compatible MAIN reference request, with high semantic risk around plane compatibility, direct tile-prep relevance, and an estimate of two to three days.

- H5 — Accumulated cancellation error is not a production detector (`crates/labs/julibrot/kernels/src/perturb.rs:298`, `crates/labs/julibrot/kernels/src/perturb.wgsl:247`): a point can accumulate enough perturbation error to return a wrong escape without tripping the single-step predicate, so deep motion can expose isolated wrong pixels; the fix shape is a conservative measured detector followed by regional secondary-reference correction, with high numerical and performance risk, direct tile-prep relevance, and an estimate of three to five days after oracle tuning.

### Medium

- M1 — Several walls are invisible (`crates/labs/julibrot/app/src/timing.rs:92`, `crates/labs/julibrot/app/src/timing.rs:114`): a two-second slider hold can miss transfer, upload, dispatch, or fence delay and report no causal stage, leaving stalls unactionable; the fix shape is a browser receipt with monotonic marks across every handoff, with low runtime risk, tile-prep relevance for per-tile service budgets, and an estimate of one to two days.

- M2 — Final planning repeats whole-header comparisons (`crates/labs/heap/src/executor.rs:957`, `crates/labs/julibrot/kernels/src/gpu.rs:681`): an eight-page Final compares the same 2,048-byte header eight times, adding avoidable CPU work to every completed scene; the fix shape is plan-level comparison and one dirty decision, with low correctness risk, tile-prep relevance for many small jobs, and an estimate of one day.

- M3 — Metadata synchronization allocates packed vectors (`crates/labs/heap/src/heap.rs:485`, `crates/labs/heap/src/span.rs:287`, `crates/labs/heap/src/executor.rs:1009`): descriptor or span dirtiness allocates and repacks before dispatch, producing allocator pressure as resources churn; the fix shape is reusable packing scratch or direct encoding, with low-to-medium lifetime risk, strong tile-prep relevance, and an estimate of one to two days.

- M4 — Page encoding allocates transient collections and scratch views (`crates/labs/heap/src/executor.rs:1053`, `crates/labs/heap/src/executor.rs:1155`): an eight-page Final creates at least 16 small vectors and eight views, which can amplify CPU jitter during fast refinement; the fix shape is retained page scratch with bounded capacities, with medium borrow and lifetime risk, strong tile-prep relevance, and an estimate of two days.

- M5 — Reference construction grows record vectors incrementally (`crates/labs/julibrot/math/src/orbit.rs:192`, `crates/labs/julibrot/math/src/orbit.rs:239`): cap-4,096 orbit construction can reallocate while the user waits, and a verification pair doubles that pressure; the fix shape is exact preallocation for the known cap with a native allocation pin, with low risk, shared-reference tile-prep relevance, and an estimate of half a day.

- M6 — Credit safety can still add visible delay (`crates/labs/julibrot/worker/src/credit.rs:153`): a depleted balance made the measured cap-4,096 request wait 10,272 us, and repeated stale results can defer the newest request even though each wait is bounded; the fix shape is generation-aware refund and admission accounting without bypassing backpressure, with medium fairness risk, tile-prep relevance for demand ordering, and an estimate of one to two days.

- M7 — PictureFast Final retains the full requested cap (`crates/labs/julibrot/kernels/src/refinement.rs:64`): a region whose census predicts earlier settlement still runs the full requested Final cap, extending completed-scene latency; the fix shape is a receipt-backed conservative cap policy with exact fallback, with high correctness risk, tile-prep relevance to rung planning, and an estimate of two to four days plus browser validation.

### Low

- L1 — Backdrop cache identity omits delivered extent (`crates/labs/julibrot/app/src/frame/loop/browser/backdrop.rs:64`): a derivation change can reuse a backdrop with incompatible extent and show an unexpected stale region; the fix shape is including delivered extent in the cache key, with low risk, no compute-lane implementation authority, and an estimate of half a day.

- L2 — Retired transient grids remain representable (`crates/labs/julibrot/app/src/frame/loop/browser/submit.rs:260`, `crates/labs/julibrot/app/src/frame/loop/browser/submit.rs:294`): a retirement race stops the frame with a typed error rather than flashing an invalid grid, so the visible result is a stalled scene; the fix shape is atomically forgetting presentation state before freeing the grid, with low risk, tile-prep relevance to transactional retirement, and an estimate of half a day.

- L3 — Plane-axis constants remain request data (`crates/labs/julibrot/worker/src/owner.rs:26`, `crates/labs/julibrot/worker/src/codec.rs:16`): an axis-contract mismatch would compute a different plane than the caller expects, although current generation guards cannot detect semantic disagreement; the fix shape is a versioned immutable axis contract in a future wire revision, with medium compatibility risk, tile-prep relevance to job identity, and an estimate of one day after the layout freeze.

- L4 — Bundle determinism is unresolved (`docs/plans/backlog.md:260`): a toolchain-dependent bundle could alter browser behavior without a source diff, but this lane cannot reproduce the build by rule; the fix shape is a separately authorized reproducible bundle check, with low source risk, no tile-prep role, and an estimate of one day in the browser lane.

### Recommendations

1. Enforcement and retention of PictureFast Final verification, paired with a non-delegating conformance oracle that can fail independently, is the first recommended lane (`crates/labs/julibrot/worker/src/codec.rs:287`, `crates/labs/julibrot/app/tests/final_conformance.rs:384`).

2. Browser wall capture for an exact 2,000 ms hold and a rapid-change burst is the second recommended lane, covering worker, credit, packing, upload, dispatch, fence, and completion before transfer optimizations are selected (`docs/plans/backlog.md:258`, `crates/labs/julibrot/app/src/timing.rs:92`).

3. Byte-identical bulk orbit packing and used-range request-return work backed by browser invariants form the third recommended lane (`crates/labs/julibrot/worker/src/browser.rs:228`, `crates/labs/julibrot/worker/src/browser.rs:981`).

4. A wire-free stage-0 tile job, paired-output plan, reference lease, ordering key, and budget arithmetic form the fourth recommended lane, preceding any dispatch or value change (`docs/julibrot/tiled-reprojection.md:227`, `docs/julibrot/tiled-reprojection.md:349`).

5. Removal of heap-side upload and dispatch amplification is the fifth recommended lane, ordered as reference staging, per-page header comparison, metadata packing allocations, and page scratch allocations (`crates/labs/heap/src/executor.rs:726`, `crates/labs/heap/src/executor.rs:957`, `crates/labs/heap/src/executor.rs:1009`, `crates/labs/heap/src/executor.rs:1053`).

6. Compatible census carry across navigation and accumulated-error correction after detector tuning form the sixth recommended lane, with the observed false-negative class as its entry criterion (`docs/plans/backlog.md:332`, `docs/plans/backlog.md:333`).
