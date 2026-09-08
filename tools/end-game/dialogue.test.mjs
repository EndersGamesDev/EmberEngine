import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {WardenDialogue, VoiceAudio, LINES} from '../../web/games/end-game/v7/dialogue.js';

function fixture(ready=true) {
  const calls=[], captions=[];
  const output={ready:()=>ready, play:k=>calls.push(['play',k]), stop:()=>calls.push(['stop']), pause:()=>calls.push(['pause']), resume:()=>calls.push(['resume'])};
  const player=new WardenDialogue(output,t=>captions.push(t));
  return {player,output,calls,captions,played:()=>calls.filter(c=>c[0]==='play').map(c=>c[1])};
}
const event=(id,kind,time=1)=>({id,kind,time});
const snapshot=(events,life=0,dead=false)=>({life,events,dead});

test('repeated snapshots never repeat a line, and skipped frames retain both story reactions',()=>{
  const f=fixture();
  const state=snapshot([event(1,'key'),event(2,'sword')]);
  f.player.ingest(state,1);f.player.tick(.01);
  assert.deepEqual(f.played(),['key']);
  for(let i=0;i<8;i++){f.player.ingest(state,2);f.player.tick(.1);}
  assert.deepEqual(f.played(),['key']);
  f.player.tick(4);
  assert.deepEqual(f.played(),['key','sword']);
  f.player.tick(4);f.player.ingest(state,5);f.player.tick(.01);
  assert.deepEqual(f.played(),['key','sword']);
});

test('story interrupts a bark; death interrupts all live lines and suppresses future speech',()=>{
  const f=fixture();
  f.player.ingest(snapshot([event(1,'movement')]),1);f.player.tick(.01);
  f.player.ingest(snapshot([event(1,'movement'),event(2,'key')]),1.1);f.player.tick(.01);
  assert.deepEqual(f.played(),['movement','key']);
  f.player.ingest(snapshot([event(3,'sword'),event(4,'death')],0,true),1.2);f.player.tick(.01);
  assert.deepEqual(f.played(),['movement','key','death']);
  f.player.tick(4);
  f.player.ingest(snapshot([event(5,'movement'),event(6,'sword')],0,true),2);f.player.tick(.01);
  assert.equal(f.player.active,null);
  assert.deepEqual(f.played(),['movement','key','death']);
});

test('a movement bark never queues behind important dialogue',()=>{
  const f=fixture();f.player.ingest(snapshot([event(1,'key')]),1);f.player.tick(.01);
  f.player.ingest(snapshot([event(2,'movement')]),2);f.player.tick(4);
  assert.deepEqual(f.played(),['key']);
  assert.equal(f.player.queue.length,0);
});

test('pause freezes caption time and resumes the same clip without replaying its event',()=>{
  const f=fixture();f.player.ingest(snapshot([event(1,'key')]),1);f.player.tick(.01);f.player.tick(.5);
  f.player.setPaused(true);const elapsed=f.player.active.elapsed;f.player.tick(20);
  assert.equal(f.player.active.elapsed,elapsed);assert.equal(f.captions.at(-1),null);
  f.player.setPaused(false);assert.equal(f.captions.at(-1),LINES.key.text);
  // Resume input can precede the first frame after a long background gap.
  f.player.tick(20);assert.equal(f.player.active.elapsed,elapsed);
  f.player.tick(.25);assert.equal(f.player.active.elapsed,elapsed+.25);
  assert.deepEqual(f.played(),['key']);
  assert.ok(f.calls.some(c=>c[0]==='pause'));assert.ok(f.calls.some(c=>c[0]==='resume'));
});

test('a new life clears old audio and accepts reused event IDs',()=>{
  const f=fixture();f.player.ingest(snapshot([event(1,'death')],0,true),1);f.player.tick(.01);
  f.player.ingest(snapshot([],1),0);assert.equal(f.player.active,null);assert.equal(f.player.dead,false);
  f.player.ingest(snapshot([event(1,'movement',.5)],1),.5);f.player.tick(.01);
  assert.deepEqual(f.played(),['death','movement']);
});

test('unavailable audio gets timed subtitles without blocking the queue',()=>{
  const f=fixture(false);f.player.ingest(snapshot([event(1,'key'),event(2,'sword')]),1);
  f.player.tick(.5);assert.equal(f.player.active,null);
  f.player.tick(.8);assert.equal(f.captions.at(-1),LINES.key.text);
  f.player.tick(4);assert.equal(f.captions.at(-1),LINES.sword.text);
});

test('old barks and stale story history are not replayed after delayed initialization',()=>{
  const f=fixture();f.player.ingest(snapshot([event(1,'movement',1),event(2,'key',2)]),20);f.player.tick(.01);
  assert.deepEqual(f.played(),[]);assert.equal(f.player.cursor,2);
});

test('chapter completion allows the death line to finish and clears live speech',()=>{
  const f=fixture();f.player.ingest(snapshot([event(1,'death')],0,true),1);f.player.tick(.01);
  f.player.finish();f.player.tick(.5);assert.equal(f.player.active.event.kind,'death');
  f.player.tick(4);assert.equal(f.player.active,null);
  const g=fixture();g.player.ingest(snapshot([event(1,'key')]),1);g.player.tick(.01);g.player.finish();
  assert.equal(g.player.active,null);assert.equal(g.player.queue.length,0);
});

function contextFixture() {
  const sources=[],gains=[];
  const context={state:'running',currentTime:10,destination:{},
    createBufferSource(){const s={connect:g=>g,disconnect(){},stop(){s.stopped=true},start:(when,offset)=>{s.offset=offset}};sources.push(s);return s},
    createGain(){const g={gain:{value:0},connect(){},disconnect(){}};gains.push(g);return g}
  };
  return {context,sources,gains};
}
test('audio pause resumes at its saved offset, mute updates gain, and stop cancels the clip',()=>{
  const f=contextFixture(),audio=new VoiceAudio(()=>f.context);
  audio.buffers.set('key',{duration:4});audio.play('key');assert.equal(f.sources[0].offset,0);
  f.context.currentTime=10.75;audio.pause();assert.equal(f.sources[0].stopped,true);
  f.context.currentTime=30;audio.resume();assert.equal(f.sources[1].offset,.75);
  audio.setVolume(0);assert.equal(f.gains.at(-1).gain.value,0);
  audio.setVolume(.6);assert.equal(f.gains.at(-1).gain.value,.6);
  audio.stop();audio.resume();assert.equal(f.sources.length,2);
});
test('blocked or missing audio never starts late when the context or buffer becomes available',()=>{
  const f=contextFixture(),audio=new VoiceAudio(()=>f.context);
  audio.play('key');audio.buffers.set('key',{duration:4});audio.resume();assert.equal(f.sources.length,0);
  f.context.state='suspended';audio.play('key');f.context.state='running';audio.resume();assert.equal(f.sources.length,0);
});
test('shipped cue timings match the actual mono PCM voice files',()=>{
  for(const line of Object.values(LINES)) {
    const wav=readFileSync(new URL(`../../assets/end-game/v6/${line.file}`,import.meta.url));
    assert.equal(wav.toString('ascii',0,4),'RIFF');assert.equal(wav.toString('ascii',8,12),'WAVE');
    let format,bytes;
    for(let i=12;i+8<=wav.length;) {
      const name=wav.toString('ascii',i,i+4),size=wav.readUInt32LE(i+4);
      if(name==='fmt ')format=wav.subarray(i+8,i+8+size);
      if(name==='data')bytes=size;
      i+=8+size+(size%2);
    }
    assert.equal(format.readUInt16LE(0),1);assert.equal(format.readUInt16LE(2),1);
    assert.equal(format.readUInt16LE(14),16);
    const duration=bytes/format.readUInt32LE(8);
    assert.ok(Math.abs(duration-line.duration)<.0001,`${line.file}: ${duration} != ${line.duration}`);
  }
});
