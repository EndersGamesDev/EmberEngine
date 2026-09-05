// Real Arena WASM, deterministic render timing, and synthetic DOM input only.
// No OS input, visible window, pointer capture, live server, or production hook.
// Build/bind the client separately before invoking this script. See README.md.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const root = process.cwd();
const webRoot = path.resolve(process.env.EMBER_QA_WEB_ROOT || path.join(root, 'web'));
const output = path.resolve(process.env.EMBER_QA_OUTPUT || path.join(root, 'target', 'ads-captures'));
const names = ['Sidearm', 'Vityaz', 'AK-47', 'M4', 'Revolver', 'Sniper', 'RPG-7'];
const weapons = process.env.EMBER_QA_WEAPONS ? process.env.EMBER_QA_WEAPONS.split(',').map(Number) : [1, 2, 3, 4, 5, 6, 7];
if (!weapons.length || new Set(weapons).size !== weapons.length || weapons.some(id => !Number.isInteger(id) || id < 1 || id > 7)) {
  throw new Error('EMBER_QA_WEAPONS must be unique IDs from 1..7');
}
// These are the intended v24 raising durations. The JSON records them so an
// older client used for a before-run cannot be mistaken for exact ADS progress.
const raiseSeconds = process.env.EMBER_QA_ADS_SECONDS
  ? process.env.EMBER_QA_ADS_SECONDS.split(',').map(Number) : [0.14, 0.18, 0.24, 0.22, 0.19, 0.45, 0.30];
if (raiseSeconds.length !== 7 || raiseSeconds.some(t => !Number.isFinite(t) || t <= 0 || t > 2)) {
  throw new Error('EMBER_QA_ADS_SECONDS requires seven positive durations, at most 2 seconds');
}
const phases = ['hip', 'raise-mid', 'ads', 'crouch-ads', 'release'];
const lowerSeconds = [0.10, 0.12, 0.14, 0.13, 0.12, 0.20, 0.18];
const baseSpread = [0.026, 0.024, 0.022, 0.019, 0.020, 0.075, 0.027];
const adsSpread = [0.42, 0.40, 0.28, 0.28, 0.30, 0.025, 0.40];
const crouchSpread = [0.70, 0.72, 0.66, 0.66, 0.70, 0.60, 0.75];
const started = Date.now();
const result = {
  purpose: 'Rendering-only real WASM + synthetic in-page input; no live networking, authority, accuracy, or frame-rate proof',
  webRoot, output, dimensions: { width: 1600, height: 900 },
  timing: { stepMs: 1000 / 60, raiseSeconds, lowerSeconds, midpointMeaning: 'Half the supplied raise duration, rounded up to a 60 Hz frame; inspect actual client output' },
  accuracyFixture: { baseSpread, adsSpread, crouchSpread, bloom: 0, moving: false, grounded: true,
    note: 'Authored stationary/no-fire state fixtures, not execution of the authoritative simulation' },
  captures: [], errors: [], warnings: [],
};
let server, browser, page, stream, socket;
let ack = 0, tick = 0;
let current = { weapon: weapons[0], crouch: false };
let latestInput;
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const html = `<!doctype html><meta charset="utf-8"><title>ADS rendering fixture</title>
<style>html,body{margin:0;background:#171b20;color:white}#stage,#ember-root{width:1600px;height:900px;position:relative}canvas{width:1600px!important;height:900px!important}#status,#scoreboard{display:none}#crosshair{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);pointer-events:none;color:#9fe0ff;font:20px sans-serif}</style>
<div id="stage"><div id="ember-root"><span id="loading">Loading real game assets</span></div><div id="crosshair">+</div><div id="scoreboard"></div></div><div id="status"></div>`;

function player(id, overrides = {}) {
  return { id, x: 60, z: 0, y: 0, vy: 0, ax: 1, az: 0, pitch: 0,
    hp: 3, score: 0, alive: true, crouch: false, shield: false, weapon: current.weapon,
    ammo: [6, 30, 30, 30, 6, 5, 1][current.weapon - 1], reserve: 60,
    reloading: false, ads_fraction: 0, spread: baseSpread[current.weapon - 1], recoil_bloom: 0,
    deaths: 0, ack, ack_age_ticks: 0, team: 0, ...overrides };
}
function state() {
  return { t: 'state', tick: tick += 2,
    players: [player(0, { crouch: current.crouch }), player(1, { x: 85, ax: -1, az: 0, weapon: 1 })],
    bullets: [], pads: [], loot: [], team_score: [0, 0], hill: 255, round_pause: 0 };
}
function sendState() { if (socket) socket.send(JSON.stringify(state())); }

// Runs before the client is loaded. The time override is restricted to this
// disposable document. At a checkpoint RAF stops and performance.now stops,
// so screenshot latency cannot advance a midpoint into a fully aimed frame.
function installFixture() {
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
      const value = JSON.parse(data);
      if (value.t === 'input') window.__qaLatestInput = value;
      return super.send(data);
    }
  };
  window.__qaIntent = { aiming: false, crouch: false };
  window.__qaConfigureState = (packet, handling) => {
    window.__qaFixture = { packet, handling };
  };
  const fixtureTick = () => {
    const fixture = window.__qaFixture;
    if (!fixture) return;
    const { packet, handling } = fixture;
    const me = packet.players[0];
    const { aiming, crouch } = window.__qaIntent;
    me.ads_fraction = Math.max(0, Math.min(1, me.ads_fraction + (aiming ? 1 / (60 * handling.raise) : -1 / (60 * handling.lower))));
    me.crouch = crouch;
    me.spread = handling.base * (1 + (handling.ads - 1) * me.ads_fraction) * (crouch ? handling.crouch : 1);
    me.ack = window.__qaLatestInput?.seq || 0;
    packet.tick++;
    // Rendering-only state delivery through the client's real message
    // handler, once per controlled frame. There is no production endpoint.
    window.__qaSocket.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(packet) }));
  };
  const rawNow = performance.now.bind(performance);
  const rawRaf = window.requestAnimationFrame.bind(window);
  let paused = false, now = 0, budget = 0, scheduled = false, next = 1, finish;
  const pending = new Map();
  Object.defineProperty(performance, 'now', { configurable: true, value: () => paused ? now : rawNow() });
  const schedule = () => {
    if (scheduled || !pending.size || (paused && budget === 0)) return;
    scheduled = true;
    rawRaf(timestamp => {
      scheduled = false;
      if (paused && budget === 0) return;
      if (paused) { now += 1000 / 60; budget--; fixtureTick(); }
      const callbacks = [...pending.values()];
      pending.clear();
      for (const callback of callbacks) callback(paused ? now : timestamp);
      if (paused && budget === 0 && finish) { const resolve = finish; finish = undefined; resolve(); }
      schedule();
    });
  };
  window.requestAnimationFrame = callback => { const id = next++; pending.set(id, callback); schedule(); return id; };
  window.cancelAnimationFrame = id => pending.delete(id);
  window.__qaClock = {
    freeze() { now = rawNow(); paused = true; },
    advance(frames) {
      if (!paused || finish || !Number.isInteger(frames) || frames < 1) throw new Error('Invalid clock advance');
      return new Promise(resolve => { budget = frames; finish = resolve; schedule(); });
    },
    read: () => ({ paused, now, remainingFrames: budget }),
  };
  window.__drawCalls = 0;
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(type, ...args) {
    const gl = getContext.call(this, type, ...args);
    if (type === 'webgl2' && gl) {
      const extension = gl.getExtension('WEBGL_debug_renderer_info');
      window.__gpu = { version: gl.getParameter(gl.VERSION), renderer: gl.getParameter(extension ? extension.UNMASKED_RENDERER_WEBGL : gl.RENDERER) };
    }
    return gl;
  };
  for (const method of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
    const original = WebGL2RenderingContext.prototype[method];
    WebGL2RenderingContext.prototype[method] = function(...args) { window.__drawCalls++; return original.apply(this, args); };
  }
}

async function input(aiming, crouch) {
  current.crouch = crouch;
  await page.evaluate(({ aiming, crouch }) => {
    window.__qaIntent = { aiming, crouch };
    const canvas = document.querySelector('canvas');
    canvas.dispatchEvent(new PointerEvent(aiming ? 'pointerdown' : 'pointerup', {
      bubbles: true, cancelable: true, pointerId: 1, pointerType: 'mouse', isPrimary: true,
      button: 2, buttons: aiming ? 2 : 0, clientX: 800, clientY: 450,
    }));
    canvas.dispatchEvent(new KeyboardEvent(crouch ? 'keydown' : 'keyup', {
      bubbles: true, cancelable: true, key: 'c', code: 'KeyC', repeat: false,
    }));
  }, { aiming, crouch });
  // Allow the in-page WebSocket route to deliver its fixed state before the
  // next simulation update. Virtual render time remains paused here.
  await page.waitForTimeout(35);
}

async function advance(frames) {
  const before = await page.evaluate(() => window.__drawCalls);
  await page.evaluate(async frames => {
    let timer;
    try {
      await Promise.race([window.__qaClock.advance(frames), new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Controlled render frames stalled')), 20000);
      })]);
    } finally { clearTimeout(timer); }
  }, frames);
  const after = await page.evaluate(() => window.__drawCalls);
  if (after <= before) throw new Error(`No GPU draws in ${frames} fixture frames`);
  // Outgoing input delivery is asynchronous relative to the last render.
  await page.waitForTimeout(20);
}

async function capture(weapon, phase, timing, expectedInput) {
  const detail = await page.evaluate(() => {
    const crosshair = document.querySelector('#crosshair');
    const style = crosshair && getComputedStyle(crosshair);
    return { status: document.querySelector('#status').textContent, drawCalls: window.__drawCalls,
      gpu: window.__gpu, clock: window.__qaClock.read(), sentState: window.__qaFixture.packet,
      legacyCrosshairVisible: Boolean(crosshair && !crosshair.hidden && style.display !== 'none' && style.visibility !== 'hidden' && style.opacity !== '0') };
  });
  if (!latestInput || latestInput.ads !== expectedInput.ads || latestInput.crouch !== expectedInput.crouch) {
    throw new Error(`Synthetic input did not reach real client in ${weapon}/${phase}: ${JSON.stringify(latestInput)}`);
  }
  const file = path.join(output, `${weapon}-${names[weapon - 1].toLowerCase()}-${phase}.png`);
  const bytes = await page.screenshot({ path: file });
  result.captures.push({ weapon, name: names[weapon - 1], phase, file, sha256: sha(bytes),
    timing, outgoingInput: latestInput, ...detail });
  console.log(JSON.stringify({ weapon, phase, file, inputAds: latestInput.ads, inputCrouch: latestInput.crouch, status: detail.status }));
}

async function smokeNewGames(context, origin) {
  // No report is submitted and no diagnostic benchmark is started. This
  // bounded smoke only checks the integrated page, new WASM exports, and a
  // real completed frame from the lab with matched main/worker bindings.
  page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
  await page.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin === origin) return route.continue();
    errors.push(`Blocked external smoke request: ${url.origin}`);
    return route.abort();
  });
  await page.routeWebSocket('**', ws => { errors.push('Unexpected WebSocket in local lab smoke'); ws.close(); });
  await page.goto(`${origin}/games/what-is-this/v1/index.html`);
  const diagnostic = await page.evaluate(async () => {
    const module = await import('/pkg/what_is_this.js');
    await module.default();
    const scenarios = JSON.parse(module.julibrot_scenario_inventory_json());
    return { stagePresent: Boolean(document.querySelector('[data-stage="stage.julibrot-slide.v1"]')),
      scenarios, reportByteBudget: module.julibrot_report_byte_budget(),
      submitDisabled: document.querySelector('#submit').disabled };
  });
  if (!diagnostic.stagePresent || !diagnostic.scenarios.length || !diagnostic.submitDisabled) {
    throw new Error(`Diagnostic smoke contract failed: ${JSON.stringify(diagnostic)}`);
  }
  await page.screenshot({ path: path.join(output, 'new-game-diagnostic.png'), fullPage: true });
  await page.goto(`${origin}/labs/julibrot/index.html`);
  const atomicViewExport = await page.evaluate(async () => {
    const module = await import('/labs/julibrot/pkg/ember_lab_julibrot.js?v=1');
    return typeof module.app_apply_saved_view === 'function';
  });
  if (!atomicViewExport) throw new Error('Julibrot bindings lack the newly integrated atomic saved-view export');
  await page.waitForFunction(() => {
    const status = document.querySelector('#status');
    return status?.classList.contains('failed') || /showing scene/.test(status?.textContent || '');
  }, null, { timeout: 90000 });
  const lab = await page.evaluate(() => {
    const facts = {};
    for (const term of document.querySelectorAll('#facts-grid dt')) facts[term.textContent] = term.nextElementSibling?.textContent;
    return { status: document.querySelector('#status').textContent, failed: document.querySelector('#status').classList.contains('failed'), facts };
  });
  await page.screenshot({ path: path.join(output, 'new-game-julibrot.png'), fullPage: true });
  result.newGameSmoke = { purpose: 'Page/export/first-frame smoke only; no benchmark or upload', diagnostic, lab, atomicViewExport, errors,
    artifacts: ['pkg/what_is_this.js', 'pkg/what_is_this_bg.wasm', 'labs/julibrot/pkg/ember_lab_julibrot.js', 'labs/julibrot/pkg/ember_lab_julibrot_bg.wasm'].map(relative => {
      const bytes = fs.readFileSync(path.join(webRoot, relative));
      return { relative, bytes: bytes.length, sha256: sha(bytes) };
    }) };
  if (lab.failed || errors.length) throw new Error(`New-game smoke failed: ${JSON.stringify(result.newGameSmoke)}`);
}

async function main() {
  fs.mkdirSync(output, { recursive: true });
  const wasm = path.join(webRoot, 'pkg', 'arena_bg.wasm');
  const bindings = path.join(webRoot, 'pkg', 'arena.js');
  if (!fs.existsSync(wasm) || !fs.existsSync(bindings)) throw new Error('Build Arena WASM and run wasm-bindgen before capturing');
  result.wasm = { path: wasm, sha256: sha(fs.readFileSync(wasm)), bytes: fs.statSync(wasm).size };
  result.bindingsSha256 = sha(fs.readFileSync(bindings));
  server = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (pathname === '/fixture') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end(html); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    const file = path.resolve(webRoot, `.${pathname}`);
    if (!file.startsWith(`${webRoot}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) { response.writeHead(404); response.end(); return; }
    response.writeHead(200, { 'Content-Type': file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.html') ? 'text/html' : file.endsWith('.css') ? 'text/css' : 'text/javascript', 'Cache-Control': 'no-store' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const origin = `http://127.0.0.1:${server.address().port}`;
  browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
    headless: true, args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'],
  });
  const context = await browser.newContext({ viewport: result.dimensions, deviceScaleFactor: 1 });
  page = await context.newPage();
  page.on('pageerror', error => result.errors.push(String(error)));
  page.on('console', message => {
    if (message.type() === 'error') result.errors.push(message.text());
    if (message.type() === 'warning') result.warnings.push(message.text());
  });
  await page.routeWebSocket('**/ads-fixture', ws => {
    socket = ws;
    ws.onMessage(data => {
      const message = JSON.parse(String(data));
      if (message.t === 'hello') {
        result.protocol = message.proto;
        ws.send(JSON.stringify({ t: 'welcome', proto: message.proto, motd: 'Rendering fixture', host: 'ads-render-fixture', version: 'fixture', commit: 'fixture' }));
      } else if (message.t === 'create_lobby') {
        ws.send(JSON.stringify({ t: 'game_joined', id: 0, seed: 12, arena_half: 100, map: 'freight-yard', mode: 'ffa',
          players: [{ id: 0, handle: 'ads-observer', color: [0.4, 0.5, 0.65] }, { id: 1, handle: 'sight-target', color: [0.7, 0.7, 0.7] }] }));
        sendState();
        stream = setInterval(sendState, 1000 / 30);
      } else if (message.t === 'input') { ack = message.seq; latestInput = message; }
      else if (message.t === 'ping') ws.send(JSON.stringify({ t: 'pong', nonce: message.nonce }));
    });
    ws.onClose(() => { clearInterval(stream); socket = null; });
  });
  await page.addInitScript(installFixture);
  await page.goto(`${origin}/fixture`);
  await page.evaluate(async origin => {
    const arena = await import('/pkg/arena.js');
    await arena.default();
    arena.start_online(JSON.stringify({ url: origin.replace('http:', 'ws:') + '/ads-fixture',
      action: 'create', lobby: 'ads-fixture', handle: 'ads-observer', map: 'freight-yard', mode: 'ffa' }));
  }, origin);
  await page.waitForFunction(() => window.__drawCalls > 100 && /Sidearm|Vityaz|AK-47|M4|Revolver|Sniper|RPG-7/.test(document.querySelector('#status').textContent), null, { timeout: 90000 });
  clearInterval(stream);
  await page.evaluate(() => window.__qaClock.freeze());
  for (const weapon of weapons) {
    current = { weapon, crouch: false };
    await page.evaluate(({ packet, handling }) => window.__qaConfigureState(packet, handling), {
      packet: state(), handling: { raise: raiseSeconds[weapon - 1], lower: lowerSeconds[weapon - 1],
        base: baseSpread[weapon - 1], ads: adsSpread[weapon - 1], crouch: crouchSpread[weapon - 1] },
    });
    await input(false, false);
    await advance(60);
    await capture(weapon, 'hip', { framesSinceInput: 60 }, { ads: false, crouch: false });
    const midpoint = Math.ceil(raiseSeconds[weapon - 1] * 0.5 * 60);
    await input(true, false);
    await advance(midpoint);
    await capture(weapon, 'raise-mid', { framesSinceInput: midpoint, elapsedMs: midpoint * 1000 / 60, intendedRaiseSeconds: raiseSeconds[weapon - 1] }, { ads: true, crouch: false });
    await advance(90 - midpoint);
    await capture(weapon, 'ads', { framesSinceInput: 90 }, { ads: true, crouch: false });
    await input(true, true);
    await advance(45);
    await capture(weapon, 'crouch-ads', { crouchFrames: 45, aimingFrames: 135 }, { ads: true, crouch: true });
    await input(false, false);
    await advance(60);
    await capture(weapon, 'release', { framesSinceInput: 60 }, { ads: false, crouch: false });
  }
  const gallery = await context.newPage();
  for (const phase of phases) {
    const shots = result.captures.filter(item => item.phase === phase);
    await gallery.setContent(`<style>body{margin:0;background:#20252b;color:white;font:16px sans-serif}main{display:grid;grid-template-columns:1fr 1fr;gap:8px;padding:8px}figure{margin:0}img{display:block;width:100%}figcaption{padding:6px}</style><h3>${phase} · real WASM / synthetic DOM fixture</h3><main>${shots.map(item => `<figure><img src="data:image/png;base64,${fs.readFileSync(item.file).toString('base64')}"><figcaption>${item.weapon} · ${item.name}</figcaption></figure>`).join('')}</main>`);
    await gallery.screenshot({ path: path.join(output, `sheet-${phase}.png`), fullPage: true });
  }
  if (process.env.EMBER_QA_NEW_GAME_SMOKE === '1') {
    clearInterval(stream);
    await gallery.close();
    await page.close();
    await smokeNewGames(context, origin);
  }
  result.passed = result.errors.length === 0 && result.captures.length === weapons.length * phases.length;
}

main().catch(async error => {
  result.errors.push(String(error));
  console.error(error);
  if (page) await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
}).finally(async () => {
  clearInterval(stream);
  if (browser) await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  result.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ captures: result.captures.length, errors: result.errors, elapsedSeconds: result.elapsedSeconds, passed: Boolean(result.passed) }));
  if (!result.passed) process.exitCode = 1;
});
