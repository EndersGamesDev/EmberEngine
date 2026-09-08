"""Metric rigid warden, chair and hand-mounted knife. Outputs only to V5 scratch work."""
import os,sys,json,time,math
from pathlib import Path
import bpy,numpy as np
from mathutils import Vector
OUT=Path(os.environ['END_GAME_V5_ASSET_WORK']);OUT.mkdir(parents=True,exist_ok=True)
os.environ.setdefault('END_GAME_V3_ASSET_WORK',str(OUT))
sys.path.insert(0,str(Path(__file__).resolve().parent))
import v3_build_hands as h
START=time.perf_counter()
MATS=['cloth','leather','skin','skinshade','iron','steel','oak','stitch','dark','eye','rust','nail','scar','shirt','sole','brass']
PARTS=[];OBJECTS={}

def frame(direction):
    d=Vector(direction).normalized();guide=Vector((0,0,-1))
    if abs(d.dot(guide))>.95:guide=Vector((0,1,0))
    u=(guide-d*guide.dot(d)).normalized();return np.array([d[:],u[:],d.cross(u)[:]]).T

def tube(g,a,b,front,wide=None,material='cloth',sides=12,taper=.90):
    a=np.array(a);b=np.array(b);h.capsule(g,0,float(np.linalg.norm(b-a)),front,wide or front,material,a,frame(b-a),taper,sides)

def sphere(g,p,r,mat='skin',segments=12,rings=6):h.ellipsoid(g,p,r,mat,segments,rings)

def seam(g,a,b,r=.0018,mat='stitch'):tube(g,a,b,r,material=mat,sides=5,taper=1)

def make_object(name,g):
    mesh=bpy.data.meshes.new(name);v=np.array(g['verts']);mesh.from_pydata([h.B(x) for x in v],[],g['faces']);mesh.update();uv=mesh.uv_layers.new(name='UVMap')
    for poly,material,smooth in zip(mesh.polygons,g['mat'],g['smooth']):
        poly.use_smooth=smooth;row,col=divmod(MATS.index(material),4)
        indices=list(poly.vertices);normal=np.abs(np.cross(v[indices[1]]-v[indices[0]],v[indices[2]]-v[indices[0]]));axes=[i for i in range(3) if i!=np.argmax(normal)]
        for loop in poly.loop_indices:
            p=v[mesh.loops[loop].vertex_index];u=(p[axes[0]]*4.7)%1;w=(p[axes[1]]*4.7)%1
            uv.data[loop].uv=(col*.25+.016+u*.218,1-(row*.25+.016+w*.218))
    mesh.materials.append(MATERIAL);obj=bpy.data.objects.new(name,mesh);bpy.context.collection.objects.link(obj);OBJECTS[name]=obj;return obj

def part(name,g,parent,pivot,**extra):
    full='warden_'+name;make_object(full,g);PARTS.append(dict(name=full,parent='warden_'+parent if parent else None,pivot=list(pivot),**extra))

def body():
    g=h.geom();sphere(g,(0,.99,0),(.185,.145,.118),'cloth',16,8)
    for y in [.992,1.018]:seam(g,(-.16,y,-.112),(.16,y,-.112),.010,'leather')
    for x in [-.033,.033]:h.box(g,(x,1.005,-.127),(.009,.043,.009),'iron')
    for y in [.988,1.023]:h.box(g,(0,y,-.127),(.07,.008,.009),'iron')
    h.box(g,(.011,1.005,-.134),(.04,.005,.006),'brass')
    # Empty sheath remains on the right belt as the knife is drawn.
    tube(g,(.20,1.015,.010),(.20,.745,.010),.025,.033,'leather',8)
    part('pelvis',g,None,(0,1,0))
    g=h.geom();tube(g,(0,1.045,0),(0,1.51,0),.155,.260,'cloth',16,.94)
    sphere(g,(0,1.42,0),(.238,.103,.145),'leather',16,8)
    for side in [-1,1]:
        sphere(g,(side*.192,1.445,0),(.127,.108,.102),'cloth',12,6)
        tube(g,(side*.102,1.09,-.141),(side*.102,1.47,-.131),.029,.104,'leather',12,.88)
        seam(g,(side*.025,1.105,-.141),(side*.032,1.455,-.113),.0022)
        seam(g,(side*.183,1.17,-.105),(side*.19,1.40,-.09),.002)
        seam(g,(side*.042,1.405,-.139),(side*.12,1.49,-.087),.009,'leather')
        h.box(g,(side*.124,1.238,-.153),(.092,.055,.012),'cloth')
        seam(g,(side*.080,1.265,-.160),(side*.164,1.265,-.160),.002)
    for y in [1.14,1.22,1.30,1.38]:sphere(g,(.022,y,-.158),(.006,.006,.003),'iron',8,4)
    # Broad diagonal worn strap and small rectangular keeper.
    tube(g,(-.17,1.45,-.10),(.15,1.12,-.14),.008,.023,'leather',6,1)
    h.box(g,(-.035,1.32,-.160),(.035,.039,.012),'iron')
    h.box(g,(-.035,1.32,-.167),(.022,.025,.006),'leather')
    part('torso',g,'pelvis',(0,1.06,0))
    g=h.geom();tube(g,(0,1.49,0),(0,1.635,-.004),.070,.075,'skin',12)
    sphere(g,(0,1.548,-.003),(.084,.061,.073),'cloth',12,6)
    for side in [-1,1]:h.box(g,(side*.042,1.529,-.043),(.06,.043,.033),'cloth')
    part('neck',g,'torso',(0,1.52,0))
    g=h.geom();sphere(g,(0,1.725,-.012),(.108,.140,.105),'skin',20,12);sphere(g,(0,1.638,-.035),(.079,.062,.072),'skin',16,8)
    sphere(g,(0,1.616,-.085),(.057,.030,.022),'skinshade',12,6)
    for side in [-1,1]:
        sphere(g,(side*.110,1.715,-.007),(.023,.039,.018),'skin',10,6)
        sphere(g,(side*.121,1.715,-.022),(.010,.026,.005),'skinshade',8,4)
        sphere(g,(side*.057,1.691,-.098),(.027,.023,.010),'skin',12,6)
        sphere(g,(side*.044,1.738,-.107),(.029,.019,.017),'skinshade',12,6)
        sphere(g,(side*.044,1.737,-.119),(.017,.005,.004),'eye',12,6)
        sphere(g,(side*.043,1.737,-.123),(.004,.004,.002),'dark',8,4)
        tube(g,(side*.066,1.756,-.108),(side*.023,1.750,-.118),.008,.011,'skin',8)
        seam(g,(side*.064,1.757,-.119),(side*.026,1.751,-.127),.003,'dark')
        sphere(g,(side*.013,1.684,-.156),(.006,.004,.004),'skinshade',8,4)
    sphere(g,(0,1.712,-.123),(.013,.033,.018),'skin',12,6)
    sphere(g,(0,1.691,-.141),(.017,.012,.014),'skin',12,6)
    seam(g,(-.024,1.654,-.113),(.023,1.654,-.114),.003,'dark')
    seam(g,(-.020,1.660,-.111),(.020,1.660,-.111),.0028,'skinshade')
    seam(g,(-.016,1.649,-.114),(.018,1.649,-.114),.003,'skin')
    for i in range(3):seam(g,(-.057+i*.006,1.691-i*.010,-.122),(-.034+i*.005,1.680-i*.010,-.123),.0018,'scar')
    for y in [1.790,1.804]:seam(g,(-.041,y,-.111),(.041,y,-.111),.0013,'skinshade')
    part('head',g,'neck',(0,1.60,-.002))
    for side,suffix in [(1,'r'),(-1,'l')]:
        x=side*.26
        g=h.geom();tube(g,(x,1.465,0),(x,1.13,0),.077,.089,'cloth',12,.78)
        sphere(g,(x,1.435,0),(.096,.092,.084),'leather',12,6)
        seam(g,(x+side*.074,1.405,-.023),(x+side*.057,1.205,-.023),.002)
        part('upperarm_'+suffix,g,'torso',(x,1.46,0),endpoint=[x,1.14,0],length=.32)
        g=h.geom();tube(g,(x,1.152,0),(x,.845,0),.058,.062,'cloth',12,.75)
        sphere(g,(x,1.14,0),(.059,.059,.056),'leather',10,6)
        if side==1:
            tube(g,(x,1.035,-.003),(x,.898,-.003),.061,.065,'iron',10,.89)
            for y in [.919,.995]:
                for dx in [-.040,.040]:sphere(g,(x+dx,y,-.049),(.004,.004,.003),'rust',8,4)
        else:
            for y in [.93,.97,1.01]:tube(g,(x-.048,y,-.02),(x+.048,y,-.02),.008,.009,'leather',8)
        part('forearm_'+suffix,g,'upperarm_'+suffix,(x,1.14,0),endpoint=[x,.85,0],length=.29)
        g=h.geom();sphere(g,(x,.812,0),(.041,.049,.021),'skin',12,6)
        sphere(g,(x,.85,0),(.033,.036,.028),'skin',10,6)
        for offset in [-.027,-.009,.009,.027]:
            a=(x+offset,.816,-.013);b=(x+offset,.785,-.036);c=(x+offset,.770,-.012);d=(x+offset,.790,.003)
            for start,end in [(a,b),(b,c),(c,d)]:tube(g,start,end,.0085,material='skin',sides=8,taper=.87)
            sphere(g,b,(.010,.010,.009),'skin',8,4)
        tube(g,(x+side*.037,.827,-.006),(x+side*.036,.794,-.041),.011,material='skin',sides=8)
        tube(g,(x+side*.036,.794,-.041),(x+side*.007,.787,-.046),.010,material='skin',sides=8)
        part('hand_'+suffix,g,'forearm_'+suffix,(x,.85,0),knife_socket=[x,.782,-.012] if side==1 else None)
        hip=(side*.105,.97,0);knee=(side*.105,.54,0);ankle=(side*.105,.11,0)
        g=h.geom();tube(g,hip,knee,.094,.092,'cloth',12,.73)
        seam(g,(side*.105+side*.084,.90,.006),(side*.105+side*.064,.60,.006),.002)
        part('thigh_'+suffix,g,'pelvis',hip,endpoint=list(knee),length=.43)
        g=h.geom();tube(g,(side*.105,.565,0),ankle,.060,.062,'cloth',12,.69)
        sphere(g,(side*.105,.535,-.035),(.060,.070,.035),'leather',10,6)
        part('shin_'+suffix,g,'thigh_'+suffix,knee,endpoint=list(ankle),length=.43)
        g=h.geom();sphere(g,(side*.105,.09,-.053),(.063,.076,.135),'leather',12,6)
        h.box(g,(side*.105,.022,-.052),(.127,.042,.251),'sole')
        tube(g,(side*.105,.09,0),(side*.105,.24,0),.063,.065,'leather',12,.92)
        for y in [.105,.135,.165]:
            seam(g,(side*.105-.025,y,-.060),(side*.105+.025,y+.010,-.060),.002,'dark')
            seam(g,(side*.105+.025,y,-.062),(side*.105-.025,y+.010,-.062),.002,'stitch')
        part('boot_'+suffix,g,'shin_'+suffix,ankle,sole_y=0.0)
        g=h.geom();sphere(g,(side*.118,.813,.064),(.113,.188,.066),'leather',12,6)
        seam(g,(side*.201,.930,.080),(side*.205,.672,.080),.0022)
        part('coat_'+suffix,g,'thigh_'+suffix,hip)

def props():
    g=h.geom()
    # Seat is behind the planted feet; origin remains the character's initial floor anchor.
    for x in [-.255,.255]:
        for z in [.115,.595]:
            tube(g,(x,0,z),(x,.56,z),.034,.038,'oak',8,1)
            h.box(g,(x,.10,z),(.078,.10,.075),'iron')
        tube(g,(x,.51,.61),(x,1.37,.68),.030,.038,'oak',8,1)
        for z in [.15,.55]:tube(g,(x,.52,z),(x,.78,z),.025,material='oak',sides=8,taper=1)
        h.box(g,(x,.79,.36),(.083,.043,.55),'oak')
    for x in [-.19,-.095,0,.095,.19]:h.box(g,(x,.51,.365),(.087,.06,.56),'oak')
    for y in [.72,.95,1.20]:h.box(g,(0,y,.643+(y-.51)*.075),(.49,.105,.035),'oak')
    for z in [.13,.59]:h.box(g,(0,.23,z),(.55,.055,.055),'oak')
    for x in [-.235,.235]:
        for y in [.735,.965,1.215]:sphere(g,(x,y,.608+(y-.51)*.075),(.008,.008,.004),'iron',8,4)
    part('chair',g,None,(0,0,0),stationary=True,seat_height=.54,origin_world=[-2.9,0,-3.7],yaw=math.pi)
    g=h.geom();anchor=np.array([.26,.782,-.012])
    h.capsule(g,-.050,.050,.011,.011,'leather',anchor,taper=1,sides=10)
    for x in [-.045,.045]:h.torus(g,anchor+np.array([x,0,0]),.012,.012,.002,'iron','YZ',12,5)
    h.box(g,anchor+np.array([.056,0,0]),(.012,.042,.045),'iron')
    # Thick spine, irregular clipped point, and an actual beveled edge.
    v=[(.061,-.009,-.022),(.285,-.009,-.019),(.375,0,0),(.268,-.009,.034),(.061,-.009,.034),(.061,.009,-.022),(.285,.009,-.019),(.268,.009,.034),(.061,.009,.034)]
    f=[(0,4,3,2,1),(5,6,2,7,8),(0,1,6,5),(1,2,6),(4,8,7,3),(3,7,2),(0,5,8,4)]
    h.add(g,v,[tuple(reversed(face)) for face in f],'steel',pivot=anchor)
    seam(g,anchor+np.array([.074,.0095,-.012]),anchor+np.array([.260,.0095,-.010]),.001,'iron')
    part('knife',g,'hand_r',anchor,grip_anchor=anchor.tolist(),blade_axis=[1,0,0],blade_tip=(anchor+np.array([.375,0,0])).tolist())

def preview():
    scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=20;scene.render.threads_mode='FIXED';scene.render.threads=4
    scene.render.resolution_x=900;scene.render.resolution_y=1100;scene.render.resolution_percentage=100;scene.view_settings.view_transform='AgX'
    scene.world=bpy.data.worlds.new('Studio');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.14,.16,.18,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.4
    target=Vector(h.B((0,.99,0)));bpy.ops.object.camera_add(location=h.B((2.1,1.8,-3.8)));cam=bpy.context.object;cam.data.type='ORTHO';cam.data.ortho_scale=2.25;cam.rotation_euler=(target-cam.location).to_track_quat('-Z','Y').to_euler();scene.camera=cam
    for p,power in [((1,3,-2),220),((-2,1.4,-.5),140)]:
        bpy.ops.object.light_add(type='AREA',location=h.B(p));o=bpy.context.object;o.data.energy=power;o.data.size=2;o.rotation_euler=(target-o.location).to_track_quat('-Z','Y').to_euler()
    OBJECTS['warden_chair'].hide_render=True
    scene.render.filepath=str(OUT/'warden-standing-preview.png');bpy.ops.render.render(write_still=True)

bpy.ops.wm.read_factory_settings(use_empty=True)
MATERIAL=bpy.data.materials.new('WardenAtlas');MATERIAL.use_nodes=True;bs=MATERIAL.node_tree.nodes['Principled BSDF'];bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Roughness'].default_value=.78;bs.inputs['Metallic'].default_value=.07
tex=MATERIAL.node_tree.nodes.new('ShaderNodeTexImage');tex.image=bpy.data.images.load(str(OUT/'warden-atlas.png'));tex.image.pack();MATERIAL.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
body();props();bpy.ops.object.select_all(action='DESELECT')
for o in OBJECTS.values():o.select_set(True)
bpy.ops.export_scene.gltf(filepath=str(OUT/'warden.glb'),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
(OUT/'warden-rig.json').write_text(json.dumps(dict(schema_version=1,units='meters',facing='-Z',up='+Y',geometry_space='standing bind coordinates; identity node transforms',height=1.865,parts=PARTS),indent=2),encoding='utf-8',newline='\n')
preview();print('WARDEN_BUILD_SECONDS',round(time.perf_counter()-START,3),flush=True)
