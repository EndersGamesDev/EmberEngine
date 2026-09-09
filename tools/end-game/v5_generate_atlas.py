"""Deterministic RGB8 materials for the articulated prison warden; no service calls."""
from pathlib import Path
import os,time
import numpy as np
from PIL import Image
started=time.perf_counter()
out=Path(os.environ['END_GAME_V5_ASSET_WORK']);out.mkdir(parents=True,exist_ok=True)
rng=np.random.default_rng(5090926)
colors=[(84,66,47),(66,42,28),(133,104,85),(82,61,50),(61,63,62),(186,193,187),(64,45,28),(122,102,75),(23,22,21),(157,149,124),(102,64,41),(105,88,72),(133,78,65),(102,93,73),(37,31,24),(103,87,48)]
atlas=np.empty((128,128,3),np.uint8)
for index,color in enumerate(colors):
    y,x=np.mgrid[:32,:32];noise=rng.normal(0,3.4,(32,32))
    grain=2*np.sin(y*.7+x*.14)+1.5*np.cos(x*1.5)
    if index in [0,1,6,13,14]:grain+=2*np.sin(x*.9)
    tile=np.clip(np.array(color)+noise[:,:,None]+grain[:,:,None],0,255).astype(np.uint8)
    if index in [4,5,10,15]:
        for _ in range(7):
            px,py=rng.integers(2,29,2);tile[py:py+1,max(0,px-2):min(32,px+4)]=np.clip(np.array(color)+22,0,255)
    row,col=divmod(index,4);atlas[row*32:(row+1)*32,col*32:(col+1)*32]=tile
Image.fromarray(atlas,'RGB').save(out/'warden-atlas.png',optimize=True)
print(f'ATLAS_WALL_SECONDS {time.perf_counter()-started:.3f}')
