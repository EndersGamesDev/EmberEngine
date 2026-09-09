"""Bounded image/cutout/TRELLIS stages for three V9 environment props."""
from pathlib import Path
import argparse, ctypes, hashlib, json, os, subprocess, time, urllib.request

ROOT = Path(os.environ.get('END_GAME_V9_WORK',str(Path(__file__).resolve().parent)))
if 'END_GAME_V9_WORK' not in os.environ and Path(__file__).resolve().parent.name!='runtime-v9-assets':
    raise RuntimeError('Set END_GAME_V9_WORK to an asset workspace outside the engine checkout before running installed reproduction tools')
CLI = Path('C:/Users/end/dev/cluster/provision/asset-forge/asset_cli.py')
STYLE = 'Realistic detailed medieval Gothic castle game asset, weathered cool gray limestone, warm worn edges, subtle damp moss in crevices, deep carved stone relief, aged dark iron accents. Single isolated object, centered entirely visible on plain light gray background, neutral diffuse studio light, sharp surface texture, three quarter front view, no ground shadow. '
NEGATIVE = 'people, character, creature, statue of person, castle building, landscape, scenery, ground plane, background architecture, text, watermark, multiple objects, cropped, bright gold, cartoon, toy, smooth plastic, blurry, floating fragments, water jet, hanging chains'
PROPS = {
    'gothic-pillar': dict(seed=909081, width=1024,height=1536,height_m=4.4,triangles=9500,
        description='One tall freestanding square Gothic stone pier with a broad carved acanthus capital and a projecting corbel for supporting a vaulted arch, layered octagonal foot, clustered slender stone shafts, heavy solid load-bearing silhouette, vertically stacked fitted stone blocks. Simple connected architecture, narrow column with top capital, no complete arch, no wall, no roof.'),
    'courtyard-fountain':dict(seed=909082,width=1344,height=1024,height_m=1.15,triangles=12000,
        description='One abandoned compact octagonal courtyard fountain, wide shallow stone basin with thick rim on a low stepped base and a short central carved pedestal carrying a small chipped scalloped bowl. Completely dry empty basin, dark old water stains and subtle moss, aged Renaissance Gothic stone carving. One connected sculptural fountain, basin interior clearly visible from slightly elevated viewpoint, no loose rubble, no statues, no flowing water.'),
    'tower-doorway':dict(seed=909083,width=1024,height=1536,height_m=6.0,triangles=11500,
        description='One freestanding thick pointed Gothic doorway surround, symmetrical broad carved stone jambs joined by a pointed arch, recessed layered arch molding, chipped stone heraldic shield at the apex without letters, wide empty open passage through the center, visible light gray background through the opening. A stone portal frame only, no door leaf, no backing wall, no floor, no gates, stout solid continuous jambs and arch.'),
}

def read_json(url):
    with urllib.request.urlopen(url, timeout=15) as r:return json.load(r)

def main():
    ap=argparse.ArgumentParser();ap.add_argument('prop',choices=PROPS);ap.add_argument('stage',choices=['image','cutout','mesh']);args=ap.parse_args()
    if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
    work=ROOT/args.prop;stage=args.stage;out=work/stage;out.mkdir(parents=True,exist_ok=True)
    spec=PROPS[args.prop];ext='*.glb' if stage=='mesh' else '*.png'
    if list(out.glob(ext)):raise SystemExit('Output already exists; refusing duplicate generation')
    if stage=='image':
        health=read_json('http://192.168.178.187:8188/queue');assert not health['queue_running'] and not health['queue_pending'],health
        payload=dict(prompt=STYLE+spec['description'],negative=NEGATIVE,width=spec['width'],height=spec['height'],steps=35,cfg=6.5,seed=spec['seed'],local_dir=str(out),timeout_s=600);tool='image_generate'
    elif stage=='cutout':
        source=next((work/'image').glob('*.png'));payload=dict(image=str(source),local_dir=str(out),margin=.04);tool='image_cutout'
    else:
        health=read_json('http://192.168.178.171:8190/health');assert health['model_loaded'] and health['current_job'] is None and health['queued']==0,health
        source=next((work/'cutout').glob('*.png'));payload=dict(image=str(source),seed=spec['seed'],texture_size=2048,local_dir=str(out),timeout_s=1200);tool='mesh_generate_pbr'
    path=work/f'{stage}-arguments.json';path.write_text(json.dumps(payload,indent=2)+'\n',encoding='utf-8',newline='\n')
    began=time.perf_counter();run=subprocess.run([os.sys.executable,str(CLI),tool,'@'+str(path)],text=True,encoding='utf-8',errors='replace',capture_output=True)
    record=dict(tool=tool,arguments=payload,stdout=run.stdout,stderr=run.stderr,exit_code=run.returncode,wall_seconds=round(time.perf_counter()-began,3),utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),files=[dict(path=str(p),bytes=p.stat().st_size,sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in out.glob(ext)])
    (work/f'{stage}-result.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print(json.dumps(record,indent=2),flush=True)
    raise SystemExit(run.returncode)

if __name__=='__main__':main()
