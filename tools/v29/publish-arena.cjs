// Retained Arena v29 Pages preparation tool. Remote publication is retired.
// Run after a clean source commit, its Arena build, and server-first proto22 deployment.
// The client protocol/address transition is atomic with the eight allowed files.
// Required build attestation: --build-commit=<full SHA> --wasm-sha256=<tested SHA256>.
// Those values must be recorded by the operator at build/QA time, not guessed here.
// No builds, service changes, address-book writes, force push or peer rebuilds.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { readyHost, arenaBook } = require('./release-book.cjs');
const root = process.cwd(), started = Date.now();
const args = process.argv.slice(2);
const option = name => args.find(arg => arg.startsWith(`--${name}=`))?.slice(name.length + 3);
assert(!args.includes('--push'), 'Direct Pages publication is retired; release tags publish through Actions');
assert(args.every(arg => /^--(?:build-commit|wasm-sha256)=/.test(arg)), 'Unknown argument');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = (directory, ...argv) => execFileSync('git', ['-c', 'core.autocrlf=false', '-C', directory, ...argv],
  { windowsHide: true, maxBuffer: 64 * 1024 * 1024 });
const text = (directory, ...argv) => String(git(directory, ...argv)).trim();
const read = file => fs.readFileSync(path.join(root, file));
const entry = 'games/arena/v29';
const allowed = ['index.html', 'games.json', 'version.json', 'server.json', `${entry}/index.html`,
  `${entry}/settings.js`, `${entry}/pkg/arena.js`, `${entry}/pkg/arena_bg.wasm`];
let worktree;
async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  assert.equal(text(root, 'status', '--porcelain'), '', 'Source must be completely clean, including untracked files');
  const commit = text(root, 'rev-parse', 'HEAD');
  assert.equal(option('build-commit'), commit, 'Recorded clean-build revision does not equal HEAD');
  assert(/^[a-f0-9]{64}$/.test(option('wasm-sha256') || ''), 'Explicit tested WASM SHA256 is required');
  const wasm = read('web/pkg/arena_bg.wasm');
  assert.equal(sha(wasm), option('wasm-sha256'), 'Tested WASM bytes changed');
  const arenaJs = read('web/pkg/arena.js');
  assert(WebAssembly.validate(wasm), 'Arena WASM is invalid');
  assert(/PROTO_VERSION: u16 = 22\b/.test(String(read('crates/arena-core/src/proto.rs'))), 'Source protocol must be22');
  git(root, 'fetch', 'origin', 'main', 'gh-pages');
  assert.equal(text(root, 'rev-parse', 'origin/main'), commit, 'HEAD must be the current published main revision');
  const base = text(root, 'rev-parse', 'origin/gh-pages');
  const getBase = file => git(root, 'show', `${base}:${file}`);
  const bookBytes = getBase('server.json'), book = JSON.parse(bookBytes);
  const expected = { fullCommit: commit, version: `r${text(root, 'rev-list', '--count', 'HEAD')}` };
  const selected = await readyHost(book, expected);
  const live = selected.welcome;
  const nextBook = arenaBook(book, selected);

  const catalog = JSON.parse(getBase('games.json'));
  const sourceCatalog = JSON.parse(read('web/games.json'));
  const updatedArena = sourceCatalog.games.filter(game => game.id === 'arena');
  assert.equal(updatedArena.length, 1, 'Source must contain one Arena catalog entry');
  const arenaLive = updatedArena[0].versions.filter(version => version.live);
  assert.equal(arenaLive.length, 1); assert.equal(arenaLive[0].path, `${entry}/`); assert.equal(arenaLive[0].proto, 22);
  assert.equal(catalog.games.filter(game => game.id === 'arena').length, 1, 'Published Arena entry must exist exactly once');
  const priorPeers = catalog.games.filter(game => game.id !== 'arena');
  catalog.games = catalog.games.map(game => game.id === 'arena' ? updatedArena[0] : game);
  assert.deepEqual(catalog.games.filter(game => game.id !== 'arena'), priorPeers, 'Peer catalog entries changed');
  const oldIndex = String(getBase('index.html'));
  const fallback = "let arenaLivePath = 'games/arena/v28/';";
  assert.equal(oldIndex.split(fallback).length, 2, 'Expected one v28 fallback; review a concurrent launcher change before publishing');
  const index = oldIndex.replace(fallback, "let arenaLivePath = 'games/arena/v29/';");
  const settings = read(`web/${entry}/settings.js`), settingsHash = sha(settings);
  const sourcePage = String(read(`web/${entry}/index.html`));
  assert.equal(sourcePage.split('./settings.js?v=1').length, 2, 'Expected one settings cache placeholder');
  const html = sourcePage.replace('./settings.js?v=1', `./settings.js?v=${settingsHash.slice(0, 12)}`);
  const version = { version: `r${text(root, 'rev-list', '--count', 'HEAD')}`, commit: text(root, 'rev-parse', '--short', 'HEAD'),
    built: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'), subject: text(root, 'log', '-1', '--pretty=%s') };

  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-v29-pages-'));
  worktree = path.join(temporary, 'pages');
  assert(path.resolve(worktree).startsWith(path.resolve(temporary) + path.sep), 'Invalid temporary worktree');
  git(root, 'worktree', 'add', '--detach', '--no-checkout', worktree, base);
  git(worktree, 'sparse-checkout', 'set', '--no-cone', '/index.html', '/games.json', '/version.json', '/server.json', `/${entry}/`);
  // --no-checkout can leave an empty index. Populate the ENTIRE tracked index
  // before staging, otherwise a partial checkout can silently delete peers.
  git(worktree, 'read-tree', '-mu', 'HEAD');
  assert.equal(text(worktree, 'write-tree'), text(root, 'rev-parse', `${base}^{tree}`), 'Sparse index does not preserve the original complete tree');
  const writes = { 'index.html': index, 'games.json': JSON.stringify(catalog, null, 2) + '\n',
    'version.json': JSON.stringify(version, null, 2) + '\n', 'server.json': JSON.stringify(nextBook, null, 2) + '\n', [`${entry}/index.html`]: html,
    [`${entry}/settings.js`]: settings, [`${entry}/pkg/arena.js`]: arenaJs, [`${entry}/pkg/arena_bg.wasm`]: wasm };
  for (const [relative, bytes] of Object.entries(writes)) {
    const file = path.join(worktree, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, bytes);
  }
  fs.writeFileSync(path.join(root, 'web', 'version.json'), writes['version.json']);
  git(worktree, 'add', '--', ...allowed);
  const changes = text(worktree, 'diff', '--cached', '--name-status').split('\n').filter(Boolean);
  assert(changes.length > 0);
  for (const change of changes) {
    const [status, file] = change.split('\t');
    assert(['A', 'M'].includes(status) && allowed.includes(file), `Out-of-scope staged change: ${change}`);
  }
  const tree = text(worktree, 'write-tree');
  assert.deepEqual(JSON.parse(git(worktree, 'show', `${tree}:server.json`)), nextBook, 'Unexpected address-book change');
  const frozen = ['games/fire', 'games/kings', 'games/what-is-this', 'labs', 'games/arena/v28'];
  for (const prefix of frozen) assert.equal(text(root, 'rev-parse', `${base}:${prefix}`),
    text(worktree, 'rev-parse', `${tree}:${prefix}`), `Peer/frozen tree changed: ${prefix}`);
  assert.equal(text(root, 'status', '--porcelain'), '', 'Source changed during publication preparation');
  assert.equal(sha(read('web/pkg/arena_bg.wasm')), option('wasm-sha256'), 'Build changed during preparation');
  const rechecked = await readyHost(book, expected);
  assert.equal(rechecked.host.name, selected.host.name, 'Selected server changed; rerun preparation');
  assert.equal(rechecked.host.ws, selected.host.ws, 'Host tunnel rotated; rerun preparation');
  const remote = text(root, 'ls-remote', 'origin', 'refs/heads/gh-pages').split(/\s+/)[0];
  assert.equal(remote, base, 'Another worker updated Pages; rerun preparation against their new tree');
  const report = { prepared: true, pushed: false, sourceCommit: commit, base, tree, worktree, version,
    wasmSha256: sha(wasm), settingsSha256: settingsHash, liveWelcome: live, selectedHost: selected.host.name, changes,
    frozen, buildAttestation: 'Operator-supplied clean-build revision and tested WASM hash; no compiler is run by this publisher.' };
  report.elapsedSeconds = (Date.now() - started) / 1000;
  const output = path.join(root, 'target', 'parkour-publish'); fs.mkdirSync(output, { recursive: true });
  fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
  console.log('PREPARED ONLY. Review the staged diff in the reported worktree; release publication belongs to the tag workflow.');
  console.log('Temporary worktree retained for inspection; no recursive cleanup is performed.');
}
main().catch(error => { console.error(error); if (worktree) console.error(`Retained worktree: ${worktree}`); process.exitCode = 1; });
