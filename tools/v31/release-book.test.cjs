'use strict';
const os = require('node:os');
os.setPriority(0, os.constants.priority.PRIORITY_LOW);
const test = require('node:test');
const assert = require('node:assert/strict');
const { readyHost, arenaBook } = require('./release-book.cjs');
const fullCommit = 'a'.repeat(40), expected = { fullCommit, version: 'r1490' };
const book = () => ({ proto: 23, ws: 'wss://old.example', v: 'old', fire_proto: 2,
  fire_ws: 'wss://fire.example', kings_ws: 'wss://kings.example', unknown: { keep: true },
  hosts: [{ name: 'dusky-osprey', ws: 'wss://old.example', proto: 23, fire_ws: 'wss://fire.example', fire_proto: 2 }],
  mirrors: [{ name: 'knecht24', url: 'https://mirror.example/host.json' }] });
const mirror = { name: 'knecht24', ws: 'wss://new.example', proto: 24, version: 'r1490', commit: 'aaaaaaa',
  fire_ws: 'wss://mirror-fire.example', kings_ws: 'wss://mirror-kings.example' };
const options = (overrides = {}) => ({ fetchEntry: async () => mirror,
  probe: async url => url === mirror.ws ? { ok: true, welcome: { host: mirror.name, proto: 24, commit: 'aaaaaaa', version: 'r1490' } } : { ok: false },
  resolveCommit: sha => { assert.match(sha, /^[a-f0-9]{7,40}$/); return fullCommit; }, ...overrides });

test('mirror-only transition changes only legacy Arena keys and never masks future mirror rotation', async () => {
  const original = book(), before = structuredClone(original);
  const selected = await readyHost(original, expected, options());
  const next = arenaBook(original, selected, 'new');
  assert.deepEqual(original, before);
  assert.deepEqual(next, { ...before, proto: 24, ws: mirror.ws, v: 'new' });
  assert(!next.hosts.some(host => host.name === mirror.name));
  const rotated = { ...mirror, ws: 'wss://rotated.example' };
  const second = await readyHost(next, expected, options({ fetchEntry: async () => rotated,
    probe: async url => ({ ok: url === rotated.ws, welcome: { host: mirror.name, proto: 24, version: 'r1490', commit: 'aaaaaaa' } }) }));
  assert.equal(second.host.ws, rotated.ws);
});
test('live Welcome outranks stale listed protocol and preserves same-host peer fields', async () => {
  const original = book(); original.hosts.push({ ...mirror, proto: 23, commit: 'bbbbbbb', extra: 'keep' });
  const selected = await readyHost(original, expected, options({ fetchEntry: async () => { throw new Error('mirror down'); } }));
  assert.equal(selected.host.name, mirror.name);
  assert.deepEqual(arenaBook(original, selected).hosts, original.hosts);
});
test('bound mirror cannot override a listed host of the same name', async () => {
  const original = book(); original.hosts.push({ name: mirror.name, ws: 'wss://stale.example', proto: 23 });
  await assert.rejects(readyHost(original, expected, options()), /No reachable/);
});
test('malformed, wrong-name, whole-book or unavailable mirror fails closed', async () => {
  for (const value of [null, [], { ...mirror, name: 'somebody-else' }, { ...mirror, hosts: [] }]) {
    await assert.rejects(readyHost(book(), expected, options({ fetchEntry: async () => value })), /No reachable/);
  }
  await assert.rejects(readyHost(book(), expected, options({ fetchEntry: async () => { throw new Error('offline'); } })), /No reachable/);
});
test('one broken mirror does not hide another valid bound mirror', async () => {
  const original = book(); original.mirrors.unshift({ name: 'broken-host', url: 'https://broken.example' });
  const selected = await readyHost(original, expected, options({ fetchEntry: async url => {
    if (url.startsWith('https://broken.example')) throw new Error('offline'); return mirror;
  } }));
  assert.equal(selected.host.name, mirror.name);
});
test('wrong live identity, protocol, build version, short/fake/ambiguous commit are refused', async () => {
  for (const patch of [{ host: 'wrong-host' }, { proto: 23 }, { version: 'r1489' }, { commit: 'a' }, { commit: 'not-a-sha' }]) {
    await assert.rejects(readyHost(book(), expected, options({ probe: async () => ({ ok: true,
      welcome: { host: mirror.name, proto: 24, version: 'r1490', commit: 'aaaaaaa', ...patch } }) })), /No reachable/);
  }
  await assert.rejects(readyHost(book(), expected, options({ resolveCommit: () => 'b'.repeat(40) })), /No reachable/);
  await assert.rejects(readyHost(book(), expected, options({ resolveCommit: () => { throw new Error('ambiguous'); } })), /No reachable/);
});
