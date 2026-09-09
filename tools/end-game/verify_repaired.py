"""Enforce RGB PNG payloads and verify clean exported mesh connectivity."""
from pathlib import Path
from collections import Counter
import io,json,struct,time,os
import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt

OUT=Path(os.environ['END_GAME_ASSET_WORK']).resolve()/'repaired'; START=time.perf_counter(); report=[]
for path in sorted(OUT.glob('*.glb')):
    b=path.read_bytes(); n=struct.unpack_from('<I',b,12)[0]; d=json.loads(b[20:20+n]); raw=b[28+n:]
    assert len(d['meshes'])==1 and len(d['meshes'][0]['primitives'])==1 and len(d['images'])==1
    p=d['meshes'][0]['primitives'][0]; assert set(p['attributes'])=={'POSITION','NORMAL','TEXCOORD_0'}
    assert d['materials'][p['material']]['pbrMetallicRoughness'].get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
    view_id=d['images'][0]['bufferView']; rebuilt=bytearray()
    for i,view in enumerate(d['bufferViews']):
        off=view.get('byteOffset',0); data=raw[off:off+view['byteLength']]
        if i==view_id:
            im=Image.open(io.BytesIO(data)).convert('RGB')
            rgb=np.array(im); empty=np.max(rgb,axis=2)<3
            # Empty bake pixels and unused atlas regions must carry nearby
            # surface color, so minification never blends black chart gutters.
            if empty.any() and not empty.all():
                nearest=distance_transform_edt(empty,return_distances=False,return_indices=True)
                rgb[empty]=rgb[nearest[0][empty],nearest[1][empty]]
            im=Image.fromarray(rgb,'RGB'); im.save(OUT/(path.stem+'-albedo.png'),optimize=True)
            sink=io.BytesIO(); im.save(sink,format='PNG',optimize=True); data=sink.getvalue()
            assert data[24]==8; image_size=list(im.size)
        view['byteOffset']=len(rebuilt); view['byteLength']=len(data); rebuilt.extend(data); rebuilt.extend(b'\0'*((-len(rebuilt))%4))
    raw=bytes(rebuilt); d['buffers'][0]['byteLength']=len(raw)
    def acc(index):
        a=d['accessors'][index]; v=d['bufferViews'][a['bufferView']]; dt={5126:'<f4',5125:'<u4',5123:'<u2'}[a['componentType']]; cols={'VEC3':3,'VEC2':2,'SCALAR':1}[a['type']]
        return np.frombuffer(raw,dtype=dt,count=a['count']*cols,offset=a.get('byteOffset',0)+v.get('byteOffset',0)).reshape((-1,cols))
    xyz=acc(p['attributes']['POSITION']); normals=acc(p['attributes']['NORMAL']); uv=acc(p['attributes']['TEXCOORD_0']); ix=acc(p['indices']).reshape((-1,3))
    assert np.isfinite(xyz).all() and np.isfinite(normals).all() and np.isfinite(uv).all()
    assert np.allclose(np.linalg.norm(normals,axis=1),1,atol=.01)
    assert (np.ptp(uv,axis=0)>.5).all()
    _,vertex_id=np.unique(np.round(xyz,5),axis=0,return_inverse=True); faces=vertex_id[ix]
    parent=list(range(vertex_id.max()+1))
    def find(i):
        while parent[i]!=i: parent[i]=parent[parent[i]]; i=parent[i]
        return i
    for a,c,e in faces:
        root=find(int(a)); parent[find(int(c))]=root; parent[find(int(e))]=root
    components=Counter(find(int(face[0])) for face in faces)
    largest=max(components.values())
    j=json.dumps(d,separators=(',',':')).encode(); j+=b' '*((-len(j))%4)
    final=struct.pack('<III',0x46546c67,2,12+8+len(j)+8+len(raw))+struct.pack('<II',len(j),0x4e4f534a)+j+struct.pack('<II',len(raw),0x004e4942)+raw
    path.write_bytes(final)
    entry=dict(file=path.name,bytes=len(final),triangles=len(ix),components=len(components),largest_component_triangles=largest,bounds_min=xyz.min(0).tolist(),bounds_max=xyz.max(0).tolist(),extent=np.ptp(xyz,axis=0).tolist(),image_size=image_size,image_mode='RGB',png_bit_depth=8,empty_atlas_pixels_filled=int(empty.sum()))
    report.append(entry); print(json.dumps(entry))
(OUT/'runtime-manifest.json').write_text(json.dumps(dict(assets=report,total_bytes=sum(e['bytes'] for e in report),wall_seconds=round(time.perf_counter()-START,2)),indent=2),encoding='utf-8')
