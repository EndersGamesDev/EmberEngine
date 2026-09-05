// Real-server harbor checks. Only private disposable lobbies and their peers
// are touched; no geometry imitation, browser, OS input, or server control.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const url = process.argv[2];
if (!url || !/^wss?:\/\//.test(url)) throw new Error('Usage: node tools/v25/network-harbor.cjs ws[s]://host');
const proto = 20;
const started = Date.now();
const output = path.resolve(process.env.EMBER_QA_OUTPUT || 'target/harbor-network');
const report = { purpose: 'Real server, private eight-player lobbies; not a load/latency benchmark or complete route proof',
  url, proto, map: 'harbor', modes: [], phases: [], errors: [], shots: 0 };
const peers = [];
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
async function until(predicate, label, timeout = 10000) {
  const began = Date.now();
  while (!predicate()) {
    if (report.errors.length) throw new Error(report.errors.join('; '));
    if (Date.now() - began > timeout) throw new Error(`Timed out: ${label}`);
    await wait(20);
  }
}
class Peer {
  constructor(handle, version = proto) {
    this.socket = new WebSocket(url);
    this.seq = 0; this.states = []; this.phase = null; this.closed = false;
    this.socket.onopen = () => this.send({ t: 'hello', proto: version, handle });
    this.socket.onerror = () => {
      // Node may report the peer's shutdown response as a transport error
      // after our deliberate LeaveLobby/close. It is not a failed test phase.
      if (!this.closed) report.errors.push(`Transport failed for ${handle}`);
    };
    this.socket.onmessage = event => {
      const m = JSON.parse(event.data);
      if (m.t === 'welcome') this.welcome = m;
      if (m.t === 'game_joined') { this.joined = m; this.id = m.id; }
      if (m.t === 'error') {
        this.error = m.message;
        if (!this.expectedError) report.errors.push(`${handle}: ${m.message}`);
      }
      if (m.t === 'shot' && this.countShots) report.shots++;
      if (m.t === 'state') {
        this.state = m;
        const mine = m.players.find(p => p.id === this.id);
        if (mine) {
          this.latest = { tick: m.tick, ...mine };
          this.states.push(this.latest);
          if (this.phase && mine.ack >= this.phase.firstSeq) this.phase.records.push(this.latest);
        }
      }
    };
    peers.push(this);
  }
  send(packet) { this.socket.send(JSON.stringify(packet)); }
  input(overrides = {}) {
    this.send({ t: 'input', seq: ++this.seq, view_tick: this.state?.tick || 0,
      mx: 0, my: 0, ax: 1, az: 0, pitch: 1.2, fire: false,
      sprint: false, crouch: false, reload: false, jump: false,
      shield: false, melee: false, ads: false, ...overrides });
  }
  close() {
    if (this.closed) return;
    this.closed = true;
    clearInterval(this.timer);
    if (this.socket.readyState === WebSocket.OPEN) this.send({ t: 'leave_lobby' });
    this.socket.close();
  }
}
function rosterCheck(packet, mode) {
  assert.equal(packet.players.length, 8, `${mode}: eight players must be present`);
  const ids = new Set(packet.players.map(p => p.id));
  assert.equal(ids.size, 8, `${mode}: duplicate player ID`);
  for (const p of packet.players) {
    assert([p.x, p.y, p.z, p.vy, p.ads_fraction, p.spread, p.recoil_bloom].every(Number.isFinite), 'Nonfinite state');
    assert(p.alive && p.hp > 0, `${mode}: spawn must be alive`);
    assert.equal(p.weapon, 1, 'Starter weapon must be the sidearm');
  }
  let minimumSeparation = Infinity;
  for (let i = 0; i < packet.players.length; i++) {
    for (let j = i + 1; j < packet.players.length; j++) {
      const a = packet.players[i], b = packet.players[j];
      minimumSeparation = Math.min(minimumSeparation, Math.hypot(a.x - b.x, a.z - b.z));
    }
  }
  assert(minimumSeparation >= 1.2, `${mode}: players overlap at spawn (${minimumSeparation} m)`);
  const teamCounts = [0, 1].map(team => packet.players.filter(p => p.team === team).length);
  if (mode === 'tdm') assert.deepEqual(teamCounts, [4, 4], 'TDM must place four players on each team');
  return { minimumSeparation, teamCounts, players: packet.players };
}
async function phase(peer, name, input, milliseconds) {
  const records = [];
  peer.phase = { firstSeq: peer.seq + 1, records };
  peer.input(input);
  peer.timer = setInterval(() => peer.input({ ...input, jump: false }), 50);
  try {
    await wait(milliseconds);
    await until(() => records.length >= 6, `${name}: six observed states`);
  } finally {
    clearInterval(peer.timer); peer.timer = null; peer.phase = null;
    peer.input();
  }
  const first = records[0], last = records.at(-1);
  const elapsedTicks = last.tick - first.tick;
  assert(elapsedTicks > 0, 'Measured phase needs distinct server ticks');
  const measured = { name, stateCount: records.length, first, last, elapsedTicks,
    speed: Math.hypot(last.x - first.x, last.z - first.z) / (elapsedTicks / 60),
    partialAds: records.some(p => p.ads_fraction > 0 && p.ads_fraction < 1),
    peakSpread: Math.max(...records.map(p => p.spread)), maxHeight: Math.max(...records.map(p => p.y)) };
  report.phases.push(measured);
  return measured;
}
async function walkTo(peer, target, label) {
  assert(Array.isArray(target) && target.length === 2 && target.every(Number.isFinite), 'Bad route waypoint');
  const first = { ...peer.latest };
  const started = Date.now();
  let samples = 0, farthest = Math.max(Math.abs(first.x), Math.abs(first.z));
  const steer = () => {
    const p = peer.latest;
    const dx = target[0] - p.x, dz = target[1] - p.z, distance = Math.hypot(dx, dz);
    // The last 0.6m slows proportionally to avoid tunnel-latency overshoot.
    const strength = Math.min(1, distance / 0.6);
    peer.input({ mx: distance ? dx / distance * strength : 0, my: distance ? dz / distance * strength : 0 });
    farthest = Math.max(farthest, Math.abs(p.x), Math.abs(p.z)); samples++;
  };
  steer(); peer.timer = setInterval(steer, 50);
  try { await until(() => Math.hypot(peer.latest.x - target[0], peer.latest.z - target[1]) < 0.25, label, 18000); }
  finally { clearInterval(peer.timer); peer.timer = null; peer.input(); }
  return { label, target, first, last: peer.latest, samples, farthestAxis: farthest, elapsedMs: Date.now() - started };
}
async function handlingAndMovement(peer) {
  // Hand-authored spawn-pocket exit supplied by harbor.rs's author. These
  // are input waypoints only; the real server still owns all collision.
  const approach = process.env.EMBER_QA_APPROACH ? JSON.parse(process.env.EMBER_QA_APPROACH)
    : [[-37, 45.4], [-31, 45.4]];
  report.approach = [];
  for (const [i, target] of approach.entries()) report.approach.push(await walkTo(peer, target, `spawn exit ${i + 1}`));
  peer.countShots = true;
  const hip = await phase(peer, 'hip', {}, 550);
  const ads = await phase(peer, 'ADS', { ads: true }, 650);
  const crouch = await phase(peer, 'crouched ADS', { ads: true, crouch: true }, 650);
  assert(hip.last.spread > ads.last.spread && ads.last.spread > crouch.last.spread && crouch.last.spread > 0, 'ADS/crouch precision ordering');
  assert(ads.last.ads_fraction > 0.99, 'ADS did not finish raising');
  report.partialAdsObserved = ads.partialAds;
  const fire = await phase(peer, 'hip fire', { fire: true }, 720);
  const recover = await phase(peer, 'recovery', {}, 1200);
  assert(report.shots > 0, 'No server Shot event');
  assert(fire.peakSpread > recover.last.spread, 'Firing spread did not recover');
  // Authored route contract, not a local collision implementation. The
  // harbor's cleared x=-31 outer lane runs south; verify from states.
  const direction = process.env.EMBER_QA_DIRECTION ? JSON.parse(process.env.EMBER_QA_DIRECTION) : [0, -1];
  assert(direction.length === 2 && direction.every(Number.isFinite) && Math.abs(Math.hypot(...direction) - 1) < 1e-6, 'Direction must be a unit XZ vector');
  const movement = { mx: direction[0], my: direction[1] };
  for (const [name, flags, expected] of [['walk', {}, 4], ['crouch walk', { crouch: true }, 2.2], ['sprint', { sprint: true }, 6.4]]) {
    const sample = await phase(peer, name, { ...movement, ...flags }, 700);
    assert(Math.abs(sample.speed - expected) < 0.16, `${name}: ${sample.speed} m/s, expected ${expected}`);
    assert(sample.maxHeight < 0.1, `${name}: speed measurement must remain grounded`);
  }
  const route = process.env.EMBER_QA_ROUTE ? JSON.parse(process.env.EMBER_QA_ROUTE) : [[-31, 32]];
  report.route = [];
  for (const [i, target] of route.entries()) report.route.push(await walkTo(peer, target, `route waypoint ${i + 1}`));
  const moved = report.phases.filter(p => ['walk', 'crouch walk', 'sprint'].includes(p.name));
  assert(moved.some(p => Math.max(Math.abs(p.first.x), Math.abs(p.first.z)) > 24.7 && Math.hypot(p.last.x - p.first.x, p.last.z - p.first.z) > 0.8),
    'No observed movement beyond the legacy24m half-boundary');
  report.beyondLegacyBoundary = true;
}
async function fullLobby(mode) {
  const name = `harbor-${mode}-${randomUUID().slice(0, 8)}`;
  const password = randomUUID();
  const members = [];
  const owner = new Peer(`harbor-${mode}-0`); members.push(owner);
  await until(() => owner.welcome, `${mode} welcome`);
  assert.equal(owner.welcome.proto, proto);
  owner.send({ t: 'create_lobby', name, password, map: 'harbor', mode });
  await until(() => owner.latest, `${mode} create`);
  assert.equal(owner.joined.map, 'harbor'); assert.equal(owner.joined.mode, mode);
  assert(owner.joined.arena_half > 24, 'Harbor must advertise larger per-level bounds');
  for (let i = 1; i < 8; i++) {
    const peer = new Peer(`harbor-${mode}-${i}`); members.push(peer);
    await until(() => peer.welcome, `${mode} player${i} welcome`);
    peer.send({ t: 'join_lobby', name, password });
    await until(() => peer.latest, `${mode} player${i} join`);
    assert.equal(peer.joined.map, 'harbor'); assert.equal(peer.joined.arena_half, owner.joined.arena_half);
  }
  await until(() => owner.state.players.length === 8, `${mode} complete roster`);
  const entry = { mode, welcome: owner.welcome, arenaHalf: owner.joined.arena_half, ...rosterCheck(owner.state, mode) };
  const ninth = new Peer(`harbor-${mode}-cap`); ninth.expectedError = true;
  await until(() => ninth.welcome, 'capacity probe welcome');
  ninth.send({ t: 'join_lobby', name, password });
  await until(() => ninth.error || ninth.joined, 'ninth peer rejection');
  assert(!ninth.joined && /full|eight|capacity|8/i.test(ninth.error), `Ninth peer was not refused: ${ninth.error}`);
  entry.ninthRejected = ninth.error; ninth.close();
  report.modes.push(entry);
  if (mode === 'tdm') {
    members[7].close();
    await until(() => owner.state.players.length === 7, 'TDM leave acknowledged');
    const replacement = new Peer('harbor-tdm-rejoin'); members.push(replacement);
    await until(() => replacement.welcome, 'TDM replacement welcome');
    replacement.send({ t: 'join_lobby', name, password });
    await until(() => replacement.latest && owner.state.players.length === 8, 'TDM replacement joined');
    entry.afterRejoin = rosterCheck(owner.state, mode);
  }
  if (mode === 'ffa') await handlingAndMovement(owner);
  for (const member of members) member.close();
  console.log(JSON.stringify({ mode, players: entry.players.length, minSpawnDistance: entry.minimumSeparation, teams: entry.teamCounts }));
}
async function main() {
  for (const mode of ['ffa', 'tdm', 'hill']) await fullLobby(mode);
  const old = new Peer('harbor-old-version', 19); old.expectedError = true;
  await until(() => old.welcome, 'old protocol welcome');
  old.send({ t: 'create_lobby', name: `old-harbor-${Date.now()}`, password: randomUUID(), map: 'harbor', mode: 'ffa' });
  await until(() => old.error || old.joined, 'old protocol rejection');
  assert(!old.joined && /protocol|version|reload/i.test(old.error), `Old protocol accepted: ${old.error}`);
  report.oldProtocolRejected = old.error; old.close();
  report.passed = true;
}
main().catch(error => { report.passed = false; report.errors.push(error.stack); process.exitCode = 1; }).finally(() => {
  for (const peer of peers) peer.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  report.stateCount = peers.reduce((sum, peer) => sum + peer.states.length, 0);
  fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'network-results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ passed: Boolean(report.passed), modes: report.modes.length, states: report.stateCount,
    shots: report.shots, elapsedSeconds: report.elapsedSeconds, errors: report.errors, output }));
});
