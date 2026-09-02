import sys, os, bpy
from mathutils import Vector
path = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)
try:
    bpy.ops.wm.collada_import(filepath=path)
except Exception as e:
    print("[inspect] collada import failed:", e); raise SystemExit(1)
print(f"[inspect] {path}: {len(bpy.data.objects)} objects, {len(bpy.data.images)} images, {len(bpy.data.materials)} materials")
for o in bpy.data.objects:
    line = f"  {o.type:9s} {o.name!r} parent={o.parent.name if o.parent else None} scale={tuple(round(s,4) for s in o.scale)}"
    if o.type == "MESH":
        pts = [o.matrix_world @ v.co for v in o.data.vertices]
        if pts:
            lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
            hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
            line += f" verts={len(pts)} faces={len(o.data.polygons)} min={tuple(round(c,3) for c in lo)} max={tuple(round(c,3) for c in hi)}"
        line += f" uv={[l.name for l in o.data.uv_layers]} mats={[m.name if m else None for m in o.data.materials]}"
    print(line)
for m in bpy.data.materials:
    imgs = []
    if m.use_nodes and m.node_tree:
        for n in m.node_tree.nodes:
            if n.type == "TEX_IMAGE" and n.image: imgs.append((n.image.name, tuple(n.image.size)))
    print(f"  material {m.name!r}: images={imgs}")
for im in bpy.data.images: print(f"  image {im.name!r} size={tuple(im.size)} path={im.filepath!r}")
