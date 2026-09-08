"""Record only this kit's matching ComfyUI jobs and local source/converted evidence."""
from pathlib import Path
import json,os,re,time,urllib.request

ROOT=Path(os.environ.get('END_GAME_V9_WORK',str(Path(__file__).resolve().parent)))
with urllib.request.urlopen('http://192.168.178.187:8188/history',timeout=15) as response:history=json.load(response)
kit=[]
for name in ['gothic-pillar','courtyard-fountain','tower-doorway']:
    work=ROOT/name;stages={stage:json.loads((work/(stage+'-result.json')).read_text()) for stage in ['image','cutout','mesh']}
    filename=Path(stages['image']['files'][0]['path']).name
    prompt_ids=[]
    for key,entry in history.items():
        outputs=entry.get('outputs',{})
        if any(im.get('filename')==filename for output in outputs.values() for im in output.get('images',[])):prompt_ids.append(key)
    mesh_job=re.search(r'job (\S+) completed',stages['mesh']['stdout']).group(1)
    manifest=json.loads((work/'converted/manifest.json').read_text())
    provenance=dict(method='SDXL concept -> rembg cutout -> TRELLIS.2 textured GLB -> local Blender decimation and RGB8 atlas reduction',image_model='SDXL 1.0 base on Specht',image_prompt_ids=prompt_ids,mesh_model='TRELLIS.2 4B, 1024_cascade, Adler RTX 4090',mesh_job=mesh_job,seed=stages['image']['arguments']['seed'],concept=stages['image']['files'][0],cutout=stages['cutout']['files'][0],raw_mesh=stages['mesh']['files'][0],wall_seconds={stage:stages[stage]['wall_seconds'] for stage in stages},source_inputs={stage:str(work/(stage+'-arguments.json')) for stage in stages})
    manifest['provenance']=provenance
    manifest['verified']='Exported RGB8 atlas, one primitive, finite positions/normals/UVs, index bounds, nondegenerate triangles, budgets, passive Blender front/back previews.'
    manifest['not_verified']='Ember runtime rendering, gameplay collision, browser/device performance; integration awaits root authorization.'
    (work/'converted/manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n');kit.append(manifest)
(ROOT/'kit-manifest.json').write_text(json.dumps(dict(assets=kit,barza_start=dict(seq=288,id='msg-288-136d1ef4'),barza_reservation=dict(seq=289,id='msg-289-b21cbc94'),barza_release=dict(seq=291,id='msg-291-d0138991'),fleet_reservations_active=False,created_utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime())),indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps([dict(prop=m['prop'],image_prompt_ids=m['provenance']['image_prompt_ids'],mesh_job=m['provenance']['mesh_job']) for m in kit],indent=2))
