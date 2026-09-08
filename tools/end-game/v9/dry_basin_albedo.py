"""Replace generated pool-like floor texels with matte dry stone; preserve the bowl."""
from pathlib import Path
import json, sys
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

path=Path(sys.argv[1]);faces=json.loads(Path(sys.argv[2]).read_text())
image=Image.open(path).convert('RGB');size=image.width
mask=Image.new('L',image.size,0);draw=ImageDraw.Draw(mask)
for face in faces:draw.polygon([(round(u*(size-1)),round((1-v)*(size-1))) for u,v in face],fill=255)
mask=mask.filter(ImageFilter.GaussianBlur(.30))
rng=np.random.default_rng(909082)
coarse=Image.fromarray(rng.integers(70,190,(32,32),dtype=np.uint8)).resize(image.size,Image.Resampling.BICUBIC)
field=np.asarray(coarse,dtype=float)-128
fine=rng.normal(0,5,(size,size));gray=np.clip(112+field*.25+fine,55,155)
color=np.stack([gray*1.03,gray,gray*.92],axis=-1).astype(np.uint8)
dry=Image.fromarray(color,'RGB')
# The generated atlas has reused green texels on reversed interior faces.
# Normalize all of those to the same dry stone before the geometric floor mask,
# avoiding green triangular leftovers without changing exterior gray carving.
original=np.asarray(image,dtype=float)
green=(original[:,:,1]>original[:,:,0]*1.025)&(original[:,:,1]>original[:,:,2]*1.025)
green_mask=Image.fromarray((green*255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(.3))
image=Image.composite(dry,image,green_mask)
Image.composite(dry,image,mask).save(path,optimize=True)
print(json.dumps(dict(dry_floor_faces=len(faces),painted_texels=int((np.asarray(mask)>0).sum()),method='UV-local matte limestone replacing generated pool-like center')))
