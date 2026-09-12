// Unit tests for deploy/check-hosts.mjs — `node --test deploy/check-hosts.test.mjs`.
//
// The gate's whole value is that it says FAILED on the day the site would have
// shipped a game nobody can enter, and says nothing on every other day. So the
// cases here are both: the ways that day looks, starting with the one that
// actually happened, and the ways a healthy site could be failed by a gate
// that trusted the book over the wire.
//
// The core is exercised against the REAL `web/hosts.js`, with only the network
// faked. Testing it against a stand-in ranking would prove the gate agrees
// with a copy of the rule rather than with the rule the players' pages run.
// The CLI is exercised as a process, because its exit code is the entire
// interface the release workflow has to it.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, rm, copyFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import * as hosts from '../web/hosts.js';
import { checkHosts, formatResult, plan, parseArgs, InputError } from './check-hosts.mjs';

// ---- the fakes -------------------------------------------------------------

/// A probe that answers from a table keyed by address. A missing address is an
/// unreachable host, which is the common case and should need no spelling.
/// `proto: null` is a server that reports no protocol at all.
const probeFrom = (table) => async (url) => {
  const r = table[url];
  if (!r) return { ok: false, reason: 'timeout' };
  const welcome = { host: r.host };
  if (r.proto !== null && r.proto !== undefined) welcome.proto = r.proto;
  return { ok: true, rttMs: r.rttMs ?? 7, welcome };
};

/// A probe that is silent for the first `failFor` attempts on an address and
/// answers after that, so a host can be made to flake rather than to fail.
/// `reasons` gives each silent attempt its own word where the difference is
/// the point; a missing one is a timeout. `count(url)` is how many sockets
/// that address was actually asked for.
const flakyProbe = (table) => {
  const asked = new Map();
  const probe = async (url) => {
    const n = (asked.get(url) ?? 0) + 1;
    asked.set(url, n);
    const r = table[url];
    if (!r) return { ok: false, reason: 'timeout' };
    if (n <= (r.failFor ?? 0)) return { ok: false, reason: (r.reasons ?? [])[n - 1] ?? 'timeout' };
    const welcome = { host: r.host };
    if (r.proto !== null && r.proto !== undefined) welcome.proto = r.proto;
    return { ok: true, rttMs: r.rttMs ?? 7, welcome };
  };
  probe.count = (url) => asked.get(url) ?? 0;
  return probe;
};

/// Mirrors, likewise: a URL the table does not carry is a mirror that did not
/// answer, which `hosts.js` treats as an absence rather than an error.
const mirrorsFrom = (table) => async (url) => table[url] ?? null;

/// The retry's pauses are spent instantly unless a test is about the schedule
/// itself. A suite that actually waited would pay three seconds for every
/// host that never answers, to prove nothing a recorded schedule does not.
const run = (opts) => checkHosts({ hosts, timeoutMs: 50, sleep: async () => {}, ...opts });

const catalog = (games) => ({ games });
const server = (proto, live = true) => ({ v: `v${proto}`, version: '1.0.0', live, proto });

// ---- the plan --------------------------------------------------------------

test('plan: a lab and a live page with no proto are both skips, not failures', () => {
  const rows = plan(catalog([
    { id: 'arena', versions: [{ v: 'v31', live: true, proto: 24 }, { v: 'v30', live: false, proto: 23 }] },
    { id: 'what-is-this', versions: [{ v: 'v1', live: true }] },
    { id: 'julibrot', kind: 'lab', versions: [{ v: 'v1', live: true }] },
  ]));
  assert.deepEqual(rows.map((r) => [r.game, r.v, r.proto, r.skip !== null]), [
    ['arena', 'v31', 24, false],
    ['what-is-this', 'v1', null, true],
    ['julibrot', 'v1', null, true],
  ]);
  assert.match(rows[2].skip, /lab/);
});

test('plan: a catalog with no games array is an input error, not an empty pass', () => {
  assert.throws(() => plan({}), InputError);
  assert.throws(() => plan(null), InputError);
});

// ---- the pass --------------------------------------------------------------

test('a live game whose host answers on its protocol passes', async () => {
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [{ name: 'lundi', ws: 'wss://arena', proto: 24, version: 'r1490' }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://arena': { proto: 24, host: 'lundi', rttMs: 31 } }),
  });
  assert.equal(out.ok, true);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 31 ms');
});

test('the skips are reported, and they do not make the run fail', async () => {
  const out = await run({
    games: catalog([
      { id: 'what-is-this', versions: [{ v: 'v1', version: '1.0.0', live: true }] },
      { id: 'julibrot', kind: 'lab', versions: [{ v: 'v1', version: '1.2.0', live: true }] },
    ]),
    book: {},
    mirrors: mirrorsFrom({}),
    probe: probeFrom({}),
  });
  assert.equal(out.ok, true);
  assert.deepEqual(out.results.map(formatResult), [
    'skip what-is-this v1: no catalog proto, no server needed',
    'skip julibrot v1: lab, no server needed',
  ]);
});

// ---- the Welcome outranks the book ----------------------------------------

test('a stale book protocol does not fail a host whose server welcomes the right one', async () => {
  // The served book is frozen between releases, so its `proto` is exactly the
  // value that goes stale. Filtering on it before probing would fail this gate
  // on the first protocol-bumping release while every player's page plays on
  // that host — the gate would be reporting on the book, not on the game.
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [{ name: 'lundi', ws: 'wss://arena', proto: 23, version: 'r1490' }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://arena': { proto: 24, host: 'lundi', rttMs: 11 } }),
  });
  assert.equal(out.ok, true);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 11 ms');
});

test('an entry with no protocol key at all is still probed, not discarded', async () => {
  // A host published by an older writer carries no protocol key. The page
  // probes it anyway (docs/hosts.md §5 step 4), so the gate must too.
  const out = await run({
    games: catalog([{ id: 'kings', versions: [server(1)] }]),
    book: { hosts: [{ name: 'lundi', kings_ws: 'wss://kings', version: 'r1490' }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://kings': { proto: 1, host: 'lundi', rttMs: 6 } }),
  });
  assert.equal(out.ok, true);
  assert.match(formatResult(out.results[0]), /^ok kings v1 proto 1 host lundi 6 ms$/);
});

test('a server that reports no protocol leaves the book’s claim standing', async () => {
  // An older server predates the field. Reading its silence as a mismatch
  // would fail a release over a host that is in fact serving the right game,
  // and `viewOf` makes exactly this fallback for the pages.
  const out = await run({
    games: catalog([{ id: 'kings', versions: [server(1)] }]),
    book: { hosts: [{ name: 'lundi', kings_ws: 'wss://kings', kings_proto: 1 }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://kings': { proto: null, host: 'lundi', rttMs: 5 } }),
  });
  assert.equal(out.ok, true);
  assert.match(formatResult(out.results[0]), /^ok kings v1 proto 1 host lundi 5 ms$/);
});

test('a server with neither a live nor a published protocol is not a pass', async () => {
  // Nothing anywhere says this host speaks the page's protocol, and the page
  // would drop it for the same reason. Silence is not agreement.
  const out = await run({
    games: catalog([{ id: 'kings', versions: [server(1)] }]),
    book: { hosts: [{ name: 'lundi', kings_ws: 'wss://kings' }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://kings': { proto: null, host: 'lundi' } }),
  });
  assert.equal(out.ok, false);
  assert.match(formatResult(out.results[0]), /lundi answered without a protocol, and its entry claims none/);
});

// ---- the failures ----------------------------------------------------------

test('no host at all for a live server game is a failure that says so', async () => {
  const out = await run({
    games: catalog([{ id: 'kings', versions: [server(1)] }]),
    book: { hosts: [{ name: 'lundi', ws: 'wss://arena', proto: 24 }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://arena': { proto: 24 } }),
  });
  assert.equal(out.ok, false);
  assert.equal(
    formatResult(out.results[0]),
    'FAILED kings v1 proto 1: no host in the book runs this game',
  );
});

test('2026-09-11: the hosts are up, and every one of them answers on the old protocol', async () => {
  // The site shipped arena 24 while every host still ran 23. Every machine was
  // healthy and the game was unenterable, which is the state nothing in the
  // publication path asked about. The verdict has to come from the sockets,
  // not from the book: these hosts say 23 on the wire.
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: {
      hosts: [
        { name: 'lundi', ws: 'wss://lundi', proto: 23, version: 'r1490' },
        { name: 'quiet-egret', ws: 'wss://egret', proto: 23, version: 'r1480' },
      ],
    },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://lundi': { proto: 23 }, 'wss://egret': { proto: 23 } }),
  });
  assert.equal(out.ok, false);
  const line = formatResult(out.results[0]);
  assert.match(line, /^FAILED arena v24 proto 24:/);
  // The diagnosis, not just the verdict: a reader has to be able to tell
  // "everything is down" from "everything is on the old build".
  assert.match(line, /lundi answered on proto 23/);
  assert.match(line, /quiet-egret answered on proto 23/);
});

test('a host that answers on another protocol than it published is not a pass', async () => {
  const out = await run({
    games: catalog([{ id: 'fire', versions: [server(2)] }]),
    book: { hosts: [{ name: 'lundi', fire_ws: 'wss://fire', fire_proto: 2 }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://fire': { proto: 1 } }),
  });
  assert.equal(out.ok, false);
  assert.match(formatResult(out.results[0]), /lundi answered on proto 1/);
});

test('every host running the game is probed, and each is named when none carries it', async () => {
  const seen = [];
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: {
      hosts: [
        { name: 'older', ws: 'wss://older', proto: 24, version: 'r100' },
        { name: 'newer', ws: 'wss://newer', proto: 24, version: 'r200' },
      ],
    },
    mirrors: mirrorsFrom({}),
    probe: async (url) => { seen.push(url); return { ok: false, reason: 'timeout' }; },
  });
  assert.equal(out.ok, false);
  // Newest build first (docs/hosts.md §5 step 3), and both were actually tried
  // — three times each, because neither ever answered.
  assert.deepEqual([...new Set(seen)].sort(), ['wss://newer', 'wss://older']);
  assert.equal(seen.length, 6);
  assert.match(formatResult(out.results[0]), /newer timeout, timeout, timeout; older timeout, timeout, timeout/);
});

// ---- the retry -------------------------------------------------------------
//
// One socket is a sample, and this gate blocks a release on it. At the
// Julibrot 1.3.0 release every Four Kings host timed out inside one window
// and answered on a rerun 1.3 s later, so the single probe reported "no host"
// about a site that had one. These cases are the blip and its neighbours: the
// host that comes back, the host that really is gone, and the host that
// answered the first time and must not be asked twice.

const oneHost = (probe, extra = {}) => run({
  games: catalog([{ id: 'arena', versions: [server(24)] }]),
  book: { hosts: [{ name: 'lundi', ws: 'wss://arena', proto: 24, version: 'r1490' }] },
  mirrors: mirrorsFrom({}),
  probe,
  ...extra,
});

test('a host that is silent once and answers on the second attempt carries the check', async () => {
  const probe = flakyProbe({ 'wss://arena': { failFor: 1, proto: 24, host: 'lundi', rttMs: 12 } });
  const out = await oneHost(probe);
  assert.equal(out.ok, true);
  assert.equal(probe.count('wss://arena'), 2);
  // The pass says which attempt paid for it. A deploy that proceeded on the
  // second socket is a deploy to look at the host after.
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 12 ms (attempt 2)');
});

test('a host that answers only on the third attempt still carries the check', async () => {
  const probe = flakyProbe({ 'wss://arena': { failFor: 2, proto: 24, host: 'lundi', rttMs: 30 } });
  const out = await oneHost(probe);
  assert.equal(out.ok, true);
  assert.equal(probe.count('wss://arena'), 3);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 30 ms (attempt 3)');
});

test('three silent attempts is a hostless game, and every attempt is named', async () => {
  const probe = flakyProbe({ 'wss://arena': { failFor: 3, reasons: ['timeout', 'error', 'closed'] } });
  const out = await oneHost(probe);
  assert.equal(out.ok, false);
  assert.equal(probe.count('wss://arena'), 3);
  // Three different words are three different machines to go and look at, so
  // the line carries all of them rather than only the last.
  assert.equal(formatResult(out.results[0]), 'FAILED arena v24 proto 24: lundi timeout, error, closed');
});

test('a host that answers the first time is asked once, and its line is unchanged', async () => {
  const probe = flakyProbe({ 'wss://arena': { proto: 24, host: 'lundi', rttMs: 8 } });
  const out = await oneHost(probe);
  assert.equal(out.ok, true);
  assert.equal(probe.count('wss://arena'), 1);
  // No attempt count on a clean pass: a healthy deploy log says what it said
  // before this retry existed.
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 8 ms');
});

test('a host that answers on another protocol is not asked again', async () => {
  // It gave its answer. Repeating the question buys the same answer for three
  // times the wall, and the 2026-09-11 state — every host up, every host on
  // the old build — is exactly the one a release must hear about quickly.
  const probe = flakyProbe({ 'wss://arena': { proto: 23, host: 'lundi' } });
  const out = await oneHost(probe);
  assert.equal(out.ok, false);
  assert.equal(probe.count('wss://arena'), 1);
  assert.match(formatResult(out.results[0]), /lundi answered on proto 23/);
});

test('a wrong-protocol answer reports the silent attempts before it', async () => {
  const probe = flakyProbe({
    'wss://arena': { failFor: 1, reasons: ['timeout'], proto: 23, host: 'lundi' },
  });
  const out = await oneHost(probe);
  assert.equal(out.ok, false);
  assert.equal(probe.count('wss://arena'), 2);
  assert.equal(formatResult(out.results[0]), 'FAILED arena v24 proto 24: lundi answered on proto 23 after timeout');
});

test('the pauses are 1 s before the second attempt and 2 s before the third', async () => {
  // Longer than the 1.3 s blip that started this, because a retry inside the
  // window that failed samples the same window twice. Nothing is waited after
  // the last attempt, which is why two pauses buy three sockets.
  const slept = [];
  const out = await oneHost(
    flakyProbe({ 'wss://arena': { failFor: 3 } }),
    { sleep: async (ms) => { slept.push(ms); } },
  );
  assert.equal(out.ok, false);
  assert.deepEqual(slept, [1000, 2000]);
});

test('the attempt count and the pauses are parameters, not a schedule in the loop', async () => {
  // One attempt is the single probe this gate used to be, and it fails on a
  // host the default would have recovered.
  const once = await oneHost(
    flakyProbe({ 'wss://arena': { failFor: 1, proto: 24, host: 'lundi' } }),
    { attempts: 1 },
  );
  assert.equal(once.ok, false);

  // A longer schedule needs no new code, and the last pause stands for every
  // attempt past the end of the list.
  const slept = [];
  const patient = flakyProbe({ 'wss://arena': { failFor: 3, proto: 24, host: 'lundi', rttMs: 4 } });
  const out = await oneHost(patient, {
    attempts: 5,
    pauseMs: [10, 20],
    sleep: async (ms) => { slept.push(ms); },
  });
  assert.equal(out.ok, true);
  assert.equal(patient.count('wss://arena'), 4);
  assert.deepEqual(slept, [10, 20, 20]);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 4 ms (attempt 4)');
});

test('each host keeps its own attempts, and one flake does not spend another host’s', async () => {
  const probe = flakyProbe({
    'wss://older': { failFor: 3 },
    'wss://newer': { failFor: 1, proto: 24, host: 'newer', rttMs: 3 },
  });
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: {
      hosts: [
        { name: 'older', ws: 'wss://older', proto: 24, version: 'r100' },
        { name: 'newer', ws: 'wss://newer', proto: 24, version: 'r200' },
      ],
    },
    mirrors: mirrorsFrom({}),
    probe,
  });
  assert.equal(out.ok, true);
  assert.equal(probe.count('wss://older'), 3);
  assert.equal(probe.count('wss://newer'), 2);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host newer 3 ms (attempt 2)');
});

// ---- the mirrors -----------------------------------------------------------

test('a bound mirror supplies the host that makes the check pass', async () => {
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [], mirrors: [{ url: 'https://m/lundi.json', name: 'lundi' }] },
    mirrors: mirrorsFrom({
      'https://m/lundi.json': { name: 'lundi', ws: 'wss://rotated', proto: 24, version: 'r1490' },
    }),
    probe: probeFrom({ 'wss://rotated': { proto: 24, rttMs: 12 } }),
  });
  assert.equal(out.ok, true);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host lundi 12 ms');
});

test('a dead mirror is an absence: the run finishes and reports the truth', async () => {
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [], mirrors: [{ url: 'https://m/dead.json', name: 'lundi' }] },
    mirrors: async () => { throw new Error('ENOTFOUND'); },
    probe: probeFrom({}),
  });
  assert.equal(out.ok, false);
  assert.equal(
    formatResult(out.results[0]),
    'FAILED arena v24 proto 24: no host in the book runs this game',
  );
});

test('a mirror serving a name it is not bound to cannot rescue the check', async () => {
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [], mirrors: [{ url: 'https://m/lundi.json', name: 'lundi' }] },
    mirrors: mirrorsFrom({
      'https://m/lundi.json': { name: 'somebody-else', ws: 'wss://evil', proto: 24 },
    }),
    probe: probeFrom({ 'wss://evil': { proto: 24 } }),
  });
  assert.equal(out.ok, false);
});

test('an unbound bare-string mirror is not fetched at all', async () => {
  // The rule comes from hosts.js itself now, so this pins that the gate reads
  // the same list the pages do rather than a copy of it.
  const asked = [];
  await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { hosts: [], mirrors: ['https://m/unbound.json', { url: 'https://m/lundi.json', name: 'lundi' }] },
    mirrors: async (url) => { asked.push(url); return null; },
    probe: probeFrom({}),
  });
  assert.deepEqual(asked, ['https://m/lundi.json']);
});

// ---- the book itself -------------------------------------------------------

test('the legacy top-level address is a host like any other', async () => {
  const out = await run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: { ws: 'wss://legacy', proto: 24 },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://legacy': { proto: 24, rttMs: 9 } }),
  });
  assert.equal(out.ok, true);
  assert.equal(formatResult(out.results[0]), 'ok arena v24 proto 24 host legacy-address 9 ms');
});

test('a book that is not an object is an input error, not a failed check', async () => {
  await assert.rejects(() => run({
    games: catalog([{ id: 'arena', versions: [server(24)] }]),
    book: [],
    mirrors: mirrorsFrom({}),
    probe: probeFrom({}),
  }), InputError);
});

test('several games are resolved together and each reports its own verdict', async () => {
  const out = await run({
    games: catalog([
      { id: 'arena', versions: [server(24)] },
      { id: 'fire', versions: [server(2)] },
      { id: 'julibrot', kind: 'lab', versions: [{ v: 'v1', version: '1.2.0', live: true }] },
    ]),
    book: { hosts: [{ name: 'lundi', ws: 'wss://arena', proto: 24, fire_ws: 'wss://fire', fire_proto: 2 }] },
    mirrors: mirrorsFrom({}),
    probe: probeFrom({ 'wss://arena': { proto: 24, rttMs: 20 } }),
  });
  assert.equal(out.ok, false);
  assert.deepEqual(out.results.map(formatResult), [
    'ok arena v24 proto 24 host lundi 20 ms',
    'FAILED fire v2 proto 2: lundi timeout, timeout, timeout',
    'skip julibrot v1: lab, no server needed',
  ]);
});

// ---- the command line ------------------------------------------------------

test('parseArgs: --tree is the three files at a site root', () => {
  const a = parseArgs(['--tree', '/site']);
  assert.equal(a.games, path.join('/site', 'games.json'));
  assert.equal(a.book, path.join('/site', 'server.json'));
  assert.equal(a.hostsJs, path.join('/site', 'hosts.js'));
});

test('parseArgs: an explicit flag still wins over --tree', () => {
  const a = parseArgs(['--tree', '/site', '--book', '/elsewhere/server.json']);
  assert.equal(a.games, path.join('/site', 'games.json'));
  assert.equal(a.book, '/elsewhere/server.json');
  // The hosts.js beside the BOOK, because that is the copy its pages import.
  assert.equal(a.hostsJs, path.join('/site', 'hosts.js'));
});

test('parseArgs: a remote book falls back to this checkout’s hosts.js', () => {
  const a = parseArgs(['--games', 'web/games.json', '--book', 'https://example.invalid/server.json']);
  assert.equal(a.hostsJs, fileURLToPath(new URL('../web/hosts.js', import.meta.url)));
});

test('parseArgs: a path book takes the hosts.js beside it', () => {
  const a = parseArgs(['--games', '/site/games.json', '--book', '/site/server.json']);
  assert.equal(a.hostsJs, path.join('/site', 'hosts.js'));
});

test('parseArgs: the refusals', () => {
  assert.throws(() => parseArgs(['--bogus']), InputError);
  assert.throws(() => parseArgs(['--book']), InputError);
  assert.throws(() => parseArgs(['--book', '/site/server.json']), InputError);
  assert.throws(() => parseArgs(['--games', '/site/games.json']), InputError);
  assert.equal(parseArgs(['--help']).help, true);
});

// The CLI as a process. The exit code is the entire interface the release
// workflow has to this script, so it is proved by running it, not by reading
// it. Mirrors are bound to local files, so none of this touches a network.
const SCRIPT = fileURLToPath(new URL('./check-hosts.mjs', import.meta.url));
const HOSTS_JS = fileURLToPath(new URL('../web/hosts.js', import.meta.url));

function cli(args) {
  return new Promise((resolve) => {
    execFile(process.execPath, [SCRIPT, ...args], (err, stdout, stderr) => {
      resolve({ code: err ? err.code ?? 1 : 0, stdout, stderr });
    });
  });
}

async function siteTree(book, games) {
  const dir = await mkdtemp(path.join(tmpdir(), 'check-hosts-'));
  await mkdir(dir, { recursive: true });
  await copyFile(HOSTS_JS, path.join(dir, 'hosts.js'));
  await writeFile(path.join(dir, 'server.json'), JSON.stringify(book));
  await writeFile(path.join(dir, 'games.json'), JSON.stringify(games));
  return dir;
}

test('CLI: a tree with no host for a live game exits 1 and names the game', async () => {
  const dir = await siteTree(
    { hosts: [] },
    catalog([{ id: 'arena', versions: [server(24)] }]),
  );
  try {
    const r = await cli(['--tree', dir]);
    assert.equal(r.code, 1);
    assert.match(r.stdout, /FAILED arena v24 proto 24: no host in the book runs this game/);
    assert.match(r.stderr, /1 live game\(s\) have no host to join/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('CLI: a tree whose only live entries need no server exits 0', async () => {
  const dir = await siteTree(
    { hosts: [] },
    catalog([
      { id: 'what-is-this', versions: [{ v: 'v1', version: '1.0.0', live: true }] },
      { id: 'julibrot', kind: 'lab', versions: [{ v: 'v1', version: '1.2.0', live: true }] },
    ]),
  );
  try {
    const r = await cli(['--tree', dir]);
    assert.equal(r.code, 0);
    assert.match(r.stdout, /every live server game has a host that answered/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('CLI: a file-bound mirror is read, so the whole path runs with no network', async () => {
  const dir = await siteTree({ hosts: [] }, catalog([{ id: 'arena', versions: [server(24)] }]));
  try {
    const mirror = path.join(dir, 'lundi.json');
    await writeFile(mirror, JSON.stringify({ name: 'lundi', ws: 'wss://127.0.0.1:9/arena', proto: 24 }));
    await writeFile(
      path.join(dir, 'server.json'),
      JSON.stringify({ hosts: [], mirrors: [{ url: mirror, name: 'lundi' }] }),
    );
    const r = await cli(['--tree', dir]);
    // The mirror's host is real enough to be ranked and probed; nothing is
    // listening on port 9, so the verdict is a named unreachable host rather
    // than "no host runs this game", which is what proves the merge happened.
    // This is the one case that spends the retry's real 3 s of pauses: the
    // CLI is exercised as a process, so there is nothing to inject a sleep
    // into, and the schedule a deploy actually waits is worth proving once.
    assert.equal(r.code, 1);
    assert.match(r.stdout, /FAILED arena v24 proto 24: lundi /);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('CLI: an unreadable input is 2, which is not the same answer as a failed check', async () => {
  const r = await cli(['--tree', path.join(tmpdir(), 'check-hosts-no-such-dir')]);
  assert.equal(r.code, 2);
  assert.match(r.stderr, /check-hosts:/);
});

test('CLI: a book that is not JSON is 2 rather than an empty pass', async () => {
  const dir = await siteTree({ hosts: [] }, catalog([{ id: 'arena', versions: [server(24)] }]));
  try {
    await writeFile(path.join(dir, 'server.json'), '{ not json');
    const r = await cli(['--tree', dir]);
    assert.equal(r.code, 2);
    assert.match(r.stderr, /cannot read/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('CLI: a hosts.js without the exports it needs is 2, and says which', async () => {
  const dir = await siteTree({ hosts: [] }, catalog([{ id: 'arena', versions: [server(24)] }]));
  try {
    await writeFile(path.join(dir, 'hosts.js'), 'export const nothing = 1;\n');
    const r = await cli(['--tree', dir]);
    assert.equal(r.code, 2);
    assert.match(r.stderr, /exports no mirrorRefs/);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('CLI: an unknown argument is 2 and prints the usage', async () => {
  const r = await cli(['--bogus']);
  assert.equal(r.code, 2);
  assert.match(r.stderr, /unknown argument --bogus/);
  assert.match(r.stderr, /usage: node deploy\/check-hosts\.mjs --tree/);
});

test('CLI: --help is 0 and prints the usage on stdout', async () => {
  const r = await cli(['--help']);
  assert.equal(r.code, 0);
  assert.match(r.stdout, /usage: node deploy\/check-hosts\.mjs --tree/);
});
