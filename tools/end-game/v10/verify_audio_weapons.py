"""Check served audio inputs and measured runtime weapon meshes without a renderer."""
from pathlib import Path
from io import BytesIO
import hashlib,json,os,struct,time,wave
import numpy as np
from PIL import Image
ROOT=Path(os.environ.get('END_GAME_V10_OUT',str(Path(os.environ.get('EMBER_ROOT',str(Path(__file__).resolve().parents[3])))/'assets/end-game/v10')))
def main():
 began=time.perf_counter();weapons=json.loads((ROOT/'weapons.json').read_text());rows=[]
 for item in weapons['weapons']:
  path=ROOT/item['file'];raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==item['sha256'];n=struct.unpack_from('<I',raw,12)[0];g=json.loads(raw[20:20+n]);binary=raw[28+n:];assert len(g['nodes'])==len(g['meshes'])==len(g['images'])==1;assert g['nodes'][0]['name']==item['node']
  p=g['meshes'][0]['primitives'][0];attrs=[]
  for key,width in [('POSITION',3),('NORMAL',3),('TEXCOORD_0',2)]:
   a=g['accessors'][p['attributes'][key]];v=g['bufferViews'][a['bufferView']];x=np.ndarray((a['count'],width),'<f4',buffer=binary,offset=v.get('byteOffset',0)+a.get('byteOffset',0),strides=(v.get('byteStride',width*4),4));assert np.isfinite(x).all();attrs.append(x)
  pos,norm,uv=attrs;assert np.allclose(np.linalg.norm(norm,axis=1),1,atol=1e-3);assert uv.min()>=0 and uv.max()<=1
  assert abs(float(pos[:,0].max())-item['tip'][0])<.003,(item['name'],pos[:,0].max());assert item['primary_grip']==[0,0,0]
  image=g['images'][0];v=g['bufferViews'][image['bufferView']];png=binary[v.get('byteOffset',0):v.get('byteOffset',0)+v['byteLength']];im=Image.open(BytesIO(png));assert im.mode=='RGB' and im.size==(256,256) and png[24]==8
  m=g['materials'][p['material']];assert m.get('alphaMode','OPAQUE')=='OPAQUE';assert m['pbrMetallicRoughness'].get('baseColorFactor',[1,1,1,1])==[1,1,1,1]
  rows.append(dict(file=item['file'],bounds_min=pos.min(0).tolist(),bounds_max=pos.max(0).tolist(),sha256=item['sha256']))
 manifest=json.loads((ROOT/'voice-manifest.json').read_text());audio=[]
 for row in manifest['files']+[manifest['ambience']]:
  path=ROOT/row['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==row['sha256']
  with wave.open(str(path),'rb') as w:assert (w.getnchannels(),w.getsampwidth(),w.getframerate())==(1,2,row['sample_rate']);y=np.frombuffer(w.readframes(w.getnframes()),'<i2');assert len(y)==row['frames']
  assert np.abs(y.astype(np.int32)).max()<32767;assert np.sqrt(np.mean(y.astype(float)**2))>10;audio.append(dict(file=row['file'],frames=len(y),seconds=row['duration'],nonclipped=True))
 result=dict(weapons=rows,audio=audio,wall_seconds=round(time.perf_counter()-began,4));(ROOT/'audio-weapons-verification.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(result))
if __name__=='__main__':main()
