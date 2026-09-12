// The shared loader: one call a live page makes instead of loading its own
// bundle, and one event stream it renders.
//
// What it is for. A page used to learn which server it would join only after
// its game bundle had downloaded, compiled and initialised, because the
// protocol it filters hosts by came from the bundle's own export. On the page
// with the largest bundle that is about a minute of a chip reading "server …"
// with nothing said about how far the download had got. The protocol is in
// `games.json` already, so discovery needs nothing from the bundle: here it
// starts in the page's first tick, beside the download rather than behind it.
//
// What lives where. The rules — which phase may follow which, the percentage,
// the rate, the stall test and the line each event carries — are in
// `crates/ember-loader`, compiled to `pkg/ember_loader.js` and tested natively
// with no browser. This file is the four pieces of browser input and output
// that a test cannot hold: discovery through `hosts.js`, the counting fetch,
// the compile, and the call into the game's own wasm-bindgen glue.
//
// Three rules shape it, and each one is a failure it is written around:
//
//   * Nothing waits for the loader's own bundle. Every task starts in the
//     first tick and the drive calls are queued with their real timestamps,
//     so the numbers a player reads are measured from when the page began.
//   * The page renders; this file never touches the DOM. Six pages keep six
//     different progress lines and one wording.
//   * A loader that cannot load is not a page that cannot play. If the wasm
//     half fails, the legacy path runs here — import the glue, let the
//     browser fetch the bundle itself — and the page starts with one honest
//     line instead of progress numbers.

import { chooseHost, probeHost, rankHosts } from './hosts.js';

/// This module's own URL, which is also where the site root is: the loader is
/// deployed beside `hosts.js` and `games.json`, so a page passes no paths it
/// does not own.
const SELF = import.meta.url;

const clock = () => (typeof performance === 'undefined' ? Date.now() : performance.now());
const CATALOG_TIMEOUT_MS = 4000;

/// The deploy stamp, taken from this module's own query.
///
/// The page carries exactly one cache token — the `?v=` on its import of this
/// file — and the deploy rewrites that one token. Every URL the loader then
/// builds carries the stamp of the page that asked for it, which is stricter
/// than reading the stamp out of the address book: the book is rewritten by
/// the next deploy, so a cached page could ask for a bundle newer than
/// itself, and a page and a bundle from different builds is exactly the pair
/// nobody can reproduce.
export function loaderStamp(url = SELF) {
  try {
    return new URL(String(url), 'http://ember.invalid/').searchParams.get('v') || '';
  } catch {
    return '';
  }
}

/// `pkg/arena.js` + a stamp -> `pkg/arena.js?v=<stamp>`.
export function withStamp(url, stamp) {
  if (!stamp) return url;
  return `${url}${url.includes('?') ? '&' : '?'}v=${encodeURIComponent(stamp)}`;
}

/// The wasm beside a wasm-bindgen glue module, by the name the generator gives
/// it. A page may still name it outright.
export function bundleWasm(glue) {
  return String(glue).replace(/\.js$/, '_bg.wasm');
}

/// The protocol a page speaks, out of the catalog.
///
/// Matched on the page's OWN version key rather than on `live`, so a page that
/// is frozen by the next release keeps the protocol it shipped with instead of
/// inheriting its successor's. The number is worth reading because nothing
/// else in the tree can drift from it: `deploy/deploy-pages.sh` refuses a
/// catalog whose live `proto` differs from its game's `crates/<id>-core`
/// constant, in both directions.
function releaseFrom(catalog, game, version) {
  const games = Array.isArray(catalog?.games) ? catalog.games : [];
  const entry = games.find((g) => g && g.id === game);
  const versions = Array.isArray(entry?.versions) ? entry.versions : [];
  return version
    ? versions.find((r) => r && r.v === version)
    : versions.find((r) => r && r.live === true);
}

export function protoFrom(catalog, game, version) {
  const found = releaseFrom(catalog, game, version);
  return typeof found?.proto === 'number' ? found.proto : null;
}

/// The decoded bundle size stamped into the served catalog by the deploy.
export function bytesFrom(catalog, game, version) {
  const bytes = releaseFrom(catalog, game, version)?.bytes;
  return Number.isSafeInteger(bytes) && bytes > 0 ? bytes : null;
}

/// A failure with a short code beside its message, so a page can tell a 404
/// from a browser that will not compile the bytes.
class LoadFailure extends Error {
  constructor(phase, code, message) {
    super(message);
    this.name = 'LoadFailure';
    this.phase = phase;
    this.code = code;
  }
}

const failureOf = (phase, error) => (error instanceof LoadFailure
  ? error
  : new LoadFailure(phase, 'error', (error && error.message) || String(error)));

function settings(options) {
  const o = options || {};
  if (!o.bundle) throw new LoadFailure('boot', 'no-bundle', 'emberLoad needs the game bundle path');
  const base = o.base
    || (typeof document === 'undefined' ? 'http://ember.invalid/' : document.baseURI);
  const stamp = o.stamp === undefined ? loaderStamp() : String(o.stamp || '');
  const root = new URL(o.root || './', SELF);
  return {
    game: String(o.game || ''),
    version: o.version ? String(o.version) : '',
    root,
    catalogUrl: new URL('games.json', root).href,
    bundleUrl: withStamp(new URL(o.bundle, base).href, stamp),
    wasmUrl: withStamp(new URL(o.wasm || bundleWasm(o.bundle), base).href, stamp),
    stamp,
    discover: o.discover !== false,
    proto: typeof o.proto === 'number' ? o.proto : null,
    override: String(o.override || '').trim(),
    stallMs: typeof o.stallMs === 'number' ? o.stallMs : 5000,
    catalogTimeoutMs: Number.isFinite(o.catalogTimeoutMs)
      ? Math.max(0, o.catalogTimeoutMs)
      : CATALOG_TIMEOUT_MS,
    onEvent: typeof o.onEvent === 'function' ? o.onEvent : () => {},
    start: typeof o.start === 'function' ? o.start : null,
    fetchImpl: o.fetchImpl || ((...args) => globalThis.fetch(...args)),
    importer: o.importer || ((url) => import(url)),
    loaderModule: o.loaderModule || null,
    // The host rule, injectable for the same reason `hosts.js` lets a caller
    // supply a socket: the picking is tested there against plain objects, and
    // this file's own tests are about the order it drives things in.
    hosts: o.hosts || { chooseHost, probeHost, rankHosts },
  };
}

/// The drive: the page's calls, in order, whether or not the rules have
/// arrived yet.
///
/// Each call records the clock at the moment it happened, so a replay after
/// the loader's own bundle lands produces the same numbers the live drive
/// would have. When the rules cannot be loaded at all the queue is dropped and
/// every later call is a no-op: a degraded load reports its own degradation
/// once and then says nothing it cannot measure.
function driver(o) {
  const queue = [];
  let model = null;
  let degraded = false;

  const deliver = (event, extra) => {
    if (!event) return;
    if (extra) Object.assign(event, extra);
    try {
      o.onEvent(event);
    } catch (e) {
      // A page's own handler must not be able to end a load.
      try { globalThis.console?.warn?.('loader.js: the page event handler threw', e); } catch {}
    }
  };

  const call = (method, at, args, extra) => deliver(model[method](at, ...args), extra);

  const drive = (method, args = [], extra = null) => {
    const at = clock();
    if (degraded) return;
    if (!model) {
      queue.push({ method, at, args, extra });
      return;
    }
    call(method, at, args, extra);
  };

  drive.ready = (loaded) => {
    model = loaded;
    for (const item of queue) call(item.method, item.at, item.args, item.extra);
    queue.length = 0;
  };

  drive.degrade = (detail) => {
    degraded = true;
    queue.length = 0;
    deliver({
      phase: 'boot',
      status: 'fail',
      elapsedMs: 0,
      phaseMs: 0,
      stalled: false,
      stalledMs: 0,
      reason: 'loader-unavailable',
      detail,
      text: `the loader could not start: ${detail} — loading without progress`,
    });
  };

  drive.degraded = () => degraded;
  return drive;
}

/// Bring up the rules. A failure here degrades the load; it never ends it.
async function rules(o, drive, t0) {
  try {
    const module = o.loaderModule
      || await o.importer(withStamp(new URL('pkg/ember_loader.js', SELF).href, o.stamp));
    if (module.default) {
      await module.default({
        module_or_path: withStamp(new URL('pkg/ember_loader_bg.wasm', SELF).href, o.stamp),
      });
    }
    drive.ready(new module.Loader(t0, o.stallMs));
    return true;
  } catch (e) {
    drive.degrade((e && e.message) || String(e));
    return false;
  }
}

/// One address, probed once and dressed as a ranked candidate, so that a
/// manual override and a ranked book have the same shape downstream.
async function onlyHost(hosts, url, game, proto) {
  const entry = { name: '', ws: url, version: '', commit: '', updated: '' };
  const probes = new Map([['', await hosts.probeHost(url, { proto })]]);
  return hosts.rankHosts([entry], { game, probes });
}

async function catalog(o) {
  let timer;
  try {
    const request = (async () => {
      try {
        const response = await o.fetchImpl(o.catalogUrl, { cache: 'no-store' });
        if (!response || !response.ok) return null;
        return await response.json();
      } catch {
        // The catalog is a convenience, not a requirement. Without it
        // discovery ranks unfiltered and response headers may still supply a
        // byte total.
        return null;
      }
    })();
    const timeout = new Promise((resolve) => {
      timer = setTimeout(() => resolve(null), o.catalogTimeoutMs);
    });
    return await Promise.race([request, timeout]);
  } finally {
    if (timer !== undefined) clearTimeout(timer);
  }
}

function hostPayload(result, proto, provisional) {
  return {
    host: result.chosen || null,
    candidates: result.candidates || [],
    wrongProto: result.wrongProto || [],
    proto: proto === null || proto === undefined ? null : proto,
    provisional: !!provisional,
  };
}

async function pick(o, proto) {
  return o.override
    ? await onlyHost(o.hosts, o.override, o.game, proto ?? 0)
    : await o.hosts.chooseHost(o.root.href, { game: o.game, proto });
}

/// Discovery, as its own track. It starts with the download and neither waits
/// for the other, and a failure here leaves the game loading: a page with no
/// server still plays offline practice and still has to say so.
async function discover(o, drive, protoPromise, rerank = false, previous = null) {
  drive(rerank ? 'rerank' : 'enter', ['host']);
  const proto = await protoPromise;
  const provisional = proto === null;
  let result;
  try {
    result = await pick(o, proto);
  } catch (e) {
    // A failed retry measured nothing new. Keep the ranking and book the first
    // pass did measure instead of turning a transient retry error into a new
    // empty result.
    const kept = previous || { chosen: null, candidates: [], wrongProto: [], book: null };
    const payload = hostPayload(kept, proto, provisional);
    drive('fail', ['host', 'discovery', (e && e.message) || String(e)], payload);
    return { ...payload, chosen: kept.chosen || null, book: kept.book || null };
  }
  const payload = hostPayload(result, proto, provisional);
  if (result.chosen) {
    drive('done', ['host'], payload);
  } else {
    const wrong = payload.wrongProto[0];
    drive(
      'fail',
      ['host', wrong ? 'wrong-protocol' : 'no-host',
        wrong
          ? `${wrong.name || 'a host'} speaks protocol ${wrong.proto} and this build speaks ${proto}`
          : 'no server answered'],
      payload,
    );
  }
  return { ...payload, chosen: result.chosen || null, book: result.book || null };
}

/// The bytes, counted as they arrive, compiled as they arrive.
///
/// The stream handed to the compiler is this loop's own, so the compile still
/// overlaps the download exactly as the browser's `instantiateStreaming` did
/// before there was anything to count. A browser that will not compile from a
/// constructed response falls back to compiling the whole buffer, which costs
/// one cached re-read and no correctness.
async function fetchAndCompile(o, drive, catalogPromise) {
  const response = await o.fetchImpl(o.wasmUrl, { credentials: 'same-origin' });
  if (!response || !response.ok) {
    const status = response ? `${response.status} ${response.statusText}` : 'no response';
    throw new LoadFailure('download', 'http', status);
  }
  // Fetch exposes decoded chunks. Start with a header in the same units when
  // there is one, without putting the stream behind the catalog request. The
  // deployed catalog can upgrade that total while the download is still open.
  const contentEncoding = response.headers?.get?.('content-encoding');
  const encoded = contentEncoding !== null && contentEncoding !== undefined;
  const headerBytes = encoded ? 0 : Number(response.headers?.get?.('content-length'));
  let declared = Number.isFinite(headerBytes) && headerBytes > 0 ? headerBytes : 0;
  let acceptingTotal = true;
  drive('total', [Number.isFinite(declared) && declared > 0 ? declared : 0]);
  const catalogTotal = catalogPromise.then((value) => {
    const found = bytesFrom(value, o.game, o.version);
    if (acceptingTotal && found && found !== declared) {
      declared = found;
      drive('total', [found]);
    }
  });
  catalogTotal.catch(() => {});

  const streaming = typeof WebAssembly.compileStreaming === 'function'
    && typeof ReadableStream === 'function'
    && !!response.body;
  if (!streaming) {
    const bytes = new Uint8Array(await response.arrayBuffer());
    drive('bytes', [bytes.byteLength]);
    acceptingTotal = false;
    drive('done', ['download']);
    drive('enter', ['compile']);
    const module = await WebAssembly.compile(bytes);
    drive('done', ['compile']);
    return module;
  }

  const reader = response.body.getReader();
  let streamedBytes = 0;
  let drained = false;
  const drain = () => {
    if (drained) return;
    drained = true;
    acceptingTotal = false;
    drive('done', ['download']);
  };
  const counted = new ReadableStream({
    async pull(controller) {
      const { done, value } = await reader.read();
      if (done) {
        drain();
        controller.close();
        return;
      }
      streamedBytes += value.byteLength;
      drive('bytes', [value.byteLength]);
      controller.enqueue(value);
    },
    cancel(reason) {
      return reader.cancel(reason);
    },
  });

  drive('enter', ['compile']);
  try {
    const module = await WebAssembly.compileStreaming(
      new Response(counted, { headers: { 'content-type': 'application/wasm' } }),
    );
    drive('done', ['compile']);
    return module;
  } catch (e) {
    if (e instanceof WebAssembly.CompileError) {
      throw new LoadFailure('compile', 'invalid-wasm', (e && e.message) || String(e));
    }
    try { await reader.cancel(e); } catch {}
    const again = await o.fetchImpl(o.wasmUrl, { credentials: 'same-origin' });
    if (!again || !again.ok) {
      throw new LoadFailure('compile', 'stream-compile', (e && e.message) || String(e));
    }
    const bytes = new Uint8Array(await again.arrayBuffer());
    const remaining = Math.max(0, bytes.byteLength - streamedBytes);
    if (remaining) drive('bytes', [remaining]);
    drain();
    const module = await WebAssembly.compile(bytes);
    drive('done', ['compile']);
    return module;
  }
}

async function bringUp(o, drive, gluePromise, catalogPromise) {
  drive('enter', ['download']);
  const module = await fetchAndCompile(o, drive, catalogPromise);
  const glue = await gluePromise;
  drive('enter', ['init']);
  try {
    await glue.default({ module_or_path: module });
  } catch (e) {
    throw failureOf('init', e);
  }
  let version = '';
  try {
    version = glue.package_version ? String(glue.package_version()) : '';
  } catch {
    version = '';
  }
  drive('done', ['init'], { version });
  return { glue, version };
}

/// The legacy path, for a load whose own rules could not be brought up: the
/// browser fetches and compiles the bundle itself, exactly as every page did
/// before this file existed.
async function withoutRules(o, gluePromise) {
  const glue = await gluePromise;
  await glue.default({ module_or_path: o.wasmUrl });
  let version = '';
  try {
    version = glue.package_version ? String(glue.package_version()) : '';
  } catch {
    version = '';
  }
  return { glue, version };
}

/// Load a game page: pick a host, fetch the bundle, compile it, start the
/// engine, and report every step.
///
/// Resolves with the game's own module namespace under `exports`, so a page
/// hands over exactly as it always did — `start_online(...)` with the chosen
/// host, `package_version()` for the label — and no game's Rust changes.
export async function emberLoad(options) {
  const o = settings(options);
  const t0 = clock();
  const drive = driver(o);
  drive('begin', []);

  // Every task starts here, in this tick. Nothing waits for the loader's own
  // bundle: the drive calls carry the clock, so they replay in order with
  // their real timestamps the moment the rules land.
  const booting = rules(o, drive, t0);
  const gluePromise = o.importer(o.bundleUrl).catch((e) => {
    throw new LoadFailure('init', 'glue', (e && e.message) || String(e));
  });
  // A download can fail before bringUp awaits the eager glue import. Mark the
  // rejection handled now while keeping the same rejecting promise for the
  // normal init path.
  gluePromise.catch(() => {});
  const catalogPromise = catalog(o);
  const protoPromise = o.proto === null && o.discover
    ? catalogPromise.then((value) => protoFrom(value, o.game, o.version))
    : Promise.resolve(o.proto);
  const hostPromise = o.discover ? discover(o, drive, protoPromise) : Promise.resolve(null);

  const ticker = setInterval(() => drive('tick', []), 250);
  let loaded;
  try {
    await booting;
    loaded = drive.degraded()
      ? await withoutRules(o, gluePromise)
      : await bringUp(o, drive, gluePromise, catalogPromise);
  } catch (e) {
    const failure = failureOf('download', e);
    drive('fail', [failure.phase, failure.code, failure.message]);
    throw failure;
  } finally {
    clearInterval(ticker);
  }

  let host = await hostPromise;
  // The early ranking shares the link with the bundle download. Once the
  // bundle is up, retry an empty result in the background on an idle link. An
  // unfiltered result is also re-ranked once the bundle can say which protocol
  // it speaks. Neither safety pass holds the playable engine behind another
  // probe window.
  let retryProto = null;
  let needsRetry = false;
  if (host) {
    let proto = host.proto;
    try {
      proto = loaded.glue.proto_version ? loaded.glue.proto_version() : proto;
    } catch {
      proto = host.proto;
    }
    if (!host.chosen || (host.provisional && typeof proto === 'number')) {
      retryProto = proto;
      needsRetry = true;
    }
  }

  if (o.start) {
    drive('enter', ['start']);
    try {
      await o.start(loaded.glue);
    } catch (e) {
      const failure = failureOf('start', e);
      drive('fail', ['start', failure.code, failure.message]);
      throw failure;
    }
    drive('done', ['start']);
  }
  drive('done', ['ready']);

  if (needsRetry && drive.degraded()) {
    host = await discover(
      o,
      drive,
      Promise.resolve(retryProto),
      true,
      host,
    );
    needsRetry = false;
  }

  const hostView = (settled) => ({
    host: settled ? settled.chosen : null,
    candidates: settled ? settled.candidates : [],
    wrongProto: settled ? settled.wrongProto : [],
    proto: settled ? settled.proto : o.proto,
    book: settled ? settled.book : null,
  });
  const first = hostView(host);
  // A timer puts every retry event after the caller has received the playable
  // first result. Pages normally consume those events; hostSettled is there
  // for a caller that explicitly wants to wait for the safety pass.
  const hostSettled = !needsRetry
    ? Promise.resolve(first)
    : new Promise((resolve) => setTimeout(resolve, 0))
      .then(() => discover(
        o,
        drive,
        Promise.resolve(retryProto),
        true,
        host,
      ))
      .then(hostView);
  hostSettled.catch(() => {});

  return {
    exports: loaded.glue,
    version: loaded.version,
    ...first,
    hostSettled,
    stamp: o.stamp,
    degraded: drive.degraded(),
  };
}
