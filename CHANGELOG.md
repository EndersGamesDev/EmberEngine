# Changelog

Every version the launcher (`web/games.json`) lists, in one place: what a player or a host operator would notice, and what changed the protocol. One section per game in launcher order, newest release first within each game.

**A release's source commit is the commit its build stamp names**, and each entry names the publication that establishes it, so the claim can be checked rather than taken. `deploy/deploy-pages.sh` runs `deploy/stamp-version.sh` before publishing, writing `web/version.json` — `r<commit count>`, the deploy's short sha and that commit's subject — and those bytes travel to the published branch with the build. A stamp reading `+dirty` means the published bytes correspond to no commit at all, and the entry says so.

**Which publication.** A frozen page is not frozen from its first publication: a hub deploy rebuilds every live page it carries, so one version can be published several times, from a different commit each time, and the first publication can even predate the protocol the launcher now records for it. The rule, applied to every row: **the source of a version is the newest publication of that version whose stamp names a commit on `main`.** A later publication from a commit `main` does not carry is recorded in the entry with its stamp and is not tagged, because a tag would pin history this repository cannot reach.

**Which stamp.** The hub stamp is one file at the root, so a publication that touches only one game leaves it reading the previous game's release. Two things outrank it, and an entry says which it used. A release that writes its own `version.json` inside its page tree is strongest: it names the full source sha and can carry the hash of the bundle beside it, which makes the claim checkable against the published bytes. Next is a targeted publication whose own commit message names the source it shipped. The root stamp is the fallback, and the rule above says which publication's root stamp counts.

**Tags.** Every release tag is annotated and signed and names its series and full three-grade version: `<series>-MAJOR.MINOR.PATCH`, with `ember` as the engine and workspace series. Historical `vN` and `<id>-vN` tags are re-issued at the same commits as `<series>-N.0.0` by `deploy/retag.sh`; during that migration the verifier accepts either name, and the old names are then removed. An existing tag is never moved — where a tag sits somewhere other than the entry's source commit, the entry records both. Releases without a tag state that directly.

The full evidence for every row, including the freeze commits and every publication considered, is the release ledger this file was built from.

## Pending

This is the repository's only ledger for finished work that has not yet been assigned to a launcher release; living plans keep their open work and replace completed-work narrative with a pointer here. Entries are grouped by game or area, newest first within each subsection, and every paragraph names the merge or source commit that establishes the change.

### End Game v5

The Warden’s Knife replaces proximity damage with an enemy who wakes, stands, draws a knife, pursues around the corridor plinth and commits to a visible windup, contact and recovery. A knife strike deals 15 damage once when range, facing, height, walls, the sliding gate and dodge timing allow it. Heavy greatsword hits interrupt upright attacks; feet settle when movement stops, the chair stays behind, and death preserves a partial rise (`7d4dae50`).

The deterministic 20-part warden and knife use 11,830 triangles and a 486,824-byte GLB, with measured grips, planted-foot rise and authored gait, draw, strike, stagger and death poses. Enemy health, attack status, hit sound, camera movement and rumble accompany the fight. Forty-five core tests, eighteen client/geometry/IK/input tests, three resolution tests, JavaScript syntax, scoped publication preservation, all 126 Pages fixtures, README coverage and done-list checks pass. Eighteen passive native pose captures were inspected. The final Idle-priority release build takes 7.08 seconds and produces a 24,166,325-byte WASM. Browser gameplay, physical DualSense/mobile controls, audible output, device vibration and 5K frame pacing remain unverified; direction-specific enemy hit reactions remain in the backlog (`7d4dae50`).

Knife interruptions now retain the struck pose before blending into stagger or collapse, including a fatal hit during an earlier stagger. Flinch starts continuously, the collapsing elbow avoids a pole flip, and saved poses clear before the next attack. All forty-eight core and twenty client tests pass, including exact joint continuity at windup and contact, and twelve additional passive native interruption captures were inspected. The final Idle-priority browser build takes 7.83 seconds and produces a 24,167,864-byte WASM; browser and physical-device verification remain outstanding (`a873c796`).

### End Game v4

Weight of the Blade adds Cut, Backhand, Finisher, Overhead and Rising arcs. One tap strikes once; three quick taps select Wolf's Fang, tap-pause-tap selects Gravebreaker, and tap-tap-pause-tap selects Rising Wolf. Counted input preserves rapid mouse/touch taps, held buttons do not repeat, and valid follow-ups buffer through recovery. Damage occurs at contact, with a short impact freeze, sparks, camera movement, sound and rumble; final gate recovery completes before the chapter overlay. Both V3 gauntlets retain their hilt grips throughout the authored arcs (`d550af5a`).

Twenty-eight core tests, fifteen client/geometry/IK/input tests, three resolution tests, JavaScript syntax, scoped publication preservation, all 126 Pages fixtures, all 222 changelog checks, README coverage and done-list checks pass. Fifteen passive native strike-phase captures and one staged heavy-impact frame were inspected; real contact behavior is covered by simulation tests. The final release helper builds the 24,773,487-byte WASM in 7.64 seconds at Idle priority. Browser gameplay, physical DualSense/mobile controls, audible output, device vibration and 5K frame pacing remain unverified. Swept weapon collision and rigged enemy reactions remain in the backlog (`d550af5a`).

Sword audio consumes event counters on each animation frame before the slower HUD refresh, preserving short windup-to-contact timing. Syntax and event ordering were checked; audible output remains unverified (`7a463b2c`).

### End Game v3

Hands of the Wolf replaces the box gauntlets with 34 articulated parts, finger curl, thumb opposition and measured key/sword grips. Grounded, collision-checked approaches lead into contact, grasp, lift, stow and recovery; the key follows its ring socket, the gate waits for key withdrawal, and the greatsword lifts its shallow-seated tip clear before rotating. The generated three-asset kit is 644,048 bytes. Fourteen core tests, eight client/geometry/IK/input tests, three resolution tests, exported GLB checks, native contact captures and scoped publication preservation pass; the browser bundle is 24,757,936 bytes. Browser gameplay, physical DualSense/mobile devices and 5K frame pacing remain unverified (`21c289ef`).

### End Game v2

The Living Cell replaces the first chapter's blockout architecture with smaller beveled and chipped masonry, generated limestone and oak surfaces, recessed mortar and barred window, fitted roof timbers, layered cage ironwork, and a fully lined barrel vault. The gate and loose floorboard animate; the gate's physical aperture follows its opening. The launcher and both publishers select v2 while retaining the original v1 page and bundle (`1ca5514a`).

Seven generated furnishings add a stitched straw cot, iron-hooped bucket, shackles, torn cloth, drain, candle stool and scattered straw in a 2.22 MB / 26,308-triangle kit. Cloth, chains, flame and gravity-timed drips animate around the same traversable escape route. Six simulation tests, five client/geometry/asset-budget tests, three resolution tests, native visual captures, scoped publication preservation and all 126 deployment fixtures pass; the final 24,818,334-byte WASM builds in 6.2 seconds through the Idle-priority helper. Browser gameplay, physical controllers/mobile devices and 5K frame rates remain unverified, and cloth/chain motion remains authored animation (`cc11b1af`).

### End Game v1

The first End Game chapter runs on Ember: awaken in a wooden-floored dungeon cell, recover a key, evade or fight the sleeping warden, claim the greatsword and break the exit chain. Wolf and werewolf armor forms, a prerecorded prologue, material densities and gravity, individually varied surfaces, local torch lighting and adaptive resolution up to a 5120-pixel width accompany keyboard/mouse, standard gamepad and touch controls. The launcher entry and scoped publisher preserve the other live games; each targeted release records its source and file hashes in the game's own `version.json` (`9e664ac0`).

Validation covered six simulation tests, three client input tests, three resolution tests, 53 engine tests, 12 GPU tests, 125 deployment fixtures, offline publication preservation, sibling-client compilation and passive native visual inspection. Release WASM builds successfully. Physical controllers, mobile devices, browser gameplay and 5K frame rates remain unverified; the unrigged armor swap and supplied dragon film establish a v1 prototype, with continuous animation and photorealistic material simulation left in the backlog (`9e664ac0`).

### Julibrot rounds two and three

Round three opened with one engine-neutral vocabulary for tile content, render and source identities, signed source rectangles, mesh handles, paired value and reconstruction spans, residency, quality, invalidation and a versioned 512-byte pose-header ABI. Kernels now own those records and Present re-exports adapters without changing the live rendering path or any frozen whole-grid oracle (`a8c1fdbd`).

Round two closed by replaying all four oracle classes together, removing the unreferenced Present-facts clone, completing worker-service replay coverage and proving the release wasm byte-identical to the baseline while the served bundle remained at deterministic parity (`6e940937`).

Retired main-grid ownership is now an explicit absent state, including restoration and failure paths, and the short, zoom, height, capture and finished-picture scenarios share one trace fixture without changing their frame or browser-action records (`2e864405`).

Presentation polling now consumes its fixed event set without allocating a vector, reprojection no longer accepts unread precision and validation parameters, and static versus HOT uniform construction makes dynamic offsets structural; the planner, trace and pixel corpora stayed unchanged (`84e8d4f8`).

Surface acquisition, warp submission, receipt validation, retention and presentation now pass through a replayable surface-resolution owner whose ordered record preserves failed turns and distinguishes present, drop and ignore outcomes (`0c746bf3`).

Scene and warp completion, refusal, retained-source bookkeeping and stable facts now pass through a replayable Present-event owner, with poll-before-apply-before-finish ordering and failure chronology pinned (`4a376831`).

Kernel planning, allocation, retirement, encoding and publication now pass through a replayable submission owner whose whole-grid job carries every semantic input by value, including the complete perturbation reference identity (`fb3cc50c`).

Worker submission, arrival processing, reference disposition, facts and channel-only reads now pass through a replayable worker-service owner with a chronological turn record (`a9f3c778`).

The browser and native Julibrot drivers now use one ordered refresh executor with typed stage tokens, replacing marker-only ordering and pinning scene-before-warp and fence-observation order (`e77ddad4`).

The same-thread and browser transports now share one ownership core while retaining ABI-3 bytes, four buffers and their measured transport differences, and app callers reach the viewer through narrowed pan, zoom, precision and navigation entries instead of the mutable owner (`807aa546`, integrating source merges `61acab7b` and `fd96a4de`).

Warm-up measurements now select the newest completed submission by a shared completion sequence instead of mistaking the longest wall duration for recency (`3f885813`).

Round two began with frozen worker-event and app-frame traces, a paired planner corpus, native whole-grid pixels and a served raw-RGBA artifact whose load-dependent facts are explicitly excluded from stable comparison (`e53da1bb`, integrating source merge `148c828f`).

The refactor survey classified the Julibrot ownership seams, measured its debt and established the round-two and round-three oracle contract without changing runtime behavior (`8734d336`).

Relief-aware zoom now reprojects the retained source through the requested view, applies palette only during presentation and holds a covering source instead of clearing it during the transition (`430a56ad`).

A relief zoom that cannot be represented now reports the refusal, retains partial-error evidence and records where each held, redrawn, warped or cleared picture placed the requested view (`b0422875`).

### Killshot engineering

The server bot and client transport now share the network predicate that treats an interrupted read as transient, Arena's remaining core lint suppressions carry reasons or disappeared where forced lint proved them stale, and frozen protocol fixtures retain their bytes (`9918e39c`).

Arena client and server networking gained the shared interrupted-read behavior, test-only sound helpers stopped entering production builds, and the removed unreferenced backdrop and basalt files leave checked-in shipped bundle hashes unchanged (`9501ec2b`).

Stale Arena lint suppressions were removed where native and wasm forced-lint runs proved the scopes clean, with no gameplay or wire change (`49b6518f`).

The completed Trench City roadmap item is already recorded under the existing v13 release entry from source `2d521397`; its design plan now points to that released record instead of maintaining a second completion list.

### UltimateLegue engineering

League's protocol sanitizer now uses the shared network implementation while preserving its game-specific fallback word and wire behavior (`9501ec2b`).

The League server now has a positional oracle for the exact roster, live-phase and state start-message sequence, and stale League client and core lint suppressions were removed (`49b6518f`).

League's V4-touched scene and core files now match the formatter of record, every server target passes the pedantic lint gate, and its browser-only dependency is scoped to wasm without changing play (`9324f9af`).

### Fire Racer engineering

Fire's protocol sanitizer now uses the shared network implementation with the same fallback and bytes as before (`9501ec2b`).

Fire client and server integration tests now wait for the exact racing, readiness and disconnect states they read instead of relying on fixed startup delays (`49b6518f`).

The first bounded cross-client waits removed scheduler races from Fire's online test without weakening its assertions (`9324f9af`).

### Four Kings engineering

Kings now uses the shared protocol sanitizer without changing its fallback or wire bytes, and its online creator waits for the two-seat waiting board before reading it, closing the observed roster/state race (`9501ec2b`).

Kings client and server integration tests now synchronize readiness and disconnect reads against the exact state transition instead of a fixed delay (`49b6518f`).

The first bounded cross-client waits removed scheduler races from the Kings online test while retaining its board assertions (`9324f9af`).

### what is this engineering

The frozen diagnostic source keeps its historical behavior while its lint annotation now states why plumbing may change without changing the hosted contract (`9918e39c`).

The diagnostic crate's client-network dependency is now wasm-only, matching its sole caller and removing it from native builds (`9324f9af`).

### Engine and workspace debt cleanup

The final inline transient-read copies moved onto the shared network predicate, the never-run 9 mm converter was removed, stale lint suppressions disappeared and the workspace gate remained green, closing the planned cleanup slices while leaving the two explicitly ranked follow-ups open (`9918e39c`).

Three unreferenced assets totaling 5,397,461 bytes were removed without changing shipped bundle hashes; network sanitization, digest framing and transient-read handling gained shared implementations with game-specific fallbacks and failure behavior preserved (`9501ec2b`).

Twenty-eight stale lint suppressions were removed across eleven crates and tools, each only after its applicable configurations produced no diagnostic, while helper and fixture suppressions that still carry a real reason remained (`49b6518f`).

The workspace returned to a green gate after League formatting and lint repairs, bounded online-test waits and correct target scoping for four browser-only dependencies (`9324f9af`).

The retired multiplayer prototype established the authoritative-server and client-snapshot boundary now used by the online games (`9484b7dd`).

Per-mesh textures and UVs, glTF texture coordinates, Blinn-Phong lighting, fog, the native diagnostic overlay, scene-rate controls and shader hot reload form the completed early rendering layer (`6cbb4612`).

The offline Blender-to-glTF path established multi-mesh instancing and the first authored Arena viewmodel (`b4ccb6ac`).

The initial workspace, offscreen scene/presenter split and local Pong wasm page established the engine and its first browser game (`01d913da`).

### Servers and hosting

The dedicated public node can now receive prebuilt artifacts from the build server, start the exact server commit named by the published pages and verify the shipped set through isolated host fixtures (`1106e450`).

### Tooling

The level editor once again constructs current `Level` values after supplies became part of the shared Arena type, restoring editor builds without changing a game release (`b252e35a`).

### Documentation and marketing

The repository now has an authority-to-surface engine design reference tying simulation ownership, rendering, reprojection and presentation contracts together, with Julibrot named as the executable miniature (`4855f249`).

The backlog was rebuilt from a four-way audit so closed and obsolete rows left the living plan and every retained row names a concrete remaining gap (`7369cb54`).

The public-presence pass established the developer landing page, contribution and issue routes, heartbeat record, launch drafts, neutral article, repository metadata and the first public release; distribution that needs a human account remains open in the marketing plan (`527c2ca3`).

## Killshot (arena)

### v31 — 2026-09-06

proto 24 · stamp r1511 · source `05aeea2b` · tag `arena-31.0.0` · published `0b33872b` (message)

Breach-12 shotgun: eight pellets per shell, a six-shell magazine, pump action and its own reload; slot 8 pickups and a custom starting loadout, with the existing Killshot HUD and controls preserved.

The shotgun is weapon 8. One shell launches eight server-authoritative deterministic pellets, each worth one HP, with a hard 25 m horizontal travel limit; effective damage falls off through dispersion rather than a damage curve, and an isolated pellet does not inherit the other weapons' instant headshot kill. Intrinsic spread stays nonzero while aiming down the sights and crouching still tighten it. Weapon 8 joins loot, per-life retained inventory, resupply and custom loadouts; slot 9 stays reserved. **Protocol 24** rejects a v30 client cleanly.

The page was frozen at `faa5ba09`. Between that freeze and the final tested runtime `a33c5ce4`, client audio and cache fixes and a shotgun reload-pose correction landed — near-miss cues restored on reflected and piercing segments, lobby-local projectile identity caches reset, and a magazine exchange that stays inside the first-person view. From `a33c5ce4` to the stamped source `05aeea2b`, gameplay and the tested wasm were unchanged: those two commits touch only documentation, the release verification tooling and the pinned server-only launcher, and no crate. `docs/plans/killshot-v31.md` names `a33c5ce4` (r1509) as the final runtime source; the stamp that shipped reads r1511.

Verified at the source: 434 native tests across core, server and client; strict Clippy and format; 115 checks in a real private browser; an eight-peer free-for-all, team-deathmatch and hill gate over 6,127 states with eight identical pellet endpoints on every peer. Not verified: trusted fullscreen and pointer lock, physical controller input, production load or human playtesting. Design and gate record: `docs/plans/killshot-v31.md`.

### v30 — 2026-09-06

proto 23 · stamp r1500 · source `374a8548` · tag `arena-30.0.0` · published `1a1347d9` (message)

Killshot: a LIFE and ammunition HUD, health and ammo boxes, per-weapon reload animations with a circular timer, weapon slots 1–9 retained until death, and a Classic or custom starting loadout.

Weapon ids 1–7 occupy fixed matching slots and slots 8–9 are reserved and visibly empty. Collected weapons and their remaining ammunition persist for the current life only, and everyone always keeps a safety pistol. Health boxes restore two health capped at five; ammo boxes refill only the equipped finite weapon's reserve, never its magazine; both are authoritative and respawn after thirty seconds, and no benefit means no consumption. Reload progress comes from server countdowns — the client may interpolate between snapshots but cannot grant ammunition, and switching a weapon consumes an input edge rather than generating rounds. **Protocol 23** carries the inventory, the reload remainder, the supplies and the loadout choice.

The page was frozen at `b51d3d5b`; the published bytes are `374a8548`, which aligned the deploy's live slot to v30 without weakening its guards. The reload motions animate the existing weapon meshes; no detachable magazine assets were authored for them.

Verified: 415 native tests, strict Clippy, 100 browser checks including all seven reloads, an eight-peer three-mode gate over 5,240 states with no errors, and four real WebGL2 supply captures. Design and gate record: `docs/plans/killshot-v30.md`.

### v29 — 2026-09-06

proto 22 · stamp r1490 · source `129bcac4` · tag `arena-29.0.0` · published `28d1b074` (message)

Vertical Breakwater Harbor: enterable port buildings, rooftop routes and wall-jump lanes, with sprint-crouch sliding, momentum-preserving slide jumps and chained wall kicks.

The 96 × 96 m terminal keeps its eight screened spawn pockets, three ground-route families, central hill, cargo stacks, warehouse, quay, cranes and moored ship, and gains three enterable service buildings, three stairways, two warehouse roof bridges and three four-metre wall-jump lanes. The spawn screens rise from 2.8 m to 3.4 m so the newly reachable roof edges do not expose a spawn centre; their footprints and rear exits are unchanged. Nothing is randomly placed and no rendering feature was added. **Protocol 22** carries the parkour state; the movement rules live in the shared simulation, so client prediction and the server agree on a slide and a wall kick.

The page was frozen at `67869ea5`; the published bytes are `129bcac4`, which made the protocol and client publication atomic through bound host mirrors rather than leaving a window where the book and the page disagree. Map design: `docs/harbor-v29.md`; plan and gates: `docs/plans/arena-v29-parkour.md`.

### v28 — 2026-09-06

proto 21 · stamp r1469 · source `e0c1dab0` · tag `arena-28.0.0` · published `0803a462` (message)

Personal controls: saved key bindings and mouse sensitivity, five HP with no floating health bars, head-sized headshot detection, and a three-second shield with a firing recovery and a reuse cooldown.

The combat rules are shared, so a client's prediction and the server resolve a headshot and a shield the same way. **Protocol 21.**

Published twice. The hub deploy that first carried the page was stamped r1469 `f28a145f`; the release was then re-cut on its own at `0803a462`, whose message names `e0c1dab0` — the fullscreen showcase in the v28 settings menu. That publication did not rewrite the root stamp, so the root still reads `f28a145f`, which is an ancestor of `e0c1dab0`: the message is the evidence and `e0c1dab0` is the source of the bytes now serving. That commit's verification covers 69 pure checks and 42 headless page, wasm and private-server checks; trusted fullscreen and pointer-lock permission are stubbed and are not claimed. Design and gates: `docs/plans/arena-v28-combat-controls.md`.

### v27 — 2026-09-05

proto 20 · stamp r1405 · source `256b40f8` · tag `arena-27.0.0` · published `e8a9e179`

Grounded environments: local contact shading at cover bases, warehouse corners and raised roof junctions across all three maps, with the existing sun, weather and gameplay preserved.

Presentation only — no movement, simulation, map geometry or protocol change, and no server restart. The shading is baked on the CPU and applied statically; normal maps and dynamic contact are deliberately deferred.

Verified: 717 native tests, 11 GPU regressions, strict Clippy, five wasm bundles, 41 of 41 Pages fixtures and 91 of 91 syntax checks; the shipping hash was rerun across all fifteen Harbor viewpoints and both older maps. The measured cost, including a residual 0.2-second Harbor join bake, is recorded in `docs/plans/arena-v27-contact.md` alongside `docs/environment-weather.md`.

### v26 — 2026-09-05

proto 20 · stamp r1401 · source `dae87d12` · tag `arena-26.0.0` · published `8f9f507a`

First material polish: view-dependent metal and paint highlights, matte concrete, a dry-weather sky reflection on harbor water, consistent crane paint and linear-light texture filtering.

The opt-in scalar `with_surface(roughness, metallic)` response arrives here: painted steel is dielectric, not bare metal, and neither the scalar surface nor the wetness reflection reflects scene geometry. Protocol 20 is unchanged and the running v25 server stays compatible.

Verified: 704 native tests, five GPU pixel tests, strict Clippy, all five wasm bundles, three maps in WebGL2, 41 Pages fixtures and 91 syntax checks. Design and limits: `docs/plans/arena-v26-materials.md`, `docs/environment-weather.md`.

### v25 — 2026-09-05

proto 20 · stamp r1397 · source `00982ed6` · tag `arena-25.0.0` · published `db30ce5e`

Breakwater Harbor: a hand-authored 96 × 96 m tactical port for eight players, with a moored ship, cranes, container routes, collision that holds at that size, and 4 m/s walking.

The map is built from real port references rather than generated layout: metric containers, a feeder ship, gantry cranes, warehouse openings, a secured quay, horizon scenery and map-specific daylight. Per-map prediction bounds are selected so the larger map does not inherit the small maps' limits; Trench City and Freight Yard, v24 weapon handling and the existing jump arc are all kept. The existing stance multipliers make the new walk 6.4 m/s sprinting and 2.2 m/s crouched. **Protocol 20.**

Verified: 695 integrated native tests, strict Clippy, two GPU suites, an eight-socket three-mode functional smoke, fifteen real wasm shadow views at 101 draws with no errors, and both older maps rejoined and rendered. Not verified: eight-player human balance. Design: `docs/plans/arena-v25-harbor.md`.

### v24 — 2026-09-05

proto 19 · stamp r1393 · source `1e2ec48d` · tag `arena-24.0.0` · published `f26bceb9`

Aim, settle, fire: individual weapon zoom and sight animations, aim-down-sights plus crouch accuracy, movement penalties, recovering spread, a weaker starter sidearm and slower ground movement with the jumps preserved.

**Protocol 19** carries the handling state and the recovered ground slowdown, so a peer that cannot read it would disagree about where a round goes.

Verified: 508 native tests, strict Clippy, release wasm and native builds, 35 reviewed headless rendering and input cases, and a real loopback handling and movement smoke.

### v23 — 2026-09-05

proto 17 · stamp r1293 · source `d045403d` · tag `arena-23.0.0` · published `4fba0df3`

Hands on the weapon: per-gun posed gloves and fingers, textured sleeves, and arm IK that keeps other players holding their weapons while aiming and crouching.

Presentation only — protocol 17 is unchanged, so the existing host stayed compatible and v22 stayed playable. Grip conventions: `docs/weapon-grips.md`.

### v22 — 2026-09-04

proto 17 · stamp r1270 · source `1c755fda` · tag `arena-22.0.0` (points at `d85a40fc`) · published `b52f81fb`

Skies and weather: a directional sun, cast shadows, drifting clouds, sheltered rain, fading smoke and wet-surface sky reflections.

These are shared engine features enabled through `Frame.environment`, so existing games keep their default lighting and weather stays presentation-only. Protocol 17 is unchanged; v21 stayed archived and v22 became the current protocol-17 client.

Published twice while it was the live release. The first publication was stamped r1265 `d85a40fc`, the release commit itself, and that is where the pre-existing `v22` tag sits; the second, at r1270 `1c755fda`, rebuilt the bundle, and those are the bytes the branch serves. The entry names the later one because that is what a player downloads, and records the tag's target because a tag here is never moved.

Verified at `d85a40fc`: 91 of 91 deploy syntax checks, 41 of 41 Pages fixtures, five wasm bundles rebuilt. Native and real WebGL2 validation is recorded in `docs/environment-weather.md`.

### v21 — 2026-09-04

proto 17 · stamp r1251+dirty · source `b595d60c` · tag `arena-21.0.0` · published `bc2ebdc3`

Boots: a footstep on every plant of the walk animation, a heavier and louder one at a sprint, silence while crouched at any speed, all panned and delayed by distance like the guns.

A step fires on a foot plant rather than a timer, so the cadence is the legs you can see; a crouching player makes no sound at all on every peer that could have heard them, and your own steps play quieter than a stranger's. Nothing new goes on the wire — crouch, position and feet height are already there and a walk is told from a sprint by measured speed — so **the protocol stays 17** and a v20 page still joins a v21 host.

This release's stamp reads `+dirty`: the published bytes do not correspond to any commit. `b595d60c` is the commit the deploy ran from, and the tag marks it with that caveat rather than implying a clean tree. The page was frozen at `483fc1c7`.

### v20 — 2026-09-04

proto 17 · stamp r1249 · source `46ed42bb` · tag `arena-20.0.0` · published `0c651391`

The realism pass: rounds at real muzzle velocity stopped by the first thing on their path, tracers and impacts by material driven from shot events, layered gunshots panned and delayed by distance, and the supersonic crack of a near miss.

Cover and head tests became exact on the segment rather than sampled at the tick's end point, which is what makes a 900 m/s round possible at all: at 15 m per tick the old sweep tunnelled through a 0.4 m wall. Speeds are the real muzzle velocities, with gravity on the revolver and the sniper and a sustainer on the rocket. A round now rarely survives a state, so the simulation records a shot event the tick it ends — owner, weapon, segment, what stopped it, which material, which victim, the surface normal — and the client draws from those events instead of from states. Sound is a deterministic synthesis kit with near, mid and far layers per gun, delayed by distance over 343 m/s and panned by bearing. **Protocol 17**: a v16 client draws rounds from bullet state alone and would see almost nothing fly, and it drops the shot event outright, so the frozen v19 page goes list-only against a v17 host.

Published five times while it was live, from r590 `5563d80a`, the commit that staged the page, through to r1249 `46ed42bb`, each rebuilding the bundle. The pre-existing `v20` tag sits on `46ed42bb`, which under the rule above is exactly the source: the tag agrees with the evidence, and its apparent divergence was an artefact of reading the first publication instead of the last. Design: `docs/plans/arena-v20-realism.md`.

### v19 — 2026-09-04

proto 16 · stamp r585 · source `f23ebc4e` · tag `arena-19.0.0` · published `59fccd8d`

Modes: free for all to 20 frags, team deathmatch to 30 with blue and red, king of the hill to 60 on the dock and the plinth; rounds end and restart after a ten-second pause.

The mode is chosen at lobby creation beside the map and travels by name, exactly as the map does, so the next mode is additive. Team deathmatch puts a joiner on the smaller team, spawns each side through `Level::spawns_for`, and spares a teammate from rounds, splash and blade alike. King of the hill uses the footprint the king block already hangs over. Every mode is a round: at the limit the simulation records the winner, the server announces it once, and after the pause every score, frag, death, bullet and loot block resets. All of it is stepped in the simulation, so a replay agrees on the second a round ends. **Protocol 16**: the new fields decode on a v15 peer, but that peer shoots teammates and watches rounds vanish, so the frozen v18 page goes list-only.

Published twice on the same day, first under r587 `b4df161f` and then under r585 `f23ebc4e` — a lower count, because the second deploy ran from an earlier commit, the one that staged the modes page. The second publication rebuilt the bundle, so it is the source. Design: `docs/plans/arena-v19-modes.md`.

### v18 — 2026-09-03

proto 15 · stamp r581 · source `d3e85297` · tag `arena-18.0.0` · published `9b38eb66`

Freight Yard: seven guns looted by bonking `?` blocks from below on both maps (Trench City's pads are gone), finite ammunition, a map per lobby, gamepads and rumble.

Each gun has its own speed, range, deterministic spread cone with bloom, and gravity where it applies; the sniper pierces one body and the rocket detonates on contact with a line-of-sight splash that hurts the shooter too. You spawn with the sidearm and everything better hangs in a block you have to jump into head-first — the ceiling clamp already stopped a jumping head at a box's bottom and now reports which box, and the server hands out a pool gun from a stateless hash of level seed, tick and player, so there is still no per-tick RNG state. A looted gun carries one magazine and one reserve, and death or an empty reserve returns the sidearm. Freight Yard is the second authored map, chosen per lobby. **Protocol 15, two bumps in one release**: 14 because `PState.weapon` now carries an id 1..7 where a v13 client reads a level 1..3, and 15 because Trench City itself changed under a name that did not, and a name can gate a new map but not a changed one.

This row is why the newest-on-main-publication rule is stated at the top. v18 was published three times. The first, `b1eb02f3` under r575 `bfbf6fff`, was a **protocol-14** build: `bfbf6fff` declares `PROTO_VERSION = 14` and that publication's own catalogue says 14, so it cannot be the source of the protocol-15 row the launcher lists. `d3e85297` is the first tree published as v18 that declares 15, under r581 at `9b38eb66`. A third publication, `070e4cd9`, rewrote the JS and wasm again under r583 `b9ae7bb1`; that commit is on no ref this repository can reach, so it is recorded here and not tagged, and it is the artefact the published branch serves today. The page was frozen on `main` at `3edcc1bc`, and `bfbf6fff` is the review-fix commit that followed it, applying nineteen confirmed defects — a per-player bloom counter instead of `mag - ammo`, a debounced predicted bonk, an audio backlog gate that counts state frames only, and the yard sightline driver's reach. Design: `docs/plans/arena-v18-freight-yard.md`.

### v17 — 2026-09-02

proto 13 · stamp r563 · source `9fc13db5` · tag `arena-17.0.0` · published `6735807f`

The scutum on Q, first person and on every player who raises one; the Murasama on E, a real cut.

Q raises a Roman scutum in place of the box plate, visible on every remote player whose state says shield because the protocol already carries that. E draws the sword in the operator's right fist and cuts diagonally while the rifle drops out of frame — first person only, because the protocol carries no melee state. No protocol change.

### v16 — 2026-09-02

proto 13 · stamp r561 · source `50c7c44c` · tag `arena-16.0.0` · published `5dc7e303`

One operator: the rifle the character carries, in its own gloved hands; E strikes with the butt.

Before this the player's revolver in bare hands did not match the character, who visibly carried a rifle and wore gloves, and remote players showed both weapons. The viewmodel is now built from the operator's own source: its armature carries the grip pose, so the rifle and the gloved fists come out already holding each other. Remote players hold that same rifle at the hand. No protocol change — the melee is first person only because the protocol carries no melee state.

### v15 — 2026-09-02

proto 13 · stamp r559 · source `4efe9571` · tag `arena-15.0.0` · published `599e345d`

The heavy revolver and real hands: the cylinder, hammer and trigger move.

The cylinder advances one chamber per confirmed round, the hammer cocks and falls, the trigger pulls, and the muzzle flash sits on the real muzzle, all driven from the viewmodel rig sidecar. Remote players carry the same revolver. No protocol change.

### v14 — 2026-09-02

proto 13 · stamp r556 · source `bb09f793` · tag `arena-14.0.0` · published `c03022a2`

Fullscreen: F11 on the native client, F or the button on the page.

The engine owns the window, so every ember game gets the native key; on the page F or the button fullscreens the stage and the canvas fills the screen at the screen's own aspect. No protocol change, so a v14 page plays on the same protocol-13 hosts as v13 and the v13 page stays playable.

The page was frozen at `5e56d155`; the deploy ran from `bb09f793`, which made the workstation host restart at logon.

### v13 — 2026-09-02

proto 13 · stamp r450 · source `a39481f7` · tag `arena-13.0.0` · published `f382a83a`

Trench City map and graphics update.

One authored map replaces the seeded box field, and every peer builds it identically: the server names it and `Level::named` resolves it, so the next map is an additive string rather than another bump. This is where an obstacle first gets a bottom — `Obstacle.base` makes a roof a walkable slab with a tunnel under it, and four rules read it. The city around the square is client-only decor listed in the level, so every client draws it and none of it is a collision volume. **Protocol 13**: a v12 client would predict against the seeded boxes while the server resolved against the trench city, so every wall would be either invisible or imaginary. The frozen v7–v11 pages read only the legacy address key and go list-only.

The page was frozen at `2d521397`; the deploy ran from `a39481f7`, which brought up the Windows workstation host that carried the release. Design: `docs/plans/arena-v13-trench-city.md`.

### v12 — 2026-09-02

proto 12 · stamp r439 · source `12193548` · tag `arena-12.0.0` · published `c79e1fc3`

E melee through shields, and headshots.

**Protocol 12.** The page was frozen at `8fc7800a` on 2026-09-01 and reached the published branch in the next hub deploy, stamped r437 `f9c40d62`, which also carried the renamed v0 page. It was published once more the same day under r439 `12193548`; that second publication changed only `index.html` and left the bundle byte-identical, so the code a player runs is still the r437 build while the page around it is the later one. The entry names the later publication because that is the rule every row follows, and says here exactly what it did and did not change.

### v11 — 2026-08-30

proto 11 · stamp r121 · source `671d187c` · tag `arena-11.0.0` · published `15b066e9`

Q shield: blocks and reflects.

**Protocol 11.** Published five times while it was the live release, from r99 `a0d27b26`, the commit that staged the page, through to r121 `671d187c`, each rebuilding the bundle from that day's `main`. The last one is the source: the bytes at `games/arena/v11/` were compiled from `671d187c`, whose own subject is about line endings, because a hub deploy rebuilds the live page from whatever `HEAD` it is run at.

### v10 — 2026-08-29

no proto · stamp — · source `2afec7a7` · tag `arena-10.0.0`

The SWAT operator and jumping.

Published before the build ticker existed, so no publication of this version names a commit and the source is the commit that froze the page: `2afec7a7`, "arena v10: the SWAT operator and jumping go live on the web". It was published twice; neither publication carries a stamp. The launcher records no protocol for this release or any earlier one.

### v9 — 2026-08-29

no proto · stamp — · source `3481a0fe` · tag `arena-9.0.0`

AI-built articulated characters.

Source is the freeze commit, "arena v9: articulated AI characters in the web build". Published three times, none of them stamped.

### v8 — 2026-08-29

no proto · stamp — · source `7867cabc` · tag `arena-8.0.0`

Textured graphics update.

Source is the freeze commit, "arena v8: textured graphics update for the web build". No stamp existed yet.

### v7 — 2026-08-29

no proto · stamp — · source `b4ccb6ac` · tag `arena-7.0.0`

Live server.

The commit that froze the page is "Arena v7: Blender asset pipeline, hands, ADS zoom, reloads, weapon upgrades" — the release that established the offline glTF pipeline this repository still uses. Published twice, neither stamped.

### v6 — 2026-08-28

no proto · stamp — · source `6c95cee4` · tag `arena-6.0.0`

Live server.

"Arena v6: lag-compensated hits, synthesized sound effects, Tab scoreboard". In this era the page tree was a single live pointer page: the commit for each version added its own directory and deleted its predecessor's, so the freeze commit and the release commit are one commit, and only the published branch keeps the older pages.

### v5 — 2026-08-28

no proto · stamp — · source `06201db3` · tag `arena-5.0.0`

No lag compensation, silent — archived.

"Arena v5: client-side movement prediction with server reconciliation".

### v4 — 2026-08-28

no proto · stamp — · source `9798cc58` · tag `arena-4.0.0`

No prediction — archived.

"Arena v4: first-person view, mouse look, sprint and crouch".

### v3 — 2026-08-28

no proto · stamp — · source `b4a9ad1e` · tag `arena-3.0.0`

Top-down — archived.

The first arena page, published with the games hub itself ("Games hub: version catalog, live-lobby showcase, auto-created accounts"). The shooter it points at is the drop-in arena that replaced online pong, on protocol 3.

### v1 — 2026-08-28

no proto · stamp — · source not recorded · no tag

The first web build, archived under its original path.

The launcher points this entry at `games/pong/v1/`, a directory that exists only on the published branch: `deploy/deploy-pages.sh` carries `V1_COMMIT="e7b85e8"` and materialises it from that published commit on every deploy. That is a publication commit — its tree holds only `.nojekyll`, `index.html` and `pkg` — not a source commit, and the build ticker did not exist yet, so nothing in this repository names the commit these bytes were compiled from. No tag, for that reason.

### v0 — 2026-09-06

no proto · stamp r1469 · source `f28a145f` · tag `arena-0.0.0` · published `92320cb1`

The original local game — the pong classic. Two players, one keyboard, first to 7; the paddle simulation runs entirely in the client and there is no online pong.

The page itself is older than its arena path: it entered as `web/games/pong/v2/index.html` with the games hub at `b4a9ad1e` and was renamed under `web/games/arena/v0/` at `11d6ab2d`, "rename: move pong lineage under arena paths". That rename is also why the launcher has no arena v2: the number belonged to the pong lineage and was not re-used. v0 is not frozen — the paddle simulation ships inside the arena bundle, so every hub deploy rebuilds this page too. Twenty-five publications, the newest with an on-`main` stamp being r1469 `f28a145f`, which is what serves.

## End Game

### v5 — 2026-09-08

no proto · stamp — · source `c79a32cd7706a141d5fff6d3dcb9ed29e4298a1d` (not on main) · no tag · published `1a1080695f9f9f0d35bff2477843a57ce1e5fb69` (release stamp)

The Warden’s Knife replaces the seated guard with an articulated enemy who rises, draws a knife and pursues the player. Telegraphed knife attacks resolve damage at contact and recover before repeating; V4 sword combos and the complete V1–V4 releases remain available.

### v4 — 2026-09-08

no proto · stamp — · source `f6fd41399a943c6dd91e9e4689f6ff1d377c5d44` (not on main) · no tag · published `f562dc067175ed5f22b357ab9439c589307fad74` (release stamp)

Weight of the Blade adds five directional greatsword arcs, single strikes and three buffered rhythm combos, with contact-timed damage, impact pauses, sparks, sound and controller feedback. Both articulated gauntlets stay attached to the shared hilt; V1, V2 and V3 retain their published pages and bundles.

### v3 — 2026-09-08

no proto · stamp — · source `91e5f93a7d28fa0e25e19ab30a56161034e45816` (not on main) · no tag · published `bd21d0f45100d1f33195c9ebead0c41843270477` (release stamp)

Hands of the Wolf adds articulated gauntlets and contact-timed floorboard, key, lock and two-hand greatsword interactions. The launcher retains V1 and V2 alongside the new release.

### v2 — 2026-09-08

no proto · stamp — · source `48a37a7115b5cab12f6162752f711d59edc6df89` (not on main) · no tag · published `e36d50bede1d46a543199c61cfd32cc45a6bafdb` (release stamp)

The Living Cell adds generated stone and timber surfaces, fitted masonry, detailed furnishings, and animated chains, cloth, gate and water. The release-local stamp and hashes identify the preserved 2.0.0 bundle.

### v1 — 2026-09-08

no proto · stamp — · source `9f4acfbf96484714efd06f47e784e96a110f8879` (not on main) · no tag · published `d5710a94593320494f7e07342932118c22d55523` (release stamp)

The Awakening introduces the first Ember dungeon chapter, wolf armor, greatsword, material physics, torch lighting, and keyboard/mouse, gamepad and touch controls. The release-local stamp and hashes identify the preserved 1.0.0 bundle.

## UltimateLegue (league)

### v4 — 2026-09-07

proto 2 · stamp r1799 · source `bc7baa80` · no tag · published `68b6eea8` (release stamp)

Crystalforge readability pass for lane and goal silhouettes, V4 feedback overlay and authoring polish built from existing gameplay.

This release is protocol-preserving and does not change movement, damage, healing, cooldown, economy or match mode rules.

Released 2026-09-07 from main `bc7baa80` (r1799), which completed the V4 ground treatment, feedback overlay, keyboard navigation and release integration after the visual groundwork at `70d4263f` and the Clippy cleanup at `38e58be2`. The V3 server on port 7784 is reused unchanged (league-core and league-server byte-identical to `cf4a95f0`, proven by the publisher's compatibility guard); the client bundle is `league_bg.wasm` sha256 `0e31cf22e8bd7990…`. Second ground pass: the lane paving is a seamless non-mirrored tile (its cross seam regenerated by one SDXL inpaint job), the garden is a darker jade block with periodic mottling, the sun is cooler and the hearth and Court spires taller with unchanged footprints. Feedback module clippy-clean; the Escape menu's Tab cycle reaches the sound inputs. Main CI 34126155702 and Pages deployment 34126489361 passed. The separate landing publication is `b11fdf73`. Proof: prove-live passed (11 public files byte-identical, 30 frozen files unchanged, real public 1v1 and 3v3 through wsprobe healthy on protocol 2 in 0.58 s and 0.42 s, 14.1 s total); prove-landing passed (CDN bytes of the landing match the source, 4 s); Pages deployment 34126489361 succeeded at 13:19:04Z. Known limits: a faint soft band recurs along the lane every 5.4 units; the heal cue never fires for the healed player because heal events carry unit 0; the bottom bar overflows below about 410 px as in V3.

### v3 — 2026-09-07

proto 2 · stamp — · source not recorded · no tag

Right-click movement and attacks, bindable attack-move, ability animation takeover, saved bindings, fullscreen controls and a complete new-player guide made this the first protocol 2 League build.

The repository records the V3 page tree at `1c8dde39`, its authoritative attack-move foundation at `3381e495` and its launcher promotion at `82c960bd`, while the release plan records the locally tested bundle hash. Those sources establish what was prepared, but not what reached Pages: the repository carries only the `league-v1` and `league-v2` tags, and its fetched `refs/remotes/pages/gh-pages` tip `b01bd38d` has no `games/league/v3/` tree. No published source stamp or V3 tag can therefore be evidenced from the repository, so this entry records neither rather than inventing them.

### v2 — 2026-09-06

proto 1 · stamp r1592 · source `2bc7dc98` · tag `league-2.0.0` · published `b01bd38d` (release stamp)

Crystalforge: five fleet-created champions, a textured garden arena, distinct combat animations, a redesigned HUD and saved keybindings.

A visual overhaul, not a rules change. The five champions are generated, reviewed and textured models; the arena is an elevated garden in weathered ivory, aged bronze, luminous turquoise and amber crystal, with three ground surfaces and three props, about 7.2 MB converted. Forty-two original emblems cover abilities, spells and items, and the art manifest keeps each generator prompt, seed and hash. Combat gains twenty individual Q/W/E/R cast designs and five projectile, strike and impact families, drawn from the engine's existing meshes and alpha particles; champion bodies recoil, lunge or hover on authoritative attack-start events, and that displacement never moves a health bar, a selection ring, collision or the authoritative unit position.

**Protocol 1 is retained**, deliberately. The new wire fields are presentation metadata — `ProjSnap.champ`, `Fx.champ` and `Fx.ability`, absent defaulting to the generic value, an effect kind announcing an accepted cast, and one spare bit in the auto-attack field beside the existing crit and spell bits. A v1 peer ignores all of it and applies the same gameplay rules, which is the test this repository applies to a bump: an old peer that plays a *different* game needs one, and an old peer that merely draws less does not. Cooldowns, damage, mana, hit tests, RNG and movement are unchanged and still authoritative.

The release carries its own stamp beside its page: `games/league/v2/version.json` names r1592, the full source sha and the SHA-256 of the bundle, and the published `league_bg.wasm` — 13,380,924 bytes — hashes to exactly that value. The publication's own message agrees, and as with v1 the root ticker does not: it still reads r1511 `05aeea2b`, because a targeted publication does not rewrite it. `2bc7dc98` is the commit that fixed the last thing standing in the way: the shared Pages assembler was hardcoded to League v1, so it now selects the validated live catalogue path and copies that version's complete UI and art tree while retaining the other League versions.

Verified at the source: 54 core presentation tests and core Clippy; 31 integrated client tests covering all twenty cast designs, five attack families, body poses, finite transforms and mesh validity; 8 server checks; and the actual wasm passing 104 gameplay and lobby checks, 46 interface checks and 42 binding checks across five champions, 1v1 and 3v3 practice, two-player online play, inventory, abilities, persistence, invalid saves, nested menus, Tab navigation and a narrow screen. Renderer-heavy suites must run sequentially — a concurrent run failed a fixed-delay keyboard assertion. A draft defect was found and fixed: a rejected or unconfirmed pick now resets card and detail together and keeps its feedback visible until a fresh choice is sent or acknowledged, proven by a controlled first-pick-drop test with no automatic resend. The combat gallery's eleven renderer checks use authored mocked fixtures and are visual evidence, not gameplay tests. `docs/plans/ultimate-league-v2.md` ends at a candidate checkpoint saying v2 is not yet live; the launcher and the published branch both now carry it as the live version, so that checkpoint is behind its own release. Design and gate record: `docs/plans/ultimate-league-v2.md`.

### v1 — 2026-09-06

proto 1 · stamp r1554 · source `4e069d5a` · tag `league-1.0.0` · published `468eb61d` (release stamp)

First build: one lane, five champions, a draft with rune pages and summoner spells, courts, cores, a shop, and bots. 1v1 and 3v3, online or against bots.

One lane along X with a core at each end; killing the enemy core wins. Two courts, one on each side of the lane, are the objective: the team that lands the killing blow takes gold and a timed boon — damage from the north, ability haste and gold from the south. Minion waves march every thirty seconds, champions level to twelve on last hits and passive experience, and eighteen shop items across three tiers fill six slots. A rune page is any three of eight, saved per browser and chosen in champion select; two summoner spells are picked at the same time. Five champions, three of them with authored kits, and picks are unique within a team but may be mirrored across teams, so all six seats draft from a roster of five. An unfilled or disconnected seat becomes a deterministic bot. **Protocol 1**, the join gate being exact equality as everywhere else in this repository, on its own port and its own address-book keys.

Everything is server-authoritative — the client never simulates an ability — which is why the `f32` transcendentals in movement and aim cannot desync a peer: only one peer simulates. The only randomness is `hash(tick, who, salt)`, so there is no per-tick RNG state. The client is a top-down camera and procedural meshes; every piece of text — HUD, shop, champion select, rune pages, scoreboard, minimap — is the page's, because the scene pass draws none.

The title ships spelled *UltimateLegue*; that is the product name under game id `league`, not a typo awaiting a fix.

This is the first release in this repository to carry its own stamp inside its page tree: `games/league/v1/version.json` names r1554, the full source sha and the SHA-256 of the shipped bundle, and the published `league_bg.wasm` hashes to exactly that value. The publication's own message agrees. The root stamp at that publication does not: it still reads r1511 `05aeea2b`, the Killshot v31 release, because a targeted publication does not rewrite it — which is precisely why a per-release stamp outranks the hub's. The page tree entered the repository earlier, at `7ef5bf53`, as an unfinished import; `4e069d5a` is what was built and served.

Verified at the source: 47 core tests and all-target core Clippy, all twenty abilities, economy and assists, revival and buff timing, projectile and zone serialisation, real two- and six-player WebSocket matches, and ten complete deterministic bot matches, all finishing. A regression fixture pins the core's retaliation: three level-one champions lose an unescorted assault at 17.3 s leaving the core at 325.7 HP, and win in 12.4 s with one minion wave escorting — which shows the escort changes the outcome, not that no early dive can succeed. Deliberately absent in v1: wards and vision, towers beyond the core, client-side prediction or rollback, item recipes, rune trees, spectators, and mid-match disconnect restore. Design and release record: `docs/plans/ultimate-league.md`.

## Fire Racer

### v2 — 2026-09-05

proto 2 (launcher says proto 1) · stamp — · source not in repository (`86086a2`) · no tag · published `80cf00ef` (message)

GT Circuit V2: three vehicles, tactical items, contact physics, garage and rematches.

That is the release the published branch serves, and the note its own published catalogue carries. It was cut at `80cf00ef`, whose message names `86086a2` — a commit `git cat-file -t` does not resolve here — so the bytes now serving cannot be tied to any commit in this repository, and there is no `fire-v2` tag because a tag would have to point at something this repository does not have. Recovering the source needs the checkout that build was made from, or a push of its history here. Two later publications, `917847ce` and `708d7265`, preserved those bytes rather than rebuilding them: a peer-preservation fix and an exact-LF retention fix, each carrying the root stamp of the hub deploy beside it.

**The launcher and the published release disagree.** `web/games.json` on `main` still describes fire v2 as protocol 1, "castle circuit, drift + boost, online lobbies" — the earlier v2, first published on 2026-08-31 at `15b066e9` from r121 `671d187c`, with the 920 m gothic-bailey circuit, the handbrake that breaks traction to drift and three boost charges. The published catalogue at `80cf00ef` moved the entry to protocol 2 with the GT Circuit note; `main`'s copy did not follow. This entry states the published truth and names the launcher's value beside it, so the two can be told apart; correcting `web/games.json` is outside this ledger's scope and is recorded in `docs/plans/backlog.md`.

### v1 — 2026-08-30

no proto · stamp r109+dirty · source `ebe380e0` · tag `fire-1.0.0` · published `ca84aa83`

Archived — the first build, before the castle circuit.

The page was frozen at `5e240574`, "Add fire racer game — 8-player car racing". Published once, from a dirty tree, so as with arena v21 the published bytes correspond to no commit exactly; the tag marks the commit the deploy ran from and this entry records the caveat.

## Four Kings

### v1 — 2026-09-02

proto 1 · stamp r1469 · source `f28a145f` · tag `kings-1.0.0` · published `92320cb1`

First build: within-class card swap, 15-second turns.

Four-corner chess on a 10 × 10 board — king, queen, two rooks, two bishops, a knight and seven pawns per corner, plus two legends. The Joker teleports between mirror tiles, is re-placed anywhere every fifth turn and captures on exactly one tile, its front-left. The Hero sleeps in the corner until it trades places with one of your pawns and wakes as a rook-plus-knight. Pawns march forward or left and capture on three diagonals, so a corner formation is never a wall. One action per turn, three timeouts and you are out, last king standing wins.

The page was frozen at `1c07df06` and first published at `fef6d6ff` from r383 `2268ba18`, "Deploy the Four Kings page and the hub catalogue" — the release event, and the date this entry carries. Because the page is live it has been rebuilt by every hub deploy since; the newest, r1469 `f28a145f`, is the source of the bytes now serving. Rules and architecture: `docs/kings-design.md`, `docs/kings-rules.md`.

## what is this?

### v1 — 2026-09-04

no proto · stamp r1469 · source `f28a145f` · tag `what-is-this-1.0.0` · published `92320cb1`

Nine-stage diagnostic suite, now with a Julibrot fast-slide test; report submission remains optional.

Fixed wasm CPU and timing kernels, floating-point and feature fingerprints, and a run that continues when a probe is unavailable. The complete report can stay on the device; sending it to the server is a separate, explicit choice.

The first publication, `070e4cd9`, was stamped r583 `b9ae7bb1` — a commit that is in this repository's object store but reachable from no ref, the deploy having been run from a working branch whose change reached `main` as a different commit with a different tree. Nine publications later the page has been rebuilt from `main` proper, and the newest with an on-`main` stamp, r1469 `f28a145f`, is what serves and what the note above describes. That is what the tag marks; the dangling first stamp is recorded here and is not tagged.

## Julibrot Lab

### v1 — 2026-09-04

no proto · stamp r1469 · source `f28a145f` · tag `julibrot-1.0.0` · published `92320cb1`

Updated slice viewer: saved-view application, retained frames across partition changes, and visible limits and measurements. A four-dimensional slice viewer that leaves every delivered limit and measurement on screen; WebGL2 with `EXT_color_buffer_float` is required.

The launcher lists the lab as a game, but it is a lab and its page is live, so every hub deploy rebuilds it. Its first publication, `d4966d3f`, was stamped r1239 `e409e6e1`, "merge lane/pages-julibrot: publish the Julibrot lab on GitHub Pages" — a commit `main` does not carry, though `main` carries the same publication as `4d7e0d28` with a different tree. Ten publications later the newest with an on-`main` stamp is r1469 `f28a145f`, which is what serves and what the note above describes. Charter: `docs/julibrot-lab.md`.
