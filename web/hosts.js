// The shared host picker: one address book, many machines, one choice.
//
// Contract: docs/hosts.md (§3 the book, §5 how a page picks). This module is
// imported by the hub and by every LIVE game page; the frozen pages keep their
// own inline discovery and read the legacy top-level keys, which is why those
// keys must keep working here too.
//
// Two rules shape the whole file:
//
//   * It is GAME-AGNOSTIC. Every per-game key is derived from the game id, so
//     a third game needs no edit here. Nothing below names `arena` or `fire`
//     except `keysFor`, which encodes the one historical asymmetry.
//   * The pure functions touch no DOM, no WebSocket and no fetch — not even at
//     module load — so `node --test` can import this file and exercise the
//     ranking rules without a browser or a server. Everything that does I/O
//     takes its transport as an injectable option for the same reason.

/// localStorage key holding the pinned host NAME. A pin beats rank (§5.6);
/// the hub's picker writes it and clears it. Shared with the game pages, so
/// it lives here rather than being spelled out in three HTML files.
export const PIN_KEY = 'ember-host';

/// Per-game key names in the address book. The arena, being first, owns the
/// bare names; every other game uses `<id>_ws` / `<id>_proto`. The same two
/// keys are used at the top level of the book and inside a host entry.
export function keysFor(id) {
  const game = String(id ?? '').trim();
  return game === 'arena'
    ? { ws: 'ws', proto: 'proto' }
    : { ws: `${game}_ws`, proto: `${game}_proto` };
}

/// `r211` -> 211. Higher is newer. Anything that does not start with a run of
/// digits (optionally behind an `r`) counts as ZERO, which sorts it last — an
/// unstamped dev build must never outrank a real one just because its version
/// string happened to sort high as text.
export function parseVersion(v) {
  const m = /^r?(\d+)/.exec(String(v ?? '').trim());
  return m ? Number(m[1]) : 0;
}

const isNum = (n) => typeof n === 'number' && Number.isFinite(n);
const str = (s) => (typeof s === 'string' ? s.trim() : '');
const now = () => (typeof performance === 'undefined' ? Date.now() : performance.now());

/// The legacy single-host shape, promoted to one unnamed entry.
///
/// Every book written before the host list — and every book a frozen deploy
/// script writes — carries only `ws` / `proto` / `fire_ws` / `fire_proto` at
/// the top level. Without this the pages would find zero hosts and refuse to
/// play against a server that is right there. The entry is deliberately
/// nameless: there is no name to know, and `renderChip` prints "server".
function legacyEntry(book) {
  const entry = { name: '', version: '', commit: '', updated: '', legacy: true };
  let anyAddress = false;
  for (const [key, value] of Object.entries(book)) {
    if (key === 'ws' || key.endsWith('_ws')) {
      if (str(value)) {
        entry[key] = str(value);
        anyAddress = true;
      }
    } else if (key === 'proto' || key.endsWith('_proto')) {
      if (isNum(value)) entry[key] = value;
    }
  }
  return anyAddress ? entry : null;
}

/// Merge the book's own `hosts` with the entries fetched from every mirror.
/// `name` is the merge key and LATER WINS, matching `publish-host.sh`: a
/// mirror's own copy of its entry is fresher than the writer's snapshot of it.
/// Falls back to the synthesized legacy entry when the list is absent or empty.
export function mergeBook(book, mirrorEntries = []) {
  const b = book && typeof book === 'object' ? book : {};
  const byKey = new Map();
  const listed = Array.isArray(b.hosts) ? b.hosts : [];
  for (const e of [...listed, ...(Array.isArray(mirrorEntries) ? mirrorEntries : [])]) {
    if (!e || typeof e !== 'object') continue;
    const name = str(e.name);
    // Map.set on an existing key keeps the insertion position and replaces
    // the value: later wins, order stays stable.
    byKey.set(name, { ...e, name });
  }
  const hosts = [...byKey.values()];
  if (hosts.length) return hosts;
  const legacy = legacyEntry(b);
  return legacy ? [legacy] : [];
}

async function fetchJson(url, timeoutMs) {
  const ctl = typeof AbortController === 'undefined' ? null : new AbortController();
  const timer = ctl ? setTimeout(() => ctl.abort(), timeoutMs) : null;
  try {
    const r = await fetch(String(url), {
      cache: 'no-store',
      ...(ctl ? { signal: ctl.signal } : {}),
    });
    if (!r.ok) return null;
    return await r.json();
  } catch {
    return null; // a mirror that does not answer is simply absent (§3)
  } finally {
    if (timer !== null) clearTimeout(timer);
  }
}

function cacheBust(url, base) {
  const u = new URL(String(url), base);
  u.searchParams.set('ts', String(Date.now()));
  return u.href;
}

/// Load the address book plus every mirror it links, in parallel, each with
/// its own timeout. A failed mirror is dropped, never fatal. Returns the raw
/// book (the pages still need `v` to cache-bust their bundles) beside the
/// merged host list.
export async function loadBook(rootUrl = './', { timeoutMs = 4000 } = {}) {
  const base = new URL(
    String(rootUrl),
    typeof location === 'undefined' ? 'http://localhost/' : location.href,
  );
  const book = (await fetchJson(cacheBust('server.json', base), timeoutMs)) || {};
  const mirrors = Array.isArray(book.mirrors) ? book.mirrors : [];
  const fetched = await Promise.all(
    mirrors.map((m) => {
      try {
        return fetchJson(cacheBust(m, base), timeoutMs);
      } catch {
        return Promise.resolve(null); // an unparseable mirror URL is absent too
      }
    }),
  );
  const entries = [];
  for (const got of fetched) {
    if (!got) continue;
    // A mirror publishes ONE entry, but accept a whole book or a bare array
    // too: a fork that mirrors the project's own `server.json` is the obvious
    // mistake to make, and dropping its hosts on the floor would be silent.
    if (Array.isArray(got)) entries.push(...got);
    else if (Array.isArray(got.hosts)) entries.push(...got.hosts);
    else entries.push(got);
  }
  return { book, hosts: mergeBook(book, entries) };
}

const openSocket = (url, socketFactory) =>
  socketFactory ? socketFactory(url) : new WebSocket(url);

/// Open a socket, say Hello, and wait for Welcome. Resolves
/// `{ ok: true, rttMs, welcome }` on the first `welcome` message, or
/// `{ ok: false, reason }` on error, close-before-welcome or timeout — and
/// ALWAYS closes the socket, the timeout path included: a probe of six dead
/// hosts must not leave six sockets connecting in the background.
///
/// `rttMs` is measured from socket creation, so it includes the TCP and TLS
/// handshake. That is deliberate: what ranks hosts is what the player will
/// actually wait for, not the application-layer round trip alone.
export function probeHost(url, options = {}) {
  const { proto = 0, timeoutMs = 5000, handle = 'browser', socketFactory = null } = options;
  return new Promise((resolve) => {
    const started = now();
    let done = false;
    let ws;
    let timer = null;
    const finish = (result) => {
      if (done) return;
      done = true;
      if (timer !== null) clearTimeout(timer);
      try {
        ws.close();
      } catch {}
      resolve(result);
    };
    try {
      ws = openSocket(String(url), socketFactory);
    } catch {
      resolve({ ok: false, reason: 'bad-url' });
      return;
    }
    timer = setTimeout(() => finish({ ok: false, reason: 'timeout' }), timeoutMs);
    ws.onerror = () => finish({ ok: false, reason: 'error' });
    ws.onclose = () => finish({ ok: false, reason: 'closed' });
    ws.onopen = () => {
      try {
        ws.send(JSON.stringify({ t: 'hello', proto, handle }));
      } catch {
        finish({ ok: false, reason: 'send' });
      }
    };
    ws.onmessage = (ev) => {
      let m;
      try {
        m = JSON.parse(ev.data);
      } catch {
        return;
      }
      if (m && m.t === 'welcome') {
        finish({ ok: true, rttMs: Math.round(now() - started), welcome: m });
      }
    };
  });
}

/// Ask one host for its lobbies. The reply is tagged `lobby_list` by the arena
/// and `lobbies` by Fire Racer; both are accepted, so a new game may use
/// either (§5). Resolves the array — an unreachable host resolves `[]`, since
/// reachability is what `probeHost` is for and a lobby list has no way to say
/// "not asked" that a caller could tell from "nothing open".
export function listLobbies(url, options = {}) {
  const { proto = 0, timeoutMs = 6000, handle = 'browser', socketFactory = null } = options;
  return new Promise((resolve) => {
    let done = false;
    let ws;
    let timer = null;
    const finish = (lobbies) => {
      if (done) return;
      done = true;
      if (timer !== null) clearTimeout(timer);
      try {
        ws.close();
      } catch {}
      resolve(lobbies);
    };
    try {
      ws = openSocket(String(url), socketFactory);
    } catch {
      resolve([]);
      return;
    }
    timer = setTimeout(() => finish([]), timeoutMs);
    ws.onerror = () => finish([]);
    ws.onclose = () => finish([]);
    ws.onopen = () => {
      try {
        ws.send(JSON.stringify({ t: 'hello', proto, handle }));
        ws.send(JSON.stringify({ t: 'list_lobbies' }));
      } catch {
        finish([]);
      }
    };
    ws.onmessage = (ev) => {
      let m;
      try {
        m = JSON.parse(ev.data);
      } catch {
        return;
      }
      if (!m || (m.t !== 'lobby_list' && m.t !== 'lobbies')) return;
      finish(Array.isArray(m.lobbies) ? m.lobbies : []);
    };
  });
}

function probeOf(probes, name) {
  if (!probes) return null;
  if (typeof probes.get === 'function') return probes.get(name) || null;
  return Object.prototype.hasOwnProperty.call(probes, name) ? probes[name] : null;
}

/// One host, as the page sees it: the book's entry with the live Welcome laid
/// over it. §5.4 — the reply's `host`, `version`, `players` and `lobbies`
/// override what the book said, because the book is a snapshot and the socket
/// is the present tense. A server that predates those fields sends empty
/// strings and zeros, so every override is guarded on being non-empty and
/// falls back to the entry.
function viewOf(entry, game, probe) {
  const k = keysFor(game);
  const ok = !!(probe && probe.ok);
  const w = ok && probe.welcome && typeof probe.welcome === 'object' ? probe.welcome : {};
  const version = str(w.version) || str(entry.version);
  const bookProto = isNum(entry[k.proto]) ? entry[k.proto] : null;
  return {
    entry,
    game,
    bookName: str(entry.name),
    name: str(w.host) || str(entry.name),
    url: str(entry[k.ws]),
    version,
    versionNum: parseVersion(version),
    commit: str(w.commit) || str(entry.commit),
    // An old server reports no players at all; the contract's answer to a
    // missing informational field is "zeros", so an unknown load ranks as
    // empty rather than as full.
    players: isNum(w.players) ? w.players : isNum(entry.players) ? entry.players : 0,
    lobbies: isNum(w.lobbies) ? w.lobbies : isNum(entry.lobbies) ? entry.lobbies : 0,
    proto: isNum(w.proto) ? w.proto : bookProto,
    updated: str(entry.updated),
    ok,
    rttMs: ok && isNum(probe.rttMs) ? probe.rttMs : null,
    reason: probe && !probe.ok ? probe.reason || 'unreachable' : null,
  };
}

const byName = (a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0);

/// PURE. The whole picking rule of §5, with the network already done.
///
/// `entries` are book entries; `probes` maps a book host name (`''` for the
/// legacy entry) to a `probeHost` result. Pass `probes` and only hosts that
/// ANSWERED survive; omit it entirely and the ranking runs on the book alone,
/// which is what the unit tests and any caller without a network do.
///
/// Order: version desc, then `updated` desc, then name. Then, among the
/// responders sharing the newest version, fewest players, then lowest round
/// trip, then name — the load and latency tie-breaks are deliberately NOT
/// applied across builds, because a newer build wins first (§5.5). A pin that
/// answered jumps to the front (§5.6).
export function rankHosts(entries, options = {}) {
  const { game, proto = null, pinned = null, probes = null } = options;
  const k = keysFor(game);
  const wanted = proto === null || proto === undefined ? null : Number(proto);

  let views = (Array.isArray(entries) ? entries : [])
    .filter((e) => e && typeof e === 'object' && str(e[k.ws]))
    .map((e) => viewOf(e, game, probeOf(probes, str(e.name))));

  if (probes) views = views.filter((v) => v.ok);
  if (wanted !== null) {
    // The live `proto` from Welcome decides when we have it, the book's key
    // otherwise. A host whose protocol is unknown from BOTH is excluded: the
    // page cannot promise a join it has no evidence will be accepted.
    views = views.filter((v) => v.proto === wanted);
  }

  views.sort(
    (a, b) =>
      b.versionNum - a.versionNum ||
      (a.updated < b.updated ? 1 : a.updated > b.updated ? -1 : 0) ||
      byName(a, b),
  );

  if (views.length) {
    const newest = views[0].versionNum;
    const head = views.filter((v) => v.versionNum === newest);
    const tail = views.filter((v) => v.versionNum !== newest);
    head.sort(
      (a, b) =>
        a.players - b.players ||
        (a.rttMs ?? Infinity) - (b.rttMs ?? Infinity) ||
        byName(a, b),
    );
    views = [...head, ...tail];
  }

  const pin = str(pinned);
  if (pin) {
    const i = views.findIndex((v) => v.name === pin || v.bookName === pin);
    if (i > 0) views = [views[i], ...views.slice(0, i), ...views.slice(i + 1)];
  }

  return { chosen: views[0] || null, candidates: views };
}

/// The pin, read defensively: a browser with storage disabled throws on the
/// getter, and a page that cannot read a preference must still pick a host.
export function readPin() {
  try {
    return str(localStorage.getItem(PIN_KEY));
  } catch {
    return '';
  }
}

export function writePin(name) {
  try {
    if (str(name)) localStorage.setItem(PIN_KEY, str(name));
    else localStorage.removeItem(PIN_KEY);
  } catch {}
}

/// Load, probe every host that runs this game, rank. `proto` narrows to the
/// hosts a page can actually play on; the hub omits it and gets everything.
///
/// Every host running the game is probed even when `proto` is set, so the hub
/// and the game pages make the same measurements and an entry whose book
/// protocol is stale is still judged by what its server actually says.
export async function chooseHost(rootUrl = './', options = {}) {
  const {
    game,
    proto = null,
    pinned,
    timeoutMs = 4000,
    probeTimeoutMs = 5000,
    socketFactory = null,
  } = options;
  const k = keysFor(game);
  const { book, hosts } = await loadBook(rootUrl, { timeoutMs });
  const running = hosts.filter((e) => str(e[k.ws]));
  const probes = new Map();
  await Promise.all(
    running.map(async (e) => {
      const r = await probeHost(str(e[k.ws]), {
        proto: proto === null || proto === undefined ? 0 : Number(proto),
        timeoutMs: probeTimeoutMs,
        socketFactory,
      });
      probes.set(str(e.name), r);
    }),
  );
  const pin = pinned === undefined ? readPin() : pinned;
  const ranked = rankHosts(running, { game, proto, pinned: pin, probes });
  return { book, hosts, probes, pinned: pin, ...ranked };
}

/// "server: amber-otter · r211 · 43 ms", with the commit and address in the
/// tooltip. The one place the empty legacy name is spelled out as "server".
export function renderChip(el, choice) {
  if (!el) return;
  if (!choice) {
    el.textContent = 'no server';
    el.title = 'no host answered — nothing to play on right now';
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
  ]
    .filter(Boolean)
    .join('\n');
}
