"""Measure exported model topology and the decorative portal's sight-line aperture."""
import bpy, bmesh, ctypes, json, os, sys, time
from pathlib import Path
from mathutils import Matrix, Vector

ROOT=Path(os.environ.get('END_GAME_V9_WORK',str(Path(__file__).resolve().parent)));name=sys.argv[sys.argv.index('--')+1];work=ROOT/name/'converted'
started=time.perf_counter()
if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
bpy.ops.wm.read_factory_settings(use_empty=True);bpy.ops.import_scene.gltf(filepath=str(work/(name+'.glb')))
obj=next(o for o in bpy.context.scene.objects if o.type=='MESH');obj.data.transform(obj.matrix_world);obj.parent=None;obj.matrix_world=Matrix.Identity(4);obj.data.update()
bm=bmesh.new();bm.from_mesh(obj.data);bmesh.ops.remove_doubles(bm,verts=list(bm.verts),dist=.00001)
boundaries=sum(e.is_boundary for e in bm.edges);nonmanifold=sum(not e.is_manifold for e in bm.edges);bm.free()
report=dict(prop=name,boundary_edges_welded=boundaries,nonmanifold_edges_welded=nonmanifold,collision_verified=False)
if name=='tower-doorway':
    rows=[]
    for z in [.10,.20,.35,.50,.75,1.0,1.25,1.5,1.75,2.0,2.15,2.3,2.5,2.75,3.0]:
        clear=[]
        for i in range(-35,36):
            x=i*.04
            hit,_,_,_=obj.ray_cast(Vector((x,3.5,z)),Vector((0,-1,0)),distance=7)
            if not hit:clear.append(round(x,3))
        center=[]
        if 0.0 in clear:
            center=[0.0]
            for direction in [-1,1]:
                for i in range(1,36):
                    x=round(direction*i*.04,3)
                    if x not in clear:break
                    center.append(x)
        rows.append(dict(height_m=z,center_clear=bool(center),clear_span_x=[min(center),max(center)] if center else None,width_sampled_m=round(max(center)-min(center),3) if center else 0))
    report['portal_rays']=rows
    placement=[]
    for world_height in [.05,.15,.4,.75,1.0,1.4,1.7,1.85,2.0]:
        z=(world_height+.40)/1.25;hits=[]
        for x in [-.36,-.24,-.12,0,.12,.24,.36]:
            hit,_,_,_=obj.ray_cast(Vector((x/1.25,3.5,z)),Vector((0,-1,0)),distance=7)
            if hit:hits.append(x)
        placement.append(dict(height_above_floor=world_height,blocked_x=hits))
    report['recommended_instance']=dict(uniform_scale=1.25,base_y_below_floor=.40,test_width=.72,ray_rows=placement)
    assert not any(row['blocked_x'] for row in placement),'Recommended portal aperture obstructs the sampled body corridor'
    body_blocked=[];body_ray_count=0
    for h in range(1,35):
        height=h*.05;z=(height+.40)/1.25
        for i in range(-9,10):
            x=i*.05;body_ray_count+=1
            hit,_,_,_=obj.ray_cast(Vector((x/1.25,3.5,z)),Vector((0,-1,0)),distance=7)
            if hit:body_blocked.append([round(x,3),round(height,3)])
    report['placed_body_clearance']=dict(width=.9,min_height=.05,max_height=1.7,sample_spacing=.05,rays=body_ray_count,blocked=body_blocked)
    assert not body_blocked,'0.9 m wide standing body corridor blocked'
    report['ray_note']='Visual mesh rays only, 4 cm raw sampling and 0.72 m placed body corridor; root must author and test collision separately.'
report['wall_seconds']=round(time.perf_counter()-started,3)
(work/'measurements.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8',newline='\n');print('V9_MEASURE '+json.dumps(report),flush=True)
