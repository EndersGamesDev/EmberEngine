"""Blender headless: SWAT operator FBX -> engine GLB with base-color
textures wired by material-name heuristic (Sketchfab FBX ships textures
unlinked). Run:
    blender --background --python swat_convert.py
"""
import os

import bpy

SRC = r"C:\Users\end\dev\ember\assets\swat\source\swat lp.fbx"
TEX = r"C:\Users\end\dev\ember\assets\swat\textures"
OUT = r"C:\Users\end\dev\ember\assets\models\swat.glb"

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=SRC)

MAP = {
    "body": "Body_Base_color.png",
    "head": "HeadGear_Base_color.png",
    "gear": "HeadGear_Base_color.png",
    "helmet": "HeadGear_Base_color.png",
    "ksvr": "KSVR_Base_color.png",
    "rifle": "KSVR_Base_color.png",
    "weapon": "KSVR_Base_color.png",
    "eyelash": "eyelashes01.png",
    "lash": "eyelashes01.png",
}

for mat in bpy.data.materials:
    lname = mat.name.lower()
    tex = next((v for k, v in MAP.items() if k in lname), None)
    print(f"[mat] {mat.name} -> {tex}")
    if not tex:
        continue
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf is None:
        print(f"[mat] {mat.name}: no principled bsdf, skipped")
        continue
    img = bpy.data.images.load(os.path.join(TEX, tex), check_existing=True)
    node = mat.node_tree.nodes.new("ShaderNodeTexImage")
    node.image = img
    mat.node_tree.links.new(bsdf.inputs["Base Color"], node.outputs["Color"])

# Join every mesh into one object: glTF then exports one mesh with one
# primitive per material (4 textures total instead of one copy per part).
meshes = [o for o in bpy.data.objects if o.type == "MESH"]
bpy.ops.object.select_all(action="DESELECT")
for o in meshes:
    o.select_set(True)
bpy.context.view_layer.objects.active = meshes[0]
bpy.ops.object.join()

for o in bpy.data.objects:
    n = len(o.data.vertices) if o.type == "MESH" else ""
    print(f"[obj] {o.name} {o.type} {n} dim={tuple(round(d, 2) for d in o.dimensions)}")

bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_yup=True)
print(f"wrote {OUT}")
