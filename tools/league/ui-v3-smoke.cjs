// Actual V3 page/WASM controls and guide. Synthetic DOM events stay in headless
// Edge; no desktop input, live rooms, foreground change or gameplay shortcuts.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const os = require('node:os');
const { startPreview } = require('./preview.cjs');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
const root = path.resolve(__dirname, '../..');
const out = path.join(root, 'target/league-ui-v3');
const started = Date.now();
const report = { checks: [], errors: [], screenshots: [], assets: [] };
const storageKey = 'ember-league-v3-keys';
let preview, browser, page;
const state = () => page.evaluate(() => window.qaState());
function check(ok, name) { assert(ok, name); report.checks.push(name); console.log('PASS ' + name); }
async function click(selector) {
  await page.waitForFunction(selector => {
    const e = document.querySelector(selector);
    return e && !e.disabled && e.getClientRects().length;
  }, selector);
  await page.evaluate(selector => { const e = document.querySelector(selector); e.focus({ preventScroll: true }); e.click(); }, selector);
}
async function visible(id, expected = true) {
  await page.waitForFunction(({ id, expected }) => Boolean(document.getElementById(id)?.getClientRects().length) === expected, { id, expected });
}
async function keyboard(code, type = 'keydown', selector = '#ember-root canvas') {
  await page.evaluate(({ code, type, selector }) => document.querySelector(selector).dispatchEvent(new KeyboardEvent(type, {
    code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code, bubbles: true, cancelable: true,
  })), { code, type, selector });
}
async function advance(seconds = 0.2) {
  const before = await state();
  assert(before.phase === 'live' && before.me?.alive, 'Expected live practice champion');
  await page.waitForFunction(target => window.qaState().secs >= target, before.secs + seconds);
  return state();
}
async function key(code, selector = '#ember-root canvas') {
  await keyboard(code, 'keydown', selector);
  await advance(0.15); // Hold through observed simulation progress, never a fixed wall-time pulse.
  await keyboard(code, 'keyup', selector);
  return advance(0.1);
}
async function pointer(button = null) {
  await page.evaluate(button => {
    const c = document.querySelector('#ember-root canvas'), r = c.getBoundingClientRect();
    const init = { bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      clientX: r.left + r.width * .72, clientY: r.top + r.height * .47, button: -1, buttons: 0 };
    const move = new PointerEvent('pointermove', init);
    Object.defineProperty(move, 'getCoalescedEvents', { value: () => [move] }); c.dispatchEvent(move);
    if (button !== null) c.dispatchEvent(new PointerEvent('pointerdown', { ...init, button, buttons: button === 2 ? 2 : 1 }));
  }, button);
  await advance(.15);
  if (button !== null) await page.evaluate(button => document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent('pointerup', {
    bubbles: true, pointerType: 'mouse', pointerId: 1, button, buttons: 0,
  })), button);
  return advance(.1);
}
const moved = (a, b) => Math.hypot(a.me.x - b.me.x, a.me.z - b.me.z);
async function shot(name) {
  const file = path.join(out, name + '.png'); await page.screenshot({ path: file }); report.screenshots.push(file);
}
async function attach() {
  await page.evaluate(async () => { window.qaWasm = await window.leagueReady; window.qaState = () => JSON.parse(window.qaWasm.state_json()); });
}
async function bind(action, code) {
  await click(`#key-rows .kset[data-act="${action}"]`);
  await keyboard(code, 'keydown', '#keys'); await keyboard(code, 'keyup', '#keys');
}
async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW); fs.mkdirSync(out, { recursive: true });
  const bundles = Object.fromEntries(['league.js', 'league_bg.wasm'].map(name => {
    const bytes = fs.readFileSync(path.join(root, 'web/pkg', name));
    report.assets.push({ name, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') });
    return [name, bytes];
  }));
  preview = await startPreview({ port: 8096, gamePort: 7796, gameVersion: 'v3', online: false });
  browser = await chromium.launch({ channel: 'msedge', headless: true, args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 } }); page.setDefaultTimeout(20000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('response', response => { if (response.status() >= 400) report.errors.push(`HTTP ${response.status()}: ${response.url()}`); });
  await page.addInitScript(() => { window.focus = () => {}; Element.prototype.setPointerCapture = () => {}; });
  await page.route('**/v3/pkg/*', route => {
    const name = new URL(route.request().url()).pathname.split('/').at(-1);
    return bundles[name] ? route.fulfill({ body: bundles[name], contentType: name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript' }) : route.abort();
  });
  await page.goto(preview.origin + '/games/league/v3/'); await attach();
  const defaults = await page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()));
  check(Object.keys(defaults).length === 15 && defaults.attackMove === 'KeyA', 'all 15 bindings include default A attack-move');
  check(await page.evaluate(() => window.qaWasm.proto_version()) === 2, 'V3 uses its independent protocol 2');
  await click('#btn-guide'); await visible('help');
  check(await page.locator('#help section').count() === 7 && await page.locator('#guide-kits details').count() === 5 && await page.locator('#guide-kits article').count() === 20, 'guide covers seven chapters, five champions and all 20 abilities');
  await click('#guide-kits details:nth-child(3) summary');
  check(await page.evaluate(() => document.getElementById('help').innerText.includes('empty potion stays') && document.getElementById('help').innerText.includes('cannot revive an already-dead')), 'guide explains potion refills and pre-lethal Hallow targeting');
  await shot('guide');
  await keyboard('Escape', 'keydown', '#help'); await keyboard('Escape', 'keyup', '#help'); await visible('help', false);
  check(await page.evaluate(() => document.activeElement.id) === 'btn-guide', 'guide Escape restores launcher focus');
  await click('#btn-keys');
  check(await page.locator('#key-rows .krow').count() === 15, 'settings exposes every binding including attack-move');
  await bind('attackMove', 'KeyG');
  check(await page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()).attackMove) === 'KeyG', 'attack-move can be remapped through actual settings');
  await bind('q', 'KeyG'); await visible('key-conflict');
  await click('#btn-conflict-take');
  const swapped = await page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()));
  check(swapped.q === 'KeyG' && swapped.attackMove === 'KeyQ' && new Set(Object.values(swapped)).size === 15, 'conflict swap atomically preserves 15 unique physical controls');
  const rejected = await page.evaluate(() => { const before = window.qaWasm.bindings_json(); try { window.qaWasm.set_bindings_json('{"attackMove":"KeyQ"}'); return false; } catch { return before === window.qaWasm.bindings_json(); } });
  check(rejected, 'conflicting attack-move map is rejected without changing controls');
  await click('#btn-keys-close'); await page.reload(); await attach();
  check(await page.evaluate(key => JSON.parse(localStorage.getItem(key)).attackMove, storageKey) === 'KeyQ' && await page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()).attackMove) === 'KeyQ', 'remapped attack-move survives reload in V3 storage');
  await click('#btn-practice');
  await page.waitForFunction(() => window.qaState().connected && window.qaState().phase === 'select');
  await click('#cards [data-c="1"]');
  await page.waitForFunction(() => window.qaState().roster[0].picked);
  await click('#btn-start'); await page.waitForFunction(() => window.qaState().phase === 'live' && window.qaState().me?.alive);
  await advance(); await click('#abils [data-abil="0"] .up');
  await page.waitForFunction(() => window.qaState().me.rk[0] === 1); await advance();
  const still = await state(); const left = await pointer(0);
  check(moved(still, left) < .001, 'left-click on the field does not issue a movement order');
  const right = await pointer(2); check(moved(left, right) > .1, 'right-click ground moves the champion');
  await key('KeyS'); const stopped = await state(); await advance(.3);
  check(moved(stopped, await state()) < .001, 'Stop cancels the ongoing movement order');
  await pointer(); const beforeAttackMove = await state();
  await key('KeyQ'); check(moved(beforeAttackMove, await state()) > .1, 'remapped attack-move produces movement from one physical key press');
  await key('KeyS');
  await key('Escape'); await visible('pause');
  check(await page.evaluate(() => document.activeElement.id) === 'btn-resume', 'Escape opens a focused match menu');
  const paused = await state(); await pointer(2); await key('KeyG'); await key('KeyQ');
  const blocked = await state();
  check(moved(paused, blocked) < .001 && blocked.me.cd[0] === 0 && blocked.me.mn === paused.me.mn, 'menu suppresses right-click, remapped ability and attack-move inputs');
  await click('#btn-help'); await visible('help'); await shot('guide-live');
  await keyboard('Escape', 'keydown', '#help'); await keyboard('Escape', 'keyup', '#help');
  await visible('help', false); await visible('pause');
  check(true, 'closing the guide returns to the still-open match menu');
  await click('#btn-keys3'); await visible('keys');
  await keyboard('Escape', 'keydown', '#keys'); await keyboard('Escape', 'keyup', '#keys');
  await visible('keys', false); await visible('pause');
  check(true, 'Escape keybinding menu returns to the match menu');
  await click('#btn-fullscreen');
  await page.waitForFunction(() => !document.getElementById('btn-fullscreen').disabled);
  const fullscreen = await page.evaluate(() => ({ active: document.fullscreenElement === document.documentElement, message: document.getElementById('fullscreen-note').textContent }));
  report.fullscreen = fullscreen;
  check(fullscreen.active || fullscreen.message.includes('unavailable'), 'fullscreen either enters the document or explains browser refusal');
  if (fullscreen.active) {
    await click('#btn-keys3'); await visible('keys'); check(true, 'keybinding dialog remains visible inside actual fullscreen'); await click('#btn-keys-close');
    await click('#btn-resume'); await advance(.35); // Let the resized WebGL surface render.
    await pointer(); const fullscreenBefore = await state(); await key('KeyQ');
    check(moved(fullscreenBefore, await state()) > .1, 'mapped attack-move remains active in fullscreen');
    await key('KeyS'); await shot('fullscreen-live');
    await key('Escape'); await visible('pause');
    await shot('fullscreen-menu'); await click('#btn-fullscreen');
    await page.waitForFunction(() => !document.fullscreenElement);
  }
  await page.evaluate(() => { window.qaFullscreen = document.documentElement.requestFullscreen; document.documentElement.requestFullscreen = () => Promise.reject(new DOMException('QA controlled refusal', 'NotAllowedError')); });
  await click('#btn-fullscreen');
  await page.waitForFunction(() => !document.getElementById('btn-fullscreen').disabled && document.getElementById('fullscreen-note').textContent.includes('blocked'));
  check(await page.evaluate(() => !document.fullscreenElement), 'controlled fullscreen rejection leaves a readable, usable windowed menu');
  await page.evaluate(() => { document.documentElement.requestFullscreen = window.qaFullscreen; });
  await click('#btn-resume'); await visible('pause', false); await advance();
  await pointer(); const mana = (await state()).me.mn; await key('KeyG');
  await page.waitForFunction(mana => window.qaState().me.cd[0] > 0 || window.qaState().me.mn < mana, mana);
  check(true, 'fresh remapped ability press works after menu/fullscreen recovery');
  await shot('live');
  check(report.errors.length === 0, 'no uncaught errors or missing assets');
  report.passed = true;
}
main().catch(async error => {
  report.failure = error.stack; process.exitCode = 1; console.error(error);
  if (page) { try { report.state = await state(); await shot('failure'); } catch {} }
}).finally(async () => {
  await browser?.close(); await preview?.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true }); fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2)); console.log(JSON.stringify(report, null, 2));
});
