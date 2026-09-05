// Real Arena WASM + authored rendering-only harbor viewpoints/roster.
// No visible browser, OS input, actual focus/pointer capture, or live server.
// Build/bind separately; this script does not build or edit production code.
// EMBER_QA_TIMING=1 adds 8 warmups + 32 GPU-completed frame-latency samples
// per view. This serializes rendering for measurement, not FPS/GPU-only time.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
const webRoot = path.resolve(process.env.EMBER_QA_WEB_ROOT || 'web');
const output = path.resolve(process.env.EMBER_QA_OUTPUT || 'target/harbor-captures');
const started = Date.now();
const map = 'harbor', arenaHalf = 48, proto = 20;
// Authored capture fixtures, not geometry or spawn-allocation verification.
// Gameplay tests observe the server's real roster in network-harbor.cjs.
const spawns = [[-37, 42], [-37, -42], [-13, 42], [-13, -42], [11, 42], [11, -42], [35, 42], [35, -42]];
const viewpoints = [
  { name: 'overview', eye: [-62, 58, -66], target: [9, 2, 0] },
  { name: 'overview-quay', eye: [45, 32, -43], target: [0, 0, 0] },
  { name: 'central-stacks', eye: [-20, 1.7, -34], target: [9, 2, -10] },
  { name: 'west-warehouse', eye: [-36, 1.7, 24], target: [-36, 2, 0] },
  { name: 'quay-ship', eye: [31, 1.7, -30], target: [62, 12, 4] },
  { name: 'cranes', eye: [21, 3, 34], target: [52, 22, -4] },
  { name: 'quay-cargo', eye: [38, 1.45, -10], target: [0, 1.45, -10] },
  ...spawns.map(([x, z], id) => ({ name: `spawn-${id + 1}`, eye: [x, 1.45, z], target: [0, 1.45, 0] })),
];
const wanted = process.env.EMBER_QA_VIEWS?.split(',');
if (wanted) assert(wanted.length && new Set(wanted).size === wanted.length && wanted.every(name => viewpoints.some(v => v.name === name)), 'EMBER_QA_VIEWS contains unknown or duplicate views');
const views = wanted ? wanted.map(name => viewpoints.find(v => v.name === name)) : viewpoints;
const maxDraws = Number(process.env.EMBER_QA_MAX_DRAWS || 2000);
const shadowDiagnostic = process.env.EMBER_QA_SHADOW_DIAGNOSTIC || '';
const timingOption = process.env.EMBER_QA_TIMING || '';
assert(['', '0', '1'].includes(timingOption), 'EMBER_QA_TIMING must be 0, 1, or unset');
const timingEnabled = timingOption === '1';
const timingWarmups = 8, timingSampleCount = 32;
assert(['', 'dump', 'unshadowed'].includes(shadowDiagnostic), 'Unknown shader diagnostic mode');
assert(Number.isFinite(maxDraws) && maxDraws > 0, 'EMBER_QA_MAX_DRAWS must be positive');
const result = { purpose: 'Real WASM rendering-only fixtures; not authoritative layout, eight-player networking, collision, or performance proof',
  webRoot, output, map, arenaHalf, expectedProtocol: proto, fixtureSpawns: spawns,
  dimensions: { width: 1600, height: 900 }, maxDrawsPerFrame: maxDraws, captures: [], errors: [], warnings: [], harborBuildLogs: [], contactBuildLogs: [] };
if (timingEnabled) {
  result.timing = {
    metric: 'Synchronized update/submission/GPU-completion frame latency; not FPS or GPU-only time',
    clock: 'Original unmodified performance.now, not the fixture virtual clock',
    completion: 'WebGL2 gl.finish after each measured callback batch; GPU drained before warmups',
    excludes: 'RAF scheduling wait, fixture snapshot injection, screenshot/readback, and Playwright transport',
    limitations: 'Serial completion changes normal GPU pipelining; includes CPU update/submission and driver wait. Shared-desktop/Idle scheduling variance remains. Not a throughput benchmark.',
    warmupFramesPerView: timingWarmups, sampleFramesPerView: timingSampleCount,
  };
}
if (shadowDiagnostic) {
  result.shaderDiagnostic = shadowDiagnostic;
  result.purpose = 'ISOLATED SHADER DIAGNOSTIC, NOT A RELEASE CAPTURE: ' + result.purpose;
}
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
let server, browser, page, socket, stream, latestInput, ack = 0, tick = 0;
let current = views[0], observerAlive = true;
const html = '<!doctype html><meta charset="utf-8"><title>Harbor rendering fixture</title>'
  + '<style>html,body{margin:0;background:#171b20;color:white}#ember-root{width:1600px;height:900px}canvas{width:1600px!important;height:900px!important}#status,#scoreboard{display:none}</style>'
  + '<div id="ember-root"><span id="loading">Loading real harbor assets</span></div><div id="status"></div><div id="scoreboard"></div>';
function state() {
  const players = spawns.map(([x, z], id) => ({ id, x, z, y: 0, vy: 0,
    ax: x > 0 ? -1 : 1, az: 0, pitch: 0, hp: 3, score: 0, alive: true,
    crouch: false, shield: false, weapon: id % 7 + 1, ammo: [6, 30, 30, 30, 6, 5, 1][id % 7], reserve: 60,
    reloading: false, ads_fraction: 0, spread: 0.026, recoil_bloom: 0, deaths: 0,
    ack, ack_age_ticks: 0, team: id % 2 }));
  Object.assign(players[0], { x: current.eye[0], y: current.eye[1] - 1.45, z: current.eye[2],
    alive: observerAlive, hp: observerAlive ? 3 : 0, weapon: 1 });
  // A spawn camera replaces that spawn's occupant. Move the displaced
  // fixture player to spawn0, rather than filming through a remote body.
  for (const p of players.slice(1)) {
    if (Math.hypot(p.x - current.eye[0], p.z - current.eye[2]) < 2) {
      [p.x, p.z] = spawns[0];
    }
  }
  return { t: 'state', tick: ++tick, players, bullets: [], pads: [], loot: [], team_score: [0, 0], hill: 255, round_pause: 0 };
}
function installFixture({ shadowDiagnostic, timingEnabled }) {
  Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
  Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
  HTMLElement.prototype.focus = function() {};
  Element.prototype.requestPointerLock = function() { return Promise.resolve(); };
  Element.prototype.setPointerCapture = function() {};
  Element.prototype.releasePointerCapture = function() {};
  const NativeWebSocket = window.WebSocket;
  window.WebSocket = class extends NativeWebSocket {
    constructor(...args) { super(...args); window.__qaSocket = this; }
    send(data) {
      const packet = JSON.parse(data);
      if (packet.t === 'input') window.__qaInput = packet;
      return super.send(data);
    }
  };
  const rawNow = performance.now.bind(performance), rawRaf = window.requestAnimationFrame.bind(window);
  let paused = false, now = 0, budget = 0, queued = false, next = 1, finish;
  let timingPhase = 'off', timingWarmupFrames = 0, timingSamples = [];
  const pending = new Map();
  Object.defineProperty(performance, 'now', { configurable: true, value: () => paused ? now : rawNow() });
  const schedule = () => {
    if (queued || !pending.size || (paused && budget === 0)) return;
    queued = true;
    rawRaf(timestamp => {
      queued = false;
      if (paused && !budget) return;
      if (paused) {
        now += 1000 / 60; budget--;
        const packet = window.__qaPacket;
        if (packet) {
          packet.tick++; packet.players[0].ack = window.__qaInput?.seq || 0;
          window.__qaSocket.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(packet) }));
        }
      }
      const callbacks = [...pending.values()]; pending.clear();
      const timed = timingEnabled && timingPhase !== 'off';
      const drawsBefore = timed ? window.__qaDraws : 0;
      const startedAt = timed ? rawNow() : 0;
      for (const callback of callbacks) callback(paused ? now : timestamp);
      if (timed) {
        // Submission alone is asynchronous. Wait for this context's actual
        // GPU work before stopping the real (never virtual) wall clock.
        window.__qaGl.finish();
        const milliseconds = rawNow() - startedAt;
        const glError = window.__qaGl.getError();
        if (glError !== 0) throw new Error(`WebGL error during timing: ${glError}`);
        const drawsPerFrame = window.__qaDraws - drawsBefore;
        if (drawsPerFrame < 1) throw new Error('Timing callback batch did not render');
        if (timingPhase === 'sample') timingSamples.push({ milliseconds, drawsPerFrame });
        else timingWarmupFrames++;
      }
      if (paused && budget === 0 && finish) { const resolve = finish; finish = undefined; resolve(); }
      schedule();
    });
  };
  window.requestAnimationFrame = callback => { const id = next++; pending.set(id, callback); schedule(); return id; };
  window.cancelAnimationFrame = id => pending.delete(id);
  window.__qaClock = {
    freeze() { now = rawNow(); paused = true; },
    advance(frames) {
      if (!paused || finish || !Number.isInteger(frames) || frames < 1) throw new Error('Invalid frame advance');
      return new Promise(resolve => { budget = frames; finish = resolve; schedule(); });
    },
    read: () => ({ paused, now, budget }),
  };
  if (timingEnabled) {
    const requireIdle = () => {
      if (!paused || budget !== 0 || finish || !window.__qaGl) throw new Error('Timing requires an idle controlled WebGL2 frame');
    };
    window.__qaTiming = {
      warmup() {
        requireIdle();
        if (timingPhase !== 'off') throw new Error('Timing already active');
        window.__qaGl.finish();
        timingWarmupFrames = 0; timingSamples = []; timingPhase = 'warmup';
      },
      sample(expectedWarmups) {
        requireIdle();
        if (timingPhase !== 'warmup' || timingWarmupFrames !== expectedWarmups) throw new Error('Timing warmup count mismatch');
        timingPhase = 'sample';
      },
      stop(expectedSamples) {
        requireIdle();
        if (timingPhase !== 'sample' || timingSamples.length !== expectedSamples) throw new Error('Timing sample count mismatch');
        timingPhase = 'off';
        return { warmupFrames: timingWarmupFrames, samples: timingSamples, virtualClockEnd: now };
      },
    };
  }
  window.__qaDraws = 0;
  window.__qaShaders = [];
  if (shadowDiagnostic) {
    const shaderSource = WebGL2RenderingContext.prototype.shaderSource;
    WebGL2RenderingContext.prototype.shaderSource = function(shader, source) {
      const record = { type: this.getShaderParameter(shader, this.SHADER_TYPE), source, patched: false };
      if (shadowDiagnostic === 'unshadowed') {
        const match = /float\s+(shadow_visibility\w*)\s*\(/.exec(source);
        if (match) {
          const start = source.indexOf('{', match.index);
          let depth = 1, end = start + 1;
          while (end < source.length && depth) {
            if (source[end] === '{') depth++;
            if (source[end] === '}') depth--;
            end++;
          }
          if (start < 0 || depth !== 0) throw new Error('Could not isolate shadow function');
          record.originalShadowFunction = source.slice(match.index, end);
          source = source.slice(0, start + 1) + '\nreturn 1.0;\n' + source.slice(end - 1);
          record.patched = true;
        }
      }
      window.__qaShaders.push(record);
      return shaderSource.call(this, shader, source);
    };
  }
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(type, ...args) {
    const gl = getContext.call(this, type, ...args);
    if (type === 'webgl2' && gl) {
      window.__qaGl = gl;
      const extension = gl.getExtension('WEBGL_debug_renderer_info');
      window.__qaGpu = { version: gl.getParameter(gl.VERSION), renderer: gl.getParameter(extension ? extension.UNMASKED_RENDERER_WEBGL : gl.RENDERER) };
    }
    return gl;
  };
  for (const method of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
    const original = WebGL2RenderingContext.prototype[method];
    WebGL2RenderingContext.prototype[method] = function(...args) { window.__qaDraws++; return original.apply(this, args); };
  }
}
async function advance(frames) {
  await page.evaluate(async frames => {
    let timer;
    try { await Promise.race([window.__qaClock.advance(frames), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Controlled frames stalled')), 20000);
    })]); } finally { clearTimeout(timer); }
  }, frames);
  await page.waitForTimeout(25);
}
async function pointCamera(view) {
  const dx = view.target[0] - view.eye[0], dy = view.target[1] - view.eye[1], dz = view.target[2] - view.eye[2];
  const yaw = Math.atan2(dz, dx), pitch = Math.atan2(dy, Math.hypot(dx, dz));
  for (let iteration = 0; iteration < 3; iteration++) {
    const old = await page.evaluate(() => window.__qaInput);
    assert(old, 'Client did not emit input for camera verification');
    const oldYaw = Math.atan2(old.az, old.ax);
    const deltaYaw = Math.atan2(Math.sin(yaw - oldYaw), Math.cos(yaw - oldYaw));
    await page.evaluate(({ deltaYaw, deltaPitch }) => {
      // Synthetic event delivery to the disposable document only. No focus()
      // call, pointer lock, browser automation input, or operating-system event.
      // The DOM focus event tells winit to accept this test document's motion;
      // it does not focus a window, element, or the operator's desktop.
      const canvas = document.querySelector('canvas');
      canvas.dispatchEvent(new FocusEvent('focus'));
      const motion = new PointerEvent('pointermove', { bubbles: true, pointerId: 1,
        pointerType: 'mouse', isPrimary: true, button: -1, buttons: 0,
        movementX: Math.round(deltaYaw / 0.0026), movementY: Math.round(-deltaPitch / 0.0026) });
      // Chromium's untrusted event has no coalesced samples by default,
      // whereas winit consumes that list. Supply this one synthetic sample.
      Object.defineProperty(motion, 'getCoalescedEvents', { value: () => [motion] });
      window.dispatchEvent(motion);
    }, { deltaYaw, deltaPitch: pitch - old.pitch });
    await advance(6);
  }
  const actual = await page.evaluate(() => window.__qaInput);
  const dot = actual.ax * Math.cos(yaw) + actual.az * Math.sin(yaw);
  assert(dot > 0.9999 && Math.abs(actual.pitch - pitch) < 0.004,
    `Camera aim did not reach ${view.name}: ${JSON.stringify({ actual, yaw, pitch, dot })}`);
  return { desiredYaw: yaw, desiredPitch: pitch, actualInput: actual };
}
async function measureViewTiming() {
  await page.evaluate(() => window.__qaTiming.warmup());
  await advance(timingWarmups);
  await page.evaluate(expected => window.__qaTiming.sample(expected), timingWarmups);
  await advance(timingSampleCount);
  const timing = await page.evaluate(expected => window.__qaTiming.stop(expected), timingSampleCount);
  assert.equal(timing.warmupFrames, timingWarmups);
  assert.equal(timing.samples.length, timingSampleCount);
  for (const sample of timing.samples) {
    assert(Number.isFinite(sample.milliseconds) && sample.milliseconds >= 0, 'Invalid real-clock timing sample');
    assert(sample.drawsPerFrame > 0 && sample.drawsPerFrame <= maxDraws, 'Timing sample exceeded draw bounds');
  }
  const sorted = timing.samples.map(sample => sample.milliseconds).sort((a, b) => a - b);
  timing.milliseconds = {
    min: sorted[0], max: sorted.at(-1),
    median: (sorted[timingSampleCount / 2 - 1] + sorted[timingSampleCount / 2]) / 2,
    p95: sorted[Math.ceil(timingSampleCount * 0.95) - 1],
    mean: sorted.reduce((sum, value) => sum + value, 0) / timingSampleCount,
  };
  timing.percentileMethod = 'Nearest-rank p95; median averages the two middle samples';
  return timing;
}
async function main() {
  fs.mkdirSync(output, { recursive: true });
  const wasm = path.join(webRoot, 'pkg/arena_bg.wasm'), bindings = path.join(webRoot, 'pkg/arena.js');
  result.wasm = { path: wasm, bytes: fs.statSync(wasm).size, sha256: sha(fs.readFileSync(wasm)) };
  result.bindingsSha256 = sha(fs.readFileSync(bindings));
  server = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (pathname === '/fixture') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end(html); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    const file = path.resolve(webRoot, `.${pathname}`);
    if (!file.startsWith(`${webRoot}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) { response.writeHead(404); response.end(); return; }
    response.writeHead(200, { 'Content-Type': file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', 'Cache-Control': 'no-store' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({ ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
    headless: true, args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  const context = await browser.newContext({ viewport: result.dimensions, deviceScaleFactor: 1 });
  page = await context.newPage();
  page.on('pageerror', error => result.errors.push(String(error)));
  page.on('console', message => {
    if (message.type() === 'error') result.errors.push(message.text());
    if (message.type() === 'warning') result.warnings.push(message.text());
    if (message.text().includes('authored harbor built')) result.harborBuildLogs.push(message.text());
    if (message.text().includes('static contact shading baked')) result.contactBuildLogs.push(message.text());
  });
  await page.route('**/*', route => new URL(route.request().url()).origin === origin ? route.continue() : route.abort());
  await page.routeWebSocket('**', ws => {
    if (!ws.url().endsWith('/harbor-fixture')) { result.errors.push('Blocked unexpected WebSocket'); ws.close(); return; }
    socket = ws;
    ws.onMessage(data => {
      const m = JSON.parse(String(data));
      if (m.t === 'hello') {
        result.protocol = m.proto;
        ws.send(JSON.stringify({ t: 'welcome', proto, motd: 'Rendering fixture', host: 'fixture', version: 'fixture', commit: 'fixture' }));
      } else if (m.t === 'create_lobby') {
        ws.send(JSON.stringify({ t: 'game_joined', id: 0, seed: 12, arena_half: arenaHalf, map, mode: 'ffa',
          players: spawns.map((_, id) => ({ id, handle: id ? `harbor-player-${id}` : 'harbor-camera', color: [0.35 + id * 0.06, 0.5, 0.65] })) }));
        socket.send(JSON.stringify(state()));
        stream = setInterval(() => socket?.send(JSON.stringify(state())), 1000 / 30);
      } else if (m.t === 'input') { ack = m.seq; latestInput = m; }
      else if (m.t === 'ping') ws.send(JSON.stringify({ t: 'pong', nonce: m.nonce }));
    });
    ws.onClose(() => { clearInterval(stream); socket = null; });
  });
  await page.addInitScript(installFixture, { shadowDiagnostic, timingEnabled });
  await page.goto(`${origin}/fixture`);
  await page.evaluate(async origin => {
    const arena = await import('/pkg/arena.js'); await arena.default();
    arena.start_online(JSON.stringify({ url: origin.replace('http:', 'ws:') + '/harbor-fixture',
      action: 'create', lobby: 'harbor-fixture', handle: 'harbor-camera', map: 'harbor', mode: 'ffa' }));
  }, origin);
  await page.waitForFunction(() => window.__qaDraws > 100 && /Sidearm/.test(document.querySelector('#status').textContent), null, { timeout: 90000 });
  assert.equal(result.protocol, proto, 'Build the current protocol20 WASM, not the previous release');
  clearInterval(stream);
  await page.evaluate(() => window.__qaClock.freeze());
  for (const view of views) {
    current = view; observerAlive = true;
    await page.evaluate(packet => { window.__qaPacket = packet; }, state());
    await advance(6);
    const camera = await pointCamera(view);
    // Keep the camera at its already-initialized feet and omit its gun. This
    // is an intentional non-gameplay observer fixture, not spectator support.
    observerAlive = false;
    await page.evaluate(packet => { window.__qaPacket = packet; }, state());
    await advance(18);
    const before = await page.evaluate(() => window.__qaDraws);
    await advance(1);
    const detail = await page.evaluate(() => ({ status: document.querySelector('#status').textContent,
      draws: window.__qaDraws, gpu: window.__qaGpu, glError: window.__qaGl.getError(),
      viewport: [...window.__qaGl.getParameter(window.__qaGl.VIEWPORT)], clock: window.__qaClock.read(), packet: window.__qaPacket }));
    detail.drawsPerFrame = detail.draws - before;
    assert(detail.drawsPerFrame > 0 && detail.drawsPerFrame <= maxDraws, `Unbounded/no rendering: ${detail.drawsPerFrame} draws/frame`);
    assert.equal(detail.glError, 0, 'WebGL error at checkpoint');
    assert.equal(detail.packet.players.length, 8);
    assert(detail.packet.players.slice(1).every(p => Math.hypot(p.x - view.eye[0], p.z - view.eye[2]) >= 2),
      'Rendering camera overlaps a synthetic remote body');
    assert(/8 in arena/.test(detail.status), `Client roster did not show eight players: ${detail.status}`);
    const file = path.join(output, `${view.name}.png`), bytes = await page.screenshot({ path: file });
    const capture = { view, camera, file, sha256: sha(bytes), ...detail };
    result.captures.push(capture);
    if (timingEnabled) capture.timing = await measureViewTiming();
    console.log(JSON.stringify({ view: view.name, file, drawsPerFrame: detail.drawsPerFrame, glError: detail.glError,
      ...(capture.timing ? { synchronizedLatencyMs: capture.timing.milliseconds } : {}) }));
  }
  const gallery = await context.newPage();
  await gallery.setContent('<style>body{margin:0;background:#20252b;color:white;font:16px sans-serif}main{display:grid;grid-template-columns:1fr 1fr;gap:8px;padding:8px}figure{margin:0}img{display:block;width:100%}figcaption{padding:6px}</style>'
    + `<h3>${shadowDiagnostic ? 'DIAGNOSTIC ' + shadowDiagnostic + ' · ' : ''}Harbor · real WASM / rendering-only eight-player fixtures</h3><main>`
    + result.captures.map(c => `<figure><img src="data:image/png;base64,${fs.readFileSync(c.file).toString('base64')}"><figcaption>${c.view.name}</figcaption></figure>`).join('') + '</main>');
  await gallery.screenshot({ path: path.join(output, 'sheet-harbor.png'), fullPage: true });
  if (shadowDiagnostic) {
    result.shaderSources = await page.evaluate(() => window.__qaShaders);
    if (shadowDiagnostic === 'unshadowed') assert(result.shaderSources.some(s => s.patched), 'No generated GLSL shadow_visibility function was found');
  }
  result.passed = result.errors.length === 0 && result.captures.length === views.length;
}
main().catch(async error => {
  result.errors.push(String(error));
  if (page) await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
}).finally(async () => {
  clearInterval(stream);
  if (browser) await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  result.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify({ passed: Boolean(result.passed), captures: result.captures.length, elapsedSeconds: result.elapsedSeconds, errors: result.errors, output }));
  if (!result.passed) process.exitCode = 1;
});
