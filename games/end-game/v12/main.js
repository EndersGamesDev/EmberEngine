import { Quality } from './quality.js';
import { VoiceAudio, WardenDialogue } from './dialogue.js';
import { CastleDialogue } from './castle-audio.js';
import { CASTLE_LINES } from './voice-lines.js';
import { renderCastle } from './castle-ui.js';
const $ = id => document.getElementById(id);
const coarse = matchMedia('(pointer:coarse)').matches;
let api, started = false, paused = false, ended = false, loading = false;
let lastEvent = -1, lastSteps = 0, lastMessageAt = 0, lastTime = performance.now(), lastHudAt = 0;
let movement = [0, 0], look = [0, 0], held = 0, volume = 0.55, audioContext;
let padPrevious = [];
let releaseGate = null, hadPointerLock = false;
let lastWhoosh = 0, lastImpact = 0, swingNoise;
let lastKnifeAttack = 0, lastKnifeHit = 0;
let lastBlock = 0, lastGuardBreak = 0, lastParry = 0, parryFlashUntil = 0, parryOpenUntil = 0;
const heldSources = new Map();
const ambience = new Audio('./ambience.wav'); ambience.loop = true; ambience.volume = volume;
const voiceAudio = new VoiceAudio(() => audioContext);
const dialogue = new WardenDialogue(voiceAudio, text => {
  $('dialogue-speaker').textContent = 'WARDEN';
  $('dialogue-text').textContent = text || '';
  $('dialogue').hidden = !text;
});
const castleVoiceAudio = new VoiceAudio(() => audioContext, CASTLE_LINES);
const castleDialogue = new CastleDialogue(castleVoiceAudio, CASTLE_LINES, line => {
  $('dialogue-speaker').textContent = line?.speaker || '';
  $('dialogue-text').textContent = line?.text || '';
  $('dialogue').hidden = !line;
});
const castleAmbience = new Audio('./castle-ambience.wav'); castleAmbience.loop = true;
let inCastle = false, lastEnemyAttack = 0, lastEnemyHit = 0, lastCastleSfx = 0;
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
  try {
    audioContext ||= new (window.AudioContext || window.webkitAudioContext)();
    audioContext.resume().then(() => { if (!paused || ended) { voiceAudio.resume(); castleVoiceAudio.resume(); } }).catch(() => {});
    voiceAudio.preload();
    castleVoiceAudio.preload();
  } catch {}
  if (volume) (inCastle ? castleAmbience : ambience).play().catch(() => {});
}
function swordSound(impact, strength = 0.7, surface = 'Body') {
  if (!audioContext || !volume || paused) return;
  const ctx=audioContext, t=ctx.currentTime;
  // Authored contact cues share the same output and pause/mute gate as swings.
  // Paving is stone; soft grass avoids the ringing tail of iron and masonry.
  const profile = ({
    Body: {filter:'lowpass', high:950, low:90, duration:0.24, gain:0.32, bass:92},
    Stone: {filter:'highpass', high:2100, low:440, duration:0.18, gain:0.25, bass:130},
    Paving: {filter:'highpass', high:2100, low:440, duration:0.18, gain:0.25, bass:130},
    Iron: {filter:'bandpass', high:3200, low:1100, duration:0.30, gain:0.18, bass:0, tones:[910,1530,2610]},
    Timber: {filter:'lowpass', high:1500, low:260, duration:0.13, gain:0.25, bass:185, tones:[270,410]},
    Grass: {filter:'lowpass', high:700, low:160, duration:0.16, gain:0.13, bass:0},
  })[surface] || {filter:'lowpass', high:950, low:90, duration:0.24, gain:0.32, bass:92};
  strength = Math.max(0, Math.min(1.5, Number.isFinite(strength) ? strength : 0.7));
  const duration = impact ? profile.duration : 0.27;
  if (!swingNoise) {
    swingNoise=ctx.createBuffer(1,Math.ceil(ctx.sampleRate*0.32),ctx.sampleRate);
    const data=swingNoise.getChannelData(0);
    for(let i=0;i<data.length;i++) data[i]=Math.random()*2-1;
  }
  const noise=ctx.createBufferSource(), filter=ctx.createBiquadFilter(), gain=ctx.createGain();
  noise.buffer=swingNoise;filter.type=impact?profile.filter:'bandpass';filter.Q.value=0.65;
  filter.frequency.setValueAtTime(impact?profile.high:700,t);
  filter.frequency.exponentialRampToValueAtTime(impact?profile.low:140,t+duration*0.85);
  gain.gain.setValueAtTime(0.001,t);
  gain.gain.exponentialRampToValueAtTime(Math.max(0.001,volume*(impact?profile.gain:0.15)*strength),t+0.012);
  gain.gain.exponentialRampToValueAtTime(0.001,t+duration);
  noise.connect(filter).connect(gain).connect(ctx.destination);noise.start(t);noise.stop(t+duration+0.02);
  if(impact && profile.bass) {
    const bass=ctx.createOscillator(), body=ctx.createGain();bass.type='triangle';
    bass.frequency.setValueAtTime(profile.bass,t);bass.frequency.exponentialRampToValueAtTime(profile.bass*0.37,t+duration*0.7);
    body.gain.setValueAtTime(Math.max(0.001,volume*0.20*strength),t);body.gain.exponentialRampToValueAtTime(0.001,t+duration);
    bass.connect(body).connect(ctx.destination);bass.start(t);bass.stop(t+duration+0.02);
  }
  if (impact) for (const [index, frequency] of (profile.tones || []).entries()) {
    const tone=ctx.createOscillator(), ring=ctx.createGain();tone.type='sine';
    tone.frequency.setValueAtTime(frequency,t);
    ring.gain.setValueAtTime(Math.max(0.001,volume*0.07*strength/(index+1)),t);
    ring.gain.exponentialRampToValueAtTime(0.001,t+duration);
    tone.connect(ring).connect(ctx.destination);tone.start(t);tone.stop(t+duration+0.02);
  }
}
const zoneLabels = {Head:'Head',LeftTorso:'Left torso',RightTorso:'Right torso',LeftLeg:'Left leg',RightLeg:'Right leg',TorsoLeft:'Left torso',TorsoRight:'Right torso',LegLeft:'Left leg',LegRight:'Right leg'};
const readable = value => String(value || '').replace(/([a-z])([A-Z])/g, '$1 $2').replaceAll('_', ' ');
function bladeFeedback(state, combat) {
  const reticle = document.querySelector('.crosshair'), focus = document.querySelector('.blade-focus');
  let point = {x:0.5,y:0.5};
  if (state.stage >= 4) {
    // The renderer projects the actual world-space blade aim using its frame
    // camera. Its x coordinate is normalized by vertical FOV, not by aspect.
    const projection = combat?.aimProjection, aspect = innerWidth / innerHeight;
    point = projection && Number.isFinite(projection.x) && Number.isFinite(projection.y) && Number.isFinite(aspect) && aspect > 0
      ? {x:0.5+0.5*projection.x/aspect,y:0.5-0.5*projection.y} : null;
    if (point && (point.x < 0 || point.x > 1 || point.y < 0 || point.y > 1)) point = null;
  }
  const reticleVisible = playing() && !state.finished && point !== null;
  reticle.hidden = !reticleVisible;
  if (point) {
    reticle.style.left = `${point.x*100}%`;reticle.style.top = `${point.y*100}%`;
    // Only the text is kept away from viewport edges; the reticle is never
    // clamped to a point that the sword cannot actually reach.
    const margin = Math.min(90,innerWidth/2);
    focus.style.left = `${Math.max(margin,Math.min(innerWidth-margin,point.x*innerWidth))}px`;
    focus.style.top = `${Math.max(0,Math.min(innerHeight-60,point.y*innerHeight+14))}px`;
  }
  const visible = reticleVisible && state.stage >= 4;
  const aimed = combat?.aimedZone, zone = combat?.impactZone;
  const surface = combat?.impactKind === 'Chain' ? 'Iron' : combat?.impactSurface;
  $('aim-zone').hidden = !visible || !aimed;
  $('aim-zone').textContent = aimed ? `Aim · ${zoneLabels[aimed] || readable(aimed)}` : '';
  const confirmed = visible && combat?.impactLeft > 0 && (zone || surface);
  $('impact-feedback').hidden = !confirmed;
  const target = zone ? zoneLabels[zone] || readable(zone) : surface === 'Timber' ? 'Wood' : surface;
  const label = confirmed ? combat.impactKind === 'Chain' ? 'Chain struck' : zone ? `Hit · ${target}` : `${target} impact` : '';
  if ($('impact-feedback').textContent !== label) $('impact-feedback').textContent = label;
  $('impact-feedback').dataset.surface = zone ? 'Body' : surface || '';
}
function guardSound(broken = false) {
  if (!audioContext || !volume || paused) return;
  const ctx = audioContext, t = ctx.currentTime, duration = broken ? 0.38 : 0.19;
  // Inharmonic short steel partials make a block distinct from a body hit.
  for (const [frequency, weight] of [[940, 1], [1570, 0.55], [2780, 0.3]]) {
    const tone = ctx.createOscillator(), gain = ctx.createGain();
    tone.type = 'sine'; tone.frequency.setValueAtTime(frequency, t);
    tone.frequency.exponentialRampToValueAtTime(frequency * (broken ? 0.42 : 0.87), t + duration);
    gain.gain.setValueAtTime(volume * weight * (broken ? 0.12 : 0.085), t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + duration);
    tone.connect(gain).connect(ctx.destination); tone.start(t); tone.stop(t + duration + 0.02);
  }
  if (broken) swordSound(true, 0.85);
}
function parrySound(deflected = false) {
  if (!audioContext || !volume || paused) return;
  const ctx = audioContext, t = ctx.currentTime, duration = deflected ? 0.17 : 0.26;
  // The same steel as a block, struck clean: brighter partials, slower decay.
  // A cut turned aside is the dull half of that — pitched down and cut short.
  for (const [frequency, weight] of [[1480, 1], [2210, 0.6], [3320, 0.42], [4650, 0.22]]) {
    const tone = ctx.createOscillator(), gain = ctx.createGain();
    tone.type = 'sine'; tone.frequency.setValueAtTime(frequency * (deflected ? 0.62 : 1), t);
    tone.frequency.exponentialRampToValueAtTime(frequency * (deflected ? 0.58 : 0.94), t + duration);
    gain.gain.setValueAtTime(volume * weight * 0.075, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + duration);
    tone.connect(gain).connect(ctx.destination); tone.start(t); tone.stop(t + duration + 0.02);
  }
}
function castleChime(kind) {
  if (!audioContext || !volume || paused) return;
  if (kind === 'gate') { guardSound(true); return; }
  const frequencies = kind === 'bell' ? [147, 294, 397, 602] : kind === 'wolf' ? [220, 330] : [330, 440, 660];
  const duration = kind === 'bell' ? 3.8 : 1.1, now = audioContext.currentTime;
  for (const [index, frequency] of frequencies.entries()) {
    const tone = audioContext.createOscillator(), gain = audioContext.createGain();
    tone.frequency.value = frequency;
    gain.gain.setValueAtTime(0.001, now);
    gain.gain.exponentialRampToValueAtTime(volume * 0.09 / (1 + index), now + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.001, now + duration);
    tone.connect(gain).connect(audioContext.destination); tone.start(now); tone.stop(now + duration + 0.03);
  }
}
function setHeld(source, mask) {
  if (mask) heldSources.set(source, mask); else heldSources.delete(source);
  held = 0;
  for (const value of heldSources.values()) held |= value;
  api?.touch_input(movement[0], movement[1], 0, 0, held);
}
const playing = () => started && !paused && !ended;
// Count actual mouse presses, including a down/up pair between render frames.
$('ember-root').addEventListener('pointerdown',event=>{
  if(event.pointerType==='mouse' && [0,1].includes(event.button) && playing() && !(held & 4) && event.target.tagName==='CANVAS') {
    if(event.button===1) event.preventDefault();
    api?.action(event.button===1 ? 64 : 2);
  }
});
$('ember-root').addEventListener('mousedown', event => {
  if (event.button === 1 && playing() && event.target.tagName === 'CANVAS') event.preventDefault();
  if (event.button === 2 && playing() && event.target.tagName === 'CANVAS') {
    event.preventDefault(); setHeld('mouse-guard', 4);
  }
});
$('ember-root').addEventListener('contextmenu', event => event.preventDefault());
window.addEventListener('mouseup', event => { if (event.button === 2) setHeld('mouse-guard', 0); });
for (const name of ['pointercancel', 'lostpointercapture']) window.addEventListener(name, event => {
  if (event.pointerType === 'mouse') setHeld('mouse-guard', 0);
});
window.addEventListener('mousemove', event => { if (!(event.buttons & 2) && heldSources.has('mouse-guard')) setHeld('mouse-guard', 0); });
function clearTouch() { movement = [0, 0]; look = [0, 0]; held = 0; heldSources.clear(); stickPointer = null; lookPointer = null; $('stick-knob').style.transform = ''; api?.touch_input(0, 0, 0, 0, 0); }
function setPause(value) {
  paused = value; globalThis.__emberInputPaused = value; api?.pause(value); clearTouch();
  dialogue.setPaused(value && !ended);
  castleDialogue.setPaused(value && !ended);
  if (value) { document.exitPointerLock?.(); ambience.pause(); castleAmbience.pause(); }
  else if (started && !ended) { unlockAudio(); $('ember-root').querySelector('canvas')?.focus(); }
  $('touch').hidden = !coarse || value || !started || ended;
}
function openSettings() {
  if ($('settings').open || loading || $('prologue').open) return;
  if (started) setPause(true);
  $('resume').textContent = started ? 'Return to the castle' : 'Return';
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
$('volume').oninput = event => { volume = Number(event.target.value) / 100; voiceAudio.setVolume(volume); castleVoiceAudio.setVolume(volume); ambience.volume = volume; castleAmbience.volume = volume * 0.65; if (volume && started && !paused) unlockAudio(); else if (!volume) { ambience.pause(); castleAmbience.pause(); } };
$('clues-up').onclick = () => $('journal-entries').scrollBy({top:-140});
$('clues-down').onclick = () => $('journal-entries').scrollBy({top:140});
$('fullscreen').onclick = async () => { try { if (document.fullscreenElement) await document.exitFullscreen(); else await $('game').requestFullscreen(); } catch { $('fullscreen').textContent = 'Fullscreen unavailable in this browser'; } };
$('trailer-button').onclick = () => { ambience.pause(); $('prologue').showModal(); $('film').play().catch(() => {}); };
function closeFilm() { $('film').pause(); $('prologue').close(); if (started) setPause(false); }
$('skip-film').onclick = closeFilm; $('film').onended = closeFilm;
$('prologue').addEventListener('cancel', event => { event.preventDefault(); closeFilm(); });
document.addEventListener('keydown', event => {
  if (event.code === 'KeyR' && playing()) {
    event.preventDefault(); if (!event.repeat && !(held & 4)) api?.action(64);
  }
  if (event.code === 'KeyF' && playing()) {
    event.preventDefault(); if (!event.repeat) setHeld('keyboard-guard', 4);
  }
  if (event.repeat && event.code === 'Escape') { event.preventDefault(); return; }
  if (event.code === 'Escape' && started && !ended && !$('prologue').open) { event.preventDefault(); if ($('settings').open) closeSettings(); else openSettings(); }
  if (['Space','AltLeft','ArrowUp','ArrowDown'].includes(event.code) && started && !paused) event.preventDefault();
  if ((event.code === 'Enter' || event.code === 'Space') && !started && !loading && !$('settings').open && !$('prologue').open && event.target === document.body) { event.preventDefault(); begin(); }
});
document.addEventListener('keyup', event => { if (event.code === 'KeyF') setHeld('keyboard-guard', 0); });
window.addEventListener('blur', () => { clearTouch(); if (started && !ended) openSettings(); });
document.addEventListener('visibilitychange', () => { if (document.hidden) { clearTouch(); if (started && !ended) openSettings(); } });
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
    if (!playing() || (event.pointerType === 'mouse' && event.button !== 0)) return;
    event.preventDefault(); button.setPointerCapture(event.pointerId);
    if (button.dataset.action && !([2,64].includes(Number(button.dataset.action)) && (held & 4))) api?.action(Number(button.dataset.action));
    if (button.dataset.hold) setHeld(`touch-${event.pointerId}`, Number(button.dataset.hold));
  });
  for (const name of ['pointerup','pointercancel','lostpointercapture']) button.addEventListener(name, event => { if (button.dataset.hold) setHeld(`touch-${event.pointerId}`, 0); });
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
  dialogue.ingest(state.dialogue, state.time);
  dialogue.tick(ms / 1000);
  castleDialogue.ingest(state.castleEvents);
  castleDialogue.tick(ms / 1000, dialogue.speaking);
  for (const event of state.castleEvents || []) {
    if (event.id <= lastCastleSfx) continue;
    lastCastleSfx = event.id;
    if (state.time - event.time < 0.5 && ['sun','wolf','bell','heal','seal','gate'].includes(event.kind)) castleChime(event.kind);
    if (state.time - event.time < 0.5 && event.kind === 'enemy_parry') parrySound(true);
  }
  const speaking = dialogue.speaking || castleDialogue.speaking;
  ambience.volume = volume * (speaking ? 0.28 : 1);
  castleAmbience.volume = volume * (speaking ? 0.18 : 0.65);
  if (inCastle !== (state.stage >= 5)) {
    inCastle = state.stage >= 5;
    (inCastle ? ambience : castleAmbience).pause();
    if (!paused && !ended && volume) (inCastle ? castleAmbience : ambience).play().catch(() => {});
  }
  const combat=state.combat;
  const warden=state.warden;
  const guard=state.guard;
  bladeFeedback(state, combat);
  if (guard) {
    if (guard.blockEvent !== lastBlock) { lastBlock = guard.blockEvent; if (lastBlock) guardSound(); }
    if (guard.breakEvent !== lastGuardBreak) { lastGuardBreak = guard.breakEvent; if (lastGuardBreak) guardSound(true); }
    if (guard.parryEvent !== lastParry) { lastParry = guard.parryEvent; if (lastParry) { parrySound(); parryFlashUntil = now + 260; } }
    const parried = now < parryFlashUntil, window = guard.parryWindow || 0.12;
    if (guard.parryWindowLeft > 0) parryOpenUntil = now + guard.parryWindowLeft * 1000;
    $('guard-flash').style.opacity = String(Math.min(0.68, Math.max(guard.impactLeft / 0.24 * (parried ? 0.62 : 0.5), guard.brokenLeft / 0.9 * 0.3, (guard.parryWindowLeft || 0) / window * 0.2)));
    $('guard-flash').dataset.broken = String(guard.brokenLeft > 0);
    $('guard-flash').dataset.parry = String(parried);
  }
  if(warden) {
    if(warden.attackEvent===0) lastKnifeAttack=0;
    if(warden.phase==='Attacking' && warden.elapsed>=warden.windup && warden.attackEvent!==lastKnifeAttack) {
      lastKnifeAttack=warden.attackEvent; swordSound(false,0.4);
    }
    if(warden.hitEvent!==lastKnifeHit) {lastKnifeHit=warden.hitEvent;if(lastKnifeHit) swordSound(true,0.6);}
    $('hurt').style.opacity=String(Math.max(0,Math.min(1,Math.max(warden.hitLeft, state.enemyCombat?.hitLeft || 0)/0.24)));
  }
  if (state.enemyCombat) {
    const enemyCombat = state.enemyCombat;
    if (enemyCombat.attackEvent !== lastEnemyAttack) { lastEnemyAttack = enemyCombat.attackEvent; if (lastEnemyAttack) swordSound(false, 0.7); }
    if (enemyCombat.hitEvent !== lastEnemyHit) { lastEnemyHit = enemyCombat.hitEvent; if (lastEnemyHit) swordSound(true, 0.9); }
  }
  // Contact sounds follow simulation frames, independently of the slower HUD.
  if(combat) {
    if(combat.swingEvent===0) lastWhoosh=0;
    if(combat.active && !combat.active.landingWait && !combat.landingWait && combat.active.elapsed>=combat.active.windup && combat.swingEvent!==lastWhoosh) {lastWhoosh=combat.swingEvent;swordSound(false);}
    if(combat.impactEvent!==lastImpact) {lastImpact=combat.impactEvent;if(lastImpact) swordSound(true,combat.impactStrength,combat.impactKind==='Chain'?'Iron':combat.impactZone?'Body':combat.impactSurface||'Body');}
  }
  if (now - lastHudAt < 80) return;
  lastHudAt = now;
  document.querySelector('.top-actions .version').textContent = `VERSION ${state.version}`;
  if (!$('boot').hidden) { $('boot').hidden = true; $('hud').hidden = false; $('touch').hidden = !coarse || paused || ended; }
  $('form').textContent = state.form.toUpperCase(); $('health').value = state.health; $('health-value').textContent = Math.ceil(state.health); $('stamina').value = state.stamina;
  $('objective').textContent = state.objective; $('prompt').textContent = state.prompt;
  $('location').textContent = state.location || 'The basement';
  renderCastle(state, $);
  $('exploration').hidden = state.stage < 5;
  $('exploration').textContent = state.exploration ? `${state.exploration.visited} / ${state.exploration.total} places discovered` : '';
  $('hint').hidden = !state.prompt;
  $('hint').querySelector('kbd').textContent = state.stage === 4 ? (state.pad ? 'R1' : coarse ? 'Strike' : 'LMB') : (state.pad ? '□' : coarse ? 'Use' : 'E');
  const enemyActive=warden && warden.phase!=='Sleeping' && warden.health>0;
  $('alert').hidden = warden ? warden.health<=0 || !warden.near || (state.alert<0.2 && !enemyActive) : state.alert<0.2;
  $('warden-status').textContent = enemyActive ? (warden.phase==='Attacking' && warden.elapsed<warden.contact ? (state.stage >= 4 ? 'Knife raised — guard or dodge' : 'Knife raised — dodge or step away') : warden.label) : 'The warden hears you';
  $('warden-health').hidden = !enemyActive;
  if(warden) $('warden-health').value=warden.health;
  if (state.event !== lastEvent) { lastEvent = state.event; lastMessageAt = now; $('message').textContent = state.message; if(state.stage<4 && !(warden?.hitLeft>0)) sound('event'); }
  if(combat) {
    $('combat-hud').hidden=state.stage<4 || state.finished;
    $('combo-name').textContent=combat.active?combat.label:guard?.amount>0?'Blade held in guard':'Greatsword ready';
    const landingWait = combat.active?.landingWait || combat.landingWait;
    $('combo-cue').textContent=landingWait?'Jump Heavy · waiting for landing':combat.queued?`${combat.queued} follow-up${combat.queued>1?'s':''} buffered`:guard?.amount>0?'Release guard to strike':combat.quickLeft>0?'Keep the three cuts connected':combat.rhythmLeft>0?'Tap now to vary the follow-up':combat.active?`${readable(combat.active.kind)} · ${combat.active.elapsed<combat.active.windup?'winding up':'following through'}`:'Three quick strikes · one connected chain';
    $('rhythm-fill').style.width=`${Math.max(0,Math.min(100,combat.rhythmLeft/0.9*100))}%`;
  }
  if (guard) {
    $('guard-status').hidden = state.stage < 4 || state.finished;
    const parried = now < parryFlashUntil, parryOpen = now < parryOpenUntil;
    $('guard-status').dataset.state = guard.brokenLeft > 0 ? 'broken' : parried || parryOpen ? 'parry' : guard.ready ? 'ready' : 'idle';
    $('guard-label').textContent = guard.brokenLeft > 0 ? 'Guard broken' : parried ? 'Parried' : parryOpen ? 'Parry window open' : guard.impactLeft > 0 ? 'Strike blocked' : guard.ready ? 'Guard ready' : guard.amount > 0 ? 'Raising / lowering guard' : 'Guard down';
    const blockCost = state.enemy?.blockCost ?? 28;
    const guardKey = state.pad ? 'L2' : coarse ? 'Guard' : 'right mouse / F';
    const readyCue = state.enemy?.unblockable ? 'Crushing slam — dodge sideways' : state.stamina < blockCost ? `Low stamina — this strike needs ${blockCost} to block` : `Tap ${guardKey} as it lands to parry · hold to block`;
    $('guard-cue').textContent = guard.brokenLeft > 0 ? 'Recover — dodge or step away'
      : parried ? 'Deflected — counter now'
      : parryOpen && state.stamina >= (guard.parryCost ?? 22) ? `Deflecting — ${Math.round(guard.parryCost ?? 22)} stamina`
      : guard.parryCooldown > 0 ? `Parry spent — guard returns in ${guard.parryCooldown.toFixed(1)}s`
      : guard.ready ? readyCue
      : combat?.active ? 'Guard waits for committed swings'
      : `Tap ${guardKey} to parry · hold to guard`;
  }
  $('message').style.opacity = now - lastMessageAt < 6000 ? '1' : '0';
  if (state.footsteps !== lastSteps) { lastSteps = state.footsteps; if (!state.crouched) sound(); }
  $('shortcuts').textContent = state.pad ? 'R1 strike · R2 heavy · × then R2 jump heavy · Tap L2 parry / hold guard · Options menu' : document.pointerLockElement ? 'LMB strike · R / middle click heavy · Space then heavy · Tap F / right mouse to parry, hold to guard' : 'Click to look · WASD move · LMB strike · R heavy · F guard (tap to parry)';
  const canvas = $('ember-root').querySelector('canvas');
  $('performance').textContent = canvas ? `${canvas.width} × ${canvas.height} · ${quality.fps || '—'} FPS · ${quality.mode === 'auto' ? 'ADAPTIVE' : quality.mode.toUpperCase()}` : '';
  if (state.finished && !ended) { ended = true; dialogue.finish(); castleDialogue.setPaused(true); setPause(true); $('complete').hidden = false; $('hud').hidden = true; $('menu-button').hidden = true; $('play-again').focus(); }
}
requestAnimationFrame(loop);
