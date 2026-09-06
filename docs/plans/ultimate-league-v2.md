# UltimateLegue v2 — Crystalforge

The owner requested a visual overhaul using the LAN asset fleet, coordinated with Fable and OpenCode through Barza. Champions are generated and reviewed in order: SW4RM, EmberKnight, The Hallow One, Bog Maw, Tessera the Clockmaker; environment generation follows the five champions. The game remains on Ember, with authoritative online 1v1/3v3 and local practice.

The original visual direction is Crystalforge: weathered ivory, aged bronze, luminous turquoise and amber crystal, carved geometric motifs, and an elevated garden arena. Strong silhouettes, clearly visible team affiliation, readable combat effects, and legible controls take priority over ornamental density.

## Ownership and interfaces

Root integrates on `codex/ultimate-league-v2`, drives the single SDXL queue, reviews each image/mesh, owns portrait derivatives and provenance, verifies the combined build, and releases it. Fable owns `scene.rs`, `scene/art.rs`, converted GLBs/sidecars and conversion tooling on `fable/league-v2-scene`. OpenCode owns `web/games/league/v2/index.html` and its UI modules on `opencode/league-v2-ui`. Assignments and progress are on Barza messages143 onward; durable contracts are kept here and in commits.

The scene appends meshes after procedural ids1–9 through `scene::build_meshes()`. Assets use embedded 8-bit base-color textures, white instance tint, and scalar surface response; vertex colors and imported PBR maps are not consumed by Ember. Generated sources remain in `target/league-art`; converted GLBs and small web derivatives ship. Target added mesh/texture payload is8MiB, with512px atlases and roughly3k–6k triangles per champion.

The client provides `bindings_json()`, `set_bindings_json(string)`, `binding_options_json()`, and `set_input_enabled(bool)`. Fourteen actions use physical `KeyboardEvent.code` values: `q,w,e,r,d,f,item1,item2,item3,item4,item5,item6,stop,shop`. Defaults are QWER, DF,1–6,S,B. Empty JSON resets; partial maps merge over defaults; invalid/duplicate maps are rejected atomically. Rust dispatches gameplay controls; DOM dispatches the configured shop key. Shift/Ctrl plus the mapped ability learns a rank. Escape is reserved for menus. The page persists validated bindings and suppresses gameplay during settings/key capture/text entry.

## Fleet and release constraints

SDXL runs on specht32:8188; root submits one image job at a time with the reproducible `tools/league/fleet-art.py` runner. Fable owns the TripoSR queue on knecht24 GPU0, using the worker's explicit GPU selection so the separately claimed MMAudio GPU remains undisturbed. No profile switching is required. TRELLIS health alone does not prove that its gated image encoder is usable.

V1 remains a frozen versioned web build. V2 is published at `games/league/v2/` and becomes the hub default only after integrated browser and public-host verification. Keep the existing League public tunnel alive across any League server update; preserve all other games, processes, tasks, address-book fields and Pages trees. Knecht host services remain inactive. All builds run at Idle priority, and validation records measured wall times.

## Verification pending

The first SDXL output completed in15.16s; the reviewed SW4RM refinement completed in14.52s. This establishes image generation, not completed mesh import or in-game visual quality. Final verification must cover all five imported champions, the arena, local/online play, persisted/remapped controls, input suppression, responsive UI, console errors, download sizes and exact public artifacts. The v2 release is not yet live.
