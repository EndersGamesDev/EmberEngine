"""Passive CPU render of exported props; no desktop input or engine writes."""
import bpy, ctypes, json, os, sys, time
from pathlib import Path
from mathutils import Vector

ROOT=Path(os.environ.get('END_GAME_V9_WORK',str(Path(__file__).resolve().parent)))
name=sys.argv[sys.argv.index('--')+1];work=ROOT/name/'converted';source=work/(name+'.glb')
started=time.perf_counter()
if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(source))
objects=[o for o in bpy.context.scene.objects if o.type=='MESH']
points=[o.matrix_world@Vector(c) for o in objects for c in o.bound_box]
lo=Vector([min(p[i] for p in points) for i in range(3)]);hi=Vector([max(p[i] for p in points) for i in range(3)])
target=(lo+hi)*.5;extent=max(hi-lo)
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=12
scene.render.threads_mode='FIXED';scene.render.threads=4
scene.render.resolution_x=768;scene.render.resolution_y=768;scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX';scene.world=bpy.data.worlds.new('Preview');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.14,.16,.19,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.45
for offset,power in [((1.5,1.6,2),160),((-1.2,-.5,1.5),130),((0,-2,1),100)]:
    bpy.ops.object.light_add(type='AREA',location=target+Vector(offset)*extent);light=bpy.context.object;light.data.energy=power*extent**2;light.data.size=extent
    light.rotation_euler=(target-light.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add();cam=bpy.context.object;cam.data.type='ORTHO';cam.data.ortho_scale=extent*1.3;scene.camera=cam
for view,offset in [('front',(1.0,2.4,.75)),('back',(-1.0,-2.4,.65))]:
    cam.location=target+Vector(offset)*extent;cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(work/('preview-'+view+'.png'));bpy.ops.render.render(write_still=True)
print('V9_PREVIEW '+json.dumps(dict(prop=name,wall_seconds=round(time.perf_counter()-started,3))),flush=True)
