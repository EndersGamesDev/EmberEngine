"""Render the final exported GLB for offline inspection, without modifying it."""
import bpy,os,time
from pathlib import Path
from mathutils import Vector
started=time.perf_counter();root=Path(os.environ['END_GAME_V7_WORK'])
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(root/'warden.glb'))
bpy.data.objects['warden_chair'].hide_render=True
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=16
scene.render.threads_mode='FIXED';scene.render.threads=4
scene.render.resolution_x=800;scene.render.resolution_y=1100;scene.render.resolution_percentage=100
scene.view_settings.view_transform='AgX'
scene.world=bpy.data.worlds.new('Preview');scene.world.use_nodes=True
scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.20,.22,.24,1)
scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
target=Vector((0,0,.98))
for p,power in [((2,3,3),260),((-2,1,2),180),((0,-3,3),180)]:
    bpy.ops.object.light_add(type='AREA',location=p);o=bpy.context.object;o.data.energy=power;o.data.size=2.5
    o.rotation_euler=(target-o.location).to_track_quat('-Z','Y').to_euler()
bpy.ops.object.camera_add(location=(2.2,4.4,1.7));camera=bpy.context.object
camera.data.type='ORTHO';camera.data.ortho_scale=2.12
camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler();scene.camera=camera
scene.render.filepath=str(root/'warden-standing-preview.png');bpy.ops.render.render(write_still=True)
print('V7_PREVIEW_WALL_SECONDS',round(time.perf_counter()-started,3),flush=True)
