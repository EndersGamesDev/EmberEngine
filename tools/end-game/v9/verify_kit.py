"""Inspect actual embedded RGB8 images, geometry, primitive count and kit budgets."""
from pathlib import Path
import hashlib, io, json, os, struct, time
import numpy as np
from PIL import Image

ROOT=Path(os.environ['END_GAME_V9_WORK'])
started=time.perf_counter();results=[]
for name in ['gothic-pillar','courtyard-fountain','tower-doorway']:
    path=ROOT/name/'converted'/(name+'.glb')
    if not path.is_file():raise FileNotFoundError(f'Missing required castle asset: {path}')
    raw=path.read_bytes();assert raw[:4]==b'glTF'
    n=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+n]);binary=raw[28+n:]
    def arr(index):
        a=g['accessors'][index];v=g['bufferViews'][a['bufferView']]
        dtype=np.dtype({5123:'<u2',5125:'<u4',5126:'<f4'}[a['componentType']]);width={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4}[a['type']]
        return np.ndarray((a['count'],width),dtype=dtype,buffer=binary,offset=v.get('byteOffset',0)+a.get('byteOffset',0),strides=(v.get('byteStride',dtype.itemsize*width),dtype.itemsize))
    prims=[p for m in g['meshes'] for p in m['primitives']];assert len(prims)==1
    p=prims[0];assert p.get('mode',4)==4
    pos=arr(p['attributes']['POSITION']);norm=arr(p['attributes']['NORMAL']);uv=arr(p['attributes']['TEXCOORD_0']);ids=arr(p['indices']).reshape(-1,3)
    assert np.isfinite(pos).all() and np.isfinite(norm).all() and np.isfinite(uv).all()
    assert ids.max()<len(pos);assert (np.linalg.norm(norm,axis=1)>.9).all()
    area=np.linalg.norm(np.cross(pos[ids[:,1]]-pos[ids[:,0]],pos[ids[:,2]]-pos[ids[:,0]]),axis=1)
    assert (area>1e-12).all(),'Degenerate exported triangle'
    material=g['materials'][p['material']];pbr=material['pbrMetallicRoughness']
    assert pbr.get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
    texid=pbr['baseColorTexture']['index'];im=g['images'][g['textures'][texid]['source']];view=g['bufferViews'][im['bufferView']]
    png=binary[view.get('byteOffset',0):][:view['byteLength']];image=Image.open(io.BytesIO(png))
    assert png[:8]==b'\x89PNG\r\n\x1a\n' and png[24]==8 and png[25]==2,'Need RGB8 PNG'
    assert image.mode=='RGB' and image.size==(768,768)
    width,height=image.size;mips=0
    while True:
        mips+=width*height*4
        if width==height==1:break
        width=max(1,width//2);height=max(1,height//2)
    result=dict(prop=name,bytes=len(raw),sha256=hashlib.sha256(raw).hexdigest(),triangles=len(ids),primitive_count=len(prims),texture_pixels=list(image.size),texture_mode=image.mode,texture_bit_depth=png[24],cloned_texture_bytes_with_mips=mips)
    manifest=json.loads(path.with_name('manifest.json').read_text(encoding='utf-8'));manifest.update(result);path.with_name('manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n');results.append(result)
totals=dict(props=len(results),bytes=sum(x['bytes'] for x in results),triangles=sum(x['triangles'] for x in results),cloned_texture_bytes_with_mips=sum(x['cloned_texture_bytes_with_mips'] for x in results))
assert totals['triangles']<=35000 and totals['bytes']<=6000000 and totals['cloned_texture_bytes_with_mips']<=12*1024*1024
record=dict(verified=results,totals=totals,wall_seconds=round(time.perf_counter()-started,4),runtime_verified=False)
(ROOT/'verification.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(record,indent=2))
