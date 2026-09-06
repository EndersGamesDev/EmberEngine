# Breach-12 presentation asset

`node tools/v31/build_shotgun.cjs` builds only the new `shotgun.glb` and
`shotgun-rig.json` in `crates/arena/assets`. The CPU-only Node generator lowers
itself to Idle priority; it does not require Blender, GPU services, downloads or
third-party assets. `node tools/v31/build_shotgun.cjs --check` compares the checked-in
bytes against a fresh in-memory export and validates the serialized GLB. Both
commands record a local report in ignored `target/shotgun-asset/results.json`.

## Runtime contract

- Weapon id: **8**, magazine-fed **Breach-12**. Gameplay capacity, ammunition,
  pellet count and timings remain authoritative in `arena-core`.
- Mesh nodes: `w_shotgun_body` and `w_shotgun_magazine`. Each has one textured
  primitive; tint it white like the existing viewmodels. No transform hierarchy
  or bone animation needs interpretation.
- Units: metres, +X forward, +Y up, +Z right. Origin is the shared AK hold frame.
- Reuse existing **grip id 3** (AK) glove meshes and sockets without modifying
  their vertices, materials or textures. No shotgun hands are baked into the GLB.
- Muzzle: `[0.86, 0.105, 0]`; sight: `[0, 0.165, 0]`.
- Magazine pivot: `[0.17, 0.015, 0]`. Its mesh remains in weapon coordinates;
  add `ReloadPose.magazine_offset` before the weapon transform. No pivot rotation
  is currently required. The magazine and support palm have the same relative
  movement during normalized reload progress `0.32..0.72`.
- Body and magazine retain first-person `without_shadow()` behavior; normal
  third-person parts use the existing rig and reach-constrained mount.

The pose cants the gun, grabs the box magazine, withdraws/reinserts it, works the
receiver and returns exactly to the original grip. Existing weapons receive a
zero magazine offset. The gunshot and reload cues are synthesized locally; one
shot cue must be emitted **per shell, not per pellet**. Appended sound enum values
preserve all legacy deterministic sound seeds.

## Geometry and provenance

No shotgun source archive was present in the local artist-source directory. This
is original authored geometry: long hollow large-bore barrel and support tube,
receiver and ejection port, charging handle, beveled fore-end, trigger guard,
stock/recoil pad, rail and iron sights, and a separate ribbed box magazine. Tiny
original RGB8 material swatches add visual separation and subtle surface grain;
they are not a claim of photoreal scanned or artist-painted PBR materials.

Export measurements: **287,632 bytes**, **2,968 triangles**, two named meshes,
one embedded **128 x 128 RGB8 PNG** and white base-color material factors. The
loader may upload the shared image once per part: approximately **175 KB** total
RGBA texture memory with mipmaps. Body bounds are `[-0.446,-0.139,-0.044]` to
`[0.86,0.178,0.085]` metres. The budget gate is under 5,000 triangles and 600 KB.

The generator checks that `viewmodel.glb`, `viewmodel-rig.json`,
`weapon-grips.glb` and `weapon-grips.json` are unchanged. Corrected export SHA-256:
`f2c2b7bab543d5b8ec0a60756fd029322347dcab7ff5329f5948e897c38ccf7b`.

## Verification boundaries

The generator verifies reproduction, GLB headers, names, triangle/texture budgets,
finite vertex data, normalized normals, valid UVs, metric bounds, RGB8 texture format and white
material factors. Pure Rust tests cover reload endpoints, finite transforms,
phase continuity, every weapon's distinct motion, palm/magazine tracking and
the shotgun's modest 1.25x ADS/heavy single-shot feedback. Root runs Cargo tests,
strict Clippy and actual engine/browser import checks serially.

Render checks must inspect first/third-person grip contact, stock clearance in
hip/ADS/recoil, front/rear iron-sight alignment, and magazine contact at reload
progress 0.32, 0.53 and 0.72. Serialized triangles must also leave the actual
float32 aim ray clear: the front bead ends 2 mm below the optical line, not on it.
Asset export checks alone do not prove the rendered grip/clearance criteria.
Reload audio is intentionally a short start-only cue (about 0.25 seconds): the
existing audio API cannot stop a whole sequence when reloading is canceled.
