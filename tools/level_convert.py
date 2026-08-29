"""Blender headless: the Free Fire "Lone Wolf" scene -> an engine level.

Produces two artifacts:
  assets/models/level.glb   one mesh per material (5 textures total)
  assets/models/level.json  collision boxes + arena bounds, in game units

The source ships merged instances (all cargo containers are one object
spanning the whole map), which are useless as collision volumes, so every
mesh is first separated into loose parts and each island contributes its
own axis-aligned box. Boxes low enough to jump onto are marked `step`.

Units: the source is ~0.65 m per unit (a cargo container measures 4.0
units and a real one is 2.6 m tall), so everything is scaled to metres,
centred on the playable middle of the street, floor at y = 0.

    blender --background --python tools/level_convert.py
"""

import json
import math

import bpy

SRC = r"C:\Users\end\dev\ember\assets\level\source\scene.gltf"
OUT_GLB = r"C:\Users\end\dev\ember\assets\models\level.glb"
OUT_JSON = r"C:\Users\end\dev\ember\assets\models\level.json"

SCALE = 0.65
# Half-length of the playable slice of the street, in game units.
ARENA_HALF_X = 46.0
ARENA_HALF_Z = 15.0
# A box no taller than this can be jumped onto.
STEP_MAX_H = 2.2
# Islands smaller than this in any horizontal axis are clutter, not cover.
MIN_BOX = 0.35


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=SRC)

    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    for o in meshes:
        me = o.data
        if not me.uv_layers:
            continue
        keep = me.uv_layers.active or me.uv_layers[0]
        for extra in [layer for layer in me.uv_layers if layer != keep]:
            me.uv_layers.remove(extra)
        keep.name = "UVMap"

    # Centre the street on the origin and scale to metres. The source is
    # Z-up in Blender; the Y-up glTF export maps (x, y, z) -> (x, z, -y).
    centre_x = 29.5  # middle of the built-up stretch, from level_inspect
    for o in meshes:
        o.scale = (SCALE, SCALE, SCALE)
        o.location = (
            (o.location.x - centre_x) * SCALE,
            o.location.y * SCALE,
            o.location.z * SCALE,
        )
    bpy.context.view_layer.update()

    # Loose parts: one island per real-world object.
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.separate(type="LOOSE")
    bpy.ops.object.mode_set(mode="OBJECT")
    parts = [o for o in bpy.data.objects if o.type == "MESH"]
    print(f"[level] {len(meshes)} objects -> {len(parts)} islands")

    boxes = []
    for o in parts:
        pts = [o.matrix_world @ v.co for v in o.data.vertices]
        if not pts:
            continue
        bx = [min(p.x for p in pts), max(p.x for p in pts)]
        by = [min(p.y for p in pts), max(p.y for p in pts)]
        bz = [min(p.z for p in pts), max(p.z for p in pts)]
        # Blender (x, y, z) -> engine (x, z, -y): y becomes up.
        emin = [bx[0], bz[0], -by[1]]
        emax = [bx[1], bz[1], -by[0]]
        size = [emax[i] - emin[i] for i in range(3)]
        if size[0] < MIN_BOX or size[2] < MIN_BOX or size[1] < 0.2:
            continue
        # Ignore geometry outside the playable slice (the source street
        # runs far past it and would be unreachable collision).
        if emin[0] > ARENA_HALF_X or emax[0] < -ARENA_HALF_X:
            continue
        if emin[2] > ARENA_HALF_Z or emax[2] < -ARENA_HALF_Z:
            continue
        # Floors and paper-thin decals are not obstacles.
        if emax[1] < 0.25:
            continue
        boxes.append(
            {
                "min": [round(v, 3) for v in emin],
                "max": [round(v, 3) for v in emax],
                "step": bool(emax[1] <= STEP_MAX_H),
            }
        )

    boxes.sort(key=lambda b: (b["min"][0], b["min"][2]))
    steps = sum(1 for b in boxes if b["step"])
    print(f"[level] {len(boxes)} collision boxes ({steps} jumpable)")

    # Render geometry: one object again, so glTF emits one primitive per
    # material instead of one per island.
    bpy.ops.object.select_all(action="DESELECT")
    for o in parts:
        o.select_set(True)
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.join()

    bpy.ops.export_scene.gltf(
        filepath=OUT_GLB, export_format="GLB", export_yup=True, use_selection=True
    )
    with open(OUT_JSON, "w", encoding="utf-8") as f:
        json.dump(
            {
                "scale": SCALE,
                "arena": {"half_x": ARENA_HALF_X, "half_z": ARENA_HALF_Z},
                "step_max_h": STEP_MAX_H,
                "boxes": boxes,
            },
            f,
            indent=1,
        )
    print(f"[level] wrote {OUT_GLB} and {OUT_JSON}")


main()
