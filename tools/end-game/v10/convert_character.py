"""Segment generated V10 bodies, close joints, bake compact atlases, retarget V7 pivots."""
import bpy,bmesh
import json,math,os,time
from pathlib import Path
import numpy as np
from mathutils import Vector,Matrix
from mathutils.kdtree import KDTree

START=time.perf_counter()
ROOT=Path(os.environ.get("EMBER_ROOT", str(Path(__file__).resolve().parents[3])))
WORK=Path(os.environ['END_GAME_V10_WORK']);WORK.mkdir(parents=True,exist_ok=True)
CHARACTER=os.environ['END_GAME_V10_CHARACTER']
CONFIG=json.loads((WORK.parent/'rig-source-config.json').read_text())[CHARACTER]
SOURCE=Path(os.environ['END_GAME_V10_SOURCE'])
RIG=json.loads((ROOT/'assets/end-game/v7/warden-rig.json').read_text())
RIG['parts']=[p for p in RIG['parts'] if p['name'] not in ['warden_chair','warden_knife']]
for p in RIG['parts']:
    p['name']=p['name'].replace('warden_','enemy_')
    if p['parent']:p['parent']=p['parent'].replace('warden_','enemy_')
PARTS={p['name'].removeprefix('enemy_'):p for p in RIG['parts']}
if CHARACTER=='cyclops':
    for name,part in PARTS.items():
        if name.endswith(('_r','_l')):
            sign=1 if name.endswith('_r') else -1
            x=.36 if name.startswith(('upperarm','forearm','hand')) else .17
            part['pivot'][0]=sign*x
            if 'endpoint' in part:part['endpoint'][0]=sign*x
BUDGET={'head':2600,'neck':200,'torso':2800,'pelvis':650,
        'upperarm':650,'forearm':550,'hand':400,'thigh':450,'shin':450,'boot':650,'coat':450}
SIZES={'head':512,'torso':512,'pelvis':256,'hand':128,'neck':128}
JOINTS=CONFIG['joints']
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
    x,y,z=p;side='r' if x>0 else 'l'
    if CHARACTER=='cyclops':
        # The generated giant has long hands reaching y=.63, below the human
        # wrist classifier. Keep every outer arm surface out of the hip hull.
        if y>1.63 or (y>1.55 and abs(x)<.14 and z<-.07):return 'head'
        if y>1.525 and abs(x)<.17:return 'neck'
        edge=.28 if y>1.40 else .32 if y>1.20 else .365 if y>.96 else .385
        if y>.61 and y<1.64 and abs(x)>edge:
            if y<.925:return 'hand_'+side
            if y<1.22:return 'forearm_'+side
            return 'upperarm_'+side
        # Remaining closed body envelopes cannot contain a distant fist.
        if y>1.055:return 'torso' if abs(x)<.39 else None
        if y>.970:return 'pelvis' if abs(x)<.365 else None
        if y>.57 and abs(x)<.385 and ((abs(z)>.135 and y>.68) or abs(x)>.29):return 'coat_'+side
        if y>.535:return 'thigh_'+side if abs(x)<.39 else None
        if y>.23:return 'shin_'+side if abs(x)<.39 else None
        return 'boot_'+side
    # Ground disks and incidental held fragments are excluded from the body.
    if y<CONFIG.get('floor_trim',0):return None
    if CHARACTER=='hollow-knight' and y<.88 and abs(x)>.30:return None
    if y>CONFIG.get('head_start',1.63) or (y>1.55 and abs(x)<.14 and z<-.07):return 'head'
    if y>1.525 and abs(x)<.17:return 'neck'
    shoulder=np.array(JOINTS['upperarm_'+side]);elbow=np.array(JOINTS['forearm_'+side]);wrist=np.array(JOINTS['hand_'+side])
    def bone_distance(a,b):
        d=b-a;t=np.clip(np.dot(p-a,d)/np.dot(d,d),0,1);return np.linalg.norm(p-(a+t*d))
    hand_end=wrist+(wrist-elbow)/np.linalg.norm(wrist-elbow)*.20
    radii=CONFIG.get('arm_radii',[.115,.105,.12])
    arm_distance=min(bone_distance(shoulder,elbow)/radii[0],bone_distance(elbow,wrist)/radii[1],bone_distance(wrist,hand_end)/radii[2])
    outside_arm=(abs(x)>.30 and z<.04 and y>.80) or (abs(x)>(.40 if CHARACTER=='cyclops' else .29) and y>1.05)
    if y>.76 and y<1.64 and abs(x)>.20 and (arm_distance<1.0 or outside_arm):
        if np.dot(p-wrist,wrist-elbow)>0:
            return 'hand_'+side if bone_distance(wrist,hand_end)<radii[2]*1.15 else None
        if np.dot(p-elbow,wrist-shoulder)>0:return 'forearm_'+side
        return 'upperarm_'+side
    if y>1.055:return 'torso'
    if y>.970:return 'pelvis'
    if .23<y<.60 and (abs(x)>.23 or abs(z)>.15):return None
    # Short outer panels articulate separately; full closed thighs remain beneath.
    if y>.57 and ((abs(z)>.135 and y>.68) or abs(x)>.225):return 'coat_'+side
    if y>.535:return 'thigh_'+side
    if y>.23:return 'shin_'+side
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
    bm=bmesh.new();bm.from_mesh(obj.data)
    bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.000001)
    bmesh.ops.dissolve_degenerate(bm,edges=list(bm.edges),dist=.0000001)
    seen=set();drop=[]
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
    mesh=bpy.data.meshes.new('enemy_'+name)
    mesh.from_pydata([B(vertices[n]) for n in used],[],[tuple(mapping[n] for n in f) for f in faces]);mesh.update()
    obj=bpy.data.objects.new('enemy_'+name,mesh);bpy.context.collection.objects.link(obj)
    bm=bmesh.new();bm.from_mesh(mesh)
    # Closed convex envelopes preserve the generated outer silhouette while
    # rejecting contradictory internal surfaces and cap spikes. Surface detail
    # is rebaked from the untouched generated sculpt.
    bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.000001)
    bmesh.ops.delete(bm,geom=list(bm.faces),context='FACES_ONLY')
    if CHARACTER=='cyclops' and group(name)=='coat':
        # Thin front/back cloth panels must not become a solid hip-spanning
        # convex wedge. Keep each facing surface in its own closed envelope.
        groups=[[v for v in bm.verts if E(v.co)[2]*sign>.13] for sign in [-1,1]]
        keep=set(v for vertices in groups for v in vertices)
        bmesh.ops.delete(bm,geom=[v for v in bm.verts if v not in keep],context='VERTS')
    else:groups=[list(bm.verts)]
    for vertices in groups:
        assert len(vertices)>8,(name,'empty panel')
        hull=bmesh.ops.convex_hull(bm,input=vertices,use_existing_faces=False)
        unused=[v for v in hull.get('geom_unused',[])+hull.get('geom_interior',[]) if isinstance(v,bmesh.types.BMVert) and v.is_valid]
        if unused:bmesh.ops.delete(bm,geom=list(set(unused)),context='VERTS')
    if name=='head':
        # Restore generated facial relief onto a closed triangulation: the
        # topology stays sealed while nose, visor and cheek recesses survive.
        bmesh.ops.subdivide_edges(bm,edges=list(bm.edges),cuts=2,use_grid_fill=True)
        for vertex in bm.verts:
            q=E(vertex.co)
            if q[1]>1.64 and q[2]<-.025:
                closest,_,_=SOURCE_TREE.find(vertex.co)
                vertex.co=vertex.co.lerp(closest,.85)
    bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces));bm.to_mesh(mesh);bm.free()
    remove_fragments(obj,40)
    return obj

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
def add_cyclops_eye(obj):
    # One recessed dark eye beneath the generated brow; no raised white sclera
    # or separate broad brow plate. The model supplies the brow and face.
    hit,point,_,_=source.ray_cast(Vector(B((0,1.729,-1))),Vector(B((0,0,1))),distance=2)
    assert hit,'Cannot locate front of cyclops face'
    front=float(E(point)[2]);RIG['oneeye_local']=[0,1.729,front-.018]
    for vertex in obj.data.vertices:
        q=E(vertex.co);r2=(q[0]/.065)**2+((q[1]-1.729)/.033)**2
        if r2<1 and q[2]<front+.025:
            q[2]+=max(0,front+.012-q[2])*(1-r2)**2;vertex.co=B(q)
    obj.data.update()
    shapes=[('socket',(0,1.729,front+.009),(.062,.029,.026),(.013,.011,.008)),
            ('iris',(0,1.729,front-.015),(.018,.019,.003),(.065,.026,.007)),
            ('pupil',(0,1.729,front-.019),(.007,.015,.002),(.002,.002,.001))]
    for label,center,size,color in shapes:
        bpy.ops.mesh.primitive_uv_sphere_add(segments=24,ring_count=12,radius=1,location=B(center));cover=bpy.context.object;cover.name='cyclops_'+label
        cover.scale=(size[0],size[2],size[1]);bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
        mat=bpy.data.materials.new('cyclops_'+label+'_source');mat.use_nodes=True;bs=mat.node_tree.nodes.get('Principled BSDF');bs.inputs['Base Color'].default_value=(*color,1);bs.inputs['Roughness'].default_value=.85
        cover.data.materials.append(mat);BAKE_SOURCES.append(cover)
        target=cover.copy();target.data=cover.data.copy();bpy.context.collection.objects.link(target);activate(obj);target.select_set(True);bpy.ops.object.join()

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
    size=SIZES.get(group(name),192)
    while obj.data.uv_layers:obj.data.uv_layers.remove(obj.data.uv_layers[0])
    obj.data.uv_layers.new(name='UVMap');activate(obj)
    bpy.ops.object.mode_set(mode='EDIT');bpy.ops.mesh.select_all(action='SELECT')
    bpy.ops.uv.smart_project(angle_limit=math.radians(66),island_margin=.008,area_weight=.3)
    bpy.ops.object.mode_set(mode='OBJECT')
    image=bpy.data.images.new('enemy_'+name+'_albedo',width=size,height=size,alpha=False)
    obj.data.materials.clear();obj.data.materials.append(material('enemy_'+name+'_material',image))
    activate(obj)
    for source in BAKE_SOURCES:source.select_set(True)
    bpy.context.view_layer.objects.active=obj
    bpy.ops.object.bake(type='DIFFUSE')
    image.filepath_raw=str(WORK/('enemy_'+name+'.png'));image.file_format='PNG';image.save();image.pack()
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
    scene.render.filepath=str(WORK/(CHARACTER+'-standing-preview.png'));bpy.ops.render.render(write_still=True)

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
solid=source.copy();solid.data=source.data.copy();bpy.context.collection.objects.link(solid);solid.name='OriginalSurfaceTarget'
# The source is already closed: remesh it directly into one solid boundary.
# Solidifying first creates a hollow double shell and interior armor fragments.
activate(solid);remesh=solid.modifiers.new('SingleSolidSource','REMESH');remesh.mode='VOXEL';remesh.voxel_size=.0045;remesh.use_smooth_shade=True
bpy.ops.object.modifier_apply(modifier=remesh.name)
remove_fragments(solid,120)
triangulate(solid);p=np.array([E(v.co) for v in solid.data.vertices]);faces={name:[] for name in JOINTS}
for poly in solid.data.polygons:
    indices=list(poly.vertices);category=classify(p[indices].mean(0))
    if category is not None:faces[category].append(indices)
print('V10_SEGMENT_COUNTS',json.dumps({k:len(v) for k,v in faces.items()}),flush=True)
scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=8
scene.render.threads_mode='FIXED';scene.render.threads=2
scene.render.bake.use_selected_to_active=True;scene.render.bake.cage_extrusion=.028;scene.render.bake.max_ray_distance=.10;scene.render.bake.margin=8
scene.render.bake.use_pass_direct=False;scene.render.bake.use_pass_indirect=False;scene.render.bake.use_pass_color=True
objects=[]
for name in JOINTS:
    begin=time.perf_counter();assert faces[name],name
    obj=make_piece(name,p,faces[name]);before_area=sum(poly.area for poly in obj.data.polygons)
    add_joint_cover(obj,name)
    if CHARACTER=='cyclops' and name=='head':add_cyclops_eye(obj)
    simplify(obj,BUDGET[group(name)])
    after_area=sum(poly.area for poly in obj.data.polygons)
    print('V10_SURFACE',name,'before',round(before_area,5),'after',round(after_area,5),flush=True)
    if name=='torso':assert after_area>before_area*.60,(name,'major body surface loss')
    assert len(obj.data.polygons)>40,(name,'surface disappeared during cleanup')
    bake(obj,name);retarget(obj,name);objects.append(obj)
    print('V10_PART',name,len(obj.data.polygons),round(time.perf_counter()-begin,2),flush=True)
bpy.data.objects.remove(solid,do_unlink=True)
activate(objects[0])
for obj in objects:obj.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(WORK/(CHARACTER+'.glb')),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
RIG['source_method']='SDXL reference -> TRELLIS.2 sculpt -> rigid segmentation, closed convex part retopology with facial relief projection, RGB source rebake, bind retarget'
RIG['source_joints']={k:list(v) for k,v in JOINTS.items()}
RIG['limits']='Single-view generated character; inferred back, rigid parts and fused fingers. Closed inner leg/joint covers use nearby generated material samples. Incidental held objects excluded; separately authored measured weapons.'
RIG['character']=CHARACTER
RIG['weapons_separate']=True
if CHARACTER!='cyclops':RIG['oneeye_local']=None
(WORK/(CHARACTER+'-rig.json')).write_text(json.dumps(RIG,indent=2)+'\n',encoding='utf-8',newline='\n')
for o in objects:
    if o.name=='enemy_chair':o.hide_render=True
preview([o for o in objects if o.name!='enemy_chair'])
print('V10_CONVERSION_WALL_SECONDS',round(time.perf_counter()-START,3),flush=True)
