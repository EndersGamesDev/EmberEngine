"""Blender headless: inspect the imported level scene — per-object world
AABBs and materials — so the engine can be given collision boxes and a
sane world scale.

    blender --background --python tools/level_inspect.py
"""

import json

import bpy

SRC = r"C:\Users\end\dev\ember\assets\level\source\scene.gltf"
OUT = r"C:\Users\end\AppData\Local\Temp\claude\C--Users-end-dev\32743292-6b13-4755-bd6e-1a900e19330d\scratchpad\level_objects.json"

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=SRC)

objs = []
for o in bpy.data.objects:
    if o.type != "MESH":
        continue
    corners = [o.matrix_world @ v.co for v in o.data.vertices]
    if not corners:
        continue
    xs = [c.x for c in corners]
    ys = [c.y for c in corners]
    zs = [c.z for c in corners]
    objs.append(
        {
            "name": o.name,
            "tris": len(o.data.polygons),
            "mats": [m.name for m in o.data.materials if m],
            # Blender Z-up world AABB.
            "min": [min(xs), min(ys), min(zs)],
            "max": [max(xs), max(ys), max(zs)],
        }
    )

objs.sort(key=lambda d: -d["tris"])
with open(OUT, "w", encoding="utf-8") as f:
    json.dump(objs, f, indent=1)

print(f"[level] {len(objs)} mesh objects")
for d in objs[:40]:
    size = [round(d["max"][i] - d["min"][i], 2) for i in range(3)]
    print(f"[obj] {d['name'][:38]:38s} tris={d['tris']:6d} size={size} mats={d['mats']}")
print(f"[level] wrote {OUT}")
