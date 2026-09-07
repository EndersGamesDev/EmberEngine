// Raw, input-driven V3 practice footage. No mocked state, fake clock, gameplay
// mutation, server, live room, desktop input, or final-trailer editing.
// Run only after the rebuilt WASM and the shared browser/GPU are available:
//   node tools/league/trailer-gameplay.cjs [--champ=0..4] [--duration=12]
'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { startPreview } = require('./preview.cjs');

const root = path.resolve(__dirname, '../..');
const out = path.join(root, 'target/league-launch/gameplay');
const names = ['SW4RM', 'EmberKnight', 'The Hallow One', 'Bog Maw', 'Tessera'];
const keys = ['swarm', 'knight', 'hallow', 'maw', 'tessera'];
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const requestedChampion = process.argv.find(arg => arg.startsWith('--champ='))?.split('=')[1];
const duration = Number(process.argv.find(arg => arg.startsWith('--duration='))?.split('=')[1] || 12);
assert(Number.isFinite(duration) && duration >= 10 && duration <= 15, 'Duration must be 10–15 real seconds');
assert(requestedChampion === undefined || /^[0-4]$/.test(requestedChampion), 'Champion must be 0–4');
const champions = requestedChampion === undefined ? [0, 1, 2, 3, 4] : [Number(requestedChampion)];
const started = Date.now();
const report = {
  kind: 'raw-actual-gameplay', gameVersion: 'v3', protocol: 2,
  label: 'Actual input-driven local 3v3 practice; silent raw renderer footage',
  gameplayManipulated: false, snapshotsMocked: false, clockManipulated: false,
  captures: [], errors: [], elapsedSeconds: 0,
};
let browser, preview, page;

function writeJson(file, value) {
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', 'utf8');
}

const state = () => page.evaluate(() => JSON.parse(window.captureWasm.state_json()));
async function click(selector) {
  await page.waitForFunction(selector => {
    const element = document.querySelector(selector);
    return element && !element.disabled && element.getClientRects().length;
  }, selector);
  await page.evaluate(selector => document.querySelector(selector).click(), selector);
}

async function advance(seconds = 0.12) {
  const before = await state();
  assert(before.phase === 'live', 'Practice match ended during capture input');
  await page.waitForFunction(secs => JSON.parse(window.captureWasm.state_json()).secs >= secs,
    before.secs + seconds, { timeout: 15000 });
}

async function key(code) {
  for (const type of ['keydown', 'keyup']) {
    await page.evaluate(({ type, code }) => {
      document.querySelector('#ember-root canvas').dispatchEvent(new KeyboardEvent(type, {
        code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code,
        bubbles: true, cancelable: true,
      }));
    }, { type, code });
    await advance(0.1);
  }
}

// Same fixed camera as scene::camera_for, projected for DOM pointer input.
// The focus estimate is the observed champion position; camera easing can
// introduce a small miss while walking, so attack-move handles initial approach.
function screenPoint(me, x, z, height = 0) {
  const length = Math.hypot(24, 16), tan = Math.tan(20 * Math.PI / 180);
  const dx = x - me.x, dz = z - me.z;
  const depth = length - (24 * height + 16 * dz) / length;
  const vertical = (16 * height - 24 * dz) / length;
  return { xf: .5 + dx / (depth * tan * (16 / 9)) * .5,
    yf: .5 - vertical / (depth * tan) * .5 };
}

async function pointer(point, button = null) {
  assert(point.xf >= .02 && point.xf <= .98 && point.yf >= .02 && point.yf <= .98,
    'Requested target lies outside the visible canvas');
  await page.evaluate(({ point, button }) => {
    const canvas = document.querySelector('#ember-root canvas'), rect = canvas.getBoundingClientRect();
    const init = { bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      clientX: rect.left + rect.width * point.xf, clientY: rect.top + rect.height * point.yf,
      button: -1, buttons: 0 };
    const move = new PointerEvent('pointermove', init);
    Object.defineProperty(move, 'getCoalescedEvents', { value: () => [move] });
    canvas.dispatchEvent(move);
    if (button !== null) canvas.dispatchEvent(new PointerEvent('pointerdown', { ...init, button, buttons: 2 }));
  }, { point, button });
  await advance(.1);
  if (button !== null) {
    await page.evaluate(button => document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent('pointerup', {
      bubbles: true, pointerType: 'mouse', pointerId: 1, button, buttons: 0,
    })), button);
    await advance(.1);
  }
}

function enemies(snapshot) {
  return snapshot.units.filter(u => [0, 1, 2].includes(u[0]) && u[1] !== snapshot.me.team && u[4] > 0)
    .map(u => ({ kind: u[0], team: u[1], x: u[2], z: u[3], hpPercent: u[4], slot: u[5],
      distance: Math.hypot(u[2] - snapshot.me.x, u[3] - snapshot.me.z) }))
    .sort((a, b) => a.distance - b.distance);
}

function inputLog(capture, action, before, extra = {}) {
  capture.inputs.push({ action, wallSeconds: (Date.now() - capture.startedAt) / 1000,
    simulationSeconds: before.secs, ...extra });
}

async function attackMove(capture, snapshot) {
  const aim = { x: Math.min(snapshot.me.x + 13, 12), z: 0 };
  const point = screenPoint(snapshot.me, aim.x, aim.z);
  inputLog(capture, 'KeyA attack-move toward the lane fight', snapshot, { aim, point });
  await pointer(point);
  await key('KeyA');
}

async function rankAvailable(capture, snapshot) {
  const ability = snapshot.me.rk[0] === 0 ? 0 : snapshot.me.rk[1] === 0 ? 1 : null;
  if (snapshot.me.pt < 1 || ability === null) return;
  inputLog(capture, 'click earned skill-point button', snapshot, { ability });
  await click(`#abils [data-abil="${ability}"] .up`);
  await page.waitForFunction(ability => JSON.parse(window.captureWasm.state_json()).me.rk[ability] > 0, ability);
}

async function castAvailable(capture, snapshot) {
  const closest = enemies(snapshot)[0];
  if (!closest || closest.distance > 12 || !snapshot.me.alive) return;
  const ability = [0, 1].find(index => snapshot.me.rk[index] > 0 && snapshot.me.cd[index] <= 0);
  if (ability === undefined) return;
  const def = capture.champion === 2 ? snapshot.units.filter(u => u[0] === 0 && u[1] === snapshot.me.team && u[4] > 0
    && Math.hypot(u[2] - snapshot.me.x, u[3] - snapshot.me.z) < 10).sort((a, b) => a[4] - b[4])[0] : null;
  const aim = def ? { x: def[2], z: def[3] } : closest;
  const point = screenPoint(snapshot.me, aim.x, aim.z);
  if (point.xf < .02 || point.xf > .98 || point.yf < .02 || point.yf > .98) return;
  inputLog(capture, ability === 0 ? 'KeyQ learned ability' : 'KeyW learned ability', snapshot,
    { ability, aim, point, manaBefore: snapshot.me.mn, cooldownBefore: snapshot.me.cd[ability] });
  await pointer(point);
  await key(ability === 0 ? 'KeyQ' : 'KeyW');
  const after = await state();
  capture.inputs.at(-1).accepted = after.me.cd[ability] > 0 || after.me.mn < snapshot.me.mn;
  capture.inputs.at(-1).cooldownAfter = after.me.cd[ability];
}

async function prepareBattle(capture) {
  const timeout = Date.now() + 60000;
  while (Date.now() < timeout) {
    const snapshot = await state();
    capture.preparationState = snapshot;
    assert(snapshot.me.alive, 'Champion died before recording began');
    await rankAvailable(capture, snapshot);
    // Wait safely for the actual first wave, then walk alongside it. Running
    // into three enemy champions before minions spawn produces poor footage.
    if (snapshot.secs < capture.waveFirst + .25) {
      await pause(400);
      continue;
    }
    const closest = enemies(snapshot)[0];
    const visibleMinions = snapshot.units.some(u => [1, 2].includes(u[0]) && u[4] > 0
      && Math.hypot(u[2] - snapshot.me.x, u[3] - snapshot.me.z) <= 16);
    if (closest?.distance <= 10 && visibleMinions) return snapshot;
    await attackMove(capture, snapshot);
    await pause(450);
  }
  throw new Error('No real lane battle with minions reached within 60 wall seconds');
}

async function startRecording() {
  return page.evaluate(() => {
    const canvas = document.querySelector('#ember-root canvas');
    const mimeType = ['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm']
      .find(type => MediaRecorder.isTypeSupported(type));
    if (!mimeType) throw new Error('Browser has no supported WebM encoder');
    const stream = canvas.captureStream(30), chunks = [];
    const recorder = new MediaRecorder(stream, { mimeType, videoBitsPerSecond: 8000000 });
    const recording = { recorder, stream, chunks, mimeType, started: performance.now(), animationFrames: 0,
      running: true, samples: [], stop: null };
    recorder.ondataavailable = event => { if (event.data.size) chunks.push(event.data); };
    recording.stop = new Promise((resolve, reject) => {
      recorder.onstop = async () => {
        try {
          const blob = new Blob(chunks, { type: mimeType }), reader = new FileReader();
          reader.onload = () => resolve({ base64: String(reader.result).split(',')[1], bytes: blob.size,
            mimeType, width: canvas.width, height: canvas.height, animationFrames: recording.animationFrames,
            wallDurationSeconds: (performance.now() - recording.started) / 1000, samples: recording.samples });
          reader.onerror = () => reject(new Error('Unable to read recorded WebM'));
          reader.readAsDataURL(blob);
        } catch (error) { reject(error); }
      };
      recorder.onerror = event => reject(new Error(event.error?.message || 'MediaRecorder failed'));
    });
    const observe = () => {
      if (!recording.running) return;
      recording.animationFrames++;
      requestAnimationFrame(observe);
    };
    recording.timer = setInterval(() => {
      const snapshot = JSON.parse(window.captureWasm.state_json());
      recording.samples.push({ wallSeconds: (performance.now() - recording.started) / 1000, ...snapshot });
    }, 500);
    requestAnimationFrame(observe);
    window.gameplayRecording = recording;
    recorder.start(500);
    return { mimeType, width: canvas.width, height: canvas.height, track: stream.getVideoTracks()[0].getSettings() };
  });
}

async function stopRecording() {
  return page.evaluate(async () => {
    const recording = window.gameplayRecording;
    recording.running = false;
    clearInterval(recording.timer);
    recording.recorder.stop();
    const result = await recording.stop;
    recording.stream.getTracks().forEach(track => track.stop());
    return result;
  });
}

function inspectVideo(file) {
  const ffmpeg = process.env.LEAGUE_FFMPEG || path.join(root, 'target/league-launch/pydeps/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe');
  if (!fs.existsSync(ffmpeg)) return { decoded: false, reason: 'Optional FFmpeg unavailable' };
  const decoded = spawnSync(ffmpeg, ['-hide_banner', '-i', file, '-map', '0:v:0', '-f', 'null', '-'],
    { encoding: 'utf8', windowsHide: true, timeout: 30000 });
  const text = decoded.stderr || '';
  const frames = [...text.matchAll(/frame=\s*(\d+)/g)].at(-1)?.[1];
  assert(decoded.status === 0 && Number(frames) > 30, 'Recorded WebM did not decode into video frames');
  return { decoded: true, frames: Number(frames), decoderOutput: text.trim() };
}

async function captureChampion(champion, bundles) {
  const capture = { champion, name: names[champion], key: keys[champion], kind: report.kind,
    label: report.label, startedAt: Date.now(), inputs: [], errors: [] };
  report.captures.push(capture);
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(20000);
  page.on('pageerror', error => capture.errors.push(error.message));
  page.on('response', response => { if (response.status() >= 400) capture.errors.push(`HTTP ${response.status()}: ${response.url()}`); });
  await page.addInitScript(() => { window.focus = () => {}; Element.prototype.setPointerCapture = () => {}; });
  await page.route('**/v3/pkg/*', route => {
    const name = new URL(route.request().url()).pathname.split('/').at(-1);
    return bundles[name] ? route.fulfill({ body: bundles[name], contentType: name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript' }) : route.abort();
  });
  await page.goto(preview.origin + '/games/league/v3/');
  await page.evaluate(async () => { window.captureWasm = await window.leagueReady; });
  assert(await page.evaluate(() => window.captureWasm.proto_version()) === 2, 'Capture requires rebuilt V3 protocol-2 WASM');
  await click('#btn-practice3');
  await page.waitForFunction(() => {
    const snapshot = JSON.parse(window.captureWasm.state_json());
    return snapshot.connected && snapshot.phase === 'select' && snapshot.roster?.length === 6;
  });
  await click(`#cards [data-c="${champion}"]`);
  await page.waitForFunction(champion => {
    const snapshot = JSON.parse(window.captureWasm.state_json());
    return snapshot.roster?.[0]?.champ === champion && snapshot.roster?.[0]?.picked;
  }, champion);
  await click('#btn-start');
  await page.waitForFunction(() => {
    const snapshot = JSON.parse(window.captureWasm.state_json());
    return snapshot.phase === 'live' && snapshot.me?.alive;
  });
  await page.evaluate(() => document.querySelector('#ember-root canvas').scrollIntoView({ block: 'center' }));
  capture.spawnState = await state();
  capture.waveFirst = await page.evaluate(() => JSON.parse(window.captureWasm.data_json()).rules.waveFirst);
  assert(capture.spawnState.roster.length === 6 && capture.spawnState.me.champ === champion, 'Real 3v3 practice draft required');
  capture.before = await prepareBattle(capture);
  console.log(`Recording ${capture.name} at ${capture.before.secs.toFixed(1)} simulated seconds, x=${capture.before.me.x.toFixed(1)}`);
  capture.video = await startRecording();
  capture.recordingStartedAt = Date.now();
  while (Date.now() - capture.recordingStartedAt < duration * 1000) {
    let snapshot = await state();
    if (snapshot.phase !== 'live') break;
    if (snapshot.me.alive) {
      await rankAvailable(capture, snapshot);
      if (snapshot.me.hp < snapshot.me.mh * .65 && snapshot.me.scd[1] <= 0) {
        inputLog(capture, 'KeyF ordinary equipped Heal spell', snapshot);
        await key('KeyF');
        snapshot = await state();
      }
      const enemy = enemies(snapshot)[0];
      if (snapshot.me.hp < snapshot.me.mh * .45) {
        const aim = { x: Math.max(snapshot.me.x - 9, -58), z: 0 };
        const point = screenPoint(snapshot.me, aim.x, aim.z);
        inputLog(capture, 'right-click retreat toward own fountain', snapshot, { aim, point });
        await pointer(point, 2);
        if (snapshot.me.hp < snapshot.me.mh * .3 && snapshot.me.scd[0] <= 0) {
          inputLog(capture, 'KeyD ordinary equipped Flash retreat', snapshot, { aim });
          await key('KeyD');
        }
      } else if (enemy?.distance <= 8) {
        const point = screenPoint(snapshot.me, enemy.x, enemy.z, 1);
        inputLog(capture, 'right-click visible enemy', snapshot, { target: enemy, point });
        await pointer(point, 2);
      } else {
        await attackMove(capture, snapshot);
      }
      await castAvailable(capture, await state());
    }
    await pause(350);
  }
  const encoded = await stopRecording();
  capture.after = await state();
  const file = path.join(out, `${capture.key}.webm`), bytes = Buffer.from(encoded.base64, 'base64');
  delete encoded.base64;
  fs.writeFileSync(file, bytes);
  capture.video = { ...capture.video, ...encoded, file, bytes: bytes.length, sha256: hash(bytes), ...inspectVideo(file) };
  capture.simulationElapsed = capture.after.secs - capture.before.secs;
  capture.acceptedCasts = capture.inputs.filter(input => input.accepted).length;
  capture.aliveSampleFraction = encoded.samples.filter(sample => sample.me?.alive).length / encoded.samples.length;
  capture.wallElapsedSeconds = (Date.now() - capture.startedAt) / 1000;
  capture.passed = capture.errors.length === 0 && capture.video.wallDurationSeconds >= 10
    && capture.simulationElapsed > 5 && capture.acceptedCasts > 0;
  writeJson(path.join(out, `${capture.key}.json`), capture);
  console.log(JSON.stringify({ name: capture.name, file, duration: capture.video.wallDurationSeconds,
    frames: capture.video.frames, simulationElapsed: capture.simulationElapsed, acceptedCasts: capture.acceptedCasts, passed: capture.passed }));
  await page.close(); page = null;
  assert(capture.passed, `${capture.name} capture lacks live simulation/cast evidence or has browser errors`);
}

async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out, { recursive: true });
  const bundles = Object.fromEntries(['league.js', 'league_bg.wasm'].map(name => [name, fs.readFileSync(path.join(root, 'web/pkg', name))]));
  report.bundles = Object.entries(bundles).map(([name, bytes]) => ({ name, bytes: bytes.length, sha256: hash(bytes) }));
  preview = await startPreview({ gameVersion: 'v3', protocol: 2, online: false, port: 8104 });
  const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
  browser = await chromium.launch({ channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  for (const champion of champions) await captureChampion(champion, bundles);
  report.passed = true;
}

main().catch(error => {
  report.errors.push(error.stack); report.passed = false; process.exitCode = 1; console.error(error);
}).finally(async () => {
  await browser?.close(); await preview?.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  const name = requestedChampion === undefined ? 'results.json' : `results-${keys[Number(requestedChampion)]}.json`;
  writeJson(path.join(out, name), report);
  console.log(`Raw gameplay capture completed in ${report.elapsedSeconds.toFixed(3)} seconds; passed=${report.passed}`);
});
