"""Articulated rigid wolf gauntlets, metric bind pivots, grip/contact previews."""
import bpy,math,json,time
import numpy as np
from mathutils import Vector,Matrix,Quaternion
from pathlib import Path
import os
OUT=Path(os.environ['END_GAME_V3_ASSET_WORK']);START=time.perf_counter();PARTS=[];GEOM={};RIG=[]
MATERIAL_TILE={'iron':(0,0),'leather':(1,0),'edge':(0,1),'brass':(1,1)}
def B(p):return (float(p[0]),-float(p[2]),float(p[1]))
def basis(direction):
    d=Vector(direction).normalized();u=(Vector((0,1,0))-d*d.y).normalized();s=d.cross(u).normalized();return np.array([d[:],u[:],s[:]]).T
def geom():return dict(verts=[],faces=[],mat=[],smooth=[])
def add(g,verts,faces,mat='iron',smooth=False,pivot=(0,0,0),orient=None):
    v=np.asarray(verts,dtype=float)
    if orient is not None:v=v@orient.T
    v+=np.asarray(pivot);off=len(g['verts']);g['verts'].extend(v.tolist());g['faces'].extend([tuple(i+off for i in f) for f in faces]);g['mat'].extend([mat]*len(faces));g['smooth'].extend([smooth]*len(faces))
def capsule(g,x0,x1,ry,rz,mat='leather',pivot=(0,0,0),orient=None,taper=.85,sides=10):
    verts=[];faces=[]
    for x,s in [(x0,.58),(x0+(x1-x0)*.13,1),(x1-(x1-x0)*.15,taper),(x1,.50*taper)]:
        for i in range(sides):a=i*math.tau/sides;verts.append((x,math.cos(a)*ry*s,math.sin(a)*rz*s))
    for j in range(3):
        for i in range(sides):faces.append((j*sides+i,j*sides+(i+1)%sides,(j+1)*sides+(i+1)%sides,(j+1)*sides+i))
    faces.extend([tuple(reversed(range(sides))),tuple(3*sides+i for i in range(sides))]);add(g,verts,faces,mat,True,pivot,orient)
def ellipsoid(g,center,radii,mat='iron',segments=8,rings=4,orient=None,pivot=(0,0,0)):
    verts=[tuple(np.array(center)+np.array([0,-radii[1],0]))];faces=[]
    for j in range(1,rings):
        b=-math.pi/2+j*math.pi/rings
        for i in range(segments):a=i*math.tau/segments;verts.append(tuple(np.array(center)+np.array([math.cos(b)*math.cos(a),math.sin(b),math.cos(b)*math.sin(a)])*radii))
    top=len(verts);verts.append(tuple(np.array(center)+np.array([0,radii[1],0])))
    for i in range(segments):
        faces.append((0,1+i,1+(i+1)%segments))
        faces.append((top,1+(rings-2)*segments+(i+1)%segments,1+(rings-2)*segments+i))
    for j in range(rings-2):
        for i in range(segments):faces.append((1+j*segments+i,1+(j+1)*segments+i,1+(j+1)*segments+(i+1)%segments,1+j*segments+(i+1)%segments))
    add(g,verts,faces,mat,True,pivot,orient)
def plate(g,x0,x1,width,y,mat='iron',pivot=(0,0,0),orient=None):
    # Low overlapping peaked plate with a tapered finger tip and bevel outline.
    outline=[(x0, -width*.43),(x0+.003,-width*.5),(x1-.004,-width*.41),(x1,0),(x1-.004,width*.41),(x0+.003,width*.5),(x0,width*.43)]
    verts=[(x,y,z) for x,z in outline]+[(x,y+.0025,z*.92) for x,z in outline]+[((x0+x1)*.5,y+.0055,0)]
    n=len(outline);faces=[tuple(reversed(range(n)))]
    for i in range(n):faces.extend([(i,(i+1)%n,(i+1)%n+n,i+n),(i+n,(i+1)%n+n,2*n)])
    add(g,verts,[tuple(reversed(f)) for f in faces],mat,False,pivot,orient)
    # Inset wear strips are geometry, so knuckle articulation stays readable.
    for z in [-width*.39,width*.39]:
        box(g,((x0+x1)*.5,y+.003,z),(max(x1-x0-.008,.002),.0008,.0011),'edge',pivot,orient)
def box(g,center,size,mat='iron',pivot=(0,0,0),orient=None):
    verts=[tuple(np.array(center)+np.array([x,y,z])*np.array(size)*.5) for x,y,z in [(-1,-1,-1),(1,-1,-1),(1,1,-1),(-1,1,-1),(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]]
    add(g,verts,[(0,3,2,1),(4,5,6,7),(0,1,5,4),(3,7,6,2),(0,4,7,3),(1,2,6,5)],mat,False,pivot,orient)
def torus(g,center,rx,rz,tube,mat='iron',plane='YZ',segments=20,sides=6):
    verts=[];faces=[]
    for i in range(segments):
        a=i*math.tau/segments
        for j in range(sides):
            t=j*math.tau/sides
            p=(tube*math.cos(t),(rx+tube*math.sin(t))*math.cos(a),(rz+tube*math.sin(t))*math.sin(a)) if plane=='YZ' else ((rx+tube*math.sin(t))*math.cos(a),(rz+tube*math.sin(t))*math.sin(a),tube*math.cos(t))
            verts.append(tuple(np.array(center)+p))
    for i in range(segments):
        for j in range(sides):faces.append((i*sides+j,i*sides+(j+1)%sides,((i+1)%segments)*sides+(j+1)%sides,((i+1)%segments)*sides+j))
    add(g,verts,faces,mat,True)
def record(name,g,parent,pivot,axis=(0,0,-1),**extra):
    GEOM[name]=g;row=dict(name=name,parent=parent,pivot=list(pivot),curl_axis=list(axis));row.update(extra);RIG.append(row)

def make_right():
    g=geom();capsule(g,-.581,-.253,.052,.055,'leather',taper=.83,sides=12)
    for x in [-.49,-.39,-.285]:
        torus(g,(x,0,0),.045 if x>-.4 else .050,.048 if x>-.4 else .053,.004,'iron',segments=16,sides=5)
    plate(g,-.48,-.33,.080,.044);record('upperarm',g,None,(-.57,0,0),category='ik_upperarm',endpoint=[-.27,0,0],length=.30)
    g=geom();capsule(g,-.282,.003,.041,.044,'leather',taper=.74,sides=12)
    for i in range(4):
        x0=-.249+i*.056;plate(g,x0,x0+.067,.083-i*.006,.039-i*.002)
        for z in (-.027,.027):ellipsoid(g,(x0+.013,.045-i*.002,z),(.0027,.0018,.0027),'edge',6,3)
    for x,ry,rz in [(-.263,.042,.045),(-.017,.033,.035)]:torus(g,(x,0,0),ry,rz,.0045,'iron',segments=20,sides=6)
    # Small cuff side closures and brass retention studs.
    for z in (-.035,.035):box(g,(-.040,-.005,z),(.035,.026,.006),'iron');ellipsoid(g,(-.035,.006,z*1.1),(.0035,.0035,.002),'brass',6,3)
    record('forearm',g,'upperarm',(-.27,0,0),category='ik_forearm',endpoint=[0,0,0],length=.27)
    g=geom();capsule(g,-.012,.098,.021,.045,'leather',taper=.98,sides=12)
    ellipsoid(g,(.035,-.004,-.030),(.030,.017,.023),'leather',12,6)
    for i in range(3):plate(g,.001+i*.028,.035+i*.028,.079+i*.002,.020+i*.0005)
    # Modest geometric wolf brow/ears on the back of the hand.
    add(g,[(.022,.033,-.015),(.031,.029,-.021),(.041,.029,-.011),(.049,.029,0),(.041,.029,.011),(.031,.029,.021),(.022,.033,.015),(.036,.037,0)],[(0,1,2,7),(2,3,7),(3,4,7),(4,5,6,7),(6,0,7)],'edge')
    for z in (-.010,.010):box(g,(.037,.038,z),(.006,.0012,.002),'iron')
    for z in [-.032,-.010,.013,.034]:ellipsoid(g,(.089,.016,z),(.012,.008,.010),'iron',8,4)
    record('palm',g,'forearm',(0,0,0),category='palm')
    fingers=[('index',(.086,0,-.033),[.040,.024,.018],.019,.07),('middle',(.094,0,-.011),[.044,.027,.019],.020,0),('ring',(.089,0,.012),[.040,.025,.018],.019,-.035),('pinky',(.078,-.001,.033),[.031,.021,.016],.016,-.10)]
    for finger,base,lengths,width,splay in fingers:
        d=np.array([math.cos(splay),0,-math.sin(splay)]);o=basis(d);axis=np.cross(d,[0,-1,0]);p=np.array(base,dtype=float)
        for segment,length in enumerate(lengths):
            w=width*(1-segment*.14);g=geom();capsule(g,-.003,length+.002,w*.43,w*.49,'leather',p,o,.82,8)
            ellipsoid(g,(0,0,0),(w*.46,w*.45,w*.48),'leather',8,4,o,p)
            plate(g,.001,length*.88,w*.94,w*.42,'iron',p,o)
            if segment==0:ellipsoid(g,(.002,w*.47,0),(w*.36,w*.16,w*.40),'iron',8,4,o,p)
            record(f'{finger}_{segment}',g,'palm' if segment==0 else f'{finger}_{segment-1}',p,axis,category='finger',digit=finger,segment=segment,length=length,tip=(p+d*length).tolist(),curl_range=[0,1.8 if segment<2 else 1.5])
            p=p+d*length
    d=np.array([.61,-.06,-.79]);d/=np.linalg.norm(d);o=basis(d);axis=np.cross(d,[0,-1,0]);axis/=np.linalg.norm(axis);p=np.array([.037,-.003,-.044])
    for segment,length in enumerate([.047,.036]):
        w=.022-segment*.003;g=geom();capsule(g,-.004,length+.003,w*.46,w*.5,'leather',p,o,.84,10)
        ellipsoid(g,(0,0,0),(w*.5,w*.48,w*.52),'leather',8,4,o,p);plate(g,.002,length*.87,w,w*.45,'iron',p,o)
        record(f'thumb_{segment}',g,'palm' if segment==0 else 'thumb_0',p,axis,category='thumb',digit='thumb',segment=segment,length=length,tip=(p+d*length).tolist(),curl_range=[0,1.45],opposition_axis=[0,-1,0] if segment==0 else None,opposition_range=[0,2.1] if segment==0 else None)
        p=p+d*length

def object_from_geom(name,g,mat,mirror=False):
    vertices=np.array(g['verts']);faces=g['faces']
    if mirror:vertices[:,2]*=-1;faces=[tuple(reversed(f)) for f in faces]
    mesh=bpy.data.meshes.new(name);mesh.from_pydata([B(v) for v in vertices],[],faces);mesh.update();uv=mesh.uv_layers.new(name='UVMap')
    for face,material_name,smooth in zip(mesh.polygons,g['mat'],g['smooth']):
        face.use_smooth=smooth;tile=MATERIAL_TILE[material_name]
        n=np.abs(np.cross(vertices[face.vertices[1]]-vertices[face.vertices[0]],vertices[face.vertices[2]]-vertices[face.vertices[0]]));axes=[a for a in range(3) if a!=int(np.argmax(n))]
        for loop in face.loop_indices:
            v=vertices[mesh.loops[loop].vertex_index];u=(v[axes[0]]*13.7)%1;w=(v[axes[1]]*13.7)%1;uv.data[loop].uv=(tile[0]*.5+.035+u*.43,tile[1]*.5+.035+w*.43)
    mesh.materials.append(mat);obj=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(obj);return obj

def matrix_for_pose(row,angles,matrices):
    parent=matrices.get(row['parent'],Matrix.Identity(4));a=angles.get(row['name'],{})
    if isinstance(a,(int,float)):a={'curl':a}
    p=Vector(row['pivot']);rotation=Matrix.Identity(4)
    if row.get('opposition_axis') and a.get('opposition',0):rotation=Matrix.Rotation(a['opposition'],4,Vector(row['opposition_axis']))
    rotation=rotation@Matrix.Rotation(a.get('curl',0),4,Vector(row['curl_axis']))
    return parent@Matrix.Translation(p)@rotation@Matrix.Translation(-p)

def preview(name,pose,objects,proxy=None,overview=False):
    scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=16;scene.render.threads_mode='FIXED';scene.render.threads=4
    scene.render.resolution_x=1100;scene.render.resolution_y=850;scene.render.resolution_percentage=100;scene.view_settings.view_transform='AgX';scene.render.image_settings.file_format='PNG'
    scene.world=bpy.data.worlds.get('World') or bpy.data.worlds.new('World');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.20,.23,.28,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.7
    C=Matrix(((1,0,0,0),(0,0,-1,0),(0,1,0,0),(0,0,0,1)));matrices={}
    for row in RIG:
        d=matrix_for_pose(row,pose,matrices);matrices[row['name']]=d
        for side in ('r','l'):
            obj=objects[f'hand_{side}_{row["name"]}'];obj.hide_render=side=='l' and not overview
            if side=='l':S=Matrix.Diagonal(Vector((1,1,-1,1)));dside=S@d@S
            else:dside=d
            offset=Matrix.Translation((0,0,-.10 if side=='r' else .10)) if overview else Matrix.Identity(4)
            obj.matrix_world=C@offset@dside@C.inverted()
    temporary=[]
    if proxy:
        temporary.append(proxy)
    target=np.array([-.14,0,0]) if overview else np.array([.055,-.01,0])
    eye=target+np.array([.25,.52,.28]) if pose=={} or overview else target+np.array([.26,-.36,.27])
    bpy.ops.object.camera_add(location=B(eye));cam=bpy.context.object;temporary.append(cam);cam.data.type='ORTHO';cam.data.ortho_scale=.88 if overview else .34;cam.rotation_euler=(Vector(B(target))-cam.location).to_track_quat('-Z','Y').to_euler();scene.camera=cam
    for offset,power in [((.10,.40,.25),25),((-.15,-.25,-.22),14)]:
        bpy.ops.object.light_add(type='AREA',location=B(target+np.array(offset)));lamp=bpy.context.object;temporary.append(lamp);lamp.data.energy=power;lamp.data.size=.35;lamp.rotation_euler=(Vector(B(target))-lamp.location).to_track_quat('-Z','Y').to_euler()
    scene.render.filepath=str(OUT/(name+'.png'));bpy.ops.render.render(write_still=True)
    for obj in temporary:bpy.data.objects.remove(obj,do_unlink=True)
    for obj in objects.values():obj.matrix_world=Matrix.Identity(4);obj.hide_render=False
    return matrices

def main():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    mat=bpy.data.materials.new('SharedWolfArmor');mat.use_nodes=True;bs=mat.node_tree.nodes['Principled BSDF'];bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Metallic'].default_value=.25;bs.inputs['Roughness'].default_value=.52
    tex=mat.node_tree.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(str(OUT/'wolf-armor-atlas.png'));tex.image.pack();mat.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    make_right();objects={}
    for side in ('r','l'):
        for row in RIG:
            name=f'hand_{side}_{row["name"]}';objects[name]=object_from_geom(name,GEOM[row['name']],mat,side=='l')
    bpy.ops.object.select_all(action='DESELECT')
    for obj in objects.values():obj.select_set(True)
    bpy.ops.export_scene.gltf(filepath=str(OUT/'wolf-hands.glb'),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT/'wolf-hands-editable.blend'))

    power={}
    for finger,angles in {'index':[.55,1.20,1.00],'middle':[.60,1.40,1.10],'ring':[.55,1.25,1.05],'pinky':[.30,.95,.60]}.items():
        for i,a in enumerate(angles):power[f'{finger}_{i}']=a
    power.update(thumb_0=dict(opposition=1.78,curl=.27),thumb_1=.40)
    pinch={}
    for finger,angles in {'index':[.30,1.30,1.20],'middle':[.28,.60,.45],'ring':[.40,.70,.55],'pinky':[.55,.80,.65]}.items():
        for i,a in enumerate(angles):pinch[f'{finger}_{i}']=a
    pinch.update(thumb_0=dict(opposition=.80,curl=.22),thumb_1=.72)
    preview('hands-open',{},objects,overview=True);preview('right-open-detail',{},objects)

    # Key is authored with ring center at origin, shaft+X, ring in XY plane.
    key=geom();torus(key,(0,0,0),.016,.018,.0032,'iron','XY',24,6);capsule(key,.015,.138,.0045,.0045,'iron',taper=.9,sides=10)
    torus(key,(.029,0,0),.006,.006,.0018,'edge','YZ',12,5)
    for x,h in [(.111,.014),(.124,.024),(.135,.018)]:box(key,(x,-h*.5-.004,0),(.009,h,.008),'iron')
    box(key,(.124,-.014,0),(.030,.005,.010),'edge')
    key_obj=object_from_geom('iron_key',key,mat)
    bpy.ops.object.select_all(action='DESELECT');key_obj.select_set(True)
    bpy.ops.export_scene.gltf(filepath=str(OUT/'iron-key.glb'),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
    key_obj.hide_render=True
    handle=geom();axis=np.array([-.22,0,.9755]);axis/=np.linalg.norm(axis);handle_frame=basis(axis);contact=np.array([.101,-.030,0]);capsule(handle,-.065,.065,.014,.014,'leather',contact,handle_frame,taper=1,sides=16)
    proxy=object_from_geom('HandleProxy',handle,mat);power_matrices=preview('right-power-grip',power,objects,proxy)
    key_obj.hide_render=False;key_obj.location=B((.091,-.043,-.044));pinch_matrices=preview('right-key-pinch',pinch,objects,key_obj)

    parts=[]
    for side in ('r','l'):
        for original in RIG:
            row=json.loads(json.dumps(original));row['name']=f'hand_{side}_{row["name"]}';row['parent']=f'hand_{side}_{row["parent"]}' if row['parent'] else None
            if side=='l':
                for field in ['pivot','tip','endpoint']:
                    if field in row:row[field][2]*=-1
                for field in ['curl_axis','opposition_axis']:
                    if row.get(field):row[field]=[-row[field][0],-row[field][1],row[field][2]]
            parts.append(row)
    contacts=dict(power_grip=dict(point=[.101,-.030,0],axis=axis.tolist(),radius=.014),pinch=dict(point=[.091,-.043,-.044],item_ring_radius=.016,item_contact_point=[.107,-.043,-.044]))
    audit={}
    for label,matrices in [('power_grip',power_matrices),('pinch',pinch_matrices)]:
        audit[label]={r['name']:list(matrices[r['name']]@Vector(r['tip'])) for r in RIG if r.get('tip') and (r['name'].endswith('_2') or r['name']=='thumb_1')}
    rig=dict(schema_version=1,units='meters',axes='X fingers-forward, Y dorsal/up, Z right; right thumb-Z, left thumb+Z',geometry_space='all vertices in absolute hand bind coordinates, node transforms identity',shoulder=[-.57,0,0],elbow=[-.27,0,0],wrist=[0,0,0],parts=parts,presets=dict(open={},power_grip=power,pinch=pinch),contacts_right=contacts,contacts_left=dict(power_grip=dict(point=[.101,-.030,0],axis=[axis[0],axis[1],-axis[2]],radius=.014),pinch=dict(point=[.091,-.043,.044],item_ring_radius=.016,item_contact_point=[.107,-.043,.044])),key=dict(file='iron-key.glb',ring_center=[0,0,0],shaft_axis=[1,0,0],ring_normal=[0,0,1],grip_anchor=[0,0,0]),pose_formula='Dchild = Dparent * T(bind_pivot) * R(opposition_axis,opposition) * R(curl_axis,curl) * T(-bind_pivot). Draw wrist_world * Dchild on bind-coordinate vertices. IK segment draw: joint_world - rotation*bind_pivot.',tip_audit_right=audit,build_seconds=round(time.perf_counter()-START,2))
    (OUT/'hands-rig.json').write_text(json.dumps(rig,indent=2),encoding='utf-8')
    print('BUILD_SECONDS',round(time.perf_counter()-START,2),flush=True)

if __name__ == '__main__':
    main()
