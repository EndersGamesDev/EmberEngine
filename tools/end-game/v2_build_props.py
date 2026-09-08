import os
"""Build the detailed cell kit: closed surfaces, separate props, baked RGB atlases."""
import bpy,bmesh,math,random,time,json,sys
import numpy as np
from mathutils import Vector
from pathlib import Path
OUT=Path(os.environ['END_GAME_V2_ASSET_WORK'])/'props'; MATS=OUT/'material-sources'; START=time.perf_counter()
R=random.Random(28092026); OBJECTS=[]; MATERIALS={}; REPORT=[]

def material(name):
    if name in MATERIALS:return MATERIALS[name]
    m=bpy.data.materials.new(name); m.use_nodes=True
    nodes=m.node_tree.nodes; tex=nodes.new('ShaderNodeTexImage'); tex.image=bpy.data.images.load(str(MATS/(name+'.png')))
    uv=nodes.new('ShaderNodeUVMap'); uv.uv_map='UVMap'; m.node_tree.links.new(uv.outputs[0],tex.inputs['Vector'])
    bs=nodes.get('Principled BSDF'); bs.inputs['Roughness'].default_value=.85; m.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    MATERIALS[name]=m; return m

def finish(obj,mat,bevel=0,smooth=False):
    bpy.ops.object.select_all(action='DESELECT'); obj.select_set(True); bpy.context.view_layer.objects.active=obj
    bpy.ops.object.transform_apply(location=False,rotation=False,scale=True)
    if bevel:
        mod=obj.modifiers.new('WornEdges','BEVEL'); mod.width=bevel; mod.segments=2
        bpy.ops.object.modifier_apply(modifier=mod.name)
    if not obj.data.uv_layers: obj.data.uv_layers.new(name='UVMap')
    obj.data.uv_layers.active.name='UVMap'
    obj.data.materials.clear(); obj.data.materials.append(material(mat))
    for p in obj.data.polygons:p.use_smooth=smooth
    if mat=='oak':wood_uv(obj)
    OBJECTS.append(obj); return obj

def wood_uv(obj):
    coordinates=np.array([v.co[:] for v in obj.data.vertices]);lo=coordinates.min(0);extent=coordinates.max(0)-lo
    uv=obj.data.uv_layers.active.data;offset=((len(OBJECTS)*37)%100)/800
    for polygon in obj.data.polygons:
        normal_axis=max(range(3),key=lambda i:abs(polygon.normal[i]));axes=[i for i in range(3) if i!=normal_axis];v=max(axes,key=lambda i:extent[i]);u=next(i for i in axes if i!=v)
        for loop in polygon.loop_indices:
            co=obj.data.vertices[obj.data.loops[loop].vertex_index].co
            uv[loop].uv=(.04+offset+(co[u]-lo[u])/max(extent[u],.00001)*.70,.05+(co[v]-lo[v])/max(extent[v],.00001)*.88)

def mesh(name,verts,faces,mat,smooth=False):
    data=bpy.data.meshes.new(name); data.from_pydata(verts,[],faces); data.update()
    obj=bpy.data.objects.new(name,data); bpy.context.collection.objects.link(obj)
    finish(obj,mat,0,smooth)
    # Source UVs project along the two dominant bounds axes of each face.
    uv=data.uv_layers.active.data
    for polygon in ([] if mat=='oak' else data.polygons):
        axis=max(range(3),key=lambda i:abs(polygon.normal[i])); a,b=[i for i in range(3) if i!=axis]
        for loop in polygon.loop_indices:
            co=data.vertices[data.loops[loop].vertex_index].co; uv[loop].uv=(co[a]*2.2+1.7,co[b]*2.2+.3)
    return obj

def box(name,location,size,mat='oak',bevel=.007,rot=None):
    bpy.ops.mesh.primitive_cube_add(size=1,location=location); obj=bpy.context.object; obj.name=name; obj.scale=size
    if rot: obj.rotation_euler=rot
    return finish(obj,mat,bevel)

def cylinder(name,location,radius,depth,mat='iron',verts=16,rot=None,bevel=.002):
    bpy.ops.mesh.primitive_cylinder_add(vertices=verts,radius=radius,depth=depth,location=location); obj=bpy.context.object; obj.name=name
    if rot:obj.rotation_euler=rot
    return finish(obj,mat,bevel,True)

def ellipsoid(name,location,size,mat,segments=16,rings=8):
    bpy.ops.mesh.primitive_uv_sphere_add(segments=segments,ring_count=rings,radius=1,location=location); obj=bpy.context.object; obj.name=name; obj.scale=size
    return finish(obj,mat,0,True)

def tube(name,points,radius,mat='iron',sides=6,closed=False):
    points=[Vector(p) for p in points]; verts=[]; count=len(points)
    for i,p in enumerate(points):
        t=(points[(i+1)%count]-points[(i-1)%count]).normalized() if closed or 0<i<count-1 else (points[1]-points[0] if i==0 else points[-1]-points[-2]).normalized()
        up=Vector((0,0,1)) if abs(t.z)<.95 else Vector((0,1,0)); u=t.cross(up).normalized(); v=t.cross(u).normalized()
        for k in range(sides): verts.append(tuple(p+radius*(math.cos(k*2*math.pi/sides)*u+math.sin(k*2*math.pi/sides)*v)))
    faces=[]
    for i in range(count if closed else count-1):
        for k in range(sides):faces.append((i*sides+k,i*sides+(k+1)%sides,((i+1)%count)*sides+(k+1)%sides,((i+1)%count)*sides+k))
    if not closed:faces.extend([tuple(reversed(range(sides))),tuple((count-1)*sides+k for k in range(sides))])
    return mesh(name,verts,faces,mat,True)

def ellipse(name,center,rx,ry,radius,mat='iron',plane='XY',segments=24,sides=6):
    points=[]
    for i in range(segments):
        a=i*math.tau/segments; c=rx*math.cos(a); s=ry*math.sin(a)
        p=(c,s,0) if plane=='XY' else ((c,0,s) if plane=='XZ' else (0,c,s)); points.append(tuple(Vector(center)+Vector(p)))
    return tube(name,points,radius,mat,sides,True)

def soft_box(name,location,size,mat,segments=24,rings=12):
    verts=[]; faces=[]
    # Rounded, sewn sacks with shallow deterministic surface wrinkling.
    for j in range(rings+1):
        b=-math.pi/2+j*math.pi/rings
        for i in range(segments):
            a=i*math.tau/segments; exponent=.27
            cp=math.copysign(abs(math.cos(b))**exponent,math.cos(b)); sp=math.copysign(abs(math.sin(b))**exponent,math.sin(b))
            x=math.copysign(abs(math.cos(a))**.32,math.cos(a))*cp*size[0]/2
            y=math.copysign(abs(math.sin(a))**.32,math.sin(a))*cp*size[1]/2
            z=sp*size[2]/2+.003*math.sin(a*9+b*5)
            verts.append(tuple(Vector(location)+Vector((x,y,z))))
    for j in range(rings):
        for i in range(segments):faces.append((j*segments+i,j*segments+(i+1)%segments,(j+1)*segments+(i+1)%segments,(j+1)*segments+i))
    return mesh(name,verts,faces,mat,True)

def cloth(name,origin,width,length,mat='rag',hanging=True,nx=24,ny=26):
    verts=[]; faces=[]
    for j in range(ny+1):
        v=j/ny
        for i in range(nx+1):
            u=i/nx; x=(u-.5)*width
            if hanging:
                y=.024*math.sin(u*math.tau*4+v*1.8)*(.3+.7*v); z=-v*length
                if j==ny:z+=.035*math.sin(i*2.1)+.017*math.sin(i*5)
                if i in (0,nx):x+=.016*math.sin(j*2.4)
            else:
                y=(v-.5)*length; z=.018*math.sin(u*math.tau*4+v*2)+.025*math.sin(v*4)
            verts.append(tuple(Vector(origin)+Vector((x,y,z))))
    for j in range(ny):
        for i in range(nx):
            if hanging and ((i-7)**2/3+(j-17)**2/5<1 or (i-18)**2/2+(j-21)**2/3<1):continue
            a=j*(nx+1)+i;faces.append((a,a+1,a+nx+2,a+nx+1))
    obj=mesh(name,verts,faces,mat,True)
    bpy.context.view_layer.objects.active=obj; bpy.ops.object.select_all(action='DESELECT'); obj.select_set(True)
    mod=obj.modifiers.new('OpaqueClothThickness','SOLIDIFY'); mod.thickness=.002; bpy.ops.object.modifier_apply(modifier=mod.name)
    return obj

def cot():
    for x in (-.445,.445):
        for y in (-.995,.995):
            box('PeggedLeg',(x,y,.265),(.065,.075,.53),bevel=.011)
            cylinder('IronNail',(x, y+(.043 if y>0 else -.043),.40),.012,.006,verts=8,rot=(math.pi/2,0,0),bevel=.001)
        box('LongSideRail',(x,0,.39),(.055,2.1,.14),bevel=.009)
    for y in (-1.0,1.0):box('EndRail',(0,y,.40),(.92,.055,.15),bevel=.009)
    for j in range(12):box('BedSlat',(0,-.92+j*.167,.426),(.85,.125,.027),bevel=.003)
    for x in (-.445,.445):box('HeadPost',(x,-1,.73),(.068,.075,.62),bevel=.011)
    for z in (.65,.91):box('HeadCrossbar',(0,-1,z),(.91,.055,.07),bevel=.006)
    for x in (-.21,0,.21):box('HeadSlat',(x,-1,.77),(.065,.035,.31),bevel=.004)
    soft_box('StrawTick',(0,0,.554),(.855,1.96,.205),'linen')
    soft_box('LumpyPillow',(.02,-.62,.70),(.60,.39,.14),'linen',24,10)
    # Raised sewn seam and visible whipstitches around mattress perimeter.
    seam=[]
    for i in range(80):
        a=i*math.tau/80; seam.append((math.copysign(abs(math.cos(a))**.35,math.cos(a))*.426,math.copysign(abs(math.sin(a))**.35,math.sin(a))*.98,.58))
    tube('SewnEdge',seam,.004,'straw',4,True)
    for i in range(64):
        y=-.9+(i%32)*.058; x=.429 if i<32 else -.429
        tube('Whipstitch',[(x,y-.006,.565),(x*1.004,y,.581),(x,y+.007,.596)],.0018,'dark',3)
    cloth('FoldedBlanket',(0,.52,.668),.79,.70,'rag',False,20,14)
    for i in range(90):
        y=R.uniform(-.94,.94); x=R.choice([-1,1])*.43; z=R.uniform(.47,.59)
        tube('EscapingStraw',[(x,y,z),(x+math.copysign(R.uniform(.02,.06),x),y+R.uniform(-.025,.025),z-.01),(x+math.copysign(R.uniform(.035,.085),x),y+R.uniform(-.04,.04),z-.018)],.0015,'straw',3)
    return dict(pivot=[0,0,0],mattress_top=[0,.66,0],head_end='positive engine Z',collision='box approximately0.96X x0.70Y x2.13Z; headboard reaches1.04Y')

def bucket():
    n=16
    for i in range(n):
        a0=i*math.tau/n+.007; a1=(i+1)*math.tau/n-.007; vertices=[]
        for z,r in [(0,.165),(.405,.224)]:
            for radius in (r,r-.021):
                for a in (a0,a1):vertices.append((radius*math.cos(a),radius*math.sin(a),z))
        mesh('OakStave',vertices,[(0,1,5,4),(3,2,6,7),(0,4,6,2),(1,3,7,5),(4,5,7,6),(0,2,3,1)],'oak')
    cylinder('WoodenBottom',(0,0,.025),.164,.04,'oak',32,bevel=.003)
    for z,r in [(.075,.181),(.335,.221)]:
        # Wide forged bands, with visible rivets and rolled edges.
        verts=[];faces=[]
        for zz,rr in [(z-.018,r),(z+.018,r),(z-.018,r+.009),(z+.018,r+.009)]:
            for i in range(48):a=i*math.tau/48;verts.append((rr*math.cos(a),rr*math.sin(a),zz))
        for i in range(48):
            j=(i+1)%48; faces.extend([(i,j,48+j,48+i),(96+i,144+i,144+j,96+j),(i,96+i,96+j,j),(48+i,48+j,144+j,144+i)])
        mesh('IronHoop',verts,faces,'iron',True)
        for i in range(12):
            a=i*math.tau/12;ellipsoid('HoopRivet',((r+.012)*math.cos(a),(r+.012)*math.sin(a),z),(.008,.008,.008),'iron',8,4)
    for x in (-.236,.236):ellipsoid('HandlePin',(x,0,.351),(.013,.016,.016),'iron',10,5)
    points=[(.237*math.cos(i*math.pi/24),0,.352+.30*math.sin(i*math.pi/24)) for i in range(25)]
    tube('BailHandle',points,.008,'iron',8)
    return dict(pivot=[0,0,0],handle_axis=[0,.352,0],collision='cylinder radius0.25m,height0.42m; handle is decor')

def chain():
    # Blender Y is normal to wall; export yields engine -Z. Origin at anchor.
    box('WallAnchor',(0,.015,0),(.105,.035,.13),'iron',.009)
    for x in (-.032,.032):
        for z in (-.045,.045):ellipsoid('MountRivet',(x,-.008,z),(.010,.006,.010),'iron',8,4)
    ellipse('WallRing',(0,-.02,0),.038,.051,.008,'iron','XZ',20,6)
    for i in range(10):
        z=-.052-i*.056; plane='XZ' if i%2==0 else 'YZ';ellipse('ChainLink',(0,-.027,z),.023,.040,.0065,'iron',plane,16,6)
    ellipse('CuffConnector',(0,-.027,-.612),.020,.030,.0065,'iron','YZ',16,6)
    ellipse('WristShackle',(0,-.027,-.71),.089,.072,.014,'iron','XZ',28,8)
    box('CuffLock',(.079,-.027,-.712),(.065,.049,.044),'iron',.007)
    cylinder('LockPin',(.09,-.027,-.684),.01,.01,'iron',8,bevel=.002)
    return dict(pivot=[0,0,0],animation='Sway whole mesh around origin: rotation about X or Z, amplitude<=0.04rad; top is wall anchor',mount_normal=[0,0,1])

def rag():
    cloth('TatteredLinen',(0,0,0),.67,.76,'rag',True,26,28)
    for x in (-.25,.25):
        ellipse('HemLoop',(x,.006,-.01),.018,.022,.003,'linen','XZ',12,4)
    return dict(pivot=[0,0,0],animation='Top-hem origin: slow0.02rad X/Z sway; mesh hangs in negative Y',mount_normal=[0,0,1])

def drain():
    for x in (-.224,.224):box('GrateFrame',(x,0,.025),(.028,.40,.046),'iron',.005)
    for y in (-.185,.185):box('GrateFrame',(0,y,.025),(.475,.031,.046),'iron',.005)
    for i in range(8):box('ForgedBar',(-.188+i*.054,0,.022),(.017,.344,.024),'iron',.004)
    box('UndersideTie',(0,0,.012),(.42,.022,.017),'iron',.003)
    for x in (-.223,.223):
        for y in (-.16,.16):cylinder('HexBolt',(x,y,.05),.012,.008,'iron',6,bevel=.001)
    return dict(pivot=[0,0,0],collision='decor inset into floor; full height0.054m',animation='static')

def candle_stool():
    for x in (-.195,.195):
        for y in (-.195,.195):box('StoolLeg',(x,y,.278),(.052,.054,.556),'oak',.007,rot=(y*.17,-x*.17,0))
    for y in (-.195,.195):box('StoolBrace',(0,y,.20),(.44,.035,.038),'oak',.004)
    for x in (-.195,.195):box('StoolBrace',(x,0,.32),(.035,.44,.038),'oak',.004)
    for i in range(4):box('SeatPlank',(-.195+i*.13,0,.585),(.125,.52,.049),'oak',.007)
    for x in (-.19,.19):
        for y in (-.19,.19):cylinder('SeatNail',(x,y,.614),.006,.003,'iron',8,bevel=0)
    cylinder('TinSaucer',(0,0,.63),.132,.015,'iron',32,bevel=.004)
    ellipse('SaucerLip',(0,0,.64),.127,.127,.007,'iron','XY',32,6)
    vertices=[];faces=[];segments=24;rings=10
    for j in range(rings+1):
        z=.639+j*.025
        for i in range(segments):
            a=i*math.tau/segments; radius=.035+.003*math.sin(a*5+j*.7)+.002*math.cos(a*9)
            zz=z+(.005*math.sin(a*3) if j==rings else 0); vertices.append((radius*math.cos(a),radius*math.sin(a),zz))
    for j in range(rings):
        for i in range(segments):faces.append((j*segments+i,j*segments+(i+1)%segments,(j+1)*segments+(i+1)%segments,(j+1)*segments+i))
    faces.append(tuple(reversed(range(segments)))); faces.append(tuple(rings*segments+i for i in range(segments)))
    mesh('DrippingCandle',vertices,faces,'wax',True)
    for i in range(9):
        a=i*2.4; h=R.uniform(.03,.12); z=R.uniform(.72,.82)
        ellipsoid('WaxDrip',(.038*math.cos(a),.038*math.sin(a),z),(.008,.008,h/2),'wax',10,6)
    for i in range(5):ellipsoid('WaxPuddle',(R.uniform(-.045,.045),R.uniform(-.045,.045),.652),(.03,.023,.006),'wax',12,5)
    tube('CharredWick',[(0,0,.885),(.004,0,.906),(.003,.002,.915)],.0025,'dark',5)
    return dict(pivot=[0,0,0],flame_anchor=[.003,.927,-.002],animation='Particle flame center above wick; point light anchor atY0.96. Stool static.',collision='box0.52x0.615x0.52m; candle is decor')

def straw_scatter():
    for i in range(105):
        x=R.uniform(-.38,.38);y=R.uniform(-.24,.24);a=R.random()*math.tau;l=R.uniform(.05,.18);z=R.uniform(.003,.016)
        tube('LooseStraw',[(x,y,z),(x+math.cos(a)*l*.5,y+math.sin(a)*l*.5,z+.004),(x+math.cos(a)*l,y+math.sin(a)*l,.003)],.0013,'straw',3)
    return dict(pivot=[0,0,0],collision='none, floor dressing',animation='static')

def bake_join(name,res):
    bpy.ops.object.select_all(action='DESELECT')
    for o in OBJECTS:o.select_set(True)
    bpy.context.view_layer.objects.active=OBJECTS[0];bpy.ops.object.join();obj=bpy.context.object;obj.name=name;obj.data.name=name
    bpy.ops.object.transform_apply(location=True,rotation=True,scale=True)
    source_uv=obj.data.uv_layers.active;source_uv.name='UVMap'
    atlas=obj.data.uv_layers.new(name='Atlas');obj.data.uv_layers.active=atlas;atlas.active_render=True
    bpy.ops.object.mode_set(mode='EDIT');bpy.ops.mesh.select_all(action='SELECT');bpy.ops.uv.smart_project(angle_limit=math.radians(68),island_margin=.008,area_weight=.3);bpy.ops.object.mode_set(mode='OBJECT')
    img=bpy.data.images.new(name+'_albedo',width=res,height=res,alpha=False)
    nodes=[]
    for m in obj.data.materials:
        node=m.node_tree.nodes.new('ShaderNodeTexImage');node.image=img;m.node_tree.nodes.active=node;node.select=True;nodes.append(node)
    scene=bpy.context.scene;scene.render.bake.use_selected_to_active=False;scene.render.bake.use_pass_direct=False;scene.render.bake.use_pass_indirect=False;scene.render.bake.use_pass_color=True;scene.render.bake.margin=6
    bpy.ops.object.bake(type='DIFFUSE')
    ao=bpy.data.images.new(name+'_ao',width=res,height=res,alpha=False)
    for node in nodes:node.image=ao
    bpy.ops.object.bake(type='AO')
    p=np.empty(res*res*4,dtype=np.float32);a=np.empty_like(p);img.pixels.foreach_get(p);ao.pixels.foreach_get(a);p=p.reshape((-1,4));a=a.reshape((-1,4));p[:,:3]*=(.68+.32*a[:,:3]);p[:,3]=1;img.pixels.foreach_set(p.ravel())
    assert np.std(p[:,:3])>.02
    img.filepath_raw=str(OUT/(name+'-albedo.png'));img.file_format='PNG';img.save();img.pack()
    # Edit-mode unwrap reallocates UV attributes: reacquire by name, never use
    # the stale pre-unwrap RNA layer pointer (Blender5.2 can crash on removal).
    obj.data.uv_layers.remove(obj.data.uv_layers['UVMap']);obj.data.uv_layers['Atlas'].name='UVMap'
    m=bpy.data.materials.new(name+'_Baked');m.use_nodes=True;bs=m.node_tree.nodes['Principled BSDF'];bs.inputs['Base Color'].default_value=(1,1,1,1);bs.inputs['Roughness'].default_value=.85
    tex=m.node_tree.nodes.new('ShaderNodeTexImage');tex.image=img;m.node_tree.links.new(tex.outputs['Color'],bs.inputs['Base Color'])
    obj.data.materials.clear();obj.data.materials.append(m)
    for face in obj.data.polygons:face.material_index=0
    obj.data.calc_loop_triangles(); return obj,len(obj.data.loop_triangles)

def preview(obj,name):
    scene=bpy.context.scene;scene.render.resolution_x=640;scene.render.resolution_y=640;scene.render.resolution_percentage=100;scene.view_settings.view_transform='AgX'
    scene.world=bpy.data.worlds.new('PreviewWorld');scene.world.use_nodes=True;scene.world.node_tree.nodes['Background'].inputs[0].default_value=(.24,.27,.31,1);scene.world.node_tree.nodes['Background'].inputs[1].default_value=.5
    points=np.array([v.co[:] for v in obj.data.vertices]);c=Vector((points.min(0)+points.max(0))/2);s=max(points.max(0)-points.min(0))
    bpy.ops.object.camera_add(location=c+Vector((1.6,-2.2,1.4))*s);cam=bpy.context.object;cam.data.type='ORTHO';cam.data.ortho_scale=s*1.32;cam.rotation_euler=(c-cam.location).to_track_quat('-Z','Y').to_euler();scene.camera=cam
    for offset,power in [((1,-1,2),180),((-1,.5,1),110)]:
        bpy.ops.object.light_add(type='AREA',location=c+Vector(offset)*s);o=bpy.context.object;o.data.energy=power*s*s;o.data.size=s*1.4;o.rotation_euler=(c-o.location).to_track_quat('-Z','Y').to_euler()
    scene.render.image_settings.file_format='PNG';scene.render.filepath=str(OUT/(name+'-preview.png'));bpy.ops.render.render(write_still=True)

BUILDERS=[('cot-detailed',cot,1024),('bucket',bucket,512),('chain-shackle',chain,512),('hanging-rag',rag,512),('drain-grate',drain,512),('candle-stool',candle_stool,512),('straw-scatter',straw_scatter,256)]
if '--' in sys.argv:
    selected=sys.argv[sys.argv.index('--')+1:]
    if selected:BUILDERS=[x for x in BUILDERS if x[0] in selected]
for name,builder,res in BUILDERS:
    began=time.perf_counter();bpy.ops.wm.read_factory_settings(use_empty=True);OBJECTS=[];MATERIALS={};R.seed(28092026+sum(map(ord,name)))
    scene=bpy.context.scene;scene.render.engine='CYCLES';scene.cycles.device='CPU';scene.cycles.samples=12;scene.render.threads_mode='FIXED';scene.render.threads=4
    anchors=builder();obj,tris=bake_join(name,res)
    bpy.ops.wm.save_as_mainfile(filepath=str(OUT/(name+'-editable.blend')))
    bpy.ops.export_scene.gltf(filepath=str(OUT/(name+'.glb')),export_format='GLB',use_selection=True,export_yup=True,export_apply=True)
    points=np.array([v.co[:] for v in obj.data.vertices]);engine=points[:,[0,2,1]]*np.array([1,1,-1])
    preview(obj,name)
    row=dict(name=name,triangles=tris,bytes=(OUT/(name+'.glb')).stat().st_size,bounds_min=engine.min(0).tolist(),bounds_max=engine.max(0).tolist(),anchors=anchors,texture=res,seconds=round(time.perf_counter()-began,2));REPORT.append(row);print('PROP_COMPLETE',json.dumps(row),flush=True)
previous=json.loads((OUT/'build-report.json').read_text()).get('props',[]) if (OUT/'build-report.json').exists() else []
combined={r['name']:r for r in previous};combined.update({r['name']:r for r in REPORT})
if 'cot-detailed' in combined:combined['cot-detailed']['anchors']['head_end']='positive engine Z'
(OUT/'build-report.json').write_text(json.dumps(dict(props=list(combined.values()),total_seconds=round(sum(r['seconds'] for r in combined.values()),2),last_run_seconds=round(time.perf_counter()-START,2)),indent=2),encoding='utf-8')
print('TOTAL_SECONDS',round(time.perf_counter()-START,2),flush=True)
