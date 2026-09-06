// Read-only public release proof; run after the scoped v28 UI publisher.
// Compares deployed bytes to local built artifacts, including frozen v27.
'use strict';
const fs = require('node:fs');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const preservedFire = '00e097b1';
const base = 'https://endersgamesdev.github.io/EmberEngine/';
const started = Date.now();
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
async function get(path) {
  const r = await fetch(base + path + (path.includes('?') ? '&' : '?') + 'qa=' + started);
  assert(r.ok, `${path}: HTTP ${r.status}`);
  return Buffer.from(await r.arrayBuffer());
}
async function main() {
  const book = JSON.parse(await get('server.json'));
  const catalog = JSON.parse(await get('games.json'));
  const version = JSON.parse(await get('version.json'));
  assert.equal(version.commit, JSON.parse(fs.readFileSync('web/version.json')).commit);
  assert.equal(book.proto, 21);
  const live = catalog.games.find(g => g.id === 'arena').versions.filter(v => v.live);
  assert.equal(live.length, 1);
  assert.equal(live[0].path, 'games/arena/v28/');
  assert.equal(live[0].proto, 21);
  const html = String(await get('games/arena/v28/index.html'));
  const settingsHash = sha(fs.readFileSync('web/games/arena/v28/settings.js'));
  assert.equal(html, fs.readFileSync('web/games/arena/v28/index.html', 'utf8')
    .replace('./settings.js?v=1', `./settings.js?v=${settingsHash.slice(0, 12)}`), 'v28 HTML and settings cache token');
  const pairs = [['web/games/arena/v28/settings.js', 'games/arena/v28/settings.js']];
  for (const [name, directory] of [['arena', 'games/arena/v28'],
    ['kings', 'games/kings/v1'], ['what_is_this', 'games/what-is-this/v1'],
    ['ember_lab_julibrot', 'labs/julibrot']]) {
    const local = name === 'ember_lab_julibrot' ? 'web/labs/julibrot/pkg' : 'web/pkg';
    for (const suffix of ['.js', '_bg.wasm']) pairs.push([`${local}/${name}${suffix}`, `${directory}/pkg/${name}${suffix}`]);
  }
  const files = await Promise.all(pairs.map(async ([local, remote]) => {
    const bytes = await get(remote), expected = sha(fs.readFileSync(local));
    assert.equal(sha(bytes), expected, remote);
    return { remote, bytes: bytes.length, sha256: expected };
  }));
  const oldCatalog = JSON.parse(execFileSync('git', ['show', `${preservedFire}:games.json`]));
  assert.deepEqual(catalog.games.find(g => g.id === 'fire'), oldCatalog.games.find(g => g.id === 'fire'));
  for (const file of ['index.html', 'garage.js', 'race.js', 'style.css', 'release.json', 'pkg/fire.js', 'pkg/fire_bg.wasm']) {
    const remote = 'games/fire/v2/' + file;
    const expected = sha(execFileSync('git', ['show', `${preservedFire}:${remote}`], { maxBuffer: 12 * 1024 * 1024 }));
    const bytes = await get(remote);
    assert.equal(sha(bytes), expected, `preserved peer release: ${remote}`);
    files.push({ remote, bytes: bytes.length, sha256: expected, preserved: true });
  }
  assert.equal(sha(await get('games/arena/v27/pkg/arena_bg.wasm')),
    'ef592c9298a96a3ccda8b8f32090ab1b5b925b85df94d649c629ea951a22e71c');
  const welcome = await new Promise((resolve, reject) => {
    const ws = new WebSocket(book.ws);
    const timeout = setTimeout(() => { ws.close(); reject(new Error('Public Welcome timed out')); }, 12000);
    ws.onopen = () => ws.send(JSON.stringify({ t: 'hello', proto: 21, handle: 'release-readonly-check' }));
    ws.onmessage = event => {
      const message = JSON.parse(event.data);
      if (message.t === 'welcome') { clearTimeout(timeout); resolve(message); ws.close(); }
    };
    ws.onerror = () => { clearTimeout(timeout); reject(new Error('Public socket failed')); };
  });
  assert.equal(welcome.proto, 21);
  assert.equal(welcome.commit, version.commit);
  const result = { passed: true, version, welcome, files, frozenV27Unchanged: true, elapsedSeconds: (Date.now() - started) / 1000 };
  fs.mkdirSync('target/controls-public', { recursive: true });
  fs.writeFileSync('target/controls-public/results.json', JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
