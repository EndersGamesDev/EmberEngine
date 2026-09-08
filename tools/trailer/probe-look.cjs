// Diagnostic: which authored mouse-motion event does the real client accept?
// Joins a private match exactly as the capture harness does, then tries one
// dispatch strategy at a time and reports the aim change each produced.
'use strict';
process.env.EMBER_TRAILER_WEB_PORT = process.env.EMBER_TRAILER_WEB_PORT || '8092';
process.env.EMBER_TRAILER_GAME_PORT = process.env.EMBER_TRAILER_GAME_PORT || '7792';

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
const webPort = Number(process.env.EMBER_TRAILER_WEB_PORT);
const gamePort = Number(process.env.EMBER_TRAILER_GAME_PORT);
const proto = 24;
const origin = `http://127.0.0.1:${webPort}`;
const gameUrl = `ws://127.0.0.1:${gamePort}`;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const mime = (f) => ({ '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html',
  '.json': 'application/json', '.css': 'text/css', '.png': 'image/png' }[path.extname(f)] || 'application/octet-stream');

const listening = (port) => new Promise((resolve) => {
  const s = net.connect({ host: '127.0.0.1', port });
  s.once('connect', () => { s.destroy(); resolve(true); });
  s.once('error', () => { s.destroy(); resolve(false); });
});

let server; let game; let browser;

(async () => {
  const pkg = path.join(webRoot, 'pkg');
  server = http.createServer((rq, rs) => {
    const p = decodeURIComponent(new URL(rq.url, origin).pathname);
    if (p === '/server.json') {
      rs.writeHead(200, { 'Content-Type': 'application/json' });
      rs.end(JSON.stringify({ v: 'probe', hosts: [{ name: 'probe', ws: gameUrl, proto }], mirrors: [] })); return;
    }
    const pp = '/games/arena/v31/pkg/';
    const base = p.startsWith(pp) ? pkg : webRoot;
    const rel = p.startsWith(pp) ? p.slice(pp.length) : `.${p}`;
    const file = path.resolve(base, rel);
    if (!file.startsWith(base) || !fs.existsSync(file) || !fs.statSync(file).isFile()) { rs.writeHead(404); rs.end(); return; }
    rs.writeHead(200, { 'Content-Type': mime(file) });
    fs.createReadStream(file).pipe(rs);
  });
  await new Promise((res) => server.listen(webPort, '127.0.0.1', res));
  game = spawn(path.join(root, 'target', 'release', 'arena-server.exe'),
    ['--bind', `127.0.0.1:${gamePort}`, '--name', 'probe'], { cwd: root, windowsHide: true, stdio: 'ignore' });
  if (game.pid) os.setPriority(game.pid, os.constants.priority.PRIORITY_LOW);
  for (let i = 0; i < 80 && !(await listening(gamePort)); i++) await sleep(100);

  browser = await chromium.launch({ channel: 'msedge' });
  const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
  await page.addInitScript(({ url, expected }) => {
    localStorage.setItem('ember-server-url-manual', '1');
    localStorage.setItem('ember-server-url', url);
    localStorage.setItem('ember-account', JSON.stringify({ handle: 'probe', id: 'probe' }));
    Object.defineProperty(navigator, 'getGamepads', { value: () => [], configurable: true });
    window.__p = { aim: null, joined: null, welcome: null };
    let locked = null;
    Object.defineProperty(document, 'pointerLockElement', { get: () => locked, configurable: true });
    HTMLElement.prototype.focus = function () { this.dispatchEvent(new FocusEvent('focus')); };
    Element.prototype.requestPointerLock = function () {
      locked = this;
      window.__p.lockTag = `${this.tagName}#${this.id || ''}.${this.className || ''}`;
      queueMicrotask(() => document.dispatchEvent(new Event('pointerlockchange')));
      return Promise.resolve();
    };
    document.exitPointerLock = () => { locked = null; };
    const W = window.WebSocket;
    window.WebSocket = class extends W {
      constructor(u, p) {
        super(u, ...(p === undefined ? [] : [p]));
        const send = this.send.bind(this);
        this.send = (d) => { try { const m = JSON.parse(d); if (m.t === 'input') window.__p.aim = m; } catch {} return send(d); };
        this.addEventListener('message', (e) => {
          const m = JSON.parse(e.data);
          if (m.t === 'welcome') window.__p.welcome = m;
          if (m.t === 'game_joined') window.__p.joined = m;
        });
      }
    };
  }, { url: gameUrl, expected: proto });

  await page.goto(`${origin}/games/arena/v31/index.html`);
  await page.waitForFunction(() => window.__p.welcome, null, { timeout: 60000 });
  await page.evaluate(() => {
    for (const [id, v] of [['lobby-name', `probe-${Math.random().toString(36).slice(2, 8)}`], ['lobby-pw', 'x'],
      ['lobby-map', 'harbor'], ['lobby-mode', 'ffa']]) {
      const el = document.querySelector(`#${id}`);
      if (el) { el.value = v; el.dispatchEvent(new Event(el.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true })); }
    }
    document.querySelector('#btn-create').dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 }));
  });
  await page.waitForFunction(() => window.__p.joined, null, { timeout: 60000 });
  await page.waitForFunction(() => window.__emberArenaSettings && window.__emberArenaSettings.paused, null, { timeout: 30000 });
  await page.evaluate(() => document.querySelector('#settings-resume')
    .dispatchEvent(new MouseEvent('click', { bubbles: true, cancelable: true, button: 0 })));
  await page.waitForFunction(() => !window.__emberArenaSettings.paused, null, { timeout: 15000 });
  await sleep(800);

  const yaw = async () => {
    const a = await page.evaluate(() => window.__p.aim);
    return a ? +(Math.atan2(a.az, a.ax) * 180 / Math.PI).toFixed(3) : null;
  };
  const lockTag = await page.evaluate(() => window.__p.lockTag || null);
  const canvasInfo = await page.evaluate(() => {
    const c = document.querySelector('#ember-root canvas');
    return c ? { tag: c.tagName, id: c.id, cls: c.className } : null;
  });

  const strategies = {
    'mousemove on canvas (bubbles)': () => {
      const c = document.querySelector('#ember-root canvas');
      const e = new MouseEvent('mousemove', { bubbles: true, cancelable: true });
      Object.defineProperty(e, 'movementX', { value: 120 });
      Object.defineProperty(e, 'movementY', { value: 0 });
      c.dispatchEvent(e);
    },
    'mousemove on document': () => {
      const e = new MouseEvent('mousemove', { bubbles: true, cancelable: true });
      Object.defineProperty(e, 'movementX', { value: 120 });
      Object.defineProperty(e, 'movementY', { value: 0 });
      document.dispatchEvent(e);
    },
    'pointermove on canvas': () => {
      const c = document.querySelector('#ember-root canvas');
      const e = new PointerEvent('pointermove', { bubbles: true, cancelable: true, pointerId: 1, isPrimary: true });
      Object.defineProperty(e, 'movementX', { value: 120 });
      Object.defineProperty(e, 'movementY', { value: 0 });
      c.dispatchEvent(e);
    },
    'mousemove on window': () => {
      const e = new MouseEvent('mousemove', { bubbles: true, cancelable: true });
      Object.defineProperty(e, 'movementX', { value: 120 });
      Object.defineProperty(e, 'movementY', { value: 0 });
      window.dispatchEvent(e);
    },
  };

  const results = {};
  for (const [name, fn] of Object.entries(strategies)) {
    const before = await yaw();
    for (let i = 0; i < 5; i++) { await page.evaluate(fn); await sleep(60); }
    await sleep(300);
    const after = await yaw();
    results[name] = { before, after, movedDegrees: before === null || after === null ? null : +(after - before).toFixed(3) };
  }
  console.log(JSON.stringify({ lockTag, canvasInfo, results }, null, 2));
})().catch((e) => { console.error(String(e)); process.exitCode = 1; })
  .finally(async () => {
    if (browser) await browser.close().catch(() => {});
    if (game && game.exitCode === null) game.kill();
    if (server) server.close();
  });
