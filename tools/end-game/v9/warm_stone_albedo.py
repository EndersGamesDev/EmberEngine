"""Apply the kit's aged warm limestone palette to actual RGB8 atlas bytes."""
from pathlib import Path
import sys
import numpy as np
from PIL import Image

path=Path(sys.argv[1]);image=Image.open(path).convert('RGB')
rgb=np.asarray(image,dtype=np.float32)
# Preserve sculpt detail and baked occlusion while moving cold gray toward
# weathered buff limestone; no runtime tint or extra texture is required.
rgb=np.clip(rgb*np.array([.925,.870,.775],dtype=np.float32),0,255).astype(np.uint8)
Image.fromarray(rgb,'RGB').save(path,optimize=True)
