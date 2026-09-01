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

/// `r211` -> 211. Higher is newer. Anything unparseable counts as zero, so
/// an unstamped dev build sorts below every real one instead of throwing.
export function parseVersion(v) {
  const m = /^r?(\d+)/.exec(String(v ?? '').trim());
  return m ? Number(m[1]) : 0;
}

const isNum = (v) => typeof v === 'number' && Number.isFinite(v);
const str = (v) => (typeof v === 'string' ? v.trim() : '');
const now = () => (typeof performance !== 'undefined' ? performance.now() : Date.now());

/// A book in the OLD single-host shape carries no `hosts`, only the legacy
/// top-level keys. Rather than special-casing that everywhere downstream,
/// synthesise one nameless entry out of them: from here on the rest of the
/// module only ever sees a list of host entries. Returns null when the book
/// names no address at all.
function legacyEntry(book) {
  const e = { name: '', version: '', commit: '', updated: '', legacy: true };
  let any = false;
  for (const [k, v] of Object.entries(book)) {
    if (k === 'ws' || k.endsWith('_ws')) {
      if (str(v)) { e[k] = str(v); any = true; }
    } else if (k === 'proto' || k.endsWith('_proto')) {
      if (isNum(v)) e[k] = v;
    }
  }
  return any ? e : null;
}

/// Merge a book's own `hosts` with the entries pulled from its mirrors.
/// `name` is the merge key and the later entry wins outright (a publish
/// REPLACES the entry of that name — it does not patch it), which is what
/// lets a mirror keep its own entry fresh without a writer.
export function mergeBook(book, mirrorEntries = []) {
  const b = book && typeof book === 'object' ? book : {};
  const byKey = new Map();
  const listed = Array.isArray(b.hosts) ? b.hosts : [];
  for (const e of [...listed, ...(Array.isArray(mirrorEntries) ? mirrorEntries : [])]) {
    if (!e || typeof e !== 'object' || Array.isArray(e)) continue;
    const name = str(e.name);
    // Map.set on an existing key keeps the insertion position and replaces
    // the value: later wins, order stable.
    byKey.set(name, { ...e, name });
  }
  const out = [...byKey.values()];
  if (out.length) return out;
  const legacy = legacyEntry(b);
  return legacy ? [legacy] : [];
}

async function fetchJson(url, timeoutMs) {
  const ctl = typeof AbortController !== 'undefined' ? new AbortController() : null;
  const timer = ctl && timeoutMs > 0 ? setTimeout(() => ctl.abort(), timeoutMs) : null;
  try {
    const r = await fetch(String(url), { cache: 'no-store', signal: ctl ? ctl.signal : undefined });
    if (!r.ok) return null;
    return await r.json();
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

/// Fetch the address book cache-busted, then every mirror in parallel with
/// its own timeout, and merge. Mirrors are third-party URLs: one that hangs
/// must not hold the page, and one that returns nonsense must not break it,
/// so each result is validated on its own and a failure is simply dropped.
/// A mirror may serve one entry, a bare array, or a whole book.
export async function loadBook(rootUrl = './', { timeoutMs = 4000 } = {}) {
  const base = typeof location !== 'undefined' ? location.href : 'http://localhost/';
  const root = new URL(String(rootUrl), base);
  const book = (await fetchJson(bust(new URL('server.json', root), base), timeoutMs)) || {};
  const mirrors = Array.isArray(book.mirrors) ? book.mirrors : [];
  const fetched = await Promise.all(mirrors.map((m) => fetchJson(bust(m, base), timeoutMs)));
  const entries = [];
  for (const r of fetched) {
    if (!r || typeof r !== 'object') continue;
    if (Array.isArray(r)) entries.push(...r);
    else if (Array.isArray(r.hosts)) entries.push(...r.hosts);
    else entries.push(r);
  }
  return { book, hosts: mergeBook(book, entries) };
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
  const ok = !!(probe && probe.ok);
  const w = (ok && probe.welcome) || {};
  const liveHost = str(w.host);
  const liveVer = str(w.version);
  const liveCommit = str(w.commit);
  const version = liveVer || str(entry.version);
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
    commit: liveCommit || str(entry.commit),
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
    const i = candidates.findIndex((v) => v.bookName === pin || v.name === pin);
    if (i > 0) candidates.unshift(...candidates.splice(i, 1));
  }
  return { chosen: candidates[0] || null, candidates };
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
  const { chosen, candidates } = rankHosts(running, { game, proto, pinned: pin, probes });
  return { book, hosts, probes, chosen, candidates, pinned: pin };
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
export function renderChip(el, choice) {
  if (!el) return;
  if (!choice) {
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
