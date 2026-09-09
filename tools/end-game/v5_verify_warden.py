"""Validate the actual V5 GLB and emit a manifest. No engine writes."""
from pathlib import Path
from io import BytesIO
import hashlib,json,os,struct,time
import numpy as np
from PIL import Image
started=time.perf_counter();root=Path(os.environ['END_GAME_V5_ASSET_WORK'])
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
assert len(g['images'])==1
images=[]
for image in g['images']:
    view=g['bufferViews'][image['bufferView']];png=binary[view.get('byteOffset',0):view.get('byteOffset',0)+view['byteLength']]
    assert png[:8]==b'\x89PNG\r\n\x1a\n' and png[24]==8 and png[25]==2
    im=Image.open(BytesIO(png));assert im.mode=='RGB' and im.size in [(128,128),(256,256)]
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
    parts.append(dict(name=node['name'],triangles=len(indices),image_index=image_index,bounds_min=pos.min(axis=0).tolist(),bounds_max=pos.max(axis=0).tolist()))
assert len(raw)<1000000 and sum(p['triangles'] for p in parts)<25000
parents=set()
for row in sidecar['parts']:
    assert row['parent'] is None or row['parent'] in parents
    parents.add(row['name'])
body=[p for p in parts if p['name'] not in ['warden_chair','warden_knife']]
low=np.min([p['bounds_min'] for p in body],axis=0);high=np.max([p['bounds_max'] for p in body],axis=0)
gpu=sum(images[p['image_index']]['size'][0]**2*4*4//3 for p in parts)
manifest=dict(schema_version=1,provenance=dict(method='Deterministic material-built Blender geometry; no generated bitmap service or imported character mesh',generators=['tools/end-game/v5_generate_atlas.py','tools/end-game/v5_build_warden.py','tools/end-game/v5_verify_warden.py'],geometry_helper='tools/end-game/v3_build_hands.py (read-only)',atlas_seed=5090926,units='meters',facing='-Z',up='+Y'),file='warden.glb',bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),nodes=20,primitives=20,triangles=sum(p['triangles'] for p in parts),textures=images,cloned_texture_gpu_bytes_with_mips=gpu,body_bounds_min=low.tolist(),body_bounds_max=high.tolist(),sidecar='warden-rig.json',parts=parts,verification='Exported GLB names/identity transforms, finite positions/UVs, normalized normals, winding agreement, zero degenerate triangles, RGB8 images, opaque white materials, hierarchy and payload budgets. Native motion/rendering verification is separate.',verification_seconds=round(time.perf_counter()-started,4))
(root/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in manifest.items() if k!='parts'},indent=2))
