// Prepare a League landing release after V3. Frozen games, catalog and hostbook
// remain byte-identical; the only hub edit is one League-only discovery link.
// Usage: node tools/league/publish-landing.cjs --source-commit=<full HEAD> [--push]
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { safeRelative, safePath, collectFiles, treeFiles, hash, gameVersion } = require('./publish.cjs');

const ENTRY = 'games/league';
const FIXED = ['index.html', 'landing.css', 'landing.js', 'story.json'];
const ANCHOR = 'card.append(h, tag, desc, update, row);';
const MARKER = '/* league-story-link:v1 */';
const HUB_LINK = `
      ${MARKER}
      if (g.id === 'league') {
        const story = document.createElement('a');
        story.href = 'games/league/';
        story.textContent = 'Story & trailer';
        story.style.cssText = 'color:#72d5b5;font-size:.82rem;margin-top:10px;display:inline-block;text-underline-offset:3px';
        card.append(story);
      }`;
const git = (cwd, ...args) => execFileSync('git', ['-c', 'core.autocrlf=false', '-C', cwd, ...args], {
  windowsHide: true, maxBuffer: 64 * 1024 * 1024,
});
const text = (cwd, ...args) => git(cwd, ...args).toString().trim();

function patchHub(html) {
  assert.equal(typeof html, 'string', 'Hub must be UTF-8 text');
  assert.equal(html.split(ANCHOR).length - 1, 1, 'Expected exactly one known hub card anchor');
  if (html.includes(MARKER)) {
    assert.equal(html.split(MARKER).length - 1, 1, 'Duplicate League hub marker');
    assert(html.includes(ANCHOR + HUB_LINK), 'Existing League hub patch differs; review it before publishing');
    return html;
  }
  return html.replace(ANCHOR, ANCHOR + HUB_LINK);
}

function landingPath(file) {
  try { safeRelative(file); } catch { return false; }
  return FIXED.some(name => file === `${ENTRY}/${name}`) || file.startsWith(`${ENTRY}/media/`);
}
function utf8(bytes) {
  const value = bytes.toString('utf8');
  assert(bytes.equals(Buffer.from(value, 'utf8')), 'Refusing to rewrite non-UTF-8 hub bytes');
  return value;
}
function objectAt(cwd, revision, file) {
  safeRelative(file);
  const rows = git(cwd, 'ls-tree', '-z', revision, '--', file).toString().split('\0').filter(Boolean);
  assert.equal(rows.length, 1, `Required published path is missing: ${file}`);
  const tab = rows[0].indexOf('\t');
  const [mode, type, oid] = rows[0].slice(0, tab).split(' ');
  assert.equal(rows[0].slice(tab + 1), file, `Unexpected published path: ${file}`);
  return { mode, type, oid };
}

function proveLandingScope(cwd, base, candidate) {
  treeFiles(cwd, candidate, ENTRY); // Catch Git-mode links even on Windows.
  const frozen = [];
  const versions = git(cwd, 'ls-tree', '-z', `${base}:${ENTRY}`).toString().split('\0').filter(Boolean)
    .map(row => row.slice(row.indexOf('\t') + 1)).filter(name => /^v[1-9][0-9]{0,5}$/.test(name));
  for (const required of ['v1', 'v2', 'v3']) assert(versions.includes(required), `Required frozen version missing: ${required}`);
  for (const version of versions) {
    const file = `${ENTRY}/${version}`, before = objectAt(cwd, base, file);
    assert.equal(before.type, 'tree', `Frozen ${version} must be a tree`);
    assert.deepEqual(objectAt(cwd, candidate, file), before, `Frozen League version changed: ${file}`);
    frozen.push({ path: file, ...before });
  }
  for (const file of ['games.json', 'server.json']) {
    const before = objectAt(cwd, base, file);
    assert.equal(before.type, 'blob', `Required Pages file is not a blob: ${file}`);
    assert(['100644', '100755'].includes(before.mode), `Published link refused: ${file}`);
    assert.deepEqual(objectAt(cwd, candidate, file), before, `Frozen Pages file changed: ${file}`);
    frozen.push({ path: file, ...before });
  }
  const parts = git(cwd, 'diff', '--no-renames', '--name-status', '-z', base, candidate).toString().split('\0').filter(Boolean);
  assert.equal(parts.length % 2, 0, 'Malformed Pages diff');
  const changes = [];
  for (let i = 0; i < parts.length; i += 2) {
    const [status, file] = parts.slice(i, i + 2); safeRelative(file);
    assert(['A', 'M', 'D'].includes(status) && (landingPath(file) || (file === 'index.html' && status === 'M')), `Unexpected Pages change: ${status} ${file}`);
    changes.push({ status, file });
  }
  const baseHub = objectAt(cwd, base, 'index.html'), nextHub = objectAt(cwd, candidate, 'index.html');
  assert(baseHub.type === 'blob' && ['100644', '100755'].includes(baseHub.mode), 'Published hub link refused');
  assert.equal(nextHub.mode, baseHub.mode, 'Hub file mode changed');
  const before = git(cwd, 'show', `${base}:index.html`);
  assert.equal(hash(git(cwd, 'show', `${candidate}:index.html`)), hash(Buffer.from(patchHub(utf8(before)))), 'Hub change exceeds the exact League discovery patch');
  return { changes, frozen, unchangedOutsideRelease: true };
}

function landingFiles(root) {
  const directory = safePath(root, `web/${ENTRY}`);
  const committed = treeFiles(root, 'HEAD', `web/${ENTRY}`).filter(row => landingPath(row.file.slice(4)));
  const files = new Map();
  for (const name of FIXED) files.set(`${ENTRY}/${name}`, fs.readFileSync(safePath(directory, name)));
  const media = collectFiles(safePath(directory, 'media'));
  assert(media.size > 0, 'Landing media is missing; finish and commit the trailer artifacts first');
  for (const [name, bytes] of media) files.set(`${ENTRY}/media/${name}`, bytes);
  assert.deepEqual([...files.keys()].sort(), committed.map(row => row.file.slice(4)).sort(), 'Landing release contains untracked, ignored or missing source files');
  for (const row of committed) assert.equal(hash(files.get(row.file.slice(4))), hash(git(root, 'show', `HEAD:${row.file}`)), `Landing bytes differ from committed source: ${row.file}`);
  const story = JSON.parse(utf8(files.get(`${ENTRY}/story.json`)));
  assert(story && typeof story === 'object' && !Array.isArray(story), 'story.json must be an object');
  return files;
}

function parseArgs(args) {
  const names = args.map(value => value.split('=')[0]);
  assert.equal(new Set(names).size, names.length, 'Duplicate argument');
  assert(args.every(value => value === '--push' || /^--source-commit=[0-9a-f]{40}$/.test(value) || /^--game-version=v[1-9][0-9]{0,5}$/.test(value)), 'Use --source-commit=<full HEAD>, optional --game-version=vN and --push');
  const commit = args.find(value => value.startsWith('--source-commit='))?.slice('--source-commit='.length);
  assert(commit, '--source-commit is required');
  const selected = gameVersion(args.find(value => value.startsWith('--game-version='))?.slice('--game-version='.length) || 'v3');
  assert(Number(selected.slice(1)) >= 3, 'The story landing requires V3 or later');
  return { commit, push: args.includes('--push'), gameVersion: selected };
}

function main({ root = process.cwd(), args = process.argv.slice(2) } = {}) {
  const started = Date.now(), options = parseArgs(args);
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  root = path.resolve(root);
  assert.equal(text(root, 'status', '--porcelain'), '', 'Source must be clean');
  const commit = text(root, 'rev-parse', 'HEAD');
  assert.equal(options.commit, commit, 'Source revision differs from HEAD');
  const files = landingFiles(root);
  git(root, 'fetch', 'origin', 'main', 'gh-pages');
  assert.equal(text(root, 'rev-parse', 'origin/main'), commit, 'Publish current main only');
  const base = text(root, 'rev-parse', 'origin/gh-pages');
  const catalog = JSON.parse(git(root, 'show', `${base}:games.json`));
  const league = catalog.games?.find(game => game.id === 'league');
  assert(league?.versions?.some(version => version.v === options.gameVersion && version.path === `${ENTRY}/${options.gameVersion}/` && version.live && version.proto === 2), `Publish ${options.gameVersion} with its protocol 2 catalog entry before the landing page`);
  // Refuse links in mutable paths before checkout or deletion. Frozen versions
  // are compared by tree id, never copied out or touched by this release.
  const existing = treeFiles(root, base, ENTRY).filter(row => landingPath(row.file));
  const hubObject = objectAt(root, base, 'index.html');
  assert(hubObject.type === 'blob' && ['100644', '100755'].includes(hubObject.mode), 'Published hub link refused');
  const baseHub = git(root, 'show', `${base}:index.html`);
  files.set('index.html', Buffer.from(patchHub(utf8(baseHub))));
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-league-landing-'));
  const worktree = path.join(temporary, 'pages');
  git(root, 'worktree', 'add', '--detach', '--no-checkout', worktree, base);
  git(worktree, 'sparse-checkout', 'set', '--no-cone', '/index.html', ...FIXED.map(name => `/${ENTRY}/${name}`), `/${ENTRY}/media/`);
  git(worktree, 'read-tree', '-mu', 'HEAD');
  assert.equal(text(worktree, 'write-tree'), text(root, 'rev-parse', `${base}^{tree}`), 'Sparse checkout changed the base index');
  for (const previous of existing) if (!files.has(previous.file)) fs.unlinkSync(safePath(worktree, previous.file));
  for (const [name, bytes] of files) {
    const file = safePath(worktree, name); fs.mkdirSync(path.dirname(file), { recursive: true }); fs.writeFileSync(file, bytes);
  }
  git(worktree, 'add', '--all', '--', 'index.html', ...FIXED.map(name => `${ENTRY}/${name}`), `${ENTRY}/media`);
  const candidate = text(worktree, 'write-tree'), scope = proveLandingScope(worktree, base, candidate);
  const actual = treeFiles(worktree, candidate, ENTRY).filter(row => landingPath(row.file)).map(row => row.file).sort();
  assert.deepEqual(actual, [...files.keys()].filter(file => file !== 'index.html').sort(), 'Staged landing tree differs from exact local manifest');
  for (const [name, bytes] of files) assert.equal(hash(git(worktree, 'show', `${candidate}:${name}`)), hash(bytes), `Staged bytes differ: ${name}`);
  assert.equal(text(root, 'status', '--porcelain'), '', 'Source changed while preparing');
  assert.equal(text(root, 'rev-parse', 'HEAD'), commit, 'Source HEAD changed while preparing');
  const currentFiles = landingFiles(root);
  for (const [name, bytes] of currentFiles) assert.equal(hash(bytes), hash(files.get(name)), `Source changed while preparing: ${name}`);
  assert.equal(text(root, 'ls-remote', 'origin', 'refs/heads/main').split(/\s+/)[0], commit, 'Concurrent main update: prepare again');
  assert.equal(text(root, 'ls-remote', 'origin', 'refs/heads/gh-pages').split(/\s+/)[0], base, 'Concurrent Pages update: prepare again');
  const report = {
    sourceCommit: commit, gameVersion: options.gameVersion, base, candidate, worktree, ...scope,
    files: [...files].map(([file, bytes]) => ({ file, bytes: bytes.length, sha256: hash(bytes) })),
    preparedOnly: !options.push, pushed: false, noChanges: scope.changes.length === 0,
  };
  if (options.push && scope.changes.length) {
    git(worktree, 'commit', '-m', `Publish League story and trailer ${commit.slice(0, 8)}; freeze game versions and peer pages`);
    git(worktree, 'push', 'origin', 'HEAD:refs/heads/gh-pages');
    report.pagesCommit = text(worktree, 'rev-parse', 'HEAD'); report.pushed = true;
  }
  report.elapsedSeconds = (Date.now() - started) / 1000;
  const reportDirectory = safePath(root, 'target/league-landing-publish');
  fs.mkdirSync(reportDirectory, { recursive: true });
  fs.writeFileSync(safePath(root, 'target/league-landing-publish/results.json'), JSON.stringify(report, null, 2) + '\n');
  return report;
}
module.exports = { patchHub, landingPath, landingFiles, proveLandingScope, parseArgs, main, ANCHOR, MARKER, HUB_LINK };
if (require.main === module) {
  try { console.log(JSON.stringify(main(), null, 2)); }
  catch (error) { console.error(error); process.exitCode = 1; }
}
