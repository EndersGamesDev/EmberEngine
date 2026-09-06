// Renderer fixture only: all server messages, targets and projectiles are
// authored below. This does not exercise matchmaking, combat rules or damage.
// --protocol-only checks the transport/fixture against a preliminary WASM.
'use strict';

const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { startPreview } = require('./preview.cjs');

const root = path.resolve(__dirname, '../..');
const protocolOnly = process.argv.includes('--protocol-only');
const out = path.join(root, 'target/league-combat-gallery', protocolOnly ? 'protocol-smoke' : 'capture');
const wsUrl = 'ws://combat-fixture.invalid/';
const names = ['SW4RM', 'EmberKnight', 'The Hallow One', 'Bog Maw', 'Tessera'];
const positions = [-8, -4, 0, 4, 8];
const started = Date.now();
const report = {
  kind: 'mocked-renderer-fixture', protocolOnly, gameplayVerified: false,
  description: 'Server-shaped snapshots and FX are authored fixtures; no real combat or server is run.',
  ageConvention: 'Browser clock milliseconds since queueing the FX snapshot; rendering is frame-quantized.',
  checks: [], errors: [], console: [], captures: [], comparisons: [], incoming: [],
};
const hash = data => crypto.createHash('sha256').update(data).digest('hex');
let browser, preview, page, socket, current, tick = 0;

function check(ok, name) {
  assert(ok, name);
  report.checks.push(name);
  console.log('PASS ' + name);
}

const roster = Array.from({ length: 6 }, (_, slot) => ({
  slot, team: slot < 3 ? 0 : 1, handle: names[slot] || 'offstage bot',
  bot: slot !== 2, champ: slot % 5, picked: true, d: 0, f: 1,
  runes: [0, 1, 2], connected: slot === 2,
}));

const champions = roster.map(row => ({
  slot: row.slot, team: row.team, alive: row.slot < 5, resp: row.slot < 5 ? 0 : 99,
  level: 12, points: 0, ranks: [3, 3, 3, 3], cds: [0, 0, 0, 0], scds: [0, 0],
  gold: 500, items: [0, 0, 0, 0, 0, 0], charges: [0, 0, 0, 0, 0, 0], d: 0, f: 1,
}));

function units() {
  const rows = roster.map(row => ({
    id: row.slot + 1, k: 0, t: row.team, slot: row.slot, def: row.champ,
    x: positions[row.slot] ?? 30, z: row.slot < 5 ? 0 : 30, fa: -Math.PI / 2,
    hp: row.slot < 5 ? 1000 : 0, mh: 1000, lv: 12, xp: 0, xpn: 1000,
    mn: 600, mm: 600, g: 500, pt: 0, rk: [3, 3, 3, 3], cd: [0, 0, 0, 0],
    scd: [0, 0], items: [0, 0, 0, 0, 0, 0], charges: [0, 0, 0, 0, 0, 0],
    dead: row.slot === 5, resp: row.slot === 5 ? 99 : 0, tp: 0,
  }));
  for (let champ = 0; champ < 5; champ++) rows.push({
    id: 100 + champ, k: 1, t: champ < 3 ? 1 : 0, slot: 255,
    x: positions[champ], z: -5, fa: Math.PI / 2, hp: 500, mh: 500,
  });
  return rows;
}

function snapshot(ability = null) {
  const result = {
    t: 'state', tick: 0, secs: 100, units: units(), champs: champions, buffs: [],
    projs: [], zones: [], kills: [0, 0], boon: [0, 0], boon_left: [0, 0],
    court_respawn: [0, 0], fx: [], log: [],
  };
  if (ability === null) return result;
  result.fx = positions.map((x, champ) => ({
    k: ability === 4 ? 0 : 13, champ, ability, x, z: 0, x2: x, z2: -5,
    v: ability === 4 ? 4 : 0,
  }));
  const projectile = (id, k, champ, x, z) => ({ id, k, t: champ < 3 ? 0 : 1, champ, x, z, dx: 0, dz: -1 });
  if (ability === 0) {
    result.projs.push(projectile(201, 1, 0, -8.35, -1.8), projectile(202, 1, 0, -8, -2.3),
      projectile(203, 1, 0, -7.65, -1.8), projectile(204, 3, 3, 4, -2.5), projectile(205, 2, 4, 8, -2.5));
    result.zones.push({ k: 0, x: -4, z: -4, r: 1.5 });
  }
  if (ability === 1) {
    result.fx.push({ k: 1, champ: 0, ability: 1, x: -8, z: 0, x2: -8, z2: -5, v: 0 });
    result.zones.push({ k: 3, x: 4, z: 0, r: 1.65 }, { k: 1, x: 8, z: -4, r: 1.25 });
  }
  if (ability === 3) result.zones.push({ k: 2, x: 8, z: -4, r: 1.65 });
  if (ability === 4) result.projs.push(projectile(301, 0, 0, -8, -2.3),
    projectile(302, 0, 2, 0, -2.3), projectile(303, 0, 4, 8, -2.3));
  return result;
}

function send(message) {
  assert(socket, 'Fixture WebSocket has not connected');
  socket.send(JSON.stringify(message));
}

function sendState(fx = []) {
  send({ ...current, tick: ++tick, fx });
}

// Snapshot time stays at 100 seconds to freeze idle/environment animation.
// Only transient FX lifetime and attack-pose age advance with the fake clock.
// Delivery cadence matches the real server's 20 Hz snapshot cadence.
async function advance(milliseconds) {
  for (let remaining = milliseconds; remaining > 0;) {
    const step = Math.min(50, remaining);
    sendState();
    await page.clock.runFor(step);
    remaining -= step;
  }
}

async function caption(text) {
  await page.evaluate(text => { document.getElementById('qa-gallery-title').textContent = text; }, text);
}

async function capture(name, ageMs) {
  const file = path.join(out, name + '.png');
  const canvas = await page.evaluate(() => {
    const r = document.querySelector('#ember-root canvas').getBoundingClientRect();
    return { x: r.x, y: r.y, width: r.width, height: r.height };
  });
  const pixels = await page.screenshot({ path: file });
  const scene = await page.screenshot({ clip: canvas });
  const entry = { name, file, ageMs, sha256: hash(pixels), sceneSha256: hash(scene), canvas };
  report.captures.push(entry);
  return entry;
}

async function setup() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out, { recursive: true });
  const wasm = fs.readFileSync(path.join(root, 'web/pkg/league_bg.wasm'));
  report.wasm = { bytes: wasm.length, sha256: hash(wasm) };
  preview = await startPreview({ port: 8097, online: false });
  const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
  browser = await chromium.launch({ channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  page = await browser.newPage({ viewport: { width: 1920, height: 1120 } });
  assert(typeof page.routeWebSocket === 'function', 'Bundled Playwright has no routeWebSocket API');
  assert(page.clock, 'Bundled Playwright has no Clock API');
  page.setDefaultTimeout(20000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('console', message => { if (message.type() === 'error') report.console.push(message.text()); });
  await page.addInitScript(() => {
    window.focus = () => {};
    Element.prototype.setPointerCapture = () => {};
  });
  await page.clock.install({ time: new Date('2026-09-06T12:00:00Z') });
  await page.routeWebSocket(wsUrl, route => {
    socket = route;
    // Deliberately never connectToServer(): .invalid is an intercepted fixture.
    route.onMessage(raw => {
      try {
        const message = JSON.parse(String(raw));
        report.incoming.push(message.t);
        if (message.t === 'hello') {
          report.proto = message.proto;
          send({ t: 'welcome', proto: message.proto, host: 'MOCKED renderer fixture',
            version: 'fixture', commit: 'mocked', players: 1, lobbies: 1 });
        } else if (message.t === 'join_lobby' || message.t === 'create_lobby') {
          send({ t: 'joined', lobby: 'MOCKED renderer gallery', id: 2, mode: 3, roster });
          send({ t: 'phase', phase: 'live', left: 0 });
          current = snapshot();
          sendState();
        } else if (message.t === 'ping') send({ t: 'pong', nonce: message.nonce });
      } catch (error) { report.errors.push('Fixture message: ' + error.stack); }
    });
  });
  await page.goto(preview.origin + '/games/league/v2/', { waitUntil: 'load' });
  await page.waitForFunction(() => document.body.dataset.leagueBoot === 'ready');
  await page.evaluate(async ws => {
    window.qaWasm = await window.leagueReady;
    window.qaState = () => JSON.parse(window.qaWasm.state_json());
    for (const id of ['menu', 'draft', 'stage-tools', 'status', 'hud']) document.getElementById(id).classList.add('hidden');
    document.querySelector('header').classList.add('hidden');
    const title = document.createElement('h2');
    title.id = 'qa-gallery-title';
    title.textContent = 'MOCKED renderer fixture · protocol setup';
    title.style.cssText = 'font:20px system-ui;margin:10px 0 4px;color:#f1ddb1';
    document.body.prepend(title);
    const legend = document.createElement('p');
    legend.textContent = 'Left → right: SW4RM · EmberKnight · The Hallow One · Bog Maw · Tessera. Authored targets and snapshots; not gameplay proof.';
    legend.style.cssText = 'font:15px system-ui;margin:0 0 8px;color:#acb9cf';
    title.after(legend);
    const stage = document.getElementById('stage');
    stage.classList.remove('hidden', 'drafting');
    stage.style.width = '1760px';
    stage.style.maxWidth = '96vw';
    document.body.style.paddingBottom = '0';
    window.qaWasm.set_input_enabled(false);
    window.qaWasm.start_online(JSON.stringify({ ws, handle: 'renderer fixture', lobby: 'mocked', create: false, mode: 3 }));
  }, wsUrl);
  await page.waitForFunction(() => window.qaState().phase === 'live' && window.qaState().connected && window.qaState().me?.slot === 2);
  const now = await page.evaluate(() => Date.now());
  await page.clock.pauseAt(now + 1000);
  await advance(1800);
  const state = await page.evaluate(() => window.qaState());
  check(state.champs.filter(champ => champ.alive).length === 5, 'server-shaped snapshot renders five living champion slots');
  check(state.units.filter(unit => unit[0] === 0 && unit[4] > 0).map(unit => unit[2]).join(',') === positions.join(','),
    'the five champion positions match the authored gallery row');
  check(report.incoming.includes('hello') && report.incoming.includes('join_lobby'), 'real WASM completes Hello and JoinLobby over the mocked WebSocket');
  check(state.me.x === 0 && state.me.z === 0, 'camera follows the center champion in slot two');
}

async function gallery() {
  await caption(protocolOnly ? 'MOCKED renderer fixture · preliminary protocol smoke' : 'MOCKED renderer fixture · five champions at rest');
  await capture('baseline', 0);
  if (protocolOnly) return;
  for (const [ability, name] of ['q', 'w', 'e', 'r', 'auto'].entries()) {
    // Phase reset is an existing wire operation that drains prior transient FX.
    send({ t: 'phase', phase: 'select', left: 60 });
    send({ t: 'phase', phase: 'live', left: 0 });
    current = snapshot(ability);
    fs.writeFileSync(path.join(out, name + '-snapshot.json'), JSON.stringify(current, null, 2) + '\n');
    sendState(current.fx);
    await advance(64);
    await caption(`MOCKED renderer fixture · ${name.toUpperCase()} · age 64 ms`);
    const early = await capture(name + '-early', 64);
    await advance(144);
    await caption(`MOCKED renderer fixture · ${name.toUpperCase()} · age 208 ms`);
    const late = await capture(name + '-late', 208);
    const changed = early.sceneSha256 !== late.sceneSha256;
    report.comparisons.push({ ability: name, changed, early: early.name, late: late.name,
      idleTimeFrozen: true, mockedProjectilePositions: true });
    check(changed, `${name.toUpperCase()}: canvas pixels change between two controlled effect ages`);
  }
  const pairs = report.comparisons.map(pair => `<section><h2>${pair.ability.toUpperCase()}</h2><div class="pair">${[pair.early, pair.late].map(name => `<a href="${name}.png"><img src="${name}.png" alt="${name}"></a>`).join('')}</div></section>`).join('');
  fs.writeFileSync(path.join(out, 'gallery.html'), `<!doctype html><meta charset="utf-8"><title>Mocked League renderer gallery</title><style>body{background:#0b0e13;color:#e8e0cc;font:16px system-ui;margin:24px}.pair{display:grid;grid-template-columns:1fr 1fr;gap:12px}img{width:100%}a{color:inherit}</style><h1>Mocked renderer gallery</h1><p>Actual WASM; authored server messages. Each pair shows effect ages 64 and 208 ms with idle time frozen. This is visual fixture coverage, not gameplay proof.</p>${pairs}`);
}

async function main() {
  await setup();
  await gallery();
  check(report.errors.length === 0, 'no uncaught browser or fixture protocol errors');
  check(report.console.length === 0, 'no browser console errors');
  report.passed = true;
}

main().catch(error => {
  report.failure = error.stack;
  console.error(error);
  process.exitCode = 1;
}).finally(async () => {
  if (report.failure && page) {
    try { await capture('failure', null); report.state = await page.evaluate(() => window.qaState?.()); }
    catch (error) { report.diagnostic = String(error); }
  }
  if (browser) await browser.close();
  if (preview) await preview.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
});
