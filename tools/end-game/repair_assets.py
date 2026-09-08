"""Voxel-reconstruct generated meshes, then bake original albedo onto clean UVs."""
import bpy,bmesh,json,time,sys,math,os
import numpy as np
from mathutils import Matrix,Vector
from pathlib import Path

ROOT=Path(os.environ["END_GAME_ASSET_WORK"]).resolve()
OUT=ROOT/'repaired'; OUT.mkdir(exist_ok=True)
START=time.perf_counter()
NAMES={'hero-wolf-form':'wolf','hero-werewolf-form':'werewolf','greatsword':'sword','sleeping_warden':'warden','wooden_cot':'cot','iron_bars_segment':'bars','wall_torch':'torch'}
specs=json.loads((ROOT/'asset-specs.json').read_text())
if '--' in sys.argv:
    wanted=sys.argv[sys.argv.index('--')+1:]
    if wanted: specs=[s for s in specs if NAMES[s['name']] in wanted]
report=[]

def positions(obj):
    a=np.zeros(len(obj.data.vertices)*3,dtype=np.float32); obj.data.vertices.foreach_get('co',a); return a.reshape((-1,3))

def material(name,image):
    mat=bpy.data.materials.new(name); mat.use_nodes=True
    bsdf=mat.node_tree.nodes['Principled BSDF']; bsdf.inputs['Roughness'].default_value=.85; bsdf.inputs['Metallic'].default_value=0
    bsdf.inputs['Base Color'].default_value=(1,1,1,1)
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage'); tex.image=image
    mat.node_tree.links.new(tex.outputs['Color'],bsdf.inputs['Base Color'])
    mat.node_tree.nodes.active=tex; tex.select=True
    return mat

def preview(obj,name):
    scene=bpy.context.scene; scene.render.resolution_x=480; scene.render.resolution_y=480; scene.render.resolution_percentage=100
    scene.world=bpy.data.worlds.new('World'); scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.18,.20,.24,1); scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
    scene.view_settings.view_transform='Standard'; scene.render.image_settings.file_format='PNG'
    a=positions(obj); center=Vector((a.min(0)+a.max(0))/2); size=max(a.max(0)-a.min(0))
    bpy.ops.object.camera_add(location=center+Vector((2,-2,1.1))*size); cam=bpy.context.object; cam.data.type='ORTHO'; cam.data.ortho_scale=size*1.3
    cam.rotation_euler=(center-cam.location).to_track_quat('-Z','Y').to_euler(); scene.camera=cam
    for offset,power in [((1,-1,2),850),((-1,1,1),550)]:
        bpy.ops.object.light_add(type='AREA',location=center+Vector(offset)*size); light=bpy.context.object; light.data.energy=power; light.data.shape='DISK'; light.data.size=size*2; light.rotation_euler=(center-light.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(OUT/(name+'-preview.png')); bpy.ops.render.render(write_still=True)

for spec in specs:
    t=time.perf_counter(); name=NAMES[spec['name']]
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene=bpy.context.scene; scene.render.engine='CYCLES'; scene.cycles.device='CPU'; scene.cycles.samples=8; scene.render.threads_mode='FIXED'; scene.render.threads=4
    bpy.ops.import_scene.gltf(filepath=spec['source'])
    source=[o for o in scene.objects if o.type=='MESH'][0]; source.name='BakeSource'
    bpy.context.view_layer.objects.active=source; bpy.ops.object.select_all(action='DESELECT'); source.select_set(True)
    world=source.matrix_world.copy(); source.parent=None; source.matrix_world=world
    bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
    # Original source textures are decoded by the importer; only their base-color
    # graph feeds the color bake. No source normals/PBR/detail maps are baked.
    source_image=None
    for mat in source.data.materials:
        bsdf=next(n for n in mat.node_tree.nodes if n.type=='BSDF_PRINCIPLED')
        links=list(bsdf.inputs['Base Color'].links)
        if links and links[0].from_node.type=='TEX_IMAGE': source_image=links[0].from_node.image
    assert source_image is not None and source_image.size[0]>0
    source_mat=material('SourceBaseColor',source_image)
    source.data.materials.clear(); source.data.materials.append(source_mat)
    for polygon in source.data.polygons: polygon.material_index=0
    target=source.copy(); target.data=source.data.copy(); scene.collection.objects.link(target); target.name=name
    bpy.ops.object.select_all(action='DESELECT'); target.select_set(True); bpy.context.view_layer.objects.active=target
    target.modifiers.clear()
    if name in ('wolf','cot'):
        # These source assets are open sheets. Give every sheet a thin closed
        # volume before voxel reconstruction or the signed-distance interior
        # disappears, leaving only seams and edge fragments.
        shell=target.modifiers.new('CloseSourceSheets','SOLIDIFY'); shell.thickness=.012; shell.offset=0; shell.use_even_offset=True
        bpy.ops.object.modifier_apply(modifier=shell.name)
    remesh=target.modifiers.new('ContinuousSurface','REMESH'); remesh.mode='VOXEL'; remesh.voxel_size=.004 if name in ('sword','bars','torch') else .006; remesh.use_smooth_shade=True
    bpy.ops.object.modifier_apply(modifier=remesh.name)
    # Remove tiny disconnected voxel remnants before decimation. These are
    # floating source fragments, not useful silhouette, and otherwise consume
    # the triangle budget while becoming isolated one-triangle raster specks.
    bm=bmesh.new(); bm.from_mesh(target.data); seen=set(); drop=[]
    for vertex in bm.verts:
        if vertex in seen: continue
        component=[]; pending=[vertex]; seen.add(vertex)
        while pending:
            v=pending.pop(); component.append(v)
            for edge in v.link_edges:
                other=edge.other_vert(v)
                if other not in seen: seen.add(other); pending.append(other)
        if len(component)<100: drop.extend(component)
    bmesh.ops.delete(bm,geom=drop,context='VERTS'); bm.to_mesh(target.data); bm.free()
    target.data.validate(); target.data.calc_loop_triangles(); before=len(target.data.loop_triangles)
    budget=spec['budget']
    if before>budget:
        dec=target.modifiers.new('Budget','DECIMATE'); dec.ratio=budget/before; dec.use_collapse_triangulate=True; bpy.ops.object.modifier_apply(modifier=dec.name)
    target.data.validate(); target.data.calc_loop_triangles(); triangles=len(target.data.loop_triangles)
    # Smart projection creates a new atlas independent of the source's UV seams.
    while target.data.uv_layers: target.data.uv_layers.remove(target.data.uv_layers[0])
    target.data.uv_layers.new(name='UVMap')
    bpy.ops.object.mode_set(mode='EDIT'); bpy.ops.mesh.select_all(action='SELECT'); bpy.ops.uv.smart_project(angle_limit=math.radians(66),island_margin=.008,area_weight=.3,correct_aspect=True,scale_to_bounds=False); bpy.ops.object.mode_set(mode='OBJECT')
    image=bpy.data.images.new(name+'_albedo',width=768,height=768,alpha=False); image.generated_color=(.25,.22,.18,1)
    target_mat=material('RuntimeBaseColor',image); target.data.materials.clear(); target.data.materials.append(target_mat)
    for polygon in target.data.polygons: polygon.material_index=0; polygon.use_smooth=True
    bpy.ops.object.select_all(action='DESELECT'); source.select_set(True); target.select_set(True); bpy.context.view_layer.objects.active=target
    scene.render.bake.use_selected_to_active=True; scene.render.bake.cage_extrusion=.035; scene.render.bake.max_ray_distance=.10
    scene.render.bake.use_pass_direct=False; scene.render.bake.use_pass_indirect=False; scene.render.bake.use_pass_color=True; scene.render.bake.margin=12
    bpy.ops.object.bake(type='DIFFUSE')
    image.filepath_raw=str(OUT/(name+'-albedo.png')); image.file_format='PNG'; image.save(); image.pack()
    pixels=np.empty(768*768*4,dtype=np.float32); image.pixels.foreach_get(pixels)
    assert np.std(pixels.reshape((-1,4))[:,:3])>.02
    bpy.data.objects.remove(source,do_unlink=True)
    a=positions(target)
    if name=='sword':
        vals,vecs=np.linalg.eigh(np.cov(a[:,[0,2]].T)); axis=vecs[:,-1]
        if axis[0]<0: axis=-axis
        theta=math.atan2(float(axis[1]),float(axis[0])); a=a@np.array(Matrix.Rotation(theta,3,'Y')).T
        lo=a.min(0); hi=a.max(0); a*=spec['size']/(hi[0]-lo[0]); lo=a.min(0); hi=a.max(0)
        grip_x=lo[0]+.18*(hi[0]-lo[0]); grip=a[abs(a[:,0]-grip_x)<.07]; center=(grip.min(0)+grip.max(0))/2; center[0]=grip_x; a-=center
    else:
        a=a@np.array(Matrix.Rotation(math.pi/2,3,'Z')).T; lo=a.min(0); hi=a.max(0)
        extent_axis=1 if name=='cot' else 2; a*=spec['size']/(hi[extent_axis]-lo[extent_axis]); lo=a.min(0); hi=a.max(0); a-=np.array([(lo[0]+hi[0])/2,(lo[1]+hi[1])/2,lo[2]])
    target.data.vertices.foreach_set('co',a.astype(np.float32).ravel()); target.data.update(); target.data.name=name
    bpy.ops.object.select_all(action='DESELECT'); target.select_set(True); bpy.context.view_layer.objects.active=target
    bpy.ops.export_scene.gltf(filepath=str(OUT/(name+'.glb')),export_format='GLB',use_selection=True,export_yup=True,export_apply=True,export_materials='EXPORT',export_image_format='AUTO')
    preview(target,name)
    entry=dict(name=name,triangles=triangles,remesh_triangles=before,bytes=(OUT/(name+'.glb')).stat().st_size,wall_seconds=round(time.perf_counter()-t,2))
    report.append(entry); print('REPAIR_COMPLETE',json.dumps(entry),flush=True)
(OUT/'repair-report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
print('TOTAL_SECONDS',round(time.perf_counter()-START,2),flush=True)
