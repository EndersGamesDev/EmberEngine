# Weapon grips and arm attachment (Arena v23)

The weapon and its fingers share one transform in both first and third person. Each authored gun has separate left and right gloved-hand meshes, baked from the existing SWAT artist rig after posing the individual finger joints in Blender. The body reaches the exported wrist sockets with a two-bone arm solver, layered after walking and crouching. This is a presentation change; Arena protocol 17, hitboxes, weapon statistics and authoritative ballistics are unchanged.

## Asset contract

`tools/v23/build_grips.py` writes `crates/arena/assets/weapon-grips.glb` and `weapon-grips.json`. Source archives remain in the ignored artist asset directory; pass `--source-root C:/Users/end/dev/ember/assets` when building from an isolated checkout. Run Blender in background at Idle priority. No additional downloaded models, runtime services or paid toolkits are needed.

The GLB has twelve meshes: `grip_1_r/l`, `grip_2_r/l`, `grip_3_r/l`, `grip_5_r/l`, `grip_6_r/l`, and `grip_7_r/l`. All vertices and sidecar sockets are in the existing weapon frame, +X forward, +Y up, +Z right, in metres. Do not centre these meshes individually on import. The M4 currently has no separate gun mesh and deliberately selects the sidearm's weapon, muzzle and grip together. Loot weapons do not draw gloves.

The sidecar supplies each mesh name, wrist, palm, authored orientation, hand proportions and finger-target audit. A fingertip-target distance is a fitting diagnostic, not proof of a collision-free or anatomically perfect finger. Inspect the actual mesh from both sides and through the game camera. The original weapon geometry and muzzle sockets are unchanged.

`tools/v23/build_sleeves.py` reconstructs the existing SWAT split arm meshes, welds their old boundaries, clips at measured joint planes with overlap, and caps the resulting boundaries. `weapon-sleeves.glb` replaces four arm parts and the torso's shoulder borders in the original bind frame. The same closed textured sleeves are used for the remote body and first-person arms; the torso repair removes inherited shoulder spikes exposed by the new pose, while preserving the lower vest and collar. Its verifier requires closed manifold sleeves, and the torso's new shoulder openings are capped. Sleeve UV islands use a 512×512 atlas; the torso preserves its separate original 1024×1024 atlas. Skeleton anchors and lengths are unchanged.

The glove atlas is one embedded 512×512 8-bit PNG with white material colour. The renderer still allocates a texture per mesh, so twelve gloves consume approximately 16 MiB including mipmaps despite sharing one image in the file. Use `Vec3::ONE` for the glove material. Source finger skinning is evaluated offline; the runtime draws the resulting posed meshes and does not yet animate individual finger bones.

## Runtime

`grips::mount` derives the gun's placement from the posed shoulders and projects both wrist sockets into the overlap of the arms' reachable regions. Crouch and locomotion therefore move the gun with the body. Gun pitch stays on the existing aim axis. `grips::pose_arms` converts world sockets back into character space and calls the shared engine's `rig::solve_arm`; only arm rotations change, never bone lengths, simulation state or leg animation. The optional shield occupies the support hand and remains upright.

`ShooterGame::drawn_muzzle` uses the same mount as the render pass. Remote shot visuals are queued until this frame's interpolation, gait and crouch pose have advanced, avoiding a one-frame muzzle lag; audio and authoritative impacts retain their event timing. A flash or tracer must not reconstruct the old box-body attachment, which placed the weapon beyond the operator's reach. The original relaxed wrist/hand parts are omitted using a cached body-part list when the replacement gloves are drawn. First-person sleeves use fixed 27 cm forearm and 29 cm upper-arm segments and follow the camera toward the glove wrists.

## Verification and limits

Run `cargo test -p arena -p ember-engine -p arena-core -p arena-server` and all-target Clippy at Idle priority. Asset-backed tests check every rendered weapon's selection, valid textures, metric geometry, 3,600 arm-reach cases, unchanged limb lengths and unrelated joints, and frame-level agreement between the actual gun mesh, gloves and effect muzzle during interpolation, aiming and crouching.

`tools/v23/browser-grips.cjs` captures the real WASM renderer and embedded assets headlessly. Its default 35 views cover all seven weapon IDs in first person and remote front/side/crouch/upward aim; optional views cover downward aim and the remote shield. Keep before and after output directories separate and inspect full-resolution captures, not only contact sheets. These are rendering-only protocol fixtures, not live networking, firing or input tests. They never move the workstation's mouse, type keys or take the foreground. Details are in `tools/v23/README.md`.

This fixes static grip placement and connected arms. It is not a complete modern-shooter animation set: reloads still lower the weapon rather than exchanging a magazine by hand, the trigger finger does not animate on each shot, there is no weapon collision/retraction pass, and the scene renderer still lacks normal/roughness maps and runtime skeletal skinning. Existing source mesh and texture detail set the visual ceiling; these changes do not establish AAA fidelity.

Provenance: these are derivatives of the operator already shipped in this repository, not newly downloaded third-party models. The local source archive does not contain a verifiable author/license record; asset provenance remains an inherited follow-up, not a verified licensing claim.

## Release verification — 2026-09-05

Verified: 329 relevant native tests passed (three existing ignored tests were not run); all-target Arena/engine Clippy passed; five release WASM bundles built. The final torso-change Arena test/Clippy/WASM gate took 13.34 seconds. The actual-game six-view shoulder checkpoint passed in 10.91 seconds; the 35-view release-candidate capture passed in 53.67 seconds and the remote shield/downward-aim pair in 5.57 seconds, with no browser errors. First-person grips, shoulder joins and remote posed silhouettes were inspected from the captures. The published Arena WASM is 41,405,099 bytes, SHA-256 `540e73c5f57bb976ddbbbc72534439d151b60046fa31fa487d3228f2f7e1a4b4`; the before build was 39,314,078 bytes. The release candidate differed only in sidecar JSON line endings; source formatting was normalized to LF and the rebuilt publication bundle received an additional eight-view browser smoke check. Captures remain ignored under `target/grip-final`, `target/grip-final-extra` and `target/grip-publish-check`, with their states and hashes in `results.json`.

Publication fixtures passed 41/41 and shell syntax checks 91/91. The existing public protocol-17 server passed an 89-state, no-firing WebSocket smoke run in 3.25 seconds; it was not restarted because the change does not alter server code or simulation. Not verified: real-controller input, hand-animated reloads, dynamic per-finger firing animation, or a frame-rate benchmark. The browser fixture is rendering evidence, not an online gameplay test.
