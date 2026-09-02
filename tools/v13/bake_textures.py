#!/usr/bin/env python3
"""Bake the v13 material pictures into the files the arena embeds.

Input: the raw generator output in ``assets/concepts/v13/textures/`` (1024²
per picture, whatever bit depth the generator felt like). Output: the atlases
and tiles in ``assets/textures/v13/``, every one 8-bit RGB, sized to the
bundle budget in ``docs/plans/arena-v13-trench-city.md`` §5.

Why the sizes are what they are: ``include_bytes!`` puts every byte of these
into the wasm bundle every web player downloads, and PNG compresses a photo
texture to roughly 1.3 bytes per pixel whatever you do. The whole set below
comes to ~8.9 MB (measured; the script prints the total when it finishes).
A 2048² container atlas alone would be ~5 MB, so the
container gets 1024² (512² per face), the things you stare at from arm's
length get 1024 on their long side, and the materials on the skyline props
40 m away get 256².

Atlas layouts (u right, v down, as the arena's ``atlas_box`` reads them):

    container.png 1024x1024   [ side  | doors ]      crate.png 1024x512  [ side | top ]
                              [ roof  | floor ]      ammo.png  1024x512  [ side | top ]

Runs on the Hunyuan venv's python, which has PIL:

    C:\\hy3d\\venv\\Scripts\\python.exe tools\\v13\\bake_textures.py

Refuses silently missing inputs: a missing raw picture is an error naming
it, never a placeholder — the engine decodes anything else than 8-bit
R8G8B8(A8) to an untextured mesh with no log line, and a wrong-size atlas
would ship a box with its doors on the roof.
"""

import os
import sys

from PIL import Image, ImageEnhance

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = os.path.join(HERE, "..", "..", "assets", "concepts", "v13", "textures")
OUT = os.path.join(HERE, "..", "..", "assets", "textures", "v13")

# Single-picture outputs: name -> (raw name, size). The raw sandbag picture
# is not baked: the sandbag lines are the generated mesh wearing burlap, and
# a tile nothing draws would still ride in every web player's bundle.
TILES = {
    "trench-wall": ("trench-wall", 512),
    "tunnel-roof": ("tunnel-roof", 512),
    "rubble": ("rubble", 512),
    "cobble": ("cobble", 1024),
    "city-wall": ("city-wall", 512),
    "plinth": ("granite", 256),
    "limestone": ("limestone", 256),
    "sandstone": ("sandstone", 256),
    "bronze": ("bronze", 256),
    "burlap": ("burlap", 256),
    "scorched-steel": ("scorched-steel", 256),
    "cast-iron": ("cast-iron", 256),
}


def load(name: str) -> Image.Image:
    path = os.path.join(RAW, f"{name}.png")
    if not os.path.exists(path):
        raise SystemExit(f"missing raw picture: {os.path.relpath(path)} — generate and fetch it first")
    return Image.open(path).convert("RGB")


def fit(img: Image.Image, w: int, h: int) -> Image.Image:
    """Resize with a high-quality filter; downscaling is the whole point."""
    return img.resize((w, h), Image.LANCZOS)


def trim(img: Image.Image, frac: float = 0.035) -> Image.Image:
    """Cut a thin margin off every edge.

    The generator draws a face picture with a sliver of studio background
    around it; on a box face that sliver becomes a dark rim on every edge.
    Trimming 3.5 percent removes the rim and keeps the brackets and straps.
    """
    w, h = img.size
    dx, dy = int(w * frac), int(h * frac)
    return img.crop((dx, dy, w - dx, h - dy))


def save(img: Image.Image, name: str) -> None:
    os.makedirs(OUT, exist_ok=True)
    path = os.path.join(OUT, f"{name}.png")
    img = img.convert("RGB")
    # Write beside, then rename: a cargo build reading include_bytes! at
    # the same moment gets the old file or the new one, never half of each.
    img.save(path + ".tmp", format="PNG", optimize=True)
    os.replace(path + ".tmp", path)
    # Re-open and prove what the engine will see: 8-bit RGB, intended size.
    back = Image.open(path)
    assert back.mode == "RGB", f"{name}: mode {back.mode}"
    assert back.size == img.size, f"{name}: size {back.size} != {img.size}"
    print(f"{name}.png {img.size[0]}x{img.size[1]} {os.path.getsize(path) // 1024} KB", flush=True)


def atlas(cells: list[list[Image.Image]], cell: int) -> Image.Image:
    rows = len(cells)
    cols = len(cells[0])
    out = Image.new("RGB", (cols * cell, rows * cell))
    for r, row in enumerate(cells):
        for c, img in enumerate(row):
            out.paste(fit(img, cell, cell), (c * cell, r * cell))
    return out


def main() -> int:
    total = 0
    # Container: side | doors over roof | floor. The floor is the roof
    # picture darkened — nobody sees a container's underside, but the atlas
    # needs a face there.
    side, doors, roof = load("container-side"), load("container-doors"), load("container-roof")
    floor = ImageEnhance.Brightness(roof).enhance(0.45)
    save(atlas([[side, doors], [roof, floor]], 512), "container")
    save(atlas([[trim(load("crate-side")), trim(load("crate-top"))]], 512), "crate")
    # The ammo pictures carry a wider sliver of studio above the lid.
    save(atlas([[trim(load("ammo-side"), 0.08), trim(load("ammo-top"), 0.08)]], 512), "ammo")
    for name, (raw, size) in TILES.items():
        img = load(raw)
        w, h = img.size
        # Three pictures the generator would not draw flat however it was
        # asked; the usable patch is cut out and stretched to the tile.
        if name == "cobble":
            # Kerbs at both edges and the strongest convergence at the top.
            img = img.crop((int(w * 0.18), int(h * 0.25), int(w * 0.82), int(h * 0.95)))
        elif name == "city-wall":
            # A strip of pavement under the wall.
            img = img.crop((0, 0, w, int(h * 0.92)))
        elif name == "tunnel-roof":
            # The vault's far end; keep the beams and sheets nearest the eye.
            img = img.crop((int(w * 0.1), 0, int(w * 0.9), int(h * 0.55)))
        save(fit(img, size, size), name)
    # The sky strip keeps its aspect: 2048x512 raw -> 2048x512.
    sky = load("sky")
    save(fit(sky, 2048, 512), "sky")
    for f in sorted(os.listdir(OUT)):
        total += os.path.getsize(os.path.join(OUT, f))
    print(f"total {total / 1_048_576:.2f} MB in {os.path.relpath(OUT)}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
