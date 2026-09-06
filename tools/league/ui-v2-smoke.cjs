// Actual v2 page + WASM settings/HUD checks. Input stays inside headless Edge
// as synthetic DOM events; preview owns and closes only its temporary services.
'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { startPreview } = require('./preview.cjs');

const root = path.resolve(__dirname, '../..');
const out = path.join(root, 'target/league-ui-v2');
const storageKey = 'ember-league-v2-keys';
const defaults = {
  q: 'KeyQ', w: 'KeyW', e: 'KeyE', r: 'KeyR', d: 'KeyD', f: 'KeyF',
  item1: 'Digit1', item2: 'Digit2', item3: 'Digit3', item4: 'Digit4',
  item5: 'Digit5', item6: 'Digit6', stop: 'KeyS', shop: 'KeyB',
};
const started = Date.now();
const report = { checks: [], failures: [], errors: [], console: [], screenshots: [], assets: [] };
let browser, preview;

function check(ok, name) {
  assert(ok, name);
  report.checks.push(name);
  console.log('PASS ' + name);
}

const state = page => page.evaluate(() => window.qaState());
const bindings = page => page.evaluate(() => JSON.parse(window.qaWasm.bindings_json()));
const stored = page => page.evaluate(key => JSON.parse(localStorage.getItem(key)), storageKey);
const command = (page, value) => page.evaluate(value => window.qaWasm.cmd_json(JSON.stringify(value)), value);
const distance = (a, b) => Math.hypot(a.me.x - b.me.x, a.me.z - b.me.z);

async function screenshot(page, name) {
  const file = path.join(out, name + '.png');
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: file });
  report.screenshots.push({ name, file, viewport: page.viewportSize() });
}

async function test(name, page, action) {
  try { await action(); }
  catch (error) {
    report.failures.push({ name, error: error.stack });
    console.error('FAIL ' + name + ': ' + error.message);
    try {
      await screenshot(page, 'failure-' + name);
      report.failures.at(-1).state = await state(page);
    } catch (diagnostic) { report.failures.at(-1).diagnostic = String(diagnostic); }
  }
}

async function attach(page) {
  await page.waitForFunction(() => ['ready', 'failed'].includes(document.body.dataset.leagueBoot));
  await page.evaluate(async () => {
    window.qaWasm = await window.leagueReady;
    if (!window.qaWasm) throw new Error(document.getElementById('engine-note').textContent);
    window.qaState = () => JSON.parse(window.qaWasm.state_json());
  });
}

async function newPage({ width = 1440, height = 1000, initialStorage } = {}) {
  const page = await browser.newPage({ viewport: { width, height } });
  page.setDefaultTimeout(20000);
  page.on('pageerror', error => report.errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') report.console.push(message.text());
  });
  await page.addInitScript(({ key, initialStorage }) => {
    window.focus = () => {};
    Element.prototype.setPointerCapture = () => {};
    if (initialStorage !== undefined && !sessionStorage.getItem('qa-storage-seeded')) {
      localStorage.setItem(key, initialStorage);
      sessionStorage.setItem('qa-storage-seeded', '1');
    }
    window.qaStorageWrites = [];
    const set = Storage.prototype.setItem;
    Storage.prototype.setItem = function (name, value) {
      if (this === localStorage && name === key) window.qaStorageWrites.push(value);
      return set.call(this, name, value);
    };
  }, { key: storageKey, initialStorage });
  await page.goto(preview.origin + '/games/league/v2/', { waitUntil: 'load' });
  await attach(page);
  return page;
}

async function click(page, selector) {
  await page.waitForFunction(selector => {
    const element = document.querySelector(selector);
    return element && !element.disabled && element.getClientRects().length &&
      getComputedStyle(element).visibility !== 'hidden';
  }, selector);
  await page.evaluate(selector => {
    const element = document.querySelector(selector);
    element.scrollIntoView({ block: 'center' });
    element.focus({ preventScroll: true });
    element.click();
  }, selector);
}

async function visible(page, selector, expected = true) {
  await page.waitForFunction(({ selector, expected }) => {
    const element = document.querySelector(selector);
    return Boolean(element?.getClientRects().length) === expected;
  }, { selector, expected });
}

async function keyboard(page, code, selector = '#ember-root canvas', type = 'keydown') {
  return page.evaluate(({ code, selector, type }) => {
    const target = document.querySelector(selector);
    const event = new KeyboardEvent(type, {
      code, key: code.startsWith('Key') ? code.slice(3).toLowerCase() : code,
      bubbles: true, cancelable: true,
    });
    target.dispatchEvent(event);
    return { prevented: event.defaultPrevented, active: document.activeElement?.id };
  }, { code, selector, type });
}

async function capture(page, action, code) {
  await click(page, `#key-rows .kset[data-act="${action}"]`);
  await keyboard(page, code, '#keys');
  await keyboard(page, code, '#keys', 'keyup');
}

async function advance(page, seconds = 0.25) {
  const before = await state(page);
  assert(before.phase === 'live' && before.me?.alive, 'Practice champion must be alive');
  await page.waitForFunction(target => window.qaState().secs >= target, before.secs + seconds);
  const after = await state(page);
  assert(after.phase === 'live' && after.me?.alive, 'Practice ended during UI input checks');
  return after;
}

async function key(page, code, selector = '#ember-root canvas') {
  await keyboard(page, code, selector);
  await advance(page, 0.15);
  await keyboard(page, code, selector, 'keyup');
  return advance(page, 0.1);
}

async function pointer(page, clickGround = false) {
  await page.evaluate(clickGround => {
    const canvas = document.querySelector('#ember-root canvas'), rect = canvas.getBoundingClientRect();
    const init = {
      bubbles: true, cancelable: true, pointerType: 'mouse', pointerId: 1, isPrimary: true,
      clientX: rect.left + rect.width * 0.74, clientY: rect.top + rect.height * 0.47,
      button: -1, buttons: 0,
    };
    const event = new PointerEvent('pointermove', init);
    Object.defineProperty(event, 'getCoalescedEvents', { value: () => [event] });
    canvas.dispatchEvent(event);
    if (clickGround) canvas.dispatchEvent(new PointerEvent('pointerdown', { ...init, button: 2, buttons: 2 }));
  }, clickGround);
  await advance(page, 0.15);
  if (clickGround) {
    await page.evaluate(() => document.querySelector('#ember-root canvas').dispatchEvent(
      new PointerEvent('pointerup', { bubbles: true, pointerType: 'mouse', pointerId: 1, button: 2, buttons: 0 })));
  }
  return advance(page, 0.1);
}

async function assertMap(page, expected, name) {
  assert.deepEqual(await bindings(page), expected, name + ': engine differs');
  assert.deepEqual(await stored(page), expected, name + ': saved map differs');
  const labels = await page.evaluate(() => ({
    q: document.querySelector('#abils [data-abil="0"] .key').textContent,
    w: document.querySelector('#abils [data-abil="1"] .key').textContent,
    shop: document.querySelector('[data-key-label="shop"]').textContent,
  }));
  assert.deepEqual(labels, { q: expected.q.slice(3), w: expected.w.slice(3), shop: expected.shop.slice(3) });
  check(true, name + ': engine, saved map and HUD labels agree');
}

async function renderedIcons(page, selector, minimum, name) {
  await page.waitForFunction(({ selector, minimum }) => {
    const icons = [...document.querySelectorAll(selector)];
    return icons.length >= minimum && icons.every(icon => {
      const box = icon.getBBox();
      return box.width > 0 && box.height > 0;
    });
  }, { selector, minimum });
  check(true, name + ': SVG symbols produce nonempty rendered geometry');
}

async function desktop(page) {
  await page.waitForFunction(() => document.querySelector('#arena-art img')?.naturalWidth > 0);
  check(true, 'menu artwork loads as an actual image');
  await screenshot(page, 'menu-desktop');
  check(!(await keyboard(page, 'Tab', '#handle')).prevented, 'menu text field does not block Tab');
  await click(page, '#btn-keys');
  await visible(page, '#keys');
  check(await page.locator('#key-rows .krow').count() === 14, 'settings has all 14 binding rows');
  await page.evaluate(() => document.querySelector('#key-rows .kset').focus());
  check(!(await keyboard(page, 'Tab', '#key-rows .kset')).prevented,
    'Tab is not blocked between settings controls outside key capture');
  await capture(page, 'q', 'KeyA');
  await capture(page, 'w', 'KeyZ');
  let expected = { ...defaults, q: 'KeyA', w: 'KeyZ' };
  await assertMap(page, expected, 'successive Q and W changes preserve both bindings');
  await capture(page, 'shop', 'KeyC');
  expected = { ...expected, shop: 'KeyC' };
  await assertMap(page, expected, 'shop change preserves both ability bindings');
  await screenshot(page, 'settings-desktop');

  const writes = await page.evaluate(() => window.qaStorageWrites.length);
  await capture(page, 'q', 'KeyZ');
  await visible(page, '#key-conflict');
  await assertMap(page, expected, 'conflict proposal leaves bindings untouched');
  check(await page.evaluate(() => window.qaStorageWrites.length) === writes,
    'conflict proposal does not persist a partial map');
  await click(page, '#btn-conflict-take');
  expected = { ...expected, q: 'KeyZ', w: 'KeyA' };
  await assertMap(page, expected, 'conflict swap preserves both keys atomically');
  check(await page.evaluate(() => window.qaStorageWrites.length) === writes + 1,
    'accepted conflict swap persists one complete replacement');

  for (const code of ['ControlLeft', 'F11', 'NotAKey']) {
    await capture(page, 'q', code);
    check(await page.evaluate(() => document.getElementById('key-status').classList.contains('bad') &&
      document.getElementById('key-status').textContent.length > 0), `${code} produces a visible rejection`);
    await assertMap(page, expected, `${code} rejection preserves the current map`);
  }
  await capture(page, 'q', 'Escape');
  await visible(page, '#keys');
  await assertMap(page, expected, 'Escape cancels capture without rebinding or closing settings');
  await click(page, '#btn-keys-close');
  await page.reload({ waitUntil: 'load' });
  await attach(page);
  await assertMap(page, expected, 'reload restores the saved map into engine and HUD');

  await click(page, '#btn-practice');
  await page.waitForFunction(() => window.qaState().phase === 'select' && window.qaState().roster?.length === 2);
  await click(page, '#cards [data-c="0"]');
  await page.waitForFunction(() => window.qaState().roster[0].picked);
  await page.waitForFunction(() => !document.getElementById('btn-start').disabled &&
    [...document.querySelectorAll('#cards img')].length === 5 &&
    [...document.querySelectorAll('#cards img')].every(image => image.naturalWidth > 0));
  check(true, 'all five champion portraits load and the ready draft is presented');
  await renderedIcons(page, '#detail .dkit use', 4, 'draft ability icons');
  await screenshot(page, 'draft-desktop');
  await click(page, '#btn-start');
  await page.waitForFunction(() => window.qaState().phase === 'live' && window.qaState().me?.alive);
  await renderedIcons(page, '#abils .face use, #spells .face use', 6, 'live ability and spell icons');
  await click(page, '#abils [data-abil="1"] .up');
  await page.waitForFunction(() => window.qaState().me.rk[1] === 1);
  await visible(page, '#abils [data-abil="1"] .cd', false);
  await pointer(page);
  await screenshot(page, 'live-hud-desktop');
  await test('live-canvas-tab', page, async () => {
    check(!(await keyboard(page, 'Tab')).prevented, 'live canvas does not block Tab');
  });

  await key(page, 'KeyB');
  await visible(page, '#shop', false);
  check(!(await state(page)).shop, 'old B key does not open the remapped shop');
  await key(page, 'KeyC');
  await visible(page, '#shop');
  check((await state(page)).shop, 'new C key opens the shop in both page and engine');
  await renderedIcons(page, '#shop .it use', 1, 'shop item icons');
  const potion = await page.evaluate(() => JSON.parse(window.qaWasm.data_json()).items.find(item => item.charges && item.cost <= 500));
  assert(potion, 'Shop should offer an affordable potion');
  await click(page, `#shop [data-buy="${potion.id}"]`);
  await page.waitForFunction(id => window.qaState().me.items.includes(id), potion.id);
  await renderedIcons(page, '#islots use', 1, 'purchased inventory icon');
  await key(page, 'KeyC');
  await visible(page, '#shop', false);
  check(!(await state(page)).shop, 'new C key closes the shop in both page and engine');

  await key(page, 'Escape');
  await visible(page, '#pause');
  await click(page, '#btn-keys3');
  await visible(page, '#keys');
  await click(page, '#btn-keys-close');
  await visible(page, '#keys', false);
  await visible(page, '#pause');
  await key(page, expected.w);
  await command(page, { cast: 1, aim: [0.5, 0.06] });
  check((await advance(page)).me.cd[1] === 0,
    'closing settings inside pause keeps keyboard and HUD casting disabled');
  await click(page, '#btn-help');
  await visible(page, '#help');
  await click(page, '#btn-help-close');
  await key(page, expected.w);
  check((await state(page)).me.cd[1] === 0, 'closing help inside pause preserves the parent input lock');
  await click(page, '#btn-resume');
  await visible(page, '#pause', false);
  await page.evaluate(() => document.querySelector('#ember-root canvas').focus());
  await advance(page);

  // These temporary DOM controls exercise the page's delegated focus handlers
  // during a live match; production text/select controls exist in hidden menus.
  await page.evaluate(() => {
    const form = document.createElement('div');
    form.id = 'qa-focus-form';
    form.innerHTML = '<input id="qa-text" aria-label="QA text"><select id="qa-select" aria-label="QA select"><option>One</option><option>Two</option></select>';
    document.body.append(form);
  });
  for (const selector of ['#qa-text', '#qa-select']) {
    await command(page, { stop: true });
    await advance(page);
    await page.evaluate(selector => document.querySelector(selector).focus(), selector);
    let before = await advance(page);
    check(!(await keyboard(page, 'Tab', selector)).prevented, `${selector} keeps normal Tab navigation available`);
    await key(page, expected.w, selector);
    await command(page, { cast: 1, aim: [0.5, 0.06] });
    const locked = await pointer(page, true);
    check(locked.me.cd[1] === 0 && distance(before, locked) < 0.05,
      `${selector} focus suppresses keyboard, UI casts and mouse movement`);
    await page.evaluate(() => document.querySelector('#ember-root canvas').focus());
    before = await advance(page);
    check(distance(before, await pointer(page, true)) > 0.2,
      `leaving ${selector} restores gameplay input`);
  }
  await command(page, { stop: true });
  await page.evaluate(() => document.getElementById('qa-focus-form').remove());
  await advance(page);
  check((await key(page, expected.w)).me.cd[1] > 0, 'a fresh remapped W cast works after dialogs and field focus end');

  await click(page, '#btn-keys2');
  await click(page, '#btn-keys-reset');
  await assertMap(page, defaults, 'reset defaults updates saved map, engine and HUD together');
  check(await page.locator('#key-status').textContent() === 'Reset to defaults.', 'reset provides visible confirmation');
  await click(page, '#btn-keys-close');
}

async function noOverflow(page, name) {
  const widths = await page.evaluate(() => ({
    viewport: innerWidth, html: document.documentElement.scrollWidth, body: document.body.scrollWidth,
    keys: document.getElementById('keys').scrollWidth,
    box: document.querySelector('.keys-box').scrollWidth,
    boxClient: document.querySelector('.keys-box').clientWidth,
  }));
  check(widths.html <= widths.viewport + 1 && widths.body <= widths.viewport + 1,
    name + ': page has no horizontal overflow (' + JSON.stringify(widths) + ')');
  if (name.includes('settings')) check(widths.box <= widths.boxClient + 1, 'mobile settings contents have no horizontal overflow');
}

async function main() {
  os.setPriority(0, os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out, { recursive: true });
  for (const file of ['web/pkg/league_bg.wasm', 'web/games/league/v2/index.html', 'web/games/league/v2/ui.js', 'web/games/league/v2/ui.css']) {
    const data = fs.readFileSync(path.join(root, file));
    report.assets.push({ file, bytes: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') });
  }
  preview = await startPreview({ port: 8096, gamePort: 7796 });
  const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');
  browser = await chromium.launch({ channel: 'msedge', headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'] });
  const desktopPage = await newPage();
  await test('desktop-bindings-and-hud', desktopPage, () => desktop(desktopPage));
  await desktopPage.close();

  const invalid = await newPage({ initialStorage: JSON.stringify({ ...defaults, q: 'KeyW' }) });
  await test('invalid-saved-map', invalid, async () => {
    await click(invalid, '#btn-keys');
    await assertMap(invalid, defaults, 'invalid saved map recovers to default bindings');
    check(await invalid.evaluate(() => {
      const status = document.getElementById('key-status');
      return status.getClientRects().length && /invalid.*default|default.*restor/i.test(status.textContent);
    }), 'invalid saved map recovery is visible when settings opens');
    await screenshot(invalid, 'invalid-saved-recovery');
  });
  await invalid.close();

  const mobile = await newPage({ width: 390, height: 844 });
  await test('mobile-menu', mobile, async () => {
    await screenshot(mobile, 'menu-mobile');
    await noOverflow(mobile, 'mobile menu');
  });
  await test('mobile-settings', mobile, async () => {
    await click(mobile, '#btn-keys');
    await visible(mobile, '#keys');
    await screenshot(mobile, 'settings-mobile');
    check(await mobile.locator('#key-rows .krow').count() === 14, 'mobile settings retains all 14 controls');
    await noOverflow(mobile, 'mobile settings');
  });
  await mobile.close();
  check(report.errors.length === 0, 'no uncaught browser errors');
  check(report.console.length === 0, 'no browser console errors');
  report.passed = report.failures.length === 0;
  if (!report.passed) process.exitCode = 1;
}

main().catch(error => {
  report.failures.push({ name: 'harness', error: error.stack });
  console.error(error);
  process.exitCode = 1;
}).finally(async () => {
  if (browser) await browser.close();
  if (preview) await preview.close();
  report.elapsedSeconds = (Date.now() - started) / 1000;
  fs.mkdirSync(out, { recursive: true });
  fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify(report, null, 2));
});
