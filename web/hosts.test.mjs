// Unit tests for web/hosts.js — run with `node --test web/hosts.test.mjs`.
//
// No browser, no server, no dependencies: the picking rule is the part of the
// multi-host feature that is easiest to get subtly wrong and hardest to see
// wrong in a browser, because a wrong order still shows a working game. So it
// is tested here, on the book data alone.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  PIN_KEY,
  chooseHost,
  keysFor,
  listLobbies,
  mergeBook,
  parseVersion,
  probeHost,
  rankHosts,
  renderChip,
} from './hosts.js';

// ---- key derivation ------------------------------------------------------

test('keysFor: the arena owns the bare names, everyone else is prefixed', () => {
  assert.deepEqual(keysFor('arena'), { ws: 'ws', proto: 'proto' });
  assert.deepEqual(keysFor('fire'), { ws: 'fire_ws', proto: 'fire_proto' });
  assert.deepEqual(keysFor('kings'), { ws: 'kings_ws', proto: 'kings_proto' });
});

test('PIN_KEY is the documented localStorage key', () => {
  assert.equal(PIN_KEY, 'ember-host');
});

// ---- versions ------------------------------------------------------------

test('parseVersion: r<N> is an integer, anything else is zero', () => {
  assert.equal(parseVersion('r211'), 211);
  assert.equal(parseVersion(' r7 '), 7);
  assert.equal(parseVersion('211'), 211);
  assert.equal(parseVersion(''), 0);
  assert.equal(parseVersion(undefined), 0);
  assert.equal(parseVersion('dev'), 0);
  // Sorts numerically, not as text: r9 must not beat r211.
  assert.ok(parseVersion('r211') > parseVersion('r9'));
});

// ---- merging -------------------------------------------------------------

test('mergeBook: entries merge by name and the later one wins', () => {
  const book = {
    hosts: [
      { name: 'amber-otter', ws: 'wss://a', version: 'r200' },
      { name: 'flint-heron', ws: 'wss://b', version: 'r100' },
    ],
  };
  const merged = mergeBook(book, [{ name: 'amber-otter', ws: 'wss://a2', version: 'r211' }]);
  assert.equal(merged.length, 2);
  assert.equal(merged[0].name, 'amber-otter');
  assert.equal(merged[0].ws, 'wss://a2');
  assert.equal(merged[0].version, 'r211');
  assert.equal(merged[1].name, 'flint-heron');
});

test('mergeBook: a legacy book with no hosts list still yields one entry', () => {
  const merged = mergeBook({
    v: '1788261977',
    proto: 12,
    fire_proto: 1,
    ws: 'wss://old.example',
    fire_ws: 'wss://old-fire.example',
  });
  assert.equal(merged.length, 1);
  assert.equal(merged[0].name, '');
  assert.equal(merged[0].ws, 'wss://old.example');
  assert.equal(merged[0].proto, 12);
  assert.equal(merged[0].fire_ws, 'wss://old-fire.example');
  assert.equal(merged[0].fire_proto, 1);
  assert.equal(merged[0].legacy, true);
});

test('mergeBook: an empty hosts list falls back to the legacy keys too', () => {
  const merged = mergeBook({ hosts: [], ws: 'wss://old.example', proto: 12 });
  assert.equal(merged.length, 1);
  assert.equal(merged[0].ws, 'wss://old.example');
});

test('mergeBook: a book with neither hosts nor addresses yields nothing', () => {
  assert.deepEqual(mergeBook({ v: '1' }), []);
  assert.deepEqual(mergeBook(null), []);
});

// ---- ranking -------------------------------------------------------------

const ok = (extra = {}) => ({ ok: true, rttMs: 50, welcome: { t: 'welcome', ...extra } });
const dead = { ok: false, reason: 'timeout' };

const entry = (name, over = {}) => ({
  name,
  ws: `wss://${name}`,
  proto: 12,
  version: 'r200',
  commit: 'abc1234',
  updated: '2026-09-01T12:00:00Z',
  ...over,
});

test('rankHosts: the newest version wins, then updated, then name', () => {
  const hosts = [
    entry('old', { version: 'r100' }),
    entry('newer', { version: 'r211' }),
    entry('newest-b', { version: 'r300', updated: '2026-09-01T09:00:00Z' }),
    entry('newest-a', { version: 'r300', updated: '2026-09-01T11:00:00Z' }),
  ];
  const { chosen, candidates } = rankHosts(hosts, { game: 'arena' });
  // No probes at all: the load/rtt tie-break has nothing to work with, so
  // `updated` decides inside the newest build.
  assert.equal(chosen.name, 'newest-a');
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['newest-a', 'newest-b', 'newer', 'old'],
  );
});

test('rankHosts: unparseable versions sort last', () => {
  const hosts = [entry('unstamped', { version: '' }), entry('stamped', { version: 'r1' })];
  const { candidates } = rankHosts(hosts, { game: 'arena' });
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['stamped', 'unstamped'],
  );
});

test('rankHosts: the protocol filter drops hosts on another build', () => {
  const hosts = [entry('twelve', { proto: 12 }), entry('eleven', { proto: 11 })];
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena', proto: 12 }).candidates.map((c) => c.name),
    ['twelve'],
  );
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena', proto: 11 }).candidates.map((c) => c.name),
    ['eleven'],
  );
});

test('rankHosts: the hub mode (no proto) keeps every protocol', () => {
  const hosts = [entry('twelve', { proto: 12 }), entry('eleven', { proto: 11 })];
  assert.equal(rankHosts(hosts, { game: 'arena' }).candidates.length, 2);
});

test('rankHosts: the live Welcome proto beats a stale book key', () => {
  const hosts = [entry('stale', { proto: 11 })];
  const probes = new Map([['stale', ok({ proto: 12, version: 'r211' })]]);
  assert.equal(rankHosts(hosts, { game: 'arena', proto: 12, probes }).candidates.length, 1);
  assert.equal(rankHosts(hosts, { game: 'arena', proto: 11, probes }).candidates.length, 0);
});

test('rankHosts: within the newest build, fewest players then lowest rtt then name', () => {
  const hosts = [entry('busy'), entry('idle-far'), entry('idle-near'), entry('idle-tie')];
  const probes = new Map([
    ['busy', { ok: true, rttMs: 5, welcome: { players: 4 } }],
    ['idle-far', { ok: true, rttMs: 200, welcome: { players: 0 } }],
    ['idle-near', { ok: true, rttMs: 20, welcome: { players: 0 } }],
    ['idle-tie', { ok: true, rttMs: 20, welcome: { players: 0 } }],
  ]);
  const { chosen, candidates } = rankHosts(hosts, { game: 'arena', probes });
  assert.equal(chosen.name, 'idle-near');
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['idle-near', 'idle-tie', 'idle-far', 'busy'],
  );
});

test('rankHosts: load and latency never lift a host over a newer build', () => {
  const hosts = [
    entry('newest', { version: 'r300' }),
    entry('emptier', { version: 'r100' }),
  ];
  const probes = new Map([
    ['newest', { ok: true, rttMs: 300, welcome: { players: 7 } }],
    ['emptier', { ok: true, rttMs: 1, welcome: { players: 0 } }],
  ]);
  assert.equal(rankHosts(hosts, { game: 'arena', probes }).chosen.name, 'newest');
});

test('rankHosts: hosts that did not answer are excluded once probes exist', () => {
  const hosts = [entry('live'), entry('down')];
  const probes = new Map([
    ['live', ok()],
    ['down', dead],
  ]);
  const { chosen, candidates } = rankHosts(hosts, { game: 'arena', probes });
  assert.equal(chosen.name, 'live');
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['live'],
  );
});

test('rankHosts: a host with no probe entry at all is excluded too', () => {
  const hosts = [entry('live'), entry('unprobed')];
  const probes = new Map([['live', ok()]]);
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena', probes }).candidates.map((c) => c.name),
    ['live'],
  );
});

test('rankHosts: a pinned host that answered goes first, and stays in the list once', () => {
  const hosts = [entry('best', { version: 'r300' }), entry('pinned', { version: 'r100' })];
  const probes = new Map([
    ['best', ok()],
    ['pinned', ok()],
  ]);
  const { chosen, candidates } = rankHosts(hosts, { game: 'arena', pinned: 'pinned', probes });
  assert.equal(chosen.name, 'pinned');
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['pinned', 'best'],
  );
});

test('rankHosts: a pin that did not answer is ignored, not fatal', () => {
  const hosts = [entry('best'), entry('pinned')];
  const probes = new Map([
    ['best', ok()],
    ['pinned', dead],
  ]);
  const { chosen } = rankHosts(hosts, { game: 'arena', pinned: 'pinned', probes });
  assert.equal(chosen.name, 'best');
});

test('rankHosts: an entry carrying only one game is invisible to the other', () => {
  const hosts = [
    { name: 'arena-only', ws: 'wss://a', proto: 12, version: 'r200' },
    { name: 'fire-only', fire_ws: 'wss://f', fire_proto: 1, version: 'r200' },
    { name: 'both', ws: 'wss://b', proto: 12, fire_ws: 'wss://bf', fire_proto: 1, version: 'r200' },
  ];
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena', proto: 12 }).candidates.map((c) => c.name),
    ['arena-only', 'both'],
  );
  assert.deepEqual(
    rankHosts(hosts, { game: 'fire', proto: 1 }).candidates.map((c) => c.name),
    ['both', 'fire-only'],
  );
});

test('rankHosts: the live Welcome overrides the book, an old server does not', () => {
  const hosts = [entry('booked', { version: 'r100', commit: 'aaa1111' })];
  const live = new Map([
    ['booked', { ok: true, rttMs: 12, welcome: { host: 'renamed', version: 'r211', commit: 'bbb2222', players: 3 } }],
  ]);
  const fresh = rankHosts(hosts, { game: 'arena', probes: live }).chosen;
  assert.equal(fresh.name, 'renamed');
  assert.equal(fresh.version, 'r211');
  assert.equal(fresh.versionNum, 211);
  assert.equal(fresh.commit, 'bbb2222');
  assert.equal(fresh.players, 3);
  assert.equal(fresh.rttMs, 12);

  // A server predating the fields answers with none of them: the book stands.
  const old = new Map([['booked', { ok: true, rttMs: 12, welcome: { proto: 12, motd: 'hi' } }]]);
  const stale = rankHosts(hosts, { game: 'arena', probes: old }).chosen;
  assert.equal(stale.name, 'booked');
  assert.equal(stale.version, 'r100');
  assert.equal(stale.commit, 'aaa1111');
  assert.equal(stale.players, 0);
});

test('rankHosts: a legacy synthesized entry ranks like any other', () => {
  const hosts = mergeBook({ ws: 'ws://localhost:7780', proto: 12 });
  const probes = new Map([['', ok({ proto: 12 })]]);
  const { chosen } = rankHosts(hosts, { game: 'arena', proto: 12, probes });
  assert.equal(chosen.name, '');
  assert.equal(chosen.url, 'ws://localhost:7780');
});

test('rankHosts: nothing to choose from is null, never a throw', () => {
  assert.equal(rankHosts([], { game: 'arena' }).chosen, null);
  assert.equal(rankHosts(undefined, { game: 'arena' }).chosen, null);
  assert.equal(rankHosts([null, {}, { name: 'x' }], { game: 'arena' }).chosen, null);
});

// ---- the socket paths, over a fake socket --------------------------------

// The smallest thing that behaves like a WebSocket for our purposes: the
// script decides what arrives and when. `open` fires on the next turn of the
// loop so that the caller has installed its handlers first, exactly as a real
// socket does.
function fakeSocket(script) {
  return (url) => {
    const ws = { url, sent: [], closed: false, close() { this.closed = true; } };
    ws.send = (data) => ws.sent.push(JSON.parse(data));
    queueMicrotask(() => {
      if (script.open === false) {
        ws.onerror?.({});
        return;
      }
      ws.onopen?.();
      for (const msg of script.messages || []) {
        ws.onmessage?.({ data: typeof msg === 'string' ? msg : JSON.stringify(msg) });
      }
      if (script.thenClose) ws.onclose?.({});
    });
    return ws;
  };
}

test('listLobbies: accepts the arena tag lobby_list', async () => {
  const rows = [{ name: 'my-arena', host: 'swift-fox', players: 1, cap: 8 }];
  const got = await listLobbies('ws://x', {
    socketFactory: fakeSocket({ messages: [{ t: 'welcome', proto: 12 }, { t: 'lobby_list', lobbies: rows }] }),
  });
  assert.deepEqual(got, rows);
});

test('listLobbies: accepts the Fire Racer tag lobbies', async () => {
  const rows = [{ name: 'race-1', host: 'driver', players: 2, cap: 8 }];
  const got = await listLobbies('ws://x', {
    socketFactory: fakeSocket({ messages: [{ t: 'welcome', proto: 1 }, { t: 'lobbies', lobbies: rows }] }),
  });
  assert.deepEqual(got, rows);
});

test('listLobbies: says hello before it asks, and closes when done', async () => {
  let socket;
  const factory = fakeSocket({ messages: [{ t: 'lobby_list', lobbies: [] }] });
  await listLobbies('ws://x', {
    proto: 12,
    socketFactory: (url) => {
      socket = factory(url);
      return socket;
    },
  });
  assert.deepEqual(socket.sent, [
    { t: 'hello', proto: 12, handle: 'browser' },
    { t: 'list_lobbies' },
  ]);
  assert.equal(socket.closed, true);
});

test('listLobbies: an unreachable host is an empty list, not a rejection', async () => {
  assert.deepEqual(await listLobbies('ws://x', { socketFactory: fakeSocket({ open: false }) }), []);
});

test('probeHost: the first welcome resolves with the round trip and the reply', async () => {
  const r = await probeHost('ws://x', {
    proto: 12,
    socketFactory: fakeSocket({
      messages: [{ t: 'welcome', proto: 12, host: 'amber-otter', version: 'r211', players: 2 }],
    }),
  });
  assert.equal(r.ok, true);
  assert.equal(r.welcome.host, 'amber-otter');
  assert.ok(Number.isFinite(r.rttMs));
});

test('probeHost: a socket that errors or closes first is a clean failure', async () => {
  const bad = await probeHost('ws://x', { socketFactory: fakeSocket({ open: false }) });
  assert.equal(bad.ok, false);
  assert.equal(bad.reason, 'error');

  const gone = await probeHost('ws://x', { socketFactory: fakeSocket({ thenClose: true }) });
  assert.equal(gone.ok, false);
  assert.equal(gone.reason, 'closed');
});

test('probeHost: a timeout resolves and closes the socket', async () => {
  let socket;
  const r = await probeHost('ws://x', {
    timeoutMs: 20,
    socketFactory: (url) => {
      socket = { url, send() {}, close() { this.closed = true; } };
      queueMicrotask(() => socket.onopen?.());
      return socket;
    },
  });
  assert.equal(r.ok, false);
  assert.equal(r.reason, 'timeout');
  assert.equal(socket.closed, true);
});

// ---- the whole pipeline, with fetch and sockets both faked ---------------

test('chooseHost: loads, probes and ranks, mirrors merged in', async () => {
  const realFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    const path = String(url);
    if (path.includes('server.json')) {
      return {
        ok: true,
        json: async () => ({
          v: '1',
          hosts: [
            { name: 'stale-mirror', ws: 'ws://mirror', proto: 12, version: 'r100' },
            { name: 'down', ws: 'ws://down', proto: 12, version: 'r999' },
          ],
          mirrors: ['http://mirror.example/host.json', 'http://gone.example/host.json'],
        }),
      };
    }
    if (path.includes('mirror.example')) {
      return {
        ok: true,
        json: async () => ({ name: 'stale-mirror', ws: 'ws://mirror', proto: 12, version: 'r211' }),
      };
    }
    throw new Error('offline');
  };
  try {
    const { chosen, candidates, hosts } = await chooseHost('http://localhost/', {
      game: 'arena',
      proto: 12,
      pinned: '',
      socketFactory: (url) =>
        url === 'ws://mirror'
          ? fakeSocket({ messages: [{ t: 'welcome', proto: 12, players: 0 }] })(url)
          : fakeSocket({ open: false })(url),
    });
    // The mirror's fresher copy of its own entry replaced the book's.
    assert.equal(hosts.find((h) => h.name === 'stale-mirror').version, 'r211');
    assert.equal(chosen.name, 'stale-mirror');
    // `down` had the newest version in the book and still loses: it is gone.
    assert.deepEqual(
      candidates.map((c) => c.name),
      ['stale-mirror'],
    );
  } finally {
    globalThis.fetch = realFetch;
  }
});

// ---- the chip ------------------------------------------------------------

test('renderChip: names the host, or says there is none', () => {
  const el = { textContent: '', title: '' };
  renderChip(el, { name: 'amber-otter', version: 'r211', rttMs: 43, commit: 'abc1234', url: 'wss://a' });
  assert.equal(el.textContent, 'server: amber-otter · r211 · 43 ms');
  assert.match(el.title, /abc1234/);
  assert.match(el.title, /wss:\/\/a/);

  renderChip(el, null);
  assert.equal(el.textContent, 'no server');

  // The legacy entry has no name; the chip still says something true.
  renderChip(el, { name: '', version: '', rttMs: null, commit: '', url: 'ws://localhost:7780' });
  assert.equal(el.textContent, 'server: server');
});
