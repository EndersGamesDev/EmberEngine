"""Inspect the actual generated source from front and back, without altering it."""
import bpy
from pathlib import Path
import json,os,time,math
import numpy as np
from mathutils import Vector,Quaternion,Matrix

started=time.perf_counter()
source=Path(os.environ['END_GAME_V10_SOURCE'])
out=Path(os.environ['END_GAME_V10_WORK']);out.mkdir(parents=True,exist_ok=True)
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(source))
objects=[o for o in bpy.context.scene.objects if o.type=='MESH']
for o in objects:
    o.data.transform(o.matrix_world);o.parent=None;o.matrix_world=Matrix.Identity(4)
vertices=np.concatenate([np.array([v.co[:] for v in o.data.vertices]) for o in objects])
lo=vertices.min(0);hi=vertices.max(0);axis=int(np.argmax(hi-lo))
rotation=Vector(np.eye(3)[axis]).rotation_difference(Vector((0,0,1))).to_matrix()
vertices=np.array([rotation@Vector(v) for v in vertices]);lo=vertices.min(0);hi=vertices.max(0)
scale=1.865/(hi[2]-lo[2]);shift=np.array([(lo[0]+hi[0])*.5,(lo[1]+hi[1])*.5,lo[2]])
for o in objects:
    a=np.array([rotation@v.co for v in o.data.vertices]);a=(a-shift)*scale
    o.data.vertices.foreach_set('co',a.astype(np.float32).ravel());o.data.update()
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=8
scene.render.threads_mode='FIXED';scene.render.threads=2
scene.render.resolution_x=640;scene.render.resolution_y=900;scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX';scene.world=bpy.data.worlds.new('Inspection');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.20,.22,.24,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.5
target=Vector((0,0,.96))
for location,power in [((2,3,3),260),((-2,-1,2),200),((0,-3,3),180)]:
    bpy.ops.object.light_add(type='AREA',location=location);light=bpy.context.object
    light.data.energy=power;light.data.size=2.5;light.rotation_euler=(target-light.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add();camera=bpy.context.object;camera.data.type='ORTHO';camera.data.ortho_scale=2.05;scene.camera=camera
for name,location in [('front',(0,-3.8,1.15)),('back',(0,3.8,1.15))]:
    camera.location=location;camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(out/f'source-{name}.png');bpy.ops.render.render(write_still=True)
record=dict(source=str(source),meshes=len(objects),vertices=sum(len(o.data.vertices) for o in objects),
            original_long_axis=axis,source_center=shift.tolist(),source_to_meters=float(scale),
            normalized_blender_bounds_min=((lo-shift)*scale).tolist(),normalized_blender_bounds_max=((hi-shift)*scale).tolist(),
            seconds=round(time.perf_counter()-started,3))
(out/'source-inspection.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
print('V10_SOURCE_INSPECTION',json.dumps(record),flush=True)
