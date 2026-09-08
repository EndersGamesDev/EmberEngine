"""Companion greatsword: same atlas/helpers as wolf hands, with actual 28mm grip."""
import sys,time,json,math
from pathlib import Path
import os
sys.path.insert(0,str(Path(__file__).resolve().parent))
import v3_build_hands as kit
import bpy,numpy as np
from mathutils import Vector
START=time.perf_counter();OUT=Path(os.environ['END_GAME_V3_ASSET_WORK'])
bpy.ops.wm.read_factory_settings(use_empty=True)
mat=bpy.data.materials.new('SharedWolfArmor');mat.use_nodes=True;bs=mat.node_tree.nodes['Principled BSDF']
bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Metallic'].default_value=.25;bs.inputs['Roughness'].default_value=.65
tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(str(OUT/'wolf-armor-atlas.png'));tex.image.pack();mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
g=kit.geom()

def bevel_slab(outline,thickness,bevel=.18,material='iron'):
    outline=np.array(outline);center=outline.mean(axis=0);n=len(outline)
    vertices=[]
    for y,inset in [(-thickness*.5,bevel),(-thickness*.12,0),(thickness*.12,0),(thickness*.5,bevel)]:
        points=center+(outline-center)*np.array([1-inset*.04,1-inset])
        vertices.extend([(x,y,z) for x,z in points])
    kit.add(g,vertices,[tuple(range(n)),tuple(reversed(range(3*n,4*n)))],material)
    for row in range(3):
        faces=[]
        for i in range(n):j=(i+1)%n;faces.append((row*n+i,(row+1)*n+i,(row+1)*n+j,row*n+j))
        kit.add(g,vertices,faces,'edge' if row!=1 else material)

# Straight-edged enormous slab; beveled silhouette, unsharpened flat tip.
bevel_slab([(.020,-.070),(.135,-.130),(1.40,-.105),(1.78,-.055),(1.85,-.006),(1.85,.006),(1.78,.055),(1.40,.105),(.135,.130),(.020,.070)],.026)
# A dark narrow fuller reads in the single base-color pipeline without normal maps.
for y in (-.0131,.0131):
    kit.box(g,(.85,y,0),(1.38,.0006,.015),'leather')
    for z in (-.030,.030):kit.box(g,(.42,y*1.06,z),(.25,.0007,.0013),'edge')

# Swept blunt crossguard, and a restrained wolf medallion at its center.
bevel_slab([(-.025,-.212),(.035,-.190),(.010,-.065),(.025,-.032),(.025,.032),(.010,.065),(.035,.190),(-.025,.212),(-.048,.175),(-.037,.055),(-.050,0),(-.037,-.055),(-.048,-.175)],.042,.22)
for z in (-.172,.172):kit.ellipsoid(g,(-.010,.018,z),(.010,.008,.016),'brass',8,4)
for y in (-.024,.024):
    kit.ellipsoid(g,(-.002,y,0),(.028,.006,.027),'iron',10,4)
    # Three chevron ridges form a compact wolf brow and snout on both broad faces.
    for x,w in [(-.010,.030),(.001,.021),(.011,.010)]:kit.box(g,(x,y*1.1,0),(.004,.003,w),'edge')

# The grip's working radius exactly matches the hand proxy; raised wrap remains <=14.2mm.
kit.capsule(g,-.320,-.025,.014,.014,'leather',taper=1,sides=12)
for i in range(10):kit.torus(g,(-.303+i*.027,0,0),.013,.013,.0012,'iron','YZ',12,5)
for x in (-.318,-.029):kit.torus(g,(x,0,0),.0145,.0145,.002,'edge','YZ',16,6)
kit.capsule(g,-.355,-.320,.010,.010,'iron',taper=1,sides=10)
kit.ellipsoid(g,(-.370,0,0),(.025,.026,.026),'iron',12,6)
kit.torus(g,(-.370,0,0),.025,.025,.002,'edge','YZ',16,5)
obj=kit.object_from_geom('wolf_greatsword',g,mat)
bpy.ops.object.select_all(action='DESELECT');obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT/'wolf-greatsword.glb'),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'wolf-greatsword-editable.blend'))

scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=16;scene.render.threads_mode='FIXED';scene.render.threads=4
scene.render.resolution_x=1400;scene.render.resolution_y=620;scene.render.resolution_percentage=100;scene.view_settings.view_transform='AgX';scene.render.image_settings.file_format='PNG'
scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.18,.21,.25,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.5
target=np.array([.72,0,0]);eye=target+np.array([.16,2.2,.80]);bpy.ops.object.camera_add(location=kit.B(eye));camera=bpy.context.object;camera.data.type='ORTHO';camera.data.ortho_scale=2.5;camera.rotation_euler=(Vector(kit.B(target))-camera.location).to_track_quat('-Z','Y').to_euler();scene.camera=camera
for offset,power in [((0,1.8,1),250),((.2,-1,-1),90)]:
    bpy.ops.object.light_add(type='AREA',location=kit.B(target+np.array(offset)));lamp=bpy.context.object;lamp.data.energy=power;lamp.data.size=2;lamp.rotation_euler=(Vector(kit.B(target))-lamp.location).to_track_quat('-Z','Y').to_euler()
scene.render.filepath=str(OUT/'wolf-greatsword.png');bpy.ops.render.render(write_still=True)
rig=dict(schema_version=1,units='meters',node='wolf_greatsword',axes='X blade/tip forward, Y thickness, Z blade width',guard_origin=[0,0,0],blade_tip=[1.85,0,0],grip_axis=[1,0,0],grip_span_x=[-.32,-.025],grip_radius=.014,grip_center=[-.1725,0,0],primary_grip=[-.13,0,0],secondary_grip=[-.25,0,0],pommel_center=[-.37,0,0],mount_formula='Choose weapon rotation R mapping +X to hand power_grip.axis. Weapon translation = hand_socket_world - R * primary_grip. Roll around grip axis is a presentation choice.',build_seconds=round(time.perf_counter()-START,2))
(OUT/'greatsword-rig.json').write_text(json.dumps(rig,indent=2),encoding='utf-8');print('SWORD_BUILD_SECONDS',rig['build_seconds'],flush=True)
