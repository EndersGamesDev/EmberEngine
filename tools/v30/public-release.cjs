// Read-only HTTP/Welcome proof after the scoped v30 publication.
// No room creation, input, server control or public writes.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const os = require('node:os');
const { execFileSync } = require('node:child_process');
const { readyHost, arenaBook } = require('./release-book.cjs');
const scope = require('./release-scope.cjs');
const publication = JSON.parse(fs.readFileSync('target/killshot-publish/results.json'));
assert(publication.pushed && publication.pagesCommit, 'A successful publication report is required');
const preserved = process.env.EMBER_QA_PRESERVED_PAGES || publication.base;
assert.equal(preserved, publication.base, 'Preservation base differs from actual publication');
const base = 'https://endersgamesdev.github.io/EmberEngine/';
const output = path.resolve(process.env.EMBER_QA_OUTPUT || 'target/killshot-public');
const started = Date.now(), files = [];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = (...args) => execFileSync('git', args, { windowsHide: true, maxBuffer: 64 * 1024 * 1024 });
const released = file => git('show', `${publication.pagesCommit}:${file}`);
async function get(file) {
  const response = await fetch(`${base}${file}${file.includes('?') ? '&' : '?'}qa=${started}`,
    { signal: AbortSignal.timeout(45000) });
  assert(response.ok, `${file}: HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}
async function checkFile(remote, expected, inherited = false) {
  const bytes = await get(remote), expectedHash = sha(expected);
  assert.equal(sha(bytes), expectedHash, `${inherited ? 'preserved peer/frozen' : 'new Arena'}: ${remote}`);
  files.push({ remote, bytes: bytes.length, sha256: expectedHash, preserved: inherited });
}
async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  for (const field of ['sourceCommit', 'pagesCommit', 'base', 'tree']) {
    assert(/^[a-f0-9]{40}$/.test(publication[field] || ''), `Publication ${field} must be a full commit/tree SHA`);
  }
  assert.equal(String(git('rev-parse', `${publication.pagesCommit}^{tree}`)).trim(), publication.tree, 'Publication tree differs');
  assert.equal(String(git('rev-parse', `${publication.pagesCommit}^`)).trim(), preserved, 'Publication parent differs from preservation base');
  const [book, catalog, version] = await Promise.all(['server.json', 'games.json', 'version.json']
    .map(async file => JSON.parse(await get(file))));
  assert.deepEqual(version, JSON.parse(released('version.json')), 'Published build stamp differs from release commit');
  assert.deepEqual(version, publication.version, 'Build stamp differs from publication attestation');
  assert.equal(book.proto, 23);
  const oldBook = JSON.parse(git('show', `${preserved}:server.json`));
  assert.deepEqual(book, arenaBook(oldBook, { host: { ws: book.ws } }, book.v), 'Only Arena legacy address/protocol/cache may change');
  const live = catalog.games.find(game => game.id === 'arena').versions.filter(version => version.live);
  assert.equal(live.length, 1); assert.equal(live[0].path, 'games/arena/v30/'); assert.equal(live[0].proto, 23);
  const html = String(released('games/arena/v30/index.html'));
  const settings = released('games/arena/v30/settings.js');
  assert.equal(sha(settings), publication.settingsSha256, 'Released settings differ from tested publication');
  assert.equal(html.split(`./settings.js?v=${sha(settings).slice(0, 12)}`).length, 2, 'v30 page/settings cache token');
  assert(/Killshot/i.test(html) && /killshot-hud/.test(html) && /killshot-reload/.test(html), 'Killshot HUD/reload shell missing');
  await checkFile('games/arena/v30/index.html', Buffer.from(html));
  await checkFile('games/arena/v30/settings.js', settings);
  for (const file of ['arena.js', 'arena_bg.wasm']) {
    const remote = `games/arena/v30/pkg/${file}`, expected = released(remote);
    if (file.endsWith('.wasm')) assert.equal(sha(expected), publication.wasmSha256, 'Released WASM differs from tested publication');
    await checkFile(remote, expected);
  }

  // Compare peers against the actual prior Pages tree, NEVER this source
  // branch's older Fire implementation or freshly built unrelated bundles.
  const oldCatalog = JSON.parse(git('show', `${preserved}:games.json`));
  assert.deepEqual(catalog, scope.catalog(oldCatalog, JSON.parse(released('games.json'))), 'Public catalog differs from scoped release');
  assert.deepEqual(catalog.games.filter(game => game.id !== 'arena'), oldCatalog.games.filter(game => game.id !== 'arena'));
  const roots = new Set(['games/arena/v29/']);
  for (const game of oldCatalog.games.filter(game => game.id !== 'arena')) {
    for (const version of game.versions.filter(version => version.live)) roots.add(version.path);
  }
  const inheritedFiles = String(git('ls-tree', '-r', '--name-only', preserved, '--', ...roots)).trim().split('\n').filter(Boolean);
  for (const remote of inheritedFiles) await checkFile(remote, git('show', `${preserved}:${remote}`), true);
  const oldIndex = String(git('show', `${preserved}:index.html`));
  assert.equal(String(await get('index.html')), scope.launcher(oldIndex),
    'Root launcher must only change its Arena fallback');
  const { host, welcome } = await readyHost(book, { fullCommit: publication.sourceCommit, version: version.version });
  assert.equal(host.ws, book.ws, 'Current discovered host and published Arena fallback differ');
  const result = { passed: true, preservedPages: preserved, version, welcome, files,
    limits: 'HTTP bytes and read-only Welcome only. Gameplay, inventory, pickups, reload presentation and eight-player modes require separate network/browser gates.',
    elapsedSeconds: (Date.now() - started) / 1000 };
  fs.mkdirSync(output, { recursive: true }); fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
