# UltimateLegue v2 — Crystalforge

The owner requested a visual overhaul using the LAN asset fleet, coordinated with Fable and OpenCode through Barza. Champions are generated and reviewed in order: SW4RM, EmberKnight, The Hallow One, Bog Maw, Tessera the Clockmaker; environment generation follows the five champions. The game remains on Ember, with authoritative online 1v1/3v3 and local practice.

What v2 actually shipped — its protocol, build stamp and source commit — is the `v2` entry under UltimateLegue in `CHANGELOG.md`; the checkpoint below predates the publication.

The original visual direction is Crystalforge: weathered ivory, aged bronze, luminous turquoise and amber crystal, carved geometric motifs, and an elevated garden arena. Strong silhouettes, clearly visible team affiliation, readable combat effects, and legible controls take priority over ornamental density.

## Ownership and interfaces

Root integrates on `codex/ultimate-league-v2`, drives the single SDXL queue, reviews each image/mesh, owns portrait derivatives and provenance, verifies the combined build, and releases it. Fable delivered `scene.rs`, `scene/art.rs`, converted GLBs/sidecars and conversion tooling on `fable/league-v2-scene` through349813c9. OpenCode's three interface files were integrated from its worktree at89efe868. Following the owner's terminal instruction, root took final UI and scene integration ownership; the external branches are preserved. Assignments and handoffs are recorded in Barza messages143–176.

The scene appends meshes after procedural ids1–9 through `scene::build_meshes()`. Assets use embedded 8-bit base-color textures, white instance tint, and scalar surface response; vertex colors and imported PBR maps are not consumed by Ember. Generated sources remain in `target/league-art`; converted GLBs and small web derivatives ship. Target added mesh/texture payload is8MiB, with512px atlases and roughly3k–6k triangles per champion.

The client provides `bindings_json()`, `set_bindings_json(string)`, `binding_options_json()`, and `set_input_enabled(bool)`. Fourteen actions use physical `KeyboardEvent.code` values: `q,w,e,r,d,f,item1,item2,item3,item4,item5,item6,stop,shop`. Defaults are QWER, DF,1–6,S,B. Empty JSON resets; partial maps merge over defaults; invalid/duplicate maps are rejected atomically. Rust dispatches gameplay controls; DOM dispatches the configured shop key. Shift/Ctrl plus the mapped ability learns a rank. Escape is reserved for menus. The page persists validated bindings and suppresses gameplay during settings/key capture/text entry.

## Fleet and release constraints

SDXL runs on specht32:8188; root submits one image job at a time with the reproducible `tools/league/fleet-art.py` runner. Fable owns the TripoSR queue on knecht24 GPU0, using the worker's explicit GPU selection so the separately claimed MMAudio GPU remains undisturbed. No profile switching is required. TRELLIS health alone does not prove that its gated image encoder is usable.

V1 remains a frozen versioned web build. V2 is published at `games/league/v2/` and becomes the hub default only after integrated browser and public-host verification. Keep the existing League public tunnel alive across any League server update; preserve all other games, processes, tasks, address-book fields and Pages trees. Knecht host services remain inactive. All builds run at Idle priority, and validation records measured wall times.

## Combat presentation

The owner subsequently requested individual designs and animations for every skillshot and auto attack. `combat.rs` draws five distinct projectile/strike/impact families and twenty individual Q/W/E/R cast designs from Ember's existing meshes and alpha particles. Champion bodies recoil, lunge or hover on authoritative attack-start events. Visual displacement never moves health bars, selection rings, collision or the authoritative unit position. Effects use open rings, thin strokes and sparse sparks so a cast does not hide its target.

`ProjSnap.champ` and `Fx.champ/ability` are presentation metadata; absent fields default to255 (generic). Ability indices0–3 mean Q/W/E/R and4 means an auto. Effect kind13 announces an accepted cast; auto effect kind0 reserves bit2 of `v` for attack starts, preserving crit bit0 and spell bit1. Original source identity survives clone removal and delayed effects. V1 ignores the extra fields and kind13 and continues to apply the same gameplay rules, so protocol1 is retained. Cooldowns, damage, mana, hit tests, RNG and movement remain authoritative and unchanged.

## Release candidate verification

The fleet generated and root reviewed all five champion references in order, followed by the lane, garden, court, obelisk, arena splash, tree and arch. Fable's five textured champion models, three ground surfaces and three props were reviewed through native captures and integrated; the converted payload is about7.2MB. The public art manifest preserves generator prompts, seeds and hashes. Forty-two original SVG emblems cover abilities, spells and items.

Core presentation changes passed 54 tests in 6.914s and core Clippy in 1.235s. The integrated client passed 31 tests in 15.718s, covering all 20 cast designs, five attack families, body poses, finite transforms and mesh validity. Server tests passed 8 checks in 8.761s after updating an Fx fixture. Strict client/core Clippy passed; the broader server invocation exposed pre-existing pedantic warnings outside this change and is not claimed clean.

The final 13,380,924-byte WASM has SHA256 `deb7b3424a4d9fb2b4ab3e953573ec85eeaa36596cc6d5627171e8a0058b73c2`. The latest strict client lint and release WASM build took 5.294s together, at Idle priority with four build jobs. The actual WASM passed 104 gameplay/lobby checks in 22.985s, 46 interface checks in 10.358s, and 42 binding checks in 8.043s. These cover five champions, 1v1/3v3 practice, two-player online play, inventory, abilities, persistence, conflicts, invalid saves, nested menus, typing focus, Tab navigation and narrow-screen layout. Run renderer-heavy suites sequentially: a concurrent run failed a fixed-delay keyboard assertion, while the final isolated suite passed.

One online draft run had an unconfirmed host pick; its cause was not established. Ten traced two-client trials passed in 19.098s without adding speculative retries. A definite presentation bug was corrected: a rejected or unconfirmed pick now resets the card and detail consistently and retains visible feedback until a fresh choice is sent or acknowledged. A controlled first-Pick-drop test passed eight assertions in 5.517s, proving no automatic resend, persistent feedback, and recovery through one fresh user click. `tools/league/draft-race.cjs --drop-first-pick` reproduces that case with a private real server.

The separate combat gallery passed 11 renderer checks in 19.386s and shows all five Q/W/E/R and auto families at two effect ages. Its snapshots and targets are authored fixtures, explicitly labelled as mocked; it is visual evidence, not a substitute for gameplay tests. Root and Fable reviewed the final scenes, including lifted Hallow ward/heal cues and stronger Maw claw/lunge cues. OpenCode confirmed its later e47bfbcb/5b8f8c5f branch is a historical snapshot; the integrated 14-action interface is authoritative (Barza 180–183).

V2 is not yet live at this candidate checkpoint. Release still requires main CI, a stamped League-only runtime update preserving the existing public tunnel, exact public artifact bytes, frozen v1 and other-game preservation, and public 1v1/3v3 probes. Procedural minions remain an optional v2.1 artwork improvement.

The first main CI run, 34054636493, passed core/server checks but caught a shared Pages assembler still hardcoded to League v1. The assembler now selects the validated live League catalog path and copies its complete UI/art tree while retaining other League versions. Targeted syntax (112 checks) and Pages (106 checks) suites passed in 24.512s using isolated local shims, including frozen v1, asset copies, and rejected invalid/duplicate live destinations. The scoped League publisher remains the release path; CI is rerun before any live runtime or Pages change.
