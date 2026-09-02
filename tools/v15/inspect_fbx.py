"""Blender headless: print what an FBX/OBJ/glTF actually contains.

    blender --background --python tools/v15/inspect_fbx.py -- <file>

Objects, types, parents, world bounds, materials and their image nodes,
UV layers, armature bones (name, parent, head), and embedded images.
"""
import sys, os
import bpy
from mathutils import Vector

path = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)
ext = os.path.splitext(path)[1].lower()
if ext == ".fbx":
    bpy.ops.import_scene.fbx(filepath=path)
elif ext == ".obj":
    bpy.ops.wm.obj_import(filepath=path)
elif ext in (".glb", ".gltf"):
    bpy.ops.import_scene.gltf(filepath=path)
else:
    raise SystemExit(f"unknown extension {ext}")
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
        line += f" vgroups={len(o.vertex_groups)} mods={[m.type for m in o.modifiers]}"
    print(line)
    if o.type == "ARMATURE":
        bones = o.data.bones
        print(f"    armature {o.name!r}: {len(bones)} bones")
        for b in bones:
            print(f"      {b.name!r} parent={b.parent.name if b.parent else None} head={tuple(round(c,3) for c in (o.matrix_world @ b.head_local))} len={round(b.length,4)}")
for m in bpy.data.materials:
    imgs = []
    if m.use_nodes and m.node_tree:
        for n in m.node_tree.nodes:
            if n.type == "TEX_IMAGE" and n.image:
                imgs.append((n.image.name, tuple(n.image.size), n.image.filepath))
    print(f"  material {m.name!r}: images={imgs}")
for im in bpy.data.images:
    print(f"  image {im.name!r} size={tuple(im.size)} packed={im.packed_file is not None} path={im.filepath!r}")
