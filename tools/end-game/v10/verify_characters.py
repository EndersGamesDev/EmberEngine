"""Validate decoded final rigid GLBs, including actual central torso coverage."""
from pathlib import Path
from io import BytesIO
import argparse,hashlib,json,os,struct,time
import numpy as np
from PIL import Image
ROOT=Path(os.environ.get('END_GAME_V10_WORK',str(Path(__file__).resolve().parent)))
def verify(name):
 started=time.perf_counter();work=ROOT/name;path=work/(name+'.glb');raw=path.read_bytes();magic,version,total=struct.unpack_from('<4sII',raw);assert (magic,version,total)==(b'glTF',2,len(raw));n=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+n]);binary=raw[28+n:]
 def data(index):
  a=g['accessors'][index];v=g['bufferViews'][a['bufferView']];dt=np.dtype({5123:'<u2',5125:'<u4',5126:'<f4'}[a['componentType']]);width={'SCALAR':1,'VEC2':2,'VEC3':3}[a['type']]
  return np.ndarray((a['count'],width),dt,buffer=binary,offset=v.get('byteOffset',0)+a.get('byteOffset',0),strides=(v.get('byteStride',dt.itemsize*width),dt.itemsize)).copy()
 rig=json.loads((work/(name+'-rig.json')).read_text());assert len(rig['parts'])==18;assert {x['name'] for x in rig['parts']}=={x['name'] for x in g['nodes']};assert not g.get('skins') and not g.get('animations')
 images=[]
 for im in g['images']:
  v=g['bufferViews'][im['bufferView']];png=binary[v.get('byteOffset',0):v.get('byteOffset',0)+v['byteLength']];assert png[:8]==b'\x89PNG\r\n\x1a\n' and png[24:26]==bytes([8,2]);p=Image.open(BytesIO(png));assert p.mode=='RGB';a=np.array(p);assert a.std()>3
  w,h=p.size;mw,mh=w,h;gpu=0
  while True:
   gpu+=mw*mh*4
   if mw==mh==1:break
   mw=max(1,mw//2);mh=max(1,mh//2)
  images.append(dict(size=[w,h],bytes=len(png),gpu_bytes_with_mips=gpu,std=float(a.std())))
 rows=[]
 for node in g['nodes']:
  assert node.get('translation',[0,0,0])==[0,0,0] and node.get('rotation',[0,0,0,1])==[0,0,0,1] and node.get('scale',[1,1,1])==[1,1,1] and not node.get('matrix')
  pp=g['meshes'][node['mesh']]['primitives'];assert len(pp)==1;p=pp[0];pos=data(p['attributes']['POSITION']);normal=data(p['attributes']['NORMAL']);uv=data(p['attributes']['TEXCOORD_0']);idx=data(p['indices']).reshape(-1,3)
  assert np.isfinite(pos).all() and np.isfinite(normal).all() and np.isfinite(uv).all();assert idx.max()<len(pos);assert uv.min()>=0 and uv.max()<=1;assert np.allclose(np.linalg.norm(normal,axis=1),1,atol=1e-3)
  tri=pos[idx];cross=np.cross(tri[:,1]-tri[:,0],tri[:,2]-tri[:,0]);areas=np.linalg.norm(cross,axis=1)*.5;assert (areas>1e-12).all(),(node['name'],'degenerate')
  mat=g['materials'][p['material']];assert mat.get('alphaMode','OPAQUE')=='OPAQUE';pbr=mat['pbrMetallicRoughness'];assert pbr.get('baseColorFactor',[1,1,1,1])==[1,1,1,1];image=g['textures'][pbr['baseColorTexture']['index']]['source']
  row=dict(name=node['name'],triangles=len(idx),bounds_min=pos.min(0).tolist(),bounds_max=pos.max(0).tolist(),surface_area=float(areas.sum()),image=image)
  if name=='cyclops' and node['name']=='enemy_pelvis':
   assert np.abs(pos[:,0]).max()<.40,(name,'arm surfaces leaked into pelvis',row)
  if name=='cyclops' and node['name'].startswith('enemy_coat'):
   assert np.abs(pos[:,0]).max()<.46,(name,'hand surfaces leaked into coat',row)
  if node['name']=='enemy_torso':
   assert areas.sum()>.45,(name,'torso area missing',areas.sum());assert np.ptp(pos,axis=0)[0]>.3 and np.ptp(pos,axis=0)[1]>.35 and np.ptp(pos,axis=0)[2]>.15
   a,b,c=tri[:,0,:2],tri[:,1,:2],tri[:,2,:2];v0=b-a;v1=c-a;den=v0[:,0]*v1[:,1]-v0[:,1]*v1[:,0];good=np.abs(den)>1e-10;covered=[]
   for x in np.linspace(-.12,.12,13):
    for y in np.linspace(1.10,1.46,19):
     v2=np.array([x,y])-a;u=np.divide(v2[:,0]*v1[:,1]-v2[:,1]*v1[:,0],den,out=np.zeros_like(den),where=good);v=np.divide(v0[:,0]*v2[:,1]-v0[:,1]*v2[:,0],den,out=np.zeros_like(den),where=good);covered.append(bool(np.any(good&(u>=0)&(v>=0)&(u+v<=1))))
   row['frontal_core_coverage']=sum(covered)/len(covered);assert row['frontal_core_coverage']>.94,(name,'torso coverage',row)
  rows.append(row)
 total_tri=sum(r['triangles'] for r in rows);gpu=sum(images[r['image']]['gpu_bytes_with_mips'] for r in rows);assert total_tri<=15000 and gpu<=6*1024*1024,(total_tri,gpu)
 provenance={stage:json.loads((work/(stage+'-result.json')).read_text()) for stage in ['image','cutout','mesh']};manifest=dict(schema_version=1,file=path.name,sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw),triangles=total_tri,nodes=18,cloned_texture_gpu_bytes_with_mips=gpu,parts=rows,images=images,provenance=provenance,sidecar=name+'-rig.json',sidecar_sha256=hashlib.sha256((work/(name+'-rig.json')).read_bytes()).hexdigest(),verification_seconds=round(time.perf_counter()-started,3),verification='Decoded GLB names, identity transforms, finite attributes, normalized normals, index/triangle/UV validity, opaque white materials, RGB8 basecolor images, memory/triangle budgets and central torso area/coverage. Native animation check remains separate.')
 sources=list((work/'mesh').glob('*.glb'));assert len(sources)==1
 src=sources[0];manifest['source_mesh']={'file':src.name,'bytes':src.stat().st_size,'sha256':hashlib.sha256(src.read_bytes()).hexdigest()}
 manifest['conversion_method']=rig['source_method'];manifest['limits']=rig['limits']
 (work/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps({k:manifest[k] for k in ['file','bytes','triangles','cloned_texture_gpu_bytes_with_mips','verification_seconds']}))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('character');verify(p.parse_args().character)
