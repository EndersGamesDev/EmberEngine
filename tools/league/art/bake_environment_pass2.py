"""Second V4 ground pass: the same reviewed SDXL pictures, re-baked so the
lane stops mirroring into large figures and the garden stops reading as a
flat olive plane. No new generator jobs; the source bytes are checked
against the durable job records exactly as `bake_environment.py` does.

    C:/hy3d/venv/Scripts/python.exe tools/league/art/bake_environment_pass2.py \
        --jobs C:/Users/end/dev/ember-league-environment/target/league-launch/assets \
        --out  <dir for the GLBs>  --art <dir for the previews>  [--lane mirror|seamless]

Lane:   `mirror`   keeps the 2x2 mirror but tiles 24 x 2.4 (one block per
                   5.8 units instead of 10) so the mirrored motif is smaller
                   than a champion.
        `seamless` drops the mirror: the picture is offset by half in both
                   axes and the cross seam feathered, giving a plain repeat
                   with no symmetry; tiled 20 x 2.
Garden: 768 block (was 1024) mirrored, darkened to 0.78, tinted toward
        jade, and multiplied by low-frequency mottling so the ground has
        mid-scale variation the pictures lacked. Smaller block = fewer
        embedded bytes; the garden is seen at distance and mipmapped.
Court:  untouched (copied through, same bytes).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bake_surface import mirrored, quad_glb  # noqa: E402

ROOT = Path(__file__).resolve().parents[3]


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_job(jobs: Path, name: str) -> tuple[dict, Image.Image]:
    folder = jobs / name
    job = json.loads((folder / "job.json").read_text(encoding="utf-8"))
    raw = (folder / (name + ".png")).read_bytes()
    assert job["sha256"] == sha(raw), f"{name}: generator bytes differ from the durable job record"
    return job, Image.open(folder / (name + ".png")).convert("RGB")


def seamless(img: Image.Image, size: int, feather: float = 0.18) -> Image.Image:
    """Offset by half in x and y (wrapping), then hide the resulting cross
    seam by blending the original back in with a feathered cross mask. The
    outer edges then match by construction and nothing is mirrored."""
    src = img.resize((size, size), Image.Resampling.LANCZOS)
    a = np.asarray(src, dtype=np.float32)
    half = size // 2
    shifted = np.roll(np.roll(a, half, axis=0), half, axis=1)
    # cross mask: 1 near the centre lines (the seam), 0 elsewhere
    coords = (np.arange(size, dtype=np.float32) - half) / size
    band = np.clip(1.0 - np.abs(coords) / feather, 0.0, 1.0)
    mask = np.maximum(band[:, None], band[None, :])
    mask = np.asarray(Image.fromarray((mask * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(size * 0.02)), dtype=np.float32) / 255.0
    out = shifted * (1.0 - mask[..., None]) + a * mask[..., None]
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), "RGB")


def mottle(img: Image.Image, seed: int, cells: int = 7, depth: float = 0.30) -> Image.Image:
    """Multiply by smooth, PERIODIC low-frequency noise so a near-uniform
    picture gets patches the size of a few champions. The noise is built on
    a torus (the random grid tiled 3x3, upsampled, centre cropped), so it
    tiles without a seam and is applied AFTER mirroring: the mottling itself
    carries no mirror symmetry, which is what the eye would otherwise read
    at block scale."""
    rng = np.random.default_rng(seed)
    size = img.size[0]
    small = rng.random((cells, cells), dtype=np.float32)
    tiled = np.tile(small, (3, 3))
    big = Image.fromarray((tiled * 255).astype(np.uint8)).resize((size * 3, size * 3), Image.Resampling.BICUBIC)
    noise = np.asarray(big, dtype=np.float32)[size:2 * size, size:2 * size] / 255.0
    noise = (noise - noise.min()) / max(noise.max() - noise.min(), 1e-6)
    gain = (1.0 - depth * 0.5) + depth * noise
    a = np.asarray(img, dtype=np.float32) * gain[..., None]
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), "RGB")


def tint(img: Image.Image, rgb: tuple[float, float, float], brightness: float) -> Image.Image:
    a = np.asarray(img, dtype=np.float32) * np.array(rgb, dtype=np.float32) * brightness
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), "RGB")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--jobs", required=True, type=Path)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--art", required=True, type=Path)
    ap.add_argument("--lane", choices=["mirror", "seamless", "inpainted"], default="inpainted")
    ap.add_argument("--inpainted", type=Path, help="directory holding lane-seamless-tile.png and job.json from the SDXL seam inpaint")
    ap.add_argument("--lane-size", type=int, default=1024)
    ap.add_argument("--lane-tiles", type=float, nargs=2, default=(26.0, 2.6))
    ap.add_argument("--garden-size", type=int, default=768)
    ap.add_argument("--garden-brightness", type=float, default=0.78)
    ap.add_argument("--court-glb", type=Path, default=ROOT / "assets/models/league/v4/surface-court.glb")
    ap.add_argument("--manifest", type=Path, help="root's manifest.json to carry forward (court row and rejected list)")
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    args.art.mkdir(parents=True, exist_ok=True)
    rows = []

    job, paving = load_job(args.jobs, "v4-paving")
    tx, tz = args.lane_tiles
    if args.lane == "mirror":
        lane_tex, mode = mirrored(paving, 768), {"mirror2x2": True}
    elif args.lane == "seamless":
        lane_tex, mode = seamless(paving, 768), {"mirror2x2": False, "seamless": "half-offset, feathered cross seam 0.18"}
    else:
        assert args.inpainted, "--inpainted <dir> is required for the inpainted lane"
        seam = json.loads((args.inpainted / "job.json").read_text(encoding="utf-8"))
        assert seam["sourceSha256"] == job["sha256"], "inpaint job was not run on the accepted paving picture"
        tile = Image.open(args.inpainted / "lane-seamless-tile.png").convert("RGB")
        lane_tex = tint(tile.resize((args.lane_size, args.lane_size), Image.Resampling.LANCZOS), (1.04, 1.0, 0.94), 1.0)
        mode = {"mirror2x2": False, "seamless": "half-offset, cross seam regenerated by SDXL inpaint (SetLatentNoiseMask, denoise 0.82, seed 6090742), rolled back",
                "inpaintJob": seam["job"], "inpaintOutputSha256": seam["outputSha256"], "tint": [1.04, 1.0, 0.94]}
    lane_glb = args.out / "surface-lane.glb"
    n = quad_glb("lane", lane_tex, tx, tz, lane_glb, 1.0)
    lane_tex.save(args.art / "lane.webp", "WEBP", quality=90, method=6)
    lane_tex.save(args.art / "lane-block.png")
    rows.append({"name": "lane", "job": job["id"], "generator": "LAN SDXL / asset-forge", "sourceSha256": job["sha256"], "spec": job["spec"], "review": "accepted (pass 2: seamless re-tile)",
                 "bake": {"size": lane_tex.size[0], "mode": "RGB8", **mode, "uvTiles": [tx, tz], "brightness": 1.0},
                 "glb": "assets/models/league/v4/surface-lane.glb", "glbBytes": n, "glbSha256": sha(lane_glb.read_bytes()),
                 "preview": "web/games/league/v4/art/lane.webp", "previewSha256": sha((args.art / "lane.webp").read_bytes())})

    job, moss = load_job(args.jobs, "v4-moss-3")
    garden = tint(moss, (0.86, 1.0, 0.90), args.garden_brightness)
    garden_tex = mottle(mirrored(garden, args.garden_size), seed=6090745)
    garden_glb = args.out / "surface-garden.glb"
    n = quad_glb("garden", garden_tex, 10.0, 5.75, garden_glb, 1.0)
    garden_tex.save(args.art / "garden.webp", "WEBP", quality=90, method=6)
    garden_tex.save(args.art / "garden-block.png")
    rows.append({"name": "garden", "job": job["id"], "generator": "LAN SDXL / asset-forge", "sourceSha256": job["sha256"], "spec": job["spec"], "review": "accepted (pass 2: darker, jade tint, periodic mottling after the mirror)",
                 "bake": {"size": args.garden_size, "mode": "RGB8", "mirror2x2": True, "uvTiles": [10.0, 5.75], "brightness": args.garden_brightness, "tint": [0.86, 1.0, 0.90], "mottle": {"cells": 7, "depth": 0.30, "seed": 6090745, "periodic": True, "afterMirror": True}},
                 "glb": "assets/models/league/v4/surface-garden.glb", "glbBytes": n, "glbSha256": sha(garden_glb.read_bytes()),
                 "preview": "web/games/league/v4/art/garden.webp", "previewSha256": sha((args.art / "garden.webp").read_bytes())})

    court_glb = args.out / "surface-court.glb"
    if args.court_glb.resolve() != court_glb.resolve():
        shutil.copyfile(args.court_glb, court_glb)
    court_row = {"name": "court", "review": "unchanged from 39390fa5", "glb": "assets/models/league/v4/surface-court.glb",
                 "glbBytes": court_glb.stat().st_size, "glbSha256": sha(court_glb.read_bytes())}
    previous = json.loads(args.manifest.read_text(encoding="utf-8")) if args.manifest else {}
    for row in previous.get("assets", []):
        if row["name"] == "court":
            court_row = {**row, "review": row.get("review", "accepted") + " (unchanged in pass 2)"}
    rows.append(court_row)

    old = ROOT / "assets/models/league/v2"
    kept = sum(p.stat().st_size for p in old.glob("*.glb") if not p.name.startswith("surface-"))
    total = kept + sum(r["glbBytes"] for r in rows)
    budget = 8 * 1024 * 1024
    assert total <= budget, f"Embedded art budget exceeded: {total}"
    manifest = {"version": "v4", "purpose": "actual in-game surface materials", "pass": 2,
                "embeddedArtBytes": total, "embeddedArtBudgetBytes": budget, "assets": rows,
                "rejected": previous.get("rejected", [])}
    text = json.dumps(manifest, indent=2) + "\n"
    (args.out / "manifest.json").write_text(text, encoding="utf-8", newline="\n")
    (args.art / "manifest.json").write_text(text, encoding="utf-8", newline="\n")
    print(json.dumps({"lane": args.lane, "embeddedArtBytes": total, "headroom": budget - total,
                      "surfaces": [{k: r[k] for k in ("name", "glbBytes")} for r in rows]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
