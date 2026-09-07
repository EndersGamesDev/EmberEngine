// Matched real-practice views, with ordinary DOM input and the actual game clock.
// Run only when the shared browser/GPU is free and the candidate WASM is ready:
//   node tools/league/visual-v4.cjs [--version=both|v3|v4] [--champ=0..4] [--require-ui]
// --require-ui turns provisional overlay checks into a release gate.
'use strict';
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { startPreview } = require('./preview.cjs');

const root = path.resolve(__dirname, '../..');
const argument = name => process.argv.find(value => value.startsWith(`--${name}=`))?.split('=').slice(1).join('=');
const version = argument('version') || 'both';
const champion = argument('champ');
const requireUi = process.argv.includes('--require-ui');
assert(['both', 'v3', 'v4'].includes(version), 'Version must be both, v3 or v4');
assert(champion === undefined || /^[0-4]$/.test(champion), 'Champion must be 0–4');
for (const value of process.argv.slice(2)) assert(/^--(?:version=|champ=|require-ui$)/.test(value), `Unknown option ${value}`);
const champions = champion === undefined ? [0, 1, 2, 3, 4] : [Number(champion)];
const versions = version === 'both' ? ['v3', 'v4'] : [version];
const names = ['swarm', 'knight', 'hallow', 'maw', 'tessera'];
const baselinePkg = process.env.LEAGUE_BASELINE_PKG || 'C:/Users/end/dev/ember-league-launch/web/pkg';
const baselineHash = 'a71887d82711fc96bf1151a4dcf50c441a51a1a05664d82639a809dfb1222700';
const out = path.join(root, 'target/league-visual-v4');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
const started = Date.now();
const report = { kind: 'actual-practice-visual-comparison', configuredSeed: '0x1ea9e67b', mode: '3v3',
  seedSource: 'LocalGame::new called by start_local; default seed unchanged between both source builds',
  matching: 'Same champion, seed, viewport and world waypoints; real-clock input timing and bot states can differ.',
  stateManipulated: false, clockManipulated: false, requireUi, captures: [], errors: [], passed: false };
let browser, preview, page, current;

function save(file, value) { fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', 'utf8'); }
function check(ok, message) { assert(ok, message); current.checks.push(message); }
const state = () => page.evaluate(() => window.visualState());
async function live() {
  const snapshot = await state();
  assert(snapshot.connected && snapshot.phase === 'live' && snapshot.me?.alive, 'Capture requires a connected, living practice champion');
  assert(Date.now() - current.startedAt < 75000, 'Champion route exceeded its 75-second wall limit');
  return snapshot;
}
async function advance(seconds = .1) {
  const before = await live();
  await page.waitForFunction(secs => window.visualState().secs >= secs, before.secs + seconds, { timeout: 12000 });
}
async function click(selector) {
  await page.waitForFunction(selector => {
    const element = document.querySelector(selector);
    return element && !element.disabled && element.getClientRects().length;
  }, selector);
  await page.evaluate(selector => document.querySelector(selector).click(), selector);
}
async function key(code, seconds = .08) {
  for (const type of ['keydown', 'keyup']) {
    await page.evaluate(({ type, code }) => document.querySelector('#ember-root canvas').dispatchEvent(new KeyboardEvent(type, {
      code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code, bubbles: true, cancelable: true,
    })), { type, code });
    await advance(seconds);
  }
}
function project(snapshot, x, z, height, aspect) {
  const camera = snapshot.feedback?.camera || { x: snapshot.me.x, z: snapshot.me.z, height: 24, back: 16, fov: 40 };
  const length = Math.hypot(camera.height, camera.back), tangent = Math.tan(camera.fov * Math.PI / 360);
  const dx = x - camera.x, dz = z - camera.z;
  const depth = length - (camera.height * height + camera.back * dz) / length;
  const vertical = (camera.back * height - camera.height * dz) / length;
  return { xf: .5 + dx / (depth * tangent * aspect) * .5, yf: .5 - vertical / (depth * tangent) * .5 };
}
async function aim(x, z, button = null, height = 0) {
  const snapshot = await live();
  const aspect = await page.locator('#ember-root canvas').evaluate(c => c.clientWidth / c.clientHeight);
  const point = project(snapshot, x, z, height, aspect);
  assert(point.xf > .02 && point.xf < .98 && point.yf > .02 && point.yf < .98, 'Input target must be on the visible canvas');
  current.inputs.push({ secs: snapshot.secs, action: button === null ? 'aim' : 'right-click', world: [x, z], point });
  await page.evaluate(({ point, button }) => {
    const canvas = document.querySelector('#ember-root canvas'), rect = canvas.getBoundingClientRect();
    const init = { bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      clientX: rect.left + rect.width * point.xf, clientY: rect.top + rect.height * point.yf, button: -1, buttons: 0 };
    const move = new PointerEvent('pointermove', init);
    Object.defineProperty(move, 'getCoalescedEvents', { value: () => [move] }); canvas.dispatchEvent(move);
    if (button !== null) canvas.dispatchEvent(new PointerEvent('pointerdown', { ...init, button, buttons: 2 }));
  }, { point, button });
  await advance(.07);
  if (button !== null) {
    await page.evaluate(() => document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent('pointerup', {
      bubbles: true, pointerType: 'mouse', pointerId: 1, button: 2, buttons: 0,
    })));
    await advance(.05);
  }
}
async function walkTo(x, z) {
  const deadline = Date.now() + 18000;
  while (Date.now() < deadline) {
    const snapshot = await live(), dx = x - snapshot.me.x, dz = z - snapshot.me.z, distance = Math.hypot(dx, dz);
    if (distance < 1.6) return snapshot;
    const step = Math.min(9, distance);
    await aim(snapshot.me.x + dx / distance * step, snapshot.me.z + dz / distance * step, 2);
    await advance(.23);
  }
  throw new Error(`Ordinary movement did not reach (${x},${z})`);
}
function feedback(snapshot) {
  if (current.version !== 'v4') return;
  const data = snapshot.feedback;
  assert(data && Number.isSafeInteger(data.session) && data.session > 0, 'V4 feedback session is missing');
  assert(Array.isArray(data.events) && data.events.length <= 48, 'Feedback queue must be bounded');
  const ids = new Set();
  for (const event of data.events) {
    assert(Number.isSafeInteger(event.id) && event.id > 0 && !ids.has(event.id), 'Feedback IDs must be positive and unique per snapshot');
    ids.add(event.id);
    assert([event.sx, event.sy, event.age, event.left].every(Number.isFinite), 'Feedback projection/timing must be finite');
    assert(event.sx >= 0 && event.sx <= 1 && event.sy >= 0 && event.sy <= 1 && event.left > 0, 'Feedback must be visible and unexpired');
    const key = `${data.session}:${event.id}`, identity = [event.kind, event.unit, event.source, event.ability];
    if (current.eventIds.has(key)) assert.deepEqual(current.eventIds.get(key), identity, 'An existing ID must not be reused for another event');
    else current.eventIds.set(key, identity);
  }
  if (data.order) {
    assert(['move', 'attack_move', 'attack'].includes(data.order.kind), 'Unknown submitted-order marker');
    assert([data.order.x, data.order.z, data.order.age, data.order.left].every(Number.isFinite) && data.order.left > 0, 'Invalid order marker');
    assert(data.order.confirmed !== true, 'Submitted intent must not claim server confirmation');
  }
}
async function overlays() {
  if (current.version !== 'v4') return;
  const ui = await page.evaluate(() => {
    const selectors = ['combat-text', 'action-feedback', 'target-panel', 'impact-frame', 'order-status'];
    const nodes = [...document.querySelectorAll('#combat-text .combat-float')];
    return { missing: selectors.filter(id => !document.getElementById(id)),
      rows: nodes.map(e => ({ id: e.dataset.event, text: e.textContent.trim(), left: e.style.left, top: e.style.top })) };
  });
  if (ui.missing.length) {
    current.uiVerified = false; current.provisionalUi = ui.missing;
    assert(!requireUi, `Required feedback UI missing: ${ui.missing.join(', ')}`);
    return;
  }
  assert(new Set(ui.rows.map(row => row.id)).size === ui.rows.length, 'UI repeated an existing event ID');
  for (const row of ui.rows) {
    assert(/^\d+:\d+$/.test(row.id || '') && row.text, 'Empty or unidentified floating text');
    assert([parseFloat(row.left), parseFloat(row.top)].every(value => Number.isFinite(value) && value >= 0 && value <= 100), 'Floating text escaped the canvas');
  }
}
async function screenshot(label) {
  const snapshot = await live(); feedback(snapshot); await overlays();
  const file = path.join(out, `${current.version}-${current.name}-${label}.png`);
  await page.screenshot({ path: file });
  current.views.push({ label, file, sha256: hash(fs.readFileSync(file)), state: snapshot });
  save(path.join(out, `${current.version}-${current.name}.json`), serializable());
  console.log(`SHOT ${current.version} ${current.name} ${label}: ${file}`);
}
function serializable() { return { ...current, eventIds: [...current.eventIds], wallSeconds: (Date.now() - current.startedAt) / 1000 }; }
async function sample(label) {
  const result = await page.evaluate(() => new Promise(resolve => {
    const began = performance.now(), intervals = [], samples = []; let previous = began, lastSample = began;
    const observe = now => {
      intervals.push(now - previous); previous = now;
      if (now - lastSample >= 100) { samples.push(window.visualState()); lastSample = now; }
      if (now - began < 2200) requestAnimationFrame(observe);
      else resolve({ milliseconds: now - began, intervals, samples });
    };
    requestAnimationFrame(observe);
  }));
  for (const snapshot of result.samples) {
    assert(snapshot.connected && snapshot.phase === 'live' && snapshot.me?.alive, 'Frame sample lost the living practice champion');
    feedback(snapshot);
  }
  const ordered = result.intervals.slice(1).sort((a, b) => a - b);
  assert(ordered.length >= 10 && result.samples.at(-1).secs > result.samples[0].secs, 'Renderer/simulation did not advance');
  current.performance.push({ label, milliseconds: result.milliseconds, animationFrames: result.intervals.length,
    meanMs: ordered.reduce((a, b) => a + b, 0) / ordered.length, p95Ms: ordered[Math.floor((ordered.length - 1) * .95)],
    maxMs: ordered.at(-1), over50ms: ordered.filter(value => value > 50).length,
    measurement: 'Browser rAF intervals, not GPU timer queries', samples: result.samples });
  await overlays();
}
function enemies(snapshot) {
  return snapshot.units.filter(u => [0, 1, 2].includes(u[0]) && u[1] !== snapshot.me.team && u[4] > 0)
    .map(u => ({ x: u[2], z: u[3], distance: Math.hypot(u[2] - snapshot.me.x, u[3] - snapshot.me.z) }))
    .sort((a, b) => a.distance - b.distance);
}
async function fight(ability) {
  await walkTo(-8, 4);
  const deadline = Date.now() + 22000;
  while (Date.now() < deadline) {
    let snapshot = await live();
    if (snapshot.me.hp < snapshot.me.mh * .6 && snapshot.me.scd[1] <= 0) { await key('KeyF'); snapshot = await live(); }
    const enemy = enemies(snapshot)[0];
    if (enemy?.distance < 8 && snapshot.me.cd[ability] <= 0) {
      const target = current.champion === 2 ? { x: snapshot.me.x, z: snapshot.me.z } : enemy;
      await aim(target.x, target.z);
      await key(ability === 0 ? 'KeyQ' : 'KeyW', .05);
      const after = await live();
      if (after.me.cd[ability] > 0) {
        check(true, 'Learned ability accepted during actual lane combat');
        current.acceptedAbility = { ability, before: snapshot, after };
        await screenshot('ability-combat');
        await sample('combat');
        return;
      }
    }
    const point = enemy ? { x: enemy.x, z: enemy.z } : { x: Math.min(snapshot.me.x + 8, 8), z: 0 };
    const distance = Math.hypot(point.x - snapshot.me.x, point.z - snapshot.me.z), step = Math.min(8, distance);
    await aim(snapshot.me.x + (point.x - snapshot.me.x) / Math.max(distance, .01) * step,
      snapshot.me.z + (point.z - snapshot.me.z) / Math.max(distance, .01) * step);
    await key('KeyA'); await advance(.25);
  }
  throw new Error('No accepted live combat cast reached within the finite route');
}
async function capture(gameVersion, id, bundles) {
  current = { version: gameVersion, champion: id, name: names[id], startedAt: Date.now(), checks: [], inputs: [],
    views: [], performance: [], errors: [], eventIds: new Map(), uiVerified: gameVersion === 'v4', bundles, passed: false };
  report.captures.push(current);
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, deviceScaleFactor: 1 });
  page.setDefaultTimeout(15000);
  page.on('pageerror', error => current.errors.push(error.message));
  page.on('response', response => { if (response.status() >= 400) current.errors.push(`HTTP ${response.status()}: ${response.url()}`); });
  await page.addInitScript(() => { window.focus = () => {}; Element.prototype.setPointerCapture = () => {}; });
  await page.goto(`${preview.origin}/games/league/${gameVersion}/`);
  await page.evaluate(async () => { window.visualWasm = await window.leagueReady; window.visualState = () => JSON.parse(window.visualWasm.state_json()); });
  check(await page.evaluate(() => window.visualWasm.proto_version()) === 2, 'Correct protocol-2 client loaded');
  await click('#btn-practice3');
  await page.waitForFunction(() => window.visualState().connected && window.visualState().phase === 'select');
  await click(`#cards [data-c="${id}"]`);
  await page.waitForFunction(id => window.visualState().roster[0].picked && window.visualState().roster[0].champ === id, id);
  await click('#btn-start'); await page.waitForFunction(() => window.visualState().phase === 'live' && window.visualState().me?.alive);
  await advance(.25); current.spawn = await live();
  check(current.spawn.me.champ === id && current.spawn.roster.length === 6, 'Real selected champion and 3v3 roster');
  await screenshot('base'); await sample('base');
  await aim(current.spawn.me.x + 4, current.spawn.me.z);
  await key('KeyQ');
  const unavailable = await live();
  if (gameVersion === 'v4') {
    feedback(unavailable);
    check(unavailable.feedback.unavailable?.slot === 0 && /Learn Q first/.test(unavailable.feedback.unavailable.text), 'Unlearned cast explains the actual unavailable condition');
    check(unavailable.me.cd[0] === 0 && !unavailable.feedback.events.some(e => e.kind === 'cast' && e.source === unavailable.me.uid), 'Unavailable cast does not invent acceptance');
  }
  const ability = id === 4 ? 1 : 0;
  await click(`#abils [data-abil="${ability}"] .up`);
  await page.waitForFunction(ability => window.visualState().me.rk[ability] === 1, ability);
  const before = await live(); await aim(before.me.x + 8, before.me.z, 2);
  const ordered = await live();
  if (gameVersion === 'v4') check(ordered.feedback.order?.kind === 'move' && ordered.feedback.order.target === 0, 'Ground click records submitted move intent');
  await walkTo(-30, 0); await aim(-20, 0, 2); await advance(.25); await screenshot('midlane-approach');
  await walkTo(-18, 12); await walkTo(-6, 14); await key('KeyS'); await advance(.25);
  const court = await live();
  check(Math.hypot(court.me.x, court.me.z - 16) < 9, 'Champion genuinely walked to the North Court');
  await screenshot('court');
  await fight(ability);
  check(current.errors.length === 0, 'No browser errors or missing assets');
  current.passed = true; save(path.join(out, `${gameVersion}-${names[id]}.json`), serializable());
  await page.close(); page = null;
}
async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW); fs.mkdirSync(out, { recursive: true });
  const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
  browser = await chromium.launch({ channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  for (const gameVersion of versions) {
    const pkgDirectory = gameVersion === 'v3' ? baselinePkg : path.join(root, 'web/pkg');
    const bundles = ['league.js', 'league_bg.wasm'].map(name => {
      const bytes = fs.readFileSync(path.join(pkgDirectory, name)); return { name, bytes: bytes.length, sha256: hash(bytes) };
    });
    if (gameVersion === 'v3') assert.equal(bundles[1].sha256, baselineHash, 'Frozen comparison baseline was replaced');
    preview = await startPreview({ port: 8108, gameVersion, protocol: 2, online: false, pkgDirectory });
    for (const id of champions) await capture(gameVersion, id, bundles);
    await preview.close(); preview = null;
  }
  report.passed = true;
}
main().catch(async error => {
  report.errors.push(error.stack); console.error(error); process.exitCode = 1;
  if (current) current.failure = error.message;
  if (page) { try { await page.screenshot({ path: path.join(out, 'failure.png') }); } catch {} }
}).finally(async () => {
  await browser?.close(); await preview?.close();
  report.captures = report.captures.map(capture => ({ ...capture, eventIds: [...capture.eventIds] }));
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  save(path.join(out, `results-${version}-${champion ?? 'all'}.json`), report);
  console.log(`Visual route finished in ${report.elapsedSeconds.toFixed(3)} seconds; passed=${report.passed}`);
});
