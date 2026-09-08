"""Run the existing asset-forge image/cutout/TRELLIS tools with recorded inputs."""
from pathlib import Path
import argparse
import ctypes
import hashlib
import json
import os
import subprocess
import time

PROMPT = ('Full body front view of one terrifying male medieval dungeon jailer, standing symmetrical A-pose, '
          'both arms held away from body, two separated legs and visible boots, empty hands, all fingers visible, '
          'entire figure centered on plain light gray background. Realistic detailed textured game character sculpt. '
          'Gaunt pale bald scarred face, deep black eye sockets, hooked nose, severe scowl, iron brow band. '
          'Weathered dark brown leather coat ending at upper thighs, ragged split hem, asymmetric blackened iron '
          'shoulder armor and bracer, worn straps, stitching, buckles, grey cloth trousers, battered tall leather boots. '
          'Sinister old prison executioner, thin cruel face, heavy hunched upper silhouette, anatomical human hands. '
          'Orthographic studio asset photography, neutral even light, readable surface grime and seams, sharp detailed face.')
NEGATIVE = ('cropped, closeup, portrait, multiple characters, duplicate limbs, fused arms, fused legs, crossed arms, '
            'hands on hips, weapon, sword, knife, staff, background scenery, ground shadow, text, watermark, '
            'cute, cartoon, mannequin, toy, wolf, animal head, fur, cape, floor length robe, smooth plastic, blurry face')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('stage',choices=['image','cutout','mesh'])
    parser.add_argument('--work',type=Path,required=True)
    parser.add_argument('--input',type=Path)
    parser.add_argument('--seed',type=int,default=709081)
    parser.add_argument('--cli',type=Path,default=Path('C:/Users/end/dev/cluster/provision/asset-forge/asset_cli.py'))
    args=parser.parse_args()
    if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
    args.work.mkdir(parents=True,exist_ok=True)
    stage=args.stage;dest=args.work/stage;dest.mkdir(exist_ok=True)
    if stage=='image':
        tool='image_generate';payload=dict(prompt=PROMPT,negative=NEGATIVE,width=1024,height=1536,steps=35,cfg=6.5,seed=args.seed,local_dir=str(dest),timeout_s=600)
    else:
        assert args.input and args.input.is_file(),'--input is required'
        if stage=='cutout':tool='image_cutout';payload=dict(image=str(args.input),local_dir=str(dest),margin=.06)
        else:tool='mesh_generate_pbr';payload=dict(image=str(args.input),seed=args.seed,texture_size=2048,local_dir=str(dest),timeout_s=1200)
    payload_path=args.work/f'{stage}-arguments.json'
    payload_path.write_text(json.dumps(payload,indent=2)+'\n',encoding='utf-8',newline='\n')
    started=time.perf_counter()
    command=[os.sys.executable,str(args.cli),tool,'@'+str(payload_path)]
    done=subprocess.run(command,text=True,encoding='utf-8',errors='replace',capture_output=True)
    result=dict(tool=tool,arguments=payload,stdout=done.stdout,stderr=done.stderr,exit_code=done.returncode,
                wall_seconds=round(time.perf_counter()-started,3),utc=time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
                input_sha256=hashlib.sha256(args.input.read_bytes()).hexdigest() if args.input else None)
    (args.work/f'{stage}-result.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps(result,indent=2),flush=True)
    raise SystemExit(done.returncode)


if __name__=='__main__':main()
