import io, json, struct, time, os, sys
from pathlib import Path
from PIL import Image

start=time.perf_counter()
root=Path(sys.argv[1]).resolve()
out=Path(os.environ['END_GAME_ASSET_WORK']).resolve(); out.mkdir(parents=True, exist_ok=True)
specs=[('hero-wolf-form','character',12000,768,2.1),('hero-werewolf-form','character',14000,768,2.45),('greatsword','character',6000,768,2.4),('sleeping_warden','level1/meshes',8000,512,1.45),('wooden_cot','level1/meshes',5000,512,2.1),('iron_bars_segment','level1/meshes',5000,512,2.7),('wall_torch','level1/meshes',4000,512,0.9)]
config=[]
for name,directory,budget,res,size in specs:
    src=root/directory/(name+'.glb'); b=src.read_bytes(); n=struct.unpack_from('<I',b,12)[0]
    doc=json.loads(b[20:20+n]); raw=b[28+n:]
    texture=doc['materials'][0]['pbrMetallicRoughness']['baseColorTexture']['index']
    texture_doc=doc['textures'][texture]
    image_index=texture_doc.get('source',texture_doc.get('extensions',{}).get('EXT_texture_webp',{}).get('source'))
    image=doc['images'][image_index]
    v=doc['bufferViews'][image['bufferView']]; offset=v.get('byteOffset',0)
    im=Image.open(io.BytesIO(raw[offset:offset+v['byteLength']])).convert('RGB')
    im.thumbnail((res,res),Image.Resampling.LANCZOS)
    tex=out/(name+'-albedo.png'); im.save(tex,optimize=True)
    config.append(dict(name=name,source=str(src),texture=str(tex),budget=budget,size=size))
(out/'asset-specs.json').write_text(json.dumps(config,indent=2),encoding='utf-8')
print(f'Texture preparation: {time.perf_counter()-start:.2f}s')
