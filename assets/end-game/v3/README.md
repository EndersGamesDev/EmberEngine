# V3 articulated wolf hands and matching items

Original procedural geometry and deterministic RGB textures authored for the End Game first-person reach, key pickup, lock use, and sword grasp. The hands follow the project's blackened iron, overlapping plate, leather palm, rivet, and restrained wolf-motif brief. No third-party meshes or bitmap inputs are embedded. No reserved fleet generator, remote image/video service, engine source, repository state, or publication was touched by this asset work. The generator uses Blender 5.2.1 CPU rendering with four threads at Idle priority.

| Runtime file | Bytes | Triangles | Nodes / primitives |
|---|---:|---:|---:|
| `wolf-hands.glb` | 487,884 | 9,358 | 34 / 34 |
| `iron-key.glb` | 36,188 | 532 | 1 / 1 |
| `wolf-greatsword.glb` | 119,976 | 2,568 | 1 / 1 |
| Total | 644,048 | 12,458 | 36 / 36 |

Each GLB embeds one shared 128×128 RGB8 PNG, with an opaque material and white base-color factor. The engine clones textures per primitive, so the hands use about 2.83 MiB of texture memory including mipmaps despite sharing one embedded image. The hand geometry expands to 28,074 vertices in the flat loader. Source material metallic/roughness values are only for the Blender previews; the engine controls its own scalar surface response. Use white instance color to preserve the atlas.

## Coordinate and loader contract

All units are meters. Hand bind coordinates are X finger-forward, Y dorsal/up, Z right. The right thumb extends toward negative Z and the left thumb toward positive Z. Both sides share wrist origin `(0,0,0)`, elbow `(-0.27,0,0)`, and shoulder `(-0.57,0,0)`. Upper arm length is 0.30 m and forearm length is 0.27 m. All exported nodes have identity transforms, no parents, one primitive, and all positions already in the common bind coordinate frame. The left geometry is mirrored into its own vertex data with reversed face winding; runtime negative scaling is unnecessary.

The combined hand file bounds are X `[-0.581,0.186]`, Y `[-0.0538042,0.0538042]`, Z `[-0.1143198,0.1143198]`. Each side has 17 parts: `hand_{r,l}_upperarm`, `forearm`, `palm`, `thumb_0`, `thumb_1`, and `{index,middle,ring,pinky}_{0,1,2}`. The actual full node names, bind pivots, parent names, finger endpoints, segment lengths, and axes live in `hands-rig.json`; `manifest.json` reports per-node bounds and exported byte hashes.

The JSON hierarchy is topologically ordered. `presets` use short suffixes such as `index_0`, shared by both sides. A number is a curl angle in radians; a record holds `curl` and optional `opposition`. Positive finger curl closes toward the palm. Left axial vectors are already reflected as pseudovectors, so apply the same positive angles on each side. For column-vector matrices, evaluate `Dchild = Dparent * T(bind_pivot) * R(opposition_axis, opposition) * R(curl_axis, curl) * T(-bind_pivot)`. Missing angles are zero. Draw a finger part using `wrist_world * Dchild` on its bind-coordinate vertices. When using separate shoulder/elbow/wrist IK, solve the two arm segments independently, use `position = joint_world - rotation * bind_pivot` for each arm mesh, and use the solved wrist frame as the parent of palm/finger deformation. Avoid applying an IK transform twice through the serialized parent chain.

## Contact anchors and checked poses

`right-open-detail.png` shows the five distinct fingers and articulated plates. `hands-open.png` shows both mirrored arms separated only for the preview. `right-power-grip.png` shows the final power preset around a real 28 mm cylinder proxy. `right-key-pinch.png` shows the final pinch preset with the exported key; thumb and index oppose across the ring rim while the other fingers stay relaxed. These are Blender previews, not claims of native Ember validation.

The right power-grip center is `(0.101,-0.030,0)`, with cylinder axis approximately `(-0.220003,0,0.975499)`, radius 0.014 m. The left point is the same and its axis is the Z reflection. The slight handle slope accommodates differing finger lengths. The little finger's final curl angles are `[0.30,0.95,0.60]`; the earlier tighter draft intersected the proxy and must not be used.

The right pinch item anchor is the key ring center `(0.091,-0.043,-0.044)` and the opposing finger contact is near its positive-X rim `(0.107,-0.043,-0.044)`. Both left Z coordinates are positive. In the checked right preview, the key has identity orientation relative to the hand bind frame. To reach an item naturally, place the wrist at `world_item_anchor - wrist_rotation * local_hand_contact`; interpolate the arm reach and curl toward that contact, then preserve the item-to-hand transform while held.

The key node is `iron_key`, with ring center and item grip anchor at `(0,0,0)`, shaft along +X, ring plane XY and normal +Z. Its ring centerline radii are 0.016 × 0.018 m with a 0.0032 m tube; its full bounds are X `[-0.0187713,0.1395]`, Y `[-0.028,0.0207713]`, Z `[-0.0077119,0.0077119]`. The shaft and bit project away from the contact rim in the pinch preview.

The companion sword node is `wolf_greatsword`. It has blade direction +X, thickness along Y, blade width along Z, guard at origin, and tip `(1.85,0,0)`. The grip runs X `[-0.32,-0.025]`, with 0.014 m working radius and 0.0142 m raised wrap radius. Use primary grip `(-0.13,0,0)` and secondary grip `(-0.25,0,0)`; the geometric grip center is `(-0.1725,0,0)`. The sidecar `greatsword-rig.json` supplies these anchors. Align weapon +X to the hand power axis, choose the weapon roll, and place `weapon_translation = world_hand_contact - weapon_rotation * primary_grip`. Sword full bounds are X `[-0.395,1.85]`, Y `[-0.03,0.03]`, Z `[-0.212,0.212]`; the blade itself is 0.26 m wide and 0.026 m thick, with a blunt beveled tip and a wider crossguard. `wolf-greatsword.png` is the checked standalone preview. The original V1 sword remains intact.

## Reproduction and verification

From the repository root, set `END_GAME_V3_ASSET_WORK` to an existing scratch directory. Run `tools/end-game/v3_generate_atlas.py` with Python, then `v3_build_hands.py` and `v3_build_sword.py` with Blender background mode at Idle priority and four threads. Run `v3_verify_assets.py` with Python. The editable Blender files and PNG previews stay in that scratch directory. The scripts are the authoritative pivot and pose source.

`v3_verify_assets.py` reads the GLB bytes and checks node names, identity transforms, one primitive per node, finite positions/UVs, normalized normals, normal-to-winding agreement, zero degenerate triangles, positive signed volumes for closed geometry helpers, opaque white-factor materials, actual embedded PNG RGB8 encoding, and size/triangle budgets. It also checks rig parent order and mirrored grip axes. All checks passed on the delivered exports. The latest hands rebuild took 44.76 seconds and the sword rebuild 8.25 seconds; exported-byte verification took about 0.03 seconds. Native renderer appearance, solved arm reach, attachment motion over time, wall clipping, and per-instance lighting remain the parent integration task's verification responsibilities.
