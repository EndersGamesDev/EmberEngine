"""Preserve full inner thighs after decimation, using each generated thigh map.

The long coat hides these volumes in the source sculpt. Each added closed
ellipsoid overlaps pelvis/knee joints and samples interior UVs of the existing
generated trouser surface. Existing geometry and embedded images stay intact.
"""
import json,math,os,struct,time
from pathlib import Path
import numpy as np

started=time.perf_counter();root=Path(os.environ['END_GAME_V7_WORK']);path=root/'warden.glb'
raw=path.read_bytes();n=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+n])
binary=bytearray(raw[28+n:]);report=[]
def read(index):
    a=g['accessors'][index];v=g['bufferViews'][a['bufferView']]
    dt=np.dtype({5123:'<u2',5125:'<u4',5126:'<f4'}[a['componentType']]);width={'SCALAR':1,'VEC2':2,'VEC3':3}[a['type']]
    return np.ndarray((a['count'],width),dtype=dt,buffer=binary,offset=v.get('byteOffset',0)+a.get('byteOffset',0),strides=(v.get('byteStride',dt.itemsize*width),dt.itemsize)).copy()
def append(array,kind,component):
    binary.extend(b'\0'*((-len(binary))%4));offset=len(binary);payload=array.tobytes();binary.extend(payload)
    view=len(g['bufferViews']);g['bufferViews'].append(dict(buffer=0,byteOffset=offset,byteLength=len(payload)))
    accessor=dict(bufferView=view,componentType=component,count=len(array),type=kind)
    if kind=='VEC3':accessor.update(min=array.min(0).tolist(),max=array.max(0).tolist())
    index=len(g['accessors']);g['accessors'].append(accessor);return index
for node in g['nodes']:
    if not node['name'].startswith('warden_thigh_'):continue
    p=g['meshes'][node['mesh']]['primitives'][0];assert set(p['attributes'])=={'POSITION','NORMAL','TEXCOORD_0'}
    pos=read(p['attributes']['POSITION']);norm=read(p['attributes']['NORMAL']);uv=read(p['attributes']['TEXCOORD_0']);indices=read(p['indices']).reshape(-1,3)
    if pos[:,1].max()>.94 and pos[:,1].min()<.55:continue
    side=1 if node['name'].endswith('_r') else -1
    center=np.array([side*.105,.755,0]);radii=np.array([.082,.250,.092]);points=[center+np.array([0,.25,0])]
    for row in range(1,6):
        theta=math.pi*row/6
        for col in range(12):
            phi=math.tau*col/12;points.append(center+radii*np.array([math.sin(theta)*math.cos(phi),math.cos(theta),math.sin(theta)*math.sin(phi)]))
    points.append(center-np.array([0,.25,0]));points=np.array(points);faces=[]
    for col in range(12):faces.append([0,1+col,1+(col+1)%12])
    for row in range(4):
        for col in range(12):
            a=1+row*12+col;b=1+row*12+(col+1)%12;c=a+12;d=b+12;faces.extend([[a,c,b],[b,c,d]])
    for col in range(12):faces.append([61,49+(col+1)%12,49+col])
    source_centers=pos[indices].mean(1);source_uv=uv[indices].mean(1)
    added_pos=[];added_norm=[];added_uv=[]
    for face in faces:
        f=points[face].copy();mid=f.mean(0)
        if np.dot(np.cross(f[1]-f[0],f[2]-f[0]),mid-center)<0:f=f[[0,2,1]]
        query=mid.copy();query[1]=.59
        nearest=np.argmin(np.sum((source_centers-query)**2,axis=1));sample=source_uv[nearest]
        for vertex in f:
            normal=(vertex-center)/(radii*radii);normal/=np.linalg.norm(normal)
            added_pos.append(vertex);added_norm.append(normal);added_uv.append(sample)
    count=len(pos);new_indices=np.arange(count,count+len(added_pos),dtype='<u4').reshape(-1,3)
    p['attributes']['POSITION']=append(np.concatenate([pos,added_pos]).astype('<f4'),'VEC3',5126)
    p['attributes']['NORMAL']=append(np.concatenate([norm,added_norm]).astype('<f4'),'VEC3',5126)
    p['attributes']['TEXCOORD_0']=append(np.concatenate([uv,added_uv]).astype('<f4'),'VEC2',5126)
    p['indices']=append(np.concatenate([indices,new_indices]).astype('<u4').reshape(-1,1),'SCALAR',5125)
    report.append(dict(node=node['name'],added_triangles=len(faces)))
binary.extend(b'\0'*((-len(binary))%4));g['buffers'][0]['byteLength']=len(binary)
encoded=json.dumps(g,separators=(',',':')).encode();encoded+=b' '*((-len(encoded))%4)
result=struct.pack('<4sII',b'glTF',2,28+len(encoded)+len(binary))
result+=struct.pack('<I4s',len(encoded),b'JSON')+encoded+struct.pack('<I4s',len(binary),b'BIN\0')+binary
path.write_bytes(result);print(json.dumps(dict(restored=report,wall_seconds=round(time.perf_counter()-started,4)),indent=2))
