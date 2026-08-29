"""Blender headless: split the Mixamo-rigged SWAT operator into one mesh
per engine rig joint, so the jointed FK rig can animate an artist-made,
textured character.

Every vertex goes to the joint of its dominant bone weight (walking up the
bone chain until a mapped bone is found); faces follow their majority
vertex. Parts keep their UVs and share the model's textures, so the whole
set exports as ONE GLB with one copy of each image. The bind-pose joint
positions are written alongside as JSON, in engine space (Y up).

    blender --background --python tools/swat_split.py
"""

import json
import os
from collections import Counter

import bmesh
import bpy

SRC = r"C:\Users\end\dev\ember\assets\swat\source\swat lp.fbx"
TEX = r"C:\Users\end\dev\ember\assets\swat\textures"
OUT_GLB = r"C:\Users\end\dev\ember\assets\models\swat-parts.glb"
OUT_JSON = r"C:\Users\end\dev\ember\assets\models\swat-rig.json"
# Atlas size per part. Each part carries its own copy at runtime, so this
# trades VRAM for fidelity: 1024 keeps the whole character near 60 MB.
TEX_SIZE = 1024

MAT_TEX = {
    "body": "Body_Base_color.png",
    "head": "HeadGear_Base_color.png",
    "gear": "HeadGear_Base_color.png",
    "ksvr": "KSVR_Base_color.png",
    "eyelash": "eyelashes01.png",
}

# Engine joint names, in the order of ember_engine::rig::joint.
JOINTS = [
    "root", "spine", "neck",
    "shoulder_l", "elbow_l", "wrist_l",
    "shoulder_r", "elbow_r", "wrist_r",
    "hip_l", "knee_l", "ankle_l",
    "hip_r", "knee_r", "ankle_r",
]
# Mixamo bone (suffix after "mixamorig:") -> engine joint.
BONE_JOINT = {
    "Hips": "root",
    "Spine": "spine", "Spine1": "spine", "Spine2": "spine",
    # Clavicles carry the deltoid and shoulder pad: rotating them with the
    # arm keeps the seam closed when the A-pose retargets to arms-down.
    "LeftShoulder": "shoulder_l", "RightShoulder": "shoulder_r",
    "Neck": "neck", "Head": "neck", "HeadTop_End": "neck",
    "LeftArm": "shoulder_l", "LeftForeArm": "elbow_l", "LeftHand": "wrist_l",
    "RightArm": "shoulder_r", "RightForeArm": "elbow_r", "RightHand": "wrist_r",
    "LeftUpLeg": "hip_l", "LeftLeg": "knee_l", "LeftFoot": "ankle_l",
    "LeftToeBase": "ankle_l", "LeftToe_End": "ankle_l",
    "RightUpLeg": "hip_r", "RightLeg": "knee_r", "RightFoot": "ankle_r",
    "RightToeBase": "ankle_r", "RightToe_End": "ankle_r",
}
# Bone whose head marks each joint's pivot.
JOINT_BONE = {
    "root": "Hips", "spine": "Spine", "neck": "Neck",
    "shoulder_l": "LeftArm", "elbow_l": "LeftForeArm", "wrist_l": "LeftHand",
    "shoulder_r": "RightArm", "elbow_r": "RightForeArm", "wrist_r": "RightHand",
    "hip_l": "LeftUpLeg", "knee_l": "LeftLeg", "ankle_l": "LeftFoot",
    "hip_r": "RightUpLeg", "knee_r": "RightLeg", "ankle_r": "RightFoot",
}


def short(bone_name):
    return bone_name.split(":")[-1]


def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=SRC)

    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    # Bone heads -> engine space: Blender is Z-up, glTF Y-up export maps
    # (x, y, z) -> (x, z, -y); joint positions must match the meshes.
    joints = {}
    for jname, bname in JOINT_BONE.items():
        bone = next(
            (b for b in arm.data.bones if short(b.name) == bname), None
        )
        if bone is None:
            raise SystemExit(f"bone for joint {jname} not found ({bname})")
        h = arm.matrix_world @ bone.head_local
        joints[jname] = [h.x, h.z, -h.y]

    # Textures.
    for mat in bpy.data.materials:
        lname = mat.name.lower()
        tex = next((v for k, v in MAT_TEX.items() if k in lname), None)
        if not tex or not mat.use_nodes:
            continue
        bsdf = mat.node_tree.nodes.get("Principled BSDF")
        if bsdf is None:
            continue
        img = bpy.data.images.load(os.path.join(TEX, tex), check_existing=True)
        if max(img.size) > TEX_SIZE:
            img.scale(TEX_SIZE, TEX_SIZE)
        node = mat.node_tree.nodes.new("ShaderNodeTexImage")
        node.image = img
        mat.node_tree.links.new(bsdf.inputs["Base Color"], node.outputs["Color"])

    # One mesh, so vertex groups and materials unify.
    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    # Join merges UV layers BY NAME: parts whose layer is named differently
    # would end up with an empty first layer, and glTF only exports
    # TEXCOORD_0 — the whole model would sample a single texel. Collapse
    # every object to one identically named UV layer first.
    for o in meshes:
        me = o.data
        if not me.uv_layers:
            print(f"[uv] {o.name} has no UV layer")
            continue
        keep = me.uv_layers.active or me.uv_layers[0]
        for extra in [layer for layer in me.uv_layers if layer != keep]:
            me.uv_layers.remove(extra)
        keep.name = "UVMap"
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.join()
    src = bpy.context.active_object
    src.modifiers.clear()

    # Vertex group index -> joint, following bone parents for unmapped
    # bones (fingers -> hand, toes -> foot).
    bones = {short(b.name): b for b in arm.data.bones}
    gi_joint = {}
    for g in src.vertex_groups:
        name = short(g.name)
        bone = bones.get(name)
        while name not in BONE_JOINT and bone is not None and bone.parent:
            bone = bone.parent
            name = short(bone.name)
        if name in BONE_JOINT:
            gi_joint[g.index] = BONE_JOINT[name]

    # Dominant weight per vertex, then majority vote per face.
    vert_joint = {}
    for v in src.data.vertices:
        best, best_w = None, 0.0
        for g in v.groups:
            j = gi_joint.get(g.group)
            if j is not None and g.weight > best_w:
                best, best_w = j, g.weight
        if best is not None:
            vert_joint[v.index] = best
    # Unweighted geometry (the rifle is rigid, not skinned) has no vote:
    # place it by material so nothing is silently dropped.
    mat_fallback = {"ksvr": "spine"}
    mat_names = [m.name.lower() if m else "" for m in src.data.materials]
    face_joint = {}
    orphans = Counter()
    for p in src.data.polygons:
        votes = Counter(vert_joint[i] for i in p.vertices if i in vert_joint)
        if votes:
            face_joint[p.index] = votes.most_common(1)[0][0]
            continue
        mname = mat_names[p.material_index] if p.material_index < len(mat_names) else ""
        joint = next((v for k, v in mat_fallback.items() if k in mname), None)
        if joint:
            face_joint[p.index] = joint
        else:
            orphans[mname] += 1
    if orphans:
        print(f"[split] unweighted faces dropped, by material: {dict(orphans)}")

    counts = Counter(face_joint.values())
    print(f"[split] faces per joint: {dict(counts)}")

    # One object per joint: copy the mesh, delete the other faces.
    made = []
    for jname in JOINTS:
        if counts.get(jname, 0) == 0:
            print(f"[split] {jname}: no faces, skipped")
            continue
        obj = src.copy()
        obj.data = src.data.copy()
        obj.name = f"rig_{jname}"
        bpy.context.scene.collection.objects.link(obj)
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        bm.faces.ensure_lookup_table()
        drop = [f for f in bm.faces if face_joint.get(f.index) != jname]
        bmesh.ops.delete(bm, geom=drop, context="FACES")
        bm.to_mesh(obj.data)
        bm.free()
        obj.data.name = obj.name
        made.append((jname, obj))
        print(f"[split] {jname}: {len(obj.data.polygons)} faces")

    # Export just the parts.
    bpy.data.objects.remove(src, do_unlink=True)
    bpy.data.objects.remove(arm, do_unlink=True)
    bpy.ops.object.select_all(action="DESELECT")
    for _, obj in made:
        obj.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=OUT_GLB,
        export_format="GLB",
        export_yup=True,
        use_selection=True,
    )
    with open(OUT_JSON, "w", encoding="utf-8") as f:
        json.dump({"joints": joints, "parts": [j for j, _ in made]}, f, indent=1)
    print(f"wrote {OUT_GLB} and {OUT_JSON} ({len(made)} parts)")


main()
