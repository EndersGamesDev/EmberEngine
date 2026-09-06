// Actual v28 page + WASM + an owned, private loopback arena-server (protocol 21).
// Run from the repository root AFTER building and wasm-bindgen:
// EMBER_QA_PLAYWRIGHT=<absolute module> node tools/v28/browser-controls.cjs
// Optional EMBER_QA_BROWSER, EMBER_QA_SERVER, EMBER_QA_OUTPUT; ports8088/7788 must be free.
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
const output = path.resolve(process.env.EMBER_QA_OUTPUT || path.join(root, 'target', 'controls-browser'));
const webPort = 8088, gamePort = 7788, proto = 21;
const origin = `http://127.0.0.1:${webPort}`, gameUrl = `ws://127.0.0.1:${gamePort}`;
const entry = `${origin}/games/arena/v28/index.html`;
const started = Date.now();
const report = { passed: false, proto, checks: [], errors: [], warnings: [], serverLog: [], screenshots: [], limits: [
  'Synthetic DOM events only; trusted pointer-lock/fullscreen permission is deliberately stubbed.',
  'Fullscreen state and refusal handling are tested with DOM stubs, not actual fullscreen layout or browser permissions.',
  'One real player on a disposable private server; no claim about remote-player combat.',
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
  const versionPkg = path.join(webRoot, 'games', 'arena', 'v28', 'pkg');
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
      response.end(JSON.stringify({ v: 'controls-private-qa', hosts: [{ name: 'controls-qa', ws: gameUrl, proto }], mirrors: [] })); return;
    }
    const packagePath = '/games/arena/v28/pkg/';
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
  game = spawn(executable, ['--bind', `127.0.0.1:${gamePort}`, '--name', 'controls-qa'], {
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
  localStorage.setItem('ember-account', JSON.stringify({ handle: 'controls-qa', id: 'private-controls-qa' }));
  Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
  Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
  window.__qa = { inputs: [], states: 0, shots: 0, joined: null, welcome: null, draws: 0,
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
        if (message.t === 'game_joined') window.__qa.joined = { id: message.id, lobby: message.lobby };
        if (message.t === 'state') window.__qa.states++;
        if (message.t === 'shot') window.__qa.shots++;
        if (message.t === 'error') window.__qa.errors.push(message.message);
      });
    }
    send(data) {
      const message = JSON.parse(data);
      if (message.t === 'input') window.__qa.inputs.push({ ...message, at: performance.now() });
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
  && ['fire', 'ads', 'sprint', 'crouch', 'reload', 'jump', 'shield', 'melee'].every(name => input[name] === false);
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
  await page.waitForFunction(() => window.__qa.welcome && document.querySelector('#host-chip').textContent.includes('controls-qa'), null, { timeout: 60000 });
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
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem('ember-arena-settings-v1')));
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
  check(details.welcome.proto === proto && details.gl && details.joined, 'actual WASM joins owned protocol21 server and draws WebGL2', details);
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
  report.traffic = await page.evaluate(() => ({ inputs: window.__qa.inputs, states: window.__qa.states, shots: window.__qa.shots,
    draws: window.__qa.draws, gl: window.__qa.gl, sockets: window.__qa.sockets, errors: window.__qa.errors }));
  check(report.traffic.errors.length === 0, 'real-server traffic contains no protocol or application errors');
  await page.reload(); await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  const persisted = await config();
  check(persisted.sensitivity === 2 && persisted.bindings.forward[0] === 'KeyI' && persisted.bindings.fire[0] === 'Mouse1', 'in-match remaps and sensitivity survive reload');
  await domClick('#btn-settings'); await domClick('#settings-reset');
  await page.reload(); await page.waitForFunction(() => window.__qa.welcome, null, { timeout: 60000 });
  const reset = await config();
  check(reset.sensitivity === 1 && reset.bindings.forward[0] === 'KeyW' && reset.bindings.fire[0] === 'Mouse0', 'restored defaults also persist across reload');
}

async function main() {
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
  await preferencesBeforeMatch(); await joinPrivateMatch();
  const baseLook = await lookTrial();
  await pauseSafety(); await remappedPlay(baseLook);
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
