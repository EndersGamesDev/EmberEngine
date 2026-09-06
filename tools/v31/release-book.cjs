// Release-only discovery reuses the actual browser's bound-mirror merge rule.
// Never copy a mirror entry into hosts[]: that would mask future mirror updates.
'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { execFileSync } = require('node:child_process');
const namePattern = /^[a-z0-9-]{3,32}$/;
const maxBytes = 512 * 1024;

async function fetchEntry(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(12000), cache: 'no-store' });
  assert(response.ok, `Mirror HTTP ${response.status}`);
  assert(Number(response.headers.get('content-length') || 0) <= maxBytes, 'Mirror too large');
  const reader = response.body.getReader();
  const chunks = []; let size = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.length;
      assert(size <= maxBytes, 'Mirror exceeded byte limit');
      chunks.push(value);
    }
  } finally { await reader.cancel().catch(() => {}); }
  return JSON.parse(Buffer.concat(chunks).toString('utf8'));
}
const resolveCommit = value => String(execFileSync('git', ['rev-parse', '--verify', `${value}^{commit}`],
  { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] })).trim();

async function readyHost(book, expected, options = {}) {
  const { mergeBook, probeHost } = await import(pathToFileURL(path.resolve(__dirname, '../../web/hosts.js')));
  const read = options.fetchEntry || fetchEntry;
  const probe = options.probe || (url => probeHost(url, { proto: 0, timeoutMs: 12000, handle: 'v31-release-readonly' }));
  const resolve = options.resolveCommit || resolveCommit;
  const refs = (Array.isArray(book.mirrors) ? book.mirrors : []).slice(0, 32)
    .filter(ref => ref && namePattern.test(ref.name) && typeof ref.url === 'string' && /^https:\/\//.test(ref.url));
  const mirrors = await Promise.all(refs.map(async ref => {
    try {
      const url = new URL(ref.url); url.searchParams.set('ts', String(Date.now()));
      const entry = await read(url.href);
      // A bound mirror can serve one named row, never a whole book.
      if (!entry || Array.isArray(entry) || Array.isArray(entry.hosts) || entry.name !== ref.name) return null;
      return { name: ref.name, entry };
    } catch { return null; }
  }));
  const candidates = mergeBook(book, mirrors.filter(Boolean))
    .filter(host => namePattern.test(host.name) && typeof host.ws === 'string' && /^wss:\/\//.test(host.ws));
  const results = await Promise.all(candidates.map(async host => {
    try {
      // A live Welcome outranks potentially stale published protocol metadata.
      const result = await probe(host.ws);
      const welcome = result.welcome;
      assert(result.ok && welcome?.proto === 24, 'Host does not serve protocol24');
      assert.equal(welcome.host, host.name, 'Welcome is not the bound host');
      assert.equal(welcome.version, expected.version, 'Host build version differs');
      assert(/^[a-f0-9]{7,40}$/.test(welcome.commit || ''), 'Invalid server commit');
      assert.equal(resolve(welcome.commit), expected.fullCommit, 'Host source commit differs or is ambiguous');
      return { host, welcome };
    } catch { return null; }
  }));
  const selected = results.find(Boolean);
  assert(selected, 'No reachable protocol24 host built from the exact release revision; publish/verify the approved server first');
  return selected;
}

function arenaBook(book, selected, cacheVersion = String(Math.floor(Date.now() / 1000))) {
  const next = { ...book, ws: selected.host.ws, proto: 24, v: cacheVersion };
  // In particular hosts[] and mirrors[] remain untouched. The host's own
  // publisher retains ownership, including its independently running games.
  assert.deepEqual(Object.fromEntries(Object.entries(next).filter(([key]) => !['ws', 'proto', 'v'].includes(key))),
    Object.fromEntries(Object.entries(book).filter(([key]) => !['ws', 'proto', 'v'].includes(key))));
  return next;
}
module.exports = { readyHost, arenaBook };
