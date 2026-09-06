# Changelog

Every version the launcher (`web/games.json`) lists, in one place: what a player or a host operator would notice, and what changed the protocol. One section per game in launcher order, newest release first within each game.

**A release's source commit is the commit its build stamp names.** `deploy/deploy-pages.sh` runs `deploy/stamp-version.sh` before publishing, writing `web/version.json` — `r<commit count>`, the deploy's short sha and that commit's subject — and those bytes travel to the published branch with the build. So the stamp, not the commit that froze the page tree in this repository, is what the release was compiled from; where the two differ the entry says so. A stamp reading `+dirty` means the published bytes correspond to no commit at all, and the entry says that too. Where a release was published on its own rather than in a whole-hub deploy, the publication's own message names the source and is taken over the stamp.

The hub stamp is one file at the root, so a publication that touches only one game leaves it reading the previous game's release. A release that writes its own `version.json` inside its page tree is therefore the stronger record, and where one exists it is what the entry uses: it names the full source sha rather than a short one, and it can carry the hash of the bundle beside it, which makes the claim checkable against the published bytes rather than merely recorded next to them.

**Tags.** The first game owns the bare names (`docs/hosts.md` §3): arena releases are `vNN`, every other game is `<id>-vNN`. Tags are annotated and signed, and an existing tag is never moved — where a tag sits somewhere other than the entry's source commit, the entry records both. Four releases have no tag, each for a stated reason.

The full evidence for every row, including the freeze commits and the publication commits, is the release ledger this file was built from.

## Killshot (arena)

### v31 — 2026-09-06

proto 24 · stamp r1511 · source `05aeea2b` · tag `v31`

Breach-12 shotgun: eight pellets per shell, a six-shell magazine, pump action and its own reload; slot 8 pickups and a custom starting loadout, with the existing Killshot HUD and controls preserved.

The shotgun is weapon 8. One shell launches eight server-authoritative deterministic pellets, each worth one HP, with a hard 25 m horizontal travel limit; effective damage falls off through dispersion rather than a damage curve, and an isolated pellet does not inherit the other weapons' instant headshot kill. Intrinsic spread stays nonzero while aiming down the sights and crouching still tighten it. Weapon 8 joins loot, per-life retained inventory, resupply and custom loadouts; slot 9 stays reserved. **Protocol 24** rejects a v30 client cleanly.

The page was frozen at `faa5ba09`; the published bytes are `05aeea2b`, which carried the protocol-24 release verifier (two stale protocol-23 assertions, now covered by acceptance and rejection regressions) and a hash- and port-pinned server-only launcher that never touches tunnels. Gameplay is unchanged between the two. `docs/plans/killshot-v31.md` names `a33c5ce4` (r1509) as the final runtime source; that is a pre-publication checkpoint, and the stamp that shipped reads r1511.

Verified at the source: 434 native tests across core, server and client; strict Clippy and format; 115 checks in a real private browser; an eight-peer free-for-all, team-deathmatch and hill gate over 6,127 states with eight identical pellet endpoints on every peer. Not verified: trusted fullscreen and pointer lock, physical controller input, production load or human playtesting. Design and gate record: `docs/plans/killshot-v31.md`.

### v30 — 2026-09-06

proto 23 · stamp r1500 · source `374a8548` · tag `v30`

Killshot: a LIFE and ammunition HUD, health and ammo boxes, per-weapon reload animations with a circular timer, weapon slots 1–9 retained until death, and a Classic or custom starting loadout.

Weapon ids 1–7 occupy fixed matching slots and slots 8–9 are reserved and visibly empty. Collected weapons and their remaining ammunition persist for the current life only, and everyone always keeps a safety pistol. Health boxes restore two health capped at five; ammo boxes refill only the equipped finite weapon's reserve, never its magazine; both are authoritative and respawn after thirty seconds, and no benefit means no consumption. Reload progress comes from server countdowns — the client may interpolate between snapshots but cannot grant ammunition, and switching a weapon consumes an input edge rather than generating rounds. **Protocol 23** carries the inventory, the reload remainder, the supplies and the loadout choice.

The page was frozen at `b51d3d5b`; the published bytes are `374a8548`, which aligned the deploy's live slot to v30 without weakening its guards. The reload motions animate the existing weapon meshes; no detachable magazine assets were authored for them.

Verified: 415 native tests, strict Clippy, 100 browser checks including all seven reloads, an eight-peer three-mode gate over 5,240 states with no errors, and four real WebGL2 supply captures. Design and gate record: `docs/plans/killshot-v30.md`.

### v29 — 2026-09-06

proto 22 · stamp r1490 · source `129bcac4` · tag `v29`

Vertical Breakwater Harbor: enterable port buildings, rooftop routes and wall-jump lanes, with sprint-crouch sliding, momentum-preserving slide jumps and chained wall kicks.

The 96 × 96 m terminal keeps its eight screened spawn pockets, three ground-route families, central hill, cargo stacks, warehouse, quay, cranes and moored ship, and gains three enterable service buildings, three stairways, two warehouse roof bridges and three four-metre wall-jump lanes. The spawn screens rise from 2.8 m to 3.4 m so the newly reachable roof edges do not expose a spawn centre; their footprints and rear exits are unchanged. Nothing is randomly placed and no rendering feature was added. **Protocol 22** carries the parkour state; the movement rules live in the shared simulation, so client prediction and the server agree on a slide and a wall kick.

The page was frozen at `67869ea5`; the published bytes are `129bcac4`, which made the protocol and client publication atomic through bound host mirrors rather than leaving a window where the book and the page disagree. Map design: `docs/harbor-v29.md`; plan and gates: `docs/plans/arena-v29-parkour.md`.

### v28 — 2026-09-06

proto 21 · stamp r1469 · source `e0c1dab0` · tag `v28`

Personal controls: saved key bindings and mouse sensitivity, five HP with no floating health bars, head-sized headshot detection, and a three-second shield with a firing recovery and a reuse cooldown.

The combat rules are shared, so a client's prediction and the server resolve a headshot and a shield the same way. **Protocol 21.**

This is the one release published twice. The hub deploy that first carried the page was stamped r1469 `f28a145f`; the release was then re-cut on its own from `e0c1dab0`, which added the fullscreen showcase to the v28 settings menu, and `f28a145f` is an ancestor of it. `e0c1dab0` is therefore the source of the bytes now serving. That commit's verification covers 69 pure checks and 42 headless page, wasm and private-server checks; trusted fullscreen and pointer-lock permission are stubbed and are not claimed. Design and gates: `docs/plans/arena-v28-combat-controls.md`.

### v27 — 2026-09-05

proto 20 · stamp r1405 · source `256b40f8` · tag `v27`

Grounded environments: local contact shading at cover bases, warehouse corners and raised roof junctions across all three maps, with the existing sun, weather and gameplay preserved.

Presentation only — no movement, simulation, map geometry or protocol change, and no server restart. The shading is baked on the CPU and applied statically; normal maps and dynamic contact are deliberately deferred.

Verified: 717 native tests, 11 GPU regressions, strict Clippy, five wasm bundles, 41 of 41 Pages fixtures and 91 of 91 syntax checks; the shipping hash was rerun across all fifteen Harbor viewpoints and both older maps. The measured cost, including a residual 0.2-second Harbor join bake, is recorded in `docs/plans/arena-v27-contact.md` alongside `docs/environment-weather.md`.

### v26 — 2026-09-05

proto 20 · stamp r1401 · source `dae87d12` · tag `v26`

First material polish: view-dependent metal and paint highlights, matte concrete, a dry-weather sky reflection on harbor water, consistent crane paint and linear-light texture filtering.

The opt-in scalar `with_surface(roughness, metallic)` response arrives here: painted steel is dielectric, not bare metal, and neither the scalar surface nor the wetness reflection reflects scene geometry. Protocol 20 is unchanged and the running v25 server stays compatible.

Verified: 704 native tests, five GPU pixel tests, strict Clippy, all five wasm bundles, three maps in WebGL2, 41 Pages fixtures and 91 syntax checks. Design and limits: `docs/plans/arena-v26-materials.md`, `docs/environment-weather.md`.

### v25 — 2026-09-05

proto 20 · stamp r1397 · source `00982ed6` · tag `v25`

Breakwater Harbor: a hand-authored 96 × 96 m tactical port for eight players, with a moored ship, cranes, container routes, collision that holds at that size, and 4 m/s walking.

The map is built from real port references rather than generated layout: metric containers, a feeder ship, gantry cranes, warehouse openings, a secured quay, horizon scenery and map-specific daylight. Per-map prediction bounds are selected so the larger map does not inherit the small maps' limits; Trench City and Freight Yard, v24 weapon handling and the existing jump arc are all kept. The existing stance multipliers make the new walk 6.4 m/s sprinting and 2.2 m/s crouched. **Protocol 20.**

Verified: 695 integrated native tests, strict Clippy, two GPU suites, an eight-socket three-mode functional smoke, fifteen real wasm shadow views at 101 draws with no errors, and both older maps rejoined and rendered. Not verified: eight-player human balance. Design: `docs/plans/arena-v25-harbor.md`.

### v24 — 2026-09-05

proto 19 · stamp r1393 · source `1e2ec48d` · tag `v24`

Aim, settle, fire: individual weapon zoom and sight animations, aim-down-sights plus crouch accuracy, movement penalties, recovering spread, a weaker starter sidearm and slower ground movement with the jumps preserved.

**Protocol 19** carries the handling state and the recovered ground slowdown, so a peer that cannot read it would disagree about where a round goes.

Verified: 508 native tests, strict Clippy, release wasm and native builds, 35 reviewed headless rendering and input cases, and a real loopback handling and movement smoke.

### v23 — 2026-09-05

proto 17 · stamp r1293 · source `d045403d` · tag `v23`

Hands on the weapon: per-gun posed gloves and fingers, textured sleeves, and arm IK that keeps other players holding their weapons while aiming and crouching.

Presentation only — protocol 17 is unchanged, so the existing host stayed compatible and v22 stayed playable. Grip conventions: `docs/weapon-grips.md`.

### v22 — 2026-09-04

proto 17 · stamp r1265 · source `d85a40fc` · tag `v22`

Skies and weather: a directional sun, cast shadows, drifting clouds, sheltered rain, fading smoke and wet-surface sky reflections.

These are shared engine features enabled through `Frame.environment`, so existing games keep their default lighting and weather stays presentation-only. Protocol 17 is unchanged; v21 stayed archived and v22 became the current protocol-17 client.

Verified: 91 of 91 deploy syntax checks, 41 of 41 Pages fixtures, five wasm bundles rebuilt. Native and real WebGL2 validation is recorded in `docs/environment-weather.md`.

### v21 — 2026-09-04

proto 17 · stamp r1251+dirty · source `b595d60c` · tag `v21`

Boots: a footstep on every plant of the walk animation, a heavier and louder one at a sprint, silence while crouched at any speed, all panned and delayed by distance like the guns.

A step fires on a foot plant rather than a timer, so the cadence is the legs you can see; a crouching player makes no sound at all on every peer that could have heard them, and your own steps play quieter than a stranger's. Nothing new goes on the wire — crouch, position and feet height are already there and a walk is told from a sprint by measured speed — so **the protocol stays 17** and a v20 page still joins a v21 host.

This release's stamp reads `+dirty`: the published bytes do not correspond to any commit. `b595d60c` is the commit the deploy ran from, and the tag marks it with that caveat rather than implying a clean tree. The page was frozen at `483fc1c7`.

### v20 — 2026-09-04

proto 17 · stamp r590 · source `5563d80a` · tag `v20` (points at `46ed42bb`)

The realism pass: rounds at real muzzle velocity stopped by the first thing on their path, tracers and impacts by material driven from shot events, layered gunshots panned and delayed by distance, and the supersonic crack of a near miss.

Cover and head tests became exact on the segment rather than sampled at the tick's end point, which is what makes a 900 m/s round possible at all: at 15 m per tick the old sweep tunnelled through a 0.4 m wall. Speeds are the real muzzle velocities, with gravity on the revolver and the sniper and a sustainer on the rocket. A round now rarely survives a state, so the simulation records a shot event the tick it ends — owner, weapon, segment, what stopped it, which material, which victim, the surface normal — and the client draws from those events instead of from states. Sound is a deterministic synthesis kit with near, mid and far layers per gun, delayed by distance over 343 m/s and panned by bearing. **Protocol 17**: a v16 client draws rounds from bullet state alone and would see almost nothing fly, and it drops the shot event outright, so the frozen v19 page goes list-only against a v17 host.

The `v20` tag predates this file and sits on `46ed42bb` ("Merge branch 'lane/arena-v18'"), not on the commit the stamp names. Existing tags are not moved, so both shas are recorded here. Design: `docs/plans/arena-v20-realism.md`.

### v19 — 2026-09-04

proto 16 · stamp r587 · source `b4df161f` · tag `v19`

Modes: free for all to 20 frags, team deathmatch to 30 with blue and red, king of the hill to 60 on the dock and the plinth; rounds end and restart after a ten-second pause.

The mode is chosen at lobby creation beside the map and travels by name, exactly as the map does, so the next mode is additive. Team deathmatch puts a joiner on the smaller team, spawns each side through `Level::spawns_for`, and spares a teammate from rounds, splash and blade alike. King of the hill uses the footprint the king block already hangs over. Every mode is a round: at the limit the simulation records the winner, the server announces it once, and after the pause every score, frag, death, bullet and loot block resets. All of it is stepped in the simulation, so a replay agrees on the second a round ends. **Protocol 16**: the new fields decode on a v15 peer, but that peer shoots teammates and watches rounds vanish, so the frozen v18 page goes list-only.

The page was frozen at `f23ebc4e`; the deploy ran two commits later from `b4df161f`, which resolved the Pages Python interpreter. Design: `docs/plans/arena-v19-modes.md`.

### v18 — 2026-09-03

proto 15 · stamp r575 · source `bfbf6fff` · tag `v18`

Freight Yard: seven guns looted by bonking `?` blocks from below on both maps (Trench City's pads are gone), finite ammunition, a map per lobby, gamepads and rumble.

Each gun has its own speed, range, deterministic spread cone with bloom, and gravity where it applies; the sniper pierces one body and the rocket detonates on contact with a line-of-sight splash that hurts the shooter too. You spawn with the sidearm and everything better hangs in a block you have to jump into head-first — the ceiling clamp already stopped a jumping head at a box's bottom and now reports which box, and the server hands out a pool gun from a stateless hash of level seed, tick and player, so there is still no per-tick RNG state. A looted gun carries one magazine and one reserve, and death or an empty reserve returns the sidearm. Freight Yard is the second authored map, chosen per lobby. **Protocol 15, two bumps in one release**: 14 because `PState.weapon` now carries an id 1..7 where a v13 client reads a level 1..3, and 15 because Trench City itself changed under a name that did not, and a name can gate a new map but not a changed one.

The page was frozen at `3edcc1bc`; the deploy ran from `bfbf6fff`, the review-fix commit that followed it and applied nineteen confirmed defects — a per-player bloom counter instead of `mag - ammo`, a debounced predicted bonk, an audio backlog gate that counts state frames only, and the yard sightline driver's reach. Design: `docs/plans/arena-v18-freight-yard.md`.

### v17 — 2026-09-02

proto 13 · stamp r563 · source `9fc13db5` · tag `v17`

The scutum on Q, first person and on every player who raises one; the Murasama on E, a real cut.

Q raises a Roman scutum in place of the box plate, visible on every remote player whose state says shield because the protocol already carries that. E draws the sword in the operator's right fist and cuts diagonally while the rifle drops out of frame — first person only, because the protocol carries no melee state. No protocol change.

### v16 — 2026-09-02

proto 13 · stamp r561 · source `50c7c44c` · tag `v16`

One operator: the rifle the character carries, in its own gloved hands; E strikes with the butt.

Before this the player's revolver in bare hands did not match the character, who visibly carried a rifle and wore gloves, and remote players showed both weapons. The viewmodel is now built from the operator's own source: its armature carries the grip pose, so the rifle and the gloved fists come out already holding each other. Remote players hold that same rifle at the hand. No protocol change — the melee is first person only because the protocol carries no melee state.

### v15 — 2026-09-02

proto 13 · stamp r559 · source `4efe9571` · tag `v15`

The heavy revolver and real hands: the cylinder, hammer and trigger move.

The cylinder advances one chamber per confirmed round, the hammer cocks and falls, the trigger pulls, and the muzzle flash sits on the real muzzle, all driven from the viewmodel rig sidecar. Remote players carry the same revolver. No protocol change.

### v14 — 2026-09-02

proto 13 · stamp r556 · source `bb09f793` · tag `v14`

Fullscreen: F11 on the native client, F or the button on the page.

The engine owns the window, so every ember game gets the native key; on the page F or the button fullscreens the stage and the canvas fills the screen at the screen's own aspect. No protocol change, so a v14 page plays on the same protocol-13 hosts as v13 and the v13 page stays playable.

The page was frozen at `5e56d155`; the deploy ran from `bb09f793`, which made the workstation host restart at logon.

### v13 — 2026-09-02

proto 13 · stamp r450 · source `a39481f7` · tag `v13`

Trench City map and graphics update.

One authored map replaces the seeded box field, and every peer builds it identically: the server names it and `Level::named` resolves it, so the next map is an additive string rather than another bump. This is where an obstacle first gets a bottom — `Obstacle.base` makes a roof a walkable slab with a tunnel under it, and four rules read it. The city around the square is client-only decor listed in the level, so every client draws it and none of it is a collision volume. **Protocol 13**: a v12 client would predict against the seeded boxes while the server resolved against the trench city, so every wall would be either invisible or imaginary. The frozen v7–v11 pages read only the legacy address key and go list-only.

The page was frozen at `2d521397`; the deploy ran from `a39481f7`, which brought up the Windows workstation host that carried the release. Design: `docs/plans/arena-v13-trench-city.md`.

### v12 — 2026-09-02

proto 12 · stamp r437 · source `f9c40d62` · tag `v12`

E melee through shields, and headshots.

**Protocol 12.** The page was frozen at `8fc7800a` on 2026-09-01 and reached the published branch in the next hub deploy, which also carried the renamed v0 page and was stamped `f9c40d62`.

### v11 — 2026-08-30

proto 11 · stamp r99 · source `a0d27b26` · tag `v11`

Q shield: blocks and reflects.

**Protocol 11.** The commit that staged the page is the commit the deploy ran from.

### v10 — 2026-08-29

no proto · stamp — · source `2afec7a7` · tag `v10`

The SWAT operator and jumping.

Published before the build ticker existed, so the source is the commit that froze the page: `2afec7a7`, "arena v10: the SWAT operator and jumping go live on the web". The launcher records no protocol for this release or any earlier one.

### v9 — 2026-08-29

no proto · stamp — · source `3481a0fe` · tag `v9`

AI-built articulated characters.

Source is the freeze commit, "arena v9: articulated AI characters in the web build". No stamp existed yet.

### v8 — 2026-08-29

no proto · stamp — · source `7867cabc` · tag `v8`

Textured graphics update.

Source is the freeze commit, "arena v8: textured graphics update for the web build". No stamp existed yet.

### v7 — 2026-08-29

no proto · stamp — · source `b4ccb6ac` · tag `v7`

Live server.

The commit that froze the page is "Arena v7: Blender asset pipeline, hands, ADS zoom, reloads, weapon upgrades" — the release that established the offline glTF pipeline this repository still uses. No stamp existed yet.

### v6 — 2026-08-28

no proto · stamp — · source `6c95cee4` · tag `v6`

Live server.

"Arena v6: lag-compensated hits, synthesized sound effects, Tab scoreboard". In this era the page tree was a single live pointer page: the commit for each version added its own directory and deleted its predecessor's, so the freeze commit and the release commit are one commit, and only the published branch keeps the older pages.

### v5 — 2026-08-28

no proto · stamp — · source `06201db3` · tag `v5`

No lag compensation, silent — archived.

"Arena v5: client-side movement prediction with server reconciliation".

### v4 — 2026-08-28

no proto · stamp — · source `9798cc58` · tag `v4`

No prediction — archived.

"Arena v4: first-person view, mouse look, sprint and crouch".

### v3 — 2026-08-28

no proto · stamp — · source `b4a9ad1e` · tag `v3`

Top-down — archived.

The first arena page, published with the games hub itself ("Games hub: version catalog, live-lobby showcase, auto-created accounts"). The shooter it points at is the drop-in arena that replaced online pong, on protocol 3.

### v1 — 2026-08-28

no proto · stamp — · source not recorded · no tag

The first web build, archived under its original path.

The launcher points this entry at `games/pong/v1/`, a directory that exists only on the published branch: `deploy/deploy-pages.sh` carries `V1_COMMIT="e7b85e8"` and materialises it from that published commit on every deploy. That is a publication commit, not a source commit, and the build ticker did not exist yet, so nothing in this repository names the commit these bytes were compiled from. No tag, for that reason.

### v0 — 2026-09-02

no proto · stamp r437 · source `f9c40d62` · tag `v0`

The original local game — the pong classic. Two players, one keyboard, first to 7; the paddle simulation runs entirely in the client and there is no online pong.

The page itself is older than its arena path: it entered as `web/games/pong/v2/index.html` with the games hub at `b4a9ad1e` and was renamed under `web/games/arena/v0/` at `11d6ab2d`, "rename: move pong lineage under arena paths". The arena-path bytes were first published in the hub deploy stamped `f9c40d62`, the same deploy that carried v12. This rename is also why the launcher has no arena v2: the number belonged to the pong lineage and was not re-used.

## UltimateLegue (league)

### v1 — 2026-09-06

proto 1 · stamp r1554 · source `4e069d5a` · tag `league-v1`

First build: one lane, five champions, a draft with rune pages and summoner spells, courts, cores, a shop, and bots. 1v1 and 3v3, online or against bots.

One lane along X with a core at each end; killing the enemy core wins. Two courts, one on each side of the lane, are the objective: the team that lands the killing blow takes gold and a timed boon — damage from the north, ability haste and gold from the south. Minion waves march every thirty seconds, champions level to twelve on last hits and passive experience, and eighteen shop items across three tiers fill six slots. A rune page is any three of eight, saved per browser and chosen in champion select; two summoner spells are picked at the same time. Five champions, three of them with authored kits, and picks are unique within a team but may be mirrored across teams, so all six seats draft from a roster of five. An unfilled or disconnected seat becomes a deterministic bot. **Protocol 1**, the join gate being exact equality as everywhere else in this repository, on its own port and its own address-book keys.

Everything is server-authoritative — the client never simulates an ability — which is why the `f32` transcendentals in movement and aim cannot desync a peer: only one peer simulates. The only randomness is `hash(tick, who, salt)`, so there is no per-tick RNG state. The client is a top-down camera and procedural meshes; every piece of text — HUD, shop, champion select, rune pages, scoreboard, minimap — is the page's, because the scene pass draws none.

The title ships spelled *UltimateLegue*; that is the product name under game id `league`, not a typo awaiting a fix.

This is the first release in this repository to carry its own stamp inside its page tree: `games/league/v1/version.json` names `r1554`, the full source sha and the SHA-256 of the shipped bundle, and the published `league_bg.wasm` hashes to exactly that value. The publication's own message agrees. The page tree entered the repository earlier, at `7ef5bf53`, as an unfinished import; `4e069d5a` is what was built and served.

Verified at the source: 47 core tests and all-target core Clippy, all twenty abilities, economy and assists, revival and buff timing, projectile and zone serialisation, real two- and six-player WebSocket matches, and ten complete deterministic bot matches, all finishing. A regression fixture pins the core's retaliation: three level-one champions lose an unescorted assault at 17.3 s leaving the core at 325.7 HP, and win in 12.4 s with one minion wave escorting — which shows the escort changes the outcome, not that no early dive can succeed. Deliberately absent in v1: wards and vision, towers beyond the core, client-side prediction or rollback, item recipes, rune trees, spectators, and mid-match disconnect restore. Design and release record: `docs/plans/ultimate-league.md`.

## Fire Racer

### v2 — 2026-09-05

proto 1 · stamp — · source not in repository (`86086a2`) · no tag

Castle circuit, drift and boost, online lobbies. A 920 m gothic-bailey circuit with a main straight, a long sweeper, a fountain chicane and a hairpin; the handbrake breaks traction and three boost charges are spendable. Practice alone or race on an authoritative server.

This release was cut twice. The page tree was frozen at `11adb490` and first published on 2026-08-31 from `671d187c` (r121). It was then re-published as the GT Circuit build from `86086a2` — a commit `git cat-file -t` does not resolve in this repository, so the bytes now serving cannot be tied to any commit here. That is why there is no `fire-v2` tag: a tag would have to point at something this repository does not have. Recovering the source would need the checkout that build was made from, or a push of its history into this repository.

### v1 — 2026-08-30

no proto · stamp r109+dirty · source `ebe380e0` · tag `fire-v1`

Archived — the first build, before the castle circuit.

The page was frozen at `5e240574`, "Add fire racer game — 8-player car racing". The deploy ran from `ebe380e0` with a dirty tree, so as with arena v21 the published bytes correspond to no commit exactly; the tag marks the commit the deploy ran from and this entry records the caveat.

## Four Kings

### v1 — 2026-09-02

proto 1 · stamp r383 · source `2268ba18` · tag `kings-v1`

First build: within-class card swap, 15-second turns.

Four-corner chess on a 10 × 10 board — king, queen, two rooks, two bishops, a knight and seven pawns per corner, plus two legends. The Joker teleports between mirror tiles, is re-placed anywhere every fifth turn and captures on exactly one tile, its front-left. The Hero sleeps in the corner until it trades places with one of your pawns and wakes as a rook-plus-knight. Pawns march forward or left and capture on three diagonals, so a corner formation is never a wall. One action per turn, three timeouts and you are out, last king standing wins.

The page was frozen at `1c07df06`; the deploy that first published it ran from `2268ba18`. Because the page is live it is rebuilt by every hub deploy, and the bytes now serving were stamped r1469 `f28a145f`. Rules and architecture: `docs/kings-design.md`, `docs/kings-rules.md`.

## what is this?

### v1 — 2026-09-04

no proto · stamp r583 · source `b9ae7bb1` (not on main) · no tag

Nine-stage diagnostic suite, now with a Julibrot fast-slide test; report submission remains optional.

Fixed wasm CPU and timing kernels, floating-point and feature fingerprints, and a run that continues when a probe is unavailable. The complete report can stay on the device; sending it to the server is a separate, explicit choice.

The stamp names `b9ae7bb1`, a commit that is in this repository's object store but reachable from no ref: the deploy ran from a working branch whose change reached `main` as a different commit, `33fe04d2`, with the same subject and date but a different tree. The two are therefore not interchangeable, and no tag is created — a tag would pin history `main` does not carry. The page was frozen at `d4bc5bdd`, and because it is live the bytes now serving were stamped r1469 `f28a145f`, which is what the note above describes.

## Julibrot Lab

### v1 — 2026-09-04

no proto · stamp r1239 · source `e409e6e1` (not on main) · no tag

Updated slice viewer: saved-view application, retained frames across partition changes, and visible limits and measurements. A four-dimensional slice viewer that leaves every delivered limit and measurement on screen; WebGL2 with `EXT_color_buffer_float` is required.

The launcher lists the lab as a game, but it is a lab and its page is live, so every hub deploy rebuilds it: the bytes now serving were stamped r1469 `f28a145f`, and the note above describes that state rather than the first publication. The first publication's stamp names `e409e6e1`, "merge lane/pages-julibrot: publish the Julibrot lab on GitHub Pages" — a commit `main` does not carry, though it carries the same publication as `4d7e0d28` with a different tree. No tag, for the same reason as what-is-this. Charter: `docs/julibrot-lab.md`.
