#!/usr/bin/env python3
"""Blender headless: the v17 viewmodel - the operator's rifle and hands
(from tools/v16) plus the scutum on Q and the Murasama on E.

    blender --background --python tools/v17/build_viewmodel.py -- [--preview]

Adds three meshes to the v16 export, all in the frames the client expects:

  * `shield`: the scutum (assets/scutum/source/Scutum_low.fbx, one mesh in
    centimetres, 2.05 m tall in the file), fitted to SHIELD_HEIGHT with its
    convex face toward +X - the frame push_shield's box plate used, so the
    client's existing shield centres and yaw work unchanged - and its
    origin at the handle, the back of the board behind the boss. Drawn
    first person while Q is held and on remote players whose PState says
    shield; the box plate stays as the fallback when the part is absent.
  * `sword`: the Murasama (assets/murasama/source/unpacked/murasama/
    murasama.fbx, one mesh, 4.94 units long with the tip at -X and the
    grip at +X, the guard the widest slice), flipped so the blade runs
    along +X with the tip at +X, scaled to SWORD_LENGTH, origin where the
    right hand closes on the grip just below the guard. Its edge faces -Z
    in the file's curve; the client rolls the slash so the edge leads.
  * `hand_sword`: the operator's posed right fist from the v16 build,
    rotated so the rifle-grip axis it closes on becomes the sword's grip
    axis, with a forearm tube of its own in the sword's frame, wearing a
    1024 copy of the body picture (one texture per mesh; the hands' 2048
    copy cannot be shared). Viewmodel-only, drawn with the sword's
    transform during the slash.

Outputs: crates/arena/assets/viewmodel.glb (rifle, hands, shield, sword,
hand_sword), viewmodel-rig.json (the rifle's muzzle), tools/v17/preview-*.png.
Run tools/v16/prep_pictures.py and tools/v17/prep_pictures.py first.
"""
import json
import math
import os
import struct
import sys

import bpy
from mathutils import Matrix, Vector

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(REPO, "tools", "v16"))
import build_operator_viewmodel as v16  # noqa: E402

SCUTUM_FBX = os.path.join(REPO, "assets", "scutum", "source", "Scutum_low.fbx")
SCUTUM_PICTURE = os.path.join(REPO, "assets", "scutum", "baked", "scutum-1024.png")
SWORD_FBX = os.path.join(REPO, "assets", "murasama", "source", "unpacked", "murasama", "murasama.fbx")
SWORD_PICTURE = os.path.join(REPO, "assets", "murasama", "baked", "murasama-1024.png")
FIST_PICTURE = os.path.join(REPO, "assets", "swat", "baked", "body-1024.png")
OUT_GLB = v16.OUT_GLB
OUT_RIG = v16.OUT_RIG
PREVIEW_DIR = HERE

# Metres. The box plate was 0.54 tall by 0.46 wide; the scutum keeps the
# width and gains height, as a scutum does. The Murasama is a long katana.
SHIELD_HEIGHT = 0.85
SWORD_LENGTH = 1.05
# Where along the grip (0 = guard, 1 = pommel) the right hand closes.
SWORD_HAND_AT = 0.32
# Where the fist goes on the sword's grip. Both frames are measured, not
# assumed: in the rifle frame the fist's grip axis is the knuckle line from
# the index to the little finger (the pistol grip it closes on) and its palm
# faces the way the fingers leave the knuckles. On the sword the grip axis
# runs toward the pommel (-X) and a right hand holding a blade edge-down has
# its palm toward the left (+Y).
SWORD_GRIP_AXIS = Vector((-1.0, 0.0, 0.0))
SWORD_PALM = Vector((0.0, 1.0, 0.0))
FIST_NUDGE = Vector((0.0, 0.0, 0.0))
# The sword hand's forearm, from its wrist, in the sword's frame: back,
# right and a little down - a one-hand hold at the right of the screen.
ELBOW_DIR_SWORD = (-0.55, -0.75, -0.30)
# A cuff, not a whole arm. The rifle's 0.36 m tube reached from a 0.6 m
# hold back to the near plane and filled the screen with black.
FOREARM_LENGTH_SWORD = 0.17

say, die, fmt, world_bbox, apply_all = v16.say, v16.die, v16.fmt, v16.world_bbox, v16.apply_all
picture_material, set_material, smooth, unparent_keep_world = v16.picture_material, v16.set_material, v16.smooth, v16.unparent_keep_world


def import_single(path, name):
    """Import an FBX that holds one mesh; return it, in world space."""
    if not os.path.isfile(path):
        die(f"missing {path}")
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path, use_anim=False)
    new = [o for o in bpy.data.objects if o not in before]
    meshes = [o for o in new if o.type == "MESH"]
    if len(meshes) != 1:
        die(f"{path}: expected one mesh, got {[(o.type, o.name) for o in new]}")
    for o in new:
        if o is not meshes[0]:
            bpy.data.objects.remove(o, do_unlink=True)
    obj = meshes[0]
    unparent_keep_world(obj)
    apply_all([obj])
    obj.name = name
    obj.data.name = name
    if not obj.data.uv_layers:
        die(f"{name} has no UVs")
    obj.data.uv_layers[0].name = "UVMap"
    lo, hi = world_bbox([obj])
    say(f"{name}: {len(obj.data.polygons)} faces, box {fmt(lo)}..{fmt(hi)}")
    return obj


def frame_and_fit(obj, rot, origin_before, scale):
    """origin_before (file frame) becomes the origin; rot (a Matrix) takes
    the file frame to the target frame; then the uniform scale."""
    obj.matrix_world = Matrix.Scale(scale, 4) @ rot.to_4x4() @ Matrix.Translation(-origin_before)
    apply_all([obj])


# ---- the scutum -------------------------------------------------------------
def build_shield():
    obj = import_single(SCUTUM_FBX, "shield")
    pts = [v.co for v in obj.data.vertices]
    lo, hi = world_bbox([obj])
    # The convex face: the centre column bulges to one side of the edges.
    centre = [p for p in pts if abs(p.x) < 0.08 * (hi.x - lo.x)]
    edges = [p for p in pts if abs(p.x) > 0.44 * (hi.x - lo.x)]
    centre_y = sum(p.y for p in centre) / len(centre)
    edges_y = sum(p.y for p in edges) / len(edges)
    front = -1.0 if centre_y < edges_y else 1.0  # sign of Y the face points to
    say(f"shield: centre column y {centre_y:.3f}, edges y {edges_y:.3f} -> face toward {'+' if front > 0 else '-'}Y")
    # Face (front*Y) -> +X, keep Z up: a yaw about Z.
    rot = Matrix.Rotation(math.radians(-90.0 * front), 3, "Z")
    # The handle: back of the board at the centre, mid height.
    back_y = centre_y - front * 0.03 * (hi.y - lo.y) / 0.35
    origin = Vector((0.0, centre_y - front * 0.02, (lo.z + hi.z) * 0.5))
    _ = back_y
    scale = SHIELD_HEIGHT / (hi.z - lo.z)
    frame_and_fit(obj, rot, origin, scale)
    set_material(obj, picture_material("scutum", SCUTUM_PICTURE, 1024))
    smooth(obj)
    lo, hi = world_bbox([obj])
    if not (hi.x > 0.0 > lo.x and hi.x - lo.x < 0.3 and hi.z - lo.z > 0.8):
        die(f"shield frame wrong: box {fmt(lo)}..{fmt(hi)}")
    say(f"shield: fitted, box {fmt(lo)}..{fmt(hi)} (face at +X {hi.x:.3f}, {hi.z - lo.z:.3f} tall, {hi.y - lo.y:.3f} wide)")
    return obj


# ---- the Murasama -----------------------------------------------------------
def build_sword():
    obj = import_single(SWORD_FBX, "sword")
    pts = [v.co for v in obj.data.vertices]
    xs = [p.x for p in pts]
    lo_x, hi_x = min(xs), max(xs)
    length = hi_x - lo_x

    def extent(a, b):
        sl = [p for p in pts if a <= p.x <= b]
        if len(sl) < 5:
            return 0.0, 0.0
        return (max(p.y for p in sl) - min(p.y for p in sl)) + (max(p.z for p in sl) - min(p.z for p in sl)), len(sl)

    # The tip is the thin end; the guard is the widest slice.
    end_lo = extent(lo_x, lo_x + 0.08 * length)[0]
    end_hi = extent(hi_x - 0.08 * length, hi_x)[0]
    tip_at_max = end_hi < end_lo
    best = None
    for i in range(40):
        a = lo_x + i * length / 40.0
        ext, n = extent(a, a + length / 40.0)
        if best is None or ext > best[0]:
            best = (ext, a + length / 80.0)
    guard_x = best[1]
    pommel_x = hi_x if not tip_at_max else lo_x
    tip_x = lo_x if not tip_at_max else hi_x
    hand_x = guard_x + (pommel_x - guard_x) * SWORD_HAND_AT
    grip = [p for p in pts if min(guard_x, pommel_x) <= p.x <= max(guard_x, pommel_x)]
    grip_yz = Vector((0.0, sum(p.y for p in grip) / len(grip), sum(p.z for p in grip) / len(grip)))
    say(f"sword: tip at {'max' if tip_at_max else 'min'} X ({tip_x:.3f}), guard at x {guard_x:.3f}, pommel at {pommel_x:.3f}, hand at x {hand_x:.3f}")
    origin = Vector((hand_x, grip_yz.y, grip_yz.z))
    # Tip to +X: identity if it already is, else a half turn about Z.
    rot = Matrix.Identity(3) if tip_at_max else Matrix.Rotation(math.pi, 3, "Z")
    scale = SWORD_LENGTH / length
    frame_and_fit(obj, rot, origin, scale)
    set_material(obj, picture_material("murasama", SWORD_PICTURE, 1024))
    smooth(obj)
    lo, hi = world_bbox([obj])
    if not (hi.x > 0.6 and lo.x < -0.05 and hi.y - lo.y < 0.2):
        die(f"sword frame wrong: box {fmt(lo)}..{fmt(hi)}")
    say(f"sword: fitted, box {fmt(lo)}..{fmt(hi)} (tip at +X {hi.x:.3f}, pommel at {lo.x:.3f})")
    return obj, (guard_x - hand_x) * scale * (1.0 if tip_at_max else -1.0)


# ---- the fist on the grip ----------------------------------------------------
def build_fist(extras):
    """Rotate the operator's right fist (rifle frame) onto the sword grip:
    the grip axis it closes on becomes -X (guard to pommel), its knuckles
    face -Z (the edge). Put the fist's centre on the grip, then hang a
    forearm off its wrist in the sword's frame and join both into
    `hand_sword` (the prefix keeps it viewmodel-only even under the old
    name rule)."""
    obj = extras["fist"]
    obj.name = "hand_sword"
    obj.data.name = "hand_sword"
    marks = extras["landmarks"]
    grip = (marks["RightHandPinky1"] - marks["RightHandIndex1"]).normalized()
    palm = marks["RightHandIndex2"] - marks["RightHandIndex1"]
    palm = (palm - grip * palm.dot(grip)).normalized()
    say(f"fist: grip axis {fmt(grip)}, palm {fmt(palm)} in the rifle frame")
    src = Matrix((grip, palm, grip.cross(palm))).transposed()
    want_palm = (SWORD_PALM - SWORD_GRIP_AXIS * SWORD_PALM.dot(SWORD_GRIP_AXIS)).normalized()
    dst = Matrix((SWORD_GRIP_AXIS, want_palm, SWORD_GRIP_AXIS.cross(want_palm))).transposed()
    rot = dst @ src.transposed()
    # The fist proper (not the forearm) sits where the hand bone's vertices
    # are: its box centre in the rifle frame is near the pistol grip; the
    # forearm trails. Rotate about the fist's centre, then move that centre
    # to the grip point.
    # What lands on the sword's grip: the hole the grip passes through,
    # between the index knuckle and the second joint of the thumb.
    centre = (marks["RightHandIndex1"] + marks["RightHandThumb2"]) * 0.5
    obj.matrix_world = Matrix.Translation(FIST_NUDGE) @ rot.to_4x4() @ Matrix.Translation(-centre)
    apply_all([obj])
    mat = picture_material("fist-body", FIST_PICTURE, 1024)
    set_material(obj, mat)
    smooth(obj)
    wrist = rot @ (extras["wrist_r"] - centre) + FIST_NUDGE
    v16.ELBOW_DIR["arm_sword"] = ELBOW_DIR_SWORD
    was = v16.FOREARM_LENGTH
    v16.FOREARM_LENGTH = FOREARM_LENGTH_SWORD
    tube = v16.forearm_tube("arm_sword", wrist, extras["sleeve_uv_r"], mat)
    v16.FOREARM_LENGTH = was
    joined = v16.copy_joined("hand_sword_joined", [obj, tube])
    for leftover in (obj, tube):
        # Free the mesh data too, or the name below lands on ".001".
        data = leftover.data
        bpy.data.objects.remove(leftover, do_unlink=True)
        bpy.data.meshes.remove(data)
    joined.name = "hand_sword"
    joined.data.name = "hand_sword"
    lo, hi = world_bbox([joined])
    say(f"hand_sword: {len(joined.data.polygons)} faces, box {fmt(lo)}..{fmt(hi)} about the grip point; wrist {fmt(wrist)}")
    return joined


# ---- export ---------------------------------------------------------------------
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
    rig = {"pivots": {}, "muzzle": v16.to_engine(muzzle)}
    with open(OUT_RIG, "w", encoding="utf-8", newline="\n") as f:
        json.dump(rig, f, indent=1)
        f.write("\n")
    say(f"wrote {OUT_GLB} ({os.path.getsize(OUT_GLB) // 1024} KB) and {OUT_RIG}")


def verify_glb():
    with open(OUT_GLB, "rb") as f:
        magic, _version, _length = struct.unpack("<III", f.read(12))
        if magic != 0x46546C67:
            die("not a GLB")
        clen, ctype = struct.unpack("<II", f.read(8))
        if ctype != 0x4E4F534A:
            die("first chunk is not JSON")
        doc = json.loads(f.read(clen))
    want = ["hand_sword", "hands", "rifle", "shield", "sword"]
    names = sorted(m.get("name") for m in doc.get("meshes", []))
    nodes = sorted(n.get("name") for n in doc.get("nodes", []))
    # The engine reads NODE names; Blender names nodes after objects and
    # meshes after mesh data, so both lists are checked.
    if names != want or nodes != want:
        die(f"expected meshes and nodes {want}; got meshes {names}, nodes {nodes}")
    for m in doc["meshes"]:
        if len(m["primitives"]) != 1:
            die(f"mesh {m['name']!r} has {len(m['primitives'])} primitives; one picture per mesh")
        mat = doc["materials"][m["primitives"][0]["material"]]
        pbr = mat.get("pbrMetallicRoughness", {})
        if "baseColorTexture" not in pbr:
            die(f"mesh {m['name']!r} has a material without a picture")
        if pbr.get("baseColorFactor", [1, 1, 1, 1])[:3] != [1, 1, 1]:
            die(f"mesh {m['name']!r} has a tinted baseColorFactor")
    images = doc.get("images", [])
    mimes = [i.get("mimeType") for i in images]
    if len(images) != 5 or any(m != "image/png" for m in mimes):
        die(f"expected five PNG images, got {mimes}")
    say(f"verified: meshes and nodes {names}, {len(images)} PNG pictures")


def render_previews(named):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.render.resolution_x = 1024
    scene.render.resolution_y = 640
    scene.render.image_settings.file_format = "PNG"
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    all_objs = [o for group in named.values() for o in group]
    for o in all_objs:
        o.hide_render = True
    for name, objs in named.items():
        for o in objs:
            o.hide_render = False
        lo, hi = world_bbox(objs)
        centre = (lo + hi) * 0.5
        span = max(hi - lo) * 1.6 + 0.2
        for view, offset, up in (("side", Vector((0.0, -span, 0.15)), "Y"), ("front", Vector((span, 0.0, 0.15)), "Y"), ("top", Vector((0.0, 0.0, span)), "X")):
            cam_data.angle = math.radians(45.0)
            cam.location = centre + offset
            cam.rotation_euler = (centre - cam.location).normalized().to_track_quat("-Z", up).to_euler()
            scene.render.filepath = os.path.join(PREVIEW_DIR, f"preview-{name}-{view}.png")
            bpy.ops.render.render(write_still=True)
        for o in objs:
            o.hide_render = True
        say(f"previews for {name}: side/front/top")
    for o in all_objs:
        o.hide_render = False


def main():
    preview = "--preview" in sys.argv
    rifle, hands, muzzle, extras = v16.build_operator()
    shield = build_shield()
    sword, _guard = build_sword()
    fist = build_fist(extras)
    objs = [rifle, hands, shield, sword, fist]
    export(objs, muzzle)
    verify_glb()
    if preview:
        render_previews({"operator": [rifle, hands], "shield": [shield], "sword": [sword, fist]})


if __name__ == "__main__":
    main()
