"""Generate the FALLBACK viewmodel (box pistol) as a GLB.

Run headless:
    blender --background --python tools/make_assets.py -- crates/arena/assets/viewmodel.glb --with-pistol

Conventions (must match the engine): +X forward, +Z up in Blender; the
default Y-up glTF export maps this to +X forward / +Y up in the engine.
Units are game units (the pistol is ~0.9 long). Node names matter:
parts starting with "arm"/"hand" are viewmodel-only (not drawn on remote
players); the part named "strip" gets recolored by weapon level.

THE SPLIT
    This script used to emit the ENTIRE viewmodel as 12 flat boxes: the
    pistol (slide, barrel, frame, grip, guard, sight_f, sight_r, strip)
    AND the hands (hand_r, arm_r, hand_l, arm_l). The pistol half is being
    replaced by the real 9mm model, so the two halves are now separate:

      * HANDS  -> tools/make_hands.py, which owns the geometry and the
                  grip-fit constants. This file does not duplicate them;
                  it imports make_hands and calls build_hands(), so the
                  fallback viewmodel's hands can never drift from the
                  real one's.
      * PISTOL -> still here, below, byte-for-byte the geometry it always
                  was. It is NOT dead code: crates/arena/src/online.rs has
                  a cube-pistol fallback (push_gun) for a viewmodel GLB
                  that fails to load, and this box pistol is the authored
                  counterpart of that. It is emitted only on request.

MODES
    (default)       hands only — equivalent to running make_hands.py
    --with-pistol   box pistol + hands = the legacy fallback viewmodel

    A pistol-less GLB written to a path named viewmodel.glb is REFUSED
    unless --force is given. Reason: load_assets() only falls back to the
    cube pistol when the GLB fails to PARSE. A GLB that parses fine but
    contains no gun parts leaves `assets.gun` empty, push_parts draws
    nothing, and the player ends up holding an invisible weapon with no
    warning logged anywhere. See online.rs load_assets/push_gun.

NOTHING HERE HAS BEEN RUN — there is no Blender on the authoring machine.
"""

import math
import os
import sys

import bpy

# make_hands.py is the sibling that owns the hand geometry. Blender sets
# __file__ for scripts run with --python, so derive the path from it
# rather than from cwd.
try:
    _HERE = os.path.dirname(os.path.abspath(__file__))
except NameError:  # pragma: no cover — Blender always sets __file__
    _HERE = os.path.abspath("tools")
if _HERE not in sys.path:
    sys.path.insert(0, _HERE)

try:
    import make_hands
except ImportError as e:
    # Never silently emit a handless viewmodel: the hands would just be
    # gone from the player's screen and nothing would say why.
    raise SystemExit(
        f"[assets] FATAL: cannot import make_hands from {_HERE!r} ({e}). "
        f"The hands half of the viewmodel lives there; refusing to emit a "
        f"viewmodel without it."
    )


def mat(name, rgb):
    return make_hands.material(name, rgb)


def box(name, loc, dim, material, rot=(0.0, 0.0, 0.0)):
    bpy.ops.mesh.primitive_cube_add(location=loc, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    if o.name != name:
        # Blender renames on collision ("strip" -> "strip.001") and the
        # runtime matches the name EXACTLY for the weapon-level recolor.
        raise SystemExit(f"[assets] FATAL: object renamed {name!r} -> {o.name!r}")
    o.scale = (dim[0] / 2.0, dim[1] / 2.0, dim[2] / 2.0)
    o.data.materials.append(material)
    return o


def build_pistol():
    """The legacy box pistol (origin = hand anchor, +X = muzzle direction).

    Kept verbatim from the pre-split script. This is the authored twin of
    the Rust cube fallback in online.rs; do not "improve" it without
    changing push_gun to match, or the two fallbacks diverge.
    """
    gunmetal = mat("gunmetal", (0.16, 0.17, 0.20))
    dark = mat("dark", (0.10, 0.10, 0.12))
    bronze = mat("bronze", (0.46, 0.32, 0.21))
    glow = mat("glow", (0.20, 0.65, 1.0))

    return [
        box("slide", (0.34, 0.0, 0.10), (0.72, 0.13, 0.13), gunmetal),
        box("barrel", (0.74, 0.0, 0.085), (0.16, 0.10, 0.10), bronze),
        box("frame", (0.30, 0.0, 0.005), (0.58, 0.11, 0.08), dark),
        box(
            "grip",
            (0.02, 0.0, -0.14),
            (0.15, 0.11, 0.28),
            dark,
            # Rake shared with the hands rather than hardcoded. This box
            # pistol and make_hands' fingers land in the SAME GLB, and they
            # used to disagree by 28 degrees (-14 here against +14 there),
            # which is a hand visibly not wrapping the grip it holds. One
            # number, one source of truth, whatever the 9mm fit pass sets.
            rot=(0.0, math.radians(make_hands.GRIP_ANGLE_DEG), 0.0),
        ),
        box("guard", (0.16, 0.0, -0.075), (0.17, 0.03, 0.03), bronze),
        box("sight_f", (0.64, 0.0, 0.185), (0.03, 0.03, 0.045), gunmetal),
        box("sight_r", (0.05, 0.0, 0.185), (0.05, 0.05, 0.035), gunmetal),
        # "strip" is the one name the runtime special-cases: it gets
        # recolored per weapon level.
        box("strip", (0.32, 0.0, 0.046), (0.50, 0.145, 0.02), glow),
    ]


def main():
    out, flags = make_hands.parse_args(sys.argv)
    if out is None:
        out = os.path.join(
            os.path.dirname(_HERE), "crates", "pong", "assets", "viewmodel.glb"
        )
        print(f"[assets] no output path given, defaulting to {out}")
    with_pistol = "--with-pistol" in flags

    print(f"[assets] blender {bpy.app.version_string}")
    print(f"[assets] output  {os.path.abspath(out)}")
    print(
        f"[assets] mode    {'pistol + hands (fallback viewmodel)' if with_pistol else 'hands only'}"
    )

    if not with_pistol and os.path.basename(out) == "viewmodel.glb":
        if "--force" not in flags:
            raise SystemExit(
                "[assets] REFUSING to write a pistol-less GLB to "
                f"{out!r}.\n"
                "[assets]   online.rs embeds this exact filename and only "
                "falls back to the cube pistol when the GLB fails to PARSE. "
                "A GLB with no gun parts parses fine and draws NO WEAPON AT "
                "ALL, silently.\n"
                "[assets]   Use --with-pistol for the fallback viewmodel, "
                "give a different output path for hands-only, or pass "
                "--force if you have already taught the Rust side to load "
                "the gun from somewhere else."
            )
        print("[assets] WARN: --force given; writing a viewmodel with NO GUN")

    bpy.ops.wm.read_factory_settings(use_empty=True)

    objs = []
    if with_pistol:
        objs += build_pistol()
    # Hands are delegated, never duplicated: one source of truth for the
    # grip fit (make_hands.GRIP_ANCHOR and friends).
    objs += make_hands.build_hands()

    # With the pistol present, non-arm parts are expected (they ARE the
    # gun), so the arm/hand name check only binds in hands-only mode.
    _data, problems = make_hands.report(objs, expect_all_arms=not with_pistol)

    # The classifier split the engine will make, computed here so a first
    # run says out loud what the runtime is going to see.
    gun = [o.name for o in objs if not (o.name.startswith("arm") or o.name.startswith("hand"))]
    arms = [o.name for o in objs if o.name.startswith("arm") or o.name.startswith("hand")]
    print(f"[assets] engine will classify {len(gun)} GUN part(s): {gun}")
    print(f"[assets] engine will classify {len(arms)} ARM part(s): {arms}")
    if with_pistol and "strip" not in gun:
        print("[assets] !!! no part named 'strip' — weapon-level recolor is dead")
    if not gun:
        print(
            "[assets] !!! ZERO gun parts in this GLB. If it is ever the file "
            "online.rs embeds, load_assets() will succeed, assets.gun will be "
            "empty, push_parts will draw nothing, and the player holds an "
            "INVISIBLE weapon — the cube fallback only fires on a parse error."
        )

    out = make_hands.export_glb(objs, out)
    if "--no-verify" not in flags:
        make_hands.verify_roundtrip(out, [o.name for o in objs])

    if problems:
        raise SystemExit(
            f"[assets] FINISHED WITH {len(problems)} PROBLEM(S) — see above. "
            f"The GLB was still written to {out}."
        )
    print("[assets] done")


if __name__ == "__main__":
    main()
