"""Blender headless: the heavy revolver + the realistic hand -> the arena's
first-person viewmodel (crates/arena/assets/viewmodel.glb) and its pivot
sidecar (viewmodel-rig.json).

    blender --background --python tools/v15/build_viewmodel.py [-- --preview]

Inputs (artist sources, gitignored):
    assets/revolver/revolver.obj        from tools/v15/dae_to_obj.py
    assets/hands/source/hand.fbx        one mesh, two hands, no rig, no UVs
    assets/hands/skin.png               a procedural skin picture

What it does, and why each step is shaped the way it is:

  * The revolver's twenty parts collapse to SIX meshes: frame, receiver,
    cylinder, cylinder_ammo, hammer, trigger. The engine clones one GPU
    texture per mesh, so twenty parts at 1024² would be ~80 MB of VRAM
    for one pistol; six is ~18 MB. The cut follows what moves: the
    cylinder spins, the hammer cocks and falls, the trigger pulls, and
    everything else is welded to the frame. The bullets and casings sit in
    the cylinder and spin with it, but wear a different picture, so they
    are their own mesh named with the same prefix.
  * Fit follows docs/asset-pipeline.md: +X muzzle, +Z up in Blender
    (+X forward / +Y up in the engine), 0.9 units long like every pistol
    before it, origin at the hold point on the grip.
  * The hand is a rigged, UV-mapped game hand with its own base-colour
    picture. Its fingers are curled around the grip by posing the rig's
    finger chains, the pose is baked into the mesh (the engine has no
    skinning for the viewmodel), and the result is mirrored for the other
    side. One static pose per hand.
  * Node names are the runtime contract (crates/arena/src/online.rs):
    hand*/arm* are viewmodel-only, everything else is also drawn on remote
    players; the animation reads the names too (cylinder*, hammer, trigger).

Every constant under FIT is measured against these two sources; a new
revolver or a new hand means re-measuring, not re-authoring.
"""

import json
import math
import os
import struct
import sys

import bpy
import bmesh
from mathutils import Matrix, Vector

TOOLS = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(TOOLS))
OBJ = os.path.join(REPO, "assets", "revolver", "revolver.obj")
# The rigged game hand (assets/rigged-hand-game-model.zip): 2k faces, UVs, a
# base-colour picture and a 22-bone rig, which is what lets the fingers be
# POSED around the grip instead of bent by arithmetic. The 194k-face sculpt
# in assets/hands.zip (no rig, no UVs) was the first attempt and is kept
# only as the fallback path in git history.
HAND_FBX = os.path.join(REPO, "assets", "hands", "rigged", "source", "hand-only-rig.fbx")
HAND_PICTURE = os.path.join(REPO, "assets", "hands", "rigged", "baked", "hand.png")   # prep_pictures.py
OUT_GLB = os.path.join(REPO, "crates", "arena", "assets", "viewmodel.glb")
OUT_RIG = os.path.join(REPO, "crates", "arena", "assets", "viewmodel-rig.json")
PREVIEW = "--preview" in sys.argv

# ---- FIT --------------------------------------------------------------------
# 0.75, not the 0.9 the box pistols used: this one has a real barrel and at
# 0.9 its muzzle reached the middle of the screen while the hands hung
# below the bottom edge. The client's viewmodel offsets were re-tuned to it.
TARGET_LENGTH = 0.75
TEX_SIZE = {"M1": 1024, "M2": 1024, "B": 512}
SKIN_SIZE = 512
# A hand is ~0.19 m wrist to fingertip; the gun's fit decides how many
# units a metre is (the OBJ is in metres), so the hand is sized after it.
# The rigged mesh runs from a forearm stub to the fingertips; the hand
# proper is about two thirds of that height.
HAND_METRES = 0.27
FIT = {"units_per_metre": 1.0, "grip": None}
HAND_FACES = 6000
# Where along the hand's height the knuckles sit (placement reads it) and
# how far each finger joint bends, per finger chain, in degrees.
KNUCKLE_FRAC = 0.62
CURL = {"index": (55, 60, 40), "middle": (60, 65, 45), "ring": (62, 65, 45), "pinky": (65, 65, 45)}
THUMB_CURL = (20, 25, 15)
# Bone chains in the FBX, root to tip (the file names them by number).
FINGERS = {
    "index": ["Bone.009", "Bone.010", "Bone.011"],
    "middle": ["Bone.012", "Bone.013", "Bone.014"],
    "ring": ["Bone.015", "Bone.016", "Bone.017"],
    "pinky": ["Bone.018", "Bone.019", "Bone.020"],
}
THUMB = ["Bone.003", "Bone.004", "Bone.005"]
# Where the hold point sits inside the grip's own box, after the axis fix:
# along the muzzle axis from its rear, and down from its top.
GRIP_ANCHOR_FRAC_X = 0.55
GRIP_ANCHOR_FROM_TOP = 0.03
# Hand placement in the fitted gun frame (units): right hand around the grip,
# left hand cupped under and in front of it. (x forward, y left, z up.)
# Placement is derived from the grip's box and the hand's own size in
# place_hands(); these are the residual nudges, in units, after that.
HAND_R_NUDGE = Vector((0.0, 0.0, 0.0))
HAND_R_EULER_DEG = (0.0, 0.0, 0.0)
HAND_L_NUDGE = Vector((0.0, 0.0, 0.0))
HAND_L_EULER_DEG = (0.0, 0.0, 0.0)

# The source ships a loose display cartridge floating in front of the muzzle
# (Bullet_Low + Casing_Low, material B); it is not part of the gun.
DELETE_PARTS = {"Bullet_Low", "Casing_Low"}
GROUPS = {
    "cylinder": ["Cylinder_Low", "Cylinder_Detail_1_Low", "Cylinder_Detail_2_Low"],
    "hammer": ["Hammer_Low"],
    "trigger": ["Trigger_Low"],
    "receiver": ["Receiver_Low", "Front_Sight_Low"],
}
# Everything not listed above is welded to the frame.


def say(*a):
    print("[vm]", *a)


def die(msg, *detail):
    say("FATAL:", msg)
    for d in detail:
        say("      ", d)
    raise SystemExit(1)


def world_bbox(objs):
    pts = [o.matrix_world @ v.co for o in objs for v in o.data.vertices]
    if not pts:
        die("bbox of nothing")
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def apply_all(objs):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)


def meshes():
    return [o for o in bpy.data.objects if o.type == "MESH"]


# ---- 1. the revolver -------------------------------------------------------
def import_revolver(reset=True):
    """Import the OBJ's twenty parts and drop the display cartridge.

    `reset` wipes the scene first, as this script's own run wants. A build
    that imports this file as a library (tools/v18) already holds other
    parts in the scene, so it passes False and only the objects the import
    added are touched; the count check reads those, not the whole scene."""
    if not os.path.isfile(OBJ):
        die(f"missing {OBJ}", "run tools/v15/dae_to_obj.py first")
    if reset:
        bpy.ops.wm.read_factory_settings(use_empty=True)
    before = set(bpy.data.objects)
    bpy.ops.wm.obj_import(filepath=OBJ, use_split_objects=True, use_split_groups=False)
    objs = [o for o in meshes() if o not in before]
    say(f"revolver: {len(objs)} parts imported")
    if len(objs) != 20:
        die(f"expected 20 parts, got {len(objs)}: {[o.name for o in objs]}")
    apply_all(objs)
    for o in list(objs):
        if o.name.split(".")[0] in DELETE_PARTS:
            say(f"dropping {o.name!r} (display cartridge, not the gun)")
            bpy.data.objects.remove(o, do_unlink=True)
            objs.remove(o)
    return objs


def fit_revolver(objs):
    """Rotate so the muzzle points +X and up is +Z, scale to TARGET_LENGTH,
    and return the grip part (for the anchor) and the receiver (for the
    muzzle)."""
    lo, hi = world_bbox(objs)
    dim = hi - lo
    order = sorted(range(3), key=lambda i: dim[i], reverse=True)
    long_ax, up_ax = order[0], order[1]
    # The grip hangs off the rear; the muzzle is the far end from it.
    grip = next(o for o in objs if o.name.startswith("Grip"))
    glo, ghi = world_bbox([grip])
    gc = (glo + ghi) * 0.5
    c = (lo + hi) * 0.5
    muzzle = Vector((0, 0, 0))
    muzzle[long_ax] = 1.0 if gc[long_ax] < c[long_ax] else -1.0
    # Up: the grip hangs DOWN from the frame, so up is away from the grip's
    # centre along the second axis.
    up = Vector((0, 0, 0))
    up[up_ax] = 1.0 if gc[up_ax] < c[up_ax] else -1.0
    wide = up.cross(muzzle)
    rot = Matrix((muzzle, wide, up)).to_4x4()  # rows map (muzzle, wide, up) -> (X, Y, Z)
    scale = TARGET_LENGTH / dim[long_ax]
    FIT["units_per_metre"] = scale
    say(f"fit: long axis {'XYZ'[long_ax]} muzzle {tuple(muzzle)} up {tuple(up)} scale {scale:.4f} units/m")
    xf = Matrix.Scale(scale, 4) @ rot
    for o in objs:
        o.matrix_world = xf @ o.matrix_world
    apply_all(objs)
    lo, hi = world_bbox(objs)
    say(f"fitted bbox min={tuple(round(v, 3) for v in lo)} max={tuple(round(v, 3) for v in hi)}")
    # Origin = hold point on the grip.
    glo, ghi = world_bbox([grip])
    anchor = Vector((
        glo.x + (ghi.x - glo.x) * GRIP_ANCHOR_FRAC_X,
        (glo.y + ghi.y) * 0.5,
        ghi.z - GRIP_ANCHOR_FROM_TOP,
    ))
    say(f"grip box min={tuple(round(v, 3) for v in glo)} max={tuple(round(v, 3) for v in ghi)} anchor={tuple(round(v, 3) for v in anchor)}")
    for o in objs:
        o.matrix_world = Matrix.Translation(-anchor) @ o.matrix_world
    apply_all(objs)
    FIT["grip"] = world_bbox([grip])
    say(f"grip after anchoring: min={tuple(round(v, 3) for v in FIT['grip'][0])} max={tuple(round(v, 3) for v in FIT['grip'][1])}")
    return grip


def merge_parts(objs):
    by_name = {o.name.split(".")[0]: o for o in objs}
    used = set()
    out = {}
    for group, names in GROUPS.items():
        members = []
        for n in names:
            o = by_name.get(n)
            if o is None:
                die(f"part {n!r} missing for group {group!r}", f"have {sorted(by_name)}")
            members.append(o)
            used.add(n)
        out[group] = join(members, group)
    rest = [o for n, o in by_name.items() if n not in used]
    out["frame"] = join(rest, "frame")
    for name, o in out.items():
        lo, hi = world_bbox([o])
        say(f"  {name:14s} faces={len(o.data.polygons):6d} mats={[m.name for m in o.data.materials]} min={tuple(round(v, 3) for v in lo)} max={tuple(round(v, 3) for v in hi)}")
    return out


def join(objs, name):
    mats = {m.name.split(".")[0] for o in objs for m in o.data.materials if m}
    if len(mats) != 1:
        die(f"group {name!r} mixes materials {sorted(mats)}: one picture per mesh")
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    if len(objs) > 1:
        bpy.ops.object.join()
    o = bpy.context.view_layer.objects.active
    o.name = name
    o.data.name = name
    return o


BAKED_DIR = os.path.join(REPO, "assets", "revolver", "baked")   # from tools/v15/prep_pictures.py


def picture_material(name, path, size):
    """A bare Principled material carrying one PACKED picture.

    Built from scratch rather than from what the OBJ importer made: the
    importer's images keep a relative path the glTF exporter could not
    resolve, and it exported the gun with no pictures at all, silently.
    The picture is loaded already at its shipping size (prep_pictures.py):
    Blender's own Image.scale() left the 4096 JPEGs untouched and the
    exporter then shipped the originals - 9 MB of viewmodel. Packing a PNG
    file makes the exporter write PNG bytes (docs/asset-pipeline.md, Path D)."""
    if not os.path.isfile(path):
        die(f"missing picture {path}")
    img = bpy.data.images.load(path, check_existing=False)
    if img.size[0] == 0:
        die(f"{path} did not decode")
    if max(img.size) > size:
        die(f"{name}: {path} is {img.size[0]}x{img.size[1]}, expected <= {size}; run tools/v15/prep_pictures.py")
    img.name = name
    img.colorspace_settings.name = "sRGB"
    img.file_format = "PNG"
    img.pack()
    if img.packed_file is None:
        die(f"{name}: image did not pack")
    mat = bpy.data.materials.new(name)
    mat.use_nodes = True
    nt = mat.node_tree
    bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    bsdf.inputs["Base Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    say(f"material {name}: {os.path.basename(path)} -> {img.size[0]}x{img.size[1]} packed")
    return mat


def bake_textures():
    """Replace every gun material with a fresh one carrying its albedo."""
    for m in bpy.data.materials:
        if m.name.split(".")[0] in TEX_SIZE:
            m.name = "old_" + m.name
    fresh = {key: picture_material(key, os.path.join(BAKED_DIR, f"{key}.png"), size)
             for key, size in TEX_SIZE.items() if key != "B"}
    for o in meshes():
        for i, m in enumerate(o.data.materials):
            key = m.name[4:].split(".")[0] if m and m.name.startswith("old_") else None
            if key in fresh:
                o.data.materials[i] = fresh[key]
    for m in list(bpy.data.materials):
        if m.name.startswith("old_"):
            bpy.data.materials.remove(m)


# ---- 2. the hand -----------------------------------------------------------
def import_hand():
    """Import the rigged hand, curl its fingers by posing the rig, bake the
    pose into the mesh, and dress it with its base-colour picture."""
    if not os.path.isfile(HAND_FBX):
        die(f"missing {HAND_FBX}", "unzip assets/rigged-hand-game-model.zip to assets/hands/rigged")
    if not os.path.isfile(HAND_PICTURE):
        die(f"missing {HAND_PICTURE}", "run tools/v15/prep_pictures.py")
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=HAND_FBX, use_anim=False)
    new = [o for o in bpy.data.objects if o not in before]
    arm = next((o for o in new if o.type == "ARMATURE"), None)
    hand = next((o for o in new if o.type == "MESH"), None)
    if arm is None or hand is None:
        die(f"expected an armature and a mesh, got {[(o.type, o.name) for o in new]}")
    for o in new:
        if o not in (arm, hand):
            bpy.data.objects.remove(o, do_unlink=True)
    say(f"hand: {hand.name!r} {len(hand.data.polygons)} faces, rig {arm.name!r} {len(arm.data.bones)} bones")

    # Pose: bend every finger joint about the bone's own X, toward the palm.
    # The sign is measured, not assumed: the fingertip must move toward the
    # side the relaxed fingers already lean to.
    bpy.ops.object.select_all(action="DESELECT")
    arm.select_set(True)
    bpy.context.view_layer.objects.active = arm
    bpy.ops.object.mode_set(mode="POSE")
    pb = arm.pose.bones
    for chain in list(FINGERS.values()) + [THUMB]:
        for b in chain:
            if b not in pb:
                die(f"rig has no bone {b!r}; bones: {[x.name for x in pb]}")
            pb[b].rotation_mode = "XYZ"

    def tip_world(chain):
        return arm.matrix_world @ pb[chain[-1]].tail

    def apply_curl(sign):
        for name, chain in FINGERS.items():
            for b, deg_ in zip(chain, CURL[name]):
                pb[b].rotation_euler = (sign * math.radians(deg_), 0.0, 0.0)
        for b, deg_ in zip(THUMB, THUMB_CURL):
            pb[b].rotation_euler = (sign * math.radians(deg_), 0.0, 0.0)
        bpy.context.view_layer.update()

    rest_tip = tip_world(FINGERS["middle"]).copy()
    rest_base = (arm.matrix_world @ pb[FINGERS["middle"][0]].head).copy()
    lean = rest_tip - rest_base
    apply_curl(+1.0)
    plus_tip = tip_world(FINGERS["middle"]).copy()
    apply_curl(-1.0)
    minus_tip = tip_world(FINGERS["middle"]).copy()
    # The palm is on the side the relaxed fingers lean to along Y.
    palm_y = 1.0 if lean.y > 0 else -1.0
    sign = 1.0 if (plus_tip.y - rest_tip.y) * palm_y > (minus_tip.y - rest_tip.y) * palm_y else -1.0
    apply_curl(sign)
    say(f"hand: fingers curled with sign {sign:+.0f}; palm on {'+' if palm_y > 0 else '-'}Y")
    bpy.ops.object.mode_set(mode="OBJECT")

    # Bake the pose into the mesh and drop the rig: the engine has no skinning
    # for the viewmodel and the export must be plain geometry.
    bpy.ops.object.select_all(action="DESELECT")
    hand.select_set(True)
    bpy.context.view_layer.objects.active = hand
    mod = next((m for m in hand.modifiers if m.type == "ARMATURE"), None)
    if mod is None:
        die("hand mesh has no armature modifier")
    bpy.ops.object.modifier_apply(modifier=mod.name)
    hand.parent = None
    bpy.data.objects.remove(arm, do_unlink=True)
    hand.name = "hand_src"
    apply_all([hand])
    bpy.ops.object.shade_smooth()

    # Its picture, at the size it ships.
    hand.data.materials.clear()
    hand.data.materials.append(picture_material("hand", HAND_PICTURE, SKIN_SIZE))
    if not hand.data.uv_layers:
        die("rigged hand has no UVs")
    hand.data.uv_layers[0].name = "UVMap"

    # Recentre: forearm end at z=0, centred in x/y.
    lo, hi = world_bbox([hand])
    centre = Vector(((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, lo.z))
    hand.matrix_world = Matrix.Translation(-centre) @ hand.matrix_world
    apply_all([hand])
    lo, hi = world_bbox([hand])
    height = hi.z - lo.z
    palm = Vector((0, palm_y, 0))
    vs = [v.co for v in hand.data.vertices]
    thumb_x = -1.0 if abs(min(v.x for v in vs)) > abs(max(v.x for v in vs)) else 1.0
    thumb = Vector((thumb_x, 0, 0))
    viewer_right = (-palm).cross(Vector((0, 0, 1)))
    is_right = viewer_right.dot(thumb) > 0
    say(f"hand: palm faces {tuple(palm)}, thumb on {tuple(thumb)} -> {'RIGHT' if is_right else 'LEFT'} hand; height {height:.3f}")
    return hand, palm, thumb, is_right, height


def place_hands(src, palm, thumb, is_right, height):
    """Make hand_r and hand_l from the source hand and put them on the grip.

    The source is one hand of known handedness (measured above); the other
    hand is its mirror. Each is posed by a pure rotation built from where
    its three local axes should point in the gun frame (x forward, y left,
    z up): fingers (local +Z), palm (local +/-Y) and thumb (local +/-X).

    Right hand: fingers run forward along the front of the grip, the palm
    faces the grip from its right side, the thumb lies up along the frame.
    Left hand: the support hand - its palm faces backward onto the right
    hand's fingers, its fingers wrap to the right around them, its thumb
    lies up the left side of the frame.
    """
    hand_height = HAND_METRES * FIT["units_per_metre"]
    scale = hand_height / height
    src.scale = (scale, scale, scale)
    apply_all([src])
    lo, hi = world_bbox([src])
    half_thick = (hi.y - lo.y) * 0.5
    half_wide = (hi.x - lo.x) * 0.5
    knuckle = hand_height * KNUCKLE_FRAC
    glo, ghi = FIT["grip"]
    gmid = (glo + ghi) * 0.5
    say(f"hand {hand_height:.3f} tall, half-thick {half_thick:.3f}, half-wide {half_wide:.3f}; knuckle at {knuckle:.3f}")
    # Right hand: wrist behind the grip so the knuckles reach its front
    # strap; palm surface on the grip's right face; centred on the grip.
    hand_r_pos = Vector((ghi.x - knuckle + 0.01, glo.y - half_thick - 0.004, gmid.z - 0.01)) + HAND_R_NUDGE
    # Left hand: wrist out on the gun's left, fingers reaching across under
    # the right hand's fingers, palm up.
    hand_l_pos = Vector((gmid.x + 0.03, ghi.y + knuckle * 0.45, glo.z + 0.02)) + HAND_L_NUDGE

    def pose(name, want_right, fingers_to, palm_to, thumb_to, pos, euler_deg):
        mirror = want_right != is_right
        o = src.copy()
        o.data = src.data.copy()
        o.name = name
        o.data.name = name
        bpy.context.collection.objects.link(o)
        thumb_axis = thumb.x
        if mirror:
            o.data.transform(Matrix.Scale(-1.0, 4, Vector((1, 0, 0))))
            o.data.flip_normals()
            thumb_axis = -thumb.x
        cx = Vector(thumb_to) * thumb_axis   # where local +X lands
        cy = Vector(palm_to) * palm.y        # where local +Y lands
        cz = Vector(fingers_to)              # where local +Z lands
        R = Matrix((cx, cy, cz)).transposed().to_4x4()
        det = R.to_3x3().determinant()
        if det < 0.5:
            die(f"{name}: the target frame is not a rotation (det {det:+.1f}); "
                "fingers/palm/thumb targets must be a right-handed triple for this hand")
        o.matrix_world = Matrix.Translation(Vector(pos)) @ mathutils_euler(euler_deg) @ R
        apply_all([o])
        lo, hi = world_bbox([o])
        say(f"  {name}: {'mirrored' if mirror else 'as-is'} min={tuple(round(v, 3) for v in lo)} max={tuple(round(v, 3) for v in hi)}")
        return o

    def snap(o, dx=None, dy=None, dz=None):
        """Translate so the posed box meets the given targets: each of dx/dy/dz
        is (which, value) with which in {"min", "max", "mid"}."""
        lo, hi = world_bbox([o])
        mid = (lo + hi) * 0.5
        shift = Vector((0, 0, 0))
        for axis, spec in enumerate((dx, dy, dz)):
            if spec is None:
                continue
            which, value = spec
            cur = {"min": lo, "max": hi, "mid": mid}[which][axis]
            shift[axis] = value - cur
        o.matrix_world = Matrix.Translation(shift) @ o.matrix_world
        apply_all([o])
        lo, hi = world_bbox([o])
        say(f"  {o.name} snapped by {tuple(round(v, 3) for v in shift)} -> min={tuple(round(v, 3) for v in lo)} max={tuple(round(v, 3) for v in hi)}")

    hand_r = pose("hand_r", True, (1, 0, 0), (0, 1, 0), (0, 0, 1), hand_r_pos, HAND_R_EULER_DEG)
    # Palm side (its +Y face) against the grip's right face; fingertips just
    # past the front strap; centred on the grip's height.
    snap(hand_r, dx=("max", ghi.x + 0.035 + HAND_R_NUDGE.x), dy=("max", glo.y - 0.002 + HAND_R_NUDGE.y), dz=("mid", gmid.z - 0.05 + HAND_R_NUDGE.z))
    rlo, rhi = world_bbox([hand_r])
    hand_l = pose("hand_l", False, (0, -1, 0), (-1, 0, 0), (0, 0, 1), hand_l_pos, HAND_L_EULER_DEG)
    # Palm (its -X face now) against the right hand's fingertips, wrist on
    # the gun's left, fingers reaching right around the right hand, a
    # little lower than the right hand.
    snap(hand_l, dx=("min", rhi.x - 0.04 + HAND_L_NUDGE.x), dy=("max", ghi.y + 0.22 + HAND_L_NUDGE.y), dz=("mid", gmid.z - 0.11 + HAND_L_NUDGE.z))
    bpy.data.objects.remove(src, do_unlink=True)
    forearm("arm_r", hand_r, wrist_end="min_x", direction=(-0.55, -0.40, -0.70))
    forearm("arm_l", hand_l, wrist_end="max_y", direction=(-0.45, 0.55, -0.70))
    return hand_r, hand_l


def forearm(name, hand, wrist_end, direction):
    """A tapered skin-coloured tube from the hand's wrist off toward where
    the body would be, so the first-person view does not end at a cut
    wrist. Named arm_* so the client draws it for the owner only."""
    lo, hi = world_bbox([hand])
    mid = (lo + hi) * 0.5
    if wrist_end == "min_x":
        wrist = Vector((lo.x + 0.025, mid.y, mid.z))
    else:
        wrist = Vector((mid.x, hi.y - 0.025, mid.z))
    d = Vector(direction).normalized()
    length = 0.5 * FIT["units_per_metre"] * 0.4
    radius = 0.045 * FIT["units_per_metre"] * 0.4
    bpy.ops.mesh.primitive_cone_add(vertices=20, radius1=radius, radius2=radius * 1.35, depth=length,
                                    location=wrist + d * (length * 0.5))
    o = bpy.context.active_object
    o.name = name
    o.data.name = name
    o.rotation_euler = d.to_track_quat("Z", "Y").to_euler()
    apply_all([o])
    bpy.ops.object.shade_smooth()
    skin = bpy.data.materials.get("hand")
    o.data.materials.append(skin)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=0.01)
    bpy.ops.object.mode_set(mode="OBJECT")
    o.data.uv_layers[0].name = "UVMap"
    lo, hi = world_bbox([o])
    say(f"  {name}: from wrist {tuple(round(v, 3) for v in wrist)} along {tuple(round(v, 2) for v in d)}, min={tuple(round(v, 3) for v in lo)} max={tuple(round(v, 3) for v in hi)}")
    return o


def mathutils_euler(deg):
    from mathutils import Euler
    return Euler(tuple(math.radians(d) for d in deg), "XYZ").to_matrix().to_4x4()


# ---- 3. export + sidecar ---------------------------------------------------
def to_engine(v):
    """Blender (x, y, z) -> engine (x, z, -y)."""
    return [round(v.x, 5), round(v.z, 5), round(-v.y, 5)]


def rig_points(parts):
    """The three pivots and the muzzle, in Blender space, from the merged
    parts' boxes: the cylinder spins about its centre, the hammer rocks
    about a point low at its rear, the trigger hangs from its top, and the
    muzzle is the receiver's front on the bore line."""
    lo, hi = world_bbox([parts["cylinder"]])
    cyl_pivot = (lo + hi) * 0.5
    lo, hi = world_bbox([parts["hammer"]])
    hammer_pivot = Vector((lo.x + (hi.x - lo.x) * 0.25, (lo.y + hi.y) * 0.5, lo.z + (hi.z - lo.z) * 0.15))
    lo, hi = world_bbox([parts["trigger"]])
    trigger_pivot = Vector(((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, hi.z))
    lo, hi = world_bbox([parts["receiver"]])
    muzzle = Vector((hi.x, (lo.y + hi.y) * 0.5, (lo.z + hi.z) * 0.5))
    return {"cylinder": cyl_pivot, "hammer": hammer_pivot, "trigger": trigger_pivot}, muzzle


def write_sidecar(parts, grip):
    pivots, muzzle = rig_points(parts)
    rig = {
        "comment": "arena viewmodel pivots in ENGINE space (+X forward, +Y up, +Z right); written by tools/v15/build_viewmodel.py",
        "pivots": {name: to_engine(p) for name, p in pivots.items()},
        "muzzle": to_engine(muzzle),
    }
    with open(OUT_RIG, "w", encoding="utf-8", newline="\n") as f:
        json.dump(rig, f, indent=1)
        f.write("\n")
    say(f"sidecar {os.path.relpath(OUT_RIG, REPO)}: {rig['pivots']} muzzle={rig['muzzle']}")


def export():
    for o in meshes():
        o.select_set(True)
    bpy.ops.export_scene.gltf(
        filepath=OUT_GLB,
        export_format="GLB",
        export_yup=True,
        export_apply=True,
        export_animations=False,
        export_skins=False,
        export_cameras=False,
        export_lights=False,
        export_normals=True,
        export_texcoords=True,
        export_materials="EXPORT",
        export_image_format="AUTO",
    )
    say(f"wrote {os.path.relpath(OUT_GLB, REPO)} ({os.path.getsize(OUT_GLB) // 1024} KB)")


def verify_glb():
    data = open(OUT_GLB, "rb").read()
    ln = struct.unpack("<I", data[12:16])[0]
    js = json.loads(data[20:20 + ln])
    names = [n.get("name") for n in js.get("nodes", [])]
    say(f"glb nodes: {names}")
    want = {"frame", "receiver", "cylinder", "hammer", "trigger", "hand_r", "hand_l", "arm_r", "arm_l"}
    if set(names) != want:
        die(f"node set {sorted(names)} != {sorted(want)}")
    for n in names:
        if (n.startswith("hand") or n.startswith("arm")) and n not in ("hand_r", "hand_l", "arm_r", "arm_l"):
            die(f"stray viewmodel-only name {n}")
    # Every image: 8-bit PNG.
    bin_off = 20 + ln
    blen = struct.unpack("<I", data[bin_off:bin_off + 4])[0]
    blob = data[bin_off + 8:bin_off + 8 + blen]
    for im in js.get("images", []):
        bv = js["bufferViews"][im["bufferView"]]
        png = blob[bv.get("byteOffset", 0):bv.get("byteOffset", 0) + bv["byteLength"]]
        if png[:8] != b"\x89PNG\r\n\x1a\n":
            die(f"image {im.get('name')} is not PNG ({im.get('mimeType')})")
        w, h, depth, ctype = struct.unpack(">IIBB", png[16:26])
        say(f"  image {im.get('name')}: {w}x{h} depth={depth} colour-type={ctype}")
        if depth != 8:
            die(f"image {im.get('name')} is {depth}-bit; the engine decodes 8-bit only, silently")
    for m in js.get("materials", []):
        if "baseColorTexture" not in m.get("pbrMetallicRoughness", {}):
            die(f"material {m.get('name')} exported without its picture")
        f = m.get("pbrMetallicRoughness", {}).get("baseColorFactor", [1, 1, 1, 1])
        if any(abs(c - 1.0) > 1e-3 for c in f[:3]):
            die(f"material {m.get('name')} baseColorFactor {f} is not white")
    say("glb verified: node contract, 8-bit PNG images, white base colour")


def render_previews():
    sc = bpy.context.scene
    sc.render.engine = "BLENDER_WORKBENCH"
    sc.display.shading.light = "STUDIO"
    sc.display.shading.color_type = "TEXTURE"
    sc.render.resolution_x, sc.render.resolution_y = 1280, 720
    sc.world = bpy.data.worlds.new("w")
    sc.world.color = (0.30, 0.32, 0.36)
    cam_data = bpy.data.cameras.new("cam")
    cam_data.lens_unit = "FOV"
    cam_data.angle = math.radians(70)
    cam = bpy.data.objects.new("cam", cam_data)
    sc.collection.objects.link(cam)
    sc.camera = cam
    views = {
        # The first-person eye, in gun space: the client puts the gun 0.5
        # forward, 0.255 right and 0.30 below the eye (online.rs).
        "fps": (Vector((-0.5, 0.255, 0.30)), Vector((0.6, 0.0, -0.05))),
        "side": (Vector((0.1, -1.6, 0.0)), Vector((0.1, 0.0, 0.0))),
        "front": (Vector((1.2, -0.6, 0.35)), Vector((0.0, 0.0, -0.1))),
        "top": (Vector((0.1, 0.0, 1.5)), Vector((0.1, 0.0, 0.0))),
    }
    for name, (eye, at) in views.items():
        fwd = (at - eye).normalized()
        world_up = Vector((0, 0, 1)) if name != "top" else Vector((0, 1, 0))
        right = fwd.cross(world_up).normalized()
        up = right.cross(fwd).normalized()
        # A camera looks down its local -Z with local +Y up.
        cam.matrix_world = Matrix((right, up, -fwd)).transposed().to_4x4()
        cam.location = eye
        sc.render.filepath = os.path.join(TOOLS, f"preview-vm-{name}.png")
        bpy.ops.render.render(write_still=True)
        say(f"preview {sc.render.filepath}")
    bpy.data.objects.remove(cam, do_unlink=True)


def build_revolver():
    """The revolver alone, for a build that imports this file as a library
    (tools/v18/build_weapons.py): the five merged parts in v15's own fit
    (0.75 long, +X muzzle, +Z up, origin on the grip) wearing the 1024
    pictures bake_textures() gives them, plus their pivots and the muzzle in
    BLENDER space, keyed by the plain part names. The caller renames the
    parts, re-sizes the pictures and converts the points; nothing here is
    wiped or exported, so the scene the caller already holds survives."""
    objs = import_revolver(reset=False)
    fit_revolver(objs)
    bake_textures()
    parts = merge_parts(objs)
    pivots, muzzle = rig_points(parts)
    return parts, pivots, muzzle


def main():
    objs = import_revolver()
    grip = fit_revolver(objs)
    bake_textures()
    parts = merge_parts(meshes())
    hand, palm, thumb, is_right, height = import_hand()
    place_hands(hand, palm, thumb, is_right, height)
    if PREVIEW:
        render_previews()
    export()
    write_sidecar(parts, grip)
    verify_glb()


if __name__ == "__main__":
    main()
