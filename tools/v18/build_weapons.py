#!/usr/bin/env python3
"""Blender headless: the v18 viewmodel - the v17 five plus the loot guns.

    blender --background --python tools/v18/build_weapons.py -- [--preview] [--split]
    C:\\hy3d\\venv\\Scripts\\python.exe tools/v18/build_weapons.py --verify

Rebuilds `rifle`, `hands`, `shield`, `sword` and `hand_sword` through the
v16 and v17 builders imported as libraries, then adds ten weapon nodes in
the frame every weapon shares: Blender +X forward, +Z up, the origin at the
top of the pistol grip at the trigger (v16's hold point, so the operator's
hands draw at the same offset for every gun), at the plan's target length
(docs/plans/arena-v18-freight-yard.md section 8):

  * `w_vityaz` (0.72 m): the PP-19-01 with its stock, glass and reticle
    deleted, seventeen meshes over seven materials joined into one and its
    seven pictures baked into ONE atlas, because the renderer samples one
    base-colour picture per mesh;
  * `w_ak47` (0.88 m): one mesh appended from the artist's .blend;
  * `w_revolver_frame`, `_receiver`, `_cylinder`, `_hammer`, `_trigger`:
    the v15 revolver through `tools/v15/build_viewmodel.py`'s
    `build_revolver()`, in v15's own fit, with the pivots the client's
    `PartAnim` reads keyed by the full node names;
  * `w_sniper` (1.15 m): rifle + rifle.001 + mag joined, five materials
    baked into one atlas, then decimated from 88 000 faces;
  * `w_rpg7` (0.95 m) and `w_rpg7_rocket`: the launcher with its hammer
    and sights joined, the rocket separate in the launcher's frame so the
    client draws it in place at the unit transform and hides it when the
    tube is empty.

Every frame is MEASURED, not assumed (docs/asset-pipeline.md, Path D): the
long axis is the largest extent, the muzzle is the thinner end, up is the
side the sight sits on or the side away from the magazine, and after the
fit the build asserts the muzzle lies forward of the origin near the front
bound and the magazine below the bore, because a derived fit that is
flipped end for end passes every tautological check. The names are the
contract: crates/arena/src/online.rs classifies on them.

Outputs: crates/arena/assets/viewmodel.glb, viewmodel-rig.json (pivots,
the sidearm's muzzle, per-weapon muzzles), tools/v18/preview-*.png.
Run tools/v16, v17 and v18 prep_pictures.py first.
"""
import importlib.util
import io
import json
import math
import os
import statistics
import struct
import sys
import time

try:
    import bpy
    from mathutils import Matrix, Vector
except ImportError:  # --verify under a plain python (PIL, no bpy)
    bpy = None

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
A = os.path.join(REPO, "assets")
OUT_GLB = os.path.join(REPO, "crates", "arena", "assets", "viewmodel.glb")
OUT_RIG = os.path.join(REPO, "crates", "arena", "assets", "viewmodel-rig.json")

# Target lengths, metres, along +X after the fit (plan 8.1).
TARGET = {"w_vityaz": 0.72, "w_ak47": 0.88, "w_sniper": 1.15, "w_rpg7": 0.95, "w_revolver": 0.75}
# The two baked atlases ship at 768, not 1024: at 1024 the GLB measured
# 18.2 MB (decimal megabytes, the plan's unit) against the plan's 16 MB
# line, and the plan's pre-decided first fallback is these two pictures,
# never the revolver's (plan 8.3). At 768 it is 17.4 MB; the next lever is
# a project decision, not this script's.
BAKE_SIZE = 768
# The picture each node ships, and its size: what verify_glb holds the file to.
INTENDED_SIZE = {
    "rifle": 1024, "hands": 2048, "shield": 1024, "sword": 1024, "hand_sword": 1024,
    "w_vityaz": BAKE_SIZE, "w_ak47": 1024, "w_sniper": BAKE_SIZE, "w_rpg7": 1024, "w_rpg7_rocket": 512,
    "w_revolver_frame": 1024, "w_revolver_receiver": 512, "w_revolver_cylinder": 512,
    "w_revolver_hammer": 512, "w_revolver_trigger": 512,
}
WANT_NODES = sorted(INTENDED_SIZE)
# Triangle budgets after the atlas bake. The bake sees the artist's
# density; what ships is what a web player downloads.
FACE_BUDGET = {"w_vityaz": 15000, "w_sniper": 15000, "w_ak47": 15000, "w_rpg7": 15000}
BAKE_SAMPLES = 16
# A bake that failed is black or one flat colour, silently: the std-dev of
# a 1 % pixel sample must clear this (8/255).
BAKE_MIN_STDDEV = 8.0 / 255.0

# What each archive must contain, by object name, after the studio is
# deleted (plan 8.1). A different set means a different file, and the
# build stops rather than converting whatever survived.
VITYAZ_FBX = os.path.join(A, "vityaz", "source", "source", "pp19 01 vityaz.fbx")
VITYAZ_OBJECTS = {
    "bolt carrier", "bottom hand guard", "Dust Cover", "Folding Stock", "holo sight", "holo sight glass",
    "holo sight glass 2", "Magazine", "pistol grip", "rear sight", "receiver", "reticle", "safety",
    "Side Mounted Rail", "side rail mount", "top rail", "Trigger",
}
VITYAZ_DROP = {"Folding Stock", "holo sight glass", "holo sight glass 2", "reticle"}
AK_BLEND = os.path.join(A, "ak47", "source", "source", "AK47.blend")
AK_PICTURE = os.path.join(A, "ak47", "baked", "ak47-1024.png")
RPG_FBX = os.path.join(A, "rpg7", "source", "RPG7", "RPG7.fbx")
RPG_OBJECTS = {"RPG7", "rocket", "hammer", "sight", "sight_adjust"}
RPG_PICTURE = os.path.join(A, "rpg7", "baked", "rpg7-1024.png")
ROCKET_PICTURE = os.path.join(A, "rpg7", "baked", "rocket-512.png")
SNIPER_FBX = os.path.join(A, "sniper", "source", "source", "1.fbx")
SNIPER_OBJECTS = {"rifle", "rifle.001", "mag", "bullet"}
SNIPER_DROP = {"bullet"}
REVOLVER_BAKED = os.path.join(A, "revolver", "baked")

T_START = time.time()


def say(msg):
    print(f"[v18] {msg}", flush=True)


def die(msg, hint=None):
    print(f"[v18] ERROR: {msg}", flush=True)
    if hint:
        print(f"[v18]   {hint}", flush=True)
    sys.exit(1)


class Step:
    """Wall time per step, printed: an action that did not say how long it
    took is a process bug (CLAUDE.md)."""

    def __init__(self, label):
        self.label = label

    def __enter__(self):
        self.t0 = time.time()
        return self

    def __exit__(self, *exc):
        say(f"{self.label}: {time.time() - self.t0:.1f} s")
        return False


# ---- the libraries -----------------------------------------------------------
if bpy is not None:
    sys.path.insert(0, os.path.join(REPO, "tools", "v17"))
    import build_viewmodel as v17  # noqa: E402  (v17 puts tools/v16 on the path and imports it)

    v16 = v17.v16
    # tools/v15 is also called build_viewmodel.py, so it is loaded under
    # its own module name rather than through sys.path.
    _spec = importlib.util.spec_from_file_location("v15_build_viewmodel", os.path.join(REPO, "tools", "v15", "build_viewmodel.py"))
    v15 = importlib.util.module_from_spec(_spec)
    _spec.loader.exec_module(v15)
    fmt, world_bbox, apply_all = v16.fmt, v16.world_bbox, v16.apply_all
    picture_material, set_material, smooth, unparent_keep_world, to_engine = v16.picture_material, v16.set_material, v16.smooth, v16.unparent_keep_world, v16.to_engine


# ---- import, and the studio ------------------------------------------------------
def delete_studio(new, expect, path):
    """Keep the meshes, remove everything else the file brought (lights,
    cameras, empties), and assert the mesh set by name: converting whatever
    happened to survive is how a backdrop ends up welded to a gun."""
    meshes = [o for o in new if o.type == "MESH"]
    others = [(o.type, o.name) for o in new if o.type != "MESH"]
    for o in new:
        if o.type != "MESH":
            bpy.data.objects.remove(o, do_unlink=True)
    found = {o.name for o in meshes}
    if found != set(expect):
        die(f"{os.path.basename(path)}: expected objects {sorted(expect)}, found {sorted(found)}", f"non-mesh objects removed: {others}")
    say(f"{os.path.basename(path)}: {len(meshes)} meshes as expected; studio removed: {others or 'nothing'}")
    return meshes


def import_fbx(path, expect):
    if not os.path.isfile(path):
        die(f"missing {path}", "unpack the weapon archives under assets/")
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=path, use_anim=False)
    return delete_studio([o for o in bpy.data.objects if o not in before], expect, path)


def append_blend_objects(path, expect):
    if not os.path.isfile(path):
        die(f"missing {path}", "unpack the weapon archives under assets/")
    with bpy.data.libraries.load(path, link=False) as (src, dst):
        dst.objects = src.objects
    new = [o for o in dst.objects if o is not None]
    for o in new:
        bpy.context.scene.collection.objects.link(o)
    return delete_studio(new, expect, path)


def uv_area(me, layer):
    """Total UV-space area of the faces: zero for a layer that is all one point."""
    data = layer.data
    area = 0.0
    for p in me.polygons:
        li = p.loop_indices
        n = len(li)
        a = 0.0
        for i in range(n):
            u0, v0 = data[li[i]].uv
            u1, v1 = data[li[(i + 1) % n]].uv
            a += u0 * v1 - u1 * v0
        area += abs(a) * 0.5
    return area


def keep_one_uv(obj):
    """One UV layer named UVMap, before any join. A join matches layers by
    NAME, and an object whose live layer is called something else lands
    its UVs in an empty layer and renders flat. The live layer is the one
    the file marks for render when that layer is not degenerate; otherwise
    the one with the most UV area (the Vityaz carries all-zero layers)."""
    me = obj.data
    if not me.uv_layers:
        die(f"{obj.name} has no UVs")
    if len(me.uv_layers) == 1:
        me.uv_layers[0].name = "UVMap"
        return
    areas = {layer.name: uv_area(me, layer) for layer in me.uv_layers}
    render = next((layer for layer in me.uv_layers if layer.active_render), me.uv_layers[0])
    keep = render if areas[render.name] > 1e-4 else max(me.uv_layers, key=lambda layer: areas[layer.name])
    say(f"  {obj.name}: uv layers {', '.join(f'{n} (area {a:.3f})' for n, a in areas.items())} -> keeping {keep.name!r}")
    for layer in [layer for layer in me.uv_layers if layer != keep]:
        me.uv_layers.remove(layer)
    me.uv_layers[0].name = "UVMap"


def prepare(objs):
    """Unparent (world kept), one UV layer each, transforms applied, and
    no modifiers: the exporter applies them (export_apply), so a
    subdivision left on the AK's .blend object turned 15 000 decimated
    triangles into 360 000 in the file, after every check had passed."""
    for o in objs:
        unparent_keep_world(o)
        keep_one_uv(o)
        if o.modifiers:
            say(f"  {o.name}: dropping modifiers {[(m.type, m.name) for m in o.modifiers]}")
            o.modifiers.clear()
    apply_all(objs)


def join_into(objs, name):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    if len(objs) > 1:
        bpy.ops.object.join()
    obj = bpy.context.view_layer.objects.active
    obj.name = name
    obj.data.name = name
    if len(obj.data.uv_layers) != 1 or obj.data.uv_layers[0].name != "UVMap":
        die(f"{name}: after the join the UV layers are {[l.name for l in obj.data.uv_layers]}")
    return obj


# ---- the frame -----------------------------------------------------------------
def measure_frame(objs, up_hint, label):
    """The rotation that puts the long axis on +X with the muzzle forward
    and `up_hint` on +Z. The muzzle end is the thinner end: the 15 % slab
    at each end of the long axis, compared by the area of its cross-section."""
    pts = [o.matrix_world @ v.co for o in objs for v in o.data.vertices]
    lo, hi = world_bbox(objs)
    dim = hi - lo
    long_ax = max(range(3), key=lambda i: dim[i])
    up = Vector(up_hint)
    if abs(up[long_ax]) > 1e-6 or abs(up.length - 1.0) > 1e-6:
        die(f"{label}: up hint {tuple(up)} must be a unit axis perpendicular to the long axis {'XYZ'[long_ax]}")
    other = [i for i in range(3) if i != long_ax and abs(up[i]) < 0.5][0]

    def cross_section(a, b):
        sl = [p for p in pts if a <= p[long_ax] <= b]
        return (max(p.dot(up) for p in sl) - min(p.dot(up) for p in sl)) * (max(p[other] for p in sl) - min(p[other] for p in sl))

    lo_end = cross_section(lo[long_ax], lo[long_ax] + 0.15 * dim[long_ax])
    hi_end = cross_section(hi[long_ax] - 0.15 * dim[long_ax], hi[long_ax])
    f = Vector((0.0, 0.0, 0.0))
    f[long_ax] = 1.0 if hi_end < lo_end else -1.0
    rot = Matrix((f, up.cross(f), up))
    say(f"{label}: long axis {'XYZ'[long_ax]} ({dim[long_ax]:.3f}), end cross-sections lo {lo_end:.4f} hi {hi_end:.4f} -> muzzle {'+' if f[long_ax] > 0 else '-'}{'XYZ'[long_ax]}, up {fmt(up)}")
    return rot, dim[long_ax]


def transform(objs, xf):
    for o in objs:
        o.matrix_world = xf @ o.matrix_world
    apply_all(objs)


def bore_line(objs):
    """Where the barrel is at the muzzle: the centre of the front 10 % slab."""
    pts = [o.matrix_world @ v.co for o in objs for v in o.data.vertices]
    lo, hi = world_bbox(objs)
    sl = [p for p in pts if p.x >= hi.x - 0.10 * (hi.x - lo.x)]
    y = (max(p.y for p in sl) + min(p.y for p in sl)) * 0.5
    z = (max(p.z for p in sl) + min(p.z for p in sl)) * 0.5
    return Vector((hi.x, y, z))


def underside(objs, slabs=80):
    """Per slab along X: the lowest and highest Z. The grip and the
    magazine are the dips below the body's underside."""
    pts = [o.matrix_world @ v.co for o in objs for v in o.data.vertices]
    lo, hi = world_bbox(objs)
    w = (hi.x - lo.x) / slabs
    rows = []
    for i in range(slabs):
        a = lo.x + i * w
        sl = [p.z for p in pts if a <= p.x <= a + w]
        rows.append((a, a + w, min(sl) if sl else None, max(sl) if sl else None))
    return rows, hi.z - lo.z


def dips(rows, height, depth_frac=0.2):
    """Runs of slabs whose floor lies more than depth_frac of the height
    below the median floor (the body's underside), sorted along X, each
    with the floor of its neighbours (where the grip meets the body)."""
    floors = [r[2] for r in rows if r[2] is not None]
    base = statistics.median(floors)
    thr = base - depth_frac * height
    length = rows[-1][1] - rows[0][0]
    runs = []
    i = 0
    while i < len(rows):
        if rows[i][2] is not None and rows[i][2] < thr:
            j = i
            while j + 1 < len(rows) and rows[j + 1][2] is not None and rows[j + 1][2] < thr:
                j += 1
            # A grip is split by its trigger guard's slab and a magazine by
            # its ribs: runs closer than 3 % of the length are one dip. The
            # RPG's front grip came out as two "deepest" dips without this.
            if runs and rows[i][0] - rows[runs[-1][1]][1] < 0.03 * length:
                runs[-1] = (runs[-1][0], j)
            else:
                runs.append((i, j))
            i = j + 1
        else:
            i += 1
    out = []
    for i, j in runs:
        behind = next((rows[k][2] for k in range(i - 1, -1, -1) if rows[k][2] is not None), base)
        ahead = next((rows[k][2] for k in range(j + 1, len(rows)) if rows[k][2] is not None), base)
        # Where the grip meets the body: the higher neighbour, but never
        # above the body's underside. A sparse slab (the AK's gap between
        # stock and receiver holds only top-of-receiver vertices) would
        # otherwise put the hold point inside the receiver.
        floor = min(rows[k][2] for k in range(i, j + 1) if rows[k][2] is not None)
        out.append({"x0": rows[i][0], "x1": rows[j][1], "floor": floor, "top": min(max(behind, ahead), base)})
    say(f"  underside median {base:.3f}, dips deeper than {depth_frac:.0%} of {height:.3f}: " + "; ".join(f"x {d['x0']:.3f}..{d['x1']:.3f} floor {d['floor']:.3f} top {d['top']:.3f}" for d in out))
    if not out:
        die("no dip below the body: is the gun upside down?")
    return out


def set_origin(objs, origin, label):
    transform(objs, Matrix.Translation(-Vector(origin)))
    lo, hi = world_bbox(objs)
    say(f"{label}: origin set at {fmt(origin)}; box {fmt(lo)}..{fmt(hi)}")


def check_fit(label, objs, muzzle, target, below_bore=None, above_bore=None):
    """A derived fit cannot check itself (docs/asset-pipeline.md): the
    muzzle must be forward of the origin and within 8 % of the front bound,
    the length on target, the grip below the barrel, and whatever hangs
    below or sits above the bore where the file says it does."""
    lo, hi = world_bbox(objs)
    length = hi.x - lo.x
    if not (muzzle.x > 0.0 and muzzle.x >= hi.x - 0.08 * length):
        die(f"{label}: muzzle {fmt(muzzle)} is not at the front of the box {fmt(lo)}..{fmt(hi)}")
    if abs(length - target) > 0.02 * target:
        die(f"{label}: {length:.3f} long, target {target}")
    if not (lo.x < 0.0 < hi.x and lo.z < 0.0 < muzzle.z):
        die(f"{label}: origin is not on the grip under the bore: box {fmt(lo)}..{fmt(hi)}, bore z {muzzle.z:.3f}")
    if below_bore is not None and not below_bore.z < muzzle.z:
        die(f"{label}: the magazine ({fmt(below_bore)}) is not below the bore ({muzzle.z:.3f}): the gun is upside down")
    if above_bore is not None and not above_bore.z > muzzle.z:
        die(f"{label}: the sight ({fmt(above_bore)}) is not above the bore ({muzzle.z:.3f}): the gun is upside down")
    say(f"{label}: fitted {length:.3f} long, box {fmt(lo)}..{fmt(hi)}, muzzle {fmt(muzzle)}")


def centre(obj):
    lo, hi = world_bbox([obj])
    return (lo + hi) * 0.5


def triangles(me):
    return sum(len(p.vertices) - 2 for p in me.polygons)


def decimate(obj, budget):
    """To a TRIANGLE budget: the AK's .blend is 15 843 polygons but they are
    n-gons that export as 311 918 vertices, so a polygon count says nothing
    about what a web player downloads."""
    tris = triangles(obj.data)
    if tris <= budget:
        say(f"{obj.name}: {tris} triangles, under the budget of {budget}")
        return
    mod = obj.modifiers.new("decimate", "DECIMATE")
    mod.ratio = budget / tris
    mod.use_collapse_triangulate = True
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)
    say(f"{obj.name}: decimated {tris} -> {triangles(obj.data)} triangles ({len(obj.data.polygons)} faces, {len(obj.data.vertices)} vertices)")


# ---- the atlas bake ----------------------------------------------------------------
def bake_atlas(obj, name, pictures, size):
    """Several materials, one picture. The original islands of every
    material are packed together into a new UV layer `atlas`; every
    material samples ITS picture through the ORIGINAL layer; Cycles bakes
    the diffuse colour into one image through `atlas`; then the original
    layer goes, `atlas` becomes UVMap and one picture_material replaces
    them all. The result is asserted non-uniform, because a bake that found
    no image node, or the wrong UV layer, writes black without an error."""
    me = obj.data
    if len(me.uv_layers) != 1:
        die(f"{name}: expected one UV layer before the bake, got {[l.name for l in me.uv_layers]}")
    orig = me.uv_layers[0]
    orig_name = orig.name
    for i, m in enumerate(me.materials):
        if m is None or m.name not in pictures:
            die(f"{name}: material slot {i} is {m.name if m else None}, no picture for it; have {sorted(pictures)}")
    used = {p.material_index for p in me.polygons}
    say(f"{name}: baking {len(used)} materials ({', '.join(me.materials[i].name for i in sorted(used))}) into a {size} atlas")

    # 1. the atlas layer, seeded with the original islands, packed. The
    #    original UVs are read out BEFORE the new layer exists: adding a
    #    layer reallocates the mesh's layer data and a reference taken
    #    earlier reads the wrong (empty) layer, silently.
    buf = [0.0] * (len(orig.data) * 2)
    orig.data.foreach_get("uv", buf)
    me.uv_layers.new(name="atlas", do_init=False)
    atlas = me.uv_layers["atlas"]
    atlas.data.foreach_set("uv", buf)
    if uv_area(me, atlas) < 0.05:
        die(f"{name}: the atlas layer did not take the original UVs (area {uv_area(me, atlas):.3f})")
    me.uv_layers.active_index = me.uv_layers.find("atlas")
    me.uv_layers["atlas"].active_render = True
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.context.scene.tool_settings.use_uv_select_sync = True
    # The margin is ADDED in the islands' own scale, not a fraction of the
    # tile: these meshes carry thousands of islands (the Vityaz receiver
    # alone is a sci-fi shell of separate plates), and a per-island
    # fraction of 0.02 left nothing but margin, spilled across a 4x4 grid
    # of tiles and zeroed every face. Measured on the Vityaz: FRACTION
    # 0.004 covers 27 % of the tile, FRACTION 0.002 45 %, ADD 0.002 67 %.
    # merge_overlap stays off because islands of DIFFERENT materials
    # overlap in the artist's UV space by construction.
    with Step(f"{name}: pack islands"):
        bpy.ops.object.mode_set(mode="EDIT")
        bpy.ops.mesh.select_all(action="SELECT")
        bpy.ops.uv.select_all(action="SELECT")
        bpy.ops.uv.pack_islands(rotate=True, scale=True, merge_overlap=False, margin_method="ADD", margin=0.002, shape_method="CONCAVE")
        bpy.ops.object.mode_set(mode="OBJECT")
    atlas = me.uv_layers["atlas"]
    coverage = uv_area(me, atlas)
    d = atlas.data
    if coverage < 0.2 or max(max(d[i].uv) for i in range(0, len(d), 7)) > 1.001 or min(min(d[i].uv) for i in range(0, len(d), 7)) < -0.001:
        die(f"{name}: the packed atlas covers {coverage:.3f} of the tile or leaves it; the pack did not fit")
    say(f"{name}: atlas islands cover {coverage:.0%} of the tile")

    # 2. one bake material per slot: picture through the original UVs,
    #    the shared target image node selected and active.
    target = bpy.data.images.new(f"{name}-atlas", size, size, alpha=False)
    target.colorspace_settings.name = "sRGB"
    for i, m in enumerate(list(me.materials)):
        path = pictures[m.name]
        if not os.path.isfile(path):
            die(f"missing {path}", "run tools/v18/prep_pictures.py")
        img = bpy.data.images.load(path, check_existing=False)
        if img.size[0] == 0:
            die(f"{path} did not decode")
        mat = bpy.data.materials.new(f"{name}_bake_{m.name}")
        mat.use_nodes = True
        nodes = mat.node_tree.nodes
        links = mat.node_tree.links
        bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = img
        uvn = nodes.new("ShaderNodeUVMap")
        uvn.uv_map = orig_name
        links.new(uvn.outputs["UV"], tex.inputs["Vector"])
        links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        tgt = nodes.new("ShaderNodeTexImage")
        tgt.image = target
        for n in nodes:
            n.select = False
        tgt.select = True
        nodes.active = tgt
        me.materials[i] = mat

    # 3. Cycles, diffuse colour only.
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = BAKE_SAMPLES
    bake = scene.render.bake
    bake.use_pass_direct = False
    bake.use_pass_indirect = False
    bake.use_pass_color = True
    bake.margin = 4
    bake.use_selected_to_active = False
    bake.target = "IMAGE_TEXTURES"
    with Step(f"{name}: cycles bake {size}x{size} at {BAKE_SAMPLES} samples"):
        result = bpy.ops.object.bake(type="DIFFUSE")
    if result != {"FINISHED"}:
        die(f"{name}: bake returned {result}")

    # 4. the bake is not trusted until sampled.
    out_path = os.path.join(A, name[2:].split("_")[0], "baked", f"{name}-atlas-{size}.png")
    target.filepath_raw = out_path
    target.file_format = "PNG"
    # Image.save() takes the PNG level from the scene's render settings,
    # whose default is 15 %; the exporter ships these bytes as they are.
    scene.render.image_settings.compression = 100
    target.save()
    px = [0.0] * (size * size * 4)
    target.pixels.foreach_get(px)
    sample = [px[k * 4 + c] for k in range(0, size * size, 100) for c in range(3)]
    sd = statistics.pstdev(sample)
    mean = statistics.fmean(sample)
    if sd < BAKE_MIN_STDDEV:
        die(f"{name}: the baked atlas is uniform (mean {mean:.3f}, std-dev {sd:.4f}); the bake failed silently", "try --split")
    say(f"{name}: atlas written to {os.path.relpath(out_path, REPO)} (sample mean {mean:.3f}, std-dev {sd:.3f})")

    # 5. the atlas is the only UV layer, and one picture the only material.
    me.uv_layers.remove(me.uv_layers[orig_name])
    me.uv_layers["atlas"].name = "UVMap"
    me.uv_layers.active_index = 0
    for m in list(me.materials):
        bpy.data.materials.remove(m)
    set_material(obj, picture_material(name, out_path, size))
    return out_path


def split_by_material(obj, name, pictures, size):
    """The --split fallback: one part per material at `size`, node names
    `w_<weapon>_<material>` (the client classifies by prefix), so the lane
    is never blocked on Cycles. Costs one mesh id and one picture clone per
    material in VRAM."""
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.separate(type="MATERIAL")
    bpy.ops.object.mode_set(mode="OBJECT")
    parts = []
    for o in bpy.context.selected_objects:
        m = next((m for m in o.data.materials if m and any(p.material_index == i for i in range(len(o.data.materials)) for p in o.data.polygons[:1])), o.data.materials[0])
        key = m.name.replace(" ", "_")
        src = pictures[m.name]
        img = bpy.data.images.load(src, check_existing=False)
        img.scale(size, size)
        if tuple(img.size) != (size, size):
            die(f"{src}: Image.scale() left it at {tuple(img.size)}")
        small = os.path.join(os.path.dirname(src), f"{key}-{size}.png")
        img.filepath_raw = small
        img.file_format = "PNG"
        img.save()
        part_name = f"{name}_{key}"
        o.name = part_name
        o.data.name = part_name
        set_material(o, picture_material(part_name, small, size))
        smooth(o)
        parts.append(o)
    say(f"{name}: split into {[p.name for p in parts]}")
    return parts


# ---- the weapons -------------------------------------------------------------------
def build_ak47():
    with Step("ak47"):
        objs = append_blend_objects(AK_BLEND, {"AK"})
        prepare(objs)
        # One mesh, no landmarks: up is the side with the smaller reach from
        # the vertex median along the second axis, because the magazine and
        # the grip hang further than the sights rise. The probe (plan 8.1)
        # says the magazine hangs -Z; the measurement must agree.
        pts = [o.matrix_world @ v.co for o in objs for v in o.data.vertices]
        lo, hi = world_bbox(objs)
        dim = hi - lo
        order = sorted(range(3), key=lambda i: dim[i], reverse=True)
        vert = order[1]
        med = statistics.median(p[vert] for p in pts)
        up = Vector((0.0, 0.0, 0.0))
        up[vert] = 1.0 if hi[vert] - med < med - lo[vert] else -1.0
        say(f"ak47: vertical axis {'XYZ'[vert]}, median {med:.3f}, reach up {hi[vert] - med:.3f} / down {med - lo[vert]:.3f} -> up {fmt(up)}")
        if tuple(up) != (0.0, 0.0, 1.0):
            die(f"ak47: measured up {fmt(up)} disagrees with the probe (magazine hangs -Z)")
        rot, length = measure_frame(objs, up, "ak47")
        transform(objs, Matrix.Scale(TARGET["w_ak47"] / length, 4) @ rot.to_4x4())
        rows, height = underside(objs)
        found = dips(rows, height)
        mag = max(found, key=lambda d: -d["floor"])
        behind = [d for d in found if d["x1"] <= mag["x0"]]
        if not behind:
            die("ak47: no grip dip behind the magazine")
        grip = behind[-1]
        bore = bore_line(objs)
        set_origin(objs, (grip["x1"], bore.y, grip["top"]), "ak47")
        obj = join_into(objs, "w_ak47")
        set_material(obj, picture_material("w_ak47", AK_PICTURE, 1024))
        smooth(obj)
        decimate(obj, FACE_BUDGET["w_ak47"])
        muzzle = bore_line([obj])
        muzzle.y = 0.0
        check_fit("w_ak47", [obj], muzzle, TARGET["w_ak47"], below_bore=Vector((0.0, 0.0, mag["floor"] - grip["top"])))
        say(f"w_ak47: {len(obj.data.polygons)} faces")
        return [obj], muzzle


def build_vityaz(split):
    with Step("vityaz"):
        objs = import_fbx(VITYAZ_FBX, VITYAZ_OBJECTS)
        for o in list(objs):
            if o.name in VITYAZ_DROP:
                bpy.data.objects.remove(o, do_unlink=True)
                objs.remove(o)
        say(f"vityaz: dropped {sorted(VITYAZ_DROP)}; {len(objs)} meshes stay")
        prepare(objs)
        by = {o.name: o for o in objs}
        body_c = centre(by["receiver"])
        mag_c = centre(by["Magazine"])
        up = Vector((0.0, 0.0, 0.0))
        vert = max((i for i in range(3)), key=lambda i: abs(mag_c[i] - body_c[i]))
        up[vert] = 1.0 if mag_c[vert] < body_c[vert] else -1.0
        say(f"vityaz: magazine {fmt(mag_c)} against receiver {fmt(body_c)} -> up {fmt(up)}")
        rot, length = measure_frame(objs, up, "vityaz")
        transform(objs, Matrix.Scale(TARGET["w_vityaz"] / length, 4) @ rot.to_4x4())
        # The grip is a named part: its top at its front edge, where the
        # trigger (also named) sits just ahead. The underside profile must
        # agree: a dip behind the magazine that overlaps the grip's box.
        glo, ghi = world_bbox([by["pistol grip"]])
        tlo, thi = world_bbox([by["Trigger"]])
        mlo, mhi = world_bbox([by["Magazine"]])
        if not (tlo.x >= glo.x - 0.02 and thi.x <= mlo.x):
            die(f"vityaz: trigger x {tlo.x:.3f}..{thi.x:.3f} is not between the grip ({glo.x:.3f}..{ghi.x:.3f}) and the magazine ({mlo.x:.3f}..{mhi.x:.3f})")
        rows, height = underside(objs)
        found = dips(rows, height)
        if not any(d["x1"] <= mlo.x + 0.01 and d["x1"] > glo.x and d["x0"] < ghi.x for d in found):
            die(f"vityaz: no underside dip behind the magazine overlaps the grip {glo.x:.3f}..{ghi.x:.3f}")
        bore = bore_line(objs)
        set_origin(objs, (ghi.x, bore.y, ghi.z), "vityaz")
        mag_c = centre(by["Magazine"])
        obj = join_into(objs, "w_vityaz")
        pictures = {m.name: os.path.join(A, "vityaz", "baked", f"{m.name.replace(' ', '_')}-1024.png") for m in obj.data.materials}
        if split:
            parts = split_by_material(obj, "w_vityaz", pictures, 512)
        else:
            bake_atlas(obj, "w_vityaz", pictures, BAKE_SIZE)
            smooth(obj)
            decimate(obj, FACE_BUDGET["w_vityaz"])
            parts = [obj]
        muzzle = bore_line(parts)
        muzzle.y = 0.0
        check_fit("w_vityaz", parts, muzzle, TARGET["w_vityaz"], below_bore=mag_c)
        say(f"w_vityaz: {sum(len(p.data.polygons) for p in parts)} faces")
        return parts, muzzle


def build_sniper(split):
    with Step("sniper"):
        # The FBX names its body `rifle`, and the sidearm node of that name
        # is already in the scene; Blender would import it as `rifle.002`.
        # The sidearm steps aside under a stash name until the sniper has
        # its own, so the expected-set check reads the file's names.
        stash = [o for o in bpy.data.objects if o.name in SNIPER_OBJECTS]
        for o in stash:
            o.name = o.name + "__stash"
            o.data.name = o.data.name + "__stash"
        objs = import_fbx(SNIPER_FBX, SNIPER_OBJECTS)
        for o in list(objs):
            if o.name in SNIPER_DROP:
                bpy.data.objects.remove(o, do_unlink=True)
                objs.remove(o)
        prepare(objs)
        by = {o.name: o for o in objs}
        body_c = centre(by["rifle"])
        mag_c = centre(by["mag"])
        # The rifle's box is dominated by the scope, so the magazine's
        # offset from the rifle's centre is read along the axis where it is
        # largest after the long axis is excluded.
        lo, hi = world_bbox(objs)
        dim = hi - lo
        long_ax = max(range(3), key=lambda i: dim[i])
        vert = max((i for i in range(3) if i != long_ax), key=lambda i: abs(mag_c[i] - body_c[i]))
        up = Vector((0.0, 0.0, 0.0))
        up[vert] = 1.0 if mag_c[vert] < body_c[vert] else -1.0
        say(f"sniper: magazine {fmt(mag_c)} against rifle {fmt(body_c)} -> up {fmt(up)}")
        rot, length = measure_frame(objs, up, "sniper")
        transform(objs, Matrix.Scale(TARGET["w_sniper"] / length, 4) @ rot.to_4x4())
        # A bullpup: the grip is the deepest dip and the magazine (a named
        # part) sits BEHIND it, in the stock.
        rows, height = underside(objs)
        found = dips(rows, height)
        grip = min(found, key=lambda d: d["floor"])
        mlo, mhi = world_bbox([by["mag"]])
        if not mhi.x <= grip["x0"] + 0.02:
            die(f"sniper: the magazine ({mlo.x:.3f}..{mhi.x:.3f}) is not behind the deepest dip ({grip['x0']:.3f}..{grip['x1']:.3f}); not the bullpup the probe saw")
        bore = bore_line(objs)
        set_origin(objs, (grip["x1"], bore.y, grip["top"]), "sniper")
        mag_c = centre(by["mag"])
        obj = join_into(objs, "w_sniper")
        for o in stash:
            o.name = o.name[: -len("__stash")]
            o.data.name = o.data.name[: -len("__stash")]
        pictures = {m.name: os.path.join(A, "sniper", "baked", f"{m.name}-1024.png") for m in obj.data.materials}
        if split:
            parts = split_by_material(obj, "w_sniper", pictures, 512)
        else:
            bake_atlas(obj, "w_sniper", pictures, BAKE_SIZE)
            smooth(obj)
            decimate(obj, FACE_BUDGET["w_sniper"])
            parts = [obj]
        muzzle = bore_line(parts)
        muzzle.y = 0.0
        check_fit("w_sniper", parts, muzzle, TARGET["w_sniper"], below_bore=mag_c)
        say(f"w_sniper: {sum(len(p.data.polygons) for p in parts)} faces")
        return parts, muzzle


def build_rpg7():
    with Step("rpg7"):
        objs = import_fbx(RPG_FBX, RPG_OBJECTS)
        prepare(objs)
        by = {o.name: o for o in objs}
        launcher = [by[n] for n in ("RPG7", "hammer", "sight", "sight_adjust")]
        rocket = by["rocket"]
        tube_c = centre(by["RPG7"])
        sight_c = centre(by["sight"])
        lo, hi = world_bbox(launcher)
        dim = hi - lo
        long_ax = max(range(3), key=lambda i: dim[i])
        vert = max((i for i in range(3) if i != long_ax), key=lambda i: abs(sight_c[i] - tube_c[i]))
        up = Vector((0.0, 0.0, 0.0))
        up[vert] = 1.0 if sight_c[vert] > tube_c[vert] else -1.0
        say(f"rpg7: sight {fmt(sight_c)} against tube {fmt(tube_c)} -> up {fmt(up)}")
        # The frame is the launcher's: the rocket protrudes from the muzzle
        # and would pull the length and the thin-end test its own way.
        rot, length = measure_frame(launcher, up, "rpg7")
        transform(objs, Matrix.Scale(TARGET["w_rpg7"] / length, 4) @ rot.to_4x4())
        # No magazine. Two grips under the tube: the trigger grip is the
        # front one, and the deeper of the two.
        rows, height = underside(launcher)
        found = dips(rows, height)
        grip = min(found, key=lambda d: d["floor"])
        if any(d["x0"] > grip["x1"] for d in found):
            die("rpg7: a dip lies ahead of the deepest one; the trigger grip should be the front grip")
        bore = bore_line(launcher)
        set_origin(objs, (grip["x1"], bore.y, grip["top"]), "rpg7")
        sight_c = centre(by["sight"])
        tube = join_into(launcher, "w_rpg7")
        set_material(tube, picture_material("w_rpg7", RPG_PICTURE, 1024))
        smooth(tube)
        decimate(tube, FACE_BUDGET["w_rpg7"])
        rocket.name = "w_rpg7_rocket"
        rocket.data.name = "w_rpg7_rocket"
        set_material(rocket, picture_material("w_rpg7_rocket", ROCKET_PICTURE, 512))
        smooth(rocket)
        muzzle = bore_line([tube])
        muzzle.y = 0.0
        check_fit("w_rpg7", [tube], muzzle, TARGET["w_rpg7"], above_bore=sight_c)
        seat_rocket(rocket, tube, muzzle)
        tlo, thi = world_bbox([tube])
        rlo, rhi = world_bbox([rocket])
        say(f"w_rpg7: {len(tube.data.polygons)} faces; w_rpg7_rocket: {len(rocket.data.polygons)} faces, box {fmt(rlo)}..{fmt(rhi)}, warhead {rhi.x - thi.x:.3f} out of the muzzle")
        return [tube, rocket], muzzle


def seat_rocket(rocket, tube, muzzle):
    """The file lays the rocket BESIDE the launcher as a display piece
    (35 cm to the side, below the tube), so it is seated by measurement:
    the warhead is the widest slab and goes forward; everything wider than
    the bore stays outside the muzzle; the booster's axis lies on the bore.
    Then the client draws it in place at the unit transform."""

    def slabs(obj, n=40):
        pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
        lo, hi = world_bbox([obj])
        w = (hi.x - lo.x) / n
        out = []
        for i in range(n):
            sl = [p for p in pts if lo.x + i * w <= p.x <= lo.x + (i + 1) * w]
            if sl:
                out.append((lo.x + i * w, lo.x + (i + 1) * w, max(max(p.y for p in sl) - min(p.y for p in sl), max(p.z for p in sl) - min(p.z for p in sl)), sl))
        return out, lo, hi

    # The artist laid the rocket at an angle to the launcher, so its axis
    # is measured first: the principal axis of its vertices, turned onto
    # +X about the rocket's own centre (a body of revolution: its roll does
    # not matter).
    import numpy as np

    pts = np.array([list(rocket.matrix_world @ v.co) for v in rocket.data.vertices])
    c = pts.mean(axis=0)
    _vals, vecs = np.linalg.eigh(np.cov((pts - c).T))
    axis = Vector(vecs[:, -1].tolist()).normalized()
    if axis.x < 0.0:
        axis = -axis
    tilt = math.degrees(axis.angle(Vector((1.0, 0.0, 0.0))))
    spin = axis.rotation_difference(Vector((1.0, 0.0, 0.0))).to_matrix().to_4x4()
    centre_v = Vector(c.tolist())
    transform([rocket], Matrix.Translation(centre_v) @ spin @ Matrix.Translation(-centre_v))
    say(f"rocket: axis {fmt(axis)} was {tilt:.1f} deg off the bore; turned onto +X")
    rows, lo, hi = slabs(rocket)
    widest = max(rows, key=lambda r: r[2])
    if (widest[0] + widest[1]) * 0.5 < (lo.x + hi.x) * 0.5:
        c = (lo + hi) * 0.5
        transform([rocket], Matrix.Translation(c) @ Matrix.Rotation(math.pi, 4, "Z") @ Matrix.Translation(-c))
        say("rocket: warhead was at the rear; turned end for end")
        rows, lo, hi = slabs(rocket)
    tlo, thi = world_bbox([tube])
    # The booster is the rocket's uniform rear body; the warhead begins
    # where the width flares past it. The flare sits just outside the
    # muzzle, which is how a PG-7 rides in its tube. (The bore itself is no
    # guide: the artist's booster is as wide as the tube's outside.)
    rear_rows = rows[: max(3, len(rows) * 2 // 5)]
    body = statistics.median(r[2] for r in rear_rows)
    flare = [r for r in rows if r[2] > 1.15 * body]
    if not flare:
        die(f"rocket: no warhead flare wider than 1.15 x the body ({body:.3f}); is this the rocket?")
    x_flare = min(r[0] for r in flare)
    rear = [p for r in rear_rows for p in r[3]]
    axis_y = (max(p.y for p in rear) + min(p.y for p in rear)) * 0.5
    axis_z = (max(p.z for p in rear) + min(p.z for p in rear)) * 0.5
    shift = Vector((muzzle.x + 0.01 - x_flare, muzzle.y - axis_y, muzzle.z - axis_z))
    transform([rocket], Matrix.Translation(shift))
    rlo, rhi = world_bbox([rocket])
    if not (tlo.x + 0.05 < rlo.x < thi.x and rhi.x > thi.x + 0.1):
        die(f"rocket: seated box {fmt(rlo)}..{fmt(rhi)} is not in the tube {fmt(tlo)}..{fmt(thi)} with the warhead out")
    say(f"rocket: body {body:.3f} wide, warhead {widest[2]:.3f} wide flaring at x {x_flare:.3f}; moved by {fmt(shift)}; {rhi.x - thi.x:.3f} out of the muzzle, {thi.x - rlo.x:.3f} in the tube, {rlo.x - tlo.x:.3f} ahead of the breech")


def build_revolver():
    with Step("revolver"):
        parts, pivots, muzzle = v15.build_revolver()
        # v15's fit as it shipped: 0.75 long, +X, origin on the grip. The
        # frame keeps its 1024 picture; the four small parts ship at 512.
        # The frame group wears M2 and the receiver M1 in the OBJ; three
        # small parts share one 512 M2 material so the file carries the
        # picture once (the renderer clones per mesh anyway).
        m2_small = picture_material("w_revolver_small", os.path.join(REVOLVER_BAKED, "M2-512.png"), 512)
        pictures = {
            "frame": picture_material("w_revolver_frame", os.path.join(REVOLVER_BAKED, "M2.png"), 1024),
            "receiver": picture_material("w_revolver_receiver", os.path.join(REVOLVER_BAKED, "M1-512.png"), 512),
            "cylinder": m2_small, "hammer": m2_small, "trigger": m2_small,
        }
        objs = []
        out_pivots = {}
        for key, obj in parts.items():
            old = {m.name.split(".")[0] for m in obj.data.materials if m}
            want = {"frame": "M2", "receiver": "M1"}.get(key, "M2")
            if old != {want}:
                die(f"revolver {key}: wears {sorted(old)}, expected {want}")
            name = f"w_revolver_{key}"
            obj.name = name
            obj.data.name = name
            keep_one_uv(obj)
            set_material(obj, pictures[key])
            objs.append(obj)
            if key in pivots:
                out_pivots[name] = pivots[key]
        check_fit("w_revolver", objs, muzzle, TARGET["w_revolver"])
        say(f"revolver: pivots {', '.join(f'{k} {fmt(v)}' for k, v in out_pivots.items())}")
        return objs, muzzle, out_pivots


# ---- export and the sidecar ------------------------------------------------------
def export(objs, sidearm_muzzle, muzzles, pivots):
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    with Step("export"):
        bpy.ops.export_scene.gltf(
            filepath=OUT_GLB,
            export_format="GLB",
            export_yup=True,
            use_selection=True,
            export_apply=True,
            export_image_format="AUTO",
        )
    # Pivot maps before any list: the sidecar scanner note in
    # docs/asset-pipeline.md; serde reads this file, the order costs nothing.
    rig = {
        "pivots": {name: to_engine(p) for name, p in sorted(pivots.items())},
        "muzzle": to_engine(sidearm_muzzle),
        "muzzles": {name: to_engine(m) for name, m in sorted(muzzles.items())},
    }
    with open(OUT_RIG, "w", encoding="utf-8", newline="\n") as f:
        json.dump(rig, f, indent=1)
        f.write("\n")
    say(f"wrote {OUT_GLB} ({os.path.getsize(OUT_GLB) / 1e6:.2f} MB) and {OUT_RIG}")


def png_header(png):
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        die("image is not a PNG")
    w, h, depth, ctype = struct.unpack(">IIBB", png[16:26])
    return w, h, depth, ctype


def verify_glb(split=False):
    """Plan 8.2.6: names, one primitive with UVs and a picture each, PNGs
    re-decoded and held to 8-bit at no more than the intended size, white
    base colour, every weapon's front bound forward and its length on
    target, the rocket in the tube, the file size. Runs inside Blender
    (PNG headers) and again under the PIL python (--verify: a full decode,
    the way the engine's decoder will see the bytes)."""
    try:
        from PIL import Image
    except ImportError:
        Image = None
    with open(OUT_GLB, "rb") as f:
        data = f.read()
    magic, _version, total = struct.unpack("<III", data[:12])
    if magic != 0x46546C67 or total != len(data):
        die("not a GLB, or truncated")
    clen, ctype = struct.unpack("<II", data[12:20])
    if ctype != 0x4E4F534A:
        die("first chunk is not JSON")
    doc = json.loads(data[20:20 + clen])
    boff = 20 + clen
    blen, btype = struct.unpack("<II", data[boff:boff + 8])
    if btype != 0x004E4942:
        die("second chunk is not BIN")
    blob = data[boff + 8:boff + 8 + blen]

    def intended(name):
        if name in INTENDED_SIZE:
            return INTENDED_SIZE[name]
        if split and (name.startswith("w_vityaz_") or name.startswith("w_sniper_")):
            return 512
        die(f"unexpected node {name!r}")

    meshes = sorted(m.get("name") for m in doc.get("meshes", []))
    nodes = sorted(n.get("name") for n in doc.get("nodes", []))
    if split:
        want = sorted(n for n in WANT_NODES if n not in ("w_vityaz", "w_sniper")) + sorted(n for n in nodes if n.startswith("w_vityaz_") or n.startswith("w_sniper_"))
        want.sort()
    else:
        want = WANT_NODES
    if meshes != want or nodes != want:
        die(f"expected nodes and meshes {want}; got meshes {meshes}, nodes {nodes}")
    for n in doc["nodes"]:
        if any(k in n for k in ("translation", "rotation", "scale", "matrix")):
            die(f"node {n['name']!r} carries a transform; the loader bakes it, but the sidecar points assume identity")

    bounds = {}
    vram = 0.0
    picture_bytes = {}
    tris_total = 0
    for m in doc["meshes"]:
        name = m["name"]
        if len(m["primitives"]) != 1:
            die(f"mesh {name!r} has {len(m['primitives'])} primitives; one picture per mesh")
        prim = m["primitives"][0]
        if "TEXCOORD_0" not in prim["attributes"]:
            die(f"mesh {name!r} has no TEXCOORD_0")
        if "material" not in prim:
            die(f"mesh {name!r} has no material")
        mat = doc["materials"][prim["material"]]
        pbr = mat.get("pbrMetallicRoughness", {})
        if "baseColorTexture" not in pbr:
            die(f"mesh {name!r} has a material without a picture")
        if any(abs(c - 1.0) > 1e-3 for c in pbr.get("baseColorFactor", [1, 1, 1, 1])[:3]):
            die(f"mesh {name!r} has a tinted baseColorFactor {pbr['baseColorFactor']}")
        img = doc["images"][doc["textures"][pbr["baseColorTexture"]["index"]]["source"]]
        if img.get("mimeType") != "image/png":
            die(f"mesh {name!r}: image {img.get('name')} is {img.get('mimeType')}, not PNG")
        bv = doc["bufferViews"][img["bufferView"]]
        png = blob[bv.get("byteOffset", 0):bv.get("byteOffset", 0) + bv["byteLength"]]
        w, h, depth, colour = png_header(png)
        if depth != 8:
            die(f"mesh {name!r}: image is {depth}-bit; the engine decodes 8-bit only, silently")
        if colour not in (0, 2, 6):
            die(f"mesh {name!r}: PNG colour type {colour} is not grey/RGB/RGBA")
        limit = intended(name)
        if max(w, h) > limit:
            die(f"mesh {name!r}: image {w}x{h} exceeds the intended {limit}")
        if Image is not None:
            im = Image.open(io.BytesIO(png))
            im.load()
            if im.mode not in ("RGB", "RGBA", "L"):
                die(f"mesh {name!r}: PIL decodes the image as {im.mode}, which the engine will not")
            if im.size != (w, h):
                die(f"mesh {name!r}: header {w}x{h} but PIL decoded {im.size}")
        vram += w * h * 4 * 4 / 3
        picture_bytes[img["bufferView"]] = bv["byteLength"]
        acc = doc["accessors"][prim["attributes"]["POSITION"]]
        tris = doc["accessors"][prim["indices"]]["count"] // 3 if "indices" in prim else acc["count"] // 3
        tris_total += tris
        bounds[name] = (acc["min"], acc["max"])
        say(f"  {name}: {acc['count']} vertices, {tris} triangles, picture {w}x{h} {depth}-bit ({bv['byteLength'] // 1024} KB), x {acc['min'][0]:.3f}..{acc['max'][0]:.3f}")

    def union(names):
        return min(bounds[n][0][0] for n in names), max(bounds[n][1][0] for n in names)

    def family(name):
        return [n for n in bounds if n == name or n.startswith(name + "_")]

    def check_length(label, names, target):
        lo, hi = union(names)
        if not hi > 0.0:
            die(f"{label}: front bound {hi:.3f} is not forward of the origin")
        if abs((hi - lo) - target) > 0.02 * target:
            die(f"{label}: {hi - lo:.3f} long in the file, target {target}")
        say(f"  {label}: {hi - lo:.3f} long, front bound {hi:.3f}")

    check_length("w_ak47", ["w_ak47"], TARGET["w_ak47"])
    check_length("w_rpg7", ["w_rpg7"], TARGET["w_rpg7"])
    check_length("w_revolver", family("w_revolver"), TARGET["w_revolver"])
    for w_name in ("w_vityaz", "w_sniper"):
        check_length(w_name, family(w_name), TARGET[w_name])
    tube, rocket = bounds["w_rpg7"], bounds["w_rpg7_rocket"]
    if not (tube[0][0] < rocket[0][0] < tube[1][0] and rocket[1][0] > tube[1][0]):
        die(f"w_rpg7_rocket x {rocket[0][0]:.3f}..{rocket[1][0]:.3f} does not ride in the tube x {tube[0][0]:.3f}..{tube[1][0]:.3f}")
    with open(OUT_RIG, encoding="utf-8") as f:
        rig = json.load(f)
    if list(rig)[:1] != ["pivots"]:
        die("sidecar: pivots must come first")
    for name, m in rig.get("muzzles", {}).items():
        # The revolver's muzzle is keyed by its frame but lies at the
        # receiver's front, so a muzzle is held to its weapon's part union.
        fam = family(name.replace("w_revolver_frame", "w_revolver"))
        if not fam:
            die(f"sidecar muzzle for a node that is not in the GLB: {name}")
        lo, hi = union(fam)
        if not (m[0] > 0.0 and m[0] <= hi + 1e-3):
            die(f"sidecar muzzle {name} {m} is not at the front of {fam} (x {lo:.3f}..{hi:.3f})")
    for name in rig.get("pivots", {}):
        if name not in bounds:
            die(f"sidecar pivot for a node that is not in the GLB: {name}")
    say(f"verified: {len(meshes)} nodes {meshes}; {len(doc['images'])} PNG images ({sum(picture_bytes.values()) / 1e6:.2f} MB of pictures), {tris_total} triangles; texture VRAM with mip chains {vram / 1e6:.1f} MB; GLB {len(data) / 1e6:.2f} MB ({'PIL decode' if Image else 'PNG headers only'})")


def main():
    preview = "--preview" in sys.argv
    split = "--split" in sys.argv
    with Step("v16 operator"):
        rifle, hands, sidearm_muzzle, extras = v16.build_operator()
    with Step("v17 shield, sword, fist"):
        shield = v17.build_shield()
        sword, _guard = v17.build_sword()
        fist = v17.build_fist(extras)
    vityaz, m_vityaz = build_vityaz(split)
    ak, m_ak = build_ak47()
    revolver, m_revolver, pivots = build_revolver()
    sniper, m_sniper = build_sniper(split)
    rpg, m_rpg = build_rpg7()
    muzzles = {"w_vityaz": m_vityaz, "w_ak47": m_ak, "w_revolver_frame": m_revolver, "w_sniper": m_sniper, "w_rpg7": m_rpg}
    objs = [rifle, hands, shield, sword, fist] + vityaz + ak + revolver + sniper + rpg
    for o in objs:
        if o.name != o.data.name:
            die(f"{o.name}: mesh data is named {o.data.name!r}; the exporter names meshes after data")
    export(objs, sidearm_muzzle, muzzles, pivots)
    verify_glb(split)
    if preview:
        v17.PREVIEW_DIR = HERE
        with Step("previews"):
            v17.render_previews({
                "operator": [rifle, hands], "shield": [shield], "sword": [sword, fist],
                "vityaz": vityaz, "ak47": ak, "revolver": revolver, "sniper": sniper, "rpg7": rpg,
            })
    say(f"total {time.time() - T_START:.1f} s")


if __name__ == "__main__":
    if bpy is None or "--verify" in sys.argv:
        verify_glb("--split" in sys.argv)
    else:
        main()
