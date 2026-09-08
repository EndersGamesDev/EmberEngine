import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {CastleDialogue} from '../../web/games/end-game/v11/castle-audio.js';
import {renderCastle} from '../../web/games/end-game/v11/castle-ui.js';

// Execute the actual shell with passive DOM/API doubles. No browser, display,
// workstation input or audio playback is driven by these input regressions.
const source=readFileSync(new URL('../../web/games/end-game/v11/main.js',import.meta.url),'utf8').replace(/^import .*;\r?\n/gm,'');
const html=readFileSync(new URL('../../web/games/end-game/v11/index.html',import.meta.url),'utf8');
function fixture() {
  const nodes=new Map(), calls=[], audio=[];
  class Element {
    constructor(id='') { this.id=id;this.handlers=new Map();this.style={};this.dataset={};this.tagName='DIV';this.hidden=false;this.open=false;this.value=55; }
    addEventListener(name,fn) { const list=this.handlers.get(name)||[];list.push(fn);this.handlers.set(name,list); }
    emit(name,props={}) { const e={pointerId:1,pointerType:'touch',button:0,buttons:0,repeat:false,preventDefault(){this.prevented=true;},target:this,currentTarget:this,...props};for(const fn of this.handlers.get(name)||[])fn(e);return e; }
    querySelector(selector) { return get(selector); }
    querySelectorAll() { return []; }
    setPointerCapture() {} focus() {} append() {} close() {this.open=false;} showModal(){this.open=true;}
    getContext(){return null;} getBoundingClientRect(){return {left:0,top:0,width:100,height:100};}
  }
  const get=id=>{if(!nodes.has(id))nodes.set(id,new Element(id));return nodes.get(id);};
  const guard=get('guard-button'), crouch=get('crouch-button'), strike=get('strike-button'), heavy=get('heavy-button'), jump=get('jump-button');
  guard.dataset.hold='4';crouch.dataset.hold='2';strike.dataset.action='2';jump.dataset.action='4';
  heavy.dataset.action=html.match(/<button[^>]*class="touch-heavy"[^>]*data-action="(\d+)"/)[1];
  const document=new Element('document');document.body=get('body');document.getElementById=get;
  document.createElement=()=>new Element();document.querySelector=get;
  document.querySelectorAll=selector=>selector==='[data-action],[data-hold]'?[guard,crouch,strike,heavy,jump]:[];
  const window=new Element('window');window.setTimeout=()=>{};
  const noop=()=>{};
  const parameter=()=>({values:[],setValueAtTime(value){this.values.push(value);},exponentialRampToValueAtTime(value){this.values.push(value);}});
  const audioNode=kind=>{const node={kind,type:'',frequency:parameter(),gain:parameter(),Q:{value:0},connect(next){return next;},start(){},stop(){}};audio.push(node);return node;};
  const audioGraph={currentTime:10,sampleRate:24000,destination:{},createBuffer:()=>({getChannelData:()=>new Float32Array(7680)}),createBufferSource:()=>audioNode('noise'),createBiquadFilter:()=>audioNode('filter'),createGain:()=>audioNode('gain'),createOscillator:()=>audioNode('tone')};
  const context=vm.createContext({document,window,console,Map,Math,Number,JSON,Array,Event:class{},
    performance:{now:()=>0},requestAnimationFrame:noop,matchMedia:()=>({matches:true}),
    innerWidth:1000,innerHeight:700,devicePixelRatio:1,navigator:{getGamepads:()=>[],deviceMemory:4},
    Audio:class{play(){return Promise.resolve();}pause(){}},
    Quality:class{constructor(){this.scale=1;this.mode='auto';this.display={};}sample(){}clamp(){}},
    VoiceAudio:class{resume(){}setVolume(){}preload(){}stop(){}pause(){}ready(){return true;}play(){}},
    CastleDialogue, renderCastle, CASTLE_LINES:{},
    WardenDialogue:class{setPaused(){}ingest(){}tick(){}finish(){}},__calls:calls,__audioGraph:audioGraph});
  vm.runInContext(source,context);
  const run=code=>vm.runInContext(code,context);
  run('api={touch_input:(...args)=>__calls.push(["held",...args]),action:mask=>__calls.push(["action",mask]),pause:flag=>__calls.push(["pause",flag])}; started=true;');
  return {run,calls,get,guard,crouch,strike,heavy,jump,audio,document,window,held:()=>run('held')};
}

test('right mouse and F hold independently and right click never attacks',()=>{
  const f=fixture(),canvas=f.get('canvas');canvas.tagName='CANVAS';
  f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:2,target:canvas});
  f.get('ember-root').emit('mousedown',{pointerType:'mouse',button:2,target:canvas});
  assert.equal(f.held(),4);assert.equal(f.calls.some(c=>c[0]==='action'),false);
  assert.equal(f.get('ember-root').emit('contextmenu').prevented,true);
  f.document.emit('keydown',{code:'KeyF'});f.window.emit('mouseup',{button:2});
  assert.equal(f.held(),4);f.document.emit('keyup',{code:'KeyF'});assert.equal(f.held(),0);
  f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:0,target:canvas});
  assert.deepEqual(f.calls.filter(c=>c[0]==='action').map(c=>c[1]),[2]);
});

test('touch release/cancel/lost capture preserve other fingers and crouch',()=>{
  for(const ending of ['pointerup','pointercancel','lostpointercapture']) {
    const f=fixture();f.guard.emit('pointerdown',{pointerId:1});f.guard.emit('pointerdown',{pointerId:2});f.crouch.emit('pointerdown',{pointerId:3});
    assert.equal(f.held(),6);f.guard.emit(ending,{pointerId:1});assert.equal(f.held(),6);
    f.guard.emit(ending,{pointerId:2});assert.equal(f.held(),2);
    f.crouch.emit('pointerup',{pointerId:3});assert.equal(f.held(),0);
  }
});

test('mouse cancellation and lost capture clear guard immediately',()=>{
  for(const ending of ['pointercancel','lostpointercapture']) {
    const f=fixture(),canvas=f.get('canvas');canvas.tagName='CANVAS';
    f.get('ember-root').emit('mousedown',{pointerType:'mouse',button:2,target:canvas});
    f.window.emit(ending,{pointerType:'mouse'});assert.equal(f.held(),0);
    assert.equal(f.calls.at(-1).at(-1),0);
  }
});

test('pause, blur and hidden document clear guard; key repeat cannot resume it',()=>{
  for(const pause of [f=>f.run('setPause(true)'),f=>f.window.emit('blur'),f=>{f.document.hidden=true;f.document.emit('visibilitychange');}]) {
    const f=fixture();f.document.emit('keydown',{code:'KeyF'});f.guard.emit('pointerdown');pause(f);
    assert.equal(f.held(),0);assert.equal(f.run('paused'),true);
    f.guard.emit('pointerdown');assert.equal(f.held(),0);
    f.run('setPause(false)');f.document.emit('keydown',{code:'KeyF',repeat:true});assert.equal(f.held(),0);
    f.document.emit('keyup',{code:'KeyF'});f.document.emit('keydown',{code:'KeyF'});assert.equal(f.held(),4);
  }
});

test('guard discards new mouse/touch strikes without replaying them on release',()=>{
  const f=fixture(),canvas=f.get('canvas');canvas.tagName='CANVAS';
  f.guard.emit('pointerdown');f.strike.emit('pointerdown',{pointerId:2});
  f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:0,target:canvas});
  f.guard.emit('pointerup');assert.equal(f.calls.some(c=>c[0]==='action'),false);
  f.strike.emit('pointerdown',{pointerId:2});assert.equal(f.calls.at(-1)[0],'action');
});

test('block and break events are consumed every frame before the HUD throttle',()=>{
  const f=fixture();
  f.run('guardSound=broken=>__calls.push(["metal",!!broken]);lastHudAt=100;api.state_json=()=>JSON.stringify({time:1,guard:{amount:1,ready:true,impactLeft:.24,brokenLeft:0,blockEvent:1,breakEvent:0}});loop(110);loop(111);');
  assert.deepEqual(f.calls.filter(c=>c[0]==='metal').map(c=>c[1]),[false]);
  f.run('api.state_json=()=>JSON.stringify({time:1,guard:{amount:0,ready:false,impactLeft:0,brokenLeft:.9,blockEvent:1,breakEvent:1}});loop(112);loop(113);');
  assert.deepEqual(f.calls.filter(c=>c[0]==='metal').map(c=>c[1]),[false,true]);
});

test('ready guard uses the visible strike cost and warns separately for unblockable slams',()=>{
  const f=fixture();
  const show=(stamina,enemy)=>{
    f.run(`lastHudAt=0;api.state_json=()=>JSON.stringify({version:'10.0.0',time:2,stage:5,finished:false,form:'Wolf',health:100,stamina:${stamina},objective:'Face the weapon',location:'Great hall',prompt:'',event:0,footsteps:0,guard:{amount:1,ready:true,impactLeft:0,brokenLeft:0,blockEvent:0,breakEvent:0},enemy:${JSON.stringify(enemy)}});loop(100);`);
    return f.get('guard-cue').textContent;
  };
  for(const cost of [32,38]) {
    assert.match(show(cost-1,{blockCost:cost,unblockable:false}),new RegExp(`Low stamina.*${cost}`));
    assert.doesNotMatch(show(cost,{blockCost:cost,unblockable:false}),/Low stamina/);
  }
  assert.match(show(100,{blockCost:null,unblockable:true}),/Crushing slam.*dodge/);
  assert.match(show(27,null),/Low stamina.*28/);
  assert.doesNotMatch(show(28,null),/Low stamina/);
});

test('castle exploration keeps controls active and shows location after escape',()=>{
  const f=fixture();
  f.get('complete').hidden=true;
  f.run(`api.state_json=()=>JSON.stringify({version:'9.0.0',time:2,stage:5,finished:false,form:'Wolf',health:100,stamina:100,objective:'Explore the garden',location:'Backyard garden',exploration:{visited:4,total:7},prompt:'',event:0,footsteps:0});loop(100);`);
  assert.equal(f.get('location').textContent,'Backyard garden');
  assert.equal(f.get('exploration').textContent,'4 / 7 places discovered');
  assert.equal(f.get('hint').hidden,true);
  assert.equal(f.get('complete').hidden,true);
  assert.equal(f.run('paused || ended'),false);
  f.strike.emit('pointerdown');
  assert.equal(f.calls.at(-1)[0],'action');
});

test('only final castle escape opens completion and stops new strikes',()=>{
  const f=fixture();f.get('complete').hidden=true;
  f.run(`api.state_json=()=>JSON.stringify({version:'10.0.0',time:100,stage:5,finished:true,form:'Wolf',health:66,stamina:80,objective:'You escaped the castle',location:'Backyard garden',exploration:{visited:7,total:7},prompt:'',event:7,footsteps:20});loop(100);`);
  assert.equal(f.get('complete').hidden,false);
  assert.equal(f.get('hud').hidden,true);
  assert.equal(f.run('playing()'),false);
  const actions=f.calls.filter(c=>c[0]==='action').length;
  f.strike.emit('pointerdown');assert.equal(f.calls.filter(c=>c[0]==='action').length,actions);
});

test('R, middle click and the actual touch Heavy button count separate heavy presses',()=>{
  const f=fixture(),canvas=f.get('canvas');canvas.tagName='CANVAS';
  assert.equal(f.heavy.dataset.action,'64');
  f.document.emit('keydown',{code:'KeyR'});
  f.document.emit('keydown',{code:'KeyR',repeat:true});
  f.document.emit('keyup',{code:'KeyR'});
  f.document.emit('keydown',{code:'KeyR'});
  assert.equal(f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:1,target:canvas}).prevented,true);
  assert.equal(f.get('ember-root').emit('mousedown',{pointerType:'mouse',button:1,target:canvas}).prevented,true);
  f.heavy.emit('pointerdown');f.heavy.emit('pointermove');f.heavy.emit('pointerup');
  f.heavy.emit('pointerdown',{pointerType:'mouse',button:1});
  f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:1,target:f.get('overlay')});
  assert.deepEqual(f.calls.filter(c=>c[0]==='action').map(c=>c[1]),[64,64,64,64]);
  f.jump.emit('pointerdown');f.heavy.emit('pointerdown');
  assert.deepEqual(f.calls.filter(c=>c[0]==='action').slice(-2).map(c=>c[1]),[4,64]);
});

test('guard, pause and ending reject every heavy input without replay after release',()=>{
  for(const mode of ['guard','pause','ending']) {
    const f=fixture(),canvas=f.get('canvas');canvas.tagName='CANVAS';
    if(mode==='guard')f.guard.emit('pointerdown');
    if(mode==='pause')f.run('setPause(true)');
    if(mode==='ending')f.run('ended=true;setPause(true)');
    f.document.emit('keydown',{code:'KeyR'});f.heavy.emit('pointerdown',{pointerId:2});
    f.get('ember-root').emit('pointerdown',{pointerType:'mouse',button:1,target:canvas});
    assert.equal(f.calls.some(c=>c[0]==='action'),false,mode);
    if(mode==='ending')continue;
    if(mode==='guard')f.guard.emit('pointerup');else f.run('setPause(false)');
    f.document.emit('keydown',{code:'KeyR',repeat:true});
    assert.equal(f.calls.some(c=>c[0]==='action'),false,mode);
    f.document.emit('keyup',{code:'KeyR'});f.document.emit('keydown',{code:'KeyR'});
    assert.deepEqual(f.calls.filter(c=>c[0]==='action').map(c=>c[1]),[64]);
  }
});

const shellState=combat=>({version:'11.0.0',time:2,stage:5,finished:false,form:'Wolf',health:100,stamina:100,objective:'Face the weapon',location:'Great hall',prompt:'',event:0,footsteps:0,combat:{swingEvent:0,impactEvent:0,impactLeft:0,impactStrength:.7,queued:0,quickLeft:0,rhythmLeft:0,aimProjection:{x:0,y:0},...combat}});
function frame(f,state,now=100) {f.run(`api.state_json=()=>${JSON.stringify(JSON.stringify(state))};loop(${now});`);}

test('five aim zones remain intent; only confirmed timed contact shows a hit label',()=>{
  const f=fixture();
  for(const zone of ['Head','Left torso','Right torso','Left leg','Right leg']) {
    frame(f,shellState({aimedZone:zone}),100);
    assert.equal(f.get('aim-zone').hidden,false);assert.equal(f.get('aim-zone').textContent,`Aim · ${zone}`);
    assert.equal(f.get('impact-feedback').hidden,true);
  }
  frame(f,shellState({aimedZone:'Head',impactZone:'Left leg',impactKind:'Enemy',impactLeft:.2,impactEvent:1}),110);
  assert.equal(f.get('impact-feedback').textContent,'Hit · Left leg');
  assert.equal(f.get('impact-feedback').hidden,false);
  frame(f,shellState({aimedZone:'Head',impactZone:'Left leg',impactKind:'Enemy',impactLeft:0,impactEvent:1}),111);
  assert.equal(f.get('impact-feedback').hidden,true);
  frame(f,shellState({impactSurface:'Timber',impactKind:'Surface',impactLeft:.2,impactEvent:2}),112);
  assert.equal(f.get('aim-zone').hidden,true);assert.equal(f.get('impact-feedback').textContent,'Wood impact');
  frame(f,shellState({impactKind:'Chain',impactLeft:.2,impactEvent:3}),113);
  assert.equal(f.get('impact-feedback').textContent,'Chain struck');
  f.run('setPause(true)');frame(f,shellState({aimedZone:'Head',impactZone:'Head',impactLeft:.2}),114);
  assert.equal(f.get('aim-zone').hidden,true);assert.equal(f.get('impact-feedback').hidden,true);
  f.run('paused=false;ended=true');frame(f,shellState({aimedZone:'Head',impactZone:'Head',impactLeft:.2}),115);
  assert.equal(f.get('aim-zone').hidden,true);assert.equal(f.get('impact-feedback').hidden,true);
});

test('airborne heavy cuts whoosh before the later landing hold and contact events play once',()=>{
  const f=fixture();f.run('swordSound=(...args)=>__calls.push(["sword",...args]);');
  const attack={kind:'Jumping heavy',elapsed:.27,windup:.28,landingWait:false};
  frame(f,shellState({active:attack,label:'Jumping heavy',swingEvent:1}),100);
  assert.doesNotMatch(f.get('combo-cue').textContent,/waiting for landing/);assert.equal(f.calls.some(c=>c[0]==='sword'),false);
  attack.elapsed=.30;
  frame(f,shellState({active:attack,swingEvent:1}),109);
  assert.deepEqual(f.calls.filter(c=>c[0]==='sword').map(c=>Array.from(c).slice(1)),[[false]]);
  attack.elapsed=.43;
  frame(f,shellState({active:attack,swingEvent:1,impactEvent:1,impactSurface:'Stone',impactKind:'Surface',impactLeft:.2}),110);
  frame(f,shellState({active:attack,swingEvent:1,impactEvent:1,impactSurface:'Stone',impactKind:'Surface',impactLeft:.15}),111);
  assert.deepEqual(f.calls.filter(c=>c[0]==='sword').map(c=>Array.from(c).slice(1)),[[false],[true,.7,'Stone']]);
  frame(f,shellState({swingEvent:1,impactEvent:2,impactZone:'Head',impactSurface:'Iron',impactKind:'Enemy',impactLeft:.2}),112);
  assert.deepEqual(Array.from(f.calls.filter(c=>c[0]==='sword').at(-1)).slice(1),[true,.7,'Body']);
  frame(f,shellState({swingEvent:1,impactEvent:3,impactKind:'Chain',impactLeft:.2}),113);
  assert.deepEqual(Array.from(f.calls.filter(c=>c[0]==='sword').at(-1)).slice(1),[true,.7,'Iron']);
  attack.elapsed=.62;attack.landingWait=true;
  frame(f,shellState({active:attack,label:'Jumping heavy',swingEvent:1,impactEvent:3}),300);
  assert.match(f.get('combo-cue').textContent,/waiting for landing/);
  const sounds=f.calls.filter(c=>c[0]==='sword').length;
  attack.elapsed=.64;attack.landingWait=false;
  frame(f,shellState({active:attack,label:'Jumping heavy',swingEvent:1,impactEvent:3}),400);
  assert.doesNotMatch(f.get('combo-cue').textContent,/waiting for landing/);
  assert.equal(f.calls.filter(c=>c[0]==='sword').length,sounds,'landing adds no replayed swing or contact sound');
});

test('authored body, stone, timber and iron graphs differ and obey mute/pause',()=>{
  const signatures=new Map();
  for(const surface of ['Body','Stone','Paving','Timber','Iron','Grass']) {
    const f=fixture();f.run(`audioContext=__audioGraph;swordSound(true,.7,'${surface}');`);
    signatures.set(surface,JSON.stringify(f.audio.filter(n=>['filter','tone'].includes(n.kind)).map(n=>({kind:n.kind,type:n.type,frequency:n.frequency.values}))));
    assert.ok(f.audio.length>0);
    const count=f.audio.length;f.run('volume=0;swordSound(true);');assert.equal(f.audio.length,count);
    f.run('volume=.55;paused=true;swordSound(true);');assert.equal(f.audio.length,count);
  }
  assert.equal(signatures.get('Stone'),signatures.get('Paving'));
  assert.equal(new Set(['Body','Stone','Timber','Iron','Grass'].map(s=>signatures.get(s))).size,5);
});

test('blade reticle uses frame projection and viewport aspect every frame',()=>{
  const f=fixture();f.run('innerWidth=1200;innerHeight=600;');
  frame(f,shellState({aimedZone:'Head',aimProjection:{x:.8,y:-.4}}),100);
  const reticle=f.get('.crosshair');
  assert.equal(reticle.hidden,false);assert.equal(reticle.style.left,'70%');assert.equal(reticle.style.top,'70%');
  f.run('innerWidth=600;innerHeight=600;');
  frame(f,shellState({aimedZone:'Head',aimProjection:{x:.8,y:-.4}}),101);
  assert.equal(reticle.style.left,'90%');assert.equal(reticle.style.top,'70%');
  assert.equal(f.get('.blade-focus').style.left,'510px');
  frame(f,shellState({aimProjection:{x:-.8,y:.5}}),102);
  assert.ok(Math.abs(Number.parseFloat(reticle.style.left)-10)<1e-8);
  assert.equal(reticle.style.top,'25%');assert.equal(f.get('aim-zone').hidden,true);
  assert.equal(reticle.hidden,false,'a neutral valid blade aim still has a reticle');
});

test('offscreen or invalid blade projection hides aim rather than clamping the reticle',()=>{
  const f=fixture();f.run('innerWidth=600;innerHeight=600;');
  frame(f,shellState({aimedZone:'Head',aimProjection:{x:.5,y:0}}),100);
  const reticle=f.get('.crosshair');assert.equal(reticle.style.left,'75%');
  for(const projection of [null,{x:1.01,y:0},{x:-1.01,y:0},{x:0,y:1.01},{x:0,y:-1.01},{x:null,y:0},{x:0,y:null}]) {
    frame(f,shellState({aimedZone:'Head',aimProjection:projection}),101);
    assert.equal(reticle.hidden,true,JSON.stringify(projection));assert.equal(f.get('aim-zone').hidden,true);
    assert.equal(reticle.style.left,'75%','invalid aim must not be moved to a viewport edge');
  }
  const unarmed=shellState({aimProjection:null});unarmed.stage=2;frame(f,unarmed,102);
  assert.equal(reticle.hidden,false);assert.equal(reticle.style.left,'50%');assert.equal(reticle.style.top,'50%');
});
