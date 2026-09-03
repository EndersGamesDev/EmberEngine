#!/usr/bin/env python3
"""The loot block's picture, drawn rather than generated.

    C:\\hy3d\\venv\\Scripts\\python.exe tools/v18/loot_texture.py

One 512x512 8-bit RGB tile, `assets/textures/v18/loot.png`, that goes on all
six faces of the block (`tiled_box` at the block's nominal size): a riveted
brass plate with a bevelled rim and a bold question mark, the way the block
everyone already knows reads from across a room. Drawn with PIL so it is
deterministic, free and regenerable; a generated picture would cost a
request and could not be reproduced byte for byte.

The engine multiplies the per-instance colour in, so a USED block is this
same picture tinted down by the client, not a second picture. Mipmaps are
built at upload, so the tile is baked at the size it needs at arm's length
and left to the chain at distance.
"""
import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
OUT = os.path.join(REPO, "assets", "textures", "v18", "loot.png")
SIZE = 512

BRASS = (214, 158, 46)
BRASS_LIGHT = (240, 196, 88)
BRASS_DARK = (128, 88, 22)
RIM_DARK = (86, 58, 14)
INK = (52, 34, 10)
PAPER = (250, 240, 214)


def font(size):
    for name in ("arialbd.ttf", "arial.ttf", "DejaVuSans-Bold.ttf"):
        for folder in (os.path.join(os.environ.get("WINDIR", r"C:\Windows"), "Fonts"), "/usr/share/fonts/truetype/dejavu"):
            path = os.path.join(folder, name)
            if os.path.isfile(path):
                return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def main():
    im = Image.new("RGB", (SIZE, SIZE), BRASS)
    d = ImageDraw.Draw(im)
    # A faint brushed grain, so the flat fill does not read as plastic.
    for y in range(0, SIZE, 3):
        shade = 6 if (y // 3) % 2 == 0 else -6
        d.line([(0, y), (SIZE, y)], fill=tuple(max(0, min(255, c + shade)) for c in BRASS))
    # Bevelled rim: light on the top/left, dark on the bottom/right, and a
    # dark outer edge that becomes the seam between faces when tiled.
    rim = 34
    for i in range(rim):
        t = i / rim
        light = tuple(int(BRASS_LIGHT[k] * (1 - t) + BRASS[k] * t) for k in range(3))
        dark = tuple(int(BRASS_DARK[k] * (1 - t) + BRASS[k] * t) for k in range(3))
        d.line([(i, i), (SIZE - 1 - i, i)], fill=light)
        d.line([(i, i), (i, SIZE - 1 - i)], fill=light)
        d.line([(i, SIZE - 1 - i), (SIZE - 1 - i, SIZE - 1 - i)], fill=dark)
        d.line([(SIZE - 1 - i, i), (SIZE - 1 - i, SIZE - 1 - i)], fill=dark)
    d.rectangle([0, 0, SIZE - 1, SIZE - 1], outline=RIM_DARK, width=6)
    # Four rivets.
    for cx, cy in ((64, 64), (SIZE - 64, 64), (64, SIZE - 64), (SIZE - 64, SIZE - 64)):
        r = 17
        d.ellipse([cx - r, cy - r + 3, cx + r, cy + r + 3], fill=RIM_DARK)
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=BRASS_DARK)
        d.ellipse([cx - r + 5, cy - r + 5, cx + r - 7, cy + r - 7], fill=BRASS_LIGHT)
    # The question mark: a dark drop shadow, a dark outline, a pale fill.
    f = font(360)
    text = "?"
    box = d.textbbox((0, 0), text, font=f)
    w, h = box[2] - box[0], box[3] - box[1]
    x, y = (SIZE - w) // 2 - box[0], (SIZE - h) // 2 - box[1] - 8
    shadow = Image.new("RGB", (SIZE, SIZE), BRASS)
    ImageDraw.Draw(shadow).text((x + 10, y + 12), text, font=f, fill=BRASS_DARK)
    shadow = shadow.filter(ImageFilter.GaussianBlur(6))
    mask = Image.new("L", (SIZE, SIZE), 0)
    ImageDraw.Draw(mask).text((x + 10, y + 12), text, font=f, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(6))
    im.paste(shadow, (0, 0), mask)
    d = ImageDraw.Draw(im)
    d.text((x, y), text, font=f, fill=INK, stroke_width=14, stroke_fill=INK)
    d.text((x, y), text, font=f, fill=PAPER)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    im.save(OUT, format="PNG", optimize=True)
    back = Image.open(OUT)
    assert back.mode == "RGB" and back.size == (SIZE, SIZE), (back.mode, back.size)
    print(f"wrote {OUT}: {back.size} {back.mode}, {os.path.getsize(OUT) // 1024} KB")


if __name__ == "__main__":
    main()
