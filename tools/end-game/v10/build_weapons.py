"""Author compact measured weapon meshes around documented hand sockets."""
import bpy,bmesh,ctypes,hashlib,json,math,os,time,struct,zlib
from pathlib import Path
import numpy as np
from mathutils import Vector
ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
ROOT=Path(os.environ.get('END_GAME_V10_WORK',str(Path(__file__).resolve().parent)));OUT=Path(os.environ.get('END_GAME_V10_OUT',str(Path(os.environ.get('EMBER_ROOT',str(Path(__file__).resolve().parents[3])))/'assets/end-game/v10')));OUT.mkdir(parents=True,exist_ok=True)
def B(p):return (p[0],-p[2],p[1])
def mesh(name,verts,faces,kind):
 m=bpy.data.meshes.new(name);m.from_pydata([B(p) for p in verts],[],faces);m.update();o=bpy.data.objects.new(name,m);bpy.context.collection.objects.link(o)
 bm=bmesh.new();bm.from_mesh(m);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(m);bm.free()
 uv=m.uv_layers.new(name='UVMap')
 for poly in m.polygons:
  for index in poly.loop_indices:
   v=m.vertices[m.loops[index].vertex_index].co;u=(v.x*1.8)%1;vv=((v.y+v.z)*2)%1
   uv.data[index].uv=((kind+.04+.92*u)/4,.04+.92*vv)
 return o
def rod(name,x0,x1,r,kind,n=12):
 v=[(x,r*math.cos(2*math.pi*i/n),r*math.sin(2*math.pi*i/n)) for x in [x0,x1] for i in range(n)];f=[tuple(range(n-1,-1,-1)),tuple(range(n,2*n))]
 f += [(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)];return mesh(name,v,f,kind)
def block(name,center,size,kind,bevel=.01):
 c=np.asarray(center);s=np.asarray(size)/2;v=[c+np.array([x,y,z])*s for x,y,z in [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]]
 o=mesh(name,v,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(2,3,7,6),(0,4,7,3),(1,2,6,5)],kind)
 bpy.context.view_layer.objects.active=o;o.select_set(True);mod=o.modifiers.new('WornBevel','BEVEL');mod.width=bevel;mod.segments=2;bpy.ops.object.modifier_apply(modifier=mod.name);o.select_set(False);return o
def blade(name,outline,thickness,kind):
 n=len(outline);v=[(x,y,z) for y in [-thickness/2,thickness/2] for x,z in outline];f=[tuple(range(n-1,-1,-1)),tuple(range(n,2*n))]+[(i,(i+1)%n,(i+1)%n+n,i+n) for i in range(n)]
 return mesh(name,v,f,kind)
def main():
 started=time.perf_counter();rng=np.random.default_rng(100910);pix=np.zeros((256,256,3),dtype=np.uint8)
 colors=[np.array([76,49,28]),np.array([112,117,116]),np.array([89,83,70]),np.array([37,29,23])]
 for k,base in enumerate(colors):
  yy,xx=np.mgrid[:256,:64];noise=rng.normal(0,5,(256,64));grain=7*np.sin(xx*.5+2*np.sin(yy*.03)) if k==0 else 4*np.sin(yy*.75+xx*.5)
  pix[:,k*64:(k+1)*64]=np.clip(base+noise[:,:,None]+grain[:,:,None],0,255)
 def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
 atlas=ROOT/'weapon-atlas.png';atlas.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',256,256,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(b'\x00'+row.tobytes() for row in pix)))+chunk(b'IEND',b''));rows=[]
 for name in ['sword','spear','axe','maul']:
  bpy.ops.wm.read_factory_settings(use_empty=True);parts=[]
  if name=='sword':
   parts=[rod('grip',-.15,.15,.027,3),block('guard',(.16,0,0),(.055,.035,.28),1),rod('pommel',-.205,-.15,.045,1),blade('blade',[(.19,-.043),(.90,-.037),(1.10,0),(.90,.037),(.19,.043)],.023,1)]
  elif name=='spear':
   parts=[rod('shaft',-.50,1.65,.022,0),rod('socket',1.48,1.67,.030,1),blade('point',[(1.58,0),(1.72,-.085),(2.0,0),(1.72,.085)],.022,1)]
  elif name=='axe':
   parts=[rod('haft',-.40,1.10,.033,0),rod('grip',-.25,.15,.036,3),blade('bearded_head',[(.76,-.075),(.94,-.22),(1.25,-.29),(1.22,.20),(.96,.18),(.76,.07)],.075,1),block('eye',(.89,0,0),(.16,.105,.14),1)]
  else:
   parts=[rod('haft',-.40,1.03,.045,0),rod('grip',-.27,.15,.049,3),block('stone_head',(1.0,0,0),(.5,.39,.5),2,.035)]
   for x in [.82,1.17]:parts.append(block('iron_band',(x,0,0),(.055,.414,.52),1,.012))
  for x in [-.09,.04]:parts.append(rod('grip_binding',x,x+.018,.03 if name=='sword' else .037 if name in ['spear','axe'] else .051,1))
  bpy.ops.object.select_all(action='DESELECT')
  for o in parts:o.select_set(True)
  bpy.context.view_layer.objects.active=parts[0];bpy.ops.object.join();o=bpy.context.object;o.name='weapon_'+name;o.data.name=o.name
  bm=bmesh.new();bm.from_mesh(o.data);bmesh.ops.triangulate(bm,faces=list(bm.faces));bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(o.data);bm.free()
  mat=bpy.data.materials.new(name+'_material');mat.use_nodes=True;bs=mat.node_tree.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Roughness'].default_value=.74;bs.inputs['Metallic'].default_value=0;im=bpy.data.images.load(str(atlas));im.pack();tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=im;mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color']);o.data.materials.clear();o.data.materials.append(mat)
  for p in o.data.polygons:p.material_index=0
  path=OUT/('weapon-'+name+'.glb');bpy.ops.export_scene.gltf(filepath=str(path),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
  tip={'sword':1.10,'spear':2.0,'axe':1.25,'maul':1.25}[name];support=[.24,0,0] if name=='spear' else [-.22,0,0] if name in ['axe','maul'] else None
  rows.append(dict(name=name,file=path.name,node=o.name,units='meters',primary_grip=[0,0,0],support_grip=support,tip=[tip,0,0],cutting_point=[tip,0,-.29] if name=='axe' else [tip,0,0],long_axis='+X',broad_face_normal='+Y',edge_axis='Z',triangles=len(o.data.polygons),bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),texture_size=[256,256],texture_gpu_bytes_with_mips=349524))
 (OUT/'weapons.json').write_text(json.dumps(dict(schema_version=1,method='Authored measured rigid weapons; seeded RGB8 wood/steel/stone/leather atlas',weapons=rows,wall_seconds=round(time.perf_counter()-started,3)),indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(rows),flush=True)
if __name__=='__main__':main()
