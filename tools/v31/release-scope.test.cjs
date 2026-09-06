'use strict';
const os = require('node:os');
os.setPriority(0, os.constants.priority.PRIORITY_LOW);
const test = require('node:test');
const assert = require('node:assert/strict');
const scope = require('./release-scope.cjs');
const fixture = () => {
  const previous = { future: 'keep', games: [
    { id: 'fire', title: 'Peer release', versions: [{ v: 'v2', path: 'games/fire/v2/', live: true, proto: 2 }], extra: { keep: true } },
    { id: 'arena', title: 'Arena', versions: [
      { v: 'v30', path: 'games/arena/v30/', live: true, proto: 23, note: 'preserve this' },
      { v: 'v28', path: 'games/arena/v28/', live: false, proto: 21 },
    ] },
  ] };
  const arena = previous.games[1];
  const source = { games: [{ ...arena, title: 'Killshot', versions: [
    { v: 'v31', path: 'games/arena/v31/', live: true, proto: 24 },
    ...arena.versions.map(version => ({ ...version, live: false })),
  ] }, { id: 'fire', versions: [{ live: true, proto: 1 }] }] };
  return { previous, source };
};
test('single fallback changes without replacing peer-authored launcher content', () => {
  const old = "// peer work\nlet arenaLivePath = 'games/arena/v30/';\nconst peer = 'fire-v2';\n";
  assert.equal(scope.launcher(old), old.replace('/v30/', '/v31/'));
  for (const bad of ['', old + old, old.replace('/v30/', '/v31/')]) assert.throws(() => scope.launcher(bad));
});
test('only Arena display metadata and new live version change; peers and frozen history survive', () => {
  const { previous, source } = fixture(), untouched = structuredClone(previous);
  const next = scope.catalog(previous, source);
  assert.deepEqual(previous, untouched);
  assert.equal(next.future, 'keep');
  assert.deepEqual(next.games[0], untouched.games[0]);
  assert.equal(next.games[1].title, 'Killshot');
  assert.deepEqual(next.games[1].versions[1], { ...previous.games[1].versions[0], live: false });
  assert.deepEqual(next.games[1].versions[2], previous.games[1].versions[1]);
});
test('lost history, changed frozen metadata, wrong protocol, duplicates and unexpected live release fail closed', () => {
  for (const mutate of [
    ({ source }) => source.games[0].versions.pop(),
    ({ source }) => { source.games[0].versions[1].note = 'rewritten'; },
    ({ source }) => { source.games[0].versions[0].proto = 23; },
    ({ source }) => { source.games[0].versions[1].live = true; },
    ({ source }) => { source.games[0].id = 'killshot'; },
    ({ source }) => source.games.push(source.games[0]),
    ({ source }) => source.games[0].versions.push(source.games[0].versions[0]),
    ({ previous }) => { previous.games[1].versions[0].path = 'games/arena/v31/'; },
    ({ source }) => { source.games[0].title = 'Arena'; },
  ]) {
    const value = fixture(); mutate(value);
    assert.throws(() => scope.catalog(value.previous, value.source));
  }
});
test('only the eight Arena release paths are eligible for publication', () => {
  assert.equal(scope.allowed.length, 8);
  assert.equal(new Set(scope.allowed).size, 8);
  assert(!scope.allowed.some(file => file.includes('/fire/') || file.includes('/v30/')));
  assert(scope.allowed.includes('games/arena/v31/pkg/arena_bg.wasm'));
});

test('public release accepts v31 protocol24 and rejects protocol23 in either book or client', () => {
  const { source } = fixture();
  assert.equal(scope.PROTO, 24);
  assert.equal(scope.assertLive({ proto: 24 }, source), source.games[0].versions[0]);
  assert.throws(() => scope.assertLive({ proto: 23 }, source), /address book/);
  const oldProtocol = structuredClone(source); oldProtocol.games[0].versions[0].proto = 23;
  assert.throws(() => scope.assertLive({ proto: 24 }, oldProtocol), /client protocol/);
});

test('public release rejects duplicate identities, duplicate live versions and stale paths', () => {
  for (const mutate of [
    value => value.games.push(structuredClone(value.games[0])),
    value => value.games[0].versions.push(structuredClone(value.games[0].versions[0])),
    value => { value.games[0].versions[0].path = 'games/arena/v30/'; },
    value => { value.games[0].versions[0].v = 'v30'; },
    value => { value.games[0].versions[0].live = false; },
  ]) {
    const { source } = fixture(); mutate(source);
    assert.throws(() => scope.assertLive({ proto: 24 }, source));
  }
});
