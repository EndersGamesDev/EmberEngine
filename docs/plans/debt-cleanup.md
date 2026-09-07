# Technical-debt inventory

Measured at main `61acab7b5a5b1f7e8defd38e7ec48e056da0611d`. This inventory excludes `crates/labs/julibrot/`, `web/labs/julibrot/`, `docs/julibrot/`, the three Julibrot sections of `docs/plans/backlog.md`, `docs/arena-and-world.md`, `local/`, and unrelated `web/games.json` entries.

The source census used anchored Rust-attribute searches, lexical marker searches followed by notice-versus-fixture classification, direct inspection of every network integration test containing a sleep, manifest-to-use-site comparison, lockfile package and reverse-edge parsing, reference searches for candidate artifacts and scripts, and inspection of all 205 backlog rows outside the three excluded sections. No local Rust compiler or Rust formatter was invoked.

## Summary

|Class|Measured count|Highest-value bite|
|-----|-------------:|------------------|
|Gate red on main|22 target diagnostics at 12 unique sites; 125 formatter hunks across 6 files|Fix `league-server` Clippy, then apply the formatter-of-record patch|
|Flaky tests|2 unwaited cross-client reads and 16 wall-clock sleeps in test code|Add the missing Kings wait, preserving the assertion|
|Lint allows|364 repository-wide attributes: 28 removable, 248 justified, and 88 hiding defects|Remove the 28 stale attributes, then split or justify the remaining active suppressions|
|Marker text|87 lexical occurrences on 81 lines; 16 are open notices and 71 are linter grammar/documentation fixtures|Correct the converter-script backlog row; no new backlog row is needed|
|Dead code and stale artifacts|15 dead-code suppressions, 3 unreferenced binary assets, 1 definitively stale script, 4 duplicated-helper families, and 1 catalog metadata defect|Delete the three unreferenced assets or record their deliberate archival role|
|Backlog hygiene|205 in-scope rows; 1 obsolete, 0 exact duplicates, 3 materially stale descriptions|Remove the satisfied formatter row and correct the three stale descriptions|
|Dependency debt|44 names at multiple locked versions and 4 dependencies declared for targets that do not use them|Move the four dependencies into their wasm target tables|

## A. Gate red on main

### Clippy

The supplied `cargo clippy --workspace --all-targets` run reports 10 errors for the `league-server` library and 12 for the library test target. The test target repeats the ten library diagnostics and adds two in `src/tests.rs`, so this is 22 target diagnostics at 12 unique source sites.

|Target count|Site|Diagnostic|Exact bite|Oracle|
|-----------:|----|----------|----------|------|
|4|`crates/league-server/src/lib.rs:99-100`|`run` has a redundant `must_use` and lacks an Errors section|Remove the redundant attribute and document the I/O failure contract|`cargo clippy -p league-server --all-targets` plus the workspace gate|
|2|`crates/league-server/src/lib.rs:100`|`TcpListener` is passed by value without being consumed|Take `&TcpListener` and update its public call at `crates/league-server/src/main.rs:65`, retaining the same `incoming()` loop|`cargo test -p league-server` plus the workspace gate|
|2|`crates/league-server/src/lib.rs:305`|Float comparison in the fixed-step `while`|Express the identical compare/subtract loop as `loop` plus an early break|`cargo test -p league-server` and deterministic `league-core` tests|
|2|`crates/league-server/src/lib.rs:403`|Clone assignment|Use `clone_from` for the handle|Workspace gate|
|2|`crates/league-server/src/lib.rs:458-627`|`handle_msg` is 170 lines|Extract message-arm helpers without changing arm order or early returns|`cargo test -p league-server` plus workspace gate|
|4|`crates/league-server/src/lib.rs:481-482`|Two `usize`\-to-`u32` truncating casts|Use checked or saturating conversions for player and lobby counts|Server tests that decode Welcome plus workspace gate|
|2|`crates/league-server/src/lib.rs:731-837`|`join_lobby` is 107 lines|Extract password validation and join-state emission helpers|`cargo test -p league-server` plus workspace gate|
|2|`crates/league-server/src/lib.rs:751-762`|Nested password check is collapsible|Collapse the condition mechanically|Workspace gate|
|1|`crates/league-server/src/tests.rs:403`|Unchecked subtraction from `Instant`|Use `checked_sub` while preserving the deliberately expired timestamp|`cargo test -p league-server`|
|1|`crates/league-server/src/tests.rs:435-567`|WebSocket test is 133 lines|Extract setup and assertion helpers; keep one ordered scenario|`cargo test -p league-server`|

The single preferred bite touches `crates/league-server/src/lib.rs`, `crates/league-server/src/main.rs`, `crates/league-server/src/tests.rs`, and any example call sites found by the final reference search. It fixes all 12 unique sites in one crate-green commit because leaving half of them retains a red `league-server` target.

### Formatter

The `cargo-fmt --all -- --check` formatter-of-record run at the same base is authoritative and reports 125 hunks in six files: 2 at `crates/league/src/combat.rs:1459`, `:1487`; 11 spanning `crates/league/src/scene/art.rs:79-234`; 105 spanning `crates/league/src/scene.rs:159-1875`; 1 at `crates/league-core/src/kits.rs:84`; 2 at `crates/league-core/src/proto.rs:437`, `:450`; and 4 at `crates/league-core/src/sim.rs:2469`, `:2543`, `:2592`, `:2601`. These are layout-only rewrites, so the bite is to apply exactly those hunks; the oracle is `cargo-fmt --all -- --check`, with no behavioral test needed beyond the workspace gate.

Rebase note: on `3d2c1837`, `combat.rs` was already formatter-clean; regenerated formatter output comprised 120 hunks across five files: 1 in `kits.rs`, 2 in `proto.rs`, 4 in `sim.rs`, 101 in `scene.rs`, and 12 in `scene/art.rs`.

## B. Flaky and wall-clock tests

The cross-client audit covered `crates/fire/tests/online_e2e.rs`, `crates/kings/tests/online_e2e.rs`, `crates/fire-server/tests/ws_e2e.rs`, `crates/kings-server/tests/ws_e2e.rs`, and `crates/arena-server/tests/ws_e2e.rs`. Exactly two reads consume client B state after waiting only on client A.

- Known Kings defect: `create_and_join` waits for Ada's two-entry roster at `crates/kings/tests/online_e2e.rs:231`, then immediately asserts Bob's roster length at `:237` without pumping or waiting Bob. Bite: add `b.wait_for(... roster.len() == 2)` immediately before the unchanged assertion. Oracle: repeated `cargo test -p kings --test online_e2e two_clients_create_join_start_move_and_time_out` and the workspace gate.
- Additional Fire defect: `two_clients_see_each_other_move` waits only for Alice to enter Racing at `crates/fire/tests/online_e2e.rs:256`, then reads Bob's race state at `:281`; Bob is pumped only in Alice's failure arm at `:258`. Bite: wait for Bob's Racing state before reading his racer array, without loosening distance assertions. Oracle: repeated `cargo test -p fire --test online_e2e two_clients_see_each_other_move` and the workspace gate.

There are 16 wall-clock sleeps in test code.

- Five fixed server-readiness delays: `crates/fire/tests/online_e2e.rs:51`, `crates/kings/tests/online_e2e.rs:62`, `crates/fire-server/tests/ws_e2e.rs:41`, `:603`, and `crates/kings-server/tests/ws_e2e.rs:39`. Replace them with bounded connection retry or a readiness handshake; oracle: the five affected integration-test binaries repeated under load.
- Six bounded-poll backoffs: `crates/fire/tests/online_e2e.rs:66`, `:101` and `crates/kings/tests/online_e2e.rs:77`, `:110`, `:444`, `:485`. These are justified CPU backoffs inside deadline-based waits, not naked synchronization; keep them and put the reason at their helper definition. Oracle: existing tests.
- Three real-time simulation pacing sleeps: `crates/fire/tests/online_e2e.rs:194`, `:287` and `crates/arena-server/tests/ws_e2e.rs:850`. These test live server cadence and are justified until a controllable clock exists; keep and document the pacing contract. Oracle: existing integration tests.
- One deliberate four-second non-reader interval at `crates/fire-server/tests/ws_e2e.rs:417` reproduces queue pressure and is the behavior under test; keep it. Oracle: `a_slow_peer_still_gets_roster_updates`.
- One redundant disconnect delay at `crates/fire-server/tests/ws_e2e.rs:487` precedes an already bounded wait for `PlayerLeft`; remove it. Oracle: `a_disconnect_frees_the_slot_for_the_next_player` repeated under load.

## C. `allow` attributes

The anchored census finds 364 actual attributes outside the named exclusions: 245 in `crates/`, 97 under `games/`, and 22 in `tools/`. A raw unanchored search returns one extra string fixture at `tools/linter/src/index.rs:922`, which is not an attribute. Forty-five attributes currently carry an inline comment or `reason = ...`; 319 do not.

Classification codes are `J` for justified, `R` for removable because the lint no longer fires, and `D` for a real maintainability or type-boundary defect hidden by the suppression. The measured count is J 248, R 28, D 88. The native forced-lint run emitted 2,267 source diagnostics, 85 generated-warning summaries, and three standalone manifest unused-dependency diagnostics. The seven-crate wasm forced-lint run emitted 1,269 source diagnostics across 14 generated-warning summaries. The J-versus-D decision for every reproduced lint follows from the invariant or defect described below.

An attribute is removable only when every cfg containing it was compiled and the forced lint was absent in that exact lexical scope. Classify combined attributes per lint member; absence from an uncompiled target is inconclusive.

`J` covers bounded coordinate/index casts, deterministic floating-point formulas, exact-value tests, opt-in command output, public naming conventions, attributes with an existing specific reason, and active suppressions in frozen historical contract crates. Every J without an inline reason still needs its existing attribute amended with a precise `reason`, grouped by crate so the review remains tractable. `D` covers unreasoned line/argument/boolean complexity, broad crate-level API/doc suppression, oversized error values, needless ownership, cfg-only test helpers compiled into production, and redundant visibility. The remaining `games/**` attributes are J because changing their lint-era source defeats their historical-fixture role; their crate headers, rather than repeated attribute edits, should state that reason.

### Complete location census

- `crates/arena-core/src/freight_yard.rs`: J 310, J 607.
- `crates/arena-core/src/harbor.rs`: J 433.
- `crates/arena-core/src/lib.rs`: J 2, J 4.
- `crates/arena-core/src/parkour.rs`: D 57, D 173, D 237, J 626.
- `crates/arena-core/src/proto.rs`: D 289, D 744, J 1004.
- `crates/arena-core/src/shooter.rs`: D 605, J 834, J 1209, J 1236, J 1269, J 1570, J 1759, D 1936, D 2096, D 2400, D 2471, J 2743, J 2756, D 2906, D 2971, J 3897, J 6590.
- `crates/arena-core/src/sim.rs`: J 191.
- `crates/arena-server/build.rs`: R 11.
- `crates/arena-server/examples/wsbot.rs`: J 2, J 4, D 44.
- `crates/arena-server/src/lib.rs`: D 2, J 192, D 213, D 308, D 442, D 732.
- `crates/arena-server/tests/ws_e2e.rs`: J 15, D 350, D 1189.
- `crates/arena/src/contact.rs`: J 119.
- `crates/arena/src/feel.rs`: D 79, J 365, J 573, D 926, J 1524, J 1777, J 2245, J 2415, J 2820, J 2829.
- `crates/arena/src/grips.rs`: D 185, D 225, D 267, D 383.
- `crates/arena/src/harbor.rs`: J 12.
- `crates/arena/src/hud.rs`: J 101.
- `crates/arena/src/lib.rs`: J 2, J 4, D 251.
- `crates/arena/src/online.rs`: J 643, D 940, D 1573, J 1837, D 2003, D 2592, D 5557, D 6772, D 7763, R 8496, D 8640.
- `crates/arena/src/props.rs`: J 305, R 720.
- `crates/arena/src/rounds.rs`: J 481, J 512, J 523, J 581.
- `crates/arena/src/settings.rs`: J 330.
- `crates/arena/src/sound.rs`: R 15, R 25, D 332, D 406, J 418, J 425, J 431, R 571, D 801, J 1116, R 1324, R 1598, R 1711.
- `crates/arena/src/viewarms.rs`: R 133.
- `crates/ember-client-net/src/lib.rs`: R 8.
- `crates/ember-client-net/src/transport.rs`: D 199, D 400, D 477.
- `crates/ember-engine/src/app.rs`: J 2, D 207, J 1156.
- `crates/ember-engine/src/environment.rs`: J 108.
- `crates/ember-engine/src/lib.rs`: J 3.
- `crates/ember-engine/src/occlusion.rs`: J 172, J 192, J 243, J 381, J 384, J 391, J 734.
- `crates/ember-engine/src/overlay.rs`: J 12.
- `crates/ember-engine/src/puppet.rs`: D 112.
- `crates/ember-engine/src/renderer.rs`: J 13, D 577, D 1024, J 2193, J 2233.
- `crates/ember-engine/src/renderer_gpu_test.rs`: D 30, D 228, J 472, D 1110.
- `crates/ember-engine/src/rig.rs`: D 616, D 766.
- `crates/ember-legacy/src/lib.rs`: R 9, R 29.
- `crates/ember-net/src/outer.rs`: J 8, D 452.
- `crates/ember-server/src/capabilities.rs`: D 4.
- `crates/ember-server/src/connection.rs`: D 4, D 217, D 219.
- `crates/ember-server/src/fixture.rs`: D 4.
- `crates/ember-server/src/lib.rs`: R 10.
- `crates/ember-server/src/product.rs`: D 4.
- `crates/ember-server/src/registry.rs`: D 4.
- `crates/ember-server/src/runtime.rs`: D 837, D 1086, D 1740, D 1779.
- `crates/fire-core/examples/track_stats.rs`: J 3.
- `crates/fire-core/src/car.rs`: J 21, J 297.
- `crates/fire-core/src/lib.rs`: J 2, J 4.
- `crates/fire-core/src/sim.rs`: J 56, J 139, J 257.
- `crates/fire-core/src/track.rs`: J 82, J 319, J 342.
- `crates/fire-server/build.rs`: R 11.
- `crates/fire-server/examples/probe.rs`: J 18, D 34.
- `crates/fire-server/src/lib.rs`: D 168.
- `crates/fire/src/game.rs`: J 665.
- `crates/fire/src/lib.rs`: J 2.
- `crates/fire/src/texgen.rs`: J 21, J 29, J 37, J 49.
- `crates/kings-core/src/board.rs`: J 80, J 100, J 287, J 304, J 579, J 660, J 664.
- `crates/kings-server/build.rs`: R 8.
- `crates/kings-server/examples/probe.rs`: J 20, D 45.
- `crates/kings-server/src/lib.rs`: D 535.
- `crates/kings/src/game.rs`: J 292, J 496, J 546.
- `crates/kings/src/hotseat.rs`: J 143.
- `crates/kings/src/lib.rs`: J 2.
- `crates/kings/src/meshes.rs`: J 81, J 112, J 114.
- `crates/kings/src/online.rs`: J 17, J 179.
- `crates/labs/heap/src/executor.rs`: D 3.
- `crates/labs/heap/src/heap.rs`: J 4.
- `crates/labs/heap/src/kernels.rs`: D 4.
- `crates/labs/heap/src/lattice.rs`: J 132, J 156, J 165, J 186, J 213, J 280, J 310.
- `crates/labs/heap/src/lattice_gpu.rs`: D 3.
- `crates/labs/heap/src/mode_c.rs`: J 186, J 220, J 357.
- `crates/labs/heap/src/spike.rs`: J 86, D 270, D 505.
- `crates/labs/heap/src/wasm.rs`: J 4, D 300.
- `crates/labs/layer/src/compute.rs`: D 432, D 808.
- `crates/labs/layer/src/demo.rs`: D 219.
- `crates/labs/layer/src/geometry.rs`: J 284, J 407, J 425.
- `crates/league-core/src/ai.rs`: J 15.
- `crates/league-core/src/kits.rs`: J 32, J 825, D 1036, D 1082, J 1106.
- `crates/league-core/src/lib.rs`: J 4, J 5, J 6, D 11, J 16.
- `crates/league-core/src/sim.rs`: J 1405, J 1567, J 1699, J 1829, J 1956.
- `crates/league-core/tests/gameplay.rs`: J 1, J 704, J 760, J 922.
- `crates/league-server/examples/wsprobe.rs`: J 7.
- `crates/league/src/combat.rs`: J 13.
- `crates/league/src/game.rs`: J 65, J 266, J 345.
- `crates/league/src/online_game.rs`: R 104, J 205.
- `crates/league/src/scene.rs`: D 21, D 727, D 828, D 881, D 1021.
- `crates/league/src/world.rs`: J 177, J 203.
- `crates/what-is-this/src/kernels.rs`: J 4, J 6.
- `crates/what-is-this/src/lib.rs`: D 172.
- `crates/what-is-this/src/render_bar.rs`: D 134.
- `games/arena/v001/src/lib.rs`: J 5; `games/arena/v001/src/proto.rs`: R 157; `games/arena/v001/src/sim.rs`: J 193; `games/arena/v001/tests/hosted_contract.rs`: R 2.
- `games/arena/v002/src/shooter.rs`: J 47, J 73, J 152, J 176, J 189, J 380.
- `games/arena/v003/src/shooter.rs`: J 47, J 76, J 155, J 179, J 192, J 377, J 452.
- `games/arena/v004/src/shooter.rs`: J 52, J 81, J 166, J 191, J 204, J 399, J 421, J 511.
- `games/arena/v005/src/shooter.rs`: J 6, J 51, J 81, J 225, J 248, J 407, J 518.
- `games/arena/v006/src/shooter.rs`: J 6, J 55, J 85, J 244, J 281, J 457, J 569.
- `games/arena/v007/src/proto.rs`: R 53; `games/arena/v007/src/shooter.rs`: J 6, J 171, J 201, J 210, J 353, J 474, J 477, J 526, J 735.
- `games/arena/v008/src/proto.rs`: R 61; `games/arena/v008/src/shooter.rs`: J 6, J 259, J 286, J 318, J 342, J 493, J 648, J 700, J 1064.
- `games/arena/v009/src/proto.rs`: J 72; `games/arena/v009/src/shooter.rs`: J 6, J 259, J 286, J 318, J 342, J 493, J 648, J 700, J 1064.
- `games/arena/v010/src/proto.rs`: J 8; `games/arena/v010/src/shooter.rs`: J 5.
- `games/arena/v011/src/proto.rs`: J 8; `games/arena/v011/src/shooter.rs`: J 5.
- `games/arena/v012/src/adapter.rs`: J 180, J 758; `games/arena/v012/src/proto.rs`: J 135; `games/arena/v012/src/shooter.rs`: J 6, J 325, J 352, J 385, J 409, J 557, J 721, J 774, J 1239, J 2821.
- `games/fire/v001/src/car.rs`: J 21, J 297; `games/fire/v001/src/lib.rs`: J 2, J 4; `games/fire/v001/src/sim.rs`: J 56, J 139, J 257; `games/fire/v001/src/track.rs`: J 82, J 319, J 342.
- `games/what-is-this/v001/src/lib.rs`: R 9.
- `tools/cli-common/src/lib.rs`: D 624, J 792; `tools/cli-common/src/publication.rs`: J 159; `tools/cli-common/src/tests/mod.rs`: R 113; `tools/cli-common/tests/public_api.rs`: R 38.
- `tools/linter/src/control.rs`: D 814, J 1963; `tools/linter/src/declaration.rs`: J 1747; `tools/linter/src/finding.rs`: J 1489; `tools/linter/src/interchange.rs`: D 2856.
- `tools/linter/src/reference.rs`: R 611, J 617, R 633, R 751, J 1211; `tools/linter/src/report.rs`: J 550; `tools/linter/src/shape.rs`: J 139; `tools/linter/src/snapshot.rs`: J 656; `tools/linter/src/spdx.rs`: J 1018; `tools/linter/src/token.rs`: R 244.
- `tools/linter/tests/corpus.rs`: J 715; `tools/linter/tests/support/git.rs`: J 42.

The 28 R attributes are: build-script output suppressions at `crates/arena-server/build.rs:11`, `crates/fire-server/build.rs:11`, and `crates/kings-server/build.rs:8`; crate or item lints at `crates/league/src/online_game.rs:104`, `crates/ember-server/src/lib.rs:10`, `crates/ember-legacy/src/lib.rs:9`, `:29`, `crates/ember-client-net/src/lib.rs:8`, `crates/arena/src/online.rs:8496`, `props.rs:720`, `sound.rs:15`, `:25`, `:571`, `:1324`, `:1598`, `:1711`, and `viewarms.rs:133`; frozen-source lints at `games/what-is-this/v001/src/lib.rs:9`, `games/arena/v008/src/proto.rs:61`, `games/arena/v007/src/proto.rs:53`, `games/arena/v001/src/proto.rs:157`, and `tests/hosted_contract.rs:2`; and tool lints at `tools/linter/src/reference.rs:611`, `:633`, `:751`, `tools/linter/src/token.rs:244`, `tools/cli-common/src/tests/mod.rs:113`, and `tests/public_api.rs:38`. Oracle: remove these attributes in crate-sized commits and require a clean ordinary Clippy run plus the affected crate tests; frozen-source removals additionally retain the hosted-contract tests.

The `dead_code` on `pan_gains` at `crates/arena/src/sound.rs:418` is J, not removable: native uses it, while the seven-crate wasm forced-lint run reports that `pan_gains` is never used because the browser panner node implements that law. Its existing comment states that target-specific reason. The three unreasoned Arena sound D items at `sound.rs:332`, `:406`, and `:801` reproduce dead-code diagnostics on both applicable runs and are referenced only by tests at `:1985`, `crates/arena/src/feel.rs:2080`, and `sound.rs:1881`; gate them with `cfg(test)` instead of compiling production-only dead code. Oracle: `cargo test -p arena` and both native and wasm checks.

## D. TODO, FIXME, and XXX markers

The lexical count is 87 occurrences on 81 lines under `crates/`, `web/`, and `tools/`; all are under `tools/`. Sixteen are actual open notices and already map to two backlog rows, while 71 are the linter's marker vocabulary, parser examples, test fixtures, or policy prose and are not work notices.

- Six unfitted hand constants are at `tools/make_hands.py:65`, `:76`, `:82`, `:90`, `:96`, and `:106`; two first-run conversion notices are at `tools/9mm_convert.py:61`, `:116`. They map to `docs/plans/backlog.md:317`, but that row must stop calling `make_hands.py` superseded because `tools/make_assets.py:18` and `:49` import and call it. The remaining work is to delete the never-run `9mm_convert.py` or mark it as a retained draft, and to measure the six hand-fit constants when the helper is next run.
- Eight labeled vendored-linter notices are at `tools/linter/src/assembly.rs:147`, `:158`, `:468`, `tools/linter/src/carrier.rs:147`, `tools/linter/src/claim.rs:304`, `tools/linter/src/comment.rs:124`, `tools/linter/src/coverage.rs:65`, and `tools/linter/src/graph.rs:95`. They map exactly to `docs/plans/backlog.md:314`, which routes them upstream.
- The 71 non-notice occurrences are grouped as `tools/linter/src/fix.rs` 29, `src/todo.rs` 16, `tests/corpus.rs` 6, `src/tests/label_calculus.rs` 5, `src/tests/profile_writers.rs` 4, `src/tests/burn_and_ratchets.rs` 4, `src/constant.rs` 4, and one each in `src/plan.rs`, `src/code.rs`, and `adr/profiles.md`. These define or test marker recognition and require neither a backlog row nor an in-place fix.

No marker is trivial source cleanup: deleting or editing either converter changes an authoring tool, and the eight labeled notices belong to the vendored upstream. The bite is documentation-only: correct `docs/plans/backlog.md:317` after a separate decision on whether `tools/9mm_convert.py` remains a draft. Oracle: reference search and the linter repository tests if its vendored text changes later.

## E. Dead code, stale artifacts, catalog state, and duplicated helpers

### Dead code and cfg reachability

There are 15 dead-code attributes outside exclusions. Nine are removable because the forced runs emit no diagnostic in their scopes: the two CLI fixture structs at `tools/cli-common/tests/public_api.rs:38`, `tools/cli-common/src/tests/mod.rs:113`; four staged linter shapes at `tools/linter/src/token.rs:244`, `tools/linter/src/reference.rs:611`, `:633`, `:751`; and three wired Arena entries at `crates/arena/src/sound.rs:25`, `:1598`, `:1711`. Three are justified and reproduce only where their target or inclusion makes them unused: `tools/linter/src/reference.rs:617`, `tools/linter/tests/support/git.rs:42`, and `crates/arena/src/sound.rs:418`. The three Arena helpers at `sound.rs:332`, `:406`, and `:801` are real production-reachability defects and should become test-only as detailed above.

### Stale binary artifacts and scripts

- `assets/models/level-backdrop.glb` is 1,462,584 bytes and has no code reader; its only non-backlog references are the pipeline example at `docs/asset-pipeline.md:14`, the generator output at `tools/level_backdrop.py:20`, and a plan stating Arena stopped drawing it at `docs/plans/arena-v13-trench-city.md:136`.
- `assets/textures/floor_basalt.png` is 1,949,513 bytes and `assets/textures/wall_basalt.png` is 1,985,364 bytes. Reference search finds only historical prose at `crates/arena/src/online.rs:331`, `crates/arena/src/props.rs:45`, and `docs/plans/backlog.md:206`; no `include_bytes` or loader references either file.
- `tools/9mm_convert.py` identifies every primary constant as a guess at `:61`, says it has never run, and is named as the superseded predecessor at `docs/asset-pipeline.md:15`; it is the one definitively stale script.
- `tools/make_hands.py` is not stale as a module: `tools/make_assets.py:18` and `:49` use it as the sole hand geometry implementation. Its standalone `hands.glb` output is unused as stated at `tools/make_hands.py:31-38`, and its six fit notices remain authoring debt.

The artifact bite deletes the three unreferenced binary files and updates `docs/asset-pipeline.md` plus `docs/plans/backlog.md`; the oracle is a repository reference search, the workspace build proving no compile-time embedding, and unchanged checked-in shipped bundles. Script deletion should be a separate bite because documentation presents it as a worked example.

### Fire catalog

`web/games.json:277-289` lists both Fire v2 and v1. V2 is the served entry because it alone has `live: true` at `:280`, and `deploy/deploy-pages.sh:22`, `:148`, `:179`, `:237` selects and copies `games/fire/v2/`. V1 is not stale: it is explicitly archived at `web/games.json:288-289`, deployment comments preserve it at `deploy/deploy-pages.sh:29-31`, and `deploy/tests/test-pages.sh:95-100` asserts preservation.

The real stale catalog data is v2 itself: `web/games.json:281-283` says protocol 1 and the castle circuit, while `CHANGELOG.md:335-343` records that the published v2 is protocol 2 GT Circuit V2 and its source commit is absent from this repository. `crates/fire-core/src/proto.rs:22` remains protocol 1, so changing only catalog text would make main disagree with its buildable source. Bite: recover/integrate the published source before reconciling `web/games.json`, `crates/fire-core`, and release metadata; oracle: deploy page tests, Fire crate tests, exact protocol handshake, and published artifact hashes.

### Duplicated helper families

- Six transient-read predicates have diverged: complete implementations at `crates/fire-core/src/proto.rs:245`, `crates/kings-core/src/proto.rs:439`, `crates/league-core/src/proto.rs:59`, inline complete checks at `crates/arena-server/src/lib.rs:431-434` and `crates/arena/src/online.rs:5017-5020`, and the defective two-kind version at `crates/ember-server/src/connection.rs:427-432`. Bite: put the predicate and Windows regression test in `ember-net`, then migrate one consumer per commit. Oracle: each consumer crate's tests plus Windows-target check.
- `sanitize` and its handle wrapper are triplicated at `crates/fire-core/src/proto.rs:258-275`, `crates/kings-core/src/proto.rs:453-471`, and `crates/league-core/src/proto.rs:35-52`; only the fallback word differs. Bite: share the sanitizer in `ember-net` and retain game-specific fallback wrappers. Oracle: protocol sanitizer tests in all three core crates.
- `update_length_prefixed` is duplicated within one crate at `crates/ember-server/src/capabilities.rs:141` and `crates/ember-server/src/runtime.rs:2092`. Bite: move the exact helper to a private shared module. Oracle: `cargo test -p ember-server` and capability fingerprint fixtures.
- Procedural primitive construction is repeated across `crates/kings/src/meshes.rs:25-198`, `crates/league/src/scene.rs:262-404`, and `crates/arena/src/rounds.rs:456-477`; there is no engine primitive module to receive it. Bite: add behavior-pinned primitive builders to `ember-engine`, migrate only byte-equivalent shapes, and retain game-specific winding or UVs. Oracle: vertex-array goldens before migration plus each game's tests.

## F. Backlog hygiene

All 205 rows outside the three excluded Julibrot sections were inspected. One row records no remaining work, no pair is an exact duplicate, and three live rows carry materially stale descriptions.

- Obsolete: `docs/plans/backlog.md:322` asks for a formatter run to learn whether the tree is clean. The supplied gate ran it and found concrete diffs, now inventoried above; delete the row when the formatting bite lands.
- Stale but still actionable: `docs/plans/backlog.md:44` says Fire re-exports the engine's shape generators, but `crates/fire/src/meshes.rs:21` re-exports only `face_normals` and `planar_uvs`, and no engine primitive module exists. Rewrite the row as the measured three-consumer primitive duplication.
- Stale but still actionable: `docs/plans/backlog.md:201` cites the Kings flake at test line 393 and proposes serialization or a wider window; the actual race is the missing Bob roster wait at `crates/kings/tests/online_e2e.rs:231-237`. Replace the row when the bug-fix bite lands.
- Stale but still actionable: `docs/plans/backlog.md:317` calls both converter scripts superseded, but `tools/make_assets.py:18`, `:49` still consumes `make_hands.py`. Split the never-run 9mm predecessor from the still-used hand helper's measurement debt.

No duplicate row can be deleted without losing a distinct constraint. The nearest overlaps in infrastructure separate scheduling, supervision, and publication credentials, and the two Fire flake rows distinguish cross-client client-state pumping from the documented Windows transport/scheduler failures.

## G. Dependency debt

### Duplicate locked versions

`Cargo.lock` contains 44 package names at more than one version. The complete list is:

- `bitflags` 1.3.2 and 2.13.1 (`Cargo.lock:325`, `:331`); `block-buffer` 0.10.4 and 0.12.1 (`:346`, `:355`); `calloop` 0.13.0 and 0.14.4 (`:417`, `:431`); `calloop-wayland-source` 0.3.0 and 0.4.1 (`:444`, `:456`).
- `cpufeatures` 0.2.17 and 0.3.1 (`Cargo.lock:725`, `:734`); `crypto-common` 0.1.7 and 0.2.2 (`:764`, `:774`); `digest` 0.10.7 and 0.11.3 (`:807`, `:817`); `equator` 0.2.2 and 0.6.0 (`:1342`, `:1351`); `equator-macro` 0.2.1 and 0.6.0 (`:1360`, `:1371`).
- `getrandom` 0.2.17, 0.3.4, and 0.4.3 (`Cargo.lock:1733`, `:1744`, `:1756`); `hashbrown` 0.15.5 and 0.17.1 (`:1948`, `:1957`); `jni` 0.21.1 and 0.22.4 (`:2194`, `:2210`); `jni-sys` 0.3.1 and 0.4.1 (`:2240`, `:2249`).
- `linux-raw-sys` 0.4.15 and 0.12.1 (`Cargo.lock:2453`, `:2459`); `miniz_oxide` 0.8.9 and 0.9.1 (`:2555`, `:2565`); `ndk` 0.8.0 and 0.9.0 (`:2677`, `:2691`); `ndk-sys` 0.5.0+25.2.9519653 and 0.6.0+11769913 (`:2712`, `:2721`).
- `objc2` 0.5.2 and 0.6.4 (`Cargo.lock:2845`, `:2855`); `objc2-app-kit` 0.2.2 and 0.3.2 (`:2864`, `:2880`); `objc2-foundation` 0.2.2 and 0.3.2 (`:2982`, `:2995`).
- `quick-error` 1.2.3 and 2.0.1 (`Cargo.lock:3440`, `:3446`); `r-efi` 5.3.0 and 6.0.0 (`:3470`, `:3476`); `redox_syscall` 0.4.1, 0.5.18, and 0.9.3 (`:3547`, `:3556`, `:3565`); `rustc-hash` 1.1.0 and 2.1.3 (`:3633`, `:3639`); `rustix` 0.38.44 and 1.1.4 (`:3654`, `:3667`).
- `shlex` 1.3.0 and 2.0.1 (`Cargo.lock:3861`, `:3867`); `smithay-client-toolkit` 0.19.2 and 0.20.0 (`:3916`, `:3941`); `syn` 1.0.109, 2.0.119, and 3.0.4 (`:4049`, `:4060`, `:4071`); `thiserror` 1.0.69 and 2.0.20 (`:4129`, `:4138`); `thiserror-impl` 1.0.69 and 2.0.20 (`:4147`, `:4158`).
- `webpki-roots` 0.26.11 and 1.0.9 (`Cargo.lock:4760`, `:4769`); `windows` 0.54.0 and 0.58.0 (`:4921`, `:4931`); `windows-core` 0.54.0 and 0.58.0 (`:4941`, `:4951`); `windows-result` 0.1.2 and 0.2.0 (`:4992`, `:5001`); `windows-sys` 0.45.0, 0.52.0, 0.59.0, 0.60.2, and 0.61.2 (`:5020`, `:5029`, `:5038`, `:5047`, `:5056`); `windows-targets` 0.42.2, 0.52.6, and 0.53.5 (`:5065`, `:5080`, `:5096`).
- `windows_aarch64_gnullvm` 0.42.2, 0.52.6, and 0.53.1 (`Cargo.lock:5113`, `:5119`, `:5125`); `windows_aarch64_msvc` 0.42.2, 0.52.6, and 0.53.1 (`:5131`, `:5137`, `:5143`); `windows_i686_gnu` 0.42.2, 0.52.6, and 0.53.1 (`:5149`, `:5155`, `:5161`); `windows_i686_gnullvm` 0.52.6 and 0.53.1 (`:5167`, `:5173`); `windows_i686_msvc` 0.42.2, 0.52.6, and 0.53.1 (`:5179`, `:5185`, `:5191`); `windows_x86_64_gnu` 0.42.2, 0.52.6, and 0.53.1 (`:5197`, `:5203`, `:5209`); `windows_x86_64_gnullvm` 0.42.2, 0.52.6, and 0.53.1 (`:5215`, `:5221`, `:5227`); `windows_x86_64_msvc` 0.42.2, 0.52.6, and 0.53.1 (`:5233`, `:5239`, `:5245`).

Reverse-edge inspection shows these are incompatible transitive families rather than duplicate direct workspace declarations: graphics/window/audio stacks account for the platform crates; the TLS stack retains older `digest`, `crypto-common`, `cpufeatures`, and `webpki-roots`; proc-macro consumers retain three `syn` generations; and matrix dependencies retain two `equator` generations. No lock-only deletion is safe. The bite is a dependency-refresh spike that updates direct roots one at a time and accepts a convergence only when the workspace gate and wasm checks remain green; no behavior-free oracle exists beyond those builds and tests.

### Dependencies unused on the compiling target

The supplied native compiler output identifies three manifest entries; source inspection identifies a fourth, `serde_json` in `crates/labs/heap`, whose uses are confined to wasm-gated modules.

- `serde_json` at `crates/labs/heap/Cargo.toml:14` is used only by wasm-gated code at `crates/labs/heap/src/wasm.rs:1356`, `src/lattice_gpu.rs:383`, and `src/spike.rs:715`; move it into the wasm dependency table.
- `serde_json` at `crates/labs/layer/Cargo.toml:13` is used only by wasm-gated code at `crates/labs/layer/src/lib.rs:44-91` and `src/demo.rs:457`; move it into the wasm dependency table.
- `web-sys` at `crates/league/Cargo.toml:27-35` is used only inside the wasm backend beginning at `crates/league/src/net.rs:62`; move it, and audit the colocated wasm-bindgen declaration, under the wasm dependency table.
- `ember-client-net` at `crates/what-is-this/Cargo.toml:10` is imported only by `crates/what-is-this/src/wasm_api.rs:6`, whose module is gated at `crates/what-is-this/src/lib.rs:502-503`; move it into the wasm dependency table.

The bite touches only those four manifests. Oracle: native workspace build and Clippy must report no `cargo::unused_dependencies` warning for those manifests, while wasm checks for Heap, League, and what-is-this prove target availability and Heap transitively checks Layer.

## Ranked bites

1. Formatter-of-record patch: `crates/league/src/combat.rs`, `crates/league/src/scene/art.rs`, `crates/league/src/scene.rs`, `crates/league-core/src/kits.rs`, `crates/league-core/src/proto.rs`, and `crates/league-core/src/sim.rs`; apply exactly the 125 reported hunks; value very high, risk negligible; oracle `cargo-fmt --all -- --check` and workspace gate.
2. `league-server` Clippy gate repair: `crates/league-server/src/lib.rs`, `src/tests.rs`, `src/main.rs`, examples and call sites; value very high, risk low to medium because message-flow helpers must preserve ordering; oracle crate tests and full workspace gate.
3. Kings missing roster wait: `crates/kings/tests/online_e2e.rs`; value high, risk negligible; oracle repeated named test without weakening its assertion, then workspace gate.
4. Fire missing Racing wait: `crates/fire/tests/online_e2e.rs`; value high, risk negligible; oracle repeated named test and workspace gate.
5. Target-scope four dependencies: `crates/labs/heap/Cargo.toml`, `crates/labs/layer/Cargo.toml`, `crates/league/Cargo.toml`, `crates/what-is-this/Cargo.toml`; value medium-high, risk low; oracle native gate plus the Heap, League, and what-is-this wasm checks.
6. Removable allow wave: remove the 28 R attributes listed in section C in one crate-sized commit at a time; value medium-high, risk low; oracle each crate's ordinary Clippy and tests, with native and wasm checks where applicable.
7. Integration-test readiness and disconnect waits: five test files named in section B; value medium, risk low to medium; oracle repeated integration tests under load and workspace gate.
8. Backlog truth repair: `docs/plans/backlog.md` only; delete the formatter row and correct primitive, Kings-race, and converter descriptions; value medium, risk negligible; no behavioral oracle beyond repository reference checks.
9. Unreferenced asset cleanup: three files under `assets/`, `docs/asset-pipeline.md`, and the backlog; value medium through 5,397,461 bytes of repository removal, risk low; oracle reference search, workspace build, and unchanged shipped bundle hashes.
10. Shared transient-read predicate: `crates/ember-net`, the six consumers, and tests; value medium-high because one copy is defective, risk medium-high due cross-crate networking behavior; oracle all network crate tests, Windows target check, and workspace gate.
11. Shared sanitization and digest helpers: the three core protocol crates and two `ember-server` modules; value medium, risk medium; oracle sanitizer fixtures, capability fingerprints, and workspace gate.
12. Active allow-remediation waves: one crate per bite using the complete census; value medium, risk ranges from low for reason annotations to medium for structural splits; oracle forced-lint output, crate tests, and workspace gate.
13. Direct League server message-ordering oracle: `crates/league-server/src/tests.rs`; clear the host inbox before `StartMatch`, then assert the exact `S2C` sequence `Roster`, `Phase Live`, `State`; value medium, risk negligible; oracle the new test plus `cargo test -p league-server` and the workspace gate.
14. Frozen-source allow rationale: frozen crate headers under `games/`; value low, risk low but historical-source churn is undesirable; oracle hosted-contract tests and source-hash policy review.
15. Procedural primitive consolidation: `ember-engine` plus one game consumer per bite; value low to medium, risk high because winding, UV, and vertex order can be behavioral; oracle new vertex-array goldens and each consumer's tests.
16. Lockfile convergence spikes: direct dependency roots and `Cargo.lock`; value low without measured size/build savings, risk medium; oracle full native and wasm gates.
