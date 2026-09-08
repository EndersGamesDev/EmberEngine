// Pure, fail-closed limits for this ONE Arena -> Killshot release.
'use strict';
const assert = require('node:assert/strict');
const ENTRY = 'games/arena/v31';
const PREVIOUS = 'games/arena/v30';
const VERSION = '31.0.0';
const PROTO = 24;
const allowed = Object.freeze(['index.html', 'games.json', 'version.json', 'server.json',
  `${ENTRY}/index.html`, `${ENTRY}/settings.js`, `${ENTRY}/pkg/arena.js`, `${ENTRY}/pkg/arena_bg.wasm`]);

function replaceLauncherFragment(value, from, to, label) {
  if (value.includes(to)) {
    assert(!value.includes(from), `${label}: old and new launcher fragments coexist`);
    return value;
  }
  assert.equal(value.split(from).length, 2, `${label}: expected exactly one old launcher fragment`);
  return value.replace(from, to);
}

function launcher(previous) {
  const from = `let arenaLivePath = '${PREVIOUS}/';`;
  const to = `let arenaLivePath = '${ENTRY}/';`;
  assert.equal(previous.split(from).length, 2, 'Expected exactly one v30 launcher fallback; review concurrent launcher changes');
  assert(!previous.includes(to), 'v31 launcher fallback already exists');
  let next = previous.replace(from, to);
  next = replaceLauncherFragment(next, '\n    const launch = (path, pending) => {',
    "\n    const versionLabel = (entry) => `Version ${entry.version.split('.')[0]}`;\n\n    const launch = (path, pending) => {",
    'version label');
  next = replaceLauncherFragment(next,
    "      updateLabel.textContent = live ? `latest update · ${live.v}` : 'latest update';",
    "      updateLabel.textContent = live ? `latest update · ${versionLabel(live)} · ${live.version}` : 'latest update';",
    'update label');
  next = replaceLauncherFragment(next,
    "        opt.textContent = v.live ? `${v.v} — live` : `${v.v} — ${v.note || 'archived'}`;",
    "        opt.textContent = v.live\n          ? `${versionLabel(v)} (${v.version}) — live`\n          : `${versionLabel(v)} (${v.version}) — ${v.note || 'archived'}`;",
    'version selector');
  next = replaceLauncherFragment(next,
    "      btn.textContent = isLab ? 'Open lab' : `Play ${live ? live.v : ''}`.trim();\n      btn.title = isLab ? `Open ${g.title}` : `Play the latest ${g.title} build`;",
    "      btn.textContent = isLab ? 'Open lab' : `Play ${live ? versionLabel(live) : ''}`.trim();\n      btn.title = live\n        ? `${isLab ? 'Open' : 'Play'} ${g.title} ${live.version}`\n        : `${isLab ? 'Open' : 'Play'} ${g.title}`;",
    'play button');
  return next;
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

function catalogVersions(previousGame, sourceGame, label) {
  assert(sourceGame, `${label}: source game required`);
  return previousGame.versions.map(oldRelease => {
    const matches = sourceGame.versions.filter(release => release.v === oldRelease.v && release.path === oldRelease.path);
    assert.equal(matches.length, 1, `${label} ${oldRelease.v}: source release required`);
    const version = matches[0].version;
    assert.match(version, /^[0-9]+\.[0-9]+\.[0-9]+$/, `${label} ${oldRelease.v}: three-grade version required`);
    if (oldRelease.version !== undefined) {
      assert.equal(version, oldRelease.version, `${label} ${oldRelease.v}: published release version changed`);
    }
    return { ...oldRelease, version };
  });
}

function catalog(previous, source) {
  const oldArena = oneArena(previous, 'published');
  const newArena = oneArena(source, 'source');
  assert.equal(source.games.length, previous.games.length, 'Source and published game identities differ');
  const sourceGames = new Map(source.games.map(game => [game.id, game]));
  assert.equal(sourceGames.size, source.games.length, 'Source game identities must be unique');
  for (const game of previous.games) assert(sourceGames.has(game.id), `Source is missing ${game.id}`);
  const before = oldArena.versions.filter(version => version.live);
  assert.equal(before.length, 1, 'Published Arena must have one live version');
  assert.equal(before[0].path, `${PREVIOUS}/`, 'Expected v30 currently live');
  if (before[0].version !== undefined) assert.equal(before[0].version, '30.0.0', 'Expected three-grade v30 release version');
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
  const versionedArena = catalogVersions(oldArena, newArena, 'Arena');
  for (const release of versionedArena) {
    assert.equal(release.version.split('.')[0], release.v.slice(1), `Arena ${release.v}: major must select its slot`);
  }
  assert.deepEqual(newArena.versions.filter(version => version.path !== `${ENTRY}/`),
    versionedArena.map(version => version.path === `${PREVIOUS}/` ? { ...version, live: false } : version),
    'Archived Arena metadata must be unchanged except adding versions and retiring v30');
  const games = previous.games.map(game => {
    if (game.id === 'arena') return newArena;
    return { ...game, versions: catalogVersions(game, sourceGames.get(game.id), game.id) };
  });
  const next = { ...previous, games };
  return next;
}

module.exports = { ENTRY, PREVIOUS, VERSION, PROTO, allowed, launcher, catalog, assertLive };
