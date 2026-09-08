import { Quality } from './quality.js';
const $ = id => document.getElementById(id);
const coarse = matchMedia('(pointer:coarse)').matches;
let api, started = false, paused = false, ended = false, loading = false;
let lastEvent = -1, lastSteps = 0, lastMessageAt = 0, lastTime = performance.now(), lastHudAt = 0;
let movement = [0, 0], look = [0, 0], held = 0, volume = 0.55, audioContext;
let padPrevious = [];
let releaseGate = null, hadPointerLock = false;
let lastWhoosh = 0, lastImpact = 0, swingNoise;
let lastKnifeAttack = 0, lastKnifeHit = 0;
const ambience = new Audio('./ambience.wav'); ambience.loop = true; ambience.volume = volume;
let maxDimension = 8192;
try {
  const probe = document.createElement('canvas').getContext('webgl2');
  if (probe) { maxDimension = probe.getParameter(probe.MAX_RENDERBUFFER_SIZE); probe.getExtension('WEBGL_lose_context')?.loseContext(); }
} catch {}
const quality = new Quality({width: innerWidth, height: innerHeight, dpr: devicePixelRatio || 1, memory: navigator.deviceMemory || 4, mobile: coarse, maxDimension});
globalThis.__emberRenderScale = quality.scale;
globalThis.__emberInputPaused = false;

globalThis.emberRumble = (strong, weak, ms) => {
  const pad = Array.from(navigator.getGamepads?.() || []).find(p => p?.connected && p.mapping === 'standard');
  const effect = pad?.vibrationActuator?.playEffect?.('dual-rumble', {duration: Math.min(ms, 5000), strongMagnitude: strong, weakMagnitude: weak});
  effect?.catch?.(() => {});
};
function sound(kind = 'step') {
  if (!volume || !audioContext || paused) return;
  const now = audioContext.currentTime;
  const oscillator = audioContext.createOscillator(), gain = audioContext.createGain();
  oscillator.type = kind === 'step' ? 'sine' : 'triangle';
  oscillator.frequency.setValueAtTime(kind === 'step' ? 90 : 180, now);
  oscillator.frequency.exponentialRampToValueAtTime(kind === 'step' ? 30 : 46, now + 0.17);
  gain.gain.setValueAtTime(volume * (kind === 'step' ? 0.07 : 0.12), now);
  gain.gain.exponentialRampToValueAtTime(0.001, now + 0.2);
  oscillator.connect(gain).connect(audioContext.destination); oscillator.start(); oscillator.stop(now + 0.22);
}
function unlockAudio() {
  try { audioContext ||= new (window.AudioContext || window.webkitAudioContext)(); audioContext.resume().catch(() => {}); } catch {}
  if (volume) ambience.play().catch(() => {});
}
function swordSound(impact, strength = 0.7) {
  if (!audioContext || !volume || paused) return;
  const ctx=audioContext, t=ctx.currentTime;
  if (!swingNoise) {
    swingNoise=ctx.createBuffer(1,Math.ceil(ctx.sampleRate*0.32),ctx.sampleRate);
    const data=swingNoise.getChannelData(0);
    for(let i=0;i<data.length;i++) data[i]=Math.random()*2-1;
  }
  const noise=ctx.createBufferSource(), filter=ctx.createBiquadFilter(), gain=ctx.createGain();
  noise.buffer=swingNoise;filter.type=impact?'lowpass':'bandpass';filter.Q.value=0.65;
  filter.frequency.setValueAtTime(impact?1600:700,t);
  filter.frequency.exponentialRampToValueAtTime(impact?120:140,t+0.23);
  gain.gain.setValueAtTime(0.001,t);
  gain.gain.exponentialRampToValueAtTime(volume*(impact?0.32:0.15)*strength,t+0.012);
  gain.gain.exponentialRampToValueAtTime(0.001,t+0.27);
  noise.connect(filter).connect(gain).connect(ctx.destination);noise.start(t);noise.stop(t+0.30);
  if(impact) {
    const bass=ctx.createOscillator(), body=ctx.createGain();bass.type='triangle';
    bass.frequency.setValueAtTime(92,t);bass.frequency.exponentialRampToValueAtTime(34,t+0.18);
    body.gain.setValueAtTime(volume*0.23*strength,t);body.gain.exponentialRampToValueAtTime(0.001,t+0.26);
    bass.connect(body).connect(ctx.destination);bass.start(t);bass.stop(t+0.28);
  }
}
// Count actual mouse presses, including a down/up pair between render frames.
$('ember-root').addEventListener('pointerdown',event=>{
  if(event.pointerType==='mouse' && event.button===0 && started && !paused && !ended && event.target.tagName==='CANVAS') api?.action(2);
});
function clearTouch() { movement = [0, 0]; look = [0, 0]; held = 0; stickPointer = null; lookPointer = null; $('stick-knob').style.transform = ''; api?.touch_input(0, 0, 0, 0, 0); }
function setPause(value) {
  paused = value; globalThis.__emberInputPaused = value; api?.pause(value); clearTouch();
  if (value) { document.exitPointerLock?.(); ambience.pause(); }
  else if (started && !ended) { unlockAudio(); $('ember-root').querySelector('canvas')?.focus(); }
  $('touch').hidden = !coarse || value || !started || ended;
}
function openSettings() {
  if ($('settings').open || loading || $('prologue').open) return;
  if (started) setPause(true);
  $('resume').textContent = started ? 'Return to the dungeon' : 'Return';
  $('settings').showModal(); $('resume').focus();
}
function closeSettings(button = null) { $('settings').close(); if (started && !ended) { if (Number.isInteger(button)) releaseGate = button; else setPause(false); } }
async function begin() {
  if (started || loading) return;
  loading = true; $('begin').disabled = true; $('boot').hidden = false; unlockAudio();
  try {
    api = await import('./pkg/end_game.js');
    await api.default();
    // Engine installs its canvas and returns; renderer initialization is asynchronous.
    api.start(); started = true; loading = false;
    if (releaseGate !== null) setPause(true);
    $('title-screen').hidden = true; $('menu-button').hidden = false; $('restart').hidden = false;
    $('boot-detail').textContent = 'Lighting the torches…';
    window.setTimeout(() => {
      if (!$('boot').hidden) {
        $('boot-detail').textContent = 'The renderer has not started. Try a browser with WebGPU or WebGL 2 and hardware acceleration enabled.';
        $('boot').append(retryButton());
      }
    }, 25000);
  } catch (error) {
    console.error(error); loading = false;
    $('boot').querySelector('h2').textContent = 'The dungeon could not load.';
    $('boot-detail').textContent = 'Check your connection and browser graphics support, then try again.';
    $('boot').querySelector('progress').hidden = true;
    $('boot').append(retryButton());
  }
}
function retryButton() { const button = document.createElement('button'); button.className = 'primary'; button.textContent = 'Try again'; button.onclick = () => location.reload(); return button; }
$('begin').onclick = begin;
$('title-settings').onclick = openSettings; $('menu-button').onclick = openSettings;
$('close-settings').onclick = closeSettings; $('resume').onclick = closeSettings;
$('settings').addEventListener('cancel', event => { event.preventDefault(); closeSettings(); });
$('restart').onclick = () => location.reload(); $('play-again').onclick = () => location.reload();
$('quality').onchange = event => { quality.set(event.target.value); globalThis.__emberRenderScale = quality.scale; };
$('volume').oninput = event => { volume = Number(event.target.value) / 100; ambience.volume = volume; if (volume && started && !paused) unlockAudio(); else if (!volume) ambience.pause(); };
$('fullscreen').onclick = async () => { try { if (document.fullscreenElement) await document.exitFullscreen(); else await $('game').requestFullscreen(); } catch { $('fullscreen').textContent = 'Fullscreen unavailable in this browser'; } };
$('trailer-button').onclick = () => { ambience.pause(); $('prologue').showModal(); $('film').play().catch(() => {}); };
function closeFilm() { $('film').pause(); $('prologue').close(); if (started) setPause(false); }
$('skip-film').onclick = closeFilm; $('film').onended = closeFilm;
$('prologue').addEventListener('cancel', event => { event.preventDefault(); closeFilm(); });
document.addEventListener('keydown', event => {
  if (event.repeat && event.code === 'Escape') { event.preventDefault(); return; }
  if (event.code === 'Escape' && started && !ended && !$('prologue').open) { event.preventDefault(); if ($('settings').open) closeSettings(); else openSettings(); }
  if (['Space','AltLeft','ArrowUp','ArrowDown'].includes(event.code) && started && !paused) event.preventDefault();
  if ((event.code === 'Enter' || event.code === 'Space') && !started && !loading && !$('settings').open && !$('prologue').open && event.target === document.body) { event.preventDefault(); begin(); }
});
window.addEventListener('blur', () => { clearTouch(); if (started && !ended) openSettings(); });
document.addEventListener('visibilitychange', () => { if (document.hidden && started && !ended) openSettings(); });
document.addEventListener('pointerlockchange', () => {
  const locked = !!document.pointerLockElement;
  if (locked) unlockAudio();
  else if (hadPointerLock && started && !paused && !ended) openSettings();
  hadPointerLock = locked;
});
window.addEventListener('resize', () => { quality.display = {...quality.display, width: innerWidth, height: innerHeight, dpr: devicePixelRatio || 1}; quality.clamp(); globalThis.__emberRenderScale = quality.scale; });

let stickPointer = null, lookPointer = null, lookPrevious;
$('stick').addEventListener('pointerdown', event => { if (stickPointer !== null) return; stickPointer = event.pointerId; event.currentTarget.setPointerCapture(event.pointerId); moveStick(event); });
function moveStick(event) {
  if (event.pointerId !== stickPointer) return;
  const box = $('stick').getBoundingClientRect();
  let x = (event.clientX - box.left - box.width / 2) / 38, y = (event.clientY - box.top - box.height / 2) / 38;
  const size = Math.max(1, Math.hypot(x, y)); x /= size; y /= size;
  movement = [x, -y]; $('stick-knob').style.transform = `translate(${x * 32}px,${y * 32}px)`;
}
$('stick').addEventListener('pointermove', moveStick);
for (const name of ['pointerup','pointercancel','lostpointercapture']) $('stick').addEventListener(name, event => { if (event.pointerId === stickPointer) { stickPointer = null; movement = [0,0]; $('stick-knob').style.transform = ''; } });
$('look-zone').addEventListener('pointerdown', event => { if (lookPointer !== null) return; lookPointer = event.pointerId; lookPrevious = [event.clientX,event.clientY]; event.currentTarget.setPointerCapture(event.pointerId); });
$('look-zone').addEventListener('pointermove', event => { if (event.pointerId === lookPointer) { look[0] += event.clientX - lookPrevious[0]; look[1] += event.clientY - lookPrevious[1]; lookPrevious = [event.clientX,event.clientY]; } });
for (const name of ['pointerup','pointercancel','lostpointercapture']) $('look-zone').addEventListener(name, event => { if (event.pointerId === lookPointer) lookPointer = null; });
for (const button of document.querySelectorAll('[data-action],[data-hold]')) {
  button.addEventListener('pointerdown', event => {
    event.preventDefault(); button.setPointerCapture(event.pointerId);
    if (button.dataset.action) api?.action(Number(button.dataset.action));
    if (button.dataset.hold) held |= Number(button.dataset.hold);
  });
  for (const name of ['pointerup','pointercancel','lostpointercapture']) button.addEventListener(name, () => { if (button.dataset.hold) held &= ~Number(button.dataset.hold); });
}
function gamepadMenu() {
  const pad = Array.from(navigator.getGamepads?.() || []).find(p => p?.connected && p.mapping === 'standard');
  const now = pad ? pad.buttons.map(b => b.pressed) : [];
  if (pad) { now[16] = pad.axes[1] < -0.65; now[17] = pad.axes[1] > 0.65; now[18] = pad.axes[0] < -0.65; now[19] = pad.axes[0] > 0.65; }
  const press = i => now[i] && !padPrevious[i];
  if (pad) $('device-label').textContent = 'Controller connected · Press × to begin';
  if (releaseGate !== null && started && !now[releaseGate]) { releaseGate = null; if (!$('settings').open && !$('prologue').open && !ended) setPause(false); }
  if (!started && !loading && !$('settings').open && !$('prologue').open && press(0)) { releaseGate = 0; begin(); }
  if (!started && !loading && press(3)) openSettings();
  if (started && !ended && press(9)) { if ($('settings').open) closeSettings(); else openSettings(); }
  if ($('settings').open) {
    const items = Array.from($('settings').querySelectorAll('button,select,input')).filter(el => !el.hidden && !el.disabled);
    let index = items.indexOf(document.activeElement);
    const down = press(13) || press(17), up = press(12) || press(16);
    if (down || up) { index = (Math.max(index, 0) + (down ? 1 : -1) + items.length) % items.length; items[index].focus(); }
    const target = document.activeElement;
    const change = press(14) || press(18) ? -1 : press(15) || press(19) ? 1 : 0;
    if (target?.tagName === 'SELECT' && (change || press(0))) {
      target.selectedIndex = (target.selectedIndex + (change || 1) + target.options.length) % target.options.length;
      target.dispatchEvent(new Event('change'));
    } else if (target?.type === 'range' && (change || press(0))) {
      target.value = Math.max(0, Math.min(100, Number(target.value) + (change || 1) * 5)); target.dispatchEvent(new Event('input'));
    } else if (press(0)) {
      if (target === $('resume') || target === $('close-settings')) closeSettings(0);
      else target?.click();
    }
    if (press(1)) closeSettings(1);
  }
  if ($('prologue').open && press(1)) closeFilm();
  if (ended && press(0)) location.reload();
  padPrevious = now;
}
function loop(now) {
  requestAnimationFrame(loop);
  gamepadMenu();
  const ms = now - lastTime; lastTime = now;
  if (!started || !api) return;
  if (!paused && !ended) {
    quality.sample(ms); globalThis.__emberRenderScale = quality.scale;
    api.touch_input(movement[0], movement[1], look[0], look[1], held); look = [0, 0];
  }
  const json = api.state_json(); if (!json) return;
  const state = JSON.parse(json);
  const combat=state.combat;
  const warden=state.warden;
  if(warden) {
    if(warden.attackEvent===0) lastKnifeAttack=0;
    if(warden.phase==='Attacking' && warden.elapsed>=warden.windup && warden.attackEvent!==lastKnifeAttack) {
      lastKnifeAttack=warden.attackEvent; swordSound(false,0.4);
    }
    if(warden.hitEvent!==lastKnifeHit) {lastKnifeHit=warden.hitEvent;if(lastKnifeHit) swordSound(true,0.6);}
    $('hurt').style.opacity=String(Math.max(0,Math.min(1,warden.hitLeft/0.24)));
  }
  // Contact sounds follow simulation frames, independently of the slower HUD.
  if(combat) {
    if(combat.swingEvent===0) lastWhoosh=0;
    if(combat.active && combat.active.elapsed>=combat.active.windup && combat.swingEvent!==lastWhoosh) {lastWhoosh=combat.swingEvent;swordSound(false);}
    if(combat.impactEvent!==lastImpact) {lastImpact=combat.impactEvent;if(lastImpact) swordSound(true,combat.impactStrength);}
  }
  if (now - lastHudAt < 80) return;
  lastHudAt = now;
  document.querySelector('.top-actions .version').textContent = `VERSION ${state.version}`;
  if (!$('boot').hidden) { $('boot').hidden = true; $('hud').hidden = false; $('touch').hidden = !coarse || paused || ended; }
  $('form').textContent = state.form.toUpperCase(); $('health').value = state.health; $('health-value').textContent = Math.ceil(state.health); $('stamina').value = state.stamina;
  $('objective').textContent = state.objective; $('prompt').textContent = state.prompt;
  $('hint').querySelector('kbd').textContent = state.stage === 4 ? (state.pad ? 'R2' : coarse ? 'Strike' : 'LMB') : (state.pad ? '□' : coarse ? 'Use' : 'E');
  const enemyActive=warden && warden.phase!=='Sleeping' && warden.health>0;
  $('alert').hidden = warden ? warden.health<=0 || !warden.near || (state.alert<0.2 && !enemyActive) : state.alert<0.2;
  $('warden-status').textContent = enemyActive ? (warden.phase==='Attacking' && warden.elapsed<warden.contact ? 'Knife raised — dodge or step away' : warden.label) : 'The warden hears you';
  $('warden-health').hidden = !enemyActive;
  if(warden) $('warden-health').value=warden.health;
  if (state.event !== lastEvent) { lastEvent = state.event; lastMessageAt = now; $('message').textContent = state.message; if(state.stage<4 && !(warden?.hitLeft>0)) sound('event'); }
  if(combat) {
    $('combat-hud').hidden=state.stage<4 || state.finished;
    $('combo-name').textContent=combat.active?combat.label:'Greatsword ready';
    $('combo-cue').textContent=combat.queued?`${combat.queued} follow-up${combat.queued>1?'s':''} buffered`:combat.quickLeft>0?'Tap quickly to chain':combat.rhythmLeft>0?'Tap now for a heavy follow-up':combat.active?`${combat.active.kind} · recovering`:'Tap to strike · pause to vary';
    $('rhythm-fill').style.width=`${Math.max(0,Math.min(100,combat.rhythmLeft/0.9*100))}%`;
  }
  $('message').style.opacity = now - lastMessageAt < 6000 ? '1' : '0';
  if (state.footsteps !== lastSteps) { lastSteps = state.footsteps; if (!state.crouched) sound(); }
  $('shortcuts').textContent = state.pad ? 'Left stick move · Right stick look · □ interact · Hold L1 crouch · Options menu' : document.pointerLockElement ? 'WASD move · Mouse look · E interact · C crouch · V camera' : 'Click the dungeon to look around · WASD move · E interact';
  const canvas = $('ember-root').querySelector('canvas');
  $('performance').textContent = canvas ? `${canvas.width} × ${canvas.height} · ${quality.fps || '—'} FPS · ${quality.mode === 'auto' ? 'ADAPTIVE' : quality.mode.toUpperCase()}` : '';
  if (state.finished && !ended) { ended = true; setPause(true); $('complete').hidden = false; $('hud').hidden = true; $('menu-button').hidden = true; $('play-again').focus(); }
}
requestAnimationFrame(loop);
