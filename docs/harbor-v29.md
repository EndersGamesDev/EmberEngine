# Breakwater Harbor v29: authored vertical routes

The 96 × 96 m terminal keeps its original eight screened spawn pockets, three ground-route families, central hill, cargo stacks, warehouse, working quay, two cranes and moored ship. The upgrade adds three enterable service buildings, three ordinary stairways, two warehouse roof bridges and three four-metre wall-jump lanes. The existing spawn screens rise from 2.8 m to 3.4 m so the newly reachable warehouse roof edges and clerestory do not expose the spawn centres; their footprints and rear exits are unchanged. There is no random placement, new downloaded asset, runtime toolchain or added rendering feature.

The release this map shipped in is the `v29` entry in `CHANGELOG.md`.

## Shared geometry is the authority

All dimensions and placements live in `crates/arena-core/src/harbor.rs`; `crates/arena/src/harbor.rs` draws each solid from those same `Obstacle` bounds and adds only shallow surface dressing. There are 160 obstacles: the original 75 plus 30 building solids, 42 stair treads, six bridge solids, the warehouse clerestory, an east pump-house screen and five roof vents. Five remain transient loot boxes, excluded from the contact-occlusion bake; including its two bounded floor boxes gives 157 source occluders. The map bounds and occlusion volume resolution are unchanged.

| Feature | World X/Z bounds | Playable heights and use |
|---|---|---|
| North dispatch office | X −24…−18, Z −24.8…−14.8 | Ground interior with opposite 2.3 m × 2.6 m door openings; roof top 4.55 m |
| South dispatch office | X −24…−18, Z 14.8…24.8 | Opposite doors, ground interior and 4.55 m roof; mirrors the northern access choice |
| Pump house | X 19…27, Z −9…−3 | Opposite doors, ground interior and 4.55 m roof overlooking the east cross street |
| North/south office stairs | X −17.9…−16.5; Z −24.8…−13.6 / 13.6…24.8 | Fourteen 0.325 m risers, each 0.8 m deep; rise toward the outer end of each office |
| Pump-house stairs | X 17.6…19, Z −14.2…−3 | Same fourteen-riser walking route, rising toward positive Z |
| North/south roof bridges | X −28.5…−23.5; Z −18.8…−16.6 / 16.6…18.8 | Deck base 4.27 m, top 4.55 m; actual solid waist parapets leave 1.9 m clear width |
| Warehouse clerestory | X −39.5…−32.5, Z −25.5…25.5 | Original roof silhouette now real raised cover, base 4.55 m and top 5.4 m |
| Pump-house screen | X 31…31.4, Z −9…−3 | 4.2 m wall; paired with the house's east face at X 27 |

Each new roof has a non-parkour walking route. The west bridges link both office roofs to the warehouse roof without closing the service lane underneath. The warehouse ridge separates eastern and western roof lanes, with counter-flanks around both ends. Five 0.8 m-high solid vent boxes provide local roof cover. The old ammo/crate/container climbing chains remain in place.

## Wall-jump lanes and safe approaches

`WALL_JUMP_LANES` contains centreline samples `[-26, -21]`, `[-26, 21]` and `[29, -6]`. All three gaps are four metres between visible solid faces, with wall tops at 4.2 m. The west lanes use the warehouse east face X −28 and office west face X −24; the eastern lane uses pump-house east face X 27 and screen west face X 31. A radius-0.6 m body contacts the western lane at centre X −27.4 or −24.6. Alternate wall normals are required by shared parkour; the map does not add a separate movement rule.

Shared parkour requires a wall face at least 1.3 m wide along its tangent, slightly wider than the standing body's 1.2 m diameter. Consequently the crane's 1.2 m-square isolated posts remain real collision/shot cover but are not wall-slide or wall-jump surfaces. This prevents an unintended twelve-metre post-to-post ascent into the non-traversable decorative upper gantry; the broad authored office, warehouse and pump-house lane faces retain wall grip. Ship and upper-crane dressing are not advertised as playable platforms.

From FFA spawn 0 at `[-37, 42]`, a clear ordinary ground approach to the southern west lane is `[-37, 45.4] → [-31, 45.4] → [-26, 45.4] → [-26, 21]`. This exits the spawn screen at the rear before entering the lane. Ground routes through both offices, the pump house, the warehouse, the cargo cross streets and the quay remain mutually connected. The old cross-links at Z ±35.5 and the uninterrupted X 38 quay route are preserved.

## Rendering and budget contract

The renderer still registers eleven Harbor meshes and five batched scenery instances. Existing base-colour textures and material presets are reused; no asset download or texture allocation is added. The new painted glazing is opaque and lies on solid side walls, never in the actual door openings. Roof fascia projects only 12 mm to avoid coplanar flicker; stair nosing paint and bridge caps are similarly shallow. Solid waist parapets are deliberately not depicted as open railings over an invisible collision box. Closed glazing is not intended to be a traversable window.

Each interior and its two doorway thresholds have a 20 mm-high concrete finish over the implicit ground. These nine upward quads cover the old 18 mm-high road markings without adding a collision step, mesh, texture or draw call. Their bounds derive from the actual interior wall faces and lintel footprints; the explicit floor-only test checks ground backing without weakening the wall/roof/trim collision checks.

Native art invariants retain the existing limits of fewer than 200,000 vertices and 4 MB of base texture pixels across Harbor meshes, verify non-container rendered envelopes against their exact colliders, and require every added surface-detail vertex to remain within 4 cm of an authoritative solid. First-person hands, weapons, the sky, water, contact-shading implementation and all other maps are unchanged by these two map modules.

## Verification handoff

Map tests cover seed-independent authored geometry, all eight screened spawn centres, connected ground routes and loot access, real door/lintel/roof collision, all fourteen risers of each ordinary roof approach, both bridges and warehouse counter-flanks, paired-wall clearances and selected roof-to-spawn/counter sightlines. Stair test waypoints are 65 cm back from the ascending tread edge: a 60 cm body radius would already be supported by the next riser at the tread's midpoint. Exact support heights are checked; the route test does not relax its required final height or teleport onto a roof. The physics module separately exercises actual alternating wall kicks on the three authored lane samples.

These tests do not prove every possible combat angle, roof-edge jump or multiplayer balance decision. Full native/Clippy gates, the eight-player authoritative network run and actual WebGL2 screenshot review are release-worker responsibilities; record their measured outcomes in the release report rather than treating this design document as an execution claim. No Cargo or browser run was launched by the map-art worker.

Useful visual QA cameras, expressed as world-space eye → target:

- North office interior: `[-21, 1.6, -18] → [-21, 1.7, -24.8]`.
- North stair exterior: `[-12, 6, -20] → [-19, 2.8, -20]`.
- North roof bridge: `[-20, 6, -17.7] → [-32, 5, -17.7]`.
- Warehouse roof counter-lane: `[-30, 6, -26.6] → [-40, 5, -20]`.
- Pump-house court: `[29, 1.6, 1] → [24, 3, -6]`.
- Eastern wall-jump lane: `[29, 2, -11] → [29, 4, -5]`.
