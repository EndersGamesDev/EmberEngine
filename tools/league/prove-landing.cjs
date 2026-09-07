// Read-only CDN proof after publish-landing.cjs --push completes.
// Usage: node tools/league/prove-landing.cjs --source-commit=<full clean HEAD>
// No browser, game socket, build, Git fetch or publication is performed.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { safeRelative, safePath, hash, gameVersion } = require('./publish.cjs');
const { landingFiles, patchHub } = require('./publish-landing.cjs');

const BASE = 'https://endersgamesdev.github.io/EmberEngine/';
const ENTRY = 'games/league/';
const OUTPUT = 'target/league-landing-publish/public-proof.json';
const git = (root, ...args) => execFileSync('git', ['-c', 'core.autocrlf=false', '-C', root, ...args], {
  windowsHide: true, maxBuffer: 64 * 1024 * 1024,
});
const text = (root, ...args) => git(root, ...args).toString().trim();

function parseArgs(args) {
  assert(args.length === 1 && /^--source-commit=[0-9a-f]{40}$/.test(args[0]),
    'Use exactly --source-commit=<full clean HEAD>');
  return args[0].slice('--source-commit='.length);
}
function cleanSource(root, commit) {
  assert.equal(text(root, 'rev-parse', 'HEAD'), commit, 'Selected source commit differs from HEAD');
  assert.equal(text(root, 'status', '--porcelain=v1', '--untracked-files=all'), '', 'Source must be clean, including untracked files');
}
function officialUrl(value, base = BASE) {
  const url = new URL(value, base), official = new URL(BASE);
  assert(url.origin === official.origin && url.pathname.startsWith(official.pathname)
    && !url.username && !url.password, `URL escapes the official EmberEngine site: ${url.href}`);
  return url;
}
function verifyLinks(html, hub, catalog, selectedVersion = 'v3') {
  selectedVersion = gameVersion(selectedVersion);
  assert.equal(patchHub(hub), hub, 'Public hub is missing the exact League Story & trailer link');
  const landingUrl = officialUrl(ENTRY), playUrl = officialUrl(`${ENTRY}${selectedVersion}/`);
  const anchors = [...html.matchAll(/<a\b[^>]*\bhref\s*=\s*(["'])(.*?)\1[^>]*>([\s\S]*?)<\/a>/gi)];
  assert(anchors.length > 0, 'Landing has no links');
  const play = [];
  for (const [, , href, body] of anchors) {
    const url = officialUrl(href, landingUrl);
    if (/\bplay\b/i.test(body.replace(/<[^>]*>/g, ' '))) {
      assert.equal(url.href, playUrl.href, `Every landing Play link must open ${selectedVersion}`);
      play.push({ href, resolved: url.href });
    }
  }
  assert(play.length > 0, 'Landing has no Play link');
  const leagues = catalog.games?.filter(game => game.id === 'league');
  assert.equal(leagues?.length, 1, 'Public hub catalog needs exactly one League entry');
  const selected = leagues[0].versions?.filter(version => version.v === selectedVersion);
  assert.equal(selected?.length, 1, `Public hub catalog needs exactly one ${selectedVersion} entry`);
  assert(selected[0].live && selected[0].proto === 2, `${selectedVersion} must be live with protocol 2`);
  assert.equal(officialUrl(selected[0].path).href, playUrl.href, `Catalog ${selectedVersion} Play destination differs`);
  assert(/<video\b[^>]*\bid=["']trailer-video["'][^>]*>/i.test(html), 'Playable trailer video is missing');
  const sources = [...html.matchAll(/<source\b[^>]*\bsrc\s*=\s*(["'])(.*?)\1/gi)];
  assert(sources.some(([, , src]) => officialUrl(src, landingUrl).href === officialUrl(`${ENTRY}media/trailer.mp4`).href),
    'Trailer source is missing or stale');
  const visible = html.replace(/<!--[\s\S]*?-->|<script\b[\s\S]*?<\/script>/gi, '')
    .replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ');
  assert(!/trailer[^.!?]{0,80}(?:coming soon|being prepared|not yet available|placeholder)|(?:placeholder|coming soon)[^.!?]{0,40}trailer/i.test(visible),
    'Stale trailer placeholder text remains');
  return { hubLandingUrl: landingUrl.href, play, catalogPlayUrl: playUrl.href, trailerSource: `${BASE}${ENTRY}media/trailer.mp4`, noStaleTrailerPlaceholder: true };
}

async function get(file, fetchImpl, deadline, nonce) {
  safeRelative(file);
  const url = officialUrl(file);
  url.searchParams.set('proof', nonce);
  const remaining = deadline - Date.now();
  assert(remaining > 0, 'Public proof exceeded its 180-second deadline');
  const response = await fetchImpl(url, { cache: 'no-store', redirect: 'error', signal: AbortSignal.timeout(Math.min(20000, remaining)) });
  assert(response.ok, `${file}: HTTP ${response.status}`);
  // Redirects are refused, and the returned response must still identify this asset.
  const finalUrl = officialUrl(response.url);
  assert.equal(finalUrl.pathname, url.pathname, `${file}: response path differs`);
  return { bytes: Buffer.from(await response.arrayBuffer()), url: finalUrl.href, status: response.status };
}

async function main({ root = process.cwd(), args = process.argv.slice(2), fetchImpl = fetch } = {}) {
  root = path.resolve(root);
  const started = Date.now(), deadline = started + 180000;
  const report = {
    passed: false, startedAt: new Date(started).toISOString(), baseUrl: BASE,
    sourceCommit: null, files: [], errors: [],
    remoteScope: 'Every file in the committed local landing/media manifest is checked. HTTP cannot enumerate extra remote files; publish-landing.cjs proves the exact Pages tree and frozen peer trees.',
  };
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  try {
    const commit = parseArgs(args); report.sourceCommit = commit;
    cleanSource(root, commit);
    // Also rejects ignored/untracked media, missing tracked files and Git-mode links.
    const local = landingFiles(root);
    const publication = JSON.parse(fs.readFileSync(safePath(root, 'target/league-landing-publish/results.json')));
    const selected = gameVersion(publication.gameVersion || 'v3'); report.gameVersion = selected;
    assert.equal(publication.sourceCommit, commit, 'Landing publication belongs to a different source commit');
    assert(!publication.preparedOnly && (publication.pushed || publication.noChanges), 'Landing publication has not completed');
    assert(/^[0-9a-f]{40}$/.test(publication.candidate), 'Invalid published candidate tree');
    report.pagesCommit = publication.pagesCommit || null;
    report.pagesTree = publication.candidate;
    const published = file => git(root, 'show', `${publication.candidate}:${file}`);
    const expected = new Map(local);
    expected.set('index.html', published('index.html'));
    const manifest = publication.files;
    assert(Array.isArray(manifest), 'Landing publication manifest is missing');
    assert.deepEqual(manifest.map(row => row.file).sort(), [...expected.keys()].sort(), 'Publication manifest differs from committed landing and hub');
    for (const row of manifest) {
      assert.equal(row.sha256, hash(expected.get(row.file)), `Publication manifest hash differs: ${row.file}`);
      assert.equal(hash(published(row.file)), row.sha256, `Published tree differs from manifest: ${row.file}`);
    }
    // These are the frozen published discovery and destination bytes, not a local rebuild.
    expected.set('games.json', published('games.json'));
    expected.set(`${ENTRY}${selected}/index.html`, published(`${ENTRY}${selected}/index.html`));
    assert(expected.size <= 128, 'Landing proof exceeds the bounded 128-file manifest');
    verifyLinks(local.get(`${ENTRY}index.html`).toString('utf8'), expected.get('index.html').toString('utf8'), JSON.parse(expected.get('games.json')), selected);
    const downloads = new Map(), queue = [...expected];
    let cursor = 0;
    // Three reads at a time; no sleeps or retries can hide an incomplete deployment.
    await Promise.all(Array.from({ length: Math.min(3, queue.length) }, async () => {
      while (cursor < queue.length) {
        const [file, bytes] = queue[cursor++];
        const result = { file, bytes: bytes.length, expectedSha256: hash(bytes), passed: false };
        try {
          const download = await get(file, fetchImpl, deadline, `${started}-${cursor}`);
          result.url = download.url; result.status = download.status;
          result.actualBytes = download.bytes.length; result.sha256 = hash(download.bytes);
          assert.equal(result.sha256, result.expectedSha256, `${file}: public bytes differ`);
          result.passed = true; downloads.set(file, download.bytes);
        } catch (error) { result.error = error.message; report.errors.push(`${file}: ${error.message}`); }
        report.files.push(result);
      }
    }));
    assert.equal(report.errors.length, 0, `${report.errors.length} public asset checks failed`);
    report.links = verifyLinks(downloads.get(`${ENTRY}index.html`).toString('utf8'), downloads.get('index.html').toString('utf8'), JSON.parse(downloads.get('games.json')), selected);
    cleanSource(root, commit);
    const after = landingFiles(root);
    assert.deepEqual([...after.keys()].sort(), [...local.keys()].sort(), 'Source manifest changed during proof');
    for (const [file, bytes] of after) assert.equal(hash(bytes), hash(local.get(file)), `Source changed during proof: ${file}`);
    report.sourceCleanAtStartAndEnd = true;
    report.mediaFiles = [...local.keys()].filter(file => file.startsWith(`${ENTRY}media/`)).length;
    report.passed = true;
  } catch (error) {
    report.errors.push(error.message);
  } finally {
    report.files.sort((a, b) => a.file.localeCompare(b.file));
    report.elapsedSeconds = (Date.now() - started) / 1000;
    const output = safePath(root, OUTPUT);
    fs.mkdirSync(path.dirname(output), { recursive: true });
    fs.writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  }
  if (!report.passed) throw new Error(`Landing public proof failed; see ${OUTPUT}: ${report.errors.join('; ')}`);
  return report;
}

module.exports = { BASE, parseArgs, officialUrl, verifyLinks, get, main };
if (require.main === module) main().then(report => console.log(JSON.stringify(report, null, 2)))
  .catch(error => { console.error(error.message); process.exitCode = 1; });
