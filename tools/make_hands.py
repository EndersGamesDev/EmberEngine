"""Generate the viewmodel HANDS + forearms as a standalone GLB.

Run headless:
    blender --background --python tools/make_hands.py -- crates/arena/assets/hands.glb

Conventions (must match the engine): +X forward, +Z up in Blender; the
default Y-up glTF export maps this to +X forward / +Y up in the engine.
Units are game units (the pistol is ~0.9 long). Node names matter:
parts starting with "arm"/"hand" are viewmodel-only (not drawn on remote
players); the part named "strip" gets recolored by weapon level.

WHY THIS FILE EXISTS
    tools/make_assets.py used to emit the whole viewmodel — a box pistol
    AND the hands — as 12 flat boxes. The pistol half is being replaced by
    the real 9mm model, so the hands were split out here. They were
    originally posed against the BOX pistol's grip, so every number under
    "FIT PARAMETERS" below is a first guess that must be re-measured
    against the real 9mm grip. Geometry is authored in a grip-local frame
    precisely so that re-fitting is a matter of editing those constants,
    never of re-authoring boxes.

SHAPE
    Four exported nodes, one mesh id each: hand_r, arm_r, hand_l, arm_l.
    Each hand is a palm + a finger block + a thumb, welded into ONE mesh
    object (three boxes, one material) — a richer silhouette than the old
    single cube for exactly the same runtime cost. Each forearm is a
    tapered-looking pair (cuff + forearm), also one object. 120 triangles
    total. Keep it that way: one material per object, because glTF emits
    one primitive per material and the engine allocates one mesh id and
    one texture upload per primitive.

CONSUMER NOTE (as of writing)
    crates/arena/src/online.rs embeds ONLY `../assets/viewmodel.glb`.
    Nothing loads hands.glb yet, so writing this file changes nothing at
    runtime until the Rust side embeds it (or until the 9mm pipeline
    merges gun + hands into viewmodel.glb). Until then the shipped
    viewmodel still comes from `make_assets.py --with-pistol`.

NOTHING HERE HAS BEEN RUN. There is no Blender on the authoring machine,
so every number below is unverified arithmetic. The script is written to
be loud on its first real run: it prints each part's measured bounds in
BOTH Blender and engine space, checks them against the grip it is
supposed to be wrapping, and re-imports its own GLB to confirm the node
names and materials survived the round trip.
"""

import json
import math
import os
import sys

import bmesh
import bpy
from mathutils import Euler, Matrix, Vector

# --------------------------------------------------------------------------
# FIT PARAMETERS — the whole point of this file. Everything else is derived.
# --------------------------------------------------------------------------

# Where the firing hand's PALM CENTRE sits, in gun space (origin = hand
# anchor, +X = muzzle). The legacy box pistol's grip box was centred at
# (0.02, 0, -0.14) with dims (0.15, 0.11, 0.28), i.e. it spanned
# x -0.055..0.095, y -0.055..0.055, z -0.28..0.0 — this anchor sits in the
# upper half of that column, where a hand actually grips.
# TODO(fit): re-measure against the real 9mm. Import
# assets/9mm/source/*.fbx, select the Frame_low part, and read off the
# grip's world AABB after the model is scaled to the ~0.9-unit convention;
# the anchor wants to be ~1/3 down the backstrap from the beavertail.
GRIP_ANCHOR = Vector((0.015, -0.005, -0.135))

# Rake of the grip about +Y, in degrees. Positive tilts the BUTT rearward
# (-X), which is how a real pistol grip leans.
# The legacy box pistol hardcoded -14 here, the OPPOSITE lean, leaving the
# hands 28 degrees off the grip they share a GLB with. make_assets.py now
# reads THIS constant for its fallback grip, so the two cannot drift again.
# TODO(fit): measure the 9mm's true backstrap angle rather than trusting
# either number.
GRIP_ANGLE_DEG = 14.0

# Lateral gap between the two palms, along +Y. Pure separation: how far
# the support hand sits to the side of the firing hand.
# TODO(fit): depends on the real grip's width (the box grip was 0.11 wide).
# Too small and the hands interpenetrate; too large and the support hand
# floats off the frame.
HAND_SEPARATION = 0.050

# Support hand relative to the firing hand, in grip-local space, ON TOP of
# HAND_SEPARATION: (forward, extra lateral, vertical). Negative Z is the
# "wrapping under" part — the support hand cradles the butt from below.
# TODO(fit): fit last, after the anchor and the angle are settled.
SUPPORT_OFFSET = Vector((0.035, 0.0, -0.070))

# Which way is +Y on screen. The Blender->engine map is (x, y, z) ->
# (x, z, -y), so the sign of the lateral axis is easy to get backwards and
# impossible to check without running the game.
# TODO(fit): if the arms enter frame from the wrong side, or the thumbs end
# up on the wrong side of the frame, set this to -1.0. That is the only
# change needed: it mirrors the hand GEOMETRY (via lateral_mirror()) as
# well as every lateral position (via flip()), so handedness flips as a
# whole rather than leaving both thumbs pointing the wrong way.
LATERAL_SIGN = 1.0

# Where each forearm runs to (roughly the shoulder/screen exit), in gun
# space. The forearm is built as the segment from the wrist to this point,
# so moving GRIP_ANCHOR re-aims the arms automatically.
# TODO(fit): these want checking against the actual viewmodel camera
# offset in online.rs (base = eye + forward*0.5 + right*0.24 ...), not
# against the model in isolation.
ARM_R_ANCHOR = Vector((-0.62, -0.26, -0.62))
ARM_L_ANCHOR = Vector((-0.55, 0.34, -0.72))

# --------------------------------------------------------------------------
# Shape constants — stylized, boxy, low-poly on purpose.
# --------------------------------------------------------------------------

# Canonical RIGHT hand, in hand-local space: +X forward, +Y left, +Z up,
# palm centre at the origin. The left hand is this mirrored in Y.
# (name, centre, dims, rotation in degrees)
HAND_BOXES = [
    # Palm: sits behind the backstrap, the bulk of the hand.
    ("palm", (-0.055, 0.000, 0.005), (0.070, 0.105, 0.165), (0.0, 0.0, 0.0)),
    # Fingers: one block wrapping the front strap, curled slightly down.
    ("fingers", (0.075, -0.005, -0.015), (0.075, 0.100, 0.140), (0.0, -6.0, 0.0)),
    # Thumb: rides high on the +Y side of the frame, pointing forward.
    ("thumb", (0.015, 0.062, 0.045), (0.115, 0.048, 0.048), (0.0, -12.0, 8.0)),
]

# Where the forearm leaves the hand, in hand-local space.
WRIST_LOCAL = Vector((-0.090, 0.0, -0.030))
FOREARM_THICK = 0.115
CUFF_LEN = 0.100
CUFF_THICK = 0.140

# Base colors, straight into Principled BSDF base color (the engine reads
# base_color_factor per primitive; there is no texture on these parts).
SKIN_RGB = (0.62, 0.44, 0.33)
GLOVE_RGB = (0.15, 0.12, 0.11)
SLEEVE_RGB = (0.20, 0.24, 0.30)
# Swap to SKIN_RGB for bare hands.
HAND_RGB = GLOVE_RGB

# Reference only: the legacy box pistol's grip AABB in gun space, used to
# report the fit on stdout. It is NOT geometry — nothing is generated from
# it. Replace the numbers once the 9mm grip has been measured.
REFERENCE_GRIP_AABB = ((-0.055, -0.055, -0.280), (0.095, 0.055, 0.000))

DEFAULT_OUT = os.path.join("crates", "pong", "assets", "hands.glb")


# --------------------------------------------------------------------------
# Helpers
# --------------------------------------------------------------------------


def flip(v):
    """LATERAL_SIGN applied to a POINT (the arm anchors). Hands go through
    lateral_mirror() instead, which flips their geometry too."""
    return Vector((v[0], LATERAL_SIGN * v[1], v[2]))


def material(name, rgb):
    """A flat Principled material whose base color the engine reads back as
    the per-part instance color."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    if bsdf is None:
        # Blender localizes/renames default nodes across versions; never
        # fall through with a white material, that is a silent regression.
        bsdf = next(
            (n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None
        )
    if bsdf is None:
        raise SystemExit(
            f"[hands] FATAL: material {name!r} has no Principled BSDF node "
            f"(nodes: {[n.type for n in m.node_tree.nodes]}). The exporter "
            f"would write a white base color and every part would be white."
        )
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    return m


def box_matrix(centre, dims, rot_deg=(0.0, 0.0, 0.0)):
    """Transform placing a unit cube at `centre` with size `dims`."""
    if min(dims) <= 0.0:
        raise SystemExit(f"[hands] FATAL: degenerate box dims {dims}")
    rot = Euler([math.radians(a) for a in rot_deg], "XYZ").to_matrix().to_4x4()
    return (
        Matrix.Translation(Vector(centre))
        @ rot
        @ Matrix.Diagonal(Vector((dims[0], dims[1], dims[2], 1.0)))
    )


def make_part(name, matrices, mat):
    """One exported node = one mesh object = several boxes, ONE material."""
    bm = bmesh.new()
    for mtx in matrices:
        try:
            bmesh.ops.create_cube(bm, size=1.0, matrix=mtx, calc_uvs=True)
        except TypeError:
            # Older bmesh has no calc_uvs; the parts are untextured anyway
            # and the engine defaults missing TEXCOORD_0 to (0, 0).
            bmesh.ops.create_cube(bm, size=1.0, matrix=mtx)
    # A mirrored matrix (the left hand) inverts winding: without this the
    # left hand renders inside-out / black and nothing warns about it.
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    obj.data.materials.append(mat)
    bpy.context.scene.collection.objects.link(obj)
    if obj.name != name:
        raise SystemExit(
            f"[hands] FATAL: object renamed {name!r} -> {obj.name!r} (name "
            f"collision). The runtime classifier keys on the exact node name."
        )
    return obj


MIRROR_Y = Matrix.Diagonal(Vector((1.0, -1.0, 1.0, 1.0)))


def lateral_mirror():
    """The LATERAL_SIGN flip as a matrix. It must mirror GEOMETRY as well
    as positions — negating y-coordinates alone would move the hands to
    the other side of the gun while leaving both thumbs pointing the wrong
    way. A Y mirror commutes with the grip's Y rotation, so applying it
    outermost leaves GRIP_ANGLE_DEG meaning exactly what it says.
    """
    return MIRROR_Y if LATERAL_SIGN < 0 else Matrix.Identity(4)


def hand_matrix(mirrored, offset_local):
    """Grip-local frame for one hand: anchor, then grip rake, then the
    hand's own offset, then the Y mirror for the left hand."""
    m = (
        lateral_mirror()
        @ Matrix.Translation(GRIP_ANCHOR)
        @ Matrix.Rotation(math.radians(GRIP_ANGLE_DEG), 4, "Y")
        @ Matrix.Translation(offset_local)
    )
    if mirrored:
        m = m @ MIRROR_Y
    return m


def forearm_matrices(wrist, anchor):
    """Cuff + forearm running from the wrist to a shoulder anchor."""
    direction = anchor - wrist
    length = direction.length
    if length < 0.05:
        raise SystemExit(
            f"[hands] FATAL: forearm length {length:.3f} — the shoulder "
            f"anchor {tuple(round(v, 3) for v in anchor)} is on top of the "
            f"wrist {tuple(round(v, 3) for v in wrist)}."
        )
    # to_track_quat aligns local +X with the direction, +Z staying up-ish.
    rot = direction.to_track_quat("X", "Z").to_matrix().to_4x4()
    unit = direction.normalized()
    forearm = (
        Matrix.Translation(wrist + direction * 0.5)
        @ rot
        @ Matrix.Diagonal(Vector((length, FOREARM_THICK, FOREARM_THICK, 1.0)))
    )
    cuff = (
        Matrix.Translation(wrist + unit * (CUFF_LEN * 0.45))
        @ rot
        @ Matrix.Diagonal(Vector((CUFF_LEN, CUFF_THICK, CUFF_THICK, 1.0)))
    )
    return [cuff, forearm], length


# --------------------------------------------------------------------------
# Build
# --------------------------------------------------------------------------


def build_hands():
    """Create the four hand/arm objects in the current scene and return
    them. Does NOT reset the scene — make_assets.py calls this after
    building its fallback pistol.
    """
    glove = material("glove", HAND_RGB)
    sleeve = material("sleeve", SLEEVE_RGB)

    made = []
    for side, mirrored, offset, arm_anchor in (
        ("r", False, Vector((0.0, 0.0, 0.0)), ARM_R_ANCHOR),
        (
            "l",
            True,
            Vector(
                (
                    SUPPORT_OFFSET.x,
                    HAND_SEPARATION + SUPPORT_OFFSET.y,
                    SUPPORT_OFFSET.z,
                )
            ),
            ARM_L_ANCHOR,
        ),
    ):
        hm = hand_matrix(mirrored, offset)
        boxes = [
            hm @ box_matrix(centre, dims, rot)
            for _label, centre, dims, rot in HAND_BOXES
        ]
        made.append(make_part(f"hand_{side}", boxes, glove))

        wrist = hm @ WRIST_LOCAL
        arm_boxes, length = forearm_matrices(wrist, flip(arm_anchor))
        print(
            f"[hands] arm_{side}: wrist={fmt(wrist)} -> "
            f"anchor={fmt(flip(arm_anchor))} len={length:.3f}"
        )
        made.append(make_part(f"arm_{side}", arm_boxes, sleeve))

    return made


# --------------------------------------------------------------------------
# Reporting / self-checks — this script's first real run is its only test.
# --------------------------------------------------------------------------


def fmt(v):
    return "(" + ", ".join(f"{c:+.3f}" for c in v) + ")"


def world_aabb(obj):
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def to_engine(v):
    """Blender (x, y, z) -> engine (x, z, -y), matching export_yup=True."""
    return Vector((v.x, v.z, -v.y))


def overlaps(a_lo, a_hi, b_lo, b_hi):
    return all(a_lo[i] < b_hi[i] and b_lo[i] < a_hi[i] for i in range(3))


def report(objs, expect_all_arms=True):
    """Measure everything and shout about anything that smells wrong.
    Returns (per-part data, problems).

    `expect_all_arms` is True for a hands-only asset, where a part whose
    name does not start with arm/hand is a bug. make_assets.py passes
    False when it emits the fallback pistol alongside the hands, since
    those parts are SUPPOSED to land in the gun list.
    """
    # matrix_world is lazily evaluated: without this, object-level scale
    # set after creation (make_assets.py's pistol boxes do exactly that)
    # would be missing from every measurement below.
    bpy.context.view_layer.update()
    grip_lo = Vector(REFERENCE_GRIP_AABB[0])
    grip_hi = Vector(REFERENCE_GRIP_AABB[1])
    problems = []
    data = {}
    tris = 0

    print("[hands] --- parts (blender space: +X fwd, +Y left, +Z up) ---")
    for obj in objs:
        lo, hi = world_aabb(obj)
        dims = hi - lo
        n_tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
        tris += n_tris
        e_lo, e_hi = to_engine(lo), to_engine(hi)
        # The Y flip in to_engine() swaps min/max on that axis.
        e_min = Vector([min(e_lo[i], e_hi[i]) for i in range(3)])
        e_max = Vector([max(e_lo[i], e_hi[i]) for i in range(3)])
        print(
            f"[hands]   {obj.name:<7} tris={n_tris:<3} "
            f"min={fmt(lo)} max={fmt(hi)} dim={fmt(dims)}"
        )
        print(f"[hands]   {'':<7} engine  min={fmt(e_min)} max={fmt(e_max)}")
        data[obj.name] = {
            "blender_min": [round(c, 4) for c in lo],
            "blender_max": [round(c, 4) for c in hi],
            "engine_min": [round(c, 4) for c in e_min],
            "engine_max": [round(c, 4) for c in e_max],
            "tris": n_tris,
            "materials": [m.name for m in obj.data.materials],
        }

        # The runtime classifier (crates/arena/src/online.rs) sorts parts by
        # name prefix; anything else lands in the GUN list and gets drawn on
        # remote players' bodies.
        if expect_all_arms and not (
            obj.name.startswith("arm") or obj.name.startswith("hand")
        ):
            problems.append(
                f"{obj.name!r} does not start with arm/hand -> it would be "
                f"classified as GUN geometry and drawn on remote players"
            )
        # >1 material means >1 glTF primitive means >1 mesh id + upload.
        if len(obj.data.materials) != 1:
            problems.append(
                f"{obj.name!r} has {len(obj.data.materials)} materials; each "
                f"one becomes its own primitive/mesh id"
            )
        if min(dims) <= 0.0:
            problems.append(f"{obj.name!r} is degenerate: dim={fmt(dims)}")

    print(f"[hands] total {len(objs)} parts, {tris} triangles")

    # Fit against the grip this is supposed to be wrapping.
    print("[hands] --- fit vs reference grip AABB ---")
    print(f"[hands]   grip  min={fmt(grip_lo)} max={fmt(grip_hi)}")
    for name in ("hand_r", "hand_l"):
        obj = next((o for o in objs if o.name == name), None)
        if obj is None:
            problems.append(f"missing part {name!r}")
            continue
        lo, hi = world_aabb(obj)
        if overlaps(lo, hi, grip_lo, grip_hi):
            print(f"[hands]   {name}: overlaps the grip volume — plausible")
        else:
            # A WARNING, deliberately not a `problems` entry: the reference
            # AABB is the LEGACY box grip and goes stale the moment the 9mm
            # lands. It must not hard-fail a run that is correct against the
            # real gun — but the fit is exactly what needs eyes, so it is
            # still shouted about.
            print(
                f"[hands] !!! {name} does NOT overlap the reference grip "
                f"volume — either the hand is floating off the gun (adjust "
                f"GRIP_ANCHOR) or REFERENCE_GRIP_AABB is stale"
            )
    hr = next((o for o in objs if o.name == "hand_r"), None)
    hl = next((o for o in objs if o.name == "hand_l"), None)
    if hr and hl:
        r_lo, r_hi = world_aabb(hr)
        l_lo, l_hi = world_aabb(hl)
        if overlaps(r_lo, r_hi, l_lo, l_hi):
            print(
                "[hands]   hand_r/hand_l bounds intersect — expected for a "
                "two-handed grip, but check for gross interpenetration"
            )
        else:
            print(
                "[hands]   hand_r/hand_l bounds are disjoint — the support "
                "hand may read as detached (check HAND_SEPARATION)"
            )

    if problems:
        print(f"[hands] !!! {len(problems)} PROBLEM(S):")
        for p in problems:
            print(f"[hands] !!!   {p}")
    else:
        print("[hands] all structural checks passed")
    return data, problems


def export_glb(objs, out):
    """Export exactly `objs` (selection-scoped, so a stray object in the
    scene can never sneak into the asset)."""
    out = os.path.abspath(out)
    parent = os.path.dirname(out)
    if parent:
        os.makedirs(parent, exist_ok=True)

    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]

    res = bpy.ops.export_scene.gltf(
        filepath=out,
        export_format="GLB",
        export_yup=True,
        use_selection=True,
    )
    if "FINISHED" not in res:
        raise SystemExit(f"[hands] FATAL: gltf export returned {res}")
    if not os.path.exists(out) or os.path.getsize(out) == 0:
        raise SystemExit(f"[hands] FATAL: {out} missing or empty after export")
    print(f"[hands] wrote {out} ({os.path.getsize(out)} bytes)")
    return out


def verify_roundtrip(out, expected):
    """Re-import the GLB we just wrote and confirm the engine will see what
    we think it will: the same node names, one primitive each, the base
    colors intact. Non-fatal — a failure here is a WARN, not a wedge."""
    print("[hands] --- verifying round trip (re-importing the GLB) ---")
    try:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.ops.import_scene.gltf(filepath=out)
    except Exception as e:  # noqa: BLE001 — diagnostics must never wedge
        print(f"[hands] WARN: could not re-import {out}: {e}")
        return
    got = sorted(o.name for o in bpy.data.objects if o.type == "MESH")
    want = sorted(expected)
    print(f"[hands]   node names: {got}")
    if got != want:
        print(f"[hands] !!! round-trip node names changed: want {want}")
        print(
            "[hands] !!! the runtime classifier keys on these exact names "
            "(arm*/hand* = viewmodel-only)"
        )
    for o in bpy.data.objects:
        if o.type != "MESH":
            continue
        mats = [m.name for m in o.data.materials]
        if len(mats) != 1:
            print(f"[hands] !!! {o.name} came back with {len(mats)} materials {mats}")
        for m in o.data.materials:
            if m is None or not m.use_nodes:
                print(f"[hands] !!! {o.name} lost its material on export")
                continue
            bsdf = next(
                (n for n in m.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None
            )
            col = tuple(round(c, 3) for c in bsdf.inputs["Base Color"].default_value[:3]) if bsdf else None
            print(f"[hands]   {o.name}: material={m.name} base_color={col}")
            if col == (1.0, 1.0, 1.0):
                print(
                    f"[hands] !!! {o.name} base color is pure white — the "
                    f"engine multiplies it by the instance color, so the "
                    f"part will not read as skin/glove"
                )


def parse_args(argv):
    args = argv[argv.index("--") + 1 :] if "--" in argv else []
    out = None
    flags = {a for a in args if a.startswith("-")}
    for a in args:
        if not a.startswith("-") and out is None:
            out = a
    return out, flags


def repo_default_out():
    """Default output derived from THIS file's location, never from cwd."""
    try:
        here = os.path.dirname(os.path.abspath(__file__))
    except NameError:
        here = os.path.abspath("tools")
    return os.path.join(os.path.dirname(here), DEFAULT_OUT)


def main():
    out, flags = parse_args(sys.argv)
    if out is None:
        out = repo_default_out()
        print(f"[hands] no output path given, defaulting to {out}")

    print(f"[hands] blender {bpy.app.version_string} ({bpy.app.version})")
    print(f"[hands] script  {globals().get('__file__', '<unknown>')}")
    print(f"[hands] output  {os.path.abspath(out)}")
    print(
        f"[hands] fit: anchor={fmt(GRIP_ANCHOR)} angle={GRIP_ANGLE_DEG}deg "
        f"sep={HAND_SEPARATION} support={fmt(SUPPORT_OFFSET)} "
        f"lateral_sign={LATERAL_SIGN:+.0f}"
    )
    if bpy.app.version < (3, 0, 0):
        print(
            "[hands] WARN: untested on Blender < 3.0 — check that "
            "export_yup is still honoured"
        )

    bpy.ops.wm.read_factory_settings(use_empty=True)
    objs = build_hands()

    stray = [o for o in bpy.data.objects if o not in objs]
    if stray:
        print(f"[hands] WARN: {len(stray)} stray object(s) in scene: "
              f"{[o.name for o in stray]} (export is selection-scoped, so "
              f"they are excluded)")

    data, problems = report(objs)
    names = [o.name for o in objs]
    out = export_glb(objs, out)

    if "--manifest" in flags:
        man = os.path.splitext(out)[0] + ".json"
        with open(man, "w", encoding="utf-8") as f:
            json.dump(
                {
                    "fit": {
                        "grip_anchor": list(GRIP_ANCHOR),
                        "grip_angle_deg": GRIP_ANGLE_DEG,
                        "hand_separation": HAND_SEPARATION,
                        "support_offset": list(SUPPORT_OFFSET),
                        "lateral_sign": LATERAL_SIGN,
                        "arm_r_anchor": list(ARM_R_ANCHOR),
                        "arm_l_anchor": list(ARM_L_ANCHOR),
                    },
                    "parts": data,
                    "problems": problems,
                },
                f,
                indent=1,
            )
        print(f"[hands] wrote {man}")

    if "--no-verify" not in flags:
        verify_roundtrip(out, names)

    if problems:
        # Loud, and a non-zero exit so a build script cannot ignore it.
        raise SystemExit(
            f"[hands] FINISHED WITH {len(problems)} PROBLEM(S) — see above. "
            f"The GLB was still written to {out}."
        )
    print("[hands] done")


if __name__ == "__main__":
    main()
