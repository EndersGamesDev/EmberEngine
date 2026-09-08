import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {CastleDialogue} from '../../web/games/end-game/v10/castle-audio.js';
import {renderCastle} from '../../web/games/end-game/v10/castle-ui.js';

// Execute the actual shell with passive DOM/API doubles. No browser, display,
// workstation input or audio playback is driven by these input regressions.
const source=readFileSync(new URL('../../web/games/end-game/v10/main.js',import.meta.url),'utf8').replace(/^import .*;\r?\n/gm,'');
function fixture() {
  const nodes=new Map(), calls=[];
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
  const guard=get('guard-button'), crouch=get('crouch-button'), strike=get('strike-button');
  guard.dataset.hold='4';crouch.dataset.hold='2';strike.dataset.action='2';
  const document=new Element('document');document.body=get('body');document.getElementById=get;
  document.createElement=()=>new Element();document.querySelector=get;
  document.querySelectorAll=selector=>selector==='[data-action],[data-hold]'?[guard,crouch,strike]:[];
  const window=new Element('window');window.setTimeout=()=>{};
  const noop=()=>{};
  const context=vm.createContext({document,window,console,Map,Math,Number,JSON,Array,Event:class{},
    performance:{now:()=>0},requestAnimationFrame:noop,matchMedia:()=>({matches:true}),
    innerWidth:1000,innerHeight:700,devicePixelRatio:1,navigator:{getGamepads:()=>[],deviceMemory:4},
    Audio:class{play(){return Promise.resolve();}pause(){}},
    Quality:class{constructor(){this.scale=1;this.mode='auto';this.display={};}sample(){}clamp(){}},
    VoiceAudio:class{resume(){}setVolume(){}preload(){}stop(){}pause(){}ready(){return true;}play(){}},
    CastleDialogue, renderCastle, CASTLE_LINES:{},
    WardenDialogue:class{setPaused(){}ingest(){}tick(){}finish(){}},__calls:calls});
  vm.runInContext(source,context);
  const run=code=>vm.runInContext(code,context);
  run('api={touch_input:(...args)=>__calls.push(["held",...args]),action:mask=>__calls.push(["action",mask]),pause:flag=>__calls.push(["pause",flag])}; started=true;');
  return {run,calls,get,guard,crouch,strike,document,window,held:()=>run('held')};
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
