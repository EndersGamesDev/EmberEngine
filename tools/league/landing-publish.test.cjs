// Isolated local Git fixtures only; no public origin, Pages push or browser.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');
const { execFileSync } = require('node:child_process');
const { patchHub, landingPath, landingFiles, proveLandingScope, parseArgs, main, ANCHOR, MARKER, HUB_LINK } = require('./publish-landing.cjs');
const { hash } = require('./publish.cjs');
os.setPriority(0, os.constants.priority.PRIORITY_LOW);
const git = (cwd, ...args) => execFileSync('git', ['-c', 'core.autocrlf=false', '-C', cwd, ...args], {
  windowsHide: true, stdio: 'pipe', env: { ...process.env, GIT_ALLOW_PROTOCOL: 'file' },
}).toString().trim();
function write(root, name, bytes) { const file = path.join(root, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, bytes); }
function removeTemporary(directory, prefix) {
  const absolute = path.resolve(directory);
  assert.equal(path.dirname(absolute), path.resolve(os.tmpdir()), 'Cleanup escaped temporary root');
  assert(path.basename(absolute).startsWith(prefix), 'Unexpected temporary cleanup target');
  fs.rmSync(absolute, { recursive: true, force: true });
}
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'league-landing-test-'));
  t.after(() => removeTemporary(root, 'league-landing-test-'));
  git(root, 'init', '-q'); git(root, 'config', 'user.name', 'League fixture'); git(root, 'config', 'user.email', 'fixture@example.invalid');
  write(root, '.gitignore', 'target/\n');
  return root;
}
const HUB = '<!-- unrelated café bytes -->\r\n<script>\nfunction cardFor(g) {\n  const h=1, tag=2, desc=3, update=4, row=5;\n  ' + ANCHOR + '\n}\n</script>\r\n<footer>peer games and accounts stay here</footer>';
function pages(root) {
  write(root, 'index.html', HUB);
  write(root, 'games.json', JSON.stringify({ games: [{ id: 'league', versions: [{ v: 'v3', path: 'games/league/v3/', proto: 2, live: true }] }, { id: 'arena', versions: [] }] }));
  write(root, 'server.json', '{"hosts":[{"name":"peer","ws":"wss://peer.invalid"}]}\n');
  for (const version of ['v1', 'v2', 'v3']) {
    write(root, `games/league/${version}/index.html`, `frozen ${version}`);
    write(root, `games/league/${version}/pkg/league_bg.wasm`, Buffer.from([0, 97, 115, 109, version.charCodeAt(1)]));
  }
  write(root, 'games/arena/v18/index.html', 'unrelated game bytes');
  write(root, 'games/league/media/obsolete.webp', 'old media');
  write(root, 'games/league/index.html', 'old landing');
  git(root, 'add', '.'); git(root, 'commit', '-qm', 'Pages fixture');
  return git(root, 'rev-parse', 'HEAD');
}
function source(root) {
  for (const [file, bytes] of Object.entries({
    'index.html': '<a href="./v3/">Play</a>', 'landing.css': 'body{color:jade}',
    'landing.js': 'console.log("landing");', 'story.json': '{"chapters":[]}',
    'media/trailer.webm': 'fixture video bytes', 'media/poster.webp': 'fixture poster bytes',
  })) write(root, `web/games/league/${file}`, bytes);
  git(root, 'add', '.'); git(root, 'commit', '-qm', 'Landing source fixture');
}
function staged(root) { git(root, 'add', '--all'); return git(root, 'write-tree'); }
function patchCandidate(root) {
  write(root, 'index.html', patchHub(HUB));
  write(root, 'games/league/index.html', 'new landing');
  write(root, 'games/league/media/trailer.webm', 'new trailer');
}

test('hub insertion preserves every unrelated byte and is idempotent', () => {
  const after = patchHub(HUB);
  assert.equal(after.replace(HUB_LINK, ''), HUB);
  assert.equal(patchHub(after), after);
  assert.equal(after.split(MARKER).length - 1, 1);
  assert(after.includes("story.href = 'games/league/'"));
  assert(after.includes("story.textContent = 'Story & trailer'"));
});

test('hub link is conditional on League and creates only a plain anchor', () => {
  for (const id of ['league', 'arena', 'fire', 'future-game']) {
    const appended = [], created = [];
    const context = { g: { id }, h: 1, tag: 2, desc: 3, update: 4, row: 5,
      card: { append: (...values) => appended.push(...values) },
      document: { createElement: tag => { const node = { tag, style: {} }; created.push(node); return node; } },
    };
    vm.runInNewContext(patchHub(ANCHOR), context);
    assert.deepEqual(appended.slice(0, 5), [1, 2, 3, 4, 5]);
    assert.equal(created.length, id === 'league' ? 1 : 0);
    if (id === 'league') assert.deepEqual([created[0].tag, created[0].href, created[0].textContent], ['a', 'games/league/', 'Story & trailer']);
  }
});

test('uncertain, duplicate or manually altered hub anchors are refused', () => {
  for (const html of ['', 'card.append(h, tag, desc, row);', HUB + ANCHOR, HUB + MARKER, patchHub(HUB).replace('Story & trailer', 'changed'), patchHub(HUB) + MARKER]) assert.throws(() => patchHub(html));
});

test('CLI is explicit and landing paths cannot reach game versions or peer files', () => {
  const commit = 'a'.repeat(40);
  assert.deepEqual(parseArgs([`--source-commit=${commit}`]), { commit, push: false, gameVersion: 'v3' });
  assert.deepEqual(parseArgs([`--source-commit=${commit}`]), { commit, push: false, gameVersion: 'v3' });
  assert.deepEqual(parseArgs([`--source-commit=${commit}`, '--game-version=v4']), { commit, push: false, gameVersion: 'v4' });
  assert.throws(() => parseArgs([`--source-commit=${commit}`, '--game-version=v2']));
  for (const args of [[], ['--push'], [`--source-commit=${commit}`, '--push'], [`--source-commit=${commit}`, '--unknown'], ['--source-commit=short']]) assert.throws(() => parseArgs(args));
  for (const file of ['games/league/index.html', 'games/league/landing.css', 'games/league/landing.js', 'games/league/story.json', 'games/league/media/nested/trailer.webm']) assert(landingPath(file), file);
  for (const file of ['index.html', 'games.json', 'server.json', 'games/league/v3/index.html', 'games/league/v2/pkg/league.js', 'games/league/media/../v3/x', 'games/league/media/', 'games/league/media2/x', 'games/arena/index.html']) assert(!landingPath(file), file);
});

test('source manifest is exact, committed and refuses ignored media', t => {
  const root = fixture(t); source(root);
  const files = landingFiles(root); assert.equal(files.size, 6);
  write(root, 'web/games/league/media/untracked.webp', 'not committed');
  assert.throws(() => landingFiles(root), /untracked, ignored or missing/);
  fs.unlinkSync(path.join(root, 'web/games/league/media/untracked.webp'));
  write(root, 'web/games/league/landing.css', 'dirty bytes');
  assert.throws(() => landingFiles(root), /differ from committed/);
});

test('source media links are refused by Git mode on every platform', t => {
  const root = fixture(t); source(root);
  const file = 'web/games/league/media/poster.webp', oid = git(root, 'rev-parse', `HEAD:${file}`);
  git(root, 'update-index', '--cacheinfo', `120000,${oid},${file}`); git(root, 'commit', '-qm', 'Link fixture');
  assert.throws(() => landingFiles(root), /link\/submodule refused/);
});

test('scope proof freezes all three versions, catalog, hostbook and unrelated hub bytes', t => {
  const root = fixture(t), base = pages(root); patchCandidate(root);
  const candidate = staged(root), scope = proveLandingScope(root, base, candidate);
  assert.equal(scope.frozen.length, 5); assert(scope.unchangedOutsideRelease);
  assert(scope.changes.some(change => change.file === 'index.html'));
  for (const [file, expected] of [
    ['games/league/v1/index.html', /Frozen League version changed/],
    ['games/league/v2/pkg/league_bg.wasm', /Frozen League version changed/],
    ['games/league/v3/index.html', /Frozen League version changed/],
    ['games.json', /Frozen Pages file changed/], ['server.json', /Frozen Pages file changed/],
    ['games/arena/v18/index.html', /Unexpected Pages change/],
    ['games/league/unexpected.txt', /Unexpected Pages change/],
    ['index.html', /exact League discovery patch/],
  ]) {
    git(root, 'reset', '--hard', base); patchCandidate(root);
    write(root, file, file === 'index.html' ? patchHub(HUB).replace('peer games', 'edited peer games') : 'unexpected change');
    assert.throws(() => proveLandingScope(root, base, staged(root)), expected, file);
  }
});

test('missing frozen version and staged landing symlink cannot pass scope', t => {
  const root = fixture(t), base = pages(root); patchCandidate(root);
  assert(path.resolve(root, 'games/league/v2').startsWith(path.resolve(root) + path.sep));
  git(root, 'rm', '-rf', '--', 'games/league/v2');
  assert.throws(() => proveLandingScope(root, base, staged(root)), /missing|Frozen/);
  git(root, 'reset', '--hard', base); patchCandidate(root); staged(root);
  const file = 'games/league/media/trailer.webm', oid = git(root, 'rev-parse', `:${file}`);
  git(root, 'update-index', '--cacheinfo', `120000,${oid},${file}`);
  assert.throws(() => proveLandingScope(root, base, git(root, 'write-tree')), /link\/submodule refused/);
});

test('V4 landing scope also freezes the newly published game version', t => {
  const root = fixture(t); pages(root);
  write(root, 'games/league/v4/index.html', 'published V4');
  git(root, 'add', '.'); git(root, 'commit', '-qm', 'V4 fixture');
  const base = git(root, 'rev-parse', 'HEAD'); patchCandidate(root);
  assert(proveLandingScope(root, base, staged(root)).frozen.some(row => row.path === 'games/league/v4'));
  write(root, 'games/league/v4/index.html', 'unexpected V4 edit');
  assert.throws(() => proveLandingScope(root, base, staged(root)), /Frozen League version changed/);
});

test('V4 public landing proof refuses stale V3 Play destinations', () => {
  const { verifyLinks } = require('./prove-landing.cjs');
  const html = '<a href="./v4/">Play now</a><video id="trailer-video"><source src="./media/trailer.mp4"></video>';
  const catalog = { games: [{ id: 'league', versions: [{ v: 'v4', path: 'games/league/v4/', live: true, proto: 2 }] }] };
  assert.equal(verifyLinks(html, patchHub(HUB), catalog, 'v4').play.length, 1);
  assert.throws(() => verifyLinks(html.replace('./v4/', './v3/'), patchHub(HUB), catalog, 'v4'), /must open v4/);
  assert.throws(() => verifyLinks(html, patchHub(HUB), catalog), /must open v3/);
});

test('prepare uses a sparse manifest without publishing or changing frozen Pages', t => {
  const root = fixture(t), base = pages(root);
  git(root, 'branch', 'gh-pages'); git(root, 'branch', '-M', 'main'); source(root);
  const commit = git(root, 'rev-parse', 'HEAD');
  const remote = path.join(root, 'target/remote.git'); fs.mkdirSync(path.dirname(remote), { recursive: true });
  git(root, 'init', '--bare', '-q', remote); git(root, 'remote', 'add', 'origin', remote); git(root, 'push', '-q', 'origin', 'main', 'gh-pages');
  const report = main({ root, args: [`--source-commit=${commit}`] });
  t.after(() => removeTemporary(path.dirname(report.worktree), 'ember-league-landing-'));
  assert.equal(report.preparedOnly, true); assert.equal(report.pushed, false); assert.equal(report.base, base);
  assert.equal(report.frozen.length, 5); assert.equal(report.files.length, 7);
  assert.equal(git(root, 'ls-remote', 'origin', 'refs/heads/gh-pages').split(/\s+/)[0], base);
  assert(!fs.existsSync(path.join(report.worktree, 'games/league/v3')), 'Frozen V3 must not be checked out');
  assert(report.changes.some(change => change.status === 'D' && change.file.endsWith('obsolete.webp')));
  const hub = fs.readFileSync(path.join(report.worktree, 'index.html'), 'utf8'); assert.equal(hub, patchHub(HUB));
  for (const file of report.files) assert.equal(hash(fs.readFileSync(path.join(report.worktree, file.file))), file.sha256);
  write(root, 'web/games/league/landing.css', 'new local revision'); git(root, 'add', '.'); git(root, 'commit', '-qm', 'Not published to main');
  assert.throws(() => main({ root, args: [`--source-commit=${git(root, 'rev-parse', 'HEAD')}`] }), /current main only/);
});
