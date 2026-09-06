// Real v2 WASM controls, without the settings UI or a multiplayer server.
// Run from any directory after building web/pkg; EMBER_QA_PLAYWRIGHT selects
// the existing Playwright installation. Only headless Edge receives synthetic
// canvas DOM events: no desktop input, focus change, or live service is used.
'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const http = require('node:http');
const os = require('node:os');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const pkg = path.join(root, 'web/pkg');
const out = path.join(root, 'target/league-bindings');
const port = Number(process.env.EMBER_QA_BINDINGS_PORT || 8095);
const origin = `http://127.0.0.1:${port}`;
const route = '/games/league/v2/';
const defaults = {
  q: 'KeyQ', w: 'KeyW', e: 'KeyE', r: 'KeyR', d: 'KeyD', f: 'KeyF',
  item1: 'Digit1', item2: 'Digit2', item3: 'Digit3', item4: 'Digit4',
  item5: 'Digit5', item6: 'Digit6', stop: 'KeyS', shop: 'KeyB',
};
const started = Date.now();
const report = { checks: [], errors: [], console: [], screenshots: [], assets: [] };
let browser, server, page;

const fixture = `<!doctype html>
<html lang="en"><meta charset="utf-8"><title>League v2 bindings fixture</title>
<style>body{margin:0;background:#080b12}#ember-root{width:1280px;height:720px}
canvas{width:100%!important;height:100%!important;display:block;outline:none}</style>
<div id="ember-root"></div><script type="module">
try {
  const wasm = await import('./pkg/league.js');
  await wasm.default();
  window.qaWasm = wasm;
  window.qaState = () => JSON.parse(wasm.state_json());
  document.body.dataset.boot = 'ready';
} catch (error) {
  window.qaBootError = String(error?.stack || error);
  document.body.dataset.boot = 'failed';
}
</script></html>`;

function check(ok, name) {
  assert(ok, name);
  report.checks.push(name);
  console.log('PASS ' + name);
}

const state = () => page.evaluate(() => window.qaState());
const command = value => page.evaluate(value => window.qaWasm.cmd_json(JSON.stringify(value)), value);
const enabled = value => page.evaluate(value => window.qaWasm.set_input_enabled(value), value);
const bindings = () => page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()));
const moved = (a, b) => Math.hypot(a.me.x - b.me.x, a.me.z - b.me.z);

// Wait on simulation progress so a slow renderer cannot produce an inert-input
// false positive merely because it has not processed a frame yet.
async function advance(seconds = 0.25) {
  const before = await state();
  assert(before.phase === 'live' && before.me?.alive, 'Practice champion must be alive');
  await page.waitForFunction(target => window.qaState().secs >= target, before.secs + seconds);
  const after = await state();
  assert(after.phase === 'live' && after.me?.alive, 'Practice ended during controls checks');
  return after;
}

async function keyEvent(type, code) {
  await page.evaluate(({ type, code }) => {
    document.querySelector('#ember-root canvas').dispatchEvent(new KeyboardEvent(type, {
      code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code,
      bubbles: true, cancelable: true,
    }));
  }, { type, code });
}

async function key(code) {
  await keyEvent('keydown', code);
  await advance(0.15);
  await keyEvent('keyup', code);
  return advance(0.1);
}

async function pointer(type = 'pointermove', button = -1, xf = 0.75, yf = 0.47) {
  await page.evaluate(({ type, button, xf, yf }) => {
    const canvas = document.querySelector('#ember-root canvas');
    const rect = canvas.getBoundingClientRect();
    const event = new PointerEvent(type, {
      bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      clientX: rect.left + rect.width * xf, clientY: rect.top + rect.height * yf,
      button, buttons: type === 'pointerdown' ? (button === 2 ? 2 : 1) : 0,
    });
    if (type === 'pointermove') Object.defineProperty(event, 'getCoalescedEvents', { value: () => [event] });
    canvas.dispatchEvent(event);
  }, { type, button, xf, yf });
}

async function mouse(button) {
  await pointer();
  await pointer('pointerdown', button);
  await advance(0.15);
  await pointer('pointerup', button);
  return advance(0.1);
}

function inert(before, after, name) {
  check(after.me.cd[1] === 0 && after.me.scd[0] === 0 &&
    after.me.mn >= before.me.mn - 0.01 && moved(before, after) < 0.05, name);
}

async function apiChecks() {
  const exported = await page.evaluate(() => [
    'bindings_json', 'set_bindings_json', 'binding_options_json', 'set_input_enabled',
  ].every(name => typeof window.qaWasm[name] === 'function'));
  check(exported, 'actual WASM exports all four bindings/input APIs');
  assert.deepEqual(await bindings(), defaults);
  check(true, 'default API map has exactly 14 expected action bindings');
  const options = await page.evaluate(() => JSON.parse(window.qaWasm.binding_options_json()));
  check(Array.isArray(options) && options.every(o => typeof o.code === 'string' &&
    typeof o.label === 'string' && o.code && o.label) &&
    new Set(options.map(o => o.code)).size === options.length &&
    [...Object.values(defaults), 'KeyT'].every(code => options.some(o => o.code === code)),
  'binding options provide distinct physical codes and readable labels');

  const alternate = Object.fromEntries(Object.keys(defaults).map((action, i) => [action, [
    'KeyG', 'KeyH', 'KeyI', 'KeyJ', 'KeyK', 'KeyL', 'Digit7', 'Digit8',
    'Digit9', 'Digit0', 'Numpad1', 'Numpad2', 'KeyM', 'KeyN',
  ][i]]));
  await page.evaluate(map => window.qaWasm.set_bindings_json(JSON.stringify(map)), alternate);
  assert.deepEqual(await bindings(), alternate);
  check(true, 'all 14 actions, including item slots, stop and shop, can be replaced');
  await page.evaluate(() => window.qaWasm.set_bindings_json('{}'));
  assert.deepEqual(await bindings(), defaults);
  check(true, 'empty binding map restores every default');
  await page.evaluate(() => window.qaWasm.set_bindings_json('{"w":"KeyT"}'));
  const remapped = { ...defaults, w: 'KeyT' };
  assert.deepEqual(await bindings(), remapped);
  check(true, 'partial map remaps W to physical KeyT and retains other defaults');

  for (const [name, json] of [
    ['duplicate physical keys', '{"q":"KeyT","w":"KeyT"}'],
    ['conflict with the shop key', '{"w":"KeyB"}'],
    ['duplicate JSON action names', '{"w":"KeyT","w":"KeyY"}'],
    ['unsupported physical code', '{"w":"NotAKey"}'],
    ['reserved Escape', '{"w":"Escape"}'],
    ['reserved modifier', '{"w":"ControlLeft"}'],
    ['unknown action', '{"w":"KeyY","unknown":"KeyX"}'],
    ['wrong value type', '{"w":7}'],
    ['malformed JSON', '{'],
  ]) {
    const result = await page.evaluate(json => {
      try { window.qaWasm.set_bindings_json(json); return { rejected: false }; }
      catch (error) { return { rejected: true, reason: String(error) }; }
    }, json);
    check(result.rejected && Boolean(result.reason), `${name} is rejected with an error`);
    assert.deepEqual(await bindings(), remapped, `${name} partially changed live bindings`);
    check(true, `${name} leaves the prior remap intact`);
  }
}

async function controlsChecks() {
  await page.evaluate(() => window.qaWasm.start_local(1));
  await page.waitForFunction(() => window.qaState().connected && document.querySelector('#ember-root canvas'));
  await command({ pick: { champ: 0, d: 0, f: 1, runes: [0, 1, 2] } });
  await page.waitForFunction(() => window.qaState().roster[0].picked);
  await command({ start: true });
  await page.waitForFunction(() => window.qaState().phase === 'live' && window.qaState().me?.alive);
  await command({ rank: 1 });
  await page.waitForFunction(() => window.qaState().me.rk[1] === 1);
  check(true, 'real practice match learns SW4RM W with its starting skill point');
  await pointer();
  await advance();
  let before = await state();
  inert(before, await key('KeyW'), 'old W key is inert after rebinding W to KeyT');

  await enabled(false);
  before = await advance();
  inert(before, await key('KeyT'), 'disabled input blocks the remapped keyboard ability');
  inert(before, await key('KeyD'), 'disabled input blocks keyboard Flash movement');
  inert(before, await mouse(2), 'disabled input blocks right-click movement');
  inert(before, await mouse(0), 'disabled input blocks left-click movement');
  // Flash is the supported spatial cmd_json action; there is no "move" field.
  await command({ cast: 1, aim: [0.5, 0.06], aspect: 16 / 9 });
  await command({ spell: 0, aim: [0.5, 0.06], aspect: 16 / 9 });
  inert(before, await advance(), 'disabled input blocks cmd_json ability and Flash commands');
  await enabled(true);
  inert(before, await advance(0.4), 'reenabling does not replay released input or paused UI commands');

  // All calls occur in one JS task, before the next engine frame can drain
  // commands. Exercise commands queued both before and during a short pause.
  await page.evaluate(() => {
    window.qaWasm.cmd_json('{"cast":1,"aim":[0.5,0.06]}');
    window.qaWasm.cmd_json('{"spell":0,"aim":[0.5,0.06]}');
    window.qaWasm.set_input_enabled(false);
    window.qaWasm.cmd_json('{"cast":1,"aim":[0.5,0.06]}');
    window.qaWasm.cmd_json('{"spell":0,"aim":[0.5,0.06]}');
    window.qaWasm.set_input_enabled(true);
  });
  inert(before, await advance(0.4), 'queued casts cannot cross a disable/enable cycle between frames');

  await enabled(false);
  await advance();
  await keyEvent('keydown', 'KeyT');
  await pointer('pointerdown', 2);
  inert(before, await advance(), 'held keyboard and mouse input remain inert while disabled');
  await enabled(true);
  inert(before, await advance(0.4), 'held key and mouse button do not replay when input resumes');
  await keyEvent('keyup', 'KeyT');
  await pointer('pointerup', 2);
  await advance();
  before = await state();
  const cast = await key('KeyT');
  check(cast.me.cd[1] > 0 && cast.me.mn < before.me.mn,
    'a fresh KeyT press casts the learned W after input resumes');

  before = await state();
  await command({ spell: 0, aim: [0.5, 0.06], aspect: 16 / 9 });
  const flashed = await advance();
  check(flashed.me.scd[0] > 0 && moved(before, flashed) > 0.5,
    'the same cmd_json Flash payload moves the champion when input is enabled');

  before = await state();
  const walking = await mouse(2);
  check(moved(before, walking) > 0.2, 'the same right-click event moves the champion when enabled');
  await enabled(false);
  before = await advance(0.1);
  await command({ stop: true });
  const ignoredStop = await key('KeyS');
  check(moved(before, ignoredStop) > 0.2,
    'disabled keyboard and cmd_json stop orders cannot cancel an existing move');
  await enabled(true);
  await advance();
  await command({ stop: true });
  const stopped = await advance(0.15);
  check(moved(stopped, await advance(0.4)) < 0.05,
    'a fresh cmd_json stop order works after input is reenabled');
  before = await state();
  check(moved(before, await mouse(0)) > 0.2,
    'the same left-click event moves the champion when enabled');
  await command({ stop: true });
}

async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out, { recursive: true });
  assert(Number.isInteger(port) && port >= 1024 && port <= 65535, 'Invalid fixture port');
  for (const file of ['league.js', 'league_bg.wasm']) {
    const data = fs.readFileSync(path.join(pkg, file));
    report.assets.push({ file, bytes: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') });
  }
  server = http.createServer((req, res) => {
    let pathname;
    try { pathname = decodeURIComponent(new URL(req.url, origin).pathname); }
    catch { res.writeHead(400).end(); return; }
    res.setHeader('Cache-Control', 'no-store');
    if (pathname === route) { res.setHeader('Content-Type', 'text/html; charset=utf-8'); res.end(fixture); return; }
    if (pathname === '/favicon.ico') { res.writeHead(204).end(); return; }
    const prefix = route + 'pkg/';
    if (!pathname.startsWith(prefix)) { res.writeHead(404).end(); return; }
    const file = path.resolve(pkg, pathname.slice(prefix.length));
    if (!file.startsWith(pkg + path.sep) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end(); return;
    }
    res.setHeader('Content-Type', path.extname(file) === '.wasm' ? 'application/wasm' : 'text/javascript');
    res.end(fs.readFileSync(file));
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(port, '127.0.0.1', resolve); });
  const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
  browser = await chromium.launch({
    channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'],
  });
  page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.setDefaultTimeout(30000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') report.console.push(message.text());
  });
  await page.route('**/*', request => {
    if (new URL(request.request().url()).origin === origin) return request.continue();
    report.errors.push('Unexpected external request: ' + request.request().url());
    return request.abort();
  });
  await page.addInitScript(() => {
    window.focus = () => {};
    Element.prototype.setPointerCapture = () => {};
  });
  await page.goto(origin + route, { waitUntil: 'load' });
  await page.waitForFunction(() => ['ready', 'failed'].includes(document.body.dataset.boot));
  assert.equal(await page.evaluate(() => document.body.dataset.boot), 'ready',
    await page.evaluate(() => window.qaBootError || 'WASM fixture did not initialize'));
  await apiChecks();
  await controlsChecks();
  check(report.errors.length === 0, 'no uncaught browser errors or external network requests');
  report.passed = true;
}

main().catch(error => {
  report.failure = error.stack;
  console.error(error);
  process.exitCode = 1;
}).finally(async () => {
  if (report.failure && page) {
    try {
      report.lastState = await state();
      const screenshot = path.join(out, 'failure.png');
      await page.screenshot({ path: screenshot });
      report.screenshots.push(screenshot);
    } catch (error) { report.diagnosticError = String(error); }
  }
  if (browser) await browser.close();
  if (server?.listening) {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
});
