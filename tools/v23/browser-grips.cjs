// Rendering-only grip fixtures through the real browser client and its assets.
// No keyboard, mouse, pointer, foreground APIs, or real game-server connections.
// After wasm-bindgen has populated web/pkg, run from the repository root:
// EMBER_QA_PLAYWRIGHT=<module path> node tools/v23/browser-grips.cjs
// Optional: EMBER_QA_WEB_ROOT=<other checkout/web>, EMBER_QA_OUTPUT=<directory>,
// EMBER_QA_BROWSER=<Chromium executable>, EMBER_QA_WEAPONS=1,3,7,
// EMBER_QA_VIEWS=first-person,front,side,crouch,aim-up,aim-down,shield.
// Reported states are deterministic visual fixtures, not gameplay/network proof.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const crypto = require('node:crypto');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const root = process.cwd();
const webRoot = path.resolve(process.env.EMBER_QA_WEB_ROOT || path.join(root, 'web'));
const output = path.resolve(process.env.EMBER_QA_OUTPUT || path.join(root, 'target', 'grip-captures'));
const names = ['Sidearm', 'Vityaz', 'AK-47', 'M4', 'Revolver', 'Sniper', 'RPG-7'];
const weapons = process.env.EMBER_QA_WEAPONS
  ? process.env.EMBER_QA_WEAPONS.split(',').map(Number) : [1, 2, 3, 4, 5, 6, 7];
if (weapons.some(id => !Number.isInteger(id) || id < 1 || id > 7)) throw new Error('Weapon IDs must be in 1..7');
const defaultViews = ['first-person', 'front', 'side', 'crouch', 'aim-up'];
const allViews = [...defaultViews, 'aim-down', 'shield'];
const views = process.env.EMBER_QA_VIEWS ? process.env.EMBER_QA_VIEWS.split(',') : defaultViews;
if (views.includes('first-person-shield')) throw new Error('First-person shield is local input state, not a protocol fixture; use the native EMBER_SCRIPT capture workflow');
if (views.some(view => !allViews.includes(view))) throw new Error('Unknown fixture view');
const started = Date.now();
const result = {
  purpose: 'Rendering-only protocol fixtures; not gameplay or live networking verification',
  webRoot, output, captures: [], errors: [], warnings: [],
  dimensions: { width: 1600, height: 900 },
  fixture: { map: 'freight-yard', observer: [60, 0, 0], remoteDistance: 2.15 },
};
let webServer;
let browser;
let capturePage;
let stream;
let socket;
let ack = 0;
let tick = 0;
let current = { weapon: weapons[0], view: 'first-person' };

const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const html = `<!doctype html><meta charset="utf-8"><title>Grip rendering fixture</title>
<style>html,body{margin:0;background:#171b20;color:white}#ember-root{width:1600px;height:900px}canvas{width:1600px!important;height:900px!important}#status,#scoreboard{display:none}</style>
<div id="ember-root"><span id="loading">Loading real game assets</span></div><div id="status"></div><div id="scoreboard"></div>`;

function player(id, weapon, overrides = {}) {
  // Beyond the radius-44 backdrop ring, on the far ground, so a facade
  // cannot enclose the camera or shadow the contact points being reviewed.
  return { id, x: 60, z: 0, y: 0, vy: 0, ax: 1, az: 0, pitch: 0,
    hp: 3, score: 0, alive: true, crouch: false, shield: false, weapon,
    ammo: [8, 30, 30, 30, 6, 5, 1][weapon - 1], reserve: 60, reloading: false, deaths: 0,
    ack, ack_age_ticks: 0, team: 0, ...overrides };
}

function fixture() {
  const ownView = current.view === 'first-person';
  const shield = current.weapon === 1 && current.view.includes('shield');
  const me = player(0, current.weapon, { alive: ownView, hp: ownView ? 3 : 0, shield: ownView && shield });
  const other = player(1, current.weapon, {
    x: 62.15, alive: !ownView,
    ax: current.view === 'front' ? -1 : 0,
    az: current.view === 'front' ? 0 : -1,
    crouch: current.view === 'crouch',
    pitch: current.view === 'aim-up' ? 0.9 : current.view === 'aim-down' ? -0.9 : 0,
    shield: !ownView && shield,
  });
  return { t: 'state', tick: tick += 2, players: [me, other], bullets: [],
    pads: [], loot: [], team_score: [0, 0], hill: 255, round_pause: 0 };
}

function sendState() {
  if (socket) socket.send(JSON.stringify(fixture()));
}

async function main() {
  fs.mkdirSync(output, { recursive: true });
  const wasm = path.join(webRoot, 'pkg', 'arena_bg.wasm');
  const bindings = path.join(webRoot, 'pkg', 'arena.js');
  if (!fs.existsSync(wasm) || !fs.existsSync(bindings)) throw new Error('Build arena WASM and run wasm-bindgen before capturing');
  result.wasm = { path: wasm, sha256: sha(fs.readFileSync(wasm)), bytes: fs.statSync(wasm).size };
  result.bindingsSha256 = sha(fs.readFileSync(bindings));
  webServer = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    if (pathname === '/fixture') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end(html); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    const file = path.resolve(webRoot, `.${pathname}`);
    if (!file.startsWith(`${webRoot}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      response.writeHead(404); response.end(); return;
    }
    response.writeHead(200, { 'Content-Type': file.endsWith('.wasm') ? 'application/wasm' : 'text/javascript', 'Cache-Control': 'no-store' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise((resolve, reject) => { webServer.once('error', reject); webServer.listen(0, '127.0.0.1', resolve); });
  const origin = `http://127.0.0.1:${webServer.address().port}`;
  browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
    headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'],
  });
  const context = await browser.newContext({ viewport: result.dimensions, deviceScaleFactor: 1 });
  const page = await context.newPage();
  capturePage = page;
  page.on('pageerror', error => result.errors.push(String(error)));
  page.on('console', message => {
    if (message.type() === 'error') result.errors.push(message.text());
    if (message.type() === 'warning') result.warnings.push(message.text());
  });
  // Browser-level routing supplies only this fixture socket. It never calls
  // connectToServer: no running server, production endpoint or login is used.
  await page.routeWebSocket('**/grip-fixture', ws => {
    socket = ws;
    ws.onMessage(data => {
      const message = JSON.parse(String(data));
      if (message.t === 'hello') {
        result.protocol = message.proto;
        ws.send(JSON.stringify({ t: 'welcome', proto: message.proto, motd: 'Rendering fixture', host: 'grip-render-fixture', version: 'fixture', commit: 'fixture' }));
      } else if (message.t === 'create_lobby') {
        ws.send(JSON.stringify({ t: 'game_joined', id: 0, seed: 12, arena_half: 80,
          map: 'freight-yard', mode: 'ffa', players: [
            { id: 0, handle: 'fixture-observer', color: [0.4, 0.5, 0.65] },
            { id: 1, handle: 'grip-subject', color: [0.7, 0.7, 0.7] },
          ] }));
        sendState();
        console.log(`Fixture connected on protocol ${result.protocol}`);
        stream = setInterval(sendState, 1000 / 30);
      } else if (message.t === 'input') {
        ack = message.seq;
      } else if (message.t === 'ping') {
        ws.send(JSON.stringify({ t: 'pong', nonce: message.nonce }));
      }
    });
    ws.onClose(() => { clearInterval(stream); socket = null; });
  });
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
    // The harness never queries the operator's real gamepad or produces input.
    Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
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
      WebGL2RenderingContext.prototype[method] = function(...args) {
        window.__drawCalls++;
        return original.apply(this, args);
      };
    }
  });
  await page.goto(`${origin}/fixture`);
  await page.evaluate(async origin => {
    const arena = await import('/pkg/arena.js');
    await arena.default();
    arena.start_online(JSON.stringify({ url: origin.replace('http:', 'ws:') + '/grip-fixture',
      action: 'create', lobby: 'grip-fixture', handle: 'fixture-observer', map: 'freight-yard', mode: 'ffa' }));
  }, origin);
  await page.waitForFunction(() => window.__drawCalls > 100 && /Sidearm|Vityaz|AK-47|M4|Revolver|Sniper|RPG-7/.test(document.querySelector('#status').textContent), null, { timeout: 90000 });
  // The first-person frames precede the remote views, so the observer's
  // standing eye is initialized before its dead/non-viewmodel fixture state.
  for (const view of views) {
    for (const weapon of weapons) {
      // Shield is a sidearm-only gameplay state. Do not manufacture an
      // impossible rifle/shield pose when a broad weapon selection is used.
      if (view.includes('shield') && weapon !== 1) continue;
      current = { weapon, view };
      sendState();
      const before = await page.evaluate(() => window.__drawCalls);
      await page.waitForTimeout(1100);
      await page.waitForFunction(previous => window.__drawCalls > previous + 100, before, { timeout: 30000 });
      const file = path.join(output, `${weapon}-${names[weapon - 1].toLowerCase()}-${view}.png`);
      const bytes = await page.locator('canvas').screenshot({ path: file });
      const detail = await page.evaluate(() => ({ status: document.querySelector('#status').textContent, drawCalls: window.__drawCalls, gpu: window.__gpu }));
      const item = { weapon, name: names[weapon - 1], view, file, sha256: sha(bytes), state: fixture(), ...detail };
      result.captures.push(item);
      console.log(JSON.stringify({ weapon, view, file, status: detail.status }));
    }
  }
  // A separate static HTML contact sheet, assembled from the captures. It is
  // generated evidence, not a replacement for inspection at full resolution.
  const gallery = await context.newPage();
  for (const view of views) {
    const shots = result.captures.filter(item => item.view === view);
    await gallery.setContent(`<style>body{margin:0;background:#20252b;color:white;font:16px sans-serif}main{display:grid;grid-template-columns:1fr 1fr;gap:8px;padding:8px}figure{margin:0}img{display:block;width:100%}figcaption{padding:6px}</style><h3>${view} · rendering-only fixture</h3><main>${shots.map(item => `<figure><img src="data:image/png;base64,${fs.readFileSync(item.file).toString('base64')}"><figcaption>${item.weapon} · ${item.name}</figcaption></figure>`).join('')}</main>`);
    await gallery.screenshot({ path: path.join(output, `sheet-${view}.png`), fullPage: true });
  }
  const expectedCaptures = views.reduce((sum, view) => sum + (view.includes('shield') ? Number(weapons.includes(1)) : weapons.length), 0);
  result.passed = result.errors.length === 0 && result.captures.length === expectedCaptures;
}

main().catch(async error => {
  result.errors.push(String(error));
  console.error(error);
  if (capturePage) {
    result.failureState = await capturePage.evaluate(() => ({ status: document.querySelector('#status')?.textContent, drawCalls: window.__drawCalls, gpu: window.__gpu })).catch(String);
    await capturePage.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
  }
}).finally(async () => {
  clearInterval(stream);
  if (browser) await browser.close();
  if (webServer) await new Promise(resolve => webServer.close(resolve));
  result.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ captures: result.captures.length, errors: result.errors, elapsedSeconds: result.elapsedSeconds, passed: Boolean(result.passed) }));
  if (!result.passed) process.exitCode = 1;
});
