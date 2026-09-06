// Actual v31 page + WASM + an owned, private loopback arena-server (protocol 24).
// Run from the repository root AFTER building and wasm-bindgen:
// EMBER_QA_PLAYWRIGHT=<absolute module> node tools/v31/browser-killshot.cjs
// Optional EMBER_QA_BROWSER, EMBER_QA_SERVER, EMBER_QA_OUTPUT; ports8088/7788 must be free.
// --shotgun-only runs the focused v31 custom8/HUD/remap/reload smoke. Default
// runs the inherited controls/seven-weapon suite and then the shotgun smoke.
// Only page.evaluate dispatches synthetic DOM events. No page.keyboard/mouse/click,
// OS input, foreground activation, real pointer capture or fullscreen is used.
// Limits: focus/capture/gamepad are stubbed; this does not prove trusted user-gesture
// permission behavior, native controls UI, real controller hardware or multiplayer combat.
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

const root = process.cwd(), webRoot = path.join(root, 'web');
const output = path.resolve(process.env.EMBER_QA_OUTPUT || path.join(root, 'target', 'killshot-v31-browser'));
const webPort = 8088, gamePort = 7788, proto = 24;
const origin = `http://127.0.0.1:${webPort}`, gameUrl = `ws://127.0.0.1:${gamePort}`;
const entry = `${origin}/games/arena/v31/index.html`;
const started = Date.now();
const shotgunOnly = process.argv.includes('--shotgun-only');
assert(process.argv.slice(2).every(argument => argument === '--shotgun-only'), 'Unknown browser test argument');
const report = { passed: false, proto, checks: [], errors: [], warnings: [], serverLog: [], screenshots: [], limits: [
  'Synthetic DOM events only; trusted pointer-lock/fullscreen permission is deliberately stubbed.',
  'Fullscreen state and refusal handling are tested with DOM stubs, not actual fullscreen layout or browser permissions.',
  'One rendered WASM player plus a passive protocol peer on a disposable private server; no claim about remote-player combat or trusted devices.',
  'No physical gamepad or native settings GUI coverage.',
] };
let game, server, browser, page;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const check = (condition, name, details = {}) => {
  assert(condition, name); report.checks.push({ name, ...details });
  console.log(`PASS ${name}`);
};
function listening(port) {
  return new Promise(resolve => {
    const socket = net.connect({ host: '127.0.0.1', port });
    socket.once('connect', () => { socket.destroy(); resolve(true); });
    socket.once('error', () => { socket.destroy(); resolve(false); });
  });
}
const mime = file => ({ '.js': 'text/javascript', '.cjs': 'text/javascript', '.wasm': 'application/wasm',
  '.html': 'text/html', '.json': 'application/json', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml' }[path.extname(file)] || 'application/octet-stream');

async function startServices() {
  fs.mkdirSync(output, { recursive: true });
  for (const port of [webPort, gamePort]) {
    if (await listening(port)) throw new Error(`Port ${port} is occupied; refusing to use or stop an existing service`);
  }
  const versionPkg = path.join(webRoot, 'games', 'arena', 'v31', 'pkg');
  const pkg = fs.existsSync(path.join(versionPkg, 'arena_bg.wasm')) ? versionPkg : path.join(webRoot, 'pkg');
  for (const file of ['arena.js', 'arena_bg.wasm']) assert(fs.existsSync(path.join(pkg, file)), `Missing built ${file} in ${pkg}`);
  report.bundle = { directory: pkg, sha256: crypto.createHash('sha256').update(fs.readFileSync(path.join(pkg, 'arena_bg.wasm'))).digest('hex') };
  server = http.createServer((request, response) => {
    let pathname;
    try { pathname = decodeURIComponent(new URL(request.url, origin).pathname); }
    catch { response.writeHead(400); response.end(); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    if (pathname === '/server.json') {
      response.writeHead(200, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      response.end(JSON.stringify({ v: 'killshot-private-qa', hosts: [{ name: 'killshot-qa', ws: gameUrl, proto }], mirrors: [] })); return;
    }
    const packagePath = '/games/arena/v31/pkg/';
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
  const executable = process.env.EMBER_QA_SERVER || path.join(root, 'target', 'release', process.platform === 'win32' ? 'arena-server.exe' : 'arena-server');
  assert(fs.existsSync(executable), `Missing server ${executable}`);
  game = spawn(executable, ['--bind', `127.0.0.1:${gamePort}`, '--name', 'killshot-qa'], {
    cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  game.stdout.on('data', data => report.serverLog.push(String(data)));
  game.stderr.on('data', data => report.serverLog.push(String(data)));
  game.on('error', error => { report.serverError = String(error); });
  if (game.pid) os.setPriority(game.pid, os.constants.priority.PRIORITY_LOW);
  for (let tries = 0; tries < 80 && !await listening(gamePort); tries++) {
    if (game.exitCode !== null || report.serverError) break;
    await sleep(100);
  }
  assert(game.exitCode === null && await listening(gamePort), `Own server failed: ${report.serverError || report.serverLog.join('')}`);
  report.serverPid = game.pid;
}

// This is installed only into the disposable headless browser document. Native
// DOM event routing remains real; devices and OS-facing methods never run.
function instrument({ gameUrl, proto }) {
  localStorage.setItem('ember-server-url-manual', '1');
  localStorage.setItem('ember-server-url', gameUrl);
  localStorage.setItem('ember-account', JSON.stringify({ handle: 'killshot-qa', id: 'private-killshot-qa' }));
  Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
  Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
  window.__qa = { inputs: [], requests: [], states: 0, stateFrames: [], shots: 0, joined: null, welcome: null, draws: 0,
    sockets: [], focusCalls: 0, captureCalls: 0, fullscreenCalls: 0, fullscreenExitCalls: 0,
    fullscreenSupported: true, rejectFullscreen: false, errors: [] };
  let locked = null, fullscreen = null;
  Object.defineProperty(document, 'pointerLockElement', { get: () => locked, configurable: true });
  Object.defineProperty(document, 'fullscreenElement', { get: () => fullscreen, configurable: true });
  Object.defineProperty(document, 'fullscreenEnabled', { get: () => window.__qa.fullscreenSupported, configurable: true });
  HTMLElement.prototype.focus = function() {
    window.__qa.focusCalls++;
    this.dispatchEvent(new FocusEvent('focus'));
    this.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
  };
  window.focus = () => { window.__qa.focusCalls++; };
  Element.prototype.setPointerCapture = () => {};
  Element.prototype.releasePointerCapture = () => {};
  Element.prototype.hasPointerCapture = () => false;
  Element.prototype.requestPointerLock = function() {
    window.__qa.captureCalls++; locked = this;
    queueMicrotask(() => document.dispatchEvent(new Event('pointerlockchange')));
    return Promise.resolve();
  };
  document.exitPointerLock = () => {
    locked = null; queueMicrotask(() => document.dispatchEvent(new Event('pointerlockchange')));
  };
  Element.prototype.requestFullscreen = function() {
    window.__qa.fullscreenCalls++;
    if (window.__qa.rejectFullscreen) return Promise.reject(new DOMException('Synthetic fullscreen rejection.', 'NotAllowedError'));
    fullscreen = this;
    queueMicrotask(() => document.dispatchEvent(new Event('fullscreenchange')));
    return Promise.resolve();
  };
  document.exitFullscreen = () => {
    window.__qa.fullscreenExitCalls++; fullscreen = null;
    queueMicrotask(() => document.dispatchEvent(new Event('fullscreenchange')));
    return Promise.resolve();
  };
  window.__qa.externalFullscreenExit = () => {
    fullscreen = null;
    document.dispatchEvent(new Event('fullscreenchange'));
  };
  const NativeWebSocket = window.WebSocket;
  window.WebSocket = class ObservedWebSocket extends NativeWebSocket {
    constructor(url, protocols) {
      if (String(url) !== gameUrl) throw new Error(`Non-private socket blocked: ${url}`);
      super(url, ...(protocols === undefined ? [] : [protocols]));
      window.__qa.sockets.push(String(url));
      this.addEventListener('message', event => {
        const message = JSON.parse(event.data);
        if (message.t === 'welcome') {
          window.__qa.welcome = message;
          if (message.proto !== proto) window.__qa.errors.push(`Server protocol ${message.proto}, expected ${proto}`);
        }
        if (message.t === 'game_joined') window.__qa.joined = message;
        if (message.t === 'state') {
          window.__qa.states++;
          const mine = message.players.find(player => player.id === window.__qa.joined?.id);
          if (mine) {
            window.__qa.stateFrames.push({ tick: message.tick, at: performance.now(), ...mine });
            if (window.__qa.stateFrames.length > 4000) window.__qa.stateFrames.shift();
          }
        }
        if (message.t === 'shot') window.__qa.shots++;
        if (message.t === 'error') window.__qa.errors.push(message.message);
      });
    }
    send(data) {
      const message = JSON.parse(data);
      if (message.t === 'input') window.__qa.inputs.push({ ...message, at: performance.now() });
      else window.__qa.requests.push(message);
      return super.send(data);
    }
  };
  const getContext = HTMLCanvasElement.prototype.getContext;
  HTMLCanvasElement.prototype.getContext = function(type, ...args) {
    const context = getContext.call(this, type, ...args);
    if (type === 'webgl2' && context) {
      const debug = context.getExtension('WEBGL_debug_renderer_info');
      window.__qa.gl = { version: context.getParameter(context.VERSION),
        renderer: context.getParameter(debug ? debug.UNMASKED_RENDERER_WEBGL : context.RENDERER) };
    }
    return context;
  };
  for (const method of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
    const original = WebGL2RenderingContext.prototype[method];
    WebGL2RenderingContext.prototype[method] = function(...args) { window.__qa.draws++; return original.apply(this, args); };
  }
}

async function domClick(selector) {
  await page.evaluate(selector => {
    const target = document.querySelector(selector);
    if (!target || target.disabled) throw new Error(`Missing/disabled ${selector}`);
    target.dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }));
  }, selector);
}
async function frames(count = 2) {
  await page.evaluate(count => new Promise(resolve => {
    const step = () => { if (--count <= 0) resolve(); else requestAnimationFrame(step); };
    requestAnimationFrame(step);
  }), count);
}
async function key(code, down, target = '#ember-root canvas', repeat = false) {
  await page.evaluate(({ code, down, target, repeat }) => {
    const element = document.querySelector(target) || document.body;
    element.dispatchEvent(new KeyboardEvent(down ? 'keydown' : 'keyup', {
      code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code === 'Space' ? ' ' : code,
      bubbles: true, cancelable: true, repeat,
    }));
  }, { code, down, target, repeat });
}
async function pointer(button, down, target = '#ember-root canvas') {
  await page.evaluate(({ button, down, target }) => {
    const element = document.querySelector(target);
    if (!element) throw new Error(`Missing pointer target ${target}`);
    element.dispatchEvent(new PointerEvent(down ? 'pointerdown' : 'pointerup', {
      bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      button, buttons: down ? [1, 4, 2][button] : 0, clientX: 400, clientY: 300,
    }));
  }, { button, down, target });
}
async function motion(dx, dy = 0) {
  await page.evaluate(({ dx, dy }) => {
    const event = new PointerEvent('pointermove', { bubbles: true, cancelable: true,
      pointerType: 'mouse', pointerId: 1, isPrimary: true, button: -1, buttons: 0, clientX: 410, clientY: 300 });
    Object.defineProperties(event, { movementX: { value: dx }, movementY: { value: dy },
      // Chromium returns [] for an untrusted event by default; give winit its
      // single authored event instead of accidentally testing zero movement.
      getCoalescedEvents: { value: () => [event] } });
    document.querySelector('#ember-root canvas').dispatchEvent(event);
  }, { dx, dy });
}
const config = () => page.evaluate(() => JSON.parse(JSON.stringify(window.__emberArenaSettings)));
async function slider(value) {
  await page.evaluate(value => {
    const slider = document.querySelector('#arena-sensitivity');
    slider.value = String(value); slider.dispatchEvent(new Event('input', { bubbles: true }));
  }, value);
}
const actionSelector = label => `.binding-button[aria-label^="Change ${label}:"]`;
async function rebind(label, code) {
  await domClick(actionSelector(label));
  await key(code, true, '#arena-settings'); await key(code, false, '#arena-settings');
}
async function openMenu(queuedKeys = []) {
  const mark = await page.evaluate(queuedKeys => {
    const canvas = document.querySelector('#ember-root canvas');
    for (const code of queuedKeys) canvas.dispatchEvent(new KeyboardEvent('keydown', { code,
      key: code === 'Space' ? ' ' : code, bubbles: true, cancelable: true }));
    // Sample and dispatch Escape in one JS turn, so an earlier normal send
    // cannot race into the interval labelled "paused" across protocol roundtrips.
    const mark = window.__qa.inputs.length;
    for (const type of ['keydown', 'keyup']) canvas.dispatchEvent(new KeyboardEvent(type,
      { code: 'Escape', key: 'Escape', bubbles: true, cancelable: true }));
    return mark;
  }, queuedKeys);
  await page.waitForFunction(() => document.querySelector('#arena-settings').open && window.__emberInputPaused);
  await frames();
  return mark;
}
async function resume() {
  await domClick('#settings-resume');
  await page.waitForFunction(() => !document.querySelector('#arena-settings').open && !window.__emberArenaSettings.paused);
  await frames(3);
}
async function index() { return page.evaluate(() => window.__qa.inputs.length); }
async function packetsAfter(start, minimum = 4) {
  await page.waitForFunction(({ start, minimum }) => window.__qa.inputs.length >= start + minimum, { start, minimum }, { timeout: 10000 });
  return page.evaluate(start => window.__qa.inputs.slice(start), start);
}
const moving = input => Math.hypot(input.mx, input.my) > 0.1;
const neutral = input => Math.abs(input.mx) < 1e-7 && Math.abs(input.my) < 1e-7
  && ['fire', 'ads', 'sprint', 'crouch', 'reload', 'jump', 'shield', 'melee'].every(name => input[name] === false)
  && (input.select_slot || 0) === 0;
const yaw = input => Math.atan2(input.az, input.ax);
const angleDelta = (a, b) => Math.atan2(Math.sin(a - b), Math.cos(a - b));
async function shot(name) {
  const file = path.join(output, `${name}.png`);
  const bytes = await page.screenshot({ path: file });
  report.screenshots.push({ file, sha256: crypto.createHash('sha256').update(bytes).digest('hex') });
}

const fullscreenSnapshot = () => page.evaluate(() => ({
  state: document.querySelector('#settings-fullscreen-state')?.textContent.trim(),
  help: document.querySelector('#settings-fullscreen-help')?.textContent.trim(),
  disabled: document.querySelector('#settings-fullscreen')?.disabled,
  wholeDocument: document.fullscreenElement === document.documentElement,
  active: Boolean(document.fullscreenElement),
  menu: document.querySelector('#arena-settings').open,
  paused: window.__emberArenaSettings.paused,
  inputPaused: window.__emberInputPaused,
  bindings: JSON.stringify(window.__emberArenaSettings.bindings),
  captures: window.__qa.captureCalls,
  enters: window.__qa.fullscreenCalls,
  exits: window.__qa.fullscreenExitCalls,
}));
async function waitFullscreen(state) {
  await page.waitForFunction(state => document.querySelector('#settings-fullscreen-state')?.textContent.trim() === state,
    state, { timeout: 5000 });
}
async function fullscreenMenu(phase) {
  const before = await fullscreenSnapshot();
  check(before.state === 'Windowed' && !before.active && !before.disabled,
    `${phase}: menu fullscreen control starts supported and windowed`);
  // Use the complete authored pointer gesture while a binding capture is armed.
  // A click-only test would miss an input router that rebinds on pointerdown.
  await domClick(actionSelector('Move forward'));
  await pointer(0, true, '#settings-fullscreen');
  await pointer(0, false, '#settings-fullscreen');
  await domClick('#settings-fullscreen');
  await waitFullscreen('Fullscreen');
  let state = await fullscreenSnapshot();
  check(state.wholeDocument && state.enters === before.enters + 1 && state.menu
    && state.paused && state.inputPaused && state.captures === before.captures
    && state.bindings === before.bindings,
  `${phase}: fullscreen pointer gesture enters the whole document without rebinding or resuming`, state);
  // Either retaining or cancelling the pending capture is acceptable; make the
  // remainder of this fixture independent of that UI choice.
  await key('Escape', true, '#arena-settings'); await key('Escape', false, '#arena-settings');
  await domClick('#settings-fullscreen'); await waitFullscreen('Windowed');
  state = await fullscreenSnapshot();
  check(!state.active && state.exits === before.exits + 1 && state.menu
    && state.paused && state.inputPaused && state.captures === before.captures,
  `${phase}: menu fullscreen exit keeps gameplay paused`);

  await domClick('#settings-fullscreen'); await waitFullscreen('Fullscreen');
  const exits = (await fullscreenSnapshot()).exits;
  await page.evaluate(() => window.__qa.externalFullscreenExit());
  await waitFullscreen('Windowed');
  state = await fullscreenSnapshot();
  check(!state.active && state.exits === exits && state.menu && state.paused
    && state.inputPaused && state.captures === before.captures,
  `${phase}: external fullscreen exit synchronizes status without recapturing`);

  const helpBefore = state.help;
  await page.evaluate(() => { window.__qa.rejectFullscreen = true; });
  await domClick('#settings-fullscreen');
  await page.waitForFunction(previous => {
    const text = document.querySelector('#settings-fullscreen-help')?.textContent.trim();
    return text && text !== previous;
  }, helpBefore, { timeout: 5000 });
  state = await fullscreenSnapshot();
  check(state.state === 'Windowed' && !state.active && state.menu && state.paused && state.inputPaused
    && state.captures === before.captures && /refus|deni|could not|fail|block|not allow/i.test(state.help),
  `${phase}: rejected fullscreen request reports an error without resuming`, { help: state.help });
  await page.evaluate(() => { window.__qa.rejectFullscreen = false; });
  await page.evaluate(() => { window.__qa.fullscreenSupported = false; window.__qa.externalFullscreenExit(); });
  state = await fullscreenSnapshot();
  check(state.disabled && state.menu && state.paused && /unavailable/i.test(state.help),
    `${phase}: unsupported fullscreen is disabled with a browser fallback hint`);
  await page.evaluate(() => { window.__qa.fullscreenSupported = true; window.__qa.externalFullscreenExit(); });
}

async function fullscreenSharedButton() {
  const before = await fullscreenSnapshot();
  check(!before.menu && !before.paused && !before.active, 'shared fullscreen test begins in active windowed gameplay');
  await domClick('#btn-fullscreen'); await waitFullscreen('Fullscreen');
  let state = await fullscreenSnapshot();
  check(state.wholeDocument && !state.menu && !state.paused && !state.inputPaused
    && state.captures === before.captures, 'in-match shared button uses the same fullscreen state without changing controls');
  await domClick('#btn-fullscreen'); await waitFullscreen('Windowed');
  state = await fullscreenSnapshot();
  check(!state.active && !state.menu && !state.paused && !state.inputPaused
    && state.captures === before.captures, 'in-match shared button exits fullscreen without changing controls');
}

async function preferencesBeforeMatch() {
  await page.goto(entry);
  await page.waitForFunction(() => window.__qa.welcome && document.querySelector('#host-chip').textContent.includes('killshot-qa'), null, { timeout: 60000 });
  await domClick('#btn-settings');
  check(await page.evaluate(() => document.querySelector('#arena-settings').open && window.__emberInputPaused), 'settings opens before a match');
  await fullscreenMenu('before match');
  await slider(2);
  await rebind('Move forward', 'KeyS');
  check((await config()).bindings.forward[0] === 'KeyW' && /already used/.test(await page.locator('#settings-feedback').textContent()), 'duplicate assignment rejected');
  await key('F11', true, '#arena-settings'); await key('F11', false, '#arena-settings');
  check((await config()).bindings.forward[0] === 'KeyW' && /reserved|unsupported/.test(await page.locator('#settings-feedback').textContent()), 'reserved key rejected');
  await key('Escape', true, '#arena-settings'); await key('Escape', false, '#arena-settings');
  await rebind('Move forward', 'KeyI');
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('ember-killshot-settings-v1')));
  check(saved.sensitivity === 2 && saved.bindings.forward[0] === 'KeyI', 'preferences save through real menu');
  await page.reload();
  await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  check((await config()).sensitivity === 2 && (await config()).bindings.forward[0] === 'KeyI', 'preferences survive page reload');
  await domClick('#btn-settings');
  await page.setViewportSize({ width: 390, height: 844 });
  await frames();
  const layout = await page.evaluate(() => {
    const dialog = document.querySelector('#arena-settings'), bounds = dialog.getBoundingClientRect();
    const content = dialog.querySelector('.settings-inner');
    const resume = document.querySelector('#settings-resume').getBoundingClientRect();
    return { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom,
      width: innerWidth, height: innerHeight, scroll: content.scrollHeight > content.clientHeight,
      overflow: getComputedStyle(content).overflowY, resumeTop: resume.top, resumeBottom: resume.bottom };
  });
  check(layout.left >= 0 && layout.right <= layout.width + 1 && layout.top >= 0 && layout.bottom <= layout.height + 1
    && layout.resumeTop >= layout.top && layout.resumeBottom <= layout.bottom
    && (!layout.scroll || ['auto', 'scroll'].includes(layout.overflow)), '390px settings dialog fits, scrolls and keeps Resume visible', layout);
  const display = await page.evaluate(() => {
    const button = document.querySelector('#settings-fullscreen'), state = document.querySelector('#settings-fullscreen-state');
    const bounds = button.getBoundingClientRect(), stateBounds = state.getBoundingClientRect();
    const content = document.querySelector('#arena-settings .settings-inner').getBoundingClientRect();
    return { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom,
      contentTop: content.top, contentBottom: content.bottom, width: bounds.width, height: bounds.height,
      stateTop: stateBounds.top, stateBottom: stateBounds.bottom, stateHeight: stateBounds.height,
      hidden: getComputedStyle(button).visibility === 'hidden' || getComputedStyle(button).display === 'none' };
  });
  check(!display.hidden && display.width > 0 && display.height > 0 && display.stateHeight > 0
    && display.left >= layout.left && display.right <= layout.right
    && display.top >= display.contentTop && display.bottom <= display.contentBottom
    && display.stateTop >= display.contentTop && display.stateBottom <= display.contentBottom,
  '390px fullscreen control and current state are visible in the mobile settings card', display);
  await shot('settings-mobile');
  await page.setViewportSize({ width: 1600, height: 900 });
  await domClick('#settings-reset');
  check((await config()).sensitivity === 1 && (await config()).bindings.forward[0] === 'KeyW', 'restore defaults works');
  await shot('settings-before-match');
  await domClick('#settings-resume'); await frames(3);
}

async function joinPrivateMatch() {
  await page.evaluate(({ lobby, password }) => {
    for (const [id, value] of [['lobby-name', lobby], ['lobby-pw', password], ['lobby-map', 'freight-yard']]) {
      const element = document.getElementById(id); element.value = value;
      element.dispatchEvent(new Event('input', { bubbles: true })); element.dispatchEvent(new Event('change', { bubbles: true }));
    }
  }, { lobby: `controls-${Date.now().toString(36)}`, password: crypto.randomUUID() });
  await domClick('#btn-create');
  await page.waitForFunction(() => window.__qa.joined && window.__qa.draws > 40
    && !document.querySelector('#settings-resume').disabled, null, { timeout: 60000 });
  const details = await page.evaluate(() => ({ welcome: window.__qa.welcome, gl: window.__qa.gl, joined: window.__qa.joined }));
  check(details.welcome.proto === proto && details.gl && details.joined, 'actual WASM joins owned protocol24 server and draws WebGL2', details);
  check((await config()).paused, 'match begins paused until explicit Resume');
  const pausedMark = await index();
  await fullscreenMenu('in match');
  check((await packetsAfter(pausedMark)).every(neutral), 'fullscreen menu interactions keep every actual outgoing action neutral');
  await resume();
  await fullscreenSharedButton();
  const mark = await index(); await key('KeyW', true);
  check((await packetsAfter(mark)).some(moving), 'default W produces actual movement packets');
  await key('KeyW', false); await packetsAfter(await index());
}

async function lookTrial() {
  const prior = (await packetsAfter(await index(), 3)).at(-1);
  const mark = await index(); await motion(10);
  const after = (await packetsAfter(mark, 4)).at(-1);
  return angleDelta(yaw(after), yaw(prior));
}

async function pauseSafety() {
  let mark = await index();
  await key('KeyW', true); await pointer(0, true);
  const held = await packetsAfter(mark, 5);
  check(held.some(input => moving(input) && input.fire), 'precondition held W and left mouse reach real client');
  // Queue edge intents immediately before Escape, with no wait for a send tick.
  mark = await openMenu(['Space', 'KeyE', 'KeyQ', 'KeyR', 'KeyC', 'ShiftLeft']);
  const before = await page.evaluate(() => ({ states: window.__qa.states, draws: window.__qa.draws,
    captures: window.__qa.captureCalls, input: window.__qa.inputs.at(-1) }));
  await motion(30, 15);
  await pointer(2, true); await pointer(2, false);
  const paused = await packetsAfter(mark, 9);
  const after = await page.evaluate(() => ({ states: window.__qa.states, draws: window.__qa.draws, captures: window.__qa.captureCalls }));
  check(paused.every(neutral), 'menu neutralizes every outgoing action and pending edge', { packets: paused.length });
  check(paused.every(input => Math.abs(angleDelta(yaw(input), yaw(before.input))) < 1e-6 && Math.abs(input.pitch - before.input.pitch) < 1e-6), 'mouse look stays fixed while menu open');
  check(after.states > before.states && after.draws > before.draws, 'server snapshots and rendering continue while paused', { before, after });
  check(after.captures === before.captures, 'menu interaction never recaptures pointer');
  await shot('settings-in-match');
  await resume();
  mark = await index();
  await key('KeyW', true, '#ember-root canvas', true); // old held key repeat, not a new physical press
  const resumed = await packetsAfter(mark, 5);
  check(resumed.every(neutral), 'explicit Resume has no stuck W, fire, or queued edges', { packets: resumed.length });
  for (const code of ['KeyW', 'Space', 'KeyE', 'KeyQ', 'KeyR', 'KeyC', 'ShiftLeft']) await key(code, false);
  await pointer(0, false);
  await packetsAfter(await index(), 3);
}

async function remappedPlay(baseLook) {
  await openMenu(); await slider(2); await rebind('Move forward', 'KeyI');
  await domClick(actionSelector('Fire')); await pointer(1, true, '#arena-settings'); await pointer(1, false, '#arena-settings');
  await frames(3); await resume();
  let mark = await index(); await key('KeyW', true);
  check((await packetsAfter(mark)).every(input => !moving(input)), 'old W binding stops moving after remap');
  await key('KeyW', false); mark = await index(); await key('KeyI', true);
  check((await packetsAfter(mark)).some(moving), 'new I binding moves through actual outgoing packets');
  await key('KeyI', false); await packetsAfter(await index(), 3);
  const doubleLook = await lookTrial();
  check(Math.abs(baseLook - 0.026) < 0.001 && Math.abs(doubleLook / baseLook - 2) < 0.03,
    'same authored 10px mouse delta produces double yaw at sensitivity2', { baseLook, doubleLook, ratio: doubleLook / baseLook });
  mark = await index(); await pointer(0, true);
  check((await packetsAfter(mark)).every(input => !input.fire), 'old left mouse binding no longer fires');
  await pointer(0, false); mark = await index(); await pointer(1, true);
  check((await packetsAfter(mark)).some(input => input.fire), 'middle mouse mapping produces actual fire intent');
  await pointer(1, false); await packetsAfter(await index(), 3);
  report.traffic = await page.evaluate(() => ({ inputs: window.__qa.inputs, states: window.__qa.states, stateFrames: window.__qa.stateFrames, shots: window.__qa.shots,
    draws: window.__qa.draws, gl: window.__qa.gl, sockets: window.__qa.sockets, errors: window.__qa.errors }));
  check(report.traffic.errors.length === 0, 'real-server traffic contains no protocol or application errors');
  check(report.traffic.stateFrames.length > 10 && report.traffic.stateFrames.every(state =>
    state.parkour && Object.values(state.parkour).every(value =>
      typeof value !== 'number' || Number.isFinite(value))),
  'actual authoritative snapshots carry finite protocol24 parkour state');
  report.parkourCoverage = 'Wire state and normal movement/settings routing only; use network-parkour.cjs for eight-player authored-route assertions.';
  await page.reload(); await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  const persisted = await config();
  check(persisted.sensitivity === 2 && persisted.bindings.forward[0] === 'KeyI' && persisted.bindings.fire[0] === 'Mouse1', 'in-match remaps and sensitivity survive reload');
  await domClick('#btn-settings'); await domClick('#settings-reset');
  await page.reload(); await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  const reset = await config();
  check(reset.sensitivity === 1 && reset.bindings.forward[0] === 'KeyW' && reset.bindings.fire[0] === 'Mouse0', 'restored defaults also persist across reload');
}

const currentState = () => page.evaluate(() => window.__qa.stateFrames.at(-1));
async function waitState(predicate, argument) {
  await page.waitForFunction(({ predicate, argument }) => {
    const state = window.__qa.stateFrames.at(-1);
    return state && new Function('state', 'argument', `return (${predicate})(state, argument)`)(state, argument);
  }, { predicate: predicate.toString(), argument }, { timeout: 10000 });
  return currentState();
}
async function tap(code) {
  const mark = await index(); await key(code, true);
  await packetsAfter(mark, 2); await key(code, false); await frames(2);
}
async function createLoadoutMatch(weapon, loadout = 'custom') {
  const credentials = { lobby: `v31-${weapon}-${crypto.randomUUID().slice(0, 8)}`, password: crypto.randomUUID() };
  await page.reload();
  await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  await page.evaluate(({ lobby, password, weapon, loadout }) => {
    for (const [id, value] of [['lobby-name', lobby], ['lobby-pw', password], ['lobby-map', 'harbor'],
      ['lobby-mode', 'ffa'], ['lobby-loadout', loadout], ['lobby-starting-weapon', String(weapon)]]) {
      const element = document.getElementById(id); element.value = value;
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
    }
  }, { ...credentials, weapon, loadout });
  const choice = await page.evaluate(() => ({ disabled: document.querySelector('#lobby-starting-weapon').disabled,
    mode: document.querySelector('#lobby-mode').value, hint: document.querySelector('#loadout-help').textContent }));
  check(choice.disabled === (loadout === 'classic') && choice.mode === 'ffa',
    `${loadout} weapon${weapon}: real UI loadout selection is independent of scoring mode`, choice);
  await domClick('#btn-create');
  await page.waitForFunction(() => window.__qa.joined && window.__qa.draws > 40
    && !document.querySelector('#settings-resume').disabled, null, { timeout: 60000 });
  await resume();
  const expected = loadout === 'classic' ? 1 : weapon;
  const state = await waitState((state, expected) => state.weapon === expected && state.alive && state.hp === 5, expected);
  const wire = await page.evaluate(() => ({ request: window.__qa.requests.find(message => message.t === 'create_lobby'), joined: window.__qa.joined }));
  check(wire.request.loadout === loadout && wire.request.starting_weapon === expected
    && wire.joined.loadout === loadout && wire.joined.starting_weapon === expected,
  `${loadout} weapon${weapon}: real CreateLobby and GameJoined agree on the selected start`, { request: { ...wire.request, password: '[private fixture]' }, joined: wire.joined });
  check(state.inventory.length === 9 && state.inventory.every((slot, index) => slot.weapon === (index === 0 ? 1 : index + 1 === expected ? expected : 0)),
  `${loadout} weapon${weapon}: authoritative inventory grants only the selected gun plus backup sidearm`);
  await page.waitForFunction(expected => !document.querySelector('#killshot-hud').hidden
    && document.querySelector('#ks-life').textContent === '5'
    && document.querySelectorAll('.ks-slot[data-selected=true]')[0]?.getAttribute('aria-label')?.startsWith(`Slot ${expected},`), expected);
  return credentials;
}
async function hudLayout(width, height) {
  await page.setViewportSize({ width, height }); await frames(4);
  const layout = await page.evaluate(() => {
    const box = selector => {
      const node = document.querySelector(selector), rect = node.getBoundingClientRect();
      return { x: rect.x, y: rect.y, right: rect.right, bottom: rect.bottom, width: rect.width, height: rect.height,
        pointer: getComputedStyle(node).pointerEvents };
    };
    return { viewport: { width: innerWidth, height: innerHeight }, stage: box('#stage'), hud: box('#killshot-hud'),
      life: box('.ks-life'), ammo: box('.ks-ammo'), slots: box('#killshot-slots'), items: [...document.querySelectorAll('.ks-slot')].map(node => {
        const rect = node.getBoundingClientRect(); return { x: rect.x, right: rect.right, width: rect.width, bottom: rect.bottom };
      }) };
  });
  const inside = box => box.width > 0 && box.height > 0 && box.x >= layout.stage.x - 1 && box.right <= layout.stage.right + 1
    && box.y >= layout.stage.y - 1 && box.bottom <= layout.stage.bottom + 1;
  check(layout.stage.right <= width + 1 && layout.stage.x >= -1 && [layout.life, layout.ammo, layout.slots].every(inside)
    && [layout.hud, layout.life, layout.ammo, layout.slots].every(box => box.pointer === 'none')
    && layout.life.right <= layout.ammo.x && layout.items.every(item => item.width > 0 && item.x >= 0 && item.right <= width + 1),
  `${width}px actual LIFE/ammo/nine-slot HUD stays inside the stage and never intercepts input`, layout);
  await shot(`killshot-hud-${width}`);
}
async function beginReload() {
  const before = await currentState();
  await pointer(0, true);
  await waitState((state, before) => state.weapon === before.weapon && state.ammo < before.ammo, before);
  await pointer(0, false);
  // Let the release reach the actual WASM/send tick before measuring rounds;
  // a queued held-fire packet may otherwise spend one more round afterward.
  await packetsAfter(await index(), 3);
  const fired = await currentState();
  await tap('KeyR');
  await waitState(state => state.reload_remaining > 0 && state.reloading);
  await page.waitForFunction(() => !document.querySelector('#killshot-reload').hidden
    && Number(document.querySelector('#ks-reload-time').textContent) > 0);
  check(fired.ammo < before.ammo, `weapon${before.weapon}: real fire decreases authoritative ammunition`, { before: before.ammo, fired: fired.ammo });
  return { before, fired };
}
async function completeReload(label, before, fired) {
  const first = await page.evaluate(() => ({ time: Number(document.querySelector('#ks-reload-time').textContent),
    progress: Number(document.querySelector('#killshot-reload').getAttribute('aria-valuenow')) }));
  await page.waitForFunction(first => !document.querySelector('#killshot-reload').hidden
    && Number(document.querySelector('#ks-reload-time').textContent) < first.time
    && Number(document.querySelector('#killshot-reload').getAttribute('aria-valuenow')) > first.progress, first);
  check(true, `${label}: visible circular reload countdown decreases while authoritative progress advances`, first);
  await shot(`reload-weapon-${before.weapon}`);
  const completed = await waitState(state => !state.reloading && state.reload_remaining === 0);
  await page.waitForFunction(ammo => document.querySelector('#killshot-reload').hidden
    && Number(document.querySelector('#ks-ammo').textContent) === ammo, completed.ammo);
  const expectedReserve = before.reserve === 255 ? 255 : before.reserve - (completed.ammo - fired.ammo);
  check(completed.ammo === before.ammo && completed.reserve === expectedReserve,
    `${label}: reload completes once, hides the indicator and consumes only real reserve`,
    { ammo: completed.ammo, reserve: completed.reserve, expectedReserve });
}
async function passivePeer(credentials) {
  const received = { id: null, frames: [], errors: [] };
  let closed = false;
  const socket = new WebSocket(gameUrl);
  socket.addEventListener('open', () => socket.send(JSON.stringify({ t: 'hello', proto, handle: 'killshot-observer' })));
  socket.addEventListener('message', event => {
    const message = JSON.parse(event.data);
    if (message.t === 'welcome') socket.send(JSON.stringify({ t: 'join_lobby', name: credentials.lobby, password: credentials.password }));
    if (message.t === 'game_joined') received.id = message.id;
    if (message.t === 'state') received.frames.push(message);
    if (message.t === 'error') received.errors.push(message.message);
  });
  socket.addEventListener('error', () => { if (!closed) received.errors.push('Passive fixture socket error'); });
  for (let attempt = 0; attempt < 50 && received.id === null && !received.errors.length; attempt++) await sleep(50);
  if (received.id === null || received.errors.length) { closed = true; socket.close(); throw Error(`Passive peer could not join: ${received.errors.join(', ')}`); }
  return { received, close: () => { closed = true; if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify({ t: 'leave_lobby' })); socket.close(); } };
}
async function killshotInventoryAndHud() {
  const credentials = await createLoadoutMatch(3);
  const peer = await passivePeer(credentials);
  try {
    const initial = await currentState();
    await page.waitForFunction(state => Number(document.querySelector('#ks-ammo').textContent) === state.ammo
      && Number(document.querySelector('#ks-reserve').textContent) === state.reserve, initial);
    check(true, 'actual HUD LIFE5, AK magazine/reserve and inventory are backed by authoritative state');
    await hudLayout(390, 844); await hudLayout(1600, 900);
    await tap('Digit1'); await waitState(state => state.weapon === 1);
    await page.waitForFunction(() => document.querySelector('#ks-weapon').textContent === 'SIDEARM' && document.querySelector('#ks-reserve').textContent === '∞');
    await tap('Digit3'); await waitState(state => state.weapon === 3);
    check(true, 'real slot1/slot3 switches retained sidearm and AK with correct HUD reserve');
    const mark = await index(); await tap('Digit9');
    const emptyPackets = await packetsAfter(mark, 4);
    check(emptyPackets.some(input => input.select_slot === 9) && (await currentState()).weapon === 3,
      'unowned slot9 reaches authority but never grants a weapon');

    const pausedMark = await openMenu(['Digit1']);
    await key('Digit3', true, '#arena-settings');
    check((await packetsAfter(pausedMark, 5)).every(neutral), 'paused weapon-slot controls and queued edges send no selection');
    await resume();
    check((await packetsAfter(await index(), 4)).every(neutral) && (await currentState()).weapon === 3,
      'Resume does not leak held slot controls');
    await key('Digit1', false); await key('Digit3', false); await frames(3);

    // Aim into the sky so the authored fixture cannot hit its passive peer or
    // accidentally self-damage with a launcher against nearby map geometry.
    await motion(0, -350); await packetsAfter(await index(), 3);
    const first = await beginReload();
    await tap('Digit1'); await waitState(state => state.weapon === 1 && !state.reloading);
    await tap('Digit3');
    const cancelled = await waitState(state => state.weapon === 3 && !state.reloading);
    check(cancelled.ammo === first.fired.ammo && cancelled.reserve === first.before.reserve,
      'switching during reload cancels it without granting rounds or consuming reserve');
    await tap('KeyR'); await waitState(state => state.reload_remaining > 0);
    await page.waitForFunction(() => !document.querySelector('#killshot-reload').hidden);
    await completeReload('AK-47', first.before, first.fired);

    const observedFrames = await page.evaluate(() => window.__qa.stateFrames);
    const byTick = new Map(observedFrames.map(state => [state.tick, state]));
    const id = await page.evaluate(() => window.__qa.joined.id);
    const same = [];
    for (const frame of peer.received.frames) {
      const browserState = byTick.get(frame.tick), observerState = frame.players.find(player => player.id === id);
      if (browserState && observerState) {
        for (const key of ['weapon', 'ammo', 'reserve', 'reload_remaining', 'inventory']) assert.deepEqual(observerState[key], browserState[key], `peer mismatch at tick${frame.tick}, ${key}`);
        same.push(frame.tick);
      }
    }
    check(same.length >= 3 && !peer.received.errors.length,
      'passive second player observes the exact same inventory, ammo and reload states at matching ticks', { matchingTicks: same.length });
    report.killshotPeer = { matchingTicks: same.length, errors: peer.received.errors };
  } finally { peer.close(); }
  for (const weapon of [1, 2, 4, 5, 6, 7]) {
    // Explicitly choose Classic once, with a stale non-pistol value, to prove
    // the disabled selector cannot smuggle a custom grant into Classic.
    await createLoadoutMatch(weapon === 1 ? 7 : weapon, weapon === 1 ? 'classic' : 'custom');
    await motion(0, -350); await packetsAfter(await index(), 3);
    const { before, fired } = await beginReload();
    await completeReload(`weapon${weapon}`, before, fired);
    const errors = await page.evaluate(() => window.__qa.errors);
    check(errors.length === 0, `weapon${weapon}: rendered private match has no protocol errors`);
  }
  report.killshotCoverage = 'Real page creation, seven actual weapon reloads, HUD/layout, finite/infinite ammo, remappable-slot default routing, empty slots, reload cancellation and passive-peer state agreement. Pickups/healing/death reset are gated in Rust/network tests, not by this stationary browser fixture.';
}

async function shotgunSmoke() {
  await createLoadoutMatch(8);
  const spawned = await currentState();
  check(spawned.ammo === 6 && spawned.reserve === 24 && spawned.weapon === 8,
    'Breach-12 custom loadout authoritatively grants six shells plus 24 reserve', { ammo: spawned.ammo, reserve: spawned.reserve });
  await page.waitForFunction(() => document.querySelector('#ks-weapon').textContent === 'BREACH-12'
    && document.querySelector('#ks-ammo').textContent === '6' && document.querySelector('#ks-reserve').textContent === '24'
    && document.querySelectorAll('.ks-slot')[7].getAttribute('data-selected') === 'true');
  check(true, 'actual HUD names Breach-12 and selects slot8 with correct shell counts');
  await hudLayout(390, 844); await hudLayout(1600, 900);
  await shot('breach12-ready');
  await tap('Digit1'); await waitState(state => state.weapon === 1);
  await tap('Digit8'); await waitState(state => state.weapon === 8);
  const mark = await index(); await tap('Digit9');
  check((await packetsAfter(mark, 3)).some(input => input.select_slot === 9) && (await currentState()).weapon === 8,
    'slot1/slot8 switches retained guns; unowned slot9 cannot replace the shotgun');
  await openMenu(); await rebind('Weapon slot 8', 'KeyZ'); await resume();
  await tap('Digit1'); await waitState(state => state.weapon === 1);
  await tap('Digit8');
  check((await currentState()).weapon === 1, 'old Digit8 no longer switches after rebinding');
  await tap('KeyZ'); await waitState(state => state.weapon === 8);
  await page.waitForFunction(() => document.querySelectorAll('.ks-slot-key')[7].textContent === 'Z');
  check(true, 'remapped shotgun key reaches actual WASM and appears in the HUD');
  await motion(0, -350); await packetsAfter(await index(), 3);
  const { before, fired } = await beginReload();
  const reloading = await currentState();
  check(reloading.weapon === 8 && reloading.reload_remaining > 2 && reloading.reload_remaining <= 2.8,
    'Breach-12 reports the authoritative 2.8-second reload countdown', { remaining: reloading.reload_remaining });
  await completeReload('Breach-12', before, fired);
  await shot('breach12-reloaded');
  const errors = await page.evaluate(() => window.__qa.errors);
  check(errors.length === 0, 'shotgun match contains no protocol/application errors');
  report.shotgunCoverage = 'Actual custom8 lobby, hotbar/name/counts, slot1/8/9 and rebind routing, firing shell expenditure, 2.8s reload countdown and reserve conservation, responsive HUD. Pellet damage and pump firing limits require separate authoritative core/network gates.';
}

async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  await startServices();
  browser = await chromium.launch({ ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
    headless: true, args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  const context = await browser.newContext({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 1 });
  await context.route('**/*', route => {
    const url = new URL(route.request().url());
    if (url.origin !== origin) { report.errors.push(`External request blocked: ${url.origin}`); return route.abort(); }
    return route.continue();
  });
  await context.addInitScript(instrument, { gameUrl, proto });
  page = await context.newPage();
  page.on('pageerror', error => report.errors.push(String(error)));
  page.on('console', message => {
    if (message.type() === 'error') report.errors.push(message.text());
    if (message.type() === 'warning') report.warnings.push(message.text());
  });
  if (shotgunOnly) {
    await page.goto(entry);
    await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  } else {
    await preferencesBeforeMatch(); await joinPrivateMatch();
    const baseLook = await lookTrial();
    await pauseSafety(); await remappedPlay(baseLook);
    await killshotInventoryAndHud();
  }
  await shotgunSmoke();
  check(report.errors.length === 0, 'page has no runtime or blocked-external errors');
  report.passed = true;
}

main().catch(async error => {
  report.errors.push(error.stack || String(error)); console.error(error);
  if (page) {
    report.failureState = await page.evaluate(() => ({ settings: window.__emberArenaSettings, qa: window.__qa,
      dialog: document.querySelector('#arena-settings')?.open, status: document.querySelector('#status')?.textContent,
      feedback: document.querySelector('#settings-feedback')?.textContent,
      fullscreenState: document.querySelector('#settings-fullscreen-state')?.textContent,
      fullscreenHelp: document.querySelector('#settings-fullscreen-help')?.textContent })).catch(() => null);
    await page.screenshot({ path: path.join(output, 'failure.png') }).catch(() => {});
  }
}).finally(async () => {
  if (browser) await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  if (game && game.exitCode === null) game.kill();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ passed: report.passed, checks: report.checks.length, elapsedSeconds: report.elapsedSeconds, output }));
  if (!report.passed) process.exitCode = 1;
});
