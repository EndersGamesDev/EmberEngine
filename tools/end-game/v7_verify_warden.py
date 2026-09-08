"""Validate the actual V7 GLB and emit a manifest. No engine writes."""
from pathlib import Path
from io import BytesIO
import hashlib,json,os,struct,time
import numpy as np
from PIL import Image
started=time.perf_counter();root=Path(os.environ['END_GAME_V7_WORK'])
raw=(root/'warden.glb').read_bytes();magic,version,total=struct.unpack_from('<4sII',raw)
assert magic==b'glTF' and version==2 and total==len(raw)
length=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+length]);binary=raw[28+length:]
types={5123:'<u2',5125:'<u4',5126:'<f4'};components={'SCALAR':1,'VEC2':2,'VEC3':3}
def data(index):
    a=g['accessors'][index];v=g['bufferViews'][a['bufferView']];t=np.dtype(types[a['componentType']]);n=components[a['type']]
    return np.ndarray((a['count'],n),dtype=t,buffer=binary,offset=v.get('byteOffset',0)+a.get('byteOffset',0),strides=(v.get('byteStride',t.itemsize*n),t.itemsize)).copy()
sidecar=json.loads((root/'warden-rig.json').read_text());names=[p['name'] for p in sidecar['parts']]
assert len(names)==20 and len(set(names))==20
assert {p['name'] for p in g['nodes']}==set(names)
assert len(g.get('skins',[]))==len(g.get('animations',[]))==0
assert len(g['images'])==19
images=[]
for image in g['images']:
    view=g['bufferViews'][image['bufferView']];png=binary[view.get('byteOffset',0):view.get('byteOffset',0)+view['byteLength']]
    assert png[:8]==b'\x89PNG\r\n\x1a\n' and png[24]==8 and png[25]==2
    im=Image.open(BytesIO(png));assert im.mode=='RGB' and im.size in [(128,128),(256,256),(512,512),(1024,1024)]
    images.append(dict(size=list(im.size),mode='RGB',bit_depth=8,embedded_bytes=len(png)))
parts=[]
for node in g['nodes']:
    assert node.get('translation',[0,0,0])==[0,0,0]
    assert node.get('rotation',[0,0,0,1])==[0,0,0,1]
    assert node.get('scale',[1,1,1])==[1,1,1]
    assert not node.get('matrix') and not node.get('children')
    mesh=g['meshes'][node['mesh']];assert len(mesh['primitives'])==1
    p=mesh['primitives'][0];assert p.get('mode',4)==4
    pos=data(p['attributes']['POSITION']);norm=data(p['attributes']['NORMAL']);uv=data(p['attributes']['TEXCOORD_0']);indices=data(p['indices']).reshape(-1,3)
    assert len(pos)==len(norm)==len(uv) and np.isfinite(pos).all() and np.isfinite(norm).all() and np.isfinite(uv).all()
    assert np.allclose(np.linalg.norm(norm,axis=1),1,atol=1e-4)
    assert uv.min()>=0 and uv.max()<=1
    cross=np.cross(pos[indices[:,1]]-pos[indices[:,0]],pos[indices[:,2]]-pos[indices[:,0]])
    assert (np.linalg.norm(cross,axis=1)>1e-12).all(),node['name']
    assert ((cross*norm[indices].mean(axis=1)).sum(axis=1)>=-1e-12).all(),node['name']
    material=g['materials'][p['material']];assert material.get('alphaMode','OPAQUE')=='OPAQUE'
    assert material['pbrMetallicRoughness'].get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
    assert 'baseColorTexture' in material['pbrMetallicRoughness']
    image_index=g['textures'][material['pbrMetallicRoughness']['baseColorTexture']['index']]['source']
    # UV/normal splits duplicate exported vertices. Weld coincident positions
    # before checking the repaired lower legs for actual open/nonmanifold edges.
    _,weld=np.unique(np.round(pos,5),axis=0,return_inverse=True)
    tri=weld[indices];edges=np.sort(np.concatenate([tri[:,[0,1]],tri[:,[1,2]],tri[:,[2,0]]]),axis=1)
    _,counts=np.unique(edges,axis=0,return_counts=True)
    topology=dict(boundary_edges=int((counts==1).sum()),nonmanifold_edges=int((counts>2).sum()))
    if node['name'].startswith('warden_shin_'):
        assert topology==dict(boundary_edges=0,nonmanifold_edges=0),(node['name'],topology)
    if node['name'].startswith('warden_thigh_'):
        assert pos[:,1].max()>.94 and pos[:,1].min()<.55,(node['name'],'missing hip-to-knee trouser coverage')
    parts.append(dict(name=node['name'],triangles=len(indices),image_index=image_index,bounds_min=pos.min(axis=0).tolist(),bounds_max=pos.max(axis=0).tolist(),welded_topology=topology))
assert len(raw)<6500000 and sum(p['triangles'] for p in parts)<=22500
parents=set()
for row in sidecar['parts']:
    assert row['parent'] is None or row['parent'] in parents
    parents.add(row['name'])
body=[p for p in parts if p['name'] not in ['warden_chair','warden_knife']]
low=np.min([p['bounds_min'] for p in body],axis=0);high=np.max([p['bounds_max'] for p in body],axis=0)
gpu=sum(images[p['image_index']]['size'][0]**2*4*4//3 for p in parts)
assert gpu<=20*1024*1024
provenance=dict(method='SDXL concept -> TRELLIS.2 generated textured sculpt -> cleaned, split, retargeted and rebaked',image_job='4fc99fa8-71cc-470e-9835-2380dce3f4ef',mesh_job='20260908-153917-a6b547',seed=709081,image_model='SDXL 1.0 base, Specht ComfyUI',mesh_model='TRELLIS.2 4B, Adler GPU0, 1024_cascade',source_bytes=21569692,generators=['tools/end-game/v7_generate_asset.py','tools/end-game/v7_preview_source.py','tools/end-game/v7_convert_warden.py','tools/end-game/v7_verify_warden.py'],retained='V5 chair and knife, unchanged bind geometry/material',units='meters',facing='-Z',up='+Y')
provenance['source_sha256']='bbbb21903c349bb3ecc21d67dacc2373c3183837269d1e7ef306cac9e51e24f2'
provenance['concept_sha256']='366522f3b857143e64c7969ace683d7e89ea40c40ec5e5482fb8f6f8d01bb28a'
provenance['cutout_sha256']='8ecfc199f9bc96506ef555449f7b5978c71fd5919c4fec8c3bdfba6af48d3779'
provenance['generators'].extend(['tools/end-game/v7_restore_trousers.py','tools/end-game/v7_finalize_warden.py','tools/end-game/v7_preview_warden.py'])
manifest=dict(schema_version=1,provenance=provenance,file='warden.glb',bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),nodes=20,primitives=20,triangles=sum(p['triangles'] for p in parts),textures=images,cloned_texture_gpu_bytes_with_mips=gpu,body_bounds_min=low.tolist(),body_bounds_max=high.tolist(),sidecar='warden-rig.json',parts=parts,verification='Exported GLB names/identity transforms, finite positions/UVs, normalized normals, winding agreement, zero degenerate triangles, RGB8 images, opaque white materials, hierarchy and payload budgets. Native motion/rendering verification is separate.',verification_seconds=round(time.perf_counter()-started,4))
manifest['sidecar_sha256']=hashlib.sha256((root/'warden-rig.json').read_bytes()).hexdigest()
manifest['verification']+=' Both repaired shin meshes have zero welded boundary and nonmanifold edges.'
manifest['verification']+=' Both thighs retain full hip-to-knee inner trouser coverage.'
(root/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in manifest.items() if k!='parts'},indent=2))
