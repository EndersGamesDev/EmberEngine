"""Turn a TripoSR mesh into an ember champion GLB.

TripoSR hands back a vertex-coloured, unwrapped mesh with tens of thousands
of triangles. The engine's loader (`ember_engine::assets::load_glb`) reads
positions, normals, TEXCOORD_0 and the material's base-colour texture, and
ignores COLOR_0 entirely, so the colour has to move from the vertices into a
picture. This script does that in five steps, each one reported with its
wall time:

  1. load      the TripoSR GLB (any orientation), keep its vertex colours
  2. decimate  quadric edge collapse to the face budget (pymeshlab)
  3. orient    Y up, +X forward, origin between the feet, height as asked
  4. unwrap    xatlas on the decimated mesh
  5. bake      a 512x512 RGB atlas: every texel's surface point looks up the
               nearest hi-res vertex colour (KD-tree), edges dilated so the
               bilinear sampler never bleeds the background in
  6. export    one GLB part, baseColorFactor white, PNG 8-bit RGB embedded,
               plus a sidecar JSON with the pivot and the measured extents

Run with the Hunyuan venv's python (has trimesh, xatlas, pymeshlab, scipy,
PIL):

  C:/hy3d/venv/Scripts/python.exe tools/league/art/bake_champion.py ^
      --input target/league-art/swarm/tripo.glb --name swarm ^
      --out assets/models/league/v2 --faces 5000 --height 1.7 ^
      --forward -y --up z

`--forward` / `--up` describe the SOURCE mesh's axes as TripoSR produced
them (look at it once; TripoSR is usually +Z up with the face toward -Y for
a front-view reference). The script rotates that into the engine frame.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import numpy as np

ATLAS = 512
AXIS = {
    "x": np.array([1.0, 0.0, 0.0]),
    "y": np.array([0.0, 1.0, 0.0]),
    "z": np.array([0.0, 0.0, 1.0]),
}


def axis_vec(spec: str) -> np.ndarray:
    sign = -1.0 if spec.startswith("-") else 1.0
    return sign * AXIS[spec.strip("+-").lower()]


def tick(label: str, t0: float) -> float:
    now = time.perf_counter()
    print(f"  {label:<10} {now - t0:6.1f} s", flush=True)
    return now


def load_coloured(path: Path):
    """Load the source as one mesh; returns (vertices, faces, colours 0..1)."""
    import trimesh

    scene = trimesh.load(str(path), force="scene")
    meshes = [g for g in scene.geometry.values() if isinstance(g, trimesh.Trimesh)]
    if not meshes:
        raise SystemExit(f"no triangle mesh in {path}")
    parts = []
    for g in meshes:
        colours = None
        vis = g.visual
        if getattr(vis, "kind", None) == "vertex" and vis.vertex_colors is not None:
            colours = np.asarray(vis.vertex_colors, dtype=np.float32)[:, :3] / 255.0
        elif getattr(vis, "kind", None) == "texture":
            try:
                colours = np.asarray(vis.to_color().vertex_colors, dtype=np.float32)[:, :3] / 255.0
            except Exception:  # noqa: BLE001 - a textureless material is fine
                colours = None
        if colours is None:
            colours = np.full((len(g.vertices), 3), 0.7, dtype=np.float32)
        parts.append((np.asarray(g.vertices, dtype=np.float64), np.asarray(g.faces, dtype=np.int64), colours))
    verts = np.concatenate([p[0] for p in parts])
    offsets = np.cumsum([0] + [len(p[0]) for p in parts[:-1]])
    faces = np.concatenate([p[1] + o for p, o in zip(parts, offsets)])
    colours = np.concatenate([p[2] for p in parts])
    return verts, faces, colours


def drop_debris(verts, faces, colours, min_fraction: float):
    """TripoSR leaves floating specks around a body. Keep every connected
    component that carries at least `min_fraction` of the faces (the body,
    a held weapon), drop the rest, and report what went."""
    import trimesh

    mesh = trimesh.Trimesh(vertices=verts, faces=faces, process=False)
    comps = mesh.split(only_watertight=False)
    if len(comps) <= 1:
        return verts, faces, colours, 0
    total = len(faces)
    keep = [c for c in comps if len(c.faces) >= total * min_fraction]
    dropped = len(comps) - len(keep)
    if not keep:
        keep = [max(comps, key=lambda c: len(c.faces))]
    # colours travel by nearest original vertex (split re-indexes vertices)
    from scipy.spatial import cKDTree

    tree = cKDTree(verts)
    out_v, out_f, out_c = [], [], []
    base = 0
    for c in keep:
        v = np.asarray(c.vertices, dtype=np.float64)
        _, idx = tree.query(v, k=1)
        out_v.append(v)
        out_f.append(np.asarray(c.faces, dtype=np.int64) + base)
        out_c.append(colours[idx])
        base += len(v)
    return np.concatenate(out_v), np.concatenate(out_f), np.concatenate(out_c), dropped


def decimate(verts: np.ndarray, faces: np.ndarray, target_faces: int):
    import pymeshlab

    if len(faces) <= target_faces:
        return verts, faces
    ms = pymeshlab.MeshSet()
    ms.add_mesh(pymeshlab.Mesh(vertex_matrix=verts, face_matrix=faces))
    ms.meshing_remove_duplicate_vertices()
    ms.meshing_decimation_quadric_edge_collapse(
        targetfacenum=int(target_faces),
        preservenormal=True,
        preservetopology=False,
        qualitythr=0.4,
        planarquadric=True,
    )
    ms.meshing_remove_unreferenced_vertices()
    m = ms.current_mesh()
    return np.asarray(m.vertex_matrix(), dtype=np.float64), np.asarray(m.face_matrix(), dtype=np.int64)


def orient(verts: np.ndarray, forward: str, up: str, height: float):
    """Rotate source axes into engine axes (+X forward, +Y up), stand the
    mesh on y=0 centred in x/z, scale to `height`. Returns (verts, matrix)."""
    f = axis_vec(forward)
    u = axis_vec(up)
    if abs(float(np.dot(f, u))) > 1e-6:
        raise SystemExit("--forward and --up must be perpendicular")
    r = np.cross(u, f)  # engine +Z is right = up x forward in a right-handed frame
    # rows are the engine axes expressed in source coordinates
    rot = np.stack([f, u, r])
    if np.linalg.det(rot) < 0:
        r = -r
        rot = np.stack([f, u, r])
    out = verts @ rot.T
    lo = out.min(axis=0)
    hi = out.max(axis=0)
    span = hi[1] - lo[1]
    scale = height / span if span > 1e-9 else 1.0
    out *= scale
    lo = out.min(axis=0)
    hi = out.max(axis=0)
    centre = (lo + hi) / 2.0
    out[:, 0] -= centre[0]
    out[:, 2] -= centre[2]
    out[:, 1] -= lo[1]
    return out, rot, scale


def unwrap(verts: np.ndarray, faces: np.ndarray):
    import xatlas

    vmap, idx, uvs = xatlas.parametrize(verts.astype(np.float32), faces.astype(np.uint32))
    return verts[vmap], idx.astype(np.int64), uvs.astype(np.float32), vmap


def bake(verts, faces, uvs, hi_verts, hi_colours, size: int):
    """Rasterise every triangle in UV space; each covered texel's surface
    point looks up the nearest hi-res vertex colour."""
    from scipy.spatial import cKDTree

    tree = cKDTree(hi_verts)
    img = np.zeros((size, size, 3), dtype=np.float32)
    hit = np.zeros((size, size), dtype=bool)
    px = uvs * (size - 1)
    for tri in faces:
        p = px[tri]
        v = verts[tri]
        xmin = max(int(np.floor(p[:, 0].min())) - 1, 0)
        xmax = min(int(np.ceil(p[:, 0].max())) + 1, size - 1)
        ymin = max(int(np.floor(p[:, 1].min())) - 1, 0)
        ymax = min(int(np.ceil(p[:, 1].max())) + 1, size - 1)
        if xmax < xmin or ymax < ymin:
            continue
        xs, ys = np.meshgrid(np.arange(xmin, xmax + 1), np.arange(ymin, ymax + 1))
        pts = np.stack([xs.ravel() + 0.5, ys.ravel() + 0.5], axis=1)
        a, b, c = p
        det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(det) < 1e-12:
            continue
        l0 = ((b[1] - c[1]) * (pts[:, 0] - c[0]) + (c[0] - b[0]) * (pts[:, 1] - c[1])) / det
        l1 = ((c[1] - a[1]) * (pts[:, 0] - c[0]) + (a[0] - c[0]) * (pts[:, 1] - c[1])) / det
        l2 = 1.0 - l0 - l1
        # a little slack keeps the seam texels covered
        inside = (l0 >= -0.02) & (l1 >= -0.02) & (l2 >= -0.02)
        if not inside.any():
            continue
        bc = np.stack([l0, l1, l2], axis=1)[inside]
        bc = np.clip(bc, 0.0, 1.0)
        bc /= bc.sum(axis=1, keepdims=True)
        surf = bc @ v
        _, nearest = tree.query(surf, k=1)
        cols = hi_colours[nearest]
        ix = pts[inside, 0].astype(int)
        iy = pts[inside, 1].astype(int)
        img[iy, ix] = cols
        hit[iy, ix] = True
    # dilate the islands so bilinear sampling at seams reads a neighbour, not black
    for _ in range(6):
        grown = hit.copy()
        acc = np.zeros_like(img)
        cnt = np.zeros((size, size), dtype=np.float32)
        for dy in (-1, 0, 1):
            for dx in (-1, 0, 1):
                if dx == 0 and dy == 0:
                    continue
                sh = np.roll(np.roll(hit, dy, axis=0), dx, axis=1)
                si = np.roll(np.roll(img, dy, axis=0), dx, axis=1)
                take = sh & ~hit
                acc[take] += si[take]
                cnt[take] += 1.0
        fill = (cnt > 0) & ~hit
        img[fill] = acc[fill] / cnt[fill][:, None]
        grown |= fill
        hit = grown
    return (np.clip(img, 0.0, 1.0) * 255.0 + 0.5).astype(np.uint8), hit


def export(out_dir: Path, name: str, verts, faces, uvs, atlas, extents, height, faces_src, faces_out, wall, atlas_size):
    import io

    import trimesh
    from PIL import Image

    out_dir.mkdir(parents=True, exist_ok=True)
    png_path = out_dir / f"{name}.png"
    Image.fromarray(atlas, "RGB").save(png_path, optimize=True)
    image = Image.open(png_path)
    material = trimesh.visual.material.PBRMaterial(
        baseColorTexture=image,
        baseColorFactor=[255, 255, 255, 255],
        metallicFactor=0.0,
        roughnessFactor=1.0,
        name=f"{name}-albedo",
    )
    # glTF's V runs top-down; xatlas's runs bottom-up
    uv_gl = uvs.copy()
    uv_gl[:, 1] = 1.0 - uv_gl[:, 1]
    mesh = trimesh.Trimesh(vertices=verts, faces=faces, process=False)
    mesh.visual = trimesh.visual.TextureVisuals(uv=uv_gl, material=material)
    mesh.metadata["name"] = name
    scene = trimesh.Scene()
    scene.add_geometry(mesh, node_name=name, geom_name=name)
    glb_path = out_dir / f"{name}.glb"
    # smooth vertex normals travel in the file: the engine's loader defaults a
    # missing NORMAL to straight up, which lights a body flat
    _ = mesh.vertex_normals
    data = scene.export(file_type="glb", include_normals=True)
    glb_path.write_bytes(data)
    side = {
        "name": name,
        "pivot": [0.0, 0.0, 0.0],
        "forward": "+x",
        "up": "+y",
        "height": height,
        "extents": {"min": [float(v) for v in extents[0]], "max": [float(v) for v in extents[1]]},
        "faces": {"source": int(faces_src), "shipped": int(faces_out)},
        "atlas": {"size": atlas_size, "png_bytes": png_path.stat().st_size},
        "glb_bytes": len(data),
        "bake_wall_seconds": round(wall, 1),
        "part_order": [name],
    }
    (out_dir / f"{name}.json").write_text(json.dumps(side, indent=2) + "\n", encoding="utf-8", newline="\n")
    return glb_path, png_path, len(data)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--input", required=True, type=Path)
    ap.add_argument("--name", required=True)
    ap.add_argument("--out", required=True, type=Path)
    ap.add_argument("--faces", type=int, default=5000)
    ap.add_argument("--height", type=float, default=1.7)
    ap.add_argument("--forward", default="-y", help="source axis that faces the camera in the reference")
    ap.add_argument("--up", default="z", help="source axis that points up")
    ap.add_argument("--atlas", type=int, default=ATLAS)
    ap.add_argument("--min-component", type=float, default=0.01,
                    help="drop connected components smaller than this fraction of the faces (TripoSR specks)")
    args = ap.parse_args()
    atlas_size = int(args.atlas)

    t_start = time.perf_counter()
    t = t_start
    verts, faces, colours = load_coloured(args.input)
    print(f"source: {len(verts)} vertices, {len(faces)} faces, colours {'present' if colours.std() > 1e-3 else 'FLAT'}")
    t = tick("load", t)
    verts, faces, colours, dropped = drop_debris(verts, faces, colours, args.min_component)
    if dropped:
        print(f"debris: dropped {dropped} floating component(s) under {args.min_component:.1%} of the faces; {len(faces)} faces remain")
    t = tick("debris", t)
    hi_verts_src = verts.copy()
    dverts, dfaces = decimate(verts, faces, args.faces)
    t = tick("decimate", t)
    dverts, rot, scale = orient(dverts, args.forward, args.up, args.height)
    hi_verts = (hi_verts_src @ rot.T) * scale
    # the same translation the decimated mesh received
    lo = (hi_verts_src @ rot.T * scale)
    # recompute translation from the decimated result so both agree exactly
    shift = np.array([0.0, 0.0, 0.0])
    d_lo = dverts.min(axis=0)
    d_hi = dverts.max(axis=0)
    h_lo = lo.min(axis=0)
    h_hi = lo.max(axis=0)
    shift[0] = -(h_lo[0] + h_hi[0]) / 2.0
    shift[2] = -(h_lo[2] + h_hi[2]) / 2.0
    shift[1] = -h_lo[1]
    hi_verts = lo + shift
    t = tick("orient", t)
    uverts, ufaces, uvs, _ = unwrap(dverts, dfaces)
    t = tick("unwrap", t)
    atlas, hit = bake(uverts, ufaces, uvs, hi_verts, colours, atlas_size)
    coverage = float(hit.mean())
    t = tick("bake", t)
    extents = (uverts.min(axis=0), uverts.max(axis=0))
    glb, png, nbytes = export(args.out, args.name, uverts, ufaces, uvs, atlas, extents, args.height, len(faces), len(ufaces), time.perf_counter() - t_start, atlas_size)
    t = tick("export", t)
    print(f"shipped: {len(ufaces)} faces, {len(uverts)} vertices, atlas {atlas_size}x{atlas_size} ({coverage:.0%} covered), "
          f"{png.stat().st_size / 1024:.0f} KB png, {nbytes / 1024:.0f} KB glb -> {glb}")
    print(f"extents x {extents[0][0]:+.2f}..{extents[1][0]:+.2f}  y {extents[0][1]:+.2f}..{extents[1][1]:+.2f}  z {extents[0][2]:+.2f}..{extents[1][2]:+.2f}")
    print(f"total wall {time.perf_counter() - t_start:.1f} s")
    return 0


if __name__ == "__main__":
    sys.exit(main())
