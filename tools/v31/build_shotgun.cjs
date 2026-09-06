// Original CPU-authored Breach-12 GLB; no downloaded models, Blender, or GPU.
// Node generates/validates only the two new shotgun assets and target report.
// Run at repository root: node tools/v31/build_shotgun.cjs [--check]
'use strict';
const fs = require('node:fs'), path = require('node:path'), zlib = require('node:zlib');
const assert = require('node:assert/strict'), crypto = require('node:crypto'), os = require('node:os');
os.setPriority(0, os.constants.priority.PRIORITY_LOW);
const started = Date.now(), root = process.cwd(), check = process.argv.includes('--check');
assert(process.argv.slice(2).every(arg => arg === '--check'), 'Only --check is supported');
const assetDir = path.join(root, 'crates/arena/assets');
const out = path.join(assetDir, 'shotgun.glb'), sidecar = path.join(assetDir, 'shotgun-rig.json');
const digest = value => crypto.createHash('sha256').update(value).digest('hex');
const preserved = Object.fromEntries(['viewmodel.glb','viewmodel-rig.json','weapon-grips.glb','weapon-grips.json']
  .map(file => [file, digest(fs.readFileSync(path.join(assetDir, file)))]));
const names = ['w_shotgun_body', 'w_shotgun_magazine'];
const palette = [[59,64,67],[94,100,104],[32,36,38],[113,107,87],[9,12,14],[175,132,62],[128,37,24],[196,165,88]];
const size = 128, tile = 32;
function crc(bytes) {
  let value = 0xffffffff;
  for (const byte of bytes) {
    value ^= byte;
    for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0);
  }
  return (value ^ 0xffffffff) >>> 0;
}
function chunk(type, bytes) {
  const tag = Buffer.from(type), result = Buffer.alloc(bytes.length + 12);
  result.writeUInt32BE(bytes.length, 0); tag.copy(result, 4); bytes.copy(result, 8);
  result.writeUInt32BE(crc(Buffer.concat([tag, bytes])), bytes.length + 8); return result;
}
function texture() {
  const raw = Buffer.alloc(size * (size * 3 + 1));
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const color = palette[(Math.floor(y / tile) * 4 + Math.floor(x / tile)) % palette.length];
    const grain = ((x * 73 + y * 151 + x * y * 7) % 7) - 3;
    for (let c = 0; c < 3; c++) raw[y * (size * 3 + 1) + 1 + x * 3 + c] = Math.max(0, Math.min(255, color[c] + grain));
  }
  const header = Buffer.alloc(13); header.writeUInt32BE(size, 0); header.writeUInt32BE(size, 4);
  header[8] = 8; header[9] = 2; // RGB8, no alpha or unsupported high-bit depth.
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]), chunk('IHDR', header), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const subtract = (a,b) => a.map((v,i) => v-b[i]);
const cross = (a,b) => [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
const meshes = names.map(name => ({ name, positions: [], normals: [], uv: [] }));
let mesh = meshes[0];
function triangle(a,b,c,surface) {
  const n = cross(subtract(b,a),subtract(c,a)), length = Math.hypot(...n);
  assert(length > 1e-12, 'Degenerate triangle');
  const u = surface % 4, v = Math.floor(surface / 4);
  for (const [i,p] of [a,b,c].entries()) {
    mesh.positions.push(...p); mesh.normals.push(...n.map(x => x/length));
    const sample = [[0.12,0.12],[0.88,0.12],[0.88,0.88]][i];
    mesh.uv.push((u+sample[0])/4, (v+sample[1])/4);
  }
}
function quad(a,b,c,d,surface) { triangle(a,b,c,surface); triangle(a,c,d,surface); }
function extrude(points, z0, z1, surface) {
  const area = points.reduce((sum,p,i) => { const q=points[(i+1)%points.length]; return sum+p[0]*q[1]-q[0]*p[1]; },0);
  const p = area < 0 ? [...points].reverse() : points;
  const at = (i,z) => [p[i][0],p[i][1],z];
  for (let i=1;i<p.length-1;i++) {
    triangle(at(0,z1),at(i,z1),at(i+1,z1),surface);
    triangle(at(0,z0),at(i+1,z0),at(i,z0),surface);
  }
  for (let i=0;i<p.length;i++) { const j=(i+1)%p.length; quad(at(i,z0),at(j,z0),at(j,z1),at(i,z1),surface); }
}
function box(x0,x1,y0,y1,z0,z1,surface,bevel=0) {
  const b=Math.min(bevel,(x1-x0)*0.25,(y1-y0)*0.25);
  const p=b ? [[x0+b,y0],[x1-b,y0],[x1,y0+b],[x1,y1-b],[x1-b,y1],[x0+b,y1],[x0,y1-b],[x0,y0+b]]
    : [[x0,y0],[x1,y0],[x1,y1],[x0,y1]];
  extrude(p,z0,z1,surface);
}
function tube(x0,x1,y,z,r,surface,inner=0,segments=24) {
  const at=(x,rad,i)=>[x,y+rad*Math.cos(i*Math.PI*2/segments),z+rad*Math.sin(i*Math.PI*2/segments)];
  for(let i=0;i<segments;i++) {
    quad(at(x0,r,i),at(x0,r,i+1),at(x1,r,i+1),at(x1,r,i),surface);
    if(inner) {
      quad(at(x0,inner,i+1),at(x0,inner,i),at(x1,inner,i),at(x1,inner,i+1),4);
      quad(at(x1,r,i),at(x1,r,i+1),at(x1,inner,i+1),at(x1,inner,i),surface);
      quad(at(x0,r,i+1),at(x0,r,i),at(x0,inner,i),at(x0,inner,i+1),surface);
    } else {
      triangle([x1,y,z],at(x1,r,i),at(x1,r,i+1),surface);
      triangle([x0,y,z],at(x0,r,i+1),at(x0,r,i),surface);
    }
  }
}

// Receiver with bevel silhouette, short side rails and an inset ejection port.
box(-0.135,0.245,0.022,0.136,-0.040,0.040,0,0.015);
box(-0.115,0.245,0.125,0.144,-0.026,0.026,1,0.005);
box(0.025,0.180,0.073,0.116,0.040,0.042,4,0.006);
box(0.045,0.145,0.076,0.084,0.042,0.045,1);
box(0.08,0.11,0.105,0.118,0.042,0.085,1,0.006); // charging handle
for(const x of [-0.10,0.00,0.205]) for(const z of [-0.042,0.042]) box(x,x+0.011,0.049,0.060,z-0.002,z+0.002,1,0.003);
// The right glove wraps the existing AK grip target [-.053,-.032,0].
extrude([[-0.105,0.027],[-0.035,0.031],[-0.015,-0.025],[-0.043,-0.139],[-0.105,-0.135],[-0.119,-0.10]],-0.024,0.024,2);
for(let i=0;i<5;i++) box(-0.10,-0.042,-0.125+i*0.021,-0.119+i*0.021,-0.025,0.025,0,0.002);
// Open trigger guard; three bars leave a visible finger-sized interior.
box(-0.035,0.102,-0.076,-0.065,-0.009,0.009,1,0.004);
box(0.092,0.103,-0.065,0.025,-0.009,0.009,1,0.004);
box(-0.031,-0.020,-0.065,0.015,-0.009,0.009,1,0.004);
extrude([[0.023,0.025],[0.034,0.02],[0.025,-0.032],[0.010,-0.042],[0.009,-0.030]],-0.004,0.004,1);
// Tactical shoulder stock, cheek rest and textured recoil pad.
extrude([[-0.425,-0.013],[-0.408,0.129],[-0.115,0.118],[-0.115,0.074],[-0.34,0.054],[-0.354,-0.018]],-0.026,0.026,2);
box(-0.445,-0.417,-0.045,0.156,-0.034,0.034,0,0.009);
box(-0.405,-0.190,0.117,0.140,-0.029,0.029,3,0.009);
for(let i=0;i<6;i++) box(-0.446,-0.444,-0.03+i*0.03,-0.024+i*0.03,-0.029,0.029,4);
// Chunky ventilated handguard is fitted around AK's +.35 support socket.
box(0.245,0.490,0.026,0.110,-0.038,0.038,2,0.015);
for(let i=0;i<7;i++) {
  const x=0.258+i*0.031;
  box(x,x+0.011,0.022,0.116,-0.041,0.041,0,0.005);
  for(const z of [-0.042,0.042]) box(x+0.013,x+0.026,0.064,0.088,z-0.001,z+0.001,4,0.003);
}
// Long large-bore barrel, support tube and a visibly hollow crown.
tube(0.215,0.860,0.105,0,0.022,1,0.010,28);
tube(0.265,0.765,0.063,0,0.015,0,0,20);
tube(0.500,0.518,0.105,0,0.028,0,0.010,28);
tube(0.782,0.807,0.105,0,0.030,0,0.010,28);
tube(0.824,0.860,0.105,0,0.032,0,0.010,28);
// Open rear notch; the front bead stops 2 mm below the .165 optical ray.
for(let i=0;i<12;i++) box(-0.11+i*0.03,-0.095+i*0.03,0.144,0.151,-0.029,0.029,0,0.003);
for(const z of [-0.021,0.010]) box(-0.075,-0.044,0.151,0.178,z,z+0.011,1,0.003);
box(0.728,0.747,0.119,0.158,-0.012,0.012,0,0.005);
box(0.734,0.742,0.158,0.163,-0.0035,0.0035,7,0.002);
// Magazine is the ONLY detachable part; all vertices retain weapon coordinates.
mesh=meshes[1];
extrude([[0.119,0.028],[0.218,0.028],[0.224,-0.191],[0.207,-0.251],[0.124,-0.244],[0.107,-0.206]],-0.042,0.042,3);
box(0.110,0.221,-0.255,-0.237,-0.046,0.046,2,0.005);
for(const z of [-0.044,0.042]) for(let i=0;i<3;i++) box(0.130+i*0.027,0.139+i*0.027,-0.217,-0.024,z,z+0.002,0,0.003);
box(0.121,0.215,-0.185,-0.158,-0.045,-0.043,6);

const document={ asset:{version:'2.0',generator:'Ember original CPU-authored Breach-12 v31'},scene:0,scenes:[{nodes:[0,1]}],
  nodes:names.map((name,i)=>({name,mesh:i})), meshes:[], accessors:[], bufferViews:[],
  buffers:[{byteLength:0}], samplers:[{magFilter:9729,minFilter:9987,wrapS:33071,wrapT:33071}],
  images:[],textures:[{source:0,sampler:0}],materials:[{name:'Breach-12 RGB8 material atlas',pbrMetallicRoughness:{baseColorFactor:[1,1,1,1],baseColorTexture:{index:0},metallicFactor:0,roughnessFactor:0.72}}] };
const chunks=[]; let offset=0;
function bufferView(bytes,target) {
  const padding=(4-offset%4)%4;if(padding){chunks.push(Buffer.alloc(padding));offset+=padding;}
  const index=document.bufferViews.length; document.bufferViews.push({buffer:0,byteOffset:offset,byteLength:bytes.length,...(target?{target}:{})});
  chunks.push(bytes);offset+=bytes.length;return index;
}
function accessor(values,components,type,bounds=false) {
  const bytes=Buffer.alloc(values.length*4);values.forEach((value,i)=>bytes.writeFloatLE(value,i*4));
  const result={bufferView:bufferView(bytes,34962),componentType:5126,count:values.length/components,type};
  if(bounds)for(const [key,fn] of [['min',Math.min],['max',Math.max]])result[key]=Array.from({length:components},(_,c)=>values.filter((_,i)=>i%components===c).reduce((a,b)=>fn(a,b)));
  document.accessors.push(result);return document.accessors.length-1;
}
for(const m of meshes) document.meshes.push({name:m.name,primitives:[{attributes:{POSITION:accessor(m.positions,3,'VEC3',true),NORMAL:accessor(m.normals,3,'VEC3'),TEXCOORD_0:accessor(m.uv,2,'VEC2')},material:0,mode:4}]});
document.images.push({name:'Breach-12 original material swatches',bufferView:bufferView(texture()),mimeType:'image/png'});
document.buffers[0].byteLength=offset;
const binary=Buffer.concat(chunks), json=Buffer.from(JSON.stringify(document));
const jsonPadded=Buffer.concat([json,Buffer.alloc((4-json.length%4)%4,32)]), binPadded=Buffer.concat([binary,Buffer.alloc((4-binary.length%4)%4)]);
const header=Buffer.alloc(12);header.writeUInt32LE(0x46546c67);header.writeUInt32LE(2,4);header.writeUInt32LE(28+jsonPadded.length+binPadded.length,8);
const jh=Buffer.alloc(8);jh.writeUInt32LE(jsonPadded.length);jh.writeUInt32LE(0x4e4f534a,4);
const bh=Buffer.alloc(8);bh.writeUInt32LE(binPadded.length);bh.writeUInt32LE(0x004e4942,4);
const glb=Buffer.concat([header,jh,jsonPadded,bh,binPadded]);
const rig={ schema:1,space:'engine +X forward, +Y up, +Z right; metres; origin is the shared AK hold frame',
  muzzles:{w_shotgun_body:[0.86,0.105,0]},pivots:{w_shotgun_magazine:[0.17,0.015,0]},grip_id:3,sight:[0,0.165,0],
  part_order:names,source:'Original authored geometry and procedural RGB8 material swatches; unchanged existing AK glove meshes reused at runtime' };
const rigBytes=Buffer.from(JSON.stringify(rig,null,2)+'\n');
if(check){assert.deepEqual(fs.readFileSync(out),glb,'GLB is not reproducible');assert.deepEqual(fs.readFileSync(sidecar),rigBytes,'Sidecar is not reproducible');}
else{fs.writeFileSync(out,glb);fs.writeFileSync(sidecar,rigBytes);}
// Validate the serialized artifact, not just pre-export builder state.
const actual=fs.readFileSync(out);assert.equal(actual.readUInt32LE(0),0x46546c67);assert.equal(actual.readUInt32LE(8),actual.length);
const jsonLength=actual.readUInt32LE(12), exported=JSON.parse(actual.subarray(20,20+jsonLength).toString());
assert.deepEqual(exported.nodes.map(n=>n.name),names);assert.deepEqual(exported.meshes.map(m=>m.name),names);
let triangles=0;
function readAccessor(index,components) {
  const a=exported.accessors[index], v=exported.bufferViews[a.bufferView];
  assert.equal(a.componentType,5126);assert.equal(a.byteOffset??0,0);
  const start=28+jsonLength+v.byteOffset;
  return Array.from({length:a.count},(_,i)=>Array.from({length:components},(_,c)=>actual.readFloatLE(start+(i*components+c)*4)));
}
// Use the actual serialized float32 triangles and the engine's optical-ray test.
// A bead ending exactly on the sight line still blocks the centre pixel.
function blocksSight(points) {
  const y=Math.fround(rig.sight[1]),z=Math.fround(rig.sight[2]);
  for(let i=0;i<points.length;i+=3) {
    const [a,b,c]=points.slice(i,i+3), e=subtract(b,a), f=subtract(c,a), det=e[1]*f[2]-f[1]*e[2];
    if(Math.abs(det)<1e-10)continue;
    const oy=y-a[1],oz=z-a[2],u=(oy*f[2]-oz*f[1])/det,v=(e[1]*oz-e[2]*oy)/det;
    if(u>=0&&v>=0&&u+v<=1&&a[0]+u*e[0]+v*f[0]>-0.62)return true;
  }
  return false;
}
for(const m of exported.meshes)for(const p of m.primitives){
  assert.equal(p.mode,4);assert.notEqual(p.attributes.NORMAL,undefined);assert.notEqual(p.attributes.TEXCOORD_0,undefined);
  const a=exported.accessors[p.attributes.POSITION];triangles+=a.count/3;assert(a.min.every(Number.isFinite)&&a.max.every(Number.isFinite));
  const positions=readAccessor(p.attributes.POSITION,3),normals=readAccessor(p.attributes.NORMAL,3),uvs=readAccessor(p.attributes.TEXCOORD_0,2);
  assert.equal(normals.length,positions.length);assert.equal(uvs.length,positions.length);
  assert(positions.flat().every(Number.isFinite));assert(normals.every(n=>n.every(Number.isFinite)&&Math.abs(Math.hypot(...n)-1)<1e-5));
  assert(uvs.flat().every(n=>Number.isFinite(n)&&n>=0&&n<=1));
  assert(!blocksSight(positions),`${m.name} obstructs the actual optical ray`);
  assert.deepEqual(exported.materials[p.material].pbrMetallicRoughness.baseColorFactor,[1,1,1,1]);
}
const imageView=exported.bufferViews[exported.images[0].bufferView];
const png=actual.subarray(28+jsonLength+imageView.byteOffset,28+jsonLength+imageView.byteOffset+imageView.byteLength);
assert.equal(png.readUInt32BE(16),size);assert.equal(png.readUInt32BE(20),size);assert.equal(png[24],8);assert.equal(png[25],2);
assert(triangles<5000&&glb.length<600000,'Shotgun asset budget exceeded');
for(const [file,hash] of Object.entries(preserved))assert.equal(digest(fs.readFileSync(path.join(assetDir,file))),hash,'Legacy asset changed');
const bounds=exported.accessors[exported.meshes[0].primitives[0].attributes.POSITION];
assert(bounds.min[0]<-0.4&&bounds.max[0]>=0.86&&bounds.max[0]<0.87,'Muzzle/facing metric frame changed');
const report={passed:true,checkOnly:check,glb:path.relative(root,out),bytes:glb.length,sha256:digest(glb),triangles,
  nodes:names,texture:{width:size,height:size,bitDepth:8,format:'RGB',copiesAtRuntime:2,estimatedBytesWithMips:Math.ceil(2*size*size*4*4/3)},
  bodyBounds:{min:bounds.min,max:bounds.max},opticalRayClear:true,rig,preserved,elapsedSeconds:(Date.now()-started)/1000,
  limits:'Serialized geometry/names/UV/material/PNG budget proof. Actual engine import, first/third-person grip contact and rendered realism require root browser/native gates.'};
const reportDir=path.join(root,'target','shotgun-asset');fs.mkdirSync(reportDir,{recursive:true});fs.writeFileSync(path.join(reportDir,'results.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report,null,2));
