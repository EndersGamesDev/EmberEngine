"""Small shared RGB8 armor atlas; four material regions, no external services."""
from pathlib import Path
import os
import numpy as np
from PIL import Image,ImageDraw
OUT=Path(os.environ['END_GAME_V3_ASSET_WORK']);N=128;R=np.random.default_rng(31092026)
a=np.zeros((N,N,3),dtype=np.uint8)
for tx,ty,color in [(0,0,[43,47,49]),(1,0,[34,27,22]),(0,1,[112,115,112]),(1,1,[104,78,40])]:
    noise=R.normal(0,3.3,(64,64,1));q=np.clip(np.array(color)[None,None,:]+noise,0,255).astype('uint8')
    if ty==0 and tx==0:
        for _ in range(22):
            x,y=R.integers(0,64,2);q[y:min(y+int(R.integers(2,8)),64),x]=[74,77,76]
    a[(1-ty)*64:(2-ty)*64,tx*64:(tx+1)*64]=q
Image.fromarray(a,'RGB').save(OUT/'wolf-armor-atlas.png',optimize=True)
print('Wrote128x128 RGB atlas', (OUT/'wolf-armor-atlas.png').stat().st_size)
