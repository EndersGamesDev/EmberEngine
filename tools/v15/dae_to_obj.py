#!/usr/bin/env python3
"""Collada -> OBJ for the heavy revolver, because Blender 5.x has no Collada
importer any more (the operator is gone, not merely disabled).

    C:\hy3d\venv\Scripts\python.exe tools/v15/dae_to_obj.py <model.dae> <out.obj>

One OBJ object per scene node (the part names are the animation contract
downstream), world transforms baked, the file's unit applied so the OBJ is in
metres, and an .mtl beside it whose map_Kd points at the source albedo of
each Collada material. Normals and the first UV set are carried; Blender's
OBJ importer keeps both.

pycollada is the parser (pip-installed into the Hunyuan venv, which already
has numpy); its bound geometries hand back vertices with the node matrices
already applied, which is exactly the baking the 9mm converter did by hand.
"""
import os, sys
import numpy as np
import collada

src, out = sys.argv[1], sys.argv[2]
# map_Kd paths resolve relative to the .mtl, so write them that way.
tex_dir = os.path.relpath(os.path.join(os.path.dirname(src), "textures"), os.path.dirname(out)).replace("\\", "/")
m = collada.Collada(src, ignore=[collada.common.DaeUnsupportedError, collada.common.DaeBrokenRefError])
unit = m.assetInfo.unitmeter or 1.0
mtl_path = os.path.splitext(out)[0] + ".mtl"
albedo = {"B": "B_albedo.jpg", "M1": "M1_albedo.jpg", "M2": "M2_albedo.jpg"}
with open(mtl_path, "w", newline="\n") as f:
    for mat in m.materials:
        f.write(f"newmtl {mat.id}\nKd 1 1 1\nmap_Kd {os.path.join(tex_dir, albedo[mat.id])}\n\n")
vo = no = to = 0
parts = 0
with open(out, "w", newline="\n") as f:
    f.write(f"mtllib {os.path.basename(mtl_path)}\n")
    for node in m.scene.nodes:
        name = node.id or "node"
        for bg in [c for c in node.objects("geometry")]:
            for bp in bg.primitives():
                if bp.vertex_index is None:
                    continue
                verts = bp.vertex * unit
                idx = bp.vertex_index.reshape(-1, 3)
                nrm = bp.normal
                nidx = bp.normal_index.reshape(-1, 3) if bp.normal_index is not None else None
                uv = bp.texcoordset[0] if bp.texcoordset else None
                tidx = bp.texcoord_indexset[0].reshape(-1, 3) if bp.texcoord_indexset else None
                mat_id = None
                for mn in (bp.original.material and [bp.original.material] or []):
                    mat_id = mn
                # the bound primitive knows its resolved material
                mat_id = getattr(bp, "material", None)
                mat_name = mat_id.id if mat_id is not None else "M2"
                f.write(f"o {name}\nusemtl {mat_name}\n")
                for v in verts: f.write(f"v {v[0]:.6f} {v[1]:.6f} {v[2]:.6f}\n")
                if nrm is not None:
                    for n in nrm: f.write(f"vn {n[0]:.5f} {n[1]:.5f} {n[2]:.5f}\n")
                if uv is not None:
                    for t in uv: f.write(f"vt {t[0]:.6f} {t[1]:.6f}\n")
                for k in range(len(idx)):
                    a = idx[k] + vo + 1
                    b = (tidx[k] + to + 1) if tidx is not None else None
                    c = (nidx[k] + no + 1) if nidx is not None else None
                    def ref(i):
                        s = str(a[i])
                        if b is not None or c is not None:
                            s += "/" + (str(b[i]) if b is not None else "")
                            if c is not None: s += "/" + str(c[i])
                        return s
                    f.write(f"f {ref(0)} {ref(1)} {ref(2)}\n")
                vo += len(verts); no += len(nrm) if nrm is not None else 0; to += len(uv) if uv is not None else 0
                parts += 1
print(f"[dae2obj] {parts} parts, {vo} vertices, unit {unit} -> {out}")
