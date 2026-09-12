// Prove that every live server game in a catalog has a host it can join.
//
//   node deploy/check-hosts.mjs --tree <dir>
//   node deploy/check-hosts.mjs --games <path|url> --book <path|url> [--hosts-js <path>]
//
// Publication used to be able to ship a site whose pages had no host. Every
// page picks its host by EXACT protocol equality (docs/hosts.md §5), so a
// release that moves a page's protocol while the running hosts still speak the
// old one produces a healthy site, a healthy release and a game nobody can
// enter. On 2026-09-11 three of four live games were in exactly that state for
// a day: the code, the release and the book were each individually fine.
//
// This asks the one question none of them asked — "is there a host on this
// page's protocol, and does it answer?" — by doing what a player's browser
// does, in the same order `chooseHost` does it: load the book, fetch its bound
// mirrors, merge, take every host that RUNS the game, probe them all, and only
// then apply the protocol filter, so a live `Welcome` outranks whatever the
// book claims. The order is the point. Filtering on the book's number first
// would discard a host before a socket ever opened, and the book's number is
// exactly the value that goes stale: a frozen entry saying `proto: 23` in
// front of a server welcoming 24 would fail this gate while every player's
// page happily played on that host.
//
// It imports the pure functions of `hosts.js` by path rather than
// reimplementing them, so the gate and the pages cannot drift apart; the copy
// it imports is the one in the tree being checked. Importing a function and
// then calling it with a different argument set is a reimplementation with
// extra steps, so the ranking is driven the way the pages drive it.
//
// Exit codes are the whole interface for the workflow that runs it:
//
//   0  every live server game found a host that answered
//   1  at least one did not
//   2  an input could not be read (no verdict was reached)
//
// No dependencies, and none are wanted: it runs on a GitHub runner's node with
// nothing installed. Real sockets come from `globalThis.WebSocket` (node 22+).

import { readFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';

// ---- the pure core ---------------------------------------------------------

/// An input that could not be read at all, which is exit 2 rather than a
/// failed check: "the book is unreachable" is not "the game has no host".
export class InputError extends Error {}

const isNum = (v) => typeof v === 'number' && Number.isFinite(v);
const str = (v) => (typeof v === 'string' ? v.trim() : '');

/// Every catalog entry a page can be published for, as the checker sees it.
/// `kind: "lab"` has no host, no protocol and no handover (docs/hosts.md §11),
/// so it is not a thing that can fail this check; a live version with no
/// `proto` is a page that speaks to no server at all.
export function plan(games) {
  if (!games || typeof games !== 'object' || !Array.isArray(games.games)) {
    throw new InputError('the catalog has no `games` array');
  }
  const rows = [];
  for (const g of games.games) {
    if (!g || typeof g !== 'object') continue;
    const id = str(g.id);
    if (!id) continue;
    const lab = str(g.kind) === 'lab';
    for (const v of Array.isArray(g.versions) ? g.versions : []) {
      if (!v || typeof v !== 'object' || v.live !== true) continue;
      const slot = str(v.v) || str(v.version) || '?';
      if (lab) rows.push({ game: id, v: slot, proto: null, skip: 'lab, no server needed' });
      else if (!isNum(v.proto)) rows.push({ game: id, v: slot, proto: null, skip: 'no catalog proto, no server needed' });
      else rows.push({ game: id, v: slot, proto: v.proto, skip: null });
    }
  }
  return rows;
}

/// Why one candidate did not carry the check, in the words a reader can act on.
function whyNot(view, result) {
  const who = view.bookName || 'legacy-address';
  if (!result || result.ok !== true) return `${who} ${(result && result.reason) || 'unreachable'}`;
  const live = result.welcome && isNum(result.welcome.proto) ? result.welcome.proto : null;
  if (live !== null) return `${who} answered on proto ${live}`;
  return `${who} answered without a protocol, and its entry claims ${view.proto === null ? 'none' : view.proto}`;
}

/// One game+version, resolved the way a page resolves it.
///
/// Two rankings on purpose, which is what `chooseHost` does. The first has the
/// protocol filter OFF and answers "who runs this game at all" — those are the
/// hosts worth a socket. The second is given the probe results, so the live
/// `Welcome` wins over the book and the protocol filter runs on what is
/// actually answering. Probes go out in parallel and each carries its own
/// timeout, so a dead host costs the run one timeout rather than a place in a
/// queue.
async function checkOne(row, merged, { hosts, probe, timeoutMs }) {
  const running = hosts.rankHosts(merged, { game: row.game, proto: null }).candidates;
  const probes = new Map();
  await Promise.all(running.map(async (c) => {
    probes.set(c.bookName, await probe(c.url, { proto: row.proto, timeoutMs, handle: 'pages-gate' }));
  }));
  const { candidates } = hosts.rankHosts(merged, { game: row.game, proto: row.proto, probes });
  const chosen = candidates[0] || null;
  // `rankHosts` has already dropped everything that did not answer and
  // everything on another protocol. Asserting it again on the chosen host is
  // the one line that makes this a GATE rather than a ranking: exact equality
  // is what the server's join gate applies, so it is what is proved here.
  if (chosen && chosen.proto === row.proto) {
    return {
      ...row,
      ok: true,
      host: chosen.bookName || 'legacy-address',
      rttMs: isNum(chosen.rttMs) ? chosen.rttMs : 0,
      tried: [],
    };
  }
  return {
    ...row,
    ok: false,
    host: null,
    rttMs: null,
    tried: running.map((c) => whyNot(c, probes.get(c.bookName))),
  };
}

/// The whole check, as one pure function over injected inputs.
///
///   games    a parsed `games.json`
///   book     a parsed `server.json`
///   mirrors  async (url) => the entry that URL served, or null for an absence
///   probe    async (url, opts) => `probeHost`'s resolution
///   hosts    the `hosts.js` module namespace (mirrorRefs, mergeBook, rankHosts)
///
/// The book is loaded and its mirrors fetched ONCE for the whole run, exactly
/// as a page does it, and the games are then resolved in parallel so the run
/// is bounded by the slowest game rather than by their sum.
export async function checkHosts({ games, book, mirrors, probe, hosts, timeoutMs = 8000 }) {
  const rows = plan(games);
  if (!book || typeof book !== 'object' || Array.isArray(book)) {
    throw new InputError('the address book is not a JSON object');
  }
  const refs = hosts.mirrorRefs(book);
  const served = await Promise.all(refs.map(async (m) => {
    try { return { name: m.name, entry: await mirrors(m.url) }; } catch { return { name: m.name, entry: null }; }
  }));
  const merged = hosts.mergeBook(book, served);

  const results = await Promise.all(rows.map(async (row) => {
    if (row.skip) return { ...row, ok: true, skipped: true };
    return checkOne(row, merged, { hosts, probe, timeoutMs });
  }));
  return { ok: results.every((r) => r.ok), results, hosts: merged, mirrors: served };
}

/// One result, as the line a reader acts on.
export function formatResult(r) {
  if (r.skipped) return `skip ${r.game} ${r.v}: ${r.skip}`;
  if (r.ok) return `ok ${r.game} ${r.v} proto ${r.proto} host ${r.host} ${r.rttMs} ms`;
  const why = r.tried.length ? r.tried.join('; ') : 'no host in the book runs this game';
  return `FAILED ${r.game} ${r.v} proto ${r.proto}: ${why}`;
}

// ---- the CLI ---------------------------------------------------------------

const MAX_BYTES = 512 * 1024;

const isUrl = (s) => /^https?:\/\//i.test(s);

/// Cache-busted, bounded, and an absence on every failure — the same rules
/// `hosts.js` applies to a mirror, for the same reason: these are bytes a
/// third party serves, and a gate that hangs on one is a gate that never
/// answers. `strict` turns the absence back into an error, which is what the
/// two inputs the run cannot proceed without need.
async function fetchJson(url, { timeoutMs = 4000, strict = false } = {}) {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeoutMs);
  try {
    const u = new URL(url);
    u.searchParams.set('ts', String(Date.now()));
    const r = await fetch(u, { cache: 'no-store', signal: ctl.signal });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    const len = Number(r.headers?.get?.('content-length'));
    if (Number.isFinite(len) && len > MAX_BYTES) throw new Error('too large');
    const text = await r.text();
    if (text.length > MAX_BYTES) throw new Error('too large');
    return JSON.parse(text);
  } catch (e) {
    if (strict) throw new InputError(`cannot read ${url}: ${e.message}`);
    return null;
  } finally {
    clearTimeout(timer);
  }
}

/// A path or a URL, read as JSON. Both are inputs the run cannot proceed
/// without, so both failures are exit 2.
async function readJson(where) {
  if (isUrl(where)) return fetchJson(where, { strict: true });
  try {
    return JSON.parse(await readFile(where, 'utf8'));
  } catch (e) {
    throw new InputError(`cannot read ${where}: ${e.message}`);
  }
}

/// A mirror is a URL in every supported shape (docs/hosts.md §3), but a local
/// tree under test may bind one to a file so the whole gate can be exercised
/// with no network at all.
const fetchMirror = (url) => (isUrl(url) ? fetchJson(url) : readFile(url, 'utf8').then(JSON.parse).catch(() => null));

const USAGE = [
  'usage: node deploy/check-hosts.mjs --tree <dir>',
  '       node deploy/check-hosts.mjs --games <path|url> --book <path|url> [--hosts-js <path>]',
];

export function parseArgs(argv) {
  const out = { games: '', book: '', hostsJs: '', tree: '', help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    const next = () => {
      const v = argv[i + 1];
      if (v === undefined) throw new InputError(`${a} needs a value`);
      i += 1;
      return v;
    };
    if (a === '--games') out.games = next();
    else if (a === '--book') out.book = next();
    else if (a === '--hosts-js') out.hostsJs = next();
    else if (a === '--tree') out.tree = next();
    else if (a === '--help' || a === '-h') out.help = true;
    else throw new InputError(`unknown argument ${a}`);
  }
  if (out.help) return out;
  // `--tree` is sugar for the three files at a site root, which is the shape
  // the deploy gate has: an extracted release asset. An explicit flag still
  // wins, so one file can be pointed somewhere else without spelling the
  // other two.
  if (out.tree) {
    out.games = out.games || path.join(out.tree, 'games.json');
    out.book = out.book || path.join(out.tree, 'server.json');
    out.hostsJs = out.hostsJs || path.join(out.tree, 'hosts.js');
  }
  if (!out.games) throw new InputError('--games (or --tree) is required');
  if (!out.book) throw new InputError('--book (or --tree) is required');
  // The hosts.js beside the book when the book is a local tree, because that
  // is the copy the pages in that tree import. A remote book has no such
  // neighbour to read, so the rule falls back to this checkout's own.
  if (!out.hostsJs) {
    out.hostsJs = isUrl(out.book)
      ? fileURLToPath(new URL('../web/hosts.js', import.meta.url))
      : path.join(path.dirname(out.book), 'hosts.js');
  }
  return out;
}

export async function main(argv) {
  let args;
  try {
    args = parseArgs(argv);
  } catch (e) {
    console.error(`check-hosts: ${e.message}`);
    for (const line of USAGE) console.error(line);
    return 2;
  }
  if (args.help) {
    for (const line of USAGE) console.log(line);
    return 0;
  }
  // A runtime with no WebSocket would fail every probe for a reason that has
  // nothing to do with the hosts, and this gate's failure blocks a release —
  // so it says "cannot tell" rather than "no host answered".
  if (typeof globalThis.WebSocket !== 'function') {
    console.error('check-hosts: this node has no global WebSocket; node 22+ is required');
    return 2;
  }
  let out;
  try {
    const hosts = await import(pathToFileURL(args.hostsJs).href);
    for (const fn of ['mirrorRefs', 'mergeBook', 'rankHosts', 'probeHost']) {
      if (typeof hosts[fn] !== 'function') throw new InputError(`${args.hostsJs} exports no ${fn}`);
    }
    const [games, book] = await Promise.all([readJson(args.games), readJson(args.book)]);
    console.log(`check-hosts: ${args.games}`);
    console.log(`check-hosts: ${args.book}`);
    out = await checkHosts({
      games,
      book,
      mirrors: fetchMirror,
      probe: (url, opts) => hosts.probeHost(url, opts),
      hosts,
    });
  } catch (e) {
    console.error(`check-hosts: ${e instanceof InputError ? e.message : e}`);
    return 2;
  }
  for (const r of out.results) console.log(formatResult(r));
  const failed = out.results.filter((r) => !r.ok);
  if (failed.length) {
    console.error(`check-hosts: ${failed.length} live game(s) have no host to join`);
    return 1;
  }
  console.log('check-hosts: every live server game has a host that answered');
  return 0;
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  process.exitCode = await main(process.argv.slice(2));
}
