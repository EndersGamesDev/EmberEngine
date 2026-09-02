#!/usr/bin/env python3
"""Bake the v17 pictures at the sizes they ship.

    C:\hy3d\venv\Scripts\python.exe tools/v17/prep_pictures.py

PIL, outside Blender, for the reason tools/v15 recorded: the exporter ships
the bytes it is handed. The scutum's picture is 4096 RGBA in the source and
the renderer wants RGB; the Murasama's is 2048. Both ship at 1024: one
mesh each, so one clone apiece in VRAM. The fist that rides the sword is
its own mesh and gets its own 1024 copy of the body picture, because the
renderer clones one texture per mesh and the hands' 2048 copy cannot be
shared.
"""
import os

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
JOBS = {
    os.path.join(REPO, "assets", "scutum", "baked", "scutum-1024.png"): (os.path.join(REPO, "assets", "scutum", "textures", "Scutum_Base_Color.png"), 1024),
    os.path.join(REPO, "assets", "murasama", "baked", "murasama-1024.png"): (os.path.join(REPO, "assets", "murasama", "textures", "Material.002_Base_Color.png"), 1024),
    os.path.join(REPO, "assets", "swat", "baked", "body-1024.png"): (os.path.join(REPO, "assets", "swat", "textures", "Body_Base_color.png"), 1024),
}

for path, (source, size) in JOBS.items():
    os.makedirs(os.path.dirname(path), exist_ok=True)
    im = Image.open(source)
    if im.mode == "RGBA":
        # The renderer cannot honour a mask (BlendState::REPLACE), so the
        # alpha is dropped. How much of the picture it covered is reported:
        # the scutum's alpha clears only the atlas padding outside its UV
        # islands (the board renders complete), and a picture that relied
        # on a cut-out would show up here as a large fraction.
        clear = sum(im.getchannel("A").histogram()[:255])
        print(f"  {os.path.basename(source)}: {100.0 * clear / (im.width * im.height):.1f}% of pixels carry alpha below 255; dropped")
    im = im.convert("RGB").resize((size, size), Image.LANCZOS)
    im.save(path, format="PNG", optimize=True)
    back = Image.open(path)
    assert back.mode == "RGB" and back.size == (size, size)
    print(f"{os.path.basename(path)} {size}x{size} {os.path.getsize(path) // 1024} KB")
