// Pure, fail-closed limits for this ONE Arena -> Killshot release.
'use strict';
const assert = require('node:assert/strict');
const ENTRY = 'games/arena/v31';
const PREVIOUS = 'games/arena/v30';
const VERSION = '31.0.0';
const PROTO = 24;
const allowed = Object.freeze(['index.html', 'games.json', 'version.json', 'server.json',
  `${ENTRY}/index.html`, `${ENTRY}/settings.js`, `${ENTRY}/pkg/arena.js`, `${ENTRY}/pkg/arena_bg.wasm`]);

function launcher(previous) {
  const from = `let arenaLivePath = '${PREVIOUS}/';`;
  const to = `let arenaLivePath = '${ENTRY}/';`;
  assert.equal(previous.split(from).length, 2, 'Expected exactly one v30 launcher fallback; review concurrent launcher changes');
  assert(!previous.includes(to), 'v31 launcher fallback already exists');
  return previous.replace(from, to);
}

function oneArena(catalog, label) {
  assert(catalog && Array.isArray(catalog.games), `${label}: games array required`);
  const games = catalog.games.filter(game => game.id === 'arena');
  assert.equal(games.length, 1, `${label}: exactly one Arena identity required`);
  assert(Array.isArray(games[0].versions), `${label}: version array required`);
  return games[0];
}

function assertLive(book, value) {
  assert.equal(book?.proto, PROTO, 'Public address book must use the current release protocol');
  const arena = oneArena(value, 'public');
  assert.equal(arena.title, 'Killshot', 'Public game title differs');
  const live = arena.versions.filter(version => version.live);
  assert.equal(live.length, 1, 'Public Arena must have exactly one live version');
  assert.equal(live[0].v, ENTRY.split('/').at(-1), 'Public live version differs');
  assert.equal(live[0].version, VERSION, 'Public live release version differs');
  assert.equal(live[0].path, `${ENTRY}/`, 'Public live path differs');
  assert.equal(live[0].proto, PROTO, 'Public live client protocol differs');
  return live[0];
}

function catalog(previous, source) {
  const oldArena = oneArena(previous, 'published');
  const newArena = oneArena(source, 'source');
  const before = oldArena.versions.filter(version => version.live);
  assert.equal(before.length, 1, 'Published Arena must have one live version');
  assert.equal(before[0].path, `${PREVIOUS}/`, 'Expected v30 currently live');
  assert.equal(before[0].version, '30.0.0', 'Expected three-grade v30 release version');
  assert.equal(before[0].proto, 23, 'Expected v30 protocol23');
  assert.equal(newArena.title, 'Killshot', 'New display name must be Killshot; internal game identity stays arena');
  const after = newArena.versions.filter(version => version.live);
  assert.equal(after.length, 1, 'Source Arena must have one live version');
  assert.equal(after[0].v, 'v31');
  assert.equal(after[0].version, VERSION);
  assert.equal(after[0].path, `${ENTRY}/`);
  assert.equal(after[0].proto, PROTO);
  assert(!oldArena.versions.some(version => version.path === `${ENTRY}/`), 'v31 already exists; never overwrite an archived release');
  assert.equal(newArena.versions.filter(version => version.path === `${ENTRY}/`).length, 1, 'Duplicate v31 entry');
  assert.deepEqual(newArena.versions.filter(version => version.path !== `${ENTRY}/`),
    oldArena.versions.map(version => version.path === `${PREVIOUS}/` ? { ...version, live: false } : version),
    'Archived Arena metadata must be unchanged except retiring v30');
  const next = { ...previous, games: previous.games.map(game => game.id === 'arena' ? newArena : game) };
  assert.deepEqual(next.games.filter(game => game.id !== 'arena'), previous.games.filter(game => game.id !== 'arena'),
    'Peer catalog entries changed');
  return next;
}

module.exports = { ENTRY, PREVIOUS, VERSION, PROTO, allowed, launcher, catalog, assertLive };
