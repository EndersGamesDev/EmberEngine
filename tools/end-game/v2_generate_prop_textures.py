import os
"""Deterministic material sources used by the V2 geometry generator."""
from pathlib import Path
import numpy as np
from PIL import Image,ImageDraw
import json,time
OUT=Path(os.environ['END_GAME_V2_ASSET_WORK'])/'props'/'material-sources'; OUT.mkdir(parents=True,exist_ok=True)
N=1024; X,Y=np.meshgrid(np.arange(N),np.arange(N)); START=time.perf_counter()
def noise(seed,nx,ny=None):
    a=np.random.default_rng(seed).integers(0,256,(ny or nx,nx),dtype=np.uint8)
    return np.asarray(Image.fromarray(a).resize((N,N),Image.Resampling.BICUBIC),dtype=float)/255
def save(name,a): Image.fromarray(np.uint8(np.clip(a,0,255)),'RGB').save(OUT/(name+'.png'),optimize=True)
base=noise(821,11)*.4+noise(822,48)*.35+noise(823,180)*.25
warp=(noise(824,12,20)-.5)*24
grain=np.sin((X+warp)*.22)+.4*np.sin((X+warp)*.94)
a=np.array([104,72,40])[None,None,:]*(.55+base[:,:,None]*.7)+grain[:,:,None]*6+(noise(825,400,14)-.5)[:,:,None]*25
for cx,cy in [(193,236),(782,612),(420,844)]:
    r=np.sqrt(((X-cx)/1.2)**2+((Y-cy)/3.2)**2); a-=np.exp(-r*r/650)[:,:,None]*np.array([37,30,18])+(np.sin(r*.23)*np.exp(-r*r/5000))[:,:,None]*7
save('oak',a)
generated_oak=Path(os.environ['END_GAME_V2_ASSET_WORK'])/'surfaces'/'oak-aged-albedo-v2.png'
if generated_oak.exists():
    Image.open(generated_oak).convert('RGB').resize((1024,1024),Image.Resampling.LANCZOS).save(OUT/'oak.png',optimize=True)
n=noise(842,13)*.5+noise(843,64)*.3+noise(844,300)*.2
rust=np.clip((n-.43)*3,0,1); iron=np.array([53,57,58])[None,None,:]*(.7+n[:,:,None]*.45)
a=iron*(1-rust[:,:,None])+np.array([112,62,28])[None,None,:]*(.6+n[:,:,None]*.6)*rust[:,:,None]
pits=np.clip((.24-noise(845,256))*6,0,.7); a*=1-pits[:,:,None]; a+=(noise(846,1024)-.5)[:,:,None]*15
save('iron',a)
cloth=(np.sin(X*np.pi/2)*np.sin(Y*np.pi/2))*4+(noise(858,256)-.5)*13
dirt=np.clip((noise(851,8)*.7+noise(852,42)*.3-.38)*1.8,0,.65)
a=np.array([151,134,102])[None,None,:]*(.95-dirt[:,:,None]) + cloth[:,:,None]
save('linen',a)
a=np.array([77,66,48])[None,None,:]*(.75+noise(855,18)[:,:,None]*.5)+cloth[:,:,None]
save('rag',a)
a=np.array([169,133,66])[None,None,:]*(.55+noise(861,32,5)[:,:,None]*.65)+(noise(862,700,10)-.5)[:,:,None]*24
save('straw',a)
a=np.array([180,157,106])[None,None,:]*(.85+noise(871,25)[:,:,None]*.15)+(noise(872,256)-.5)[:,:,None]*5
save('wax',a)
save('dark',np.zeros((N,N,3))+np.array([24,19,13]))
(OUT/'README.md').write_text('# Material sources\n\nLocally generated seeded base-color material maps. Reproduce with ../generate_prop_textures.py from the runtime-v2-assets root. The root worker may substitute its generated oak image before running build_props.py; the bake records the source paths. No external service was called by this generator.\n',encoding='utf-8')
print('Material source generation seconds',round(time.perf_counter()-START,2))
