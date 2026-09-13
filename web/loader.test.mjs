// Unit tests for web/loader.js — run with `node --test web/loader.test.mjs`.
//
// What is tested here is the ORDER the loader drives things in and the URLs it
// builds: the arithmetic and the phase rules live in `crates/ember-loader` and
// are tested natively there, and the host picking lives in `web/hosts.js` and
// is tested there. So the machine is a recording stand-in, and what these
// tests assert is the sequence a page would see — including the sequence it
// sees when the loader's own bundle never arrives.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';

import { bundleWasm, bytesFrom, emberLoad, loaderStamp, protoFrom, withStamp } from './loader.js';

/// The eight bytes of an empty wasm module: a real compile, no fixtures.
const EMPTY_WASM = new Uint8Array([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

/// A stand-in for the compiled rules that records how it was driven and hands
/// back the smallest event a page could act on.
function recorder() {
  const calls = [];
  const event = (phase, status, extra = {}) => ({
    phase, status, elapsedMs: 0, phaseMs: 0, text: `${phase}/${status}`, ...extra,
  });
  class Loader {
    constructor(t0, stallMs) {
      calls.push(['new', t0 === undefined ? null : 'clock', stallMs]);
    }
    begin() { calls.push(['begin']); return event('boot', 'begin'); }
    enter(at, phase) { calls.push(['enter', phase]); return event(phase, 'begin'); }
    rerank() { calls.push(['rerank', 'host']); return event('host', 'begin'); }
    done(at, phase) { calls.push(['done', phase]); return event(phase, 'done'); }
    total(at, total) { calls.push(['total', total]); return event('download', 'progress'); }
    bytes(at, bytes) { calls.push(['bytes', bytes]); return event('download', 'progress'); }
    tick() { return null; }
    fail(at, phase, reason, detail) {
      calls.push(['fail', phase, reason]);
      return event(phase, 'fail', { reason, detail });
    }
    get ended() { return false; }
  }
  return { calls, module: { Loader } };
}

function glueStub({ version = '31.0.0', proto = 24 } = {}) {
  const seen = [];
  return {
    seen,
    module: {
      default: async (arg) => { seen.push(arg); },
      package_version: () => version,
      proto_version: () => proto,
      start_online: () => {},
    },
  };
}

function bundleResponse(bytes = EMPTY_WASM, { length = true } = {}) {
  const headers = { 'content-type': 'application/wasm' };
  if (length) headers['content-length'] = String(bytes.byteLength);
  return new Response(bytes, { headers });
}

/// The options every test starts from: nothing real is reached.
function harness(over = {}) {
  const rules = recorder();
  const glue = glueStub(over.glue || {});
  const fetched = [];
  const events = [];
  const options = {
    game: 'arena',
    version: 'v31',
    bundle: './pkg/arena.js',
    base: 'https://example.invalid/games/arena/v31/',
    stamp: '7',
    discover: false,
    loaderModule: rules.module,
    importer: async () => glue.module,
    onEvent: (e) => events.push(`${e.phase}/${e.status}`),
    fetchImpl: async (url) => {
      fetched.push(String(url));
      return String(url).endsWith('games.json')
        ? new Response('missing', { status: 404 })
        : bundleResponse();
    },
    ...over.options,
  };
  return { rules, glue, fetched, events, options };
}

const phases = (calls) => calls
  .filter(([m]) => ['enter', 'rerank', 'done', 'fail', 'begin'].includes(m))
  .map(([m, phase]) => (m === 'begin' ? 'begin' : `${m}:${phase}`));

// ---- the small pure pieces ------------------------------------------------

test('loaderStamp reads the deploy stamp off this module own query', () => {
  assert.equal(loaderStamp('https://x.invalid/loader.js?v=2304'), '2304');
  assert.equal(loaderStamp('https://x.invalid/loader.js'), '');
  assert.equal(loaderStamp('not a url at all'), '');
});

test('withStamp respects a query that is already there', () => {
  assert.equal(withStamp('./pkg/arena.js', '9'), './pkg/arena.js?v=9');
  assert.equal(withStamp('./pkg/arena.js?a=1', '9'), './pkg/arena.js?a=1&v=9');
  assert.equal(withStamp('./pkg/arena.js', ''), './pkg/arena.js');
});

test('bundleWasm names the binary beside the glue', () => {
  assert.equal(bundleWasm('./pkg/arena.js'), './pkg/arena_bg.wasm');
  assert.equal(bundleWasm('./pkg/what_is_this.js'), './pkg/what_is_this_bg.wasm');
});

// The page's own version key, not `live`: the day arena v32 goes live, the
// v31 page is still served and must keep speaking protocol 24.
test('protoFrom reads the entry for the page own version', () => {
  const catalog = {
    games: [
      {
        id: 'arena',
        versions: [
          { v: 'v32', live: true, proto: 25 },
          { v: 'v31', live: false, proto: 24 },
          { v: 'v11', live: false },
        ],
      },
      { id: 'julibrot', versions: [{ v: 'v1', live: true }] },
    ],
  };
  assert.equal(protoFrom(catalog, 'arena', 'v31'), 24);
  assert.equal(protoFrom(catalog, 'arena', 'v32'), 25);
  assert.equal(protoFrom(catalog, 'arena', ''), 25, 'with no version key the live entry answers');
  assert.equal(protoFrom(catalog, 'arena', 'v11'), null, 'an entry with no proto has none');
  assert.equal(protoFrom(catalog, 'arena', 'v99'), null);
  assert.equal(protoFrom(catalog, 'kings', 'v1'), null);
  assert.equal(protoFrom(null, 'arena', 'v31'), null);
  assert.equal(protoFrom({ games: 'nonsense' }, 'arena', 'v31'), null);
});

test('bytesFrom reads only a positive safe decoded bundle size', () => {
  const catalog = {
    games: [{
      id: 'arena',
      versions: [
        { v: 'v32', live: true, bytes: 44_000_000 },
        { v: 'v31', live: false, bytes: 43_000_000 },
      ],
    }],
  };
  assert.equal(bytesFrom(catalog, 'arena', 'v31'), 43_000_000);
  assert.equal(bytesFrom(catalog, 'arena', ''), 44_000_000);
  assert.equal(bytesFrom({ games: [{ id: 'arena', versions: [{ v: 'v31', bytes: -1 }] }] }, 'arena', 'v31'), null);
  assert.equal(bytesFrom(null, 'arena', 'v31'), null);
});

// ---- the load -------------------------------------------------------------

test('a load drives every phase in order and hands the game back', async () => {
  const h = harness();
  const result = await emberLoad(h.options);
  // The compile begins at the first byte and finishes after the last: the
  // overlap is the reason these are two phases rather than one spinner.
  assert.deepEqual(phases(h.rules.calls), [
    'begin',
    'enter:download',
    'enter:compile',
    'done:download',
    'done:compile',
    'enter:init',
    'done:init',
    'done:ready',
  ]);
  assert.equal(result.exports, h.glue.module);
  assert.equal(result.version, '31.0.0');
  assert.equal(result.degraded, false);
  assert.equal(result.host, null, 'a page that asked for no discovery gets no host');
  assert.deepEqual(h.glue.seen.map((a) => typeof a.module_or_path), ['object']);
  assert.ok(h.glue.seen[0].module_or_path instanceof WebAssembly.Module);
});

test('the bundle URLs carry the page own deploy stamp', async () => {
  const h = harness();
  await emberLoad(h.options);
  assert.deepEqual(h.fetched, [
    new URL('./games.json', import.meta.url).href,
    'https://example.invalid/games/arena/v31/pkg/arena_bg.wasm?v=7',
  ]);
});

test('an unencoded response length is the fallback decoded total', async () => {
  const counted = new Response(EMPTY_WASM, { headers: { 'content-type': 'application/wasm', 'content-length': '8' } });
  const h = harness({
    options: {
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response('missing', { status: 404 })
        : counted),
    },
  });
  await emberLoad(h.options);
  assert.deepEqual(h.rules.calls.find(([m]) => m === 'total'), ['total', 8]);
});

test('without catalog bytes an encoded response has no decoded total', async () => {
  const streamed = new Response(
    new ReadableStream({ start(c) { c.enqueue(EMPTY_WASM); c.close(); } }),
    { headers: { 'content-type': 'application/wasm', 'content-length': '4', 'content-encoding': 'gzip' } },
  );
  const g = harness({
    options: {
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response('missing', { status: 404 })
        : streamed),
    },
  });
  await emberLoad(g.options);
  assert.deepEqual(g.rules.calls.find(([m]) => m === 'total'), ['total', 0]);
});

test('catalog bytes provide the decoded total for an encoded response', async () => {
  let finishBundleResponse;
  const waitingBundle = new Promise((resolve) => { finishBundleResponse = resolve; });
  const h = harness({
    options: {
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response(JSON.stringify({
          games: [{ id: 'arena', versions: [{ v: 'v31', live: true, bytes: 8 }] }],
        }))
        : waitingBundle),
    },
  });
  const loading = emberLoad(h.options);
  await new Promise((resolve) => setImmediate(resolve));
  finishBundleResponse(new Response(EMPTY_WASM, {
    headers: {
      'content-type': 'application/wasm',
      'content-length': '4',
      'content-encoding': 'gzip',
    },
  }));
  await loading;
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'total'), [
    ['total', 0],
    ['total', 8],
  ]);
});

test('an unencoded header reports progress before a later catalog upgrades it', async () => {
  let finishCatalog;
  let finishBundle;
  const catalogResponse = new Promise((resolve) => { finishCatalog = resolve; });
  const bundle = new ReadableStream({
    start(controller) {
      controller.enqueue(EMPTY_WASM);
      finishBundle = () => controller.close();
    },
  });
  const h = harness({
    options: {
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? catalogResponse
        : new Response(bundle, {
          headers: { 'content-type': 'application/wasm', 'content-length': '8' },
        })),
    },
  });
  const loading = emberLoad(h.options);
  while (!h.rules.calls.some(([method]) => method === 'total')) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.deepEqual(h.rules.calls.filter(([method]) => method === 'total'), [['total', 8]]);
  finishCatalog(new Response(JSON.stringify({
    games: [{ id: 'arena', versions: [{ v: 'v31', live: true, bytes: 16 }] }],
  })));
  while (h.rules.calls.filter(([method]) => method === 'total').length < 2) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.deepEqual(h.rules.calls.filter(([method]) => method === 'total'), [
    ['total', 8],
    ['total', 16],
  ]);
  finishBundle();
  await loading;
});

test('a silent catalog is bounded and discovery continues without it', async () => {
  const fleet = withHosts();
  const h = harness({
    options: {
      discover: true,
      catalogTimeoutMs: 5,
      hosts: fleet.hosts,
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Promise(() => {})
        : bundleResponse()),
    },
  });
  const result = await emberLoad(h.options);
  const settled = await result.hostSettled;
  assert.deepEqual(fleet.asked, [null, 24]);
  assert.equal(result.proto, 24);
  assert.equal(settled.host, fleet.chosen);
});

test('every byte is counted', async () => {
  const h = harness({
    options: {
      fetchImpl: async (url) => {
        if (String(url).endsWith('games.json')) return new Response('missing', { status: 404 });
        const halves = new ReadableStream({
          start(c) {
            c.enqueue(EMPTY_WASM.slice(0, 4));
            c.enqueue(EMPTY_WASM.slice(4));
            c.close();
          },
        });
        return new Response(halves, {
          headers: { 'content-type': 'application/wasm', 'content-length': '8' },
        });
      },
    },
  });
  await emberLoad(h.options);
  const counted = h.rules.calls.filter(([m]) => m === 'bytes').map(([, n]) => n);
  assert.equal(counted.reduce((a, b) => a + b, 0), 8);
});

test('a bundle that is not there fails the download and says so', async () => {
  const h = harness({ options: { fetchImpl: async () => new Response('nope', { status: 404 }) } });
  await assert.rejects(emberLoad(h.options), (e) => {
    assert.equal(e.phase, 'download');
    assert.equal(e.code, 'http');
    return true;
  });
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'fail'), [['fail', 'download', 'http']]);
});

test('bytes that are not a module fail the compile without a retry loop', async () => {
  let fetches = 0;
  const h = harness({
    options: {
      fetchImpl: async (url) => {
        if (String(url).endsWith('games.json')) return new Response('missing', { status: 404 });
        fetches += 1;
        return new Response(new Uint8Array([1, 2, 3, 4]), {
          headers: { 'content-type': 'application/wasm', 'content-length': '4' },
        });
      },
    },
  });
  await assert.rejects(emberLoad(h.options), (e) => e.phase === 'compile');
  assert.ok(fetches <= 2, `one re-read at most, saw ${fetches}`);
});

test('the buffered compile fallback cancels the first stream and counts the retry', async () => {
  const original = WebAssembly.compileStreaming;
  let cancelled = false;
  let fetches = 0;
  const first = new ReadableStream({
    start(controller) { controller.enqueue(EMPTY_WASM); },
    cancel() { cancelled = true; },
  });
  const h = harness({
    options: {
      fetchImpl: async (url) => {
        if (String(url).endsWith('games.json')) return new Response('missing', { status: 404 });
        fetches += 1;
        return fetches === 1
          ? new Response(first, { headers: { 'content-type': 'application/wasm' } })
          : bundleResponse();
      },
    },
  });
  WebAssembly.compileStreaming = async () => { throw new TypeError('constructed response unsupported'); };
  try {
    await emberLoad(h.options);
  } finally {
    WebAssembly.compileStreaming = original;
  }
  const counted = h.rules.calls.filter(([m]) => m === 'bytes').map(([, n]) => n);
  assert.equal(counted.reduce((a, b) => a + b, 0), EMPTY_WASM.byteLength);
  assert.equal(fetches, 2);
  assert.equal(cancelled, true);
});

test('an engine init error fails init rather than download', async () => {
  const h = harness();
  h.options.importer = async () => ({
    ...h.glue.module,
    default: async () => { throw new Error('engine refused the module'); },
  });
  await assert.rejects(emberLoad(h.options), (error) => error.phase === 'init');
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'fail'), [['fail', 'init', 'error']]);
});

test('an eager glue rejection is handled when the download fails first', async () => {
  const unhandled = [];
  const listener = (reason) => unhandled.push(reason);
  process.on('unhandledRejection', listener);
  const h = harness({
    options: {
      importer: async () => { throw new Error('glue missing'); },
      fetchImpl: async () => new Response('gone', { status: 404 }),
    },
  });
  try {
    await assert.rejects(emberLoad(h.options), (error) => error.phase === 'download');
    await new Promise((resolve) => setImmediate(resolve));
    assert.deepEqual(unhandled, []);
  } finally {
    process.off('unhandledRejection', listener);
  }
});

// The whole point of the fallback: a page whose loader will not load is still
// a page that plays.
test('a loader that cannot start says so once and loads the game anyway', async () => {
  const h = harness({
    options: {
      loaderModule: null,
      importer: async (url) => {
        if (String(url).includes('ember_loader')) throw new Error('404');
        return h.glue.module;
      },
    },
  });
  const result = await emberLoad(h.options);
  assert.deepEqual(h.events, ['boot/fail']);
  assert.equal(h.rules.calls.length, 0, 'nothing is driven once the rules are gone');
  assert.equal(result.degraded, true);
  assert.equal(result.exports, h.glue.module);
  assert.equal(result.version, '31.0.0');
  assert.equal(
    h.glue.seen[0].module_or_path,
    'https://example.invalid/games/arena/v31/pkg/arena_bg.wasm?v=7',
    'the browser fetches the bundle itself, exactly as it did before',
  );
});

test('a degraded load awaits the retry because no event can deliver it later', async () => {
  const fleet = withHosts();
  let passes = 0;
  fleet.hosts.chooseHost = async (root, options) => {
    fleet.asked.push(options.proto);
    passes += 1;
    return passes === 1
      ? { chosen: null, candidates: [], wrongProto: [], book: { v: '7' } }
      : { chosen: fleet.chosen, candidates: [fleet.chosen], wrongProto: [], book: { v: '8' } };
  };
  const h = harness({
    options: {
      loaderModule: null,
      discover: true,
      proto: 24,
      hosts: fleet.hosts,
      importer: async (url) => {
        if (String(url).includes('ember_loader')) throw new Error('404');
        return h.glue.module;
      },
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options);
  assert.deepEqual(fleet.asked, [24, 24]);
  assert.equal(result.degraded, true);
  assert.equal(result.host, fleet.chosen);
  assert.deepEqual(result.book, { v: '8' });
  assert.deepEqual(await result.hostSettled, {
    host: fleet.chosen,
    candidates: [fleet.chosen],
    wrongProto: [],
    proto: 24,
    book: { v: '8' },
  });
  assert.deepEqual(h.events, ['boot/fail']);
});

test('a page handler that throws does not end the load', async () => {
  const h = harness({ options: { onEvent: () => { throw new Error('render'); } } });
  const result = await emberLoad(h.options);
  assert.equal(result.exports, h.glue.module);
});

// ---- discovery ------------------------------------------------------------

function withHosts(over = {}, hosts = {}) {
  const chosen = { name: 'sokol', url: 'wss://sokol.invalid', proto: 24 };
  const asked = [];
  return {
    asked,
    chosen,
    hosts: {
      chooseHost: async (root, opts) => {
        asked.push(opts.proto);
        return { chosen, candidates: [chosen], wrongProto: [], book: { v: '7' } };
      },
      probeHost: async () => ({ ok: true, rttMs: 5 }),
      rankHosts: () => ({ chosen, candidates: [chosen], wrongProto: [] }),
      ...hosts,
    },
    ...over,
  };
}

test('discovery runs beside the download and settles before the bundle does', async () => {
  const fleet = withHosts();
  const h = harness({
    options: {
      discover: true,
      hosts: fleet.hosts,
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response(JSON.stringify({ games: [{ id: 'arena', versions: [{ v: 'v31', live: true, proto: 24 }] }] }))
        : bundleResponse()),
    },
  });
  const result = await emberLoad(h.options);
  assert.deepEqual(fleet.asked, [24], 'the catalog protocol is what the ranking filtered on');
  assert.equal(result.host, fleet.chosen);
  assert.deepEqual(result.candidates, [fleet.chosen]);
  assert.equal(result.proto, 24);
  assert.deepEqual(result.book, { v: '7' });
  const order = phases(h.rules.calls);
  assert.ok(order.indexOf('done:host') < order.indexOf('done:init'), order.join(' '));
  assert.ok(order.indexOf('enter:host') < order.indexOf('done:download'), order.join(' '));
});

test('first discovery may be empty and the post-bundle retry finds the host', async () => {
  const fleet = withHosts();
  let passes = 0;
  let finishRetry;
  fleet.hosts.chooseHost = async (root, opts) => {
    fleet.asked.push(opts.proto);
    passes += 1;
    if (passes === 1) return { chosen: null, candidates: [], wrongProto: [], book: { v: '7' } };
    return await new Promise((resolve) => {
      finishRetry = () => resolve({
        chosen: fleet.chosen,
        candidates: [fleet.chosen],
        wrongProto: [],
        book: { v: '8' },
      });
    });
  };
  const timeline = [];
  const h = harness({
    options: {
      discover: true,
      proto: 24,
      hosts: fleet.hosts,
      onEvent: (event) => timeline.push(`${event.phase}/${event.status}`),
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options).then((value) => {
    timeline.push('resolved');
    return value;
  });
  assert.equal(result.host, null, 'the playable result carries the completed first pass');
  assert.deepEqual(result.book, { v: '7' });
  while (!finishRetry) await new Promise((resolve) => setTimeout(resolve, 0));
  finishRetry();
  const settled = await result.hostSettled;
  assert.deepEqual(fleet.asked, [24, 24]);
  assert.equal(settled.host, fleet.chosen);
  assert.deepEqual(settled.book, { v: '8' });
  assert.ok(timeline.indexOf('resolved') < timeline.lastIndexOf('host/done'), timeline.join(' '));
  assert.deepEqual(phases(h.rules.calls).filter((phase) => phase.endsWith(':host')), [
    'enter:host',
    'fail:host',
    'rerank:host',
    'done:host',
  ]);
  assert.ok(
    phases(h.rules.calls).indexOf('done:ready') < phases(h.rules.calls).indexOf('rerank:host'),
    phases(h.rules.calls).join(' '),
  );
});

test('a retry error keeps the first ranking and book', async () => {
  const fleet = withHosts();
  let passes = 0;
  fleet.hosts.chooseHost = async () => {
    passes += 1;
    if (passes === 1) {
      return { chosen: fleet.chosen, candidates: [fleet.chosen], wrongProto: [], book: { v: '7' } };
    }
    throw new Error('mirror disappeared');
  };
  const h = harness({
    options: {
      discover: true,
      hosts: fleet.hosts,
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options);
  const settled = await result.hostSettled;
  assert.equal(settled.host, fleet.chosen);
  assert.deepEqual(settled.candidates, [fleet.chosen]);
  assert.deepEqual(settled.book, { v: '7' });
});

test('no host answering twice leaves the game loading', async () => {
  const fleet = withHosts({}, {
    chooseHost: async () => ({ chosen: null, candidates: [], wrongProto: [] }),
  });
  const h = harness({
    options: {
      discover: true,
      proto: 24,
      hosts: fleet.hosts,
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options);
  await result.hostSettled;
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'fail'), [
    ['fail', 'host', 'no-host'],
    ['fail', 'host', 'no-host'],
  ]);
  assert.equal(result.host, null);
  assert.equal(result.exports, h.glue.module, 'the game still loaded');
  assert.ok(phases(h.rules.calls).includes('done:ready'));
});

test('a host on another protocol is reported as that, not as silence', async () => {
  const fleet = withHosts({}, {
    chooseHost: async () => ({
      chosen: null,
      candidates: [],
      wrongProto: [{ name: 'sokol', proto: 23 }],
    }),
  });
  const h = harness({
    options: { discover: true, proto: 24, hosts: fleet.hosts, fetchImpl: async () => bundleResponse() },
  });
  const result = await emberLoad(h.options);
  await result.hostSettled;
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'fail'), [
    ['fail', 'host', 'wrong-protocol'],
    ['fail', 'host', 'wrong-protocol'],
  ]);
});

// Without the catalog the first ranking has no protocol filter, so the chip is
// provisional and the bundle's own answer settles it.
test('an unreadable catalog ranks unfiltered and re-ranks once the bundle can say', async () => {
  const fleet = withHosts();
  const h = harness({
    options: {
      discover: true,
      hosts: fleet.hosts,
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response('gone', { status: 500 })
        : bundleResponse()),
    },
  });
  const result = await emberLoad(h.options);
  const settled = await result.hostSettled;
  assert.deepEqual(fleet.asked, [null, 24], 'unfiltered first, then the bundle protocol');
  assert.equal(result.proto, 24);
  assert.equal(settled.proto, 24);
  const hostPhases = phases(h.rules.calls).filter((p) => p.endsWith(':host'));
  assert.deepEqual(hostPhases, ['enter:host', 'done:host', 'rerank:host', 'done:host']);
});

const realGlueUrl = new URL('./pkg/ember_loader.js', import.meta.url);
const realWasmUrl = new URL('./pkg/ember_loader_bg.wasm', import.meta.url);
const realLoaderBuilt = existsSync(realGlueUrl) && existsSync(realWasmUrl);

test('the generated Loader accepts the retry sequence used by emberLoad', {
  skip: realLoaderBuilt ? false : 'build ember-loader and run wasm-bindgen first',
}, async () => {
  const real = await import(realGlueUrl.href);
  const module = await WebAssembly.compile(await readFile(realWasmUrl));
  await real.default({ module_or_path: module });
  const fleet = withHosts();
  let passes = 0;
  fleet.hosts.chooseHost = async () => {
    passes += 1;
    return passes === 1
      ? { chosen: null, candidates: [], wrongProto: [] }
      : { chosen: fleet.chosen, candidates: [fleet.chosen], wrongProto: [] };
  };
  const events = [];
  const h = harness({
    options: {
      loaderModule: real,
      discover: true,
      proto: 24,
      hosts: fleet.hosts,
      onEvent: (event) => events.push(event),
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options);
  const settled = await result.hostSettled;
  assert.equal(result.host, null);
  assert.equal(settled.host, fleet.chosen);
  assert.deepEqual(
    events.filter((event) => event.phase === 'host').map((event) => event.status),
    ['begin', 'fail', 'begin', 'done'],
  );
  assert.equal(events.some((event) => event.reason === 'out-of-order'), false);
});

test('the generated Loader omits fields that do not apply', {
  skip: realLoaderBuilt ? false : 'build ember-loader and run wasm-bindgen first',
}, async () => {
  const real = await import(realGlueUrl.href);
  const module = await WebAssembly.compile(await readFile(realWasmUrl));
  await real.default({ module_or_path: module });
  const loader = new real.Loader(0, 5_000);
  loader.begin(0);
  const download = loader.enter(1, 'download');
  assert.equal(download.loaded, 0);
  assert.equal('total' in download, false);
  assert.equal('percent' in download, false);
  assert.equal('etaMs' in download, false);
  const host = loader.enter(2, 'host');
  assert.equal('loaded' in host, false);
});

test('all multiplayer pages connect late host events to their online controls', async () => {
  const [arena, fire, kings, league] = await Promise.all([
    readFile(new URL('./games/arena/v31/index.html', import.meta.url), 'utf8'),
    readFile(new URL('./games/fire/v2/race.js', import.meta.url), 'utf8'),
    readFile(new URL('./games/kings/v1/index.html', import.meta.url), 'utf8'),
    readFile(new URL('./games/league/v4/ui.js', import.meta.url), 'utf8'),
  ]);
  assert.ok(arena.includes('chosen = e.host || null;') && arena.includes('const wanted = target || chosen;'));
  assert.ok(arena.includes("if (typeof e.proto === 'number') PROTO = e.proto;"));
  assert.ok(fire.includes('chosen = e.host || null;') && fire.includes("$('btn-online').disabled = false;"));
  assert.ok(fire.includes("if (typeof e.proto === 'number') PROTO = e.proto;"));
  assert.ok(kings.includes('chosen = e.host || null;') && kings.includes("const kingsWs = () => (chosen ? chosen.url : '');"));
  assert.ok(league.includes('adoptHosts({') && league.includes("for (const id of ['btn-create', 'btn-quick'])"));
  assert.ok(league.includes("if (typeof e.proto === 'number') PROTO = e.proto;"));
});

// The deploy proves a LIVE entry's protocol equals its crate constant, but a
// page frozen by a later release keeps this code and loses that proof.
test('a catalog that disagrees with the bundle is corrected by the bundle', async () => {
  const fleet = withHosts();
  const h = harness({
    glue: { proto: 25 },
    options: {
      discover: true,
      hosts: fleet.hosts,
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response(JSON.stringify({ games: [{ id: 'arena', versions: [{ v: 'v31', live: true, proto: 24 }] }] }))
        : bundleResponse()),
    },
  });
  const result = await emberLoad(h.options);
  const settled = await result.hostSettled;
  assert.deepEqual(fleet.asked, [24, 25], 'the catalog ranked first, the bundle corrected it');
  assert.equal(result.proto, 25);
  assert.equal(settled.proto, 25);
});

test('a catalog that agrees with the bundle is ranked once', async () => {
  const fleet = withHosts();
  const h = harness({
    glue: { proto: 24 },
    options: {
      discover: true,
      hosts: fleet.hosts,
      fetchImpl: async (url) => (String(url).endsWith('games.json')
        ? new Response(JSON.stringify({ games: [{ id: 'arena', versions: [{ v: 'v31', live: true, proto: 24 }] }] }))
        : bundleResponse()),
    },
  });
  const result = await emberLoad(h.options);
  await result.hostSettled;
  assert.deepEqual(fleet.asked, [24]);
});

test('a manual override replaces the book with one probed address', async () => {
  const probed = [];
  const fleet = withHosts({}, {
    chooseHost: async () => { throw new Error('the override must not consult the book'); },
    probeHost: async (url) => { probed.push(url); return { ok: true, rttMs: 3 }; },
  });
  const h = harness({
    options: {
      discover: true,
      proto: 24,
      override: 'wss://typed.invalid',
      hosts: fleet.hosts,
      fetchImpl: async () => bundleResponse(),
    },
  });
  const result = await emberLoad(h.options);
  assert.deepEqual(probed, ['wss://typed.invalid']);
  assert.equal(result.host, fleet.chosen);
});

// ---- handing over ---------------------------------------------------------

test('a start hook runs before ready and reports its own phase', async () => {
  const started = [];
  const h = harness({ options: { start: async (exports) => { started.push(exports); } } });
  await emberLoad(h.options);
  assert.deepEqual(started.length, 1);
  const order = phases(h.rules.calls);
  assert.deepEqual(order.slice(-3), ['enter:start', 'done:start', 'done:ready']);
});

test('a start hook that throws fails the start phase', async () => {
  const h = harness({ options: { start: async () => { throw new Error('no canvas'); } } });
  await assert.rejects(emberLoad(h.options), (e) => e.phase === 'start');
  assert.deepEqual(h.rules.calls.filter(([m]) => m === 'fail'), [['fail', 'start', 'error']]);
});

test('a page with no bundle path is refused before anything is fetched', async () => {
  await assert.rejects(emberLoad({ game: 'arena' }), (e) => e.code === 'no-bundle');
});
