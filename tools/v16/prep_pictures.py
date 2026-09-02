#!/usr/bin/env python3
"""Bake the operator's pictures at the sizes the v16 viewmodel ships.

    C:\hy3d\venv\Scripts\python.exe tools/v16/prep_pictures.py

Done with PIL outside Blender for the reason recorded in tools/v15: what
Blender's Image.scale() does to a loaded picture is not what the glTF
exporter ships, and the only way to be sure of the bytes in the bundle is
to hand it a file already at size and let it embed that.

The body picture is 4096² in the source and the operators' parts ship it
at 1024; the first-person hands are the closest thing on screen and get
2048 - one mesh, so one clone in VRAM. The rifle's 2048 source ships at
1024, like the revolver did.
"""
import os

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
SRC = os.path.join(REPO, "assets", "swat", "textures")
OUT = os.path.join(REPO, "assets", "swat", "baked")
JOBS = {"body-2048.png": ("Body_Base_color.png", 2048), "ksvr-1024.png": ("KSVR_Base_color.png", 1024)}

os.makedirs(OUT, exist_ok=True)
for name, (source, size) in JOBS.items():
    im = Image.open(os.path.join(SRC, source)).convert("RGB").resize((size, size), Image.LANCZOS)
    path = os.path.join(OUT, name)
    im.save(path, format="PNG", optimize=True)
    back = Image.open(path)
    assert back.mode == "RGB" and back.size == (size, size)
    print(f"{name} {size}x{size} {os.path.getsize(path) // 1024} KB")
