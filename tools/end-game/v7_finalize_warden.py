"""Remove tiny disconnected sculpt remnants without changing baked UVs/normals.

Run after v7_convert_warden.py, before v7_verify_warden.py. Only index accessors
change; the byte-exact baked PNGs and vertex attributes remain intact.
"""
import hashlib,json,os,struct,time
from pathlib import Path
import numpy as np

started=time.perf_counter();root=Path(os.environ['END_GAME_V7_WORK'])
path=root/'warden.glb';raw=path.read_bytes();n=struct.unpack_from('<I',raw,12)[0]
g=json.loads(raw[20:20+n]);binary=bytearray(raw[28+n:]);report=[]

def data(index):
    a=g['accessors'][index];v=g['bufferViews'][a['bufferView']]
    dtype=np.dtype({5123:'<u2',5125:'<u4',5126:'<f4'}[a['componentType']])
    width={'SCALAR':1,'VEC2':2,'VEC3':3}[a['type']]
    return np.ndarray((a['count'],width),dtype=dtype,buffer=binary,
        offset=v.get('byteOffset',0)+a.get('byteOffset',0),
        strides=(v.get('byteStride',dtype.itemsize*width),dtype.itemsize))

for node in g['nodes']:
    if node['name'] in ['warden_chair','warden_knife']:continue
    primitive=g['meshes'][node['mesh']]['primitives'][0]
    pos=data(primitive['attributes']['POSITION']);indices=data(primitive['indices']).reshape(-1,3)
    welded,mapping=np.unique(np.round(pos,5),axis=0,return_inverse=True)
    triangles=mapping[indices];parent=np.arange(len(welded))
    def find(i):
        while parent[i]!=i:
            parent[i]=parent[parent[i]];i=parent[i]
        return i
    for a,b,c in triangles:
        a,b,c=find(a),find(b),find(c);parent[b]=a;parent[c]=a
    labels=np.array([find(i) for i in range(len(welded))]);face_labels=labels[triangles[:,0]]
    keep=np.ones(len(indices),dtype=bool)
    for component in np.unique(face_labels):
        faces=face_labels==component;points=welded[labels==component]
        if faces.sum()<64 and np.linalg.norm(np.ptp(points,axis=0))<.075:
            keep[faces]=False
    if keep.all():continue
    kept=indices[keep].copy();removed=int((~keep).sum())
    indices[:len(kept)]=kept
    accessor=g['accessors'][primitive['indices']];accessor['count']=int(kept.size)
    if 'min' in accessor:accessor['min']=[int(kept.min())]
    if 'max' in accessor:accessor['max']=[int(kept.max())]
    report.append(dict(node=node['name'],removed_triangles=removed))

encoded=json.dumps(g,separators=(',',':')).encode();encoded+=b' '*((-len(encoded))%4)
result=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
result+=struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
path.write_bytes(result)
print(json.dumps(dict(removed=report,sha256=hashlib.sha256(result).hexdigest(),
    wall_seconds=round(time.perf_counter()-started,4)),indent=2))
