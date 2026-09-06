// Read-only HTTP/Welcome proof after the scoped v29 publication.
// No room creation, input, server control or public writes.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const os = require('node:os');
const { execFileSync } = require('node:child_process');
const preserved = process.env.EMBER_QA_PRESERVED_PAGES || '3dc66e0a';
const base = 'https://endersgamesdev.github.io/EmberEngine/';
const output = path.resolve(process.env.EMBER_QA_OUTPUT || 'target/parkour-public');
const started = Date.now(), files = [];
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = (...args) => execFileSync('git', args, { windowsHide: true, maxBuffer: 64 * 1024 * 1024 });
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
  const [book, catalog, version] = await Promise.all(['server.json', 'games.json', 'version.json']
    .map(async file => JSON.parse(await get(file))));
  assert.deepEqual(version, JSON.parse(fs.readFileSync('web/version.json')), 'Build stamp differs');
  assert.equal(book.proto, 22);
  const live = catalog.games.find(game => game.id === 'arena').versions.filter(version => version.live);
  assert.equal(live.length, 1); assert.equal(live[0].path, 'games/arena/v29/'); assert.equal(live[0].proto, 22);
  const html = String(await get('games/arena/v29/index.html'));
  const settings = fs.readFileSync('web/games/arena/v29/settings.js');
  assert.equal(html, fs.readFileSync('web/games/arena/v29/index.html', 'utf8')
    .replace('./settings.js?v=1', `./settings.js?v=${sha(settings).slice(0, 12)}`), 'v29 page/settings cache token');
  assert(/wall-slide|wall slide/i.test(html) && /wall kick|kick away/i.test(html), 'Parkour controls help missing');
  await checkFile('games/arena/v29/settings.js', settings);
  for (const file of ['arena.js', 'arena_bg.wasm']) await checkFile(`games/arena/v29/pkg/${file}`, fs.readFileSync(`web/pkg/${file}`));

  // Compare peers against the actual prior Pages tree, NEVER this source
  // branch's older Fire implementation or freshly built unrelated bundles.
  const oldCatalog = JSON.parse(git('show', `${preserved}:games.json`));
  assert.deepEqual(catalog.games.filter(game => game.id !== 'arena'), oldCatalog.games.filter(game => game.id !== 'arena'));
  const roots = new Set(['games/arena/v28/']);
  for (const game of oldCatalog.games.filter(game => game.id !== 'arena')) {
    for (const version of game.versions.filter(version => version.live)) roots.add(version.path);
  }
  const inheritedFiles = String(git('ls-tree', '-r', '--name-only', preserved, '--', ...roots)).trim().split('\n').filter(Boolean);
  for (const remote of inheritedFiles) await checkFile(remote, git('show', `${preserved}:${remote}`), true);
  const oldIndex = String(git('show', `${preserved}:index.html`));
  assert.equal(String(await get('index.html')), oldIndex.replace("let arenaLivePath = 'games/arena/v28/';", "let arenaLivePath = 'games/arena/v29/';"),
    'Root launcher must only change its Arena fallback');
  const welcome = await new Promise((resolve, reject) => {
    const socket = new WebSocket(book.ws);
    const timeout = setTimeout(() => { socket.close(); reject(new Error('Public Welcome timed out')); }, 12000);
    socket.onopen = () => socket.send(JSON.stringify({ t: 'hello', proto: 0, handle: 'v29-release-readonly' }));
    socket.onmessage = event => {
      const message = JSON.parse(event.data);
      if (message.t === 'welcome') { clearTimeout(timeout); resolve(message); socket.close(); }
    };
    socket.onerror = () => { clearTimeout(timeout); reject(new Error('Public socket failed')); };
  });
  assert.equal(welcome.proto, 22); assert.equal(welcome.commit, version.commit);
  const result = { passed: true, preservedPages: preserved, version, welcome, files,
    limits: 'HTTP bytes and read-only Welcome only. Gameplay and eight-player parkour require separate network/browser gates.',
    elapsedSeconds: (Date.now() - started) / 1000 };
  fs.mkdirSync(output, { recursive: true }); fs.writeFileSync(path.join(output, 'results.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
