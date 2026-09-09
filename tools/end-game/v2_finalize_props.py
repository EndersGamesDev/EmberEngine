import os
"""Validate GLBs, pack RGB8 atlases, pad gutters, normalize floor, record pivots."""
from pathlib import Path
import io,json,struct,time,hashlib
import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt
ROOT=Path(os.environ['END_GAME_V2_ASSET_WORK']);OUT=ROOT/'props';START=time.perf_counter()
build=json.loads((OUT/'build-report.json').read_text()); spec={r['name']:r for r in build['props']};rows=[]
for path in sorted(OUT.glob('*.glb')):
    b=path.read_bytes();n=struct.unpack_from('<I',b,12)[0];doc=json.loads(b[20:20+n]);raw=b[28+n:]
    assert len(doc['meshes'])==1 and len(doc['meshes'][0]['primitives'])==1 and len(doc['images'])==1
    p=doc['meshes'][0]['primitives'][0];assert set(p['attributes'])=={'POSITION','NORMAL','TEXCOORD_0'}
    m=doc['materials'][p['material']];assert m.get('alphaMode','OPAQUE')=='OPAQUE'
    assert m['pbrMetallicRoughness'].get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
    assert not any(k in doc['nodes'][0] for k in ['matrix','translation','rotation','scale'])
    iv=doc['images'][0]['bufferView'];rebuilt=bytearray();texture_bytes=0
    for i,view in enumerate(doc['bufferViews']):
        off=view.get('byteOffset',0);data=raw[off:off+view['byteLength']]
        if i==iv:
            source_png=OUT/(path.stem+'-albedo.png')
            im=(Image.open(source_png) if doc['images'][0].get('mimeType')=='image/jpeg' and source_png.exists() else Image.open(io.BytesIO(data))).convert('RGB');rgb=np.array(im);empty=np.max(rgb,axis=2)<3
            if empty.any() and not empty.all():
                nearest=distance_transform_edt(empty,return_distances=False,return_indices=True);rgb[empty]=rgb[nearest[0][empty],nearest[1][empty]]
            im=Image.fromarray(rgb,'RGB');im.save(OUT/(path.stem+'-albedo.png'),optimize=True)
            sink=io.BytesIO();im.save(sink,format='JPEG',quality=93,subsampling=0,optimize=True);data=sink.getvalue()
            assert Image.open(io.BytesIO(data)).mode=='RGB'
            sof=data.find(b'\xff\xc0');assert sof>=0 and data[sof+4]==8
            doc['images'][0]['mimeType']='image/jpeg'
            texture_bytes=len(data);image_size=list(im.size)
        view['byteOffset']=len(rebuilt);view['byteLength']=len(data);rebuilt.extend(data);rebuilt.extend(b'\0'*((-len(rebuilt))%4))
    doc['buffers'][0]['byteLength']=len(rebuilt)
    def acc(index):
        a=doc['accessors'][index];v=doc['bufferViews'][a['bufferView']];dt={5126:'<f4',5125:'<u4',5123:'<u2'}[a['componentType']];sz={'VEC3':3,'VEC2':2,'SCALAR':1}[a['type']]
        return np.frombuffer(rebuilt,dtype=dt,count=a['count']*sz,offset=a.get('byteOffset',0)+v.get('byteOffset',0)).reshape((-1,sz))
    xyz=acc(p['attributes']['POSITION']);norm=acc(p['attributes']['NORMAL']);uv=acc(p['attributes']['TEXCOORD_0']);ix=acc(p['indices']).reshape((-1,3))
    assert np.isfinite(xyz).all() and np.isfinite(norm).all() and np.isfinite(uv).all()
    assert np.allclose(np.linalg.norm(norm,axis=1),1,atol=.02)
    assert (np.ptp(uv,axis=0)>.1).all()
    floor=path.stem not in ['chain-shackle','hanging-rag'];shift=-float(spec[path.stem]['bounds_min'][1]) if floor else 0
    if not doc.get('extras',{}).get('runtime_v2_finalized'):
        xyz[:,1]+=shift
    doc.setdefault('extras',{})['runtime_v2_finalized']=True
    pa=doc['accessors'][p['attributes']['POSITION']];pa['min']=xyz.min(0).tolist();pa['max']=xyz.max(0).tolist()
    anchors=json.loads(json.dumps(spec[path.stem]['anchors']))
    for key in ['mattress_top','flame_anchor','handle_axis']:
        if key in anchors:anchors[key][1]+=shift
    anchors['pivot']=[0,0,0]
    j=json.dumps(doc,separators=(',',':')).encode();j+=b' '*((-len(j))%4);raw=bytes(rebuilt)
    final=struct.pack('<III',0x46546c67,2,12+8+len(j)+8+len(raw))+struct.pack('<II',len(j),0x4e4f534a)+j+struct.pack('<II',len(raw),0x004e4942)+raw
    path.write_bytes(final)
    row=dict(file=path.name,node=doc['nodes'][0].get('name'),triangles=len(ix),primitives=1,bytes=len(final),texture_bytes=texture_bytes,texture_size=image_size,texture_mode='RGB',texture_bit_depth=8,texture_format='JPEG quality93 no chroma subsampling',bounds_min=xyz.min(0).tolist(),bounds_max=xyz.max(0).tolist(),size=np.ptp(xyz,axis=0).tolist(),anchors=anchors,sha256=hashlib.sha256(final).hexdigest());rows.append(row);print(json.dumps(row))
total_tri=sum(r['triangles'] for r in rows);total_tex=sum(r['texture_bytes'] for r in rows)
assert total_tri<=30000,(total_tri,'triangle budget exceeded');assert total_tex<=3000000,(total_tex,'embedded texture budget exceeded')
assert sum(r['bytes'] for r in rows)<=3000000,'complete GLB budget exceeded'
oak_source=ROOT/'surfaces'/'oak-aged-albedo-v2.png'
manifest=dict(units='meters',axes='Y up; cot length Z, headboard positive Z; wall props face positive Z',props=rows,total_triangles=total_tri,total_embedded_texture_bytes=total_tex,total_glb_bytes=sum(r['bytes'] for r in rows),build_seconds=build['total_seconds'],verify_seconds=round(time.perf_counter()-START,2),provenance='Local deterministic Blender geometry, seeded non-wood materials, and root-supplied ImageGen aged-oak picture. No reserved LAN image or mesh service called; fleet reservations honored.',oak_source=str(oak_source),oak_source_sha256=hashlib.sha256(oak_source.read_bytes()).hexdigest() if oak_source.exists() else None)
(OUT/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
print('TOTAL',json.dumps({k:v for k,v in manifest.items() if k.startswith('total_')}))
