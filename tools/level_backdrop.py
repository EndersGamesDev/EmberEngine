"""Blender headless: build the arena's skyline from the Free Fire "Lone
Wolf" factory street.

The source is a straight street of near-identical factory blocks. The
arena is a square, so instead of importing the street as-is this takes a
few of its buildings and rings them around the arena facing inward — the
fight happens in a factory courtyard instead of an empty basalt box.

Only the building shells are kept (one material, one texture), so the
result is small enough to embed in the wasm build.

    blender --background --python tools/level_backdrop.py
"""

import math

import bpy

SRC = r"C:\Users\end\dev\ember\assets\level\source\scene.gltf"
OUT = r"C:\Users\end\dev\ember\assets\models\level-backdrop.glb"

# Must match arena_core::shooter::ARENA_HALF.
ARENA_HALF = 24.0
# Source units -> game units (a cargo container measures 4.0 units and a
# real one is 2.6 m tall).
SCALE = 0.65
# How many blocks to ring the arena with, and how far out their centres
# sit: far enough that the shells never intrude on the play space.
COUNT = 8
# A block is ~19 deep and ~34 wide once scaled, and sits tangentially, so
# its near face lands at RING_R - 9.5: comfortably outside the wall.
RING_R = ARENA_HALF + 16.0
KEEP_MATERIAL = "Factory_Building001"


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=SRC)

    # The importer parents everything under a rotated root (glTF is Y-up,
    # Blender is Z-up). Flatten that away so each mesh's own coordinates
    # ARE its world coordinates — the placement maths below depends on it.
    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = next(
        o for o in bpy.data.objects if o.type == "MESH"
    )
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    bpy.ops.object.select_all(action="DESELECT")
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        o.select_set(True)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    buildings = []
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        mats = [m.name for m in o.data.materials if m]
        if any(KEEP_MATERIAL in m for m in mats):
            buildings.append(o)
        else:
            bpy.data.objects.remove(o, do_unlink=True)
    if not buildings:
        raise SystemExit("no building meshes found")
    print(f"[backdrop] {len(buildings)} source buildings")

    for o in buildings:
        me = o.data
        if me.uv_layers:
            keep = me.uv_layers.active or me.uv_layers[0]
            for extra in [layer for layer in me.uv_layers if layer != keep]:
                me.uv_layers.remove(extra)
            keep.name = "UVMap"

    # One block is the template; the rest of the street is discarded.
    template = buildings[0]
    for o in buildings[1:]:
        bpy.data.objects.remove(o, do_unlink=True)

    # Put the template's own origin at the middle of its footprint and its
    # base on the floor, so ring placement is exact.
    pts = [template.matrix_world @ v.co for v in template.data.vertices]
    cx = (min(p.x for p in pts) + max(p.x for p in pts)) * 0.5
    cy = (min(p.y for p in pts) + max(p.y for p in pts)) * 0.5
    cz = min(p.z for p in pts)
    for v in template.data.vertices:
        v.co.x -= cx
        v.co.y -= cy
        v.co.z -= cz
    template.location = (0.0, 0.0, 0.0)
    template.rotation_euler = (0.0, 0.0, 0.0)
    template.scale = (SCALE, SCALE, SCALE)

    # Distant scenery, and the wasm build embeds it: a quarter of the
    # source density still reads as a factory block at arena range.
    bpy.context.view_layer.objects.active = template
    dec = template.modifiers.new("decimate", "DECIMATE")
    dec.ratio = 0.25
    bpy.ops.object.modifier_apply(modifier=dec.name)
    print(f"[backdrop] template decimated to {len(template.data.polygons)} faces")

    made = []
    for i in range(COUNT):
        a = i / COUNT * math.tau
        obj = template.copy()
        obj.data = template.data.copy()
        obj.name = f"backdrop_{i}"
        # Blender is Z-up here; the Y-up glTF export turns this ring into
        # the arena's XZ plane.
        obj.location = (math.cos(a) * RING_R, math.sin(a) * RING_R, 0.0)
        # Turn the block's long axis tangential, so the ring reads as a
        # continuous facade and no wall points at the arena.
        obj.rotation_euler = (0.0, 0.0, a)
        bpy.context.scene.collection.objects.link(obj)
        made.append(obj)
    bpy.data.objects.remove(template, do_unlink=True)

    bpy.ops.object.select_all(action="DESELECT")
    for o in made:
        o.select_set(True)
    bpy.context.view_layer.objects.active = made[0]
    # Bake each block's placement into its vertices first. Joining without
    # this leaves the whole ring expressed inside the first block's rotated
    # local space, and the exporter's up-axis conversion then tips the
    # skyline onto its side.
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.join()
    joined = bpy.context.active_object
    print(f"[backdrop] {len(joined.data.polygons)} faces in the ring")

    bpy.ops.export_scene.gltf(
        filepath=OUT, export_format="GLB", export_yup=True, use_selection=True
    )
    print(f"[backdrop] wrote {OUT}")


main()
