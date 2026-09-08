"""Retarget the generated jailer into the 20 rigid V5 joints and rebake small RGB atlases."""
import bpy,bmesh
import json,math,os,time
from pathlib import Path
import numpy as np
from mathutils import Vector,Matrix
from mathutils.kdtree import KDTree

START=time.perf_counter()
ROOT=Path(__file__).resolve().parents[2]
WORK=Path(os.environ['END_GAME_V7_WORK']);WORK.mkdir(parents=True,exist_ok=True)
SOURCE=Path(os.environ['END_GAME_V7_SOURCE'])
RIG=json.loads((ROOT/'assets/end-game/v5/warden-rig.json').read_text())
PARTS={p['name'].removeprefix('warden_'):p for p in RIG['parts']}
BUDGET={'head':2800,'neck':400,'torso':3600,'pelvis':1200,
        'upperarm':900,'forearm':800,'hand':650,'thigh':450,'shin':450,'boot':650,'coat':2000}
SIZES={'head':1024,'torso':1024,'pelvis':512,'hand':512}
JOINTS={
    'pelvis':(0,1,.025),'torso':(0,1.06,.025),'neck':(0,1.52,.015),'head':(0,1.60,-.002),
    'upperarm_r':(.220,1.515,.045),'forearm_r':(.342,1.285,.070),'hand_r':(.368,1.055,-.117),
    'upperarm_l':(-.226,1.515,-.008),'forearm_l':(-.351,1.285,-.006),'hand_l':(-.382,1.014,-.080),
    'thigh_r':(.103,.990,.062),'shin_r':(.101,.540,.060),'boot_r':(.099,.110,.058),
    'thigh_l':(-.130,.990,.060),'shin_l':(-.135,.540,.060),'boot_l':(-.139,.110,.058),
    'coat_r':(.105,.970,.025),'coat_l':(-.105,.970,.025),
}
CHILD={'upperarm':'forearm','forearm':'hand','thigh':'shin','shin':'boot'}

def B(p):return (float(p[0]),-float(p[2]),float(p[1]))
def E(p):return np.array([p[0],p[2],-p[1]],dtype=float)
def group(name):return name.rsplit('_',1)[0] if name.endswith(('_r','_l')) else name
def material(name,image):
    m=bpy.data.materials.new(name);m.use_nodes=True;bs=m.node_tree.nodes.get('Principled BSDF')
    bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Roughness'].default_value=.86;bs.inputs['Metallic'].default_value=0
    t=m.node_tree.nodes.new('ShaderNodeTexImage');t.image=image;m.node_tree.links.new(t.outputs['Color'],bs.inputs['Base Color'])
    m.node_tree.nodes.active=t;t.select=True;return m

def classify(p):
    x,y,z=p
    if y>1.665 or (y>1.615 and abs(x)<.124 and z<-.057):return 'head'
    if y>1.528 and abs(x)<.163:return 'neck'
    side='r' if x>0 else 'l';s=1 if x>0 else -1
    # The left fingertips descend below the coat's upper boundary. Classify
    # their distal wrist volume before the arm-height gate, keeping every
    # finger on the hand when its bind position moves down during retargeting.
    if abs(x)>.295 and .77<y<1.3:
        elbow=np.array(JOINTS['forearm_'+side]);wrist=np.array(JOINTS['hand_'+side])
        hand_axis=wrist-elbow;hand_axis/=np.linalg.norm(hand_axis)
        distal=np.dot(p-wrist,hand_axis);radial=np.linalg.norm(p-wrist-hand_axis*distal)
        if -.008<distal<.235 and radial<.102:return 'hand_'+side
    threshold=float(np.interp(y,[.89,1.05,1.25,1.43,1.60],[.305,.295,.270,.220,.200]))
    if y>.90 and y<1.625 and abs(x)>threshold:
        elbow=np.array(JOINTS['forearm_'+side]);wrist=np.array(JOINTS['hand_'+side]);shoulder=np.array(JOINTS['upperarm_'+side])
        if np.dot(p-wrist,wrist-elbow)>0:return 'hand_'+side
        if np.dot(p-elbow,wrist-shoulder)>0:return 'forearm_'+side
        return 'upperarm_'+side
    if y>1.073:return 'torso'
    if y>.970:return 'pelvis'
    coat=y>.635 or (y>.405 and abs(x)>.221) or (y>.48 and (z>.152 or z<-.137))
    if coat:return 'coat_'+side
    if y>.535:return 'thigh_'+side
    if y>.265:return 'shin_'+side
    return 'boot_'+side

def triangulate(o):
    bm=bmesh.new();bm.from_mesh(o.data);bmesh.ops.triangulate(bm,faces=list(bm.faces));bm.to_mesh(o.data);bm.free();o.data.update()

def normals(o):
    bm=bmesh.new();bm.from_mesh(o.data);bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(o.data);bm.free();o.data.update()
    o.data.set_sharp_from_angle(angle=math.radians(50))

def activate(o):
    bpy.ops.object.select_all(action='DESELECT');o.select_set(True);bpy.context.view_layer.objects.active=o

def simplify(o,budget):
    o.data.validate();triangulate(o)
    if len(o.data.polygons)>budget:
        activate(o);d=o.modifiers.new('TriangleBudget','DECIMATE');d.ratio=budget/len(o.data.polygons);d.use_collapse_triangulate=True;bpy.ops.object.modifier_apply(modifier=d.name)
    remove_fragments(o,12)
    o.data.validate();triangulate(o);normals(o);o.data.validate()
    for p in o.data.polygons:p.use_smooth=True

def remove_fragments(obj,minimum=100):
    bm=bmesh.new();bm.from_mesh(obj.data);seen=set();drop=[]
    for vertex in bm.verts:
        if vertex in seen:continue
        pending=[vertex];seen.add(vertex);component=[]
        while pending:
            v=pending.pop();component.append(v)
            for e in v.link_edges:
                other=e.other_vert(v)
                if other not in seen:seen.add(other);pending.append(other)
        if len(component)<minimum:drop.extend(component)
    if drop:bmesh.ops.delete(bm,geom=drop,context='VERTS')
    bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.000001)
    bmesh.ops.dissolve_degenerate(bm,edges=list(bm.edges),dist=.0000001)
    bm.to_mesh(obj.data);bm.free();obj.data.validate();obj.data.update()

def make_piece(name,vertices,faces):
    used=sorted({i for f in faces for i in f});mapping={n:i for i,n in enumerate(used)}
    mesh=bpy.data.meshes.new('warden_'+name)
    mesh.from_pydata([B(vertices[n]) for n in used],[],[tuple(mapping[n] for n in f) for f in faces]);mesh.update()
    obj=bpy.data.objects.new('warden_'+name,mesh);bpy.context.collection.objects.link(obj)
    bm=bmesh.new();bm.from_mesh(mesh)
    boundary=[e for e in bm.edges if e.is_boundary]
    if boundary and group(name) not in ['coat','shin']:bmesh.ops.holes_fill(bm,edges=boundary,sides=0)
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(mesh);bm.free()
    remove_fragments(obj,40)
    return obj

def close_piece(obj,name):
    if group(name) not in ['coat','shin']:return
    activate(obj)
    if group(name) in ['coat','shin']:
        shell=obj.modifiers.new('ThinClothPanel','SOLIDIFY');shell.thickness=.011;shell.offset=0
        bpy.ops.object.modifier_apply(modifier=shell.name)
    remesh=obj.modifiers.new('ClosedJointSurface','REMESH');remesh.mode='VOXEL';remesh.voxel_size=.0038;remesh.use_smooth_shade=True
    bpy.ops.object.modifier_apply(modifier=remesh.name)
    remove_fragments(obj,80)

def add_joint_cover(obj,name):
    kind=group(name)
    if kind not in ['upperarm','forearm','hand','thigh','shin','neck']:return
    center=np.array(JOINTS[name]);r={'upperarm':.087,'forearm':.050,'hand':.031,'thigh':.083,'shin':.066,'neck':.070}[kind]
    if kind=='upperarm':center[1]+=.015
    bpy.ops.mesh.primitive_uv_sphere_add(segments=12,ring_count=6,radius=1,location=B(center))
    cover=bpy.context.object;cover.scale=(r,r,r*.85);bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    if kind in ['thigh','shin']:
        # The generated coat hides the thighs. Add trouser coverage inside it,
        # so separating the coat panels cannot expose an empty volume.
        endpoint=np.array(JOINTS[name.replace(kind,CHILD[kind])]);mid=(center+endpoint)*.5
        cover.location=B(mid);cover.scale=(1,1,np.linalg.norm(center-endpoint)/(2*r)*1.28)
    bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
    # The cover is a separate bake source textured from the generated nearby
    # surface, then joined into the same rigid primitive after duplication.
    uv=cover.data.uv_layers.new(name='UVMap')
    for poly in cover.data.polygons:
        q=E(poly.center)
        if kind=='thigh':q[1]=.575+(q[1]-.70)*.07
        _,idx,_=SOURCE_TREE.find(Vector(B(q)))
        sample=SOURCE_UV[idx]
        for loop in poly.loop_indices:uv.data[loop].uv=sample
    cover.data.materials.append(SOURCE_MATERIAL)
    BAKE_SOURCES.append(cover)
    target=cover.copy();target.data=cover.data.copy();bpy.context.collection.objects.link(target)
    activate(obj);target.select_set(True);bpy.ops.object.join()
    if kind=='shin':
        # Closed continuous trouser volume unions with the generated outer
        # folds. Blanket polygon caps across irregular cut loops are avoided.
        remesh=obj.modifiers.new('SolidTrouserUnion','REMESH');remesh.mode='VOXEL';remesh.voxel_size=.0038;remesh.use_smooth_shade=True
        bpy.ops.object.modifier_apply(modifier=remesh.name);remove_fragments(obj,80)

def retarget(obj,name):
    start=np.array(JOINTS[name]);target=np.array(PARTS[name]['pivot']);kind=group(name)
    if kind in CHILD:
        child=CHILD[kind]+'_'+name[-1];end=np.array(JOINTS[child]);dest=np.array(PARTS[child]['pivot'])
        d=(end-start)/np.linalg.norm(end-start);v=(dest-target)/np.linalg.norm(dest-target)
        rotation=Vector(d).rotation_difference(Vector(v)).to_matrix();scale=np.linalg.norm(dest-target)/np.linalg.norm(end-start)
    elif kind=='hand':
        parent='forearm_'+name[-1];d=(start-np.array(JOINTS[parent]));d/=np.linalg.norm(d);v=np.array([0,-1,0])
        rotation=Vector(d).rotation_difference(Vector(v)).to_matrix();scale=1
    else:d=np.array([0,-1,0]);rotation=Matrix.Identity(3);scale=1
    verts=[]
    for vertex in obj.data.vertices:
        original=E(vertex.co);p=original-start;p=p+d*np.dot(p,d)*(scale-1)
        transformed=target+np.array(rotation@Vector(p))
        if kind=='upperarm':
            # Keep the high generated shoulder cap at its jacket seam, while
            # bringing the lower sleeve into the existing straight-arm bind.
            preserve=float(np.clip((original[1]-1.40)/.145,0,1))
            seam=original-np.array([0,0,.025]);transformed=transformed*(1-preserve)+seam*preserve
        verts.append(B(transformed))
    a=np.asarray(verts,dtype=np.float32)
    if kind=='boot':a[:,2]-=a[:,2].min()
    obj.data.vertices.foreach_set('co',a.ravel());obj.data.update();normals(obj);obj.data.validate()
    # Extremely sharp cap corners must use their face normal, not an average
    # spanning both sides of a fold. This is checked again after GLB export.
    corner_normals=[n.vector.copy() for n in obj.data.corner_normals]
    for poly in obj.data.polygons:
        mean=sum((corner_normals[i] for i in poly.loop_indices),Vector())/len(poly.loop_indices)
        if mean.dot(poly.normal)<.05:poly.use_smooth=False
    obj.data.update()

def bake(obj,name):
    size=SIZES.get(group(name),256)
    while obj.data.uv_layers:obj.data.uv_layers.remove(obj.data.uv_layers[0])
    obj.data.uv_layers.new(name='UVMap');activate(obj)
    bpy.ops.object.mode_set(mode='EDIT');bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.smart_project(angle_limit=math.radians(66),island_margin=.008,area_weight=.3)
    bpy.ops.object.mode_set(mode='OBJECT')
    image=bpy.data.images.new('warden_'+name+'_albedo',width=size,height=size,alpha=False)
    obj.data.materials.clear();obj.data.materials.append(material('warden_'+name+'_material',image))
    activate(obj)
    for source in BAKE_SOURCES:source.select_set(True)
    bpy.context.view_layer.objects.active=obj
    bpy.ops.object.bake(type='DIFFUSE')
    image.filepath_raw=str(WORK/('warden_'+name+'.png'));image.file_format='PNG';image.save();image.pack()
    sample=np.asarray(image.pixels[::4]);assert np.std(sample)>.012,(name,'blank bake')

def preview(objects):
    for source in BAKE_SOURCES:source.hide_render=True
    for o in objects:o.hide_render=False
    scene=bpy.context.scene;scene.cycles.samples=16;scene.render.resolution_x=800;scene.render.resolution_y=1100
    scene.render.resolution_percentage=100;scene.view_settings.view_transform='AgX'
    scene.world=bpy.data.worlds.new('Preview');scene.world.use_nodes=True
    scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.20,.22,.24,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
    target=Vector((0,0,.98))
    for p,power in [((2,3,3),260),((-2,1,2),180),((0,-3,3),180)]:
        bpy.ops.object.light_add(type='AREA',location=p);o=bpy.context.object;o.data.energy=power;o.data.size=2.5;o.rotation_euler=(target-o.location).to_track_quat('-Z','Y').to_euler()
    bpy.ops.object.camera_add(location=(2.2,4.4,1.7));camera=bpy.context.object;camera.data.type='ORTHO';camera.data.ortho_scale=2.12
    camera.rotation_euler=(target-camera.location).to_track_quat('-Z','Y').to_euler();scene.camera=camera
    scene.render.filepath=str(WORK/'warden-standing-preview.png');bpy.ops.render.render(write_still=True)

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(SOURCE))
source=next(o for o in bpy.context.scene.objects if o.type=='MESH')
source.data.transform(source.matrix_world);source.parent=None;source.matrix_world=Matrix.Identity(4)
v=np.array([p.co[:] for p in source.data.vertices]);lo=v.min(0);hi=v.max(0)
assert np.argmax(hi-lo)==2,'Expected GLB Y-up after Blender import'
center=np.array([(lo[0]+hi[0])*.5,(lo[1]+hi[1])*.5,lo[2]])
v=(v-center)*1.865/(hi[2]-lo[2]);v[:,:2]*=-1
source.data.vertices.foreach_set('co',v.astype(np.float32).ravel());source.data.update();source.name='GeneratedBakeSource'
SOURCE_MATERIAL=source.data.materials[0];bs=SOURCE_MATERIAL.node_tree.nodes.get('Principled BSDF')
source_image=bs.inputs['Base Color'].links[0].from_node.image
SOURCE_MATERIAL=material('GeneratedBaseColorOnly',source_image)
source.data.materials.clear();source.data.materials.append(SOURCE_MATERIAL)
for poly in source.data.polygons:poly.material_index=0
SOURCE_TREE=KDTree(len(source.data.vertices));SOURCE_UV=np.zeros((len(source.data.vertices),2))
for vertex in source.data.vertices:SOURCE_TREE.insert(vertex.co,vertex.index)
SOURCE_TREE.balance();uv=source.data.uv_layers.active
for loop in source.data.loops:SOURCE_UV[loop.vertex_index]=uv.data[loop.index].uv[:]
BAKE_SOURCES=[source]
solid=source.copy();solid.data=source.data.copy();bpy.context.collection.objects.link(solid);solid.name='SolidSource'
activate(solid);remesh=solid.modifiers.new('RemoveInteriorSurfaces','REMESH');remesh.mode='VOXEL';remesh.voxel_size=.0035;remesh.use_smooth_shade=True
shell=solid.modifiers.new('CloseSourceSheets','SOLIDIFY');shell.thickness=.009;shell.offset=0;shell.use_even_offset=True
bpy.ops.object.modifier_move_up(modifier=shell.name)
bpy.ops.object.modifier_apply(modifier=shell.name)
bpy.ops.object.modifier_apply(modifier=remesh.name)
remove_fragments(solid,120)
triangulate(solid);p=np.array([E(v.co) for v in solid.data.vertices]);faces={name:[] for name in JOINTS}
for poly in solid.data.polygons:
    indices=list(poly.vertices);faces[classify(p[indices].mean(0))].append(indices)
print('V7_SEGMENT_COUNTS',json.dumps({k:len(v) for k,v in faces.items()}),flush=True)
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=8
scene.render.threads_mode='FIXED';scene.render.threads=4
scene.render.bake.use_selected_to_active=True;scene.render.bake.cage_extrusion=.028;scene.render.bake.max_ray_distance=.10;scene.render.bake.margin=8
scene.render.bake.use_pass_direct=False;scene.render.bake.use_pass_indirect=False;scene.render.bake.use_pass_color=True
objects=[]
for name in JOINTS:
    begin=time.perf_counter();assert faces[name],name
    obj=make_piece(name,p,faces[name]);close_piece(obj,name);add_joint_cover(obj,name);simplify(obj,BUDGET[group(name)])
    assert len(obj.data.polygons)>=BUDGET[group(name)]*.50,(name,'surface disappeared during cleanup')
    bake(obj,name);retarget(obj,name);objects.append(obj)
    print('V7_PART',name,len(obj.data.polygons),round(time.perf_counter()-begin,2),flush=True)
bpy.data.objects.remove(solid,do_unlink=True)
# Retain only the already-articulated V5 stationary chair and blade.
before=set(bpy.data.objects);bpy.ops.import_scene.gltf(filepath=str(ROOT/'assets/end-game/v5/warden.glb'))
for obj in list(set(bpy.data.objects)-before):
    if obj.type=='MESH' and obj.name in ['warden_chair','warden_knife']:
        obj.data.transform(obj.matrix_world);obj.parent=None;obj.matrix_world=Matrix.Identity(4);objects.append(obj)
    else:bpy.data.objects.remove(obj,do_unlink=True)
activate(objects[0])
for obj in objects:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(WORK/'warden.glb'),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
RIG['source_method']='SDXL reference -> TRELLIS.2 sculpt -> voxel cleanup, rigid segmentation, RGB rebake, bind retarget'
RIG['source_joints']={k:list(v) for k,v in JOINTS.items()}
RIG['limits']='Single-view inferred back; partially fused generated finger forms; inner thigh and joint covers use generated material samples; V5 chair/knife retained.'
(WORK/'warden-rig.json').write_text(json.dumps(RIG,indent=2)+'\n',encoding='utf-8',newline='\n')
for o in objects:
    if o.name=='warden_chair':o.hide_render=True
preview([o for o in objects if o.name!='warden_chair'])
print('V7_CONVERSION_WALL_SECONDS',round(time.perf_counter()-START,3),flush=True)
