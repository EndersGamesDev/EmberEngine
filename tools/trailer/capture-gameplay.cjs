// Record real Killshot gameplay as a numbered JPEG frame sequence, for the trailer.
//
//   node tools/trailer/capture-gameplay.cjs --shot sweep-harbor --out target/trailer/shots
//
// It owns everything it touches: a private loopback web server, a private
// arena-server, and a disposable headless browser. It refuses to start if either
// port is already listening, and it never contacts a public host — the injected
// WebSocket class throws on any URL that is not its own server.
//
// WHY A SCREENCAST AND NOT recordVideo: Playwright's video recorder needs a
// separate ffmpeg binary it wants to download. The Chrome DevTools screencast
// needs nothing, hands back JPEG frames straight from the compositor at the
// browser's real frame rate, and leaves the muxing to whichever ffmpeg we
// choose. Frames are written as f00000.jpg…; assemble them with
// tools/trailer/build-trailer.sh.
//
// OPERATOR SAFETY: like every ember harness, input here is authored DOM events
// dispatched inside the headless document (page.evaluate). No page.keyboard,
// no page.mouse, no OS input, no foreground activation, no real pointer lock or
// fullscreen — those APIs are stubbed. Someone is sitting at this machine.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const net = require('node:net');
const os = require('node:os');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const root = process.cwd();
const webRoot = path.join(root, 'web');
const webPort = Number(process.env.EMBER_TRAILER_WEB_PORT || 8090);
const gamePort = Number(process.env.EMBER_TRAILER_GAME_PORT || 7790);
const proto = 24;
const version = 'v31';
const origin = `http://127.0.0.1:${webPort}`;
const gameUrl = `ws://127.0.0.1:${gamePort}`;
const entry = `${origin}/games/arena/${version}/index.html`;

const argv = process.argv.slice(2);
const flag = (name, fallback) => {
  const at = argv.indexOf(`--${name}`);
  return at === -1 ? fallback : argv[at + 1];
};
const shotName = flag('shot', 'harbor-establish');
const outRoot = path.resolve(flag('out', path.join(root, 'target', 'trailer', 'shots')));
const width = Number(flag('width', 1280));
const height = Number(flag('height', 720));

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const mime = (file) => ({ '.js': 'text/javascript', '.cjs': 'text/javascript', '.wasm': 'application/wasm',
  '.html': 'text/html', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png',
  '.svg': 'image/svg+xml' }[path.extname(file)] || 'application/octet-stream');

let server;
let game;
let browser;
let page;
const report = { shot: shotName, frames: 0, errors: [] };

function listening(port) {
  return new Promise((resolve) => {
    const socket = net.connect({ host: '127.0.0.1', port });
    socket.once('connect', () => { socket.destroy(); resolve(true); });
    socket.once('error', () => { socket.destroy(); resolve(false); });
  });
}

// ---------------------------------------------------------------- the shots
//
// Each shot names the lobby it wants and a timeline. `t` is milliseconds from
// the moment recording starts, so the numbers below read as the edit does.
const SHOTS = {
  // Measured on this client at default sensitivity: 3600 px of authored motion
  // turns 521.4 degrees, so roughly 6.9 px per degree. The prerolls below are
  // written in those pixels because that is what the harness actually sends.
  'harbor-establish': {
    map: 'harbor', mode: 'ffa', loadout: 'custom', weapon: '6', seconds: 6,
    what: 'Breakwater Harbor: a walk along the quay with the gantry cranes on the skyline',
    // The harbour spawn is walled in, and sprinting INTO that wall goes nowhere:
    // travel first on the spawn heading, which is open, and only then turn to
    // the quay. Order matters more than duration here.
    preroll: [
      { t: 0, hold: ['forward', 'sprint'] },
      { t: 4000, release: ['forward', 'sprint'] },
      { t: 4200, look: [2240, 0], over: 1200 },
    ],
    timeline: [
      { t: 0, hold: ['forward'] },
      { t: 0, look: [240, 0], over: 5400 },
      { t: 5400, release: ['forward'] },
    ],
  },
  'yard-sprint': {
    map: 'freight-yard', mode: 'ffa', loadout: 'custom', weapon: '3', seconds: 7,
    what: 'Freight Yard: a sprint down the container corridor into a slide',
    preroll: [{ t: 0, look: [1380, 0], over: 900 }],
    timeline: [
      { t: 0, hold: ['forward', 'sprint'] },
      { t: 0, look: [200, 0], over: 4200 },
      { t: 4400, tap: ['crouch'] },
      { t: 5400, release: ['sprint'] },
      { t: 6400, release: ['forward'] },
    ],
  },
  'breach-12': {
    map: 'trench-city', mode: 'ffa', loadout: 'custom', weapon: '8', seconds: 8,
    what: 'Trench City: the Breach-12 — aim, three shells, then the pump reload',
    preroll: [{ t: 0, look: [2700, 0], over: 1400 }],
    timeline: [
      { t: 0, look: [120, -10], over: 1000 },
      { t: 1100, aim: true },
      { t: 1600, fire: 90 },
      { t: 2500, fire: 90 },
      { t: 3400, fire: 90 },
      { t: 4200, aim: false },
      { t: 4400, tap: ['reload'] },
      { t: 4500, look: [-90, 8], over: 3000 },
    ],
  },
  'blade-and-shield': {
    map: 'trench-city', mode: 'ffa', loadout: 'custom', weapon: '5', seconds: 6,
    what: 'Trench City: the scutum on Q, the Murasama on E',
    preroll: [{ t: 0, look: [2700, 0], over: 1400 }],
    timeline: [
      { t: 0, look: [90, 0], over: 1500 },
      { t: 800, tap: ['shield'] },
      { t: 2600, tap: ['melee'] },
      { t: 3400, look: [-140, 0], over: 2200 },
      { t: 4300, tap: ['melee'] },
    ],
  },
  'smg-yard': {
    map: 'freight-yard', mode: 'ffa', loadout: 'custom', weapon: '2', seconds: 7,
    what: 'Freight Yard: Vityaz fire while strafing, then a reload',
    preroll: [{ t: 0, look: [1380, 0], over: 900 }],
    timeline: [
      { t: 0, hold: ['left'] },
      { t: 0, look: [140, -6], over: 3000 },
      { t: 700, fire: 600 },
      { t: 1900, fire: 500 },
      { t: 2800, release: ['left'] },
      { t: 2900, hold: ['right'] },
      { t: 3300, fire: 900 },
      { t: 4700, tap: ['reload'] },
      { t: 4800, look: [-120, 8], over: 2000 },
      { t: 6500, release: ['right'] },
    ],
  },
};

// An ad-hoc shot assembled from flags, for scouting a map before a take is
// worth writing down:
//   --map harbor --hold forward,sprint --look 3600 --over 8000 --seconds 9
if (flag('map', null)) {
  SHOTS['ad-hoc'] = {
    map: flag('map', 'harbor'),
    mode: flag('mode', 'ffa'),
    loadout: flag('loadout', 'custom'),
    weapon: flag('weapon', '3'),
    seconds: Number(flag('seconds', 8)),
    what: 'ad-hoc scouting take',
    timeline: [
      ...(flag('hold', null) ? [{ t: 0, hold: flag('hold').split(',') }] : []),
      { t: 0, look: [Number(flag('look', 1800)), Number(flag('pitch', 0))], over: Number(flag('over', 7000)) },
    ],
  };
}

// Default bindings, as the settings schema names them.
const KEY = { forward: 'KeyW', back: 'KeyS', left: 'KeyA', right: 'KeyD', jump: 'Space',
  sprint: 'ShiftLeft', crouch: 'KeyC', reload: 'KeyR', shield: 'KeyQ', melee: 'KeyE' };

// -------------------------------------------------------------- the services
async function startServices() {
  for (const port of [webPort, gamePort]) {
    if (await listening(port)) throw new Error(`Port ${port} is occupied; refusing to use or stop an existing service`);
  }
  const versionPkg = path.join(webRoot, 'games', 'arena', version, 'pkg');
  const pkg = fs.existsSync(path.join(versionPkg, 'arena_bg.wasm')) ? versionPkg : path.join(webRoot, 'pkg');
  for (const file of ['arena.js', 'arena_bg.wasm']) {
    assert(fs.existsSync(path.join(pkg, file)), `Missing built ${file} in ${pkg} — run the cargo/wasm-bindgen build first`);
  }
  report.bundle = {
    directory: pkg,
    sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(pkg, 'arena_bg.wasm'))).digest('hex'),
  };

  server = http.createServer((request, response) => {
    let pathname;
    try { pathname = decodeURIComponent(new URL(request.url, origin).pathname); }
    catch { response.writeHead(400); response.end(); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    if (pathname === '/server.json') {
      response.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      response.end(JSON.stringify({ v: 'killshot-trailer', hosts: [{ name: 'killshot-trailer', ws: gameUrl, proto }], mirrors: [] }));
      return;
    }
    const packagePath = `/games/arena/${version}/pkg/`;
    const base = pathname.startsWith(packagePath) ? pkg : webRoot;
    const relative = pathname.startsWith(packagePath) ? pathname.slice(packagePath.length) : `.${pathname}`;
    const file = path.resolve(base, relative);
    if (!file.startsWith(`${base}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      response.writeHead(404); response.end(); return;
    }
    response.writeHead(200, { 'Content-Type': mime(file), 'Cache-Control': 'no-store' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(webPort, '127.0.0.1', resolve); });

  const executable = process.env.EMBER_QA_SERVER
    || path.join(root, 'target', 'release', process.platform === 'win32' ? 'arena-server.exe' : 'arena-server');
  assert(fs.existsSync(executable), `Missing server ${executable}`);
  game = spawn(executable, ['--bind', `127.0.0.1:${gamePort}`, '--name', 'killshot-trailer'],
    { cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  game.on('error', (error) => report.errors.push(String(error)));
  if (game.pid) os.setPriority(game.pid, os.constants.priority.PRIORITY_LOW);
  for (let tries = 0; tries < 80 && !(await listening(gamePort)); tries++) {
    if (game.exitCode !== null) break;
    await sleep(100);
  }
  assert(game.exitCode === null && await listening(gamePort), 'Own arena-server failed to start');
  report.serverPid = game.pid;
}

// Installed only into the disposable headless document: the OS-facing methods
// are stubbed so nothing can reach the operator's desktop, and a few signals are
// exposed so the recorder knows when the client is actually drawing.
function instrument({ gameUrl: url, proto: expected }) {
  localStorage.setItem('ember-server-url-manual', '1');
  localStorage.setItem('ember-server-url', url);
  localStorage.setItem('ember-account', JSON.stringify({ handle: 'trailer', id: 'private-trailer' }));
  Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
  Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
  window.__cap = { joined: null, welcome: null, draws: 0, errors: [], aim: [] };
  let locked = null;
  let fullscreen = null;
  Object.defineProperty(document, 'pointerLockElement', { get: () => locked, configurable: true });
  Object.defineProperty(document, 'fullscreenElement', { get: () => fullscreen, configurable: true });
  Object.defineProperty(document, 'fullscreenEnabled', { get: () => true, configurable: true });
  HTMLElement.prototype.focus = function focusStub() {
    this.dispatchEvent(new FocusEvent('focus'));
    this.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
  };
  window.focus = () => {};
  Element.prototype.setPointerCapture = () => {};
  Element.prototype.releasePointerCapture = () => {};
  Element.prototype.hasPointerCapture = () => false;
  Element.prototype.requestPointerLock = function lockStub() {
    locked = this;
    queueMicrotask(() => document.dispatchEvent(new Event('pointerlockchange')));
    return Promise.resolve();
  };
  document.exitPointerLock = () => {
    locked = null;
    queueMicrotask(() => document.dispatchEvent(new Event('pointerlockchange')));
  };
  Element.prototype.requestFullscreen = function fullscreenStub() {
    fullscreen = this;
    queueMicrotask(() => document.dispatchEvent(new Event('fullscreenchange')));
    return Promise.resolve();
  };
  document.exitFullscreen = () => {
    fullscreen = null;
    queueMicrotask(() => document.dispatchEvent(new Event('fullscreenchange')));
    return Promise.resolve();
  };
  const NativeWebSocket = window.WebSocket;
  window.WebSocket = class PrivateOnlyWebSocket extends NativeWebSocket {
    constructor(target, protocols) {
      if (String(target) !== url) throw new Error(`Non-private socket blocked: ${target}`);
      super(target, ...(protocols === undefined ? [] : [protocols]));
      const send = this.send.bind(this);
      // Record the client's own aim so a take with a dead camera is a reported
      // failure rather than something someone has to notice in the picture.
      this.send = (data) => {
        try {
          const message = JSON.parse(data);
          if (message.t === 'input') { window.__cap.aim.push(Math.atan2(message.az, message.ax)); window.__cap.last = message; }
        } catch { /* not ours */ }
        return send(data);
      };
      this.addEventListener('message', (event) => {
        const message = JSON.parse(event.data);
        if (message.t === 'welcome') {
          window.__cap.welcome = message;
          if (message.proto !== expected) window.__cap.errors.push(`Server protocol ${message.proto}, expected ${expected}`);
        }
        if (message.t === 'game_joined') window.__cap.joined = message;
        if (message.t === 'error') window.__cap.errors.push(message.message);
      });
    }
  };
  for (const method of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
    const original = WebGL2RenderingContext.prototype[method];
    WebGL2RenderingContext.prototype[method] = function counted(...args) {
      window.__cap.draws++;
      return original.apply(this, args);
    };
  }
}

// ----------------------------------------------------------------- the input
const CANVAS = '#ember-root canvas';

async function domClick(selector) {
  await page.evaluate((target) => {
    const element = document.querySelector(target);
    if (!element || element.disabled) throw new Error(`Missing/disabled ${target}`);
    element.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }));
  }, selector);
}

async function key(code, down) {
  await page.evaluate(({ code: c, down: d }) => {
    document.querySelector('#ember-root canvas')
      .dispatchEvent(new KeyboardEvent(d ? 'keydown' : 'keyup', { code: c, bubbles: true, cancelable: true }));
  }, { code, down });
}

async function pointer(button, down) {
  await page.evaluate(({ button: b, down: d }) => {
    document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent(d ? 'pointerdown' : 'pointerup',
      { button: b, buttons: d ? (b === 0 ? 1 : 2) : 0, bubbles: true, cancelable: true, pointerId: 1, isPrimary: true }));
  }, { button, down });
}

// The look event, copied from tools/v31/browser-killshot.cjs because getting it
// wrong is silent: winit reads the motion out of getCoalescedEvents(), and
// Chromium returns [] for an untrusted event, so a plain synthetic mousemove
// turns the camera by exactly zero and records six seconds of a frozen view.
async function motion(dx, dy) {
  await page.evaluate(({ dx: x, dy: y }) => {
    const event = new PointerEvent('pointermove', { bubbles: true, cancelable: true,
      pointerType: 'mouse', pointerId: 1, isPrimary: true, button: -1, buttons: 0, clientX: 410, clientY: 300 });
    Object.defineProperties(event, { movementX: { value: x }, movementY: { value: y },
      getCoalescedEvents: { value: () => [event] } });
    document.querySelector('#ember-root canvas').dispatchEvent(event);
  }, { dx, dy });
}

// A look is spread over its duration in ~16 ms steps so the camera glides
// instead of snapping: one big movementX would teleport the view in one frame.
//
// The deltas are carried as whole pixels with the fraction kept in a remainder.
// A slow pan divides into well under one pixel per step, and a fractional
// movementX is truncated to zero somewhere below winit — which is silent: the
// camera simply never moves, and the take looks deliberate.
async function glide(dx, dy, over) {
  const steps = Math.max(1, Math.round(over / 16));
  let carryX = 0;
  let carryY = 0;
  for (let i = 0; i < steps; i++) {
    carryX += dx / steps;
    carryY += dy / steps;
    const stepX = Math.trunc(carryX);
    const stepY = Math.trunc(carryY);
    if (stepX || stepY) {
      carryX -= stepX;
      carryY -= stepY;
      await motion(stepX, stepY);
    }
    await sleep(16);
  }
}

async function runTimeline(shot) {
  const started = Date.now();
  const held = new Set();
  const pending = [];
  for (const step of shot.timeline) {
    pending.push((async () => {
      await sleep(Math.max(0, step.t - (Date.now() - started)));
      if (step.hold) for (const name of step.hold) { held.add(name); await key(KEY[name], true); }
      if (step.release) for (const name of step.release) { held.delete(name); await key(KEY[name], false); }
      if (step.tap) {
        for (const name of step.tap) { await key(KEY[name], true); await sleep(60); await key(KEY[name], false); }
      }
      if (step.aim !== undefined) await pointer(2, step.aim);
      if (step.fire) { await pointer(0, true); await sleep(step.fire); await pointer(0, false); }
      if (step.look) await glide(step.look[0], step.look[1], step.over || 500);
    })());
  }
  await Promise.all(pending);
  for (const name of held) await key(KEY[name], false);
}

// ------------------------------------------------------------------- capture
async function main() {
  const started = Date.now();
  const shot = SHOTS[shotName];
  assert(shot, `Unknown shot "${shotName}". Known: ${Object.keys(SHOTS).join(', ')}`);
  const outDir = path.join(outRoot, shotName);
  fs.rmSync(outDir, { recursive: true, force: true });
  fs.mkdirSync(outDir, { recursive: true });

  await startServices();
  browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
  });
  page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  await page.addInitScript(instrument, { gameUrl, proto });
  page.on('pageerror', (error) => report.errors.push(String(error)));

  await page.goto(entry);
  await page.waitForFunction(() => window.__cap.welcome, null, { timeout: 60000 });

  await page.evaluate(({ lobby, password, map, mode, loadout, weapon }) => {
    for (const [id, value] of [['lobby-name', lobby], ['lobby-pw', password], ['lobby-map', map],
      ['lobby-mode', mode], ['lobby-loadout', loadout], ['lobby-starting-weapon', weapon]]) {
      const element = document.querySelector(`#${id}`);
      if (!element) continue;
      element.value = value;
      element.dispatchEvent(new Event(element.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }));
    }
  }, { lobby: `trailer-${crypto.randomUUID().slice(0, 8)}`, password: crypto.randomUUID(),
    map: shot.map, mode: shot.mode, loadout: shot.loadout, weapon: shot.weapon });
  await domClick('#btn-create');

  // Wait for a joined match that is genuinely drawing, then let the first
  // frames settle: the opening moments show pop-in that no trailer wants.
  await page.waitForFunction(() => window.__cap.joined && window.__cap.draws > 400, null, { timeout: 60000 });

  // A match BEGINS PAUSED behind the personal-setup dialog, and a paused client
  // still sends input packets — they are simply neutral. Skipping this step
  // therefore does not fail loudly: it records a frozen camera for six seconds.
  // Wait for the paused state rather than sampling it, then resume explicitly.
  await page.waitForFunction(() => window.__emberArenaSettings && window.__emberArenaSettings.paused,
    null, { timeout: 30000 });
  await domClick('#settings-resume');
  await page.waitForFunction(() => !document.querySelector('#arena-settings').open
    && !window.__emberArenaSettings.paused, null, { timeout: 15000 });

  // Gameplay look only reaches the client once the canvas holds the mouse.
  // Resuming usually takes the lock; ask for it only if it did not.
  if (!(await page.evaluate(() => document.pointerLockElement !== null))) {
    await pointer(0, true);
    await pointer(0, false);
  }
  await page.waitForFunction(() => document.pointerLockElement !== null, null, { timeout: 15000 });

  // Trailer framing: drop the page's own navigation and status chrome and let
  // the stage fill the viewport. The game's own HUD stays — LIFE, ammo and the
  // reload timer are the v31 feature, not decoration. This is presentation
  // only; nothing about the running match changes.
  if (flag('chrome', 'hidden') === 'hidden') {
    await page.addStyleTag({ content: `
      header, #status, #fs-row { display: none !important; }
      body { margin: 0 !important; overflow: hidden !important; }
      #game { margin: 0 !important; padding: 0 !important; max-width: none !important; width: 100vw !important; }
      #stage { margin: 0 !important; width: 100vw !important; height: 100vh !important;
               border: 0 !important; border-radius: 0 !important; }
    ` });
    await page.evaluate(() => window.dispatchEvent(new Event('resize')));
    await sleep(600);
  }
  await sleep(1200);
  report.joined = await page.evaluate(() => ({ welcome: window.__cap.welcome, gl: window.__cap.gl }));

  const cdp = await page.context().newCDPSession(page);
  let frames = 0;
  cdp.on('Page.screencastFrame', async (frame) => {
    fs.writeFileSync(path.join(outDir, `f${String(frames++).padStart(5, '0')}.jpg`), Buffer.from(frame.data, 'base64'));
    try { await cdp.send('Page.screencastFrameAck', { sessionId: frame.sessionId }); } catch { /* stopped */ }
  });
  // Framing before film: a spawn faces wherever it faces, and turning to the
  // shot's heading is setup, not content. The preroll runs with the recorder off.
  if (shot.preroll) {
    await runTimeline({ timeline: shot.preroll });
    await sleep(400);
  }
  // Measure the camera over the RECORDED take only: counting the preroll's turn
  // would let a frozen take pass on the strength of its own setup move.
  await page.evaluate(() => { window.__cap.aim.length = 0; });
  await cdp.send('Page.startScreencast', { format: 'jpeg', quality: 92, maxWidth: width, maxHeight: height, everyNthFrame: 1 });

  await runTimeline(shot);
  await sleep(300);
  await cdp.send('Page.stopScreencast');

  const aim = await page.evaluate(() => window.__cap.aim);
  const unwrapped = [];
  let turns = 0;
  for (let i = 0; i < aim.length; i++) {
    if (i) {
      let d = aim[i] - aim[i - 1];
      while (d > Math.PI) d -= 2 * Math.PI;
      while (d < -Math.PI) d += 2 * Math.PI;
      turns += d;
    }
    unwrapped.push(turns);
  }
  report.sampleInput = await page.evaluate(() => window.__cap.last);
  report.aimPackets = aim.length;
  report.yawTravelDegrees = +((Math.max(...unwrapped, 0) - Math.min(...unwrapped, 0)) * 180 / Math.PI).toFixed(1);

  report.frames = frames;
  report.seconds = shot.seconds;
  report.what = shot.what;
  report.elapsedSeconds = (Date.now() - started) / 1000;
  report.errors.push(...(await page.evaluate(() => window.__cap.errors)));
  report.passed = frames > 30 && report.errors.length === 0 && report.yawTravelDegrees > 5;
  fs.writeFileSync(path.join(outDir, 'capture.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report));
}

main()
  .catch((error) => { report.errors.push(String(error && error.stack || error)); console.error(String(error)); process.exitCode = 1; })
  .finally(async () => {
    if (browser) await browser.close().catch(() => {});
    if (game && game.exitCode === null) game.kill();
    if (server) server.close();
  });
