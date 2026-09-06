// Real-server harbor checks. Only private disposable lobbies and their peers
// are touched; no geometry imitation, browser, OS input, or server control.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { randomUUID } = require('node:crypto');
const { shotgunEndpoints } = require('./shotgun-events.cjs');
const url = process.argv[2];
if (url !== 'ws://127.0.0.1:7788') throw new Error('Only the owned private QA endpoint ws://127.0.0.1:7788 is allowed; never use live port7780');
const proto = 24;
const started = Date.now();
const output = path.resolve(process.env.EMBER_QA_OUTPUT || 'target/killshot-v31-network');
const report = { purpose: 'Real server, private eight-player lobbies; not a load/latency benchmark or complete route proof',
  url, proto, map: 'harbor', modes: [], phases: [], errors: [], shots: 0 };
const peers = [];
require('node:os').setPriority(0, require('node:os').constants.priority.PRIORITY_LOW);
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
    this.seq = 0; this.states = []; this.observed = []; this.shotEvents = []; this.phase = null; this.closed = false;
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
      if (m.t === 'shot') this.shotEvents.push(m);
      if (m.t === 'state') {
        this.state = m;
        const actor = m.players.find(player => player.id === this.watchId);
        if (actor) this.observed.push({ tick: m.tick, ...actor });
        const mine = m.players.find(p => p.id === this.id);
        if (mine) {
          this.latest = { tick: m.tick, ...mine };
          this.states.push(this.latest);
          if (this.captureStates) this.captureStates.push(this.latest);
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
      shield: false, melee: false, ads: false, select_slot: 0, ...overrides });
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
    assert(p.parkour && p.parkour.velocity.length === 2 && p.parkour.velocity.every(Number.isFinite)
      && ['slide_remaining', 'slide_cooldown', 'wall_cooldown'].every(key => Number.isFinite(p.parkour[key])), 'Missing/nonfinite parkour wire state');
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
async function phase(peer, name, input, milliseconds, afterInput = {}) {
  const records = [];
  peer.phase = { firstSeq: peer.seq + 1, records };
  peer.input(input);
  peer.timer = setInterval(() => peer.input({ ...input, jump: false }), 50);
  try {
    await wait(milliseconds);
    await until(() => records.length >= 6, `${name}: six observed states`);
  } finally {
    clearInterval(peer.timer); peer.timer = null; peer.phase = null;
    peer.input(afterInput);
  }
  const first = records[0], last = records.at(-1);
  const elapsedTicks = last.tick - first.tick;
  assert(elapsedTicks > 0, 'Measured phase needs distinct server ticks');
  const measured = { name, stateCount: records.length, first, last, elapsedTicks,
    speed: Math.hypot(last.x - first.x, last.z - first.z) / (elapsedTicks / 60),
    partialAds: records.some(p => p.ads_fraction > 0 && p.ads_fraction < 1),
    peakSpread: Math.max(...records.map(p => p.spread)), maxHeight: Math.max(...records.map(p => p.y)) };
  if (records.some(state => state.parkour?.slide_remaining > 0 || state.parkour?.momentum)) measured.records = records;
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

const normLength = normal => Math.hypot(...normal);
const dot = (a, b) => a[0] * b[0] + a[1] * b[1];
async function settle(peer, label) {
  peer.input();
  await until(() => peer.latest.vy === 0 && peer.latest.y < 0.05, label, 5000);
  await phase(peer, label + ' neutral', {}, 150);
}
async function parkourRoute(peer, observers) {
  // These are authored waypoints, not a second collision implementation.
  // The north-west service-house/warehouse alley has faces x=-28/-24.
  const approach = process.env.EMBER_QA_PARKOUR_APPROACH ? JSON.parse(process.env.EMBER_QA_PARKOUR_APPROACH)
    : [[-26, 32], [-26, 26.2], [-26, 21]];
  report.parkourApproach = [];
  for (const [i, target] of approach.entries()) report.parkourApproach.push(await walkTo(peer, target, `parkour approach ${i + 1}`));
  const parkourStart = peer.states.length;
  await phase(peer, 'slide run-up', { my: 1, sprint: true }, 250);
  const mark = peer.states.length;
  await phase(peer, 'ground slide', { my: 1, sprint: true, crouch: true }, 900, { crouch: true });
  const slide = peer.states.slice(mark).filter(state => state.parkour.slide_remaining > 0);
  assert(slide.length >= 5, 'No authoritative sustained ground slide observed');
  assert(slide.every(state => state.crouch && state.y < 0.05), 'Ground slide did not retain grounded crouch');
  assert(Math.max(...slide.map(state => normLength(state.parkour.velocity))) > 8.5, 'Slide never exceeded ordinary sprint speed');
  assert(slide.at(-1).parkour.slide_remaining < slide[0].parkour.slide_remaining, 'Slide timer did not advance');
  const heldMark = peer.states.length;
  await phase(peer, 'held crouch cannot retrigger slide', { crouch: true }, 1500);
  assert(peer.states.slice(heldMark).every(state => state.parkour.slide_remaining === 0), 'Held crouch restarted an exhausted slide');
  // Move to the same lane again only after the initial slide and cooldown.
  await walkTo(peer, [-26, 21], 'return to paired-wall lane');
  await walkTo(peer, [-27.3, 21], 'approach west physical wall');
  await settle(peer, 'before wall slide');
  const wallSlideMark = peer.states.length;
  await phase(peer, 'jump then hold crouch into wall', { mx: -1, crouch: true, jump: true }, 1100);
  const wallSlide = peer.states.slice(wallSlideMark).filter(state =>
    state.y > 0.05 && state.vy < -0.1 && normLength(state.parkour.wall_normal) > 0.9);
  assert(wallSlide.length >= 3, 'No descending physical-wall contact captured');
  assert(wallSlide.every(state => state.vy >= -2.501), 'Wall slide exceeded2.5m/s fall cap');
  await settle(peer, 'wall-slide landing');
  await walkTo(peer, [-27.3, 21], 'paired-wall kick start');

  const kicks = [], kickFrames = [], kickStart = Date.now();
  const decisions = [];
  // Keep diagnostics reachable by the final report before any timeout/assertion.
  // Record every authoritative snapshot, not just frames sampled by the input timer.
  report.parkour = { slideFrames: slide, wallSlideFrames: wallSlide, kicksRequested: kicks,
    kickFrames, decisions, initial: peer.latest };
  peer.captureStates = kickFrames;
  let first = true, lastRequestedTick = -100;
  const steer = () => {
    const state = peer.latest, motion = state.parkour;
    const normal = motion.wall_normal, prior = motion.last_wall_normal;
    const onNewWall = state.y > 0.05 && normLength(normal) > 0.9
      && dot(normal, prior) < 0.5 && motion.wall_cooldown === 0;
    const jump = first || (onNewWall && state.tick - lastRequestedTick >= 3);
    if (jump && !first) {
      lastRequestedTick = state.tick;
      kicks.push({ requestedAtTick: state.tick, normal, y: state.y });
    }
    // Follow only a CONFIRMED impulse toward the opposing wall. Flipping on
    // request anticipates acceptance and can leave the face before a delayed
    // server processes the pulse. This is the same input policy used by the
    // actual Level::harbor native route fixture, not a movement simulation.
    const mx = normLength(prior) > 0.9 ? prior[0] : -1;
    decisions.push({ atTick: state.tick, seq: peer.seq + 1, mx, jump,
      normal, lastWallNormal: prior, y: state.y, ack: state.ack });
    peer.input({ mx, crouch: true, jump });
    first = false;
  };
  const airborneChains = () => {
    const chains = []; let chain = [], previous = [0, 0];
    for (const state of kickFrames) {
      if (state.vy === 0 && !state.parkour.momentum) {
        if (chain.length) chains.push(chain);
        chain = []; previous = [0, 0];
      }
      const motion = state.parkour;
      if (motion.momentum && motion.wall_cooldown > 0
        && normLength(motion.last_wall_normal) > 0.9 && dot(motion.last_wall_normal, previous) < 0.5) {
        chain.push(state); previous = motion.last_wall_normal;
      }
    }
    if (chain.length) chains.push(chain);
    return chains;
  };
  const opposingChain = () => airborneChains().find(chain => {
    const landed = kickFrames.find(state => state.tick > chain[0].tick && state.vy === 0 && !state.parkour.momentum);
    return chain.some((state, index) => index > 0
      && dot(state.parkour.last_wall_normal, chain[index - 1].parkour.last_wall_normal) < -0.9)
      && kickFrames.some(state => state.tick >= chain[0].tick && (!landed || state.tick < landed.tick)
        && state.parkour.momentum && state.parkour.wall_cooldown > 0 && state.y > 2.0);
  });
  steer(); peer.timer = setInterval(steer, 30);
  try {
    await until(() => opposingChain(), 'authoritative alternating opposing-wall kicks', 10000);
  } finally {
    clearInterval(peer.timer); peer.timer = null; peer.input();
    peer.captureStates = null;
    report.parkour.elapsedKickSeconds = (Date.now() - kickStart) / 1000;
    report.parkour.final = peer.latest;
    report.parkour.maxHeight = Math.max(0, ...kickFrames.map(state => state.y));
    report.parkour.acceptedChains = airborneChains();
  }
  const actual = kickFrames.filter(state => state.parkour.momentum && state.parkour.wall_cooldown > 0);
  assert(actual.some(state => Math.abs(state.parkour.velocity[0]) > 5.5 && state.vy > 5), 'No observed outward and upward wall-kick impulse');
  const chain = opposingChain();
  assert(chain, 'Opposing kicks did not occur within one observed airborne chain');
  const between = kickFrames.filter(state => state.tick >= chain[0].tick && state.tick <= chain[1].tick);
  assert(between.every(state => state.parkour.momentum), 'Momentum was lost between opposing wall kicks');
  const momentumSamples = between.filter(state => state.y > 0.1);
  assert(momentumSamples.length >= 4, 'Momentum did not survive between wall kicks');
  // All seven other connected players must receive the actor's authoritative
  // slide AND momentum, not just a locally manufactured input flag.
  await wait(150);
  report.parkour.observers = observers.map(observer => {
    const seen = observer.observed.filter(state => state.tick >= peer.states[parkourStart].tick);
    assert(seen.some(state => state.parkour.slide_remaining > 0), 'Remote player never received the slide state');
    assert(seen.some(state => state.parkour.momentum && state.parkour.wall_cooldown > 0), 'Remote player never received a wall-kick state');
    return { id: observer.id, snapshots: seen.length, sawSlide: true, sawWallKick: true };
  });
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
  if (mode === 'ffa') {
    for (const member of members.slice(1)) member.watchId = owner.id;
    await handlingAndMovement(owner);
    await parkourRoute(owner, members.slice(1));
  }
  for (const member of members) member.close();
  console.log(JSON.stringify({ mode, players: entry.players.length, minSpawnDistance: entry.minimumSeparation, teams: entry.teamCounts }));
}
async function shotgunLobby() {
  const name = `breach12-${randomUUID().slice(0, 8)}`, password = randomUUID(), members = [];
  const owner = new Peer('breach12-owner'); members.push(owner);
  await until(() => owner.welcome, 'Breach-12 owner welcome');
  assert.equal(owner.welcome.proto, proto);
  owner.send({ t: 'create_lobby', name, password, map: 'harbor', mode: 'ffa', loadout: 'custom', starting_weapon: 8 });
  await until(() => owner.latest, 'Breach-12 room creation');
  for (let index = 1; index < 8; index++) {
    const peer = new Peer(`breach12-peer-${index}`); members.push(peer);
    await until(() => peer.welcome, `Breach-12 peer${index} welcome`);
    peer.send({ t: 'join_lobby', name, password });
    await until(() => peer.latest, `Breach-12 peer${index} joined`);
  }
  await until(() => members.every(peer => peer.state.players.length === 8), 'all eight shotgun rosters');
  for (const peer of members) {
    assert.equal(peer.joined.loadout, 'custom'); assert.equal(peer.joined.starting_weapon, 8); assert.equal(peer.joined.mode, 'ffa');
    assert.equal(peer.latest.weapon, 8); assert.equal(peer.latest.ammo, 6); assert.equal(peer.latest.reserve, 24);
    assert.equal(peer.latest.inventory.length, 9);
    assert.deepEqual(peer.latest.inventory.map(slot => slot.weapon), [1, 0, 0, 0, 0, 0, 0, 8, 0]);
    peer.watchId = owner.id;
  }
  const beforeTick = owner.latest.tick;
  // Actual gameplay input only: aim above the other players into clear sky.
  // Releasing after the first authoritative ammo decrement keeps the trigger
  // below its 0.85-second second-shot boundary on the private loopback fixture.
  owner.input({ fire: true, pitch: 1.2 });
  await until(() => owner.latest.tick > beforeTick && owner.latest.ammo === 5, 'one shell spent');
  owner.input();
  await until(() => members.every(peer => peer.shotEvents.filter(event => event.weapon === 8 && event.owner === owner.id).length >= 8),
    'eight pellet endpoints received by every player');
  const observations = members.map(peer => {
    const endpoints = peer.shotEvents.filter(event => event.weapon === 8 && event.owner === owner.id);
    return { peer: peer.id, ...shotgunEndpoints(endpoints, owner.id) };
  });
  for (const observed of observations) assert.deepEqual(observed.endpoints, observations[0].endpoints, 'Peers disagree on actual pellet endpoints');
  assert.equal(owner.latest.ammo, 5, 'Eight pellets must spend exactly one shell');
  assert.equal(owner.latest.reserve, 24, 'Firing must not directly spend reserve');
  const shellKey = observations[0].shellKey;
  assert.equal(observations[0].projectileIds.length, 8);
  assert(observations[0].tickModulo40 >= beforeTick % (2 ** 40), 'Projectile tick predates the actual input');

  owner.input({ select_slot: 1 });
  await until(() => owner.latest.weapon === 1, 'switch from shotgun to sidearm'); owner.input();
  assert.equal(owner.latest.inventory[7].ammo, 5, 'Switching away must retain the partially loaded shotgun');
  assert.equal(owner.latest.inventory[7].reserve, 24);
  owner.input({ select_slot: 8 });
  await until(() => owner.latest.weapon === 8, 'switch back to shotgun'); owner.input();
  assert.equal(owner.latest.ammo, 5); assert.equal(owner.latest.reserve, 24);
  owner.input({ select_slot: 9 });
  const unownedSequence = owner.seq;
  await until(() => owner.latest.ack >= unownedSequence, 'unowned slot9 acknowledged'); owner.input();
  assert.equal(owner.latest.weapon, 8); assert.equal(owner.latest.ammo, 5);

  owner.input({ reload: true });
  await until(() => owner.latest.reloading && owner.latest.reload_remaining > 0, 'shotgun reload begins'); owner.input();
  const reloadStart = owner.latest;
  assert(reloadStart.reload_remaining <= 2.8001, 'Reload timer exceeds the 2.8-second weapon duration');
  await until(() => owner.latest.tick > reloadStart.tick && !owner.latest.reloading && owner.latest.ammo === 6, 'shotgun reload completes', 6000);
  assert.equal(owner.latest.reserve, 23, 'One reloaded shell must spend one reserve round, not eight');
  const reloadEnd = owner.latest;
  await until(() => members.every(peer => {
    const state = peer.state.players.find(player => player.id === owner.id);
    return state?.weapon === 8 && state.ammo === 6 && state.reserve === 23 && !state.reloading;
  }), 'all eight peers observe completed shell reload');
  assert(members.every(peer => peer.observed.some(state => state.reload_remaining > 0)), 'Every observer must receive the active reload timer');
  for (const peer of members) {
    const ownShotgun = peer.state.players.find(player => player.id === owner.id);
    assert.equal(ownShotgun.inventory[7].ammo, 6); assert.equal(ownShotgun.inventory[7].reserve, 23);
  }
  report.shotgun = { passed: true, players: members.length, startingWeapon: 8, shellKey,
    ammoBefore: 6, ammoAfterShot: 5, ammoAfterReload: 6, reserveBefore: 24, reserveAfterReload: 23,
    reloadStart, reloadEnd, observations,
    limits: 'Real gameplay inputs and server events only. This sky-shot fixture proves pellet identity/broadcast and shell accounting, not pellet damage, hit balance or graphics.' };
  for (const peer of members) peer.close();
  console.log(JSON.stringify({ shotgun: true, players: 8, pelletEndpointsPerPeer: 8, shellKey, magazine: 6, reserve: 23 }));
}

async function main() {
  for (const mode of ['ffa', 'tdm', 'hill']) await fullLobby(mode);
  await shotgunLobby();
  const old = new Peer('harbor-old-version', 23); old.expectedError = true;
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
