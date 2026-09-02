#!/usr/bin/env python3
"""Blender headless: build the v16 viewmodel from the operator itself.

    blender --background --python tools/v16/build_operator_viewmodel.py -- [--preview]

The v15 viewmodel put a revolver in a bare hand on a character who visibly
carries a rifle and wears gloves: every remote player showed two weapons
(the rifle welded to the spine by tools/swat_split.py, plus the client's
gun at the hand) and the first-person hands were not the operator's. This
build takes weapon and hands from the operator's own FBX
(assets/swat/source/swat lp.fbx), so what the player holds is what every
other player sees them hold.

The FBX is not a bind pose with a rifle floating in front of it. Its
armature carries a POSE - 42 bones, fingers included - in which the
operator holds the rifle: right fist on the pistol grip at the trigger,
left fist on the vertical foregrip. tools/swat_split.py strips that pose
(it removes the modifier and keeps the A-pose for the engine's own rig);
this build applies it, so the hands come out already on the weapon, in the
artist's grip, and nothing here guesses a finger angle or snaps a box.

  * the rifle: 31 rigid KSVR-material objects joined into ONE part (one
    picture, one mesh: the renderer clones a texture per mesh), in the
    frame the file already uses - +X muzzle, +Z up (Blender) - with the
    origin moved to the top of the pistol grip, at the model's own metric
    scale. It is a bullpup: the magazine sits behind the grip and the
    stock reaches back to the shoulder;
  * the hands: cut from the posed body by dominant bone weight (a hand
    bone or one of its fingers), wearing the body's own picture - the
    gloves every remote operator shows; the forearms: built as tapered
    tubes from the posed wrist toward the posed elbow, because the
    operator's sleeves are a dozen big polygons weighted to the hand and
    upper-arm bones and any cut through them is a fan of shards. The tubes
    wear the sleeve's own colour, sampled from the body picture. All four
    pieces join into ONE mesh, `hands`, so that picture is cloned once in
    VRAM, not four times. The fists keep the artist's pose; the forearms
    take first-person directions (ELBOW_DIR), because the artist's is a
    low ready with the support elbow held high, and carried into an aimed
    frame that arm pointed at the sky.

tools/swat_split.py drops the rifle from the body parts in the same change;
the client holds this `rifle` part at the remote hand instead.

Outputs: crates/arena/assets/viewmodel.glb, viewmodel-rig.json (the muzzle;
no pivots - nothing on this rifle spins), tools/v16/preview-*.png.
Run tools/v16/prep_pictures.py first.
"""
import json
import math
import os
import struct
import sys
from collections import Counter

import bmesh
import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
SRC = os.path.join(REPO, "assets", "swat", "source", "swat lp.fbx")
BODY_PICTURE = os.path.join(REPO, "assets", "swat", "baked", "body-2048.png")
RIFLE_PICTURE = os.path.join(REPO, "assets", "swat", "baked", "ksvr-1024.png")
OUT_GLB = os.path.join(REPO, "crates", "arena", "assets", "viewmodel.glb")
OUT_RIG = os.path.join(REPO, "crates", "arena", "assets", "viewmodel-rig.json")
PREVIEW_DIR = HERE

# Picture sizes as they ship. One mesh each, so VRAM is one clone apiece:
# 2048 for the hands (the closest thing on screen), 1024 for the rifle.
TEX_SIZE = {"body": 2048, "rifle": 1024}

# Named rifle objects that anchor the frame: the trigger marks the pistol
# grip (the hold point is the top of the trigger's box, on the rifle's
# centre line), the barrel marks the muzzle, the stock the back.
TRIGGER_OBJECT = "trigger_low"
BARREL_OBJECT = "barrel_low"
STOCK_OBJECT = "stockBack_low"
SIGHT_OBJECT = "backSight_low"

# The forearms are built, not cut: the operator's sleeves are a dozen big
# polygons weighted to the hand and upper-arm bones, and any cut through
# them is a fan of shards. Each forearm is a tapered tube from the posed
# wrist toward the posed elbow, wearing the sleeve's own colour: its UVs all
# sit on one point inside the sleeve's island of the body picture, found as
# the mean UV of the faces the forearm bone owns.
FOREARM_RADII = (0.040, 0.052)  # at the wrist, at the elbow
FOREARM_REACH = (0.03, 0.06)  # into the hand, past the elbow (metres)
FOREARM_LENGTH = 0.27  # wrist to elbow (metres)
FOREARM_SEGMENTS = 20
# Where each elbow lies from its wrist, in the rifle's frame (+X muzzle,
# +Y left, +Z up). The artist's pose is a low ready with the support elbow
# held high; carried into an aimed first-person frame that arm points at
# the sky. The fists keep the artist's pose; the forearms take these.
ELBOW_DIR = {"arm_r": (-0.75, -0.30, -0.55), "arm_l": (-0.45, 0.55, -0.55)}

SIDES = {"hand_r": "Right", "hand_l": "Left"}
ARM_OF = {"hand_r": "arm_r", "hand_l": "arm_l"}

# Where the eye sits relative to the hold point in the engine (the client's
# viewmodel offsets), for the first-person preview.
EYE_IN_GUN_SPACE = Vector((-0.60, 0.20, 0.24))


def say(msg):
    print(f"[v16] {msg}", flush=True)


def die(msg, hint=None):
    print(f"[v16] ERROR: {msg}", flush=True)
    if hint:
        print(f"[v16]   {hint}", flush=True)
    sys.exit(1)


def short(name):
    return name.split(":")[-1]


def fmt(v):
    return tuple(round(c, 3) for c in v)


def world_bbox(objs):
    lo = Vector((math.inf,) * 3)
    hi = Vector((-math.inf,) * 3)
    for o in objs:
        for v in o.data.vertices:
            p = o.matrix_world @ v.co
            lo = Vector(map(min, lo, p))
            hi = Vector(map(max, hi, p))
    return lo, hi


def apply_all(objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    result = bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    if result != {"FINISHED"}:
        die(f"transform_apply on {[o.name for o in objs]} returned {result}")
    for o in objs:
        if o.matrix_world != Matrix.Identity(4):
            die(f"{o.name}: transform not applied, matrix_world is {o.matrix_world}")


def unparent_keep_world(o):
    mw = o.matrix_world.copy()
    o.parent = None
    o.matrix_world = mw


def picture_material(name, path, size):
    """A fresh Principled material whose base colour is the picture at
    `path`, packed into the file so the exporter embeds THESE bytes."""
    if not os.path.isfile(path):
        die(f"missing {path}", "run tools/v16/prep_pictures.py")
    img = bpy.data.images.load(path, check_existing=False)
    img.name = name
    if tuple(img.size) != (size, size):
        die(f"{path} is {tuple(img.size)}, expected {size}x{size}")
    img.pack()
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nodes = mat.node_tree.nodes
    bsdf = nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    bsdf.inputs["Roughness"].default_value = 0.7
    tex = nodes.new("ShaderNodeTexImage")
    tex.image = img
    mat.node_tree.links.new(bsdf.inputs["Base Color"], tex.outputs["Color"])
    return mat


def set_material(obj, mat):
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    for p in obj.data.polygons:
        p.material_index = 0


def smooth(obj):
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.shade_smooth()


# ---- 1. import -------------------------------------------------------------
def import_operator():
    if not os.path.isfile(SRC):
        die(f"missing {SRC}", "unzip the operator source to assets/swat/source")
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.fbx(filepath=SRC, use_anim=False)
    arm = next((o for o in bpy.data.objects if o.type == "ARMATURE"), None)
    body = next((o for o in bpy.data.objects if o.type == "MESH" and o.vertex_groups), None)
    rifle = [
        o
        for o in bpy.data.objects
        if o.type == "MESH" and not o.vertex_groups and any(m and "ksvr" in m.name.lower() for m in o.data.materials)
    ]
    if arm is None or body is None or not rifle:
        die(f"expected an armature, a skinned body and rifle parts; got {[(o.type, o.name) for o in bpy.data.objects]}")
    posed = sum(1 for b in arm.pose.bones if b.matrix_basis != Matrix.Identity(4))
    say(f"operator: body {body.name!r} {len(body.data.polygons)} faces, rig {len(arm.data.bones)} bones ({posed} posed), rifle {len(rifle)} objects")
    if posed == 0:
        die("the armature carries no pose; this build relies on the artist's grip pose in the FBX")
    return arm, body, rifle


def posed_head(arm, name):
    b = next((b for b in arm.pose.bones if short(b.name) == name), None)
    if b is None:
        die(f"rig has no bone {name!r}")
    return arm.matrix_world @ b.head


# ---- 2. the rifle ----------------------------------------------------------
def rifle_frame(rifle_objs):
    """The rifle's own frame in the file's world space: the hold point, the
    rotation that puts its axis on +X with the sights up, and the muzzle
    (already in that frame, about the hold).

    The artist posed the weapon in the hands, yawed and pitched at a low
    ready, so the file's X is not the barrel. The axis is measured from the
    stock to the barrel, up from the rear sight."""
    by_name = {o.name: o for o in rifle_objs}
    for want in (TRIGGER_OBJECT, BARREL_OBJECT, STOCK_OBJECT, SIGHT_OBJECT):
        if want not in by_name:
            die(f"rifle has no object {want!r}; objects: {sorted(by_name)}")

    def centre(name):
        lo, hi = world_bbox([by_name[name]])
        return (lo + hi) * 0.5

    barrel, stock, sight, trigger = (centre(n) for n in (BARREL_OBJECT, STOCK_OBJECT, SIGHT_OBJECT, TRIGGER_OBJECT))
    f = (barrel - stock).normalized()
    up_hint = sight - stock
    u = (up_hint - f * up_hint.dot(f)).normalized()
    left = u.cross(f).normalized()
    # Columns [f, left, u] take the rifle's frame to the world; the
    # transpose brings the world into the rifle's frame.
    to_rifle = Matrix((f, left, u))
    # The hold point: the trigger's top, on the barrel's line.
    hold = trigger + u * (world_bbox([by_name[TRIGGER_OBJECT]])[1].z - trigger.z)
    hold = hold - left * (hold - stock).dot(left)
    pts = [o.matrix_world @ v.co for o in rifle_objs for v in o.data.vertices]
    along = [(p - hold).dot(f) for p in pts]
    muzzle = Vector((max(along), 0.0, (barrel - hold).dot(u)))
    tilt = math.degrees(math.acos(max(-1.0, min(1.0, f.x))))
    say(f"rifle: axis {fmt(f)} ({tilt:.1f} deg off the file's X), up {fmt(u)}, hold {fmt(hold)}, {max(along) - min(along):.3f} long, muzzle {fmt(muzzle)}")
    return hold, to_rifle, muzzle


def build_rifle(rifle_objs):
    for o in rifle_objs:
        unparent_keep_world(o)
        me = o.data
        keep = me.uv_layers.active or me.uv_layers[0]
        for extra in [layer for layer in me.uv_layers if layer != keep]:
            me.uv_layers.remove(extra)
        keep.name = "UVMap"
    bpy.ops.object.select_all(action="DESELECT")
    for o in rifle_objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = rifle_objs[0]
    bpy.ops.object.join()
    rifle = bpy.context.active_object
    rifle.name = "rifle"
    rifle.data.name = "rifle"
    apply_all([rifle])
    set_material(rifle, picture_material("ksvr", RIFLE_PICTURE, TEX_SIZE["rifle"]))
    smooth(rifle)
    say(f"rifle: {len(rifle.data.polygons)} faces in one mesh")
    return rifle


# ---- 3. the hands and forearms --------------------------------------------
def bake_body(body):
    """Apply the armature's pose to the body, in world space."""
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    bpy.context.view_layer.objects.active = body
    mod = next((m for m in body.modifiers if m.type == "ARMATURE"), None)
    if mod is None:
        die("body has no armature modifier")
    bpy.ops.object.modifier_apply(modifier=mod.name)
    body.modifiers.clear()
    unparent_keep_world(body)
    apply_all([body])
    if not body.data.uv_layers:
        die("body has no UVs")
    body.data.uv_layers[0].name = "UVMap"


def cut(body, arm):
    """The hands, by dominant bone weight, one object each; plus what the
    forearm tubes need: the posed elbow-to-wrist segments and a UV inside
    each sleeve."""
    names = {g.index: short(g.name) for g in body.vertex_groups}
    hand_of = {}
    for v in body.data.vertices:
        if not v.groups:
            continue
        g = max(v.groups, key=lambda g: g.weight)
        n = names[g.group]
        for part, side in SIDES.items():
            if n.startswith(f"{side}Hand"):
                hand_of[v.index] = part
    segs = {}
    for part, side in SIDES.items():
        elbow = posed_head(arm, f"{side}ForeArm")
        wrist = posed_head(arm, f"{side}Hand")
        segs[ARM_OF[part]] = (elbow, wrist)

    # Sleeve vertices whose strongest bone is the forearm itself: their
    # faces' mean UV is a point inside the sleeve's island.
    forearm_of = {}
    for v in body.data.vertices:
        if not v.groups:
            continue
        n = names[max(v.groups, key=lambda g: g.weight).group]
        for part, side in SIDES.items():
            if n == f"{side}ForeArm":
                forearm_of[v.index] = ARM_OF[part]
    uv_layer = body.data.uv_layers[0].data
    sleeve_uv = {}
    for arm_part in ARM_OF.values():
        uvs = []
        for p in body.data.polygons:
            if any(forearm_of.get(i) == arm_part for i in p.vertices):
                uvs.extend(uv_layer[li].uv.copy() for li in p.loop_indices)
        if not uvs:
            die(f"no sleeve faces for {arm_part}")
        sleeve_uv[arm_part] = sum(uvs, Vector((0.0, 0.0))) / len(uvs)
        say(f"{arm_part}: sleeve colour from UV {fmt(sleeve_uv[arm_part])} ({len(uvs)} loops)")

    face_part = {}
    for p in body.data.polygons:
        hands = Counter(hand_of[i] for i in p.vertices if i in hand_of)
        if hands and sum(hands.values()) * 2 >= len(p.vertices):
            face_part[p.index] = hands.most_common(1)[0][0]
    counts = Counter(face_part.values())
    say(f"body cut: faces per part {dict(counts)}")

    parts = {}
    for part in ("hand_r", "hand_l"):
        if counts.get(part, 0) < 8:
            die(f"only {counts.get(part, 0)} faces for {part}")
        obj = body.copy()
        obj.data = body.data.copy()
        obj.name = part
        obj.data.name = part
        bpy.context.scene.collection.objects.link(obj)
        bm = bmesh.new()
        bm.from_mesh(obj.data)
        bm.faces.ensure_lookup_table()
        drop = [f for f in bm.faces if face_part.get(f.index) != part]
        bmesh.ops.delete(bm, geom=drop, context="FACES")
        # The face delete leaves the other faces' vertices behind; the
        # exporter would drop them, but every box measured here would span
        # the whole body.
        loose = [v for v in bm.verts if not v.link_faces]
        bmesh.ops.delete(bm, geom=loose, context="VERTS")
        bm.to_mesh(obj.data)
        bm.free()
        parts[part] = obj
        lo, hi = world_bbox([obj])
        say(f"{part}: {len(obj.data.polygons)} faces, box {fmt(lo)}..{fmt(hi)}")
    bpy.data.objects.remove(body, do_unlink=True)
    bpy.data.objects.remove(arm, do_unlink=True)
    for leftover in [o for o in bpy.data.objects if o.name not in parts and o.name != "rifle"]:
        bpy.data.objects.remove(leftover, do_unlink=True)
    skin = picture_material("body", BODY_PICTURE, TEX_SIZE["body"])
    for obj in parts.values():
        set_material(obj, skin)
        smooth(obj)
    wrists = {arm_part: wrist for arm_part, (_elbow, wrist) in segs.items()}
    return parts, wrists, sleeve_uv, skin


def forearm_tube(name, wrist, uv, mat):
    """A tapered tube from just inside the hand toward the elbow, its UVs
    pinned to one point of the sleeve. In the rifle's frame."""
    axis = Vector(ELBOW_DIR[name]).normalized()
    elbow = wrist + axis * FOREARM_LENGTH
    length = FOREARM_LENGTH + FOREARM_REACH[0] + FOREARM_REACH[1]
    me = bpy.data.meshes.new(name)
    bm = bmesh.new()
    bmesh.ops.create_cone(
        bm,
        cap_ends=True,
        segments=FOREARM_SEGMENTS,
        radius1=FOREARM_RADII[0],
        radius2=FOREARM_RADII[1],
        depth=length,
    )
    uv_lay = bm.loops.layers.uv.new("UVMap")
    for f in bm.faces:
        for loop in f.loops:
            loop[uv_lay].uv = uv
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(obj)
    # The cone runs along its own Z, centred; stand it on the axis with its
    # wrist end FOREARM_REACH[0] inside the hand.
    start = wrist - axis * FOREARM_REACH[0]
    rot = axis.to_track_quat("Z", "Y").to_matrix().to_4x4()
    obj.matrix_world = Matrix.Translation(start + axis * (length * 0.5)) @ rot
    apply_all([obj])
    set_material(obj, mat)
    smooth(obj)
    lo, hi = world_bbox([obj])
    say(f"{name}: tube {length:.3f} long from the wrist {fmt(wrist)} toward the elbow {fmt(elbow)}, box {fmt(lo)}..{fmt(hi)}")
    return obj


def join_hands(parts):
    objs = [parts[n] for n in ("hand_r", "arm_r", "hand_l", "arm_l")]
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    hands = bpy.context.active_object
    hands.name = "hands"
    hands.data.name = "hands"
    say(f"hands: {len(hands.data.polygons)} faces in one mesh")
    return hands


def align(objs, hold, to_rifle):
    """Everything into the rifle's frame about the hold point: the hands
    come along, since they and the rifle share the file's world."""
    for o in objs:
        o.matrix_world = to_rifle.to_4x4() @ Matrix.Translation(-hold) @ o.matrix_world
    apply_all(objs)


# ---- 4. export -------------------------------------------------------------
def to_engine(v):
    return [round(v.x, 5), round(v.z, 5), round(-v.y, 5)]


def export(objs, muzzle):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.export_scene.gltf(
        filepath=OUT_GLB,
        export_format="GLB",
        export_yup=True,
        use_selection=True,
        export_apply=True,
        export_image_format="AUTO",
    )
    rig = {"pivots": {}, "muzzle": to_engine(muzzle)}
    with open(OUT_RIG, "w", encoding="utf-8", newline="\n") as f:
        json.dump(rig, f, indent=1)
        f.write("\n")
    say(f"wrote {OUT_GLB} ({os.path.getsize(OUT_GLB) // 1024} KB) and {OUT_RIG}: muzzle {rig['muzzle']}")


def verify_glb():
    with open(OUT_GLB, "rb") as f:
        magic, _version, _length = struct.unpack("<III", f.read(12))
        if magic != 0x46546C67:
            die("not a GLB")
        clen, ctype = struct.unpack("<II", f.read(8))
        if ctype != 0x4E4F534A:
            die("first chunk is not JSON")
        doc = json.loads(f.read(clen))
    names = [m.get("name") for m in doc.get("meshes", [])]
    if sorted(names) != ["hands", "rifle"]:
        die(f"expected meshes hands + rifle, got {names}")
    for m in doc["meshes"]:
        for prim in m["primitives"]:
            mat = doc["materials"][prim["material"]]
            if "baseColorTexture" not in mat.get("pbrMetallicRoughness", {}):
                die(f"mesh {m['name']!r} has a material without a picture")
    images = doc.get("images", [])
    mimes = [i.get("mimeType") for i in images]
    if len(images) != 2 or any(m != "image/png" for m in mimes):
        die(f"expected two PNG images, got {mimes}")
    say(f"verified: meshes {names}, {len(images)} PNG pictures, nodes {[n.get('name') for n in doc.get('nodes', [])]}")


# ---- 5. previews -----------------------------------------------------------
def render_previews(objs):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.render.resolution_x = 1024
    scene.render.resolution_y = 640
    scene.render.image_settings.file_format = "PNG"
    lo, hi = world_bbox(objs)
    centre = (lo + hi) * 0.5
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "eye": (EYE_IN_GUN_SPACE, EYE_IN_GUN_SPACE + Vector((1.0, 0.0, -0.05)), 80.0, "Y"),
        "side": (centre + Vector((0.0, -1.5, 0.1)), centre, 45.0, "Y"),
        "top": (centre + Vector((0.0, 0.0, 1.5)), centre, 45.0, "X"),
        "front": (centre + Vector((1.5, 0.0, 0.1)), centre, 45.0, "Y"),
    }
    for name, (pos, target, fov, up) in views.items():
        cam_data.angle = math.radians(fov)
        cam.location = pos
        cam.rotation_euler = (target - pos).normalized().to_track_quat("-Z", up).to_euler()
        scene.render.filepath = os.path.join(PREVIEW_DIR, f"preview-{name}.png")
        bpy.ops.render.render(write_still=True)
        say(f"preview {name}: {scene.render.filepath}")


def main():
    preview = "--preview" in sys.argv
    arm, body, rifle_objs = import_operator()
    hold, to_rifle, muzzle = rifle_frame(rifle_objs)
    rifle = build_rifle(rifle_objs)
    bake_body(body)
    parts, wrists, sleeve_uv, skin = cut(body, arm)
    align([rifle, parts["hand_r"], parts["hand_l"]], hold, to_rifle)
    for arm_part, wrist in wrists.items():
        parts[arm_part] = forearm_tube(arm_part, to_rifle @ (wrist - hold), sleeve_uv[arm_part], skin)
    hands = join_hands(parts)
    objs = [rifle, hands]
    for o in objs:
        lo, hi = world_bbox([o])
        say(f"{o.name}: box {fmt(lo)}..{fmt(hi)} about the hold point")
    export(objs, muzzle)
    verify_glb()
    if preview:
        render_previews(objs)


main()
