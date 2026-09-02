#!/usr/bin/env python3
"""Downscale the revolver's albedo pictures to the sizes the viewmodel ships.

    C:\\hy3d\\venv\\Scripts\\python.exe tools/v15/prep_pictures.py

Done here with PIL rather than inside Blender because Blender's Image.scale()
silently left the 4096² JPEGs untouched and the glTF exporter then shipped
the original JPEG bytes: a 9 MB viewmodel with 4096² pictures on a renderer
that clones one texture per mesh. The build script loads THESE PNGs, packs
them and exports them as they are.

Sizes are a VRAM and bundle budget, not a quality preference: the gun fills
the lower right of the screen, so its two pictures get 1024²; the hand's
skin picture is procedural and 512² is plenty.
"""
import os

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
SRC = os.path.join(REPO, "assets", "revolver", "source", "model", "textures")
OUT = os.path.join(REPO, "assets", "revolver", "baked")
SIZES = {"M1": 1024, "M2": 1024}

HAND_SRC = os.path.join(REPO, "assets", "hands", "rigged", "textures", "hand-lp_Material.001_BaseColor.png")
HAND_OUT = os.path.join(REPO, "assets", "hands", "rigged", "baked", "hand.png")

os.makedirs(os.path.dirname(HAND_OUT), exist_ok=True)
hand = Image.open(HAND_SRC).convert("RGB").resize((512, 512), Image.LANCZOS)
hand.save(HAND_OUT, format="PNG", optimize=True)
print(f"hand.png 512x512 {os.path.getsize(HAND_OUT) // 1024} KB")

os.makedirs(OUT, exist_ok=True)
for key, size in SIZES.items():
    im = Image.open(os.path.join(SRC, f"{key}_albedo.jpg")).convert("RGB")
    im = im.resize((size, size), Image.LANCZOS)
    path = os.path.join(OUT, f"{key}.png")
    im.save(path, format="PNG", optimize=True)
    back = Image.open(path)
    assert back.mode == "RGB" and back.size == (size, size)
    print(f"{key}.png {size}x{size} {os.path.getsize(path) // 1024} KB")
