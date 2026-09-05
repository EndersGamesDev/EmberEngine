// Real-server protocol/handling smoke in a disposable one-player lobby.
// Node 22+ built-in WebSocket; no browser, OS input, or asset fixture.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const url = process.argv[2];
if (!url || !/^wss?:\/\//.test(url)) throw new Error('Usage: node tools/v24/network-smoke.cjs ws[s]://host');
const proto = 19;
const started = Date.now();
const report = { url, proto, phases: [], errors: [], shots: 0 };
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
const until = async (predicate, label, timeout = 8000) => {
  const began = Date.now();
  while (!predicate()) {
    if (report.errors.length) throw new Error(report.errors.join('; '));
    if (Date.now() - began > timeout) throw new Error(`Timed out: ${label}`);
    await wait(20);
  }
};
let ws, id, latest, phase, timer, seq = 0;
const snapshots = [];
const input = overrides => ({ t: 'input', seq: ++seq, view_tick: 0, mx: 0, my: 0,
  ax: 1, az: 0, pitch: 0.2, fire: false, sprint: false, crouch: false,
  reload: false, jump: false, shield: false, melee: false, ads: false, ...overrides });
async function runPhase(name, overrides, ms) {
  const firstSeq = seq + 1;
  const records = [];
  phase = { name, firstSeq, records };
  ws.send(JSON.stringify(input(overrides)));
  timer = setInterval(() => ws.send(JSON.stringify(input({ ...overrides, jump: false }))), 50);
  await wait(ms);
  await until(() => records.length >= 5, `${name} snapshots`);
  clearInterval(timer); timer = null;
  const last = records.at(-1);
  report.phases.push({ name, states: records.length, first: records[0], last,
    sawPartialAds: records.some(p => p.ads_fraction > 0 && p.ads_fraction < 1),
    peakSpread: Math.max(...records.map(p => p.spread)) });
  return report.phases.at(-1);
}
async function rejectOldProtocol() {
  const old = new WebSocket(url);
  let error, joined = false;
  old.onopen = () => {
    old.send(JSON.stringify({ t: 'hello', proto: 17, handle: 'old-proto-check' }));
    old.send(JSON.stringify({ t: 'create_lobby', name: `old-v24-${Date.now()}`, password: null, map: 'freight-yard', mode: 'ffa' }));
  };
  old.onmessage = event => {
    const m = JSON.parse(event.data);
    if (m.t === 'error') error = m.message;
    if (m.t === 'game_joined') joined = true;
  };
  try {
    await until(() => error || joined, 'old protocol rejection');
    assert(!joined && /protocol|version|reload/i.test(error), `Stale client not rejected: ${error}`);
    report.oldProtocolRejected = error;
  } finally { old.close(); }
}
async function main() {
  ws = new WebSocket(url);
  ws.onopen = () => {
    ws.send(JSON.stringify({ t: 'hello', proto, handle: 'v24-handling-check' }));
    ws.send(JSON.stringify({ t: 'create_lobby', name: `ads-${Date.now()}`, password: randomUUID(), map: 'freight-yard', mode: 'ffa' }));
  };
  ws.onerror = () => report.errors.push('WebSocket transport failed');
  ws.onmessage = event => {
    const m = JSON.parse(event.data);
    if (m.t === 'welcome') report.welcome = m;
    if (m.t === 'game_joined') id = m.id;
    if (m.t === 'error') report.errors.push(m.message);
    if (m.t === 'shot') report.shots++;
    if (m.t === 'state' && id !== undefined) {
      const p = m.players.find(p => p.id === id);
      if (!p) return;
      latest = { tick: m.tick, ...p };
      snapshots.push(latest);
      if (phase && p.ack >= phase.firstSeq) phase.records.push(latest);
    }
  };
  await until(() => latest, 'game join');
  assert.equal(report.welcome.proto, proto);
  assert.equal(latest.weapon, 1, 'Spawn must use the sidearm');
  assert(Number.isFinite(latest.ads_fraction) && Number.isFinite(latest.spread) && Number.isFinite(latest.recoil_bloom), 'Missing authoritative handling state');
  const hip = await runPhase('standing hip', {}, 650);
  const ads = await runPhase('standing ADS', { ads: true }, 750);
  const crouch = await runPhase('crouched ADS', { ads: true, crouch: true }, 750);
  assert(hip.last.spread > ads.last.spread && ads.last.spread > crouch.last.spread && crouch.last.spread > 0,
    'Accuracy must improve from hip to ADS to crouched ADS');
  assert(ads.last.ads_fraction > 0.99, 'ADS did not settle');
  // A tunnel may batch away the short transition; core tests pin its duration.
  report.sawPartialAds = ads.sawPartialAds;
  const fire = await runPhase('hip fire', { fire: true }, 720);
  const recover = await runPhase('recover', {}, 1200);
  assert(report.shots > 0, 'No authoritative shots received');
  assert(fire.peakSpread > recover.last.spread, 'Spread did not recover after release');
  // Freight Yard spawn backlots leave the first metre toward centre clear.
  const norm = Math.hypot(latest.x, latest.z) || 1;
  const direction = { mx: -latest.x / norm, my: -latest.z / norm };
  const walk = await runPhase('walk', direction, 700);
  const low = await runPhase('crouch walk', { ...direction, crouch: true }, 700);
  const speed = phase => Math.hypot(phase.last.x - phase.first.x, phase.last.z - phase.first.z) / ((phase.last.tick - phase.first.tick) / 60);
  report.walkSpeed = speed(walk);
  report.crouchSpeed = speed(low);
  assert(Math.abs(report.walkSpeed - 2) < 0.08, `Ground walk ${report.walkSpeed} m/s`);
  assert(Math.abs(report.crouchSpeed - 1.1) < 0.08, `Crouch walk ${report.crouchSpeed} m/s`);
  const jump = await runPhase('jump', { ...direction, jump: true }, 500);
  assert(jump.last.y > 0.1 || jump.first.y > 0.1, 'Jump did not leave ground');
  report.passed = true;
  await rejectOldProtocol();
}
main().catch(error => { report.passed = false; report.errors.push(error.stack); process.exitCode = 1; }).finally(() => {
  if (timer) clearInterval(timer);
  if (ws) ws.close();
  report.elapsedMs = Date.now() - started;
  report.stateCount = snapshots.length;
  if (process.env.EMBER_QA_OUTPUT) {
    fs.mkdirSync(process.env.EMBER_QA_OUTPUT, { recursive: true });
    fs.writeFileSync(path.join(process.env.EMBER_QA_OUTPUT, 'network-results.json'), JSON.stringify(report, null, 2) + '\n');
  }
  console.log(JSON.stringify(report, null, 2));
});
