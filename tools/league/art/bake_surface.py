"""Turn a generated ground picture into a tiling surface the engine can draw.

The references are 1024-square pictures whose edges do not meet, so a
plain repeat shows a grid of seams. Mirroring the picture into a 2x2 block
makes every edge meet its own reflection, which tiles without a seam at
the cost of a visible symmetry at twice the tile size; for paving, ferns
and flagstones seen from 24 units up that reads as pattern, not as a bug.

The surface ships as a GLB quad (1x1 in x/z at y=0, normal +Y) whose UVs
already run 0..tiles, with the mirrored 8-bit RGB picture embedded, so
`ember_engine::assets::load_glb` hands the scene a textured `MeshData`
with no new decoder; the scene scales the quad to the field it covers
and the sampler repeats. Pick `--tiles` so one mirrored block spans the
number of world units you want: a 160-unit field with 8-unit ferns is
--tiles 20.

  C:/hy3d/venv/Scripts/python.exe tools/league/art/bake_surface.py ^
      --input target/league-art/garden/reference.png --name garden ^
      --out assets/models/league/v2 --size 1024 --tiles-x 20 --tiles-z 11.5
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

import numpy as np


def mirrored(img, size: int):
    """2x2 mirror block, downscaled so the whole block is `size` square."""
    from PIL import Image, ImageOps

    img = img.convert("RGB")
    half = size // 2
    tile = img.resize((half, half), Image.LANCZOS)
    block = Image.new("RGB", (size, size))
    block.paste(tile, (0, 0))
    block.paste(ImageOps.mirror(tile), (half, 0))
    block.paste(ImageOps.flip(tile), (0, half))
    block.paste(ImageOps.flip(ImageOps.mirror(tile)), (half, half))
    return block


def quad_glb(name: str, img, tiles_x: float, tiles_z: float, out: Path, brightness: float) -> int:
    import trimesh
    from PIL import Image

    # a unit quad in x/z, +Y normal, wound counter-clockwise seen from above
    v = np.array([[-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [0.5, 0.0, 0.5], [-0.5, 0.0, 0.5]], dtype=np.float64)
    f = np.array([[0, 2, 1], [0, 3, 2]], dtype=np.int64)
    uv = np.array([[0.0, 0.0], [tiles_x, 0.0], [tiles_x, tiles_z], [0.0, tiles_z]], dtype=np.float32)
    if abs(brightness - 1.0) > 1e-6:
        arr = np.asarray(img, dtype=np.float32) * brightness
        img = Image.fromarray(np.clip(arr, 0, 255).astype(np.uint8), "RGB")
    material = trimesh.visual.material.PBRMaterial(
        baseColorTexture=img,
        baseColorFactor=[255, 255, 255, 255],
        metallicFactor=0.0,
        roughnessFactor=1.0,
        name=f"{name}-albedo",
    )
    mesh = trimesh.Trimesh(vertices=v, faces=f, process=False)
    mesh.visual = trimesh.visual.TextureVisuals(uv=uv, material=material)
    scene = trimesh.Scene()
    scene.add_geometry(mesh, node_name=name, geom_name=name)
    _ = mesh.vertex_normals
    data = scene.export(file_type="glb", include_normals=True)
    out.write_bytes(data)
    return len(data)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--input", required=True, type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--size", type=int, default=1024, help="square size of the shipped mirrored block")
    ap.add_argument("--tiles-x", type=float, default=1.0, help="mirrored blocks across the quad's x")
    ap.add_argument("--tiles-z", type=float, default=1.0, help="mirrored blocks across the quad's z")
    ap.add_argument("--brightness", type=float, default=1.0)
    args = ap.parse_args()
    from PIL import Image

    t0 = time.perf_counter()
    src = Image.open(args.input)
    print(f"source {args.input.name}: {src.size} {src.mode}")
    block = mirrored(src, args.size)
    assert block.mode == "RGB" and block.size == (args.size, args.size), (block.mode, block.size)
    args.out.mkdir(parents=True, exist_ok=True)
    glb = args.out / f"surface-{args.name}.glb"
    # the picture goes straight into the GLB; a preview for humans lands in
    # target/league-art, never beside the asset (git ships the GLB only)
    preview = Path("target/league-art") / args.name / f"surface-{args.name}.png"
    if preview.parent.is_dir():
        block.save(preview, optimize=True)
    nbytes = quad_glb(args.name, block, args.tiles_x, args.tiles_z, glb, args.brightness)
    print(f"shipped {glb.name}: {nbytes / 1024:.0f} KB, block {args.size}x{args.size} RGB8, uv 0..{args.tiles_x} x 0..{args.tiles_z}, {time.perf_counter() - t0:.1f} s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
