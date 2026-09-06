// Two real private clients: record draft card clicks, actual socket traffic,
// and HUD state. All events are synthetic DOM events in headless Edge.
'use strict';
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { startPreview } = require('./preview.cjs');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
const root = path.resolve(__dirname, '../..');
const out = path.join(root, process.env.LEAGUE_DRAFT_OUT || 'target/league-draft-race');
const started = Date.now();
const wasm = fs.readFileSync(path.join(root, 'web/pkg/league_bg.wasm'));
const glue = fs.readFileSync(path.join(root, 'web/pkg/league.js'));
const ui = fs.readFileSync(path.join(root, 'web/games/league/v2/ui.js'));
const report = { wasmSha256: crypto.createHash('sha256').update(wasm).digest('hex'),
  uiSha256: crypto.createHash('sha256').update(ui).digest('hex'), runs: [], errors: [] };
let browser, preview;
const now = () => Date.now() - started;
const snapshot = page => page.evaluate(() => ({
  state: window.qaState?.(), active: document.activeElement?.id || document.activeElement?.tagName,
  visibility: document.visibilityState, focused: document.hasFocus(),
  cards: [...document.querySelectorAll('#cards .card')].map(card => ({
    champ: card.dataset.c, disabled: card.disabled, selected: card.classList.contains('sel'),
    handler: typeof card.onclick, connected: card.isConnected,
  })),
  runes: [...document.querySelectorAll('#runebox .rune.on')].map(r => r.dataset.r),
  note: document.getElementById('draft-note').textContent,
  clickTrace: window.qaClicks, commandTrace: window.qaCommands,
}));
async function page(run, label) {
  const p = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  const trace = run[label] = { frames: [], steps: [], errors: [] };
  p.on('pageerror', error => trace.errors.push(error.message));
  p.on('console', msg => { if (msg.type() === 'error') trace.errors.push(msg.text()); });
  let sequence = 0;
  p.on('websocket', socket => {
    const id = ++sequence;
    for (const [event, direction] of [['framesent', 'out'], ['framereceived', 'in']]) {
      socket.on(event, ({ payload }) => {
        let data; try { data = JSON.parse(String(payload)); } catch { data = String(payload); }
        // Live snapshots are large; draft lifecycle messages are retained whole.
        if (data.t === 'state') data = { t: data.t, tick: data.tick, secs: data.secs };
        trace.frames.push({ at: now(), id, url: socket.url(), direction, data });
      });
    }
    socket.on('close', () => trace.frames.push({ at: now(), id, close: true }));
  });
  await p.addInitScript(() => {
    window.focus = () => {};
    Element.prototype.setPointerCapture = () => {};
    window.qaClicks = [];
    window.qaCommands = [];
    const stringify = JSON.stringify;
    JSON.stringify = function(value, ...args) {
      if (value?.pick) window.qaCommands.push({ at: performance.now(), command: structuredClone(value) });
      return stringify.call(this, value, ...args);
    };
    document.addEventListener('click', event => {
      const card = event.target.closest?.('#cards .card');
      if (!card) return;
      const record = { at: performance.now(), champ: card.dataset.c,
        disabled: card.disabled, focused: document.hasFocus(), visibility: document.visibilityState,
        active: document.activeElement?.id, before: window.qaState?.() };
      window.qaClicks.push(record);
      queueMicrotask(() => {
        record.selectedAfter = card.classList.contains('sel');
        record.runesAfter = [...document.querySelectorAll('#runebox .rune.on')].map(r => r.dataset.r);
      });
    }, true);
  });
  await p.route('**/pkg/league_bg.wasm', route => route.fulfill({ contentType: 'application/wasm', body: wasm }));
  await p.route('**/pkg/league.js', route => route.fulfill({ contentType: 'text/javascript', body: glue }));
  await p.route('**/games/league/v2/ui.js', route => route.fulfill({ contentType: 'text/javascript', body: ui }));
  await p.goto(`${preview.origin}/games/league/v2/`, { waitUntil: 'load' });
  await p.evaluate(async () => { window.qaWasm = await window.leagueReady;
    window.qaState = () => JSON.parse(window.qaWasm.state_json()); });
  return p;
}
async function click(p, selector) {
  await p.waitForFunction(selector => { const e = document.querySelector(selector);
    return e && !e.disabled && e.getBoundingClientRect().width > 0; }, selector, { timeout: 10000 });
  await p.evaluate(selector => document.querySelector(selector).click(), selector);
}
async function clickCard(p, run, label) {
  run[label].steps.push({ at: now(), action: 'card' });
  await click(p, '#cards [data-c="0"]');
}
async function warmup(count) {
  for (let index = 0; index < count; index++) {
    const run = {};
    const p = await page(run, 'practice');
    await click(p, '#btn-practice');
    await p.waitForFunction(() => window.qaState().connected && window.qaState().phase === 'select');
    await click(p, '#cards [data-c="1"]');
    await p.waitForFunction(() => window.qaState().roster[0].picked);
    await click(p, '#btn-start');
    await p.waitForFunction(() => window.qaState().phase === 'live' && window.qaState().me);
    await p.close();
  }
}

async function exercise(index) {
  const variants = ['standard', 'concurrent-clicks', 'host-early', 'guest-first', 'rAF-clicks',
    'host-input-focused', 'bindings-menu-roundtrip', 'host-cpu-throttle', 'parallel-page-init', 'standard-repeat'];
  const run = { index, variant: variants[index], started: now() };
  report.runs.push(run);
  let a, b;
  try {
    if (index === 8) [a, b] = await Promise.all([page(run, 'host'), page(run, 'guest')]);
    else { a = await page(run, 'host'); b = await page(run, 'guest'); }
    if (index === 7) {
      const session = await a.context().newCDPSession(a);
      await session.send('Emulation.setCPUThrottlingRate', { rate: 4 });
    }
    if (index === 6) { await click(a, '#btn-keys'); await click(a, '#btn-keys-close'); }
    const lobby = `draft-race-${process.pid}-${index}`;
    await a.evaluate(lobby => { document.getElementById('newlobby').value = lobby;
      document.getElementById('newmode').value = '1'; }, lobby);
    await click(a, '#btn-create');
    await a.waitForFunction(() => window.qaState().connected && window.qaState().roster.length === 2);
    if (index === 2) await clickCard(a, run, 'host');
    await click(b, '#btn-refresh');
    await b.waitForFunction(lobby => [...document.querySelectorAll('#lobbies li')].some(li =>
      li.querySelector('b')?.textContent === lobby && li.querySelector('button')), lobby);
    await b.evaluate(lobby => [...document.querySelectorAll('#lobbies li')].find(li =>
      li.querySelector('b')?.textContent === lobby).querySelector('button').click(), lobby);
    await b.waitForFunction(() => window.qaState().connected && window.qaState().slot === 1);
    if (index === 5) await a.evaluate(() => document.getElementById('pickd').focus());
    if (index === 1) await Promise.all([clickCard(a, run, 'host'), clickCard(b, run, 'guest')]);
    else if (index === 3) { await clickCard(b, run, 'guest'); await clickCard(a, run, 'host'); }
    else if (index === 4) {
      await Promise.all([a, b].map(p => p.evaluate(() => new Promise(resolve => requestAnimationFrame(() => {
        document.querySelector('#cards [data-c="0"]').click(); resolve();
      })))));
    } else { if (index !== 2) await clickCard(a, run, 'host'); await clickCard(b, run, 'guest'); }
    await Promise.all([a, b].map(p => p.waitForFunction(() =>
      window.qaState().roster.filter(r => r.picked).length === 2, null, { timeout: 6000 })));
    run.passed = true;
  } catch (error) {
    run.failure = error.stack;
    if (a) {
      await a.screenshot({ path: path.join(out, `failure-${index}-host.png`), fullPage: true }).catch(() => {});
      run.hostFailure = await snapshot(a).catch(error => ({ error: error.message }));
      // A single immediate retry distinguishes a lost first click from stale
      // socket state; it never changes the initial result of this trial.
      await a.evaluate(() => document.querySelector('#cards [data-c="0"]')?.click()).catch(() => {});
      try {
        await a.waitForFunction(() => window.qaState().roster.filter(r => r.picked).length === 2, null, { timeout: 2000 });
        run.recoveredBySecondHostClick = true;
      } catch { run.recoveredBySecondHostClick = false; }
    }
  } finally {
    if (a) run.hostFinal = await snapshot(a).catch(error => ({ error: error.message }));
    if (b) run.guestFinal = await snapshot(b).catch(error => ({ error: error.message }));
    await Promise.all([a, b].filter(Boolean).map(p => p.close()));
    run.ended = now();
    fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2));
    console.log(JSON.stringify({ trial: index, variant: run.variant, passed: !!run.passed,
      error: run.failure?.split('\n')[0], hostPicks: run.host?.frames.filter(f => f.direction === 'out' && f.data?.t === 'pick'),
      guestPicks: run.guest?.frames.filter(f => f.direction === 'out' && f.data?.t === 'pick'),
      recovered: run.recoveredBySecondHostClick }));
  }
}
async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out, { recursive: true });
  preview = await startPreview({ port: 8098, gamePort: 7798 });
  browser = await chromium.launch({ channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  if (process.env.LEAGUE_DRAFT_WARMUP) await warmup(Number(process.env.LEAGUE_DRAFT_WARMUP));
  for (let index = 0; index < 10; index++) await exercise(index);
  report.passed = report.runs.every(run => run.passed && !run.host.errors.length && !run.guest.errors.length);
  assert.equal(report.runs.length, 10);
  assert(report.passed, 'A draft trial failed; inspect results.json for clicks, commands and socket frames');
}
main().catch(error => { report.errors.push(error.stack); process.exitCode = 1; }).finally(async () => {
  await browser?.close(); await preview?.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ passed: report.passed, trials: report.runs.length, errors: report.errors, elapsedSeconds: report.elapsedSeconds }));
});
