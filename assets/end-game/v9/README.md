# V9 castle environment props

Three generated static props add carved detail around the authored basement, great-room, courtyard and tower route. Each GLB has one primitive with a white base-color factor and one embedded 768 × 768 RGB8 atlas. The actual atlas pixels carry warm weathered limestone color; the dry basin has no water or transparent material. Use white instance color and scalar roughness 0.9 / metallic 0.0. Register each model once and reuse its mesh ID for instances.

| File | Triangles | Unit-scale size X × Y × Z | Placement |
|---|---|---|---|
| `gothic-pillar.glb` | 9,456 | 2.42 × 4.40 × 2.73 m | Solid decorative cluster at great-room wall bays, outside traversal corridors |
| `courtyard-fountain.glb` | 11,996 | 2.26 × 1.15 × 2.25 m | Dry carved basin; reserve about 1.35 m radius for an authored collider |
| `tower-doorway.glb` | 11,477 | 3.45 × 6.01 × 2.70 m | Carved portal, see the placement contract below |

The anchors are centered at ground contact, +Y is up, and the front faces -Z. Exact bounds, source jobs, hashes and conversion evidence are in the corresponding JSON sidecars. Combined geometry is 32,929 triangles; combined texture storage including all mip levels is 9,437,160 bytes. All three exported GLBs fit below 6 MB.

Place the tower portal at uniform scale 1.25 with its anchor 0.40 m below the floor. Rotate +90 degrees around Y for the west-facing tower entry. This placement passed 646 visual mesh rays through a 0.90 m wide corridor from 0.05 to 1.70 m above the floor, and another sampled 0.72 m corridor reaching 2.0 m. The sampled rays and transform are recorded in `tower-doorway-clearance.json`. Author jamb/lintel collision around the opening; a full AABB would block the route. The Gothic support's smaller holes are decorative and are not a player passage.

Raw concepts, 16–17 MB source meshes, conversion logs, six passive Blender views, material-atlas views and the full outside-checkout manifest remain in `C:/Users/end/dev/end-game/runtime-v9-assets`. Reproduction scripts are in `tools/end-game/v9/`. The three concept and TRELLIS jobs used existing fleet services and their reservations were released after generation.

Verified during asset preparation: actual embedded RGB8 texture format and mip budgets, one primitive per model, finite positions/normals/UVs, valid indices, nondegenerate triangles, file hashes, six front/back passive Blender previews and sampled portal visibility clearance. These decorative generated meshes retain some broken/nonmanifold edges and baked occlusion. They are not watertight physics assets. Gameplay collision and native/browser integration are separate runtime checks.
