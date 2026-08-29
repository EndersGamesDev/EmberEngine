"""Generate the viewmodel assets (pistol + hands/arms) as a GLB.

Run headless:
    blender --background --python tools/make_assets.py -- crates/pong/assets/viewmodel.glb

Conventions (must match the engine): +X forward, +Z up in Blender; the
default Y-up glTF export maps this to +X forward / +Y up in the engine.
Units are game units (the pistol is ~0.9 long). Node names matter:
parts starting with "arm"/"hand" are viewmodel-only (not drawn on remote
players); the part named "strip" gets recolored by weapon level.
"""

import math
import sys

import bpy

OUT = sys.argv[sys.argv.index("--") + 1] if "--" in sys.argv else "viewmodel.glb"

bpy.ops.wm.read_factory_settings(use_empty=True)


def mat(name, rgb):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    return m


GUNMETAL = mat("gunmetal", (0.16, 0.17, 0.20))
DARK = mat("dark", (0.10, 0.10, 0.12))
BRONZE = mat("bronze", (0.46, 0.32, 0.21))
GLOW = mat("glow", (0.20, 0.65, 1.0))
GLOVE = mat("glove", (0.15, 0.12, 0.11))
SLEEVE = mat("sleeve", (0.20, 0.24, 0.30))


def box(name, loc, dim, material, rot=(0.0, 0.0, 0.0)):
    bpy.ops.mesh.primitive_cube_add(location=loc, rotation=rot)
    o = bpy.context.active_object
    o.name = name
    o.scale = (dim[0] / 2.0, dim[1] / 2.0, dim[2] / 2.0)
    o.data.materials.append(material)
    return o


# ---- the pistol (origin = hand anchor, +X = muzzle direction) ----
box("slide", (0.34, 0.0, 0.10), (0.72, 0.13, 0.13), GUNMETAL)
box("barrel", (0.74, 0.0, 0.085), (0.16, 0.10, 0.10), BRONZE)
box("frame", (0.30, 0.0, 0.005), (0.58, 0.11, 0.08), DARK)
box("grip", (0.02, 0.0, -0.14), (0.15, 0.11, 0.28), DARK, rot=(0.0, math.radians(-14), 0.0))
box("guard", (0.16, 0.0, -0.075), (0.17, 0.03, 0.03), BRONZE)
box("sight_f", (0.64, 0.0, 0.185), (0.03, 0.03, 0.045), GUNMETAL)
box("sight_r", (0.05, 0.0, 0.185), (0.05, 0.05, 0.035), GUNMETAL)
box("strip", (0.32, 0.0, 0.046), (0.50, 0.145, 0.02), GLOW)

# ---- hands + forearms (viewmodel only) ----
box("hand_r", (0.02, -0.005, -0.13), (0.17, 0.16, 0.19), GLOVE)
box("arm_r", (-0.30, 0.12, -0.34), (0.60, 0.12, 0.12), SLEEVE,
    rot=(0.0, math.radians(28), math.radians(-14)))
box("hand_l", (0.17, 0.06, -0.17), (0.14, 0.13, 0.14), GLOVE)
box("arm_l", (-0.16, 0.34, -0.40), (0.55, 0.11, 0.11), SLEEVE,
    rot=(math.radians(-16), math.radians(26), math.radians(28)))

bpy.ops.export_scene.gltf(filepath=OUT, export_format="GLB", export_yup=True)
print(f"wrote {OUT}")
