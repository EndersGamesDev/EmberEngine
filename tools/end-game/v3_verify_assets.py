"""Validate the exported runtime bytes, never the authoring scene. Python + numpy + Pillow."""
from pathlib import Path
import os
from io import BytesIO
import ast,hashlib,json,math,struct,time
import numpy as np
from PIL import Image

START=time.perf_counter()
ROOT=Path(os.environ['END_GAME_V3_ASSET_WORK'])
DTYPES={5120:'i1',5121:'u1',5122:'<i2',5123:'<u2',5125:'<u4',5126:'<f4'}
COUNTS={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}

# Reuse pure geometry helper definitions without importing Blender or running builds.
source=ast.parse(Path(__file__).with_name('v3_build_hands.py').read_text())
helper_names={'geom','add','capsule','ellipsoid','plate','box','torus'}
scope={'np':np,'math':math}
exec(compile(ast.Module(body=[n for n in source.body if isinstance(n,ast.FunctionDef) and n.name in helper_names],type_ignores=[]),'geometry_helper_check','exec'),scope)
volumes={}
for helper,args in [('capsule',(-.1,.1,.04,.05)),('ellipsoid',((0,0,0),(.03,.04,.05))),('plate',(0,.07,.035,.02)),('box',((0,0,0),(.04,.05,.06))),('torus',((0,0,0),.04,.05,.006))]:
    g=scope['geom']();scope[helper](g,*args);v=np.array(g['verts']);volume=0
    for face in g['faces']:
        for i in range(1,len(face)-1):volume+=np.dot(v[face[0]],np.cross(v[face[i]],v[face[i+1]]))/6
    assert volume>0,(helper,'inward closed-shape winding',volume)
    volumes[helper]=float(volume)

def read_glb(path):
    raw=path.read_bytes();magic,version,total=struct.unpack_from('<4sII',raw)
    assert magic==b'glTF' and version==2 and total==len(raw)
    json_len=struct.unpack_from('<I',raw,12)[0]
    gltf=json.loads(raw[20:20+json_len]);bin_start=20+json_len
    bin_length,bin_type=struct.unpack_from('<I4s',raw,bin_start)
    assert bin_type==b'BIN\0';binary=raw[bin_start+8:bin_start+8+bin_length]
    return raw,gltf,binary

def accessor(gltf,binary,index):
    a=gltf['accessors'][index];v=gltf['bufferViews'][a['bufferView']]
    dtype=np.dtype(DTYPES[a['componentType']]);n=COUNTS[a['type']]
    start=v.get('byteOffset',0)+a.get('byteOffset',0);stride=v.get('byteStride',dtype.itemsize*n)
    return np.ndarray((a['count'],n),dtype=dtype,buffer=binary,offset=start,strides=(stride,dtype.itemsize)).copy()

def check(path,expected):
    raw,gltf,binary=read_glb(path);names=[n['name'] for n in gltf['nodes']]
    assert len(names)==len(set(names)) and set(names)==set(expected),(names,expected)
    assert len(gltf.get('skins',[]))==len(gltf.get('animations',[]))==0
    images=[]
    for image in gltf['images']:
        view=gltf['bufferViews'][image['bufferView']];data=binary[view.get('byteOffset',0):view.get('byteOffset',0)+view['byteLength']]
        decoded=Image.open(BytesIO(data));assert decoded.mode=='RGB' and decoded.size==(128,128)
        assert data[:8]==b'\x89PNG\r\n\x1a\n' and data[24]==8 and data[25]==2
        images.append(dict(bytes=len(data),mode=decoded.mode,size=list(decoded.size),bit_depth=8))
    assert len(images)==1
    parts=[]
    for node in gltf['nodes']:
        assert node.get('translation',[0,0,0])==[0,0,0]
        assert node.get('rotation',[0,0,0,1])==[0,0,0,1]
        assert node.get('scale',[1,1,1])==[1,1,1]
        assert node.get('matrix',[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1])==[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
        assert not node.get('children')
        mesh=gltf['meshes'][node['mesh']];assert len(mesh['primitives'])==1
        primitive=mesh['primitives'][0];assert primitive.get('mode',4)==4
        positions=accessor(gltf,binary,primitive['attributes']['POSITION'])
        normals=accessor(gltf,binary,primitive['attributes']['NORMAL'])
        uvs=accessor(gltf,binary,primitive['attributes']['TEXCOORD_0'])
        indices=accessor(gltf,binary,primitive['indices']).reshape(-1,3)
        assert len(positions)==len(normals)==len(uvs)
        assert all(np.isfinite(a).all() for a in [positions,normals,uvs])
        assert np.allclose(np.linalg.norm(normals,axis=1),1,atol=1e-4)
        assert uvs.min()>=0 and uvs.max()<=1
        material=gltf['materials'][primitive['material']]
        assert material.get('alphaMode','OPAQUE')=='OPAQUE'
        assert material['pbrMetallicRoughness'].get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
        assert 'baseColorTexture' in material['pbrMetallicRoughness']
        face=np.cross(positions[indices[:,1]]-positions[indices[:,0]],positions[indices[:,2]]-positions[indices[:,0]])
        magnitude=np.linalg.norm(face,axis=1);valid=magnitude>1e-12
        agreement=(face[valid]*normals[indices[valid]].mean(axis=1)).sum(axis=1)
        inverted=int((agreement < -1e-12).sum())
        assert inverted==0,(node['name'],inverted)
        assert valid.all(),(node['name'],'zero-area triangles')
        parts.append(dict(name=node['name'],triangles=len(indices),degenerate_triangles=int((~valid).sum()),bounds_min=positions.min(axis=0).round(7).tolist(),bounds_max=positions.max(axis=0).round(7).tolist(),normal_winding_inversions=inverted))
    low=np.min([p['bounds_min'] for p in parts],axis=0);high=np.max([p['bounds_max'] for p in parts],axis=0)
    return dict(file=path.name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),nodes=len(names),primitives=len(parts),triangles=sum(p['triangles'] for p in parts),degenerate_triangles=sum(p['degenerate_triangles'] for p in parts),bounds_min=low.tolist(),bounds_max=high.tolist(),size=(high-low).round(7).tolist(),textures=images,parts=parts)

rig=json.loads((ROOT/'hands-rig.json').read_text())
hands=check(ROOT/'wolf-hands.glb',[p['name'] for p in rig['parts']]);key=check(ROOT/'iron-key.glb',['iron_key'])
assert hands['triangles']<=15000 and hands['bytes']+key['bytes']<=1500000
assert len(rig['parts'])==34 and len([p for p in rig['parts'] if p['name'].startswith('hand_r_')])==17
seen=set()
for part in rig['parts']:
    assert part['parent'] is None or part['parent'] in seen
    seen.add(part['name'])
right=np.array(rig['contacts_right']['power_grip']['axis']);left=np.array(rig['contacts_left']['power_grip']['axis'])
assert np.allclose(left,right*np.array([1,1,-1]))
files=[hands,key]
if (ROOT/'wolf-greatsword.glb').exists():
    sword=check(ROOT/'wolf-greatsword.glb',['wolf_greatsword']);assert sword['bytes']<300000;files.append(sword)
manifest=dict(schema_version=1,description='Paired articulated wolf gauntlets with 34 rigid identity-transform nodes; ring-pivot iron key; matching greatsword.',units='meters',files=files,total_bytes=sum(f['bytes'] for f in files),total_triangles=sum(f['triangles'] for f in files),hand_texture_gpu_bytes_with_mips=34*128*128*4*4//3,hand_vertex_runtime_bytes_estimate=hands['triangles']*3*32,bind_sidecar='hands-rig.json',authoring_sources=['generate_atlas.py','build_hands.py','build_greatsword.py','wolf-hands-editable.blend','wolf-greatsword-editable.blend'],closed_shape_positive_signed_volumes=volumes,verification='Exported GLB structure, indices, normals, UVs, RGB8 embedded image, identity node transforms, names/hierarchy sidecar, triangle and byte budgets; outward closed-shape helpers; Blender pose previews visually inspected. Native Ember rendering remains integration verification.',verification_seconds=round(time.perf_counter()-START,4))
(ROOT/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print(json.dumps({k:v for k,v in manifest.items() if k not in ['files','authoring_sources']},indent=2))
print('PART_DEGENERATES',[(p['name'],p['degenerate_triangles']) for p in hands['parts'] if p['degenerate_triangles']])
