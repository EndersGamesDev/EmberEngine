"""Convert a generated static prop into one budgeted, textured Ember primitive."""
import bpy, bmesh
import ctypes, hashlib, json, math, os, subprocess, sys, time
from pathlib import Path
from mathutils import Matrix, Vector
import numpy as np

TOOLS=Path(__file__).resolve().parent
ROOT=Path(os.environ.get('END_GAME_V9_WORK',str(TOOLS)))
sys.path.insert(0,str(TOOLS))
from generate_kit import PROPS

started=time.perf_counter()
if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
name=sys.argv[sys.argv.index('--')+1];spec=PROPS[name];work=ROOT/name
source=next((work/'mesh').glob('*.glb'));out=work/'converted';out.mkdir(exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(source))
objects=[o for o in bpy.context.scene.objects if o.type=='MESH']
for o in objects:
    o.data.transform(o.matrix_world);o.parent=None;o.matrix_world=Matrix.Identity(4)
    if o.data.uv_layers:o.data.uv_layers.active.name='UVMap'
    o.select_set(True)
bpy.context.view_layer.objects.active=objects[0]
bpy.ops.object.join();obj=bpy.context.object;obj.name=name.replace('-','_');obj.data.name=obj.name
assert len(obj.data.uv_layers)==1,'Expected exactly one source UV layer'
base_images=[]
for mat in obj.data.materials:
    bs=mat.node_tree.nodes.get('Principled BSDF')
    links=bs.inputs['Base Color'].links
    assert len(links)==1 and links[0].from_node.type=='TEX_IMAGE','Expected a base-colour image'
    base_images.append(links[0].from_node.image)
assert len({im.as_pointer() for im in base_images})==1,'Source needs an explicit atlas bake before joining'
image=base_images[0]
positions=np.array([v.co[:] for v in obj.data.vertices]);lo=positions.min(0);hi=positions.max(0)
scale=spec['height_m']/float(hi[2]-lo[2]);center=np.array([(lo[0]+hi[0])*.5,(lo[1]+hi[1])*.5,lo[2]])
positions=(positions-center)*scale
obj.data.vertices.foreach_set('co',positions.astype(np.float32).ravel());obj.data.update()
bm=bmesh.new();bm.from_mesh(obj.data)
bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.000002)
bmesh.ops.dissolve_degenerate(bm,edges=list(bm.edges),dist=.0000001)
bmesh.ops.triangulate(bm,faces=list(bm.faces));bm.to_mesh(obj.data);bm.free()
before=len(obj.data.polygons)
if before>spec['triangles']:
    mod=obj.modifiers.new('StaticPropBudget','DECIMATE');mod.ratio=spec['triangles']/before;mod.use_collapse_triangulate=True
    bpy.ops.object.modifier_apply(modifier=mod.name)
bm=bmesh.new();bm.from_mesh(obj.data);bmesh.ops.triangulate(bm,faces=list(bm.faces));bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
bm.to_mesh(obj.data);bm.free();obj.data.validate();obj.data.update()
for poly in obj.data.polygons:poly.use_smooth=True;poly.material_index=0
obj.data.set_sharp_from_angle(angle=math.radians(55))
image.scale(768,768);image.colorspace_settings.name='sRGB'
scene=bpy.context.scene;scene.render.image_settings.file_format='PNG';scene.render.image_settings.color_mode='RGB';scene.render.image_settings.color_depth='8'
scene.view_settings.view_transform='Standard';scene.view_settings.look='None';scene.view_settings.exposure=0;scene.view_settings.gamma=1
image.save_render(filepath=str(out/'albedo.png'),scene=scene)
# Packed imported images may save their original pixels despite image.scale().
# Normalize the actual bytes before reloading, so the exported GLB is RGB8/768.
subprocess.run(['C:/hy3d/venv/Scripts/python.exe','-c',
    'from PIL import Image;import sys;p=sys.argv[1];im=Image.open(p).convert("RGB").resize((768,768),Image.Resampling.LANCZOS);im.save(p,optimize=True)',str(out/'albedo.png')],check=True)
if name=='courtyard-fountain':
    floor=[];uv=obj.data.uv_layers.active.data
    for poly in obj.data.polygons:
        c=poly.center
        radial=math.hypot(c.x,c.y)
        inward=c.x*poly.normal.x+c.y*poly.normal.y<-.05
        if radial<1.02 and .40<c.z<1.10 and (inward or poly.normal.z>.10):
            floor.append([list(uv[i].uv) for i in poly.loop_indices])
    mask_path=out/'dry-floor-uv.json';mask_path.write_text(json.dumps(floor),encoding='utf-8')
    subprocess.run(['C:/hy3d/venv/Scripts/python.exe',str(TOOLS/'dry_basin_albedo.py'),str(out/'albedo.png'),str(mask_path)],check=True)
subprocess.run(['C:/hy3d/venv/Scripts/python.exe',str(TOOLS/'warm_stone_albedo.py'),str(out/'albedo.png')],check=True)
image=bpy.data.images.load(str(out/'albedo.png'),check_existing=False)
mat=bpy.data.materials.new(obj.name+'_limestone');mat.use_nodes=True
bs=mat.node_tree.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Roughness'].default_value=.9;bs.inputs['Metallic'].default_value=0
tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=image;mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
obj.data.materials.clear();obj.data.materials.append(mat)
bpy.ops.object.select_all(action='DESELECT');obj.select_set(True);bpy.context.view_layer.objects.active=obj
output=out/(name+'.glb')
bpy.ops.export_scene.gltf(filepath=str(output),export_format='GLB',use_selection=True,export_yup=True,export_normals=True,export_texcoords=True,export_materials='EXPORT',export_image_format='AUTO',export_animations=False,export_cameras=False,export_lights=False)
positions=np.array([v.co[:] for v in obj.data.vertices]);eng=np.stack([positions[:,0],positions[:,2],-positions[:,1]],axis=1)
record=dict(prop=name,source=str(source),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),output=str(output),bytes=output.stat().st_size,sha256=hashlib.sha256(output.read_bytes()).hexdigest(),source_triangles=before,triangles=len(obj.data.polygons),height_m=spec['height_m'],engine_bounds_min=eng.min(0).tolist(),engine_bounds_max=eng.max(0).tolist(),base_anchor='center of ground contact',axes='engine +Y up; concept front faces -Z',albedo_pixels=[768,768],scalar_surface=dict(roughness=.9,metallic=0),wall_seconds=round(time.perf_counter()-started,3))
(out/'manifest.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
print('V9_CONVERSION '+json.dumps(record),flush=True)
