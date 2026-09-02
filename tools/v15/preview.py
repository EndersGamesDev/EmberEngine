"""Blender headless: render a quick Workbench turnaround of a model.

    blender --background --python tools/v15/preview.py -- <model> <out_prefix>

Four 800x800 views (front/back/left/right in Blender's frame) with a
studio-lit solid shading, so a mesh with no textures still reads.
"""
import sys, os, math, bpy
from mathutils import Vector
args = sys.argv[sys.argv.index("--") + 1:]
path, out = args[0], args[1]
bpy.ops.wm.read_factory_settings(use_empty=True)
ext = os.path.splitext(path)[1].lower()
if ext == ".fbx": bpy.ops.import_scene.fbx(filepath=path)
elif ext == ".dae": bpy.ops.wm.collada_import(filepath=path)
elif ext in (".glb", ".gltf"): bpy.ops.import_scene.gltf(filepath=path)
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
pts = [o.matrix_world @ v.co for o in meshes for v in o.data.vertices]
lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
c = (lo + hi) * 0.5; r = max(hi - lo) * 1.1
sc = bpy.context.scene
sc.render.engine = "BLENDER_WORKBENCH"
sc.display.shading.light = "STUDIO"; sc.display.shading.color_type = "TEXTURE"
sc.render.resolution_x = sc.render.resolution_y = 800
sc.render.film_transparent = False
sc.world = bpy.data.worlds.new("w"); sc.world.color = (0.35, 0.35, 0.38)
cam_data = bpy.data.cameras.new("cam"); cam_data.type = "ORTHO"; cam_data.ortho_scale = r * 1.05
cam = bpy.data.objects.new("cam", cam_data); sc.collection.objects.link(cam); sc.camera = cam
views = {"front": (0, -1, 0), "back": (0, 1, 0), "left": (-1, 0, 0), "right": (1, 0, 0), "top": (0, 0, 1)}
for name, d in views.items():
    d = Vector(d)
    cam.location = c + d * r * 3
    up = Vector((0, 1, 0)) if name == "top" else Vector((0, 0, 1))
    cam.rotation_euler = (-d).to_track_quat("-Z", "Y" if name != "top" else "Y").to_euler()
    # look at c with the chosen up
    look = (c - cam.location).normalized()
    cam.rotation_euler = look.to_track_quat("-Z", "Z" if name != "top" else "Y").to_euler()
    sc.render.filepath = f"{out}-{name}.png"
    bpy.ops.render.render(write_still=True)
    print(f"[preview] wrote {sc.render.filepath}")
print(f"[preview] bounds min={tuple(round(v,3) for v in lo)} max={tuple(round(v,3) for v in hi)}")
