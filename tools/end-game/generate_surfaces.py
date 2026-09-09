"""Deterministic 1024px RGB base-color surfaces; run with Python + numpy + Pillow."""
from pathlib import Path
import json, time, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

START=time.perf_counter(); OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else Path(__file__).resolve().parent; N=1024; OUT.mkdir(parents=True, exist_ok=True)
X,Y=np.meshgrid(np.arange(N),np.arange(N))

def noise(seed,scale_x,scale_y=None):
    rng=np.random.default_rng(seed)
    a=rng.integers(0,256,(max(2,scale_y or scale_x),max(2,scale_x)),dtype=np.uint8)
    return np.asarray(Image.fromarray(a).resize((N,N),Image.Resampling.BICUBIC),dtype=np.float32)/255

def fbm(seed):
    return sum(noise(seed+i,2**(i+2))*w for i,w in enumerate([.32,.25,.18,.12,.08,.05]))

def shade(height,strength=1):
    dy,dx=np.gradient(height)
    return np.clip((dx*-.7-dy*.6)*strength,-.18,.18)

def save(name,a):
    p=OUT/(name+'.png'); Image.fromarray(np.uint8(np.clip(a,0,255)),'RGB').save(p,optimize=True)
    check=Image.open(p); assert check.mode=='RGB' and check.size==(N,N)
    print(name,p.stat().st_size,flush=True)

# Broad planks with nonlinear longitudinal grain, pores, knots and worn edges.
n=fbm(3100); long=noise(3170,192,9); fine=noise(3171,1024,90)
warp=(noise(3172,8,10)-.5)*28+(noise(3173,18,18)-.5)*8
grain=np.sin((X+warp)*.17)+.5*np.sin((X+warp)*.73)
height=.23*n+.09*grain+.09*long
groove=(np.mod(X+2*np.sin(Y*.003),256)<4)
edge=np.minimum(np.mod(X,256),255-np.mod(X,256))
base=np.array([113,72,39])[None,None,:]*(.59+n[:,:,None]*.82)
base+=(long-.5)[:,:,None]*30+grain[:,:,None]*6+(fine-.5)[:,:,None]*16
base+=shade(height,15)[:,:,None]*120
for cx,cy in [(121,228),(631,735),(887,460)]:
    r=np.sqrt(((X-cx)/1.4)**2+((Y-cy)/4.2)**2)
    knot=np.exp(-r*r/650); ring=(np.sin(r*.31)*.5+.5)*np.exp(-r*r/4200)
    base-=knot[:,:,None]*np.array([51,37,23])+ring[:,:,None]*np.array([11,8,5])
base[groove]*=.25
base+=(np.exp(-edge/3)*7)[:,:,None]
save('oak',base)

# Fine stone grain with broad mineral shifts, fissures, chips and dark wet areas.
n=fbm(4200); grit=noise(4261,1024); medium=noise(4262,144)
height=.6*n+.10*medium
base=np.array([88,91,89])[None,None,:]*(.55+n[:,:,None]*.85)
base+=(grit-.5)[:,:,None]*25+shade(height,10)[:,:,None]*95
cracks=Image.new('L',(N,N)); d=ImageDraw.Draw(cracks); rng=np.random.default_rng(4281)
for i in range(33):
    x,y=rng.uniform(0,N,2); angle=rng.uniform(0,6.28); pts=[(x,y)]
    for j in range(int(rng.integers(5,14))):
        angle+=rng.uniform(-.65,.65); x+=np.cos(angle)*rng.uniform(10,26); y+=np.sin(angle)*rng.uniform(10,26); pts.append((x,y))
    d.line(pts,fill=int(rng.integers(100,200)),width=int(rng.integers(1,3)))
cr=np.asarray(cracks,dtype=float)/255
crsoft=np.asarray(cracks.filter(ImageFilter.GaussianBlur(2.5)),dtype=float)/255
wet=np.clip((noise(4290,9)-.58)*3.5,0,.58)
base-=cr[:,:,None]*38+crsoft[:,:,None]*18
base*=1-wet[:,:,None]*.45
chips=(medium>.79)&(grit>.63); base[chips]+=12
base+=(np.clip(noise(4291,13)-.65,0,1)*34)[:,:,None]*np.array([.45,.36,.18])
save('stone',base)

# Forged iron: mottled oxide, porous dark pits, scraped bright high areas.
n=fbm(5300); grit=noise(5311,1024); pitting=noise(5312,220)
rust=np.clip((noise(5320,18)*.65+noise(5321,74)*.35-.41)*3,0,1)
steel=np.array([57,62,64])[None,None,:]*(.7+n[:,:,None]*.55)
oxide=np.array([122,58,25])[None,None,:]*(.6+n[:,:,None]*.65)
base=steel*(1-rust[:,:,None])+oxide*rust[:,:,None]
height=(pitting-.5)*.16+n*.4
base+=shade(height,8)[:,:,None]*60+(grit-.5)[:,:,None]*24
pits=np.clip((.24-pitting)*5,0,.75)
base*=1-pits[:,:,None]*.7
scrape=np.clip((noise(5340,5,700)-.76)*5,0,.65)*(1-rust*.8)
base+=scrape[:,:,None]*np.array([47,49,47])
save('iron',base)
(OUT/'surfaces.json').write_text(json.dumps(dict(generator='generate_surfaces.py',size=[N,N],mode='RGB',bit_depth=8,seeds=dict(oak=3100,stone=4200,iron=5300),wall_seconds=round(time.perf_counter()-START,2)),indent=2),encoding='utf-8')
print('Wall time',round(time.perf_counter()-START,2),'s')
