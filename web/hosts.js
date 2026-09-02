// Host discovery, shared by the hub and every live game page.
//
// The model is in docs/hosts.md: many independent machines, one published
// address book (server.json), and the CLIENT doing the choosing so that no
// host has to know about any other. This module is the client half of that
// contract and nothing else — it is deliberately game-agnostic (the game id
// is a parameter, and both per-game keys are derived from it), so adding a
// game needs no edit here.
//
// Two rules shape the code:
//
//   * Nothing runs at import time. No fetch, no WebSocket, no DOM. The pure
//     functions are unit-tested under `node --test`, which has no DOM and
//     must be able to import this file without a browser.
//   * Every network path fails soft. A missing server.json, a mirror that
//     404s, a host that never answers — each is an absence, never an
//     exception. A page that cannot reach anything still renders.

/// Where the hub's picker stores the player's pinned host name. Empty or
/// absent means automatic.
export const PIN_KEY = 'ember-host';

/// The arena, being first, owns the bare key names; every other game gets
/// `<id>_ws` / `<id>_proto`. Same derivation at the top level of the book
/// and inside a host entry (docs/hosts.md §3).
export function keysFor(id) {
  const g = String(id || '').trim();
  return g === 'arena' ? { ws: 'ws', proto: 'proto' } : { ws: `${g}_ws`, proto: `${g}_proto` };
}

/// The build stamp is per game for the same reason the address is: one host
/// may run the arena from one commit and Fire Racer from another, and a
/// one-game deploy must not restamp the other. Same derivation as `keysFor`
/// — the arena owns the bare `version`/`commit`, everyone else is prefixed —
/// and a reader falls back to the bare pair, because entries written by an
/// older writer carry only that (docs/hosts.md §3).
export function verKeysFor(id) {
  const g = String(id || '').trim();
  return g === 'arena'
    ? { version: 'version', commit: 'commit' }
    : { version: `${g}_version`, commit: `${g}_commit` };
}

/// The documented host-name shape (docs/hosts.md §6). It is checked here too,
/// because the client is the one path where a third party's bytes arrive: a
/// name with a slash or a space in it is not a host anyone published.
const NAME_RE = /^[a-z0-9-]{3,32}$/;

/// Bounds on untrusted input. The book is one file a writer pushes, but the
/// mirrors it links to are third-party bytes served by anyone, and neither
/// list has a natural end. Every one of these is a ceiling nobody legitimate
/// reaches: the fleet is a handful of machines, not a hundred.
const MAX_HOSTS = 128;
const MAX_MIRRORS = 32;
const MAX_BYTES = 512 * 1024;

const warn = (msg) => { try { globalThis.console?.warn?.(`hosts.js: ${msg}`); } catch { /* no console */ } };

/// `r211` -> 211. Higher is newer. Anything unparseable counts as zero, so
/// an unstamped dev build sorts below every real one instead of throwing.
export function parseVersion(v) {
  const m = /^r?(\d+)/.exec(String(v ?? '').trim());
  return m ? Number(m[1]) : 0;
}

const isNum = (v) => typeof v === 'number' && Number.isFinite(v);
const str = (v) => (typeof v === 'string' ? v.trim() : '');
const now = () => (typeof performance !== 'undefined' ? performance.now() : Date.now());

/// The key the synthesised legacy entry is merged under. A Symbol on purpose:
/// the legacy entry is nameless, and a string key of `''` is exactly what a
/// mirror entry with no name would produce, so a sentinel no payload can
/// spell is the only way "a mirror can never erase the legacy address" holds.
const LEGACY_KEY = Symbol('legacy');

/// One entry, normalised, or null when it is not an entry at all. Every value
/// that leaves this module is the shape the rest of the code assumes: address
/// keys are non-empty strings or absent, protocol keys are numbers or absent,
/// the name is the documented shape. Doing it here rather than at each call
/// site is what makes `entry[k.ws].trim()` safe in a page that never heard of
/// mirrors — a hostile `"ws": 123` is an absence, not a TypeError.
function sanitiseEntry(e) {
  if (!e || typeof e !== 'object' || Array.isArray(e)) return null;
  const name = str(e.name);
  if (name && !NAME_RE.test(name)) return null;
  const out = { ...e, name };
  delete out.legacy; // only this module may claim to be the legacy entry
  for (const k of Object.keys(out)) {
    if (k === 'ws' || k.endsWith('_ws')) {
      const v = str(out[k]);
      if (v) out[k] = v; else delete out[k];
    } else if (k === 'proto' || k.endsWith('_proto')) {
      if (!isNum(out[k])) delete out[k];
    }
  }
  return out;
}

/// The legacy top-level keys, as one nameless entry — but only for the games
/// no merged entry carries. A book in the OLD single-host shape has no
/// `hosts` at all and yields its whole self here, which is the case the
/// promotion was written for; a HALF-migrated book (the first per-game
/// publish has written `hosts[]` for the arena, and `fire_ws` at the top
/// level is still the only fire address there is) yields just the missing
/// game's keys. Returns null when every address is already represented.
function legacyEntry(book, merged) {
  const covered = new Set();
  for (const e of merged) {
    for (const k of Object.keys(e)) if (k === 'ws' || k.endsWith('_ws')) covered.add(k);
  }
  const e = { name: '', version: '', commit: '', updated: '', legacy: true };
  let any = false;
  for (const [k, v] of Object.entries(book)) {
    if ((k === 'ws' || k.endsWith('_ws')) && !covered.has(k) && str(v)) {
      e[k] = str(v);
      any = true;
      const p = k === 'ws' ? 'proto' : `${k.slice(0, -3)}_proto`;
      if (isNum(book[p])) e[p] = book[p];
    }
  }
  return any ? e : null;
}

/// Merge a book's own `hosts` with the entries its mirrors served.
///
/// `name` is the merge key. Inside the book's own list the later entry wins
/// outright (a publish REPLACES the entry of that name — it does not patch
/// it). Mirrors are the untrusted half and play by a stricter rule: each is
/// BOUND to the one name the writer authorised it for, may serve exactly one
/// entry, and may only fill a name the book does not already carry. A mirror
/// that serves someone else's name — or a whole book — is dropped, because
/// "add my box to the list" is the whole privilege a writer meant to grant,
/// and the alternative is one linked URL silently redirecting every player.
///
/// `mirrors` is `[{ name, entry }]`: the bound name from the book's
/// `mirrors[]`, and whatever that URL answered with.
export function mergeBook(book, mirrors = []) {
  const b = book && typeof book === 'object' && !Array.isArray(book) ? book : {};
  const byKey = new Map();
  const listed = Array.isArray(b.hosts) ? b.hosts : [];
  for (const e of listed) {
    const v = sanitiseEntry(e);
    if (!v) continue;
    // Map.set on an existing key keeps the insertion position and replaces
    // the value: later wins, order stable.
    if (!byKey.has(v.name) && byKey.size >= MAX_HOSTS) break;
    byKey.set(v.name, v);
  }

  // Seeded BEFORE the mirrors, so a mirror can add itself next to the legacy
  // address but can never take its place.
  const legacy = legacyEntry(b, byKey.values());
  if (legacy && byKey.size < MAX_HOSTS) byKey.set(LEGACY_KEY, legacy);

  for (const m of Array.isArray(mirrors) ? mirrors : []) {
    if (!m || typeof m !== 'object') continue;
    const bound = str(m.name);
    if (!NAME_RE.test(bound)) continue;
    if (byKey.has(bound)) continue; // the book's own entry wins
    const v = sanitiseEntry(m.entry);
    if (!v || v.name !== bound) {
      warn(`mirror ${bound} served an entry it is not bound to; ignored`);
      continue;
    }
    if (byKey.size >= MAX_HOSTS) break;
    byKey.set(bound, v);
  }
  return [...byKey.values()];
}

async function fetchJson(url, timeoutMs) {
  const ctl = typeof AbortController !== 'undefined' ? new AbortController() : null;
  const timer = ctl && timeoutMs > 0 ? setTimeout(() => ctl.abort(), timeoutMs) : null;
  try {
    const r = await fetch(String(url), { cache: 'no-store', signal: ctl ? ctl.signal : undefined });
    if (!r.ok) return null;
    // Bounded before it is parsed. The timeout alone does not bound a mirror:
    // half a megabyte of nonsense arrives well inside four seconds, and
    // JSON.parse on it is time every visitor pays.
    const len = Number(r.headers?.get?.('content-length'));
    if (Number.isFinite(len) && len > MAX_BYTES) return null;
    const text = await r.text();
    if (text.length > MAX_BYTES) return null;
    return JSON.parse(text);
  } catch {
    return null; // a mirror that does not answer is absent, not an error
  } finally {
    if (timer) clearTimeout(timer);
  }
}

function bust(url, base) {
  try {
    const u = new URL(String(url), base);
    u.searchParams.set('ts', String(Date.now()));
    return u.toString();
  } catch {
    return String(url);
  }
}

/// One entry of the book's `mirrors[]`, as a bound reference, or null.
///
/// The object form `{ url, name }` is the only trusted one: the name is what
/// the writer authorised that URL to publish. A bare string is the old shape
/// and is UNBOUND — it names no host, so nothing it serves can be attributed,
/// and it is logged and skipped rather than trusted or thrown over.
function mirrorRef(m) {
  if (typeof m === 'string') {
    warn(`mirror ${m} is a bare URL with no host name; ignored — see docs/hosts.md §3`);
    return null;
  }
  if (!m || typeof m !== 'object' || Array.isArray(m)) return null;
  const url = str(m.url);
  const name = str(m.name);
  if (!url || !NAME_RE.test(name)) {
    if (url) warn(`mirror ${url} is bound to no valid host name; ignored`);
    return null;
  }
  return { url, name };
}

/// Fetch the address book cache-busted, then every mirror in parallel with
/// its own timeout, and merge. Mirrors are third-party URLs: one that hangs
/// must not hold the page, one that returns nonsense must not break it, and
/// one that returns a megabyte must not cost the visitor anything, so each
/// result is bounded and validated on its own and a failure is dropped.
/// A mirror serves exactly one entry, under the name it is bound to.
export async function loadBook(rootUrl = './', { timeoutMs = 4000 } = {}) {
  const base = typeof location !== 'undefined' ? location.href : 'http://localhost/';
  const root = new URL(String(rootUrl), base);
  const book = (await fetchJson(bust(new URL('server.json', root), base), timeoutMs)) || {};
  try {
    const listed = (Array.isArray(book.mirrors) ? book.mirrors : []).slice(0, MAX_MIRRORS);
    const refs = listed.map(mirrorRef).filter(Boolean);
    const fetched = await Promise.all(refs.map((m) => fetchJson(bust(m.url, base), timeoutMs)));
    return { book, hosts: mergeBook(book, refs.map((m, i) => ({ name: m.name, entry: fetched[i] }))) };
  } catch (e) {
    // The contract is that a mirror is an absence, never an error that
    // reaches the player. A throw anywhere in the merge would otherwise
    // reject this promise — and both game pages await it at module top
    // level, so the page would not start at all.
    warn(`the mirror merge failed (${e}); continuing with the book alone`);
    return { book, hosts: mergeBook(book, []) };
  }
}

/// Open a socket, say Hello, and wait for the Welcome. Resolves
/// `{ok:true, rttMs, welcome}` or `{ok:false, reason}` — never rejects, and
/// always closes the socket, including on the timeout path: a probe that
/// leaked its socket would leave one connection per host per refresh.
///
/// `rttMs` is measured from socket construction, so it includes the TCP and
/// TLS handshake. That is deliberate: what the ranking wants is "how long
/// until this host can start my game", not a warm ping.
export function probeHost(url, { proto = 0, timeoutMs = 5000, handle = 'browser', socketFactory } = {}) {
  return new Promise((resolve) => {
    const target = str(url);
    if (!target) { resolve({ ok: false, reason: 'no-url' }); return; }
    const make = socketFactory || ((u) => new globalThis.WebSocket(u));
    let ws;
    const t0 = now();
    try { ws = make(target); } catch { resolve({ ok: false, reason: 'bad-url' }); return; }
    let done = false;
    let timer = null;
    const finish = (res) => {
      if (done) return;
      done = true;
      if (timer !== null) clearTimeout(timer);
      try { ws.close(); } catch { /* already gone */ }
      resolve(res);
    };
    timer = setTimeout(() => finish({ ok: false, reason: 'timeout' }), timeoutMs);
    ws.onerror = () => finish({ ok: false, reason: 'error' });
    ws.onclose = () => finish({ ok: false, reason: 'closed' });
    ws.onopen = () => {
      try { ws.send(JSON.stringify({ t: 'hello', proto, handle })); } catch { finish({ ok: false, reason: 'send' }); }
    };
    ws.onmessage = (ev) => {
      let m;
      try { m = JSON.parse(ev.data); } catch { return; }
      if (m && m.t === 'welcome') finish({ ok: true, rttMs: Math.round(now() - t0), welcome: m });
    };
  });
}

function probeOf(probes, name) {
  if (!probes) return null;
  if (typeof probes.get === 'function') return probes.get(name) || null;
  return Object.prototype.hasOwnProperty.call(probes, name) ? probes[name] : null;
}

/// One host, as a page wants to show and rank it: the book's entry with the
/// live Welcome laid over it. The live values win when the server sent them,
/// and a server that predates those fields sends nothing, so the book's
/// values stand — which is the whole reason the book carries them at all.
function viewOf(entry, game, probe) {
  const k = keysFor(game);
  const vk = verKeysFor(game);
  const ok = !!(probe && probe.ok);
  const w = (ok && probe.welcome) || {};
  const liveHost = str(w.host);
  const liveVer = str(w.version);
  const liveCommit = str(w.commit);
  // This game's own stamp, falling back to the bare pair: an entry written
  // by an older writer carries only that, and reading 0 there would rank
  // every such host below every new-format one (docs/hosts.md §3).
  const version = liveVer || str(entry[vk.version]) || str(entry.version);
  return {
    entry,
    game,
    // The book's name stays visible next to the live one: it is the merge
    // key, the pin key and the key the probe map is built on, so anything
    // that has to address this host again uses `bookName`.
    bookName: str(entry.name),
    name: liveHost || str(entry.name),
    url: str(entry[k.ws]),
    version,
    versionNum: parseVersion(version),
    commit: liveCommit || str(entry[vk.commit]) || str(entry.commit),
    updated: str(entry.updated),
    proto: isNum(w.proto) ? w.proto : (isNum(entry[k.proto]) ? entry[k.proto] : null),
    players: isNum(w.players) ? w.players : (isNum(entry.players) ? entry.players : 0),
    lobbies: isNum(w.lobbies) ? w.lobbies : (isNum(entry.lobbies) ? entry.lobbies : 0),
    ok,
    rttMs: ok && isNum(probe.rttMs) ? probe.rttMs : null,
    reason: probe && !probe.ok ? String(probe.reason || 'unreachable') : null,
    legacy: !!entry.legacy,
  };
}

const cmpName = (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);

/// The pick, as one pure function (docs/hosts.md §5 steps 2, 3, 5 and 6).
///
/// Pure on purpose: everything that touches the network happens in
/// `chooseHost`, so the rule itself can be tested exhaustively with plain
/// objects and no server anywhere.
///
///   * filter to the hosts that run this game, and — when `proto` is given —
///     to those that speak it. A probe's Welcome outranks the book here too:
///     the book records what was published, the Welcome is what is running.
///   * order by version desc, then `updated` desc, then name.
///   * among the responders sharing the NEWEST version only: fewest players,
///     then lowest rtt, then name. Older builds keep their version order
///     below — an emptier old host must never beat a newer one, or nobody
///     would ever move to the new build.
///   * a pin that answered goes first, and the rest stay in rank order
///     behind it, so failover from a pinned host is still the normal rule.
///
/// `probes` is a Map (or plain object) keyed by the BOOK name. Passing it at
/// all means "these are the hosts I asked": anything that did not answer is
/// dropped. Omitting it ranks the book alone, which is what the ordering
/// tests and any pre-probe render want.
export function rankHosts(entries, { game, proto = null, pinned = null, probes = null } = {}) {
  const k = keysFor(game);
  const wanted = proto === null || proto === undefined ? null : Number(proto);
  let list = (Array.isArray(entries) ? entries : [])
    .filter((e) => e && typeof e === 'object' && str(e[k.ws]))
    .map((e) => viewOf(e, game, probeOf(probes, str(e.name))));
  if (probes) list = list.filter((v) => v.ok);
  // The hosts the protocol filter is about to drop are kept as data. A page
  // that reports "the games are offline" when a host answered on protocol 11
  // is telling the player to leave during exactly the deploy window where the
  // truthful answer — "play the build that speaks 11" — still has a game in
  // it (docs/hosts.md §5).
  const wrongProto = wanted === null ? [] : list.filter((v) => v.proto !== wanted);
  if (wanted !== null) list = list.filter((v) => v.proto === wanted);

  list.sort((a, b) => (b.versionNum - a.versionNum)
    || (a.updated < b.updated ? 1 : a.updated > b.updated ? -1 : 0)
    || cmpName(a, b));

  const newest = list.length ? list[0].versionNum : 0;
  const head = list.filter((v) => v.versionNum === newest);
  const tail = list.filter((v) => v.versionNum !== newest);
  head.sort((a, b) => (a.players - b.players)
    || ((a.rttMs ?? Infinity) - (b.rttMs ?? Infinity))
    || cmpName(a, b));

  const candidates = [...head, ...tail];
  const pin = str(pinned);
  if (pin) {
    // The BOOK name only. The pin is written from the hosts strip, which is
    // keyed on `bookName`, so nothing ever pins a live Welcome name — and
    // matching one would let a host reporting someone else's name absorb the
    // pin, leaving the host the player actually chose unpromoted.
    const i = candidates.findIndex((v) => v.bookName === pin);
    if (i > 0) candidates.unshift(...candidates.splice(i, 1));
  }
  return { chosen: candidates[0] || null, candidates, wrongProto };
}

/// Read the pinned host name. Guarded: a browser with storage disabled
/// throws on the getter itself, and that must not take the page down.
export function readPin() {
  try { return str(globalThis.localStorage?.getItem(PIN_KEY)); } catch { return ''; }
}

export function writePin(name) {
  try {
    if (str(name)) globalThis.localStorage.setItem(PIN_KEY, str(name));
    else globalThis.localStorage.removeItem(PIN_KEY);
  } catch { /* storage disabled: the pin is a convenience, not a requirement */ }
}

/// Load, probe every host that runs this game, rank. Probing happens BEFORE
/// the protocol filter deliberately: the book's protocol key can be stale or
/// (on an old publish) missing, and one extra socket per host is a far
/// cheaper mistake than silently hiding a host that would have worked.
export async function chooseHost(rootUrl = './', {
  game,
  proto = null,
  pinned,
  timeoutMs = 4000,
  probeTimeoutMs = 5000,
  socketFactory,
} = {}) {
  const { book, hosts } = await loadBook(rootUrl, { timeoutMs });
  const k = keysFor(game);
  const running = hosts.filter((e) => e && typeof e === 'object' && str(e[k.ws]));
  const probes = new Map();
  await Promise.all(running.map(async (e) => {
    const r = await probeHost(str(e[k.ws]), {
      proto: proto ?? 0,
      timeoutMs: probeTimeoutMs,
      socketFactory,
    });
    probes.set(str(e.name), r);
  }));
  const pin = pinned === undefined ? readPin() : str(pinned);
  const { chosen, candidates, wrongProto } = rankHosts(running, { game, proto, pinned: pin, probes });
  return { book, hosts, probes, chosen, candidates, wrongProto, pinned: pin };
}

/// Ask one host for its open lobbies. The arena tags the reply `lobby_list`
/// and Fire Racer tags it `lobbies`; both are accepted, so a new game may
/// use either and needs no change here. Resolves the array — an unreachable
/// host resolves empty, because the hosts strip already says which hosts
/// answered and a lobby list has no second way to say "nothing".
export function listLobbies(url, { proto = 0, timeoutMs = 6000, handle = 'browser', socketFactory } = {}) {
  return new Promise((resolve) => {
    const target = str(url);
    if (!target) { resolve([]); return; }
    const make = socketFactory || ((u) => new globalThis.WebSocket(u));
    let ws;
    try { ws = make(target); } catch { resolve([]); return; }
    let done = false;
    let timer = null;
    const finish = (lobbies) => {
      if (done) return;
      done = true;
      if (timer !== null) clearTimeout(timer);
      try { ws.close(); } catch { /* already gone */ }
      resolve(lobbies);
    };
    timer = setTimeout(() => finish([]), timeoutMs);
    ws.onerror = () => finish([]);
    ws.onclose = () => finish([]);
    ws.onopen = () => {
      try {
        ws.send(JSON.stringify({ t: 'hello', proto, handle }));
        ws.send(JSON.stringify({ t: 'list_lobbies' }));
      } catch { finish([]); }
    };
    ws.onmessage = (ev) => {
      let m;
      try { m = JSON.parse(ev.data); } catch { return; }
      if (!m || (m.t !== 'lobby_list' && m.t !== 'lobbies')) return;
      finish(Array.isArray(m.lobbies) ? m.lobbies : []);
    };
  });
}

/// The "where am I playing" chip every page carries. Name, build and round
/// trip on the face; commit and address in the tooltip, because the address
/// is a tunnel URL nobody wants to read but everybody needs when reporting a
/// problem.
/// `wrongProto` (and the page's own `proto`) turn the empty case into two
/// different sentences. "Offline" and "everyone answered on another protocol"
/// are not the same news: the second one still has a playable build behind
/// it, and saying the first tells the player to give up.
export function renderChip(el, choice, { wrongProto = [], proto = null } = {}) {
  if (!el) return;
  if (!choice) {
    const other = (Array.isArray(wrongProto) ? wrongProto : []).filter((v) => v && isNum(v.proto));
    if (other.length) {
      el.textContent = 'wrong protocol';
      el.title = [
        isNum(proto) ? `this build speaks protocol ${proto}` : 'this build speaks another protocol',
        ...other.slice(0, 4).map((v) => `${v.name || 'a host'} speaks ${v.proto}`),
        'play the build that matches, from the games page',
      ].join('\n');
      return;
    }
    el.textContent = 'no server';
    el.title = 'no host answered — the games are offline right now';
    return;
  }
  const bits = [`server: ${choice.name || 'server'}`];
  if (choice.version) bits.push(choice.version);
  if (isNum(choice.rttMs)) bits.push(`${choice.rttMs} ms`);
  el.textContent = bits.join(' · ');
  el.title = [
    choice.name ? `host ${choice.name}` : 'unnamed host (legacy address book)',
    choice.version ? `build ${choice.version}` : null,
    choice.commit ? `commit ${choice.commit}` : null,
    choice.url || null,
  ].filter(Boolean).join('\n');
}
