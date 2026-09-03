#!/usr/bin/env python3
"""Bake the v18 weapon pictures at the sizes they ship, or at the sizes the
atlas bake reads.

    C:\\hy3d\\venv\\Scripts\\python.exe tools/v18/prep_pictures.py

PIL, outside Blender, for the reason tools/v15 recorded: the glTF exporter
ships the bytes it is handed, and Blender's own Image.scale() once left a
set of 4096 pictures untouched. Every output is re-opened to assert the
mode and size the engine will see, because the decoder returns None for
anything but 8-bit RGB/RGBA with no log line.

Two kinds of output:

  * shipping pictures, one per single-material part: the AK, the RPG
    launcher, the rocket, and the revolver's two pictures at 512 for its
    four small parts (its frame keeps the 1024 M2.png that v15 baked);
  * bake INPUTS for the two multi-material weapons (Vityaz, sniper): each
    material's base colour at 1024, so the Cycles atlas bake in
    build_weapons.py never touches a 4096 source. The Vityaz's stock is
    deleted by the build, so its picture is not prepared.

A source with an alpha channel is dropped to RGB and the fraction of pixels
it covered is reported (the v17 lesson): the renderer cannot honour a mask,
so a picture that relied on a cut-out shows up here as a large number.
"""
import os
import time

from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
A = os.path.join(REPO, "assets")

# output path -> (source path, size)
JOBS = {
    os.path.join(A, "ak47", "baked", "ak47-1024.png"): (os.path.join(A, "ak47", "source", "textures", "AK_Base_color.png"), 1024),
    os.path.join(A, "rpg7", "baked", "rpg7-1024.png"): (os.path.join(A, "rpg7", "source", "RPG7", "textures", "RPG7_Albedo.png"), 1024),
    os.path.join(A, "rpg7", "baked", "rocket-512.png"): (os.path.join(A, "rpg7", "source", "RPG7", "textures", "RPG7Rocket_Albedo.png"), 512),
    os.path.join(A, "revolver", "baked", "M1-512.png"): (os.path.join(A, "revolver", "baked", "M1.png"), 512),
    os.path.join(A, "revolver", "baked", "M2-512.png"): (os.path.join(A, "revolver", "baked", "M2.png"), 512),
}
# The Vityaz's material names carry spaces in the FBX ("hand guard"); the
# file names use underscores, and build_weapons.py maps one to the other.
for mat in ("Body", "grip", "hand_guard", "holo_sight", "magazine", "rail", "side_rail"):
    JOBS[os.path.join(A, "vityaz", "baked", f"{mat}-1024.png")] = (os.path.join(A, "vityaz", "source", "textures", f"space_pp19_texture_{mat}_BaseColor.png"), 1024)
for mat in ("front", "middle", "back", "SCOPE", "MAG"):
    JOBS[os.path.join(A, "sniper", "baked", f"{mat}-1024.png")] = (os.path.join(A, "sniper", "source", "textures", f"texturing_{mat}_BaseColor.png"), 1024)


def main():
    t_all = time.time()
    total = 0
    for path, (source, size) in JOBS.items():
        t0 = time.time()
        if not os.path.isfile(source):
            raise SystemExit(f"missing {source}: unpack the weapon archives under assets/ first")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        im = Image.open(source)
        note = ""
        if im.mode == "RGBA":
            clear = sum(im.getchannel("A").histogram()[:255])
            note = f"; {100.0 * clear / (im.width * im.height):.1f}% of pixels carried alpha below 255, dropped"
        im = im.convert("RGB")
        if im.size != (size, size):
            im = im.resize((size, size), Image.LANCZOS)
        im.save(path, format="PNG", optimize=True)
        back = Image.open(path)
        if back.mode != "RGB" or back.size != (size, size):
            raise SystemExit(f"{path} re-opened as {back.mode} {back.size}, expected RGB {size}x{size}")
        kb = os.path.getsize(path) // 1024
        total += kb
        print(f"{os.path.relpath(path, REPO)} {size}x{size} {kb} KB from {os.path.basename(source)} {time.time() - t0:.1f} s{note}")
    print(f"{len(JOBS)} pictures, {total} KB, {time.time() - t_all:.1f} s")


if __name__ == "__main__":
    main()
