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
  loadBook,
  mergeBook,
  parseVersion,
  probeHost,
  rankHosts,
  renderChip,
  verKeysFor,
} from './hosts.js';

// ---- key derivation ------------------------------------------------------

test('keysFor: the arena owns the bare names, everyone else is prefixed', () => {
  assert.deepEqual(keysFor('arena'), { ws: 'ws', proto: 'proto' });
  assert.deepEqual(keysFor('fire'), { ws: 'fire_ws', proto: 'fire_proto' });
  assert.deepEqual(keysFor('kings'), { ws: 'kings_ws', proto: 'kings_proto' });
});

test('verKeysFor: the build stamp is derived the same way as the address', () => {
  assert.deepEqual(verKeysFor('arena'), { version: 'version', commit: 'commit' });
  assert.deepEqual(verKeysFor('fire'), { version: 'fire_version', commit: 'fire_commit' });
  assert.deepEqual(verKeysFor('kings'), { version: 'kings_version', commit: 'kings_commit' });
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

const mirror = (name, entry) => ({ name, entry });

test('mergeBook: entries merge by name and the later one wins', () => {
  const book = {
    hosts: [
      { name: 'amber-otter', ws: 'wss://a', version: 'r200' },
      { name: 'flint-heron', ws: 'wss://b', version: 'r100' },
      { name: 'amber-otter', ws: 'wss://a2', version: 'r211' },
    ],
  };
  const merged = mergeBook(book);
  assert.equal(merged.length, 2);
  assert.equal(merged[0].name, 'amber-otter');
  assert.equal(merged[0].ws, 'wss://a2');
  assert.equal(merged[0].version, 'r211');
  assert.equal(merged[1].name, 'flint-heron');
});

test('mergeBook: a mirror adds the one entry it is bound to', () => {
  const book = { hosts: [{ name: 'flint-heron', ws: 'wss://b', version: 'r100' }] };
  const merged = mergeBook(book, [
    mirror('amber-otter', { name: 'amber-otter', ws: 'wss://a', version: 'r211' }),
  ]);
  assert.deepEqual(merged.map((h) => h.name), ['flint-heron', 'amber-otter']);
  assert.equal(merged[1].ws, 'wss://a');
});

test('mergeBook: a mirror cannot publish another host’s entry', () => {
  const book = { hosts: [{ name: 'flint-heron', ws: 'wss://real', version: 'r100' }] };
  // The classic hijack: the mirror is bound to `amber-otter` and serves
  // `flint-heron` pointing at its own socket.
  const merged = mergeBook(book, [
    mirror('amber-otter', { name: 'flint-heron', ws: 'wss://evil', version: 'r999999' }),
  ]);
  assert.deepEqual(merged.map((h) => h.name), ['flint-heron']);
  assert.equal(merged[0].ws, 'wss://real');
});

test('mergeBook: the book’s own entry wins over a mirror of the same name', () => {
  const book = { hosts: [{ name: 'amber-otter', ws: 'wss://real', version: 'r100' }] };
  const merged = mergeBook(book, [
    mirror('amber-otter', { name: 'amber-otter', ws: 'wss://evil', version: 'r999999' }),
  ]);
  assert.equal(merged.length, 1);
  assert.equal(merged[0].ws, 'wss://real');
});

test('mergeBook: a mirror may not serve an array or a whole book', () => {
  const book = { hosts: [] };
  const asArray = mergeBook(book, [mirror('amber-otter', [{ name: 'amber-otter', ws: 'wss://a' }])]);
  const asBook = mergeBook(book, [mirror('amber-otter', { hosts: [{ name: 'amber-otter', ws: 'wss://a' }] })]);
  assert.deepEqual(asArray, []);
  assert.deepEqual(asBook, []);
});

test('mergeBook: a name outside [a-z0-9-]{3,32} is not a host', () => {
  const merged = mergeBook({
    hosts: [
      { name: 'Amber Otter', ws: 'wss://a' },
      { name: 'ok', ws: 'wss://b' },
      { name: 'amber-otter', ws: 'wss://c' },
    ],
  });
  assert.deepEqual(merged.map((h) => h.name), ['amber-otter']);
  // And a mirror bound to a malformed name is not bound at all.
  assert.deepEqual(mergeBook({}, [mirror('a/b', { name: 'a/b', ws: 'wss://x' })]), []);
});

test('mergeBook: address keys leave the module as strings or not at all', () => {
  const merged = mergeBook({
    hosts: [{ name: 'amber-otter', ws: 123, fire_ws: 'wss://f', proto: 'twelve', fire_proto: 1 }],
  });
  assert.equal(merged.length, 1);
  assert.equal('ws' in merged[0], false);
  assert.equal('proto' in merged[0], false);
  assert.equal(merged[0].fire_ws, 'wss://f');
  assert.equal(merged[0].fire_proto, 1);
});

test('mergeBook: neither the book nor its mirrors can grow the list without end', () => {
  const hosts = Array.from({ length: 500 }, (_, i) => ({
    name: `host-${String(i).padStart(4, '0')}`,
    ws: `wss://${i}`,
  }));
  const mirrors = Array.from({ length: 500 }, (_, i) => {
    const name = `mirror-${String(i).padStart(4, '0')}`;
    return mirror(name, { name, ws: `wss://m${i}` });
  });
  assert.equal(mergeBook({ hosts }, mirrors).length, 128);
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

test('mergeBook: a mirror cannot erase the legacy address', () => {
  const merged = mergeBook(
    { ws: 'wss://old', proto: 12 },
    [mirror('stranger-heron', { name: 'stranger-heron', ws: 'wss://x' })],
  );
  assert.equal(merged.length, 2);
  const legacy = merged.find((h) => h.legacy);
  assert.equal(legacy.ws, 'wss://old');
  assert.equal(legacy.proto, 12);
  assert.equal(legacy.name, '');
  // Seeded first, so the legacy address is also the first fallback.
  assert.equal(merged[0], legacy);
});

test('mergeBook: a half-migrated book keeps the other game’s legacy address', () => {
  // The first per-game publish has written hosts[] for the arena only; the
  // still-live fire address is only at the top level.
  const merged = mergeBook({
    proto: 12,
    fire_proto: 1,
    ws: 'wss://old-arena',
    fire_ws: 'wss://live-fire',
    hosts: [{ name: 'amber-otter', ws: 'wss://new-arena', proto: 12, version: 'r211' }],
  });
  assert.equal(merged.length, 2);
  assert.equal(merged[0].name, 'amber-otter');
  const legacy = merged[1];
  assert.equal(legacy.legacy, true);
  assert.equal(legacy.fire_ws, 'wss://live-fire');
  assert.equal(legacy.fire_proto, 1);
  // The arena is carried by a real entry, so the stale top-level `ws` is not
  // resurrected as a second arena host.
  assert.equal('ws' in legacy, false);
  assert.deepEqual(
    rankHosts(merged, { game: 'fire', proto: 1 }).candidates.map((c) => c.url),
    ['wss://live-fire'],
  );
});

test('mergeBook: a book whose hosts cover every game synthesises nothing', () => {
  const merged = mergeBook({
    ws: 'wss://old-arena',
    proto: 12,
    hosts: [{ name: 'amber-otter', ws: 'wss://new-arena', proto: 12 }],
  });
  assert.deepEqual(merged.map((h) => h.name), ['amber-otter']);
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

test('rankHosts: the newest version wins, then name inside it', () => {
  const hosts = [
    entry('old', { version: 'r100' }),
    entry('newer', { version: 'r211' }),
    entry('newest-b', { version: 'r300', updated: '2026-09-01T11:00:00Z' }),
    entry('newest-a', { version: 'r300', updated: '2026-09-01T09:00:00Z' }),
  ];
  const { chosen, candidates } = rankHosts(hosts, { game: 'arena' });
  // No probes at all: players and rtt tie, so NAME decides inside the newest
  // build (§5 step 5) — asserted AGAINST the `updated` order, which is a
  // property of the head sort and not of this one.
  assert.equal(chosen.name, 'newest-a');
  assert.deepEqual(
    candidates.map((c) => c.name),
    ['newest-a', 'newest-b', 'newer', 'old'],
  );
});

test('rankHosts: below the newest build, the newer `updated` ranks first', () => {
  // §5 step 3's `updated` clause survives only in the tail: the head is
  // re-sorted by load and rtt, so this is the one place it decides anything.
  const hosts = [
    entry('newest', { version: 'r300' }),
    entry('a-stale', { version: 'r100', updated: '2026-09-01T09:00:00Z' }),
    entry('b-fresh', { version: 'r100', updated: '2026-09-01T11:00:00Z' }),
  ];
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena' }).candidates.map((c) => c.name),
    ['newest', 'b-fresh', 'a-stale'],
  );
});

test('rankHosts: a game’s own build stamp wins, and falls back to the bare one', () => {
  const hosts = [
    // Written by the current writer: the arena is old here, fire is new.
    { name: 'split-stamp', ws: 'wss://a', fire_ws: 'wss://f', version: 'r100', fire_version: 'r300' },
    // Written by an older writer that knew only the bare pair.
    { name: 'old-writer', ws: 'wss://b', fire_ws: 'wss://g', version: 'r200', commit: 'aaa1111' },
  ];
  assert.deepEqual(
    rankHosts(hosts, { game: 'arena' }).candidates.map((c) => c.name),
    ['old-writer', 'split-stamp'],
  );
  assert.deepEqual(
    rankHosts(hosts, { game: 'fire' }).candidates.map((c) => c.name),
    ['split-stamp', 'old-writer'],
  );
  // The fallback is what keeps an older entry from ranking as an unstamped 0.
  assert.equal(rankHosts(hosts, { game: 'fire' }).candidates[1].versionNum, 200);
  assert.equal(rankHosts(hosts, { game: 'fire' }).candidates[1].commit, 'aaa1111');
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
  const r = rankHosts(hosts, { game: 'arena' });
  assert.equal(r.candidates.length, 2);
  assert.deepEqual(r.wrongProto, []);
});

test('rankHosts: a host that answered on another protocol is reported, not lost', () => {
  // The protocol-bump window: the page is on 12, the only host still on 11.
  const hosts = [entry('amber-otter', { proto: 11 })];
  const probes = new Map([['amber-otter', ok({ proto: 11 })]]);
  const r = rankHosts(hosts, { game: 'arena', proto: 12, probes });
  assert.equal(r.chosen, null);
  assert.deepEqual(r.candidates, []);
  assert.deepEqual(r.wrongProto.map((v) => [v.name, v.proto, v.url]), [['amber-otter', 11, 'wss://amber-otter']]);
});

test('rankHosts: a host that never answered is not reported as a mismatch', () => {
  const hosts = [entry('down', { proto: 11 })];
  const r = rankHosts(hosts, { game: 'arena', proto: 12, probes: new Map([['down', dead]]) });
  assert.deepEqual(r.wrongProto, []);
});

test('renderChip: a protocol mismatch is not "offline"', () => {
  const el = { textContent: '', title: '' };
  renderChip(el, null, { wrongProto: [{ name: 'amber-otter', proto: 11 }], proto: 12 });
  assert.equal(el.textContent, 'wrong protocol');
  assert.match(el.title, /this build speaks protocol 12/);
  assert.match(el.title, /amber-otter speaks 11/);

  renderChip(el, null, { wrongProto: [], proto: 12 });
  assert.equal(el.textContent, 'no server');
  assert.match(el.title, /offline/);
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

test('rankHosts: the pin matches the book name, never a live Welcome name', () => {
  // A host reporting someone else's name in its Welcome must not absorb the
  // pin: the pin is written from the hosts strip, which is keyed on the book
  // name, so the real `pinned` entry is the one that has to be promoted.
  const hosts = [entry('impostor'), entry('pinned', { version: 'r100' })];
  const probes = new Map([
    ['impostor', ok({ host: 'pinned' })],
    ['pinned', ok()],
  ]);
  const { chosen } = rankHosts(hosts, { game: 'arena', pinned: 'pinned', probes });
  assert.equal(chosen.bookName, 'pinned');
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

// The smallest thing that behaves like the half of `Response` fetchJson uses.
const fakeResponse = (body) => ({
  ok: true,
  headers: { get: () => null },
  text: async () => (typeof body === 'string' ? body : JSON.stringify(body)),
});

async function withFetch(routes, fn) {
  const realFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    const path = String(url);
    for (const [needle, body] of routes) {
      if (path.includes(needle)) return fakeResponse(body);
    }
    throw new Error('offline');
  };
  try { return await fn(); } finally { globalThis.fetch = realFetch; }
}

test('chooseHost: loads, probes and ranks, a bound mirror merged in', async () => {
  const r = await withFetch([
    ['server.json', {
      v: '1',
      hosts: [{ name: 'down', ws: 'ws://down', proto: 12, version: 'r999' }],
      mirrors: [
        { url: 'http://mirror.example/host.json', name: 'quiet-raven' },
        { url: 'http://gone.example/host.json', name: 'flint-heron' },
      ],
    }],
    ['mirror.example', { name: 'quiet-raven', ws: 'ws://mirror', proto: 12, version: 'r211' }],
  ], () => chooseHost('http://localhost/', {
    game: 'arena',
    proto: 12,
    pinned: '',
    socketFactory: (url) =>
      url === 'ws://mirror'
        ? fakeSocket({ messages: [{ t: 'welcome', proto: 12, players: 0 }] })(url)
        : fakeSocket({ open: false })(url),
  }));
  // The mirror published its own entry, under the name it is bound to.
  assert.equal(r.hosts.find((h) => h.name === 'quiet-raven').version, 'r211');
  assert.equal(r.chosen.name, 'quiet-raven');
  // `down` had the newest version in the book and still loses: it is gone.
  assert.deepEqual(r.candidates.map((c) => c.name), ['quiet-raven']);
});

test('loadBook: a mirror serving another host’s address cannot redirect anyone', async () => {
  const { hosts } = await withFetch([
    ['server.json', {
      hosts: [{ name: 'amber-otter', ws: 'wss://real', proto: 12, version: 'r211' }],
      mirrors: [{ url: 'http://evil.example/host.json', name: 'quiet-raven' }],
    }],
    ['evil.example', { name: 'amber-otter', ws: 'wss://evil', proto: 12, version: 'r999999' }],
  ], () => loadBook('http://localhost/'));
  assert.deepEqual(hosts.map((h) => h.ws), ['wss://real']);
});

test('loadBook: a bare mirror URL is unbound, so nothing it serves is trusted', async () => {
  const { hosts } = await withFetch([
    ['server.json', {
      hosts: [{ name: 'amber-otter', ws: 'wss://real', proto: 12 }],
      mirrors: ['http://legacy.example/host.json'],
    }],
    ['legacy.example', { name: 'quiet-raven', ws: 'wss://x', proto: 12 }],
  ], () => loadBook('http://localhost/'));
  // The old shape names no host, so nothing it serves can be attributed.
  assert.deepEqual(hosts.map((h) => h.name), ['amber-otter']);
});

test('loadBook: a huge mirror payload is an absence, never a rejection', async () => {
  // Both halves of the old spread: 200k entries in one payload, and a body
  // far past the size guard. Neither may reject the promise both game pages
  // await at module top level.
  const flood = JSON.stringify({ hosts: Array.from({ length: 200000 }, (_, i) => ({ name: `h-${i}`, ws: `wss://${i}` })) });
  const { hosts } = await withFetch([
    ['server.json', {
      hosts: [{ name: 'amber-otter', ws: 'wss://real', proto: 12 }],
      mirrors: [{ url: 'http://flood.example/host.json', name: 'quiet-raven' }],
    }],
    ['flood.example', flood],
  ], () => loadBook('http://localhost/'));
  assert.deepEqual(hosts.map((h) => h.name), ['amber-otter']);
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
