"""Blender headless: the 9mm pistol FBX -> engine GLB, split per part,
with the albedo wired by hand (the FBX links only the normal map) and a
sidecar JSON carrying the per-part pivots the GLB importer throws away.

Run:
    blender --background --python tools/9mm_convert.py -- assets/models/9mm.glb

The sidecar lands next to the GLB as <name>-rig.json (pass a second path
after the GLB to override). Paths to the source FBX and the textures are
derived from this file's location, so the script moves with the repo.

FIRST RUN IS A MEASUREMENT RUN. Nothing here has ever seen the real
mesh: step 3 prints every object's world bbox, the combined bbox, the
dimensions and the UV layer names, plus a derived scale factor and axis
correction. Read those numbers, paste them into the constants below, and
flip AUTOFIT_FROM_MEASUREMENT to False so the conversion is
deterministic instead of heuristic.

Why it is built the way it is:
  * The engine loads GLB only, samples exactly ONE base-color texture per
    mesh, and has no normal/roughness/metallic/AO/emissive input and no
    tangent channel. Six of the seven maps can never be consumed, so only
    GunGS_Albedo is exported and the rest are actively purged - shipping
    them would just bloat a GLB that gets include_bytes!'d into the wasm.
  * assets.rs decodes ONLY 8-bit R8G8B8A8 / R8G8B8 / R8 and silently
    returns None otherwise: a 16-bit image untextures the gun with no
    error logged anywhere. verify_glb() at the bottom parses the exported
    GLB's PNG IHDR and shouts if the bit depth or colour type drifts.
  * There are no mipmaps and the texture is cloned per primitive (one GPU
    texture per mesh id), so 7 parts x 2048^2 x 4B would be ~112 MB of
    VRAM for one pistol. TEX_SIZE=512 is mandatory, not a nicety.
  * The parts are deliberately NOT joined: slide recoil, hammer fall and
    trigger pull are per-part rigid animation, and joining would weld them
    into one immovable lump. The cost is one 512^2 texture upload per part
    (~7.3 MB VRAM total) instead of one - paid knowingly.
"""

import json
import math
import os
import struct
import sys

import bpy
from mathutils import Euler, Matrix, Vector

# ---------------------------------------------------------------------------
# Paths (relative to this file: tools/ -> repo root). Never hardcode a user
# profile here; the copies in swat_convert.py are from a dead machine.
# ---------------------------------------------------------------------------
TOOLS_DIR = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(TOOLS_DIR)
SRC = os.path.join(
    REPO, "assets", "9mm", "source", "0ae7c8526de44d0ab63e6b5d21341fd2.fbx.fbx"
)
TEX_DIR = os.path.join(REPO, "assets", "9mm", "textures")
# Double extension is genuine: these are 8-bit PNGs named *.tga.png.
ALBEDO = "GunGS_Albedo.tga.png"

# ---------------------------------------------------------------------------
# TODO(first run): every constant in this block is a GUESS. Run the script
# once, read the "[9mm] MEASURED" block, paste the printed values in here,
# then set AUTOFIT_FROM_MEASUREMENT = False. While it stays True the script
# overrides SCALE / AXIS_FIX_EULER_DEG with what it derives from the mesh and
# prints both, so a first run still produces a roughly correct GLB.
# ---------------------------------------------------------------------------
AUTOFIT_FROM_MEASUREMENT = True

# Engine units per source unit. Guess assumes the usual Sketchfab
# centimetre-ish FBX (a 19 cm pistol arriving as ~19 units): 0.9 / 19.
SCALE = 0.047
# Blender-space rotation applied before scaling, to land on the engine
# convention (+X = muzzle, +Z = up in Blender; export_yup makes that
# +X forward / +Y up in the engine). Guess assumes the FBX importer already
# righted Z-up and left the barrel along -Y.
AXIS_FIX_EULER_DEG = (0.0, 0.0, 90.0)
# Where the origin (= the hand anchor) sits inside the fitted bbox, as
# fractions of it: along the muzzle axis from the rear, across the width
# from the left, and up from the bottom. The old box pistol put the anchor
# just behind the trigger and level with the top of the grip.
GRIP_ANCHOR_FRAC = (0.10, 0.50, 0.60)
# Extra nudge applied after the fractional anchor, in the POST-FIX Blender
# frame (+X muzzle, +Y width, +Z up) and in engine-sized units. This is the
# knob to touch when the gun sits wrong in the hand at runtime; it does not
# need a re-measure.
ORIGIN_OFFSET_EXTRA = (0.0, 0.0, 0.0)

# Overall pistol length along the muzzle axis, in engine units. The old box
# pistol in make_assets.py is ~0.9 long and the viewmodel offsets are tuned
# to it, so matching it keeps the hand placement usable.
TARGET_LENGTH = 0.9
# Downscale target for the albedo. See the module docstring: this is a VRAM
# budget, not a quality preference.
TEX_SIZE = 512

# FBX node name -> engine node name. The runtime contract in
# crates/pong/src/online.rs: parts named arm*/hand* are viewmodel-only,
# everything else is also drawn on remote players, and a part named exactly
# "strip" is recoloured per weapon level. None of these names collide with
# arm*/hand*, which is asserted below - a stray "hand..." name would
# silently make the gun invisible to everyone but its owner.
RENAME = {
    "Frame_low": "frame",
    "Slide_low": "slide",
    "Trigger_low": "trigger",
    "Hammer_low": "hammer",
    "Clip_low": "mag",
    "Ejector_low": "ejector",
    "Slide Stop_low": "slide_stop",  # embedded space is in the source
}
# Objects the studio scene ships that are not the gun.
DELETE_NAMES = {"Plane", "Plane001", "Sky001"}
DELETE_MATERIALS = {"plane"}  # lowercased material-name match
GUN_MATERIAL = "gungs"  # lowercased match; the FBX spells it "GunGs"

# TODO(first run): rotation pivots, as a fraction of each part's OWN bbox
# (along muzzle / across width / up). Defaults to the part centre; the
# entries below are mechanical guesses about where the pins are and want
# checking against the step-3 per-part boxes.
#   hammer  - pivots on a pin at its base, low and forward of the spur
#   trigger - pivots on a pin at its top front
#   slide / slide_stop / mag / ejector translate rather than rotate, so
#   their pivot only matters as an anchor point.
PART_PIVOT_FRAC = {
    "frame": (0.5, 0.5, 0.5),
    "slide": (0.0, 0.5, 0.5),  # rear face: recoil travels along -X
    "trigger": (0.85, 0.5, 0.9),
    "hammer": (0.5, 0.5, 0.1),
    "mag": (0.5, 0.5, 1.0),  # top of the magazine, where it meets the well
    "ejector": (0.5, 0.5, 0.5),
    "slide_stop": (0.5, 0.5, 0.5),
}
# Which part's front face defines the muzzle tip. The client currently
# guesses the muzzle as origin + forward*0.95, tuned to the old box pistol.
MUZZLE_PART = "slide"


def say(*parts):
    print("[9mm]", *parts)


def die(msg, *detail):
    """Fail loudly. A silently wrong asset costs far more than a crash."""
    say("FATAL:", msg)
    for line in detail:
        say("      ", line)
    raise SystemExit(1)


def fmt(v, n=4):
    return "(" + ", ".join(f"{float(c):.{n}f}" for c in v) + ")"


# ---------------------------------------------------------------------------
# 1. factory settings + import
# ---------------------------------------------------------------------------
def import_fbx():
    if not os.path.isfile(SRC):
        die(
            f"source FBX not found: {SRC}",
            "expected assets/9mm/source/0ae7c8526de44d0ab63e6b5d21341fd2.fbx.fbx",
        )
    bpy.ops.wm.read_factory_settings(use_empty=True)
    say(f"blender {bpy.app.version_string}")
    say(f"importing {os.path.relpath(SRC, REPO)} ({os.path.getsize(SRC)} bytes)")
    # use_anim=False: the source has zero AnimationCurves anyway, and an
    # empty action on the export side is one more thing to explain later.
    try:
        bpy.ops.import_scene.fbx(filepath=SRC, use_anim=False)
    except TypeError:
        bpy.ops.import_scene.fbx(filepath=SRC)
    objs = list(bpy.data.objects)
    say(f"imported {len(objs)} objects:")
    for o in objs:
        # Material slots can hold None; a bare m.name would crash the report
        # that exists to tell the operator what actually imported.
        slots = getattr(o.data, "materials", None) or []
        mats = [m.name if m is not None else "<empty slot>" for m in slots]
        say(f"   {o.type:9s} {o.name!r} materials={mats}")
    if not objs:
        die("the FBX imported zero objects", "importer silently produced nothing")


# ---------------------------------------------------------------------------
# 2. delete the studio junk, then assert the gun is exactly what we expect
# ---------------------------------------------------------------------------
def strip_studio():
    # Snapshot world transforms FIRST. Removing a parent (the FBX root empty
    # usually carries the unit conversion) leaves its children holding only
    # their LOCAL matrix, which silently rescales the gun.
    keep_world = {
        o.name: o.matrix_world.copy() for o in bpy.data.objects if o.type == "MESH"
    }
    doomed = []
    for o in list(bpy.data.objects):
        why = None
        if o.name in DELETE_NAMES:
            why = "name"
        elif o.type == "MESH" and o.data is not None:
            for m in o.data.materials:
                if m is not None and m.name.lower().split(".")[0] in DELETE_MATERIALS:
                    why = f"material {m.name!r}"
                    break
        if why is None and o.type != "MESH":
            # Empties/lights/cameras: the FBX root and studio lighting. They
            # export as nodes the engine would treat as (empty) gun parts.
            why = f"non-mesh ({o.type})"
        if why:
            doomed.append((o, why))
    for o, why in doomed:
        say(f"delete {o.name!r} ({why})")
        bpy.data.objects.remove(o, do_unlink=True)

    meshes = [o for o in bpy.data.objects if o.type == "MESH"]
    for o in meshes:
        world = keep_world.get(o.name)
        if world is None:
            die(f"{o.name!r} appeared during deletion - unexpected")
        o.parent = None
        o.matrix_world = world
    bpy.context.view_layer.update()

    found = sorted(o.name for o in meshes)
    want = sorted(RENAME)
    if found != want:
        die(
            "the surviving object set is not the 7 expected gun parts",
            f"expected ({len(want)}): {want}",
            f"found    ({len(found)}): {found}",
            f"missing:  {sorted(set(want) - set(found))}",
            f"extra:    {sorted(set(found) - set(want))}",
            "if the FBX legitimately changed, update RENAME; do not relax this check",
        )
    say(f"7/7 expected gun objects survived: {found}")
    return meshes


# ---------------------------------------------------------------------------
# 3. MEASURE. This is the primary output of the first run.
# ---------------------------------------------------------------------------
def world_bbox(obj):
    """World-space (min, max) from the actual vertices, not obj.bound_box
    (which is local and stale until a depsgraph update)."""
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    if not pts:
        die(f"{obj.name!r} has no vertices")
    lo = Vector((min(p.x for p in pts), min(p.y for p in pts), min(p.z for p in pts)))
    hi = Vector((max(p.x for p in pts), max(p.y for p in pts), max(p.z for p in pts)))
    return lo, hi


def measure(meshes, label):
    say("")
    say(f"===== MEASURED ({label}) =====")
    boxes = {}
    for o in sorted(meshes, key=lambda x: x.name):
        lo, hi = world_bbox(o)
        boxes[o.name] = (lo, hi)
        dim = hi - lo
        uvs = [layer.name for layer in o.data.uv_layers]
        say(
            f"  {o.name:12s} verts={len(o.data.vertices):6d} faces={len(o.data.polygons):6d}"
        )
        say(f"      min={fmt(lo)} max={fmt(hi)} dim={fmt(dim)}")
        say(f"      uv_layers={uvs}")
        if not uvs:
            die(
                f"{o.name!r} has NO UV layer",
                "an unmapped part samples a single texel and looks flat-coloured",
            )
    lo = Vector([min(b[0][i] for b in boxes.values()) for i in range(3)])
    hi = Vector([max(b[1][i] for b in boxes.values()) for i in range(3)])
    dim = hi - lo
    say(f"  COMBINED min={fmt(lo)} max={fmt(hi)}")
    say(f"  COMBINED dim={fmt(dim)}  (longest axis = {'XYZ'[max(range(3), key=lambda i: dim[i])]})")
    say("===== end MEASURED =====")
    say("")
    return boxes, (lo, hi)


def suggest_fit(meshes, combined):
    """Derive a scale factor and an axis correction from the geometry.

    A pistol is longer than it is tall and taller than it is wide, so the
    axis extents rank length > height > width. The muzzle end is the half
    with the SMALLER vertical extent (the other half carries the grip), and
    the grip hangs DOWN, which fixes the sign of up. Everything here is a
    heuristic and is printed as a suggestion, never applied silently.
    """
    lo, hi = combined
    dim = hi - lo
    order = sorted(range(3), key=lambda i: dim[i], reverse=True)
    long_ax, up_ax, wide_ax = order[0], order[1], order[2]
    length = dim[long_ax]
    if length <= 0.0:
        die("combined bounding box is degenerate", f"dim={fmt(dim)}")

    pts = [o.matrix_world @ v.co for o in meshes for v in o.data.vertices]
    mid = (lo[long_ax] + hi[long_ax]) * 0.5
    halves = {True: [], False: []}
    for p in pts:
        halves[p[long_ax] >= mid].append(p)
    if not halves[True] or not halves[False]:
        die("could not split the mesh along its long axis")

    def vext(sample):
        return max(p[up_ax] for p in sample) - min(p[up_ax] for p in sample)

    # The grip end is vertically taller: the muzzle points at the other one.
    muzzle_positive = vext(halves[True]) < vext(halves[False])
    muzzle = Vector((0.0, 0.0, 0.0))
    muzzle[long_ax] = 1.0 if muzzle_positive else -1.0

    # Up: from the barrel half's centre, the grip half reaches much further
    # one way than the other. That way is DOWN.
    front = halves[muzzle_positive]
    rear = halves[not muzzle_positive]
    front_c = sum(p[up_ax] for p in front) / len(front)
    reach_pos = max(p[up_ax] for p in rear) - front_c
    reach_neg = front_c - min(p[up_ax] for p in rear)
    up = Vector((0.0, 0.0, 0.0))
    up[up_ax] = -1.0 if reach_pos > reach_neg else 1.0

    # Rows map (muzzle, width, up) onto (+X, +Y, +Z). Taking width as
    # up x muzzle keeps the basis right-handed, so nothing is mirrored.
    wide = up.cross(muzzle)
    rot = Matrix((muzzle, wide, up)).to_4x4()
    euler = [math.degrees(a) for a in rot.to_euler("XYZ")]
    scale = TARGET_LENGTH / length

    say("===== SUGGESTED FIT (heuristic - verify, then hardcode) =====")
    say(f"  extents: length={length:.4f} on {'XYZ'[long_ax]}, "
        f"height={dim[up_ax]:.4f} on {'XYZ'[up_ax]}, "
        f"width={dim[wide_ax]:.4f} on {'XYZ'[wide_ax]}")
    say(f"  muzzle direction (blender) = {fmt(muzzle, 1)}")
    say(f"  up direction     (blender) = {fmt(up, 1)}")
    say(f"  SCALE               = {scale:.6f}   (currently {SCALE:.6f})")
    say(f"  AXIS_FIX_EULER_DEG  = ({euler[0]:.1f}, {euler[1]:.1f}, {euler[2]:.1f})"
        f"   (currently {AXIS_FIX_EULER_DEG})")
    if abs(scale - SCALE) > 0.2 * max(scale, 1e-9):
        say("  !! SCALE constant disagrees with the measurement by >20% - one of")
        say("     them is wrong. The measurement is the one that saw the mesh.")
    say("===== end SUGGESTED FIT =====")
    say("")
    return scale, tuple(euler)


# ---------------------------------------------------------------------------
# 4. wire the albedo (the FBX links only the normal map)
# ---------------------------------------------------------------------------
def wire_albedo():
    mats = [m for m in bpy.data.materials if GUN_MATERIAL in m.name.lower()]
    if not mats:
        die(
            f"no material matching {GUN_MATERIAL!r}",
            f"materials present: {[m.name for m in bpy.data.materials]}",
        )
    if len(mats) > 1:
        say(f"WARNING: {len(mats)} materials match {GUN_MATERIAL!r}: "
            f"{[m.name for m in mats]} - wiring all of them")

    path = os.path.join(TEX_DIR, ALBEDO)
    if not os.path.isfile(path):
        listing = sorted(os.listdir(TEX_DIR)) if os.path.isdir(TEX_DIR) else "<no dir>"
        die(f"albedo not found: {path}", f"textures dir holds: {listing}")

    img = bpy.data.images.load(path, check_existing=True)
    # A load that fails to DECODE still yields an image datablock, sized
    # 0x0. That is the quiet path to an untextured gun, so check it.
    if tuple(img.size) == (0, 0) or img.size[0] == 0 or img.size[1] == 0:
        die(
            f"{ALBEDO} loaded as a 0x0 image - it did not decode",
            f"file is {os.path.getsize(path)} bytes at {path}",
        )
    say(f"loaded {ALBEDO}: {img.size[0]}x{img.size[1]} depth={img.depth} "
        f"channels={img.channels} float={img.is_float}")
    if img.depth > 32:
        say(f"WARNING: source is {img.depth}-bit. assets.rs decodes ONLY 8-bit "
            "R8G8B8A8/R8G8B8/R8 and returns None otherwise - the gun would be")
        say("         untextured with NO error logged. verify_glb() re-checks "
            "the exported PNG; do not ignore it.")

    src_w, src_h = img.size
    if src_w != src_h:
        say(f"WARNING: albedo is {src_w}x{src_h}, not square. Scaling to a "
            "square target would stretch every texel and land every UV")
        say("         lookup on the wrong pixel, with nothing logged - the "
            "aspect ratio is preserved below instead.")
    if max(img.size) > TEX_SIZE:
        # Preserve aspect: a forced square target silently smears a
        # non-square atlas, and the resolution check in verify_glb() passes
        # happily on the result because it only looks at the larger side.
        s = TEX_SIZE / max(src_w, src_h)
        img.scale(max(1, round(src_w * s)), max(1, round(src_h * s)))
        say(f"downscaled to {img.size[0]}x{img.size[1]} "
            f"(~{7 * img.size[0] * img.size[1] * 4 / 1e6:.1f} MB VRAM across 7 parts)")
    img.colorspace_settings.name = "sRGB"
    img.file_format = "PNG"
    # Pack it: the glTF exporter will happily copy the ORIGINAL file bytes
    # for an on-disk image it thinks is unmodified, which would silently
    # ship the full-resolution 2048 texture and undo the downscale.
    try:
        img.pack()
    except RuntimeError as exc:
        say(f"WARNING: could not pack the scaled image ({exc}) - if the GLB check "
            "below reports 2048x2048, this is why.")

    for mat in mats:
        mat.use_nodes = True
        nt = mat.node_tree
        bsdf = nt.nodes.get("Principled BSDF") or next(
            (n for n in nt.nodes if n.type == "BSDF_PRINCIPLED"), None
        )
        if bsdf is None:
            die(
                f"material {mat.name!r} has no Principled BSDF",
                f"nodes: {[n.type for n in nt.nodes]}",
            )
        # Drop every existing image node first. The FBX wires the normal map,
        # and the engine has no normal input and no tangent channel - keeping
        # it would only embed a second 2048 texture into a GLB that gets
        # include_bytes!'d into the wasm binary.
        for node in [n for n in nt.nodes if n.type == "TEX_IMAGE"]:
            say(f"  dropping unused texture node {node.image.name if node.image else '<none>'} "
                f"from {mat.name!r}")
            nt.nodes.remove(node)
        node = nt.nodes.new("ShaderNodeTexImage")
        node.image = img
        node.location = (bsdf.location.x - 400, bsdf.location.y)
        nt.links.new(bsdf.inputs["Base Color"], node.outputs["Color"])
        # White multiplier: the shader multiplies the sampled texel by the
        # per-instance colour, so a tinted base colour would double-tint.
        bsdf.inputs["Base Color"].default_value = (1.0, 1.0, 1.0, 1.0)
        say(f"  wired {ALBEDO} -> {mat.name!r} Base Color")

    # Purge the six maps the engine cannot consume, so nothing can drag them
    # back into the export.
    for other in [i for i in bpy.data.images if i is not img and i.users == 0]:
        say(f"  purging unused image {other.name!r}")
        bpy.data.images.remove(other)
    return img


# ---------------------------------------------------------------------------
# 5. one UV layer, one name
# ---------------------------------------------------------------------------
def collapse_uvs(meshes):
    """glTF exports only TEXCOORD_0, and any later join merges UV layers BY
    NAME. Mismatched names leave the first layer empty and the whole model
    samples a single texel - the hard-won rule from swat_split.py."""
    for o in meshes:
        me = o.data
        if not me.uv_layers:
            die(f"{o.name!r} has no UV layer")
        old = (me.uv_layers.active or me.uv_layers[0]).name
        dropped = [layer.name for layer in me.uv_layers if layer.name != old]
        # Remove BY NAME and re-fetch afterwards: removing a UV layer can
        # reallocate the collection, so a Python reference held across a
        # remove() is not safe to rename through.
        for name in dropped:
            me.uv_layers.remove(me.uv_layers[name])
        if len(me.uv_layers) != 1:
            die(f"{o.name!r} still has {len(me.uv_layers)} UV layers after collapse")
        me.uv_layers[0].name = "UVMap"
        me.uv_layers.active_index = 0
        say(f"  {o.name:12s} uv {old!r} -> 'UVMap'" + (f" (dropped {dropped})" if dropped else ""))


# ---------------------------------------------------------------------------
# 6. rename to the runtime contract
# ---------------------------------------------------------------------------
def rename(meshes):
    for o in meshes:
        new = RENAME[o.name]
        o.name = new
        o.data.name = new
    names = sorted(o.name for o in meshes)
    say(f"renamed to {names}")
    for n in names:
        if n.startswith("arm") or n.startswith("hand"):
            die(
                f"part {n!r} starts with arm/hand",
                "online.rs would classify it as a viewmodel-only limb, so it "
                "would vanish from every remote player's view of the gun",
            )
    if "strip" not in names:
        say("note: no part is named 'strip', so the per-weapon-level accent "
            "recolour will not apply to this gun.")


# ---------------------------------------------------------------------------
# 7. scale + axis + re-origin
# ---------------------------------------------------------------------------
def transform(meshes, scale, euler_deg):
    # Build the matrix through Euler(...,"XYZ") rather than composing three
    # Matrix.Rotation calls: suggest_fit prints the angles via to_euler("XYZ"),
    # and only the matching constructor round-trips them (Blender's XYZ order
    # composes as Rz @ Ry @ Rx, which is not the obvious hand-written order).
    rot = Euler([math.radians(a) for a in euler_deg], "XYZ").to_matrix().to_4x4()
    pre = Matrix.Scale(scale, 4) @ rot
    for o in meshes:
        o.matrix_world = pre @ o.matrix_world
    bpy.context.view_layer.update()
    say(f"applied scale={scale:.6f} euler_deg=({euler_deg[0]:.1f}, "
        f"{euler_deg[1]:.1f}, {euler_deg[2]:.1f})")

    _, (lo, hi) = measure(meshes, "after scale + axis fix, before re-origin")
    dim = hi - lo
    # Only meaningful for a HARDCODED fit. Under AUTOFIT both are
    # tautologies -- suggest_fit rotates the longest axis onto +X and scales
    # it to TARGET_LENGTH, so dim.x IS TARGET_LENGTH and IS the largest
    # extent, by construction. Gated so they do not read as live coverage
    # they cannot provide; the muzzle-direction check in write_sidecar() is
    # what actually catches a bad fit under AUTOFIT.
    if not AUTOFIT_FROM_MEASUREMENT:
        if abs(dim.x - TARGET_LENGTH) > 0.25 * TARGET_LENGTH:
            say(f"WARNING: the pistol is {dim.x:.3f} along +X but should be "
                f"about {TARGET_LENGTH} - the axis fix probably puts the "
                "barrel on the wrong axis.")
        if dim.x < dim.y or dim.x < dim.z:
            say("WARNING: +X is NOT the longest axis after the fix. The "
                "muzzle is not pointing along +X; the viewmodel will be "
                "sideways.")

    origin = Vector(
        (
            lo.x + GRIP_ANCHOR_FRAC[0] * dim.x,
            lo.y + GRIP_ANCHOR_FRAC[1] * dim.y,
            lo.z + GRIP_ANCHOR_FRAC[2] * dim.z,
        )
    ) - Vector(ORIGIN_OFFSET_EXTRA)
    for o in meshes:
        o.matrix_world = Matrix.Translation(-origin) @ o.matrix_world
    bpy.context.view_layer.update()
    say(f"re-origined: hand anchor was at blender {fmt(origin)}")

    # Bake it all into the vertices: the engine's GLB importer bakes world
    # transforms and DISCARDS the hierarchy anyway, so leaving transforms on
    # the nodes would only make the GLB disagree with the sidecar.
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.context.view_layer.update()
    return measure(meshes, "final, engine-bound (still Blender Z-up)")


# ---------------------------------------------------------------------------
# 8/9. export + sidecar
# ---------------------------------------------------------------------------
def to_engine(v):
    """Blender (x, y, z) -> engine (x, z, -y): the mapping export_yup=True
    performs on the geometry, documented in swat_split.py. Pivots MUST use
    the same one or they land in a different space than the meshes."""
    return [round(float(v[0]), 5), round(float(v[2]), 5), round(-float(v[1]), 5)]


def to_engine_box(lo, hi):
    """Same mapping for a box. The negated axis SWAPS min and max there, so
    a componentwise to_engine() of each corner would emit a box whose min is
    greater than its max on z (the bug level_convert.py spells out)."""
    return (
        [round(float(lo.x), 5), round(float(lo.z), 5), round(-float(hi.y), 5)],
        [round(float(hi.x), 5), round(float(hi.z), 5), round(-float(lo.y), 5)],
    )


def export_glb(meshes, out_glb):
    os.makedirs(os.path.dirname(os.path.abspath(out_glb)) or ".", exist_ok=True)
    bpy.ops.object.select_all(action="DESELECT")
    for o in meshes:
        o.select_set(True)
    bpy.context.view_layer.objects.active = meshes[0]
    kwargs = dict(
        filepath=out_glb,
        export_format="GLB",
        export_yup=True,
        use_selection=True,
        # 'AUTO' keeps a PNG image a PNG (there is no "PNG" enum member -
        # the choices are AUTO/JPEG/WEBP/NONE depending on Blender version).
        # img.file_format is forced to PNG in wire_albedo, so this lands on
        # the engine's known-good 8-bit PNG decode path. verify_glb checks.
        export_image_format="AUTO",
        # No tangent channel exists in the engine vertex format.
        export_tangents=False,
    )
    try:
        bpy.ops.export_scene.gltf(**kwargs)
    except TypeError as exc:
        say(f"WARNING: exporter rejected the full kwargs ({exc}); retrying with "
            "the minimal set. Check the exported image format by hand.")
        bpy.ops.export_scene.gltf(
            filepath=out_glb,
            export_format="GLB",
            export_yup=True,
            use_selection=True,
        )
    if not os.path.isfile(out_glb):
        die(f"export reported success but {out_glb} does not exist")
    say(f"wrote {out_glb} ({os.path.getsize(out_glb)} bytes)")


def write_sidecar(meshes, boxes, combined, out_json, scale, euler_deg):
    """Sidecar shape mirrors swat_split.py's swat-rig.json (a dict of named
    points plus a parts list), because rig.rs-adjacent code already reads
    that shape and a familiar file is a file people actually check."""
    order = ["frame", "slide", "trigger", "hammer", "mag", "ejector", "slide_stop"]
    by_name = {o.name: o for o in meshes}
    missing = [n for n in order if n not in by_name]
    if missing:
        die(f"parts missing at sidecar time: {missing}")

    pivots, offsets, part_boxes = {}, {}, {}
    for name in order:
        lo, hi = boxes[by_name[name].name]
        dim = hi - lo
        frac = PART_PIVOT_FRAC.get(name, (0.5, 0.5, 0.5))
        pivot = Vector(
            (lo.x + frac[0] * dim.x, lo.y + frac[1] * dim.y, lo.z + frac[2] * dim.z)
        )
        centre = (lo + hi) * 0.5
        pivots[name] = to_engine(pivot)
        # The root sits at the origin after re-origining, so the "local
        # offset from the gun root" is just the part's centre in root space -
        # kept distinct from the pivot, which is where it hinges.
        offsets[name] = to_engine(centre)
        bmin, bmax = to_engine_box(lo, hi)
        part_boxes[name] = {"min": bmin, "max": bmax}

    # Muzzle tip: the front face of the slide, on its bore centreline. The
    # client currently guesses origin + forward*0.95 (tuned to the old box
    # pistol); this replaces the guess with the real geometry.
    mlo, mhi = boxes[by_name[MUZZLE_PART].name]
    muzzle = Vector((mhi.x, (mlo.y + mhi.y) * 0.5, (mlo.z + mhi.z) * 0.5))
    clo, chi = combined
    gmin, gmax = to_engine_box(clo, chi)

    # The muzzle must end up FORWARD, and near the front of the gun. With
    # AUTOFIT on, the length and longest-axis guards in transform() are
    # tautologies by construction (suggest_fit rotates the longest axis onto
    # +X and scales it to TARGET_LENGTH), so the only thing separating
    # "barrel forward" from "barrel back through the player" is the
    # vertical-extent heuristic picking an end. A 180-degree flip there
    # leaves length, longest axis and handedness all correct and would
    # otherwise ship silently, exit 0, no warning.
    if muzzle.x <= 0.5 * chi.x:
        die(
            f"the muzzle ended up at blender x={muzzle.x:.3f} with the gun "
            f"spanning {clo.x:.3f}..{chi.x:.3f} - the axis fit put the "
            "barrel BACKWARDS",
            "suggest_fit picked the wrong end; set AUTOFIT_FROM_MEASUREMENT="
            "False and hardcode AXIS_FIX_EULER_DEG with 180 added about up",
        )

    # Key ORDER matters here, and not for taste. The established reader for
    # this file shape, parse_joint in crates/ember-engine/src/rig.rs, is a
    # substring scanner: it finds "<name>" and takes the NEXT [ ... ]. If a
    # bare list of part-name strings came first, the first hit for "slide"
    # would be inside that list and the next [ would be frame's pivot -- so
    # every part would silently rotate about the frame's pivot, with no
    # parse error. The per-part maps therefore come FIRST, and the name list
    # is called "part_order" so it cannot collide with a pivot key at all.
    data = {
        "pivots": pivots,
        "offsets": offsets,
        "boxes": part_boxes,
        "muzzle": to_engine(muzzle),
        "bbox": {"min": gmin, "max": gmax},
        "source": os.path.relpath(SRC, REPO).replace("\\", "/"),
        "scale": round(scale, 6),
        "axis_fix_euler_deg": [round(a, 3) for a in euler_deg],
        "tex_size": TEX_SIZE,
        "part_order": order,
    }
    os.makedirs(os.path.dirname(os.path.abspath(out_json)) or ".", exist_ok=True)
    with open(out_json, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=1)
    say(f"wrote {out_json}")
    say(f"  muzzle tip (engine) = {data['muzzle']}  "
        f"(client currently assumes forward*0.95)")
    for name in order:
        say(f"  {name:12s} pivot={pivots[name]} offset={offsets[name]}")


# ---------------------------------------------------------------------------
# post-export verification: parse the GLB we just wrote, stdlib only
# ---------------------------------------------------------------------------
def verify_glb(path):
    """Re-read the exported GLB and check the two things that fail SILENTLY
    at runtime: a missing/unusable base-colour texture, and a PNG the
    engine's decoder rejects (16-bit, or grey+alpha)."""
    say("")
    say("===== VERIFY (exported GLB) =====")
    with open(path, "rb") as f:
        blob = f.read()
    if len(blob) < 12 or blob[:4] != b"glTF":
        die(f"{path} is not a GLB (magic {blob[:4]!r})")
    ver, total = struct.unpack_from("<II", blob, 4)
    say(f"  container: glTF v{ver}, {total} bytes declared, {len(blob)} on disk")

    js, binchunk, off = None, b"", 12
    while off + 8 <= len(blob):
        clen, ctype = struct.unpack_from("<II", blob, off)
        body = blob[off + 8 : off + 8 + clen]
        if ctype == 0x4E4F534A:
            js = json.loads(body.decode("utf-8"))
        elif ctype == 0x004E4942:
            binchunk = body
        off += 8 + clen
    if js is None:
        die("GLB has no JSON chunk")

    names = [n.get("name") for n in js.get("nodes", [])]
    say(f"  nodes ({len(names)}): {names}")
    want = set(RENAME.values())
    got = set(n for n in names if n)
    if got != want:
        die(
            "exported node names do not match the runtime contract",
            f"expected: {sorted(want)}",
            f"exported: {sorted(got)}",
        )

    untextured = []
    for mesh in js.get("meshes", []):
        for prim in mesh.get("primitives", []):
            if "TEXCOORD_0" not in prim.get("attributes", {}):
                untextured.append(f"{mesh.get('name')}: no TEXCOORD_0")
                continue
            mi = prim.get("material")
            if mi is None:
                untextured.append(f"{mesh.get('name')}: no material")
                continue
            pbr = js["materials"][mi].get("pbrMetallicRoughness", {})
            if "baseColorTexture" not in pbr:
                untextured.append(f"{mesh.get('name')}: material has no baseColorTexture")
    if untextured:
        die(
            "primitives that will render untextured",
            *untextured,
            "the shader samples exactly one base-colour texture; without it "
            "the gun is a flat instance-coloured blob",
        )
    say(f"  all {len(js.get('meshes', []))} meshes have TEXCOORD_0 + a base-colour texture")

    images = js.get("images", [])
    if not images:
        die("GLB embeds no images at all - the albedo did not make it in")
    if len(images) > 1:
        say(f"  WARNING: {len(images)} images embedded; only the base colour is "
            "ever sampled, the rest is dead weight in the wasm binary.")
    views = js.get("bufferViews", [])
    for i, im in enumerate(images):
        mime = im.get("mimeType")
        bv = im.get("bufferView")
        if bv is None:
            say(f"  image[{i}] is an external URI ({im.get('uri')}) - GLB should "
                "embed it; the engine reads GLB bytes only.")
            continue
        v = views[bv]
        start = v.get("byteOffset", 0)
        data = binchunk[start : start + v["byteLength"]]
        say(f"  image[{i}] mime={mime} {len(data)} bytes")
        if mime != "image/png":
            say(f"  WARNING: image[{i}] is {mime}, not PNG - PNG is the "
                "known-good decode path.")
            continue
        if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
            say(f"  WARNING: image[{i}] has no readable PNG IHDR; cannot check "
                "bit depth.")
            continue
        w, h, depth, ctype_png = struct.unpack_from(">IIBB", data, 16)
        kinds = {0: "grey", 2: "RGB", 3: "palette", 4: "grey+alpha", 6: "RGBA"}
        say(f"    {w}x{h} {depth}-bit {kinds.get(ctype_png, ctype_png)}")
        if max(w, h) > TEX_SIZE:
            die(
                f"exported texture is {w}x{h}, not {TEX_SIZE}",
                "the downscale did not survive export (the exporter probably "
                "copied the original file bytes); the pistol would cost "
                f"~{7 * w * h * 4 / 1e6:.0f} MB of VRAM",
            )
        if depth != 8:
            die(
                f"exported texture is {depth}-bit",
                "assets.rs decodes ONLY 8-bit R8G8B8A8/R8G8B8/R8 and silently "
                "returns None otherwise - the gun would be untextured with no "
                "error logged",
            )
        if ctype_png not in (0, 2, 6):
            say(f"  WARNING: colour type {kinds.get(ctype_png, ctype_png)} is not "
                "one of grey/RGB/RGBA; assets.rs may reject the decoded form.")
    say("===== end VERIFY =====")


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    out_glb = argv[0] if argv else os.path.join(REPO, "assets", "models", "9mm.glb")
    out_json = (
        argv[1]
        if len(argv) > 1
        else os.path.splitext(out_glb)[0] + "-rig.json"
    )

    import_fbx()
    meshes = strip_studio()
    _, combined = measure(meshes, "as imported, studio junk removed")
    fit_scale, fit_euler = suggest_fit(meshes, combined)

    wire_albedo()
    collapse_uvs(meshes)
    rename(meshes)

    scale, euler = SCALE, AXIS_FIX_EULER_DEG
    if AUTOFIT_FROM_MEASUREMENT:
        say("AUTOFIT_FROM_MEASUREMENT is on: using the DERIVED scale and axis "
            "fix, not the constants. Paste the suggestion above into SCALE and")
        say("AXIS_FIX_EULER_DEG, then set the flag False to freeze it.")
        scale, euler = fit_scale, fit_euler

    boxes, final = transform(meshes, scale, euler)
    export_glb(meshes, out_glb)
    write_sidecar(meshes, boxes, final, out_json, scale, euler)
    verify_glb(out_glb)
    say("done. Nothing here has been run against a real Blender - treat the "
        "first run's output as the source of truth, not this script's guesses.")


main()
