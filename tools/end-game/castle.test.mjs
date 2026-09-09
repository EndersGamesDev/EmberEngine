import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {CastleDialogue} from '../../web/games/end-game/v12/castle-audio.js';
import {castleView} from '../../web/games/end-game/v12/castle-ui.js';
import {CASTLE_LINES} from '../../web/games/end-game/v12/voice-lines.js';

const lines = Object.fromEntries(['boss_intro','boss_phase2','boss_defeat','escape_clue','escape_ending'].map(k => [k,{duration:1,text:k,speaker:'Castellan'}]));
function fixture(ready=true) {
  const calls=[], captions=[];
  const output={ready:()=>ready,play:k=>calls.push(k),stop:()=>{},pause:()=>{},resume:()=>{}};
  const player=new CastleDialogue(output,lines,t=>captions.push(t));
  return {player,calls,captions};
}
const tick=(p,n=20)=>{for(let i=0;i<n;i++)p.tick(.25);};
test('fixed tick event history preserves queued story and repeated snapshots do not replay',()=>{
  const f=fixture(), events=[{id:1,kind:'boss_intro'},{id:2,kind:'boss_phase2'}];
  f.player.ingest(events);f.player.tick(.1);assert.deepEqual(f.calls,['boss_intro']);
  tick(f.player);f.player.ingest(events);assert.deepEqual(f.calls,['boss_intro','boss_phase2']);
});
test('boss death and escape supersede combat lines; castle checkpoint does not replay ids',()=>{
  const f=fixture(); f.player.ingest([{id:1,kind:'boss_intro'}]);f.player.tick(.1);
  f.player.ingest([{id:2,kind:'boss_phase2'},{id:3,kind:'boss_defeat'}]);f.player.tick(.1);
  assert.deepEqual(f.calls,['boss_intro','boss_defeat']);
  f.player.ingest([{id:4,kind:'escape_ending'}]);f.player.tick(.1);
  f.player.ingest([{id:1,kind:'boss_intro'}]);tick(f.player);
  assert.deepEqual(f.calls,['boss_intro','boss_defeat','escape_ending']);
});
test('pause and long background frame do not consume speech; unavailable clips retain captions',()=>{
  const f=fixture(false);f.player.ingest([{id:1,kind:'escape_clue'}]);tick(f.player,6);
  assert.equal(f.captions.at(-1).text,'escape_clue');
  const elapsed=f.player.active.elapsed;f.player.setPaused(true);tick(f.player,50);
  f.player.setPaused(false);f.player.tick(120);
  assert.equal(f.player.active.elapsed,elapsed);
});
test('castle speech waits for the warden channel',()=>{
  const f=fixture();f.player.ingest([{id:1,kind:'boss_intro'}]);f.player.tick(.1,true);
  assert.deepEqual(f.calls,[]);f.player.tick(.1,false);assert.deepEqual(f.calls,['boss_intro']);
});
test('HUD exposes discovered clues and distinguishes unblockable boss warnings',()=>{
  const state={stage:5,objective:'Seek the bell',quest:{sequence:2,journal:['The wolf wakes.'],seal:true,shrinesUsed:1,defeated:4},enemy:{name:'Castellan',health:190,maxHealth:420,boss:true,cue:'Maul raised — dodge',unblockable:true},exploration:{visited:4}};
  const view=castleView(state);
  assert.deepEqual(view.journal,['The wolf wakes.']);
  assert.equal(view.seals,'Ancient seals · 2 / 3 awakened');
  assert.match(view.inventory,/2 healing shrines/);assert.equal(view.enemy.danger,true);
  assert.equal(castleView({...state,stage:0}),null);
});
test('every shipped castle voice matches its caption duration and generated source bytes',()=>{
  assert.deepEqual(Object.keys(CASTLE_LINES).sort(),Object.keys(lines).sort());
  for(const line of Object.values(CASTLE_LINES)) {
    const file=readFileSync(new URL(`../../web/games/end-game/v12/${line.file}`,import.meta.url));
    const source=readFileSync(new URL(`../../assets/end-game/v10/${line.file}`,import.meta.url));
    assert.ok(file.equals(source),line.file);
    assert.equal(file.toString('ascii',0,4),'RIFF');
    let rate=0,data=0;
    for(let p=12;p+8<=file.length;){const name=file.toString('ascii',p,p+4),size=file.readUInt32LE(p+4);if(name==='fmt ')rate=file.readUInt32LE(p+16);if(name==='data')data=size;p+=8+size+(size%2);}
    assert.ok(rate>0&&data>0,line.file);
    assert.ok(Math.abs(data/rate-line.duration)<0.001,line.file);
  }
});
