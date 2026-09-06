"""Look at a generated mesh without a viewer: extents, counts, colour, and
three orthographic point-splat pictures (front/side/top) with the vertex
colours, so the bake's --forward/--up can be chosen from the picture.

  C:/hy3d/venv/Scripts/python.exe tools/league/art/inspect_mesh.py in.glb out_prefix

Writes out_prefix-xy.png (looking along -Z: X right, Y up),
out_prefix-zy.png (looking along -X: Z right, Y up) and out_prefix-xz.png
(looking down -Y: X right, Z down). Whatever axis the finial or head is on
is "up"; whatever face carries the front detail is "forward".
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    import trimesh
    from PIL import Image

    src = Path(sys.argv[1])
    prefix = sys.argv[2]
    scene = trimesh.load(str(src), force="scene")
    meshes = [g for g in scene.geometry.values() if isinstance(g, trimesh.Trimesh)]
    print(f"{src.name}: {len(meshes)} mesh(es), {src.stat().st_size / 1024:.0f} KB")
    for name, g in scene.geometry.items():
        if not isinstance(g, trimesh.Trimesh):
            continue
        v = np.asarray(g.vertices)
        lo, hi = v.min(axis=0), v.max(axis=0)
        kind = getattr(g.visual, "kind", None)
        has_uv = kind == "texture" and getattr(g.visual, "uv", None) is not None
        cols = None
        if kind == "vertex":
            cols = np.asarray(g.visual.vertex_colors)[:, :3]
        elif kind == "texture":
            try:
                cols = np.asarray(g.visual.to_color().vertex_colors)[:, :3]
            except Exception:  # noqa: BLE001
                cols = None
        print(f"  {name}: {len(g.faces)} faces, {len(v)} verts, visual={kind}, uv={has_uv}, "
              f"colour={'yes' if cols is not None and cols.std() > 1 else 'no'}")
        print(f"    x {lo[0]:+.3f}..{hi[0]:+.3f}  y {lo[1]:+.3f}..{hi[1]:+.3f}  z {lo[2]:+.3f}..{hi[2]:+.3f}"
              f"  (spans {hi[0]-lo[0]:.3f} {hi[1]-lo[1]:.3f} {hi[2]-lo[2]:.3f})")
        if cols is None:
            cols = np.full((len(v), 3), 180, dtype=np.uint8)
        size = 512
        span = float((hi - lo).max()) or 1.0
        centre = (lo + hi) / 2.0
        norm = (v - centre) / span * (size * 0.9) + size / 2.0
        views = {
            "xy": (0, 1, 2, -1.0),  # X right, Y up, sort by -Z (front = larger Z first drawn last)
            "zy": (2, 1, 0, 1.0),   # Z right, Y up, sort by X
            "xz": (0, 2, 1, 1.0),   # X right, Z down, sort by Y
        }
        for tag, (ax, ay, depth, sign) in views.items():
            img = np.full((size, size, 3), 40, dtype=np.uint8)
            order = np.argsort(sign * v[:, depth])
            xs = np.clip(norm[order, ax].astype(int), 0, size - 1)
            ys = np.clip(size - 1 - norm[order, ay].astype(int) if tag != "xz" else norm[order, ay].astype(int), 0, size - 1)
            c = cols[order]
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    img[np.clip(ys + dy, 0, size - 1), np.clip(xs + dx, 0, size - 1)] = c
            Image.fromarray(img, "RGB").save(f"{prefix}-{tag}.png")
            print(f"    wrote {prefix}-{tag}.png")
    return 0


if __name__ == "__main__":
    sys.exit(main())
