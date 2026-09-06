/* Pure settings/startup regression checks; no browser, DOM input or GPU.
 * Run: node --experimental-vm-modules tools/v30/settings.test.cjs
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const started = performance.now();
const pageRoot = path.resolve(__dirname, '../../web/games/arena/v30');
const html = fs.readFileSync(path.join(pageRoot, 'index.html'), 'utf8');
const settings = require(path.join(pageRoot, 'settings.js'));
if (!vm.SourceTextModule) {
  throw new Error('Run with node --experimental-vm-modules tools/v30/settings.test.cjs');
}

let checks = 0;
const check = fn => { fn(); checks++; };
const base = settings.defaults();
check(() => assert.equal(settings.ACTIONS.length, 22));
check(() => assert.deepEqual(settings.validateConfig(base), base));
check(() => assert.deepEqual(base.bindings.sprint, ['ShiftLeft', 'ShiftRight']));
check(() => assert.deepEqual(base.bindings.melee, ['KeyE']));
for (const key of ['KeyZ', 'Digit0', 'Space', 'Tab', 'ShiftRight', 'ArrowUp', 'Mouse0', 'Mouse1', 'Mouse2']) {
  check(() => assert(settings.allowedCode(key)));
}
for (const key of ['Escape', 'Enter', 'ControlLeft', 'AltLeft', 'MetaLeft', 'F1', 'F11', 'Mouse3', 'KeyAA', '']) {
  check(() => assert(!settings.allowedCode(key)));
}
for (const value of [NaN, Infinity, -Infinity, 0, 3.01, '1']) {
  check(() => assert.throws(() => settings.validateConfig({ ...base, sensitivity: value })));
}
for (const action of settings.ACTIONS) {
  check(() => {
    const changed = settings.rebind(base, action.id, 'KeyZ');
    assert.deepEqual(changed.bindings[action.id], ['KeyZ']);
    assert.deepEqual(base, settings.defaults(), 'rebinding must not mutate the prior config');
  });
}
check(() => assert.throws(() => settings.rebind(base, 'fire', 'KeyW'), /already used/));
check(() => assert.throws(() => settings.rebind(base, 'unknown', 'KeyZ'), /Unknown/));
check(() => assert.throws(() => settings.rebind(base, 'fire', 'Escape'), /reserved/));
check(() => assert.deepEqual(settings.rebind(base, 'fire', 'Mouse1').bindings.fire, ['Mouse1']));
for (const codes of [[], ['KeyZ', 'KeyZ'], ['KeyZ', 'KeyX', 'KeyV'], ['Escape']]) {
  check(() => assert.throws(() => settings.validateConfig({ ...base, bindings: { ...base.bindings, fire: codes } })));
}
check(() => assert.throws(() => settings.validateConfig({ ...base, bindings: { ...base.bindings, unknown: ['KeyZ'] } })));
for (let slot = 1; slot <= 9; slot++) {
  check(() => assert.deepEqual(base.bindings[`slot${slot}`], [`Digit${slot}`]));
}
check(() => assert.throws(() => settings.rebind(base, 'fire', 'Digit1'), /already used/));
check(() => assert.notEqual(settings.STORAGE_KEY, settings.LEGACY_STORAGE_KEY, 'new preferences must not break frozen versions'));
const legacy = { version: 1, sensitivity: 1.65, bindings: Object.fromEntries(Object.entries(base.bindings).filter(([id]) => !id.startsWith('slot'))) };
check(() => {
  const migrated = settings.migrateLegacy(legacy);
  assert.equal(migrated.adjusted, false);
  assert.deepEqual(migrated.config, { ...base, sensitivity: 1.65 });
});
check(() => {
  const original = JSON.parse(JSON.stringify(legacy));
  original.bindings.fire = ['Digit1']; original.bindings.ads = ['Digit0'];
  const before = JSON.stringify(original);
  const migrated = settings.migrateLegacy(original);
  assert.equal(migrated.adjusted, true);
  assert.deepEqual(migrated.config.bindings.fire, ['Digit1']);
  assert.deepEqual(migrated.config.bindings.ads, ['Digit0']);
  assert.deepEqual(migrated.config.bindings.slot2, ['Digit2']);
  assert.notEqual(migrated.config.bindings.slot1[0], 'Digit1');
  assert.equal(JSON.stringify(original), before, 'legacy data is never mutated');
  assert.deepEqual(settings.validateConfig(migrated.config), migrated.config);
});
for (const invalid of [null, {}, { ...legacy, version: 2 }, { ...legacy, sensitivity: 99 }, { ...legacy, bindings: { ...legacy.bindings, fire: ['KeyW'] } }]) {
  check(() => assert.throws(() => settings.migrateLegacy(invalid)));
}

// HUD math is pure and authority driven, including malformed telemetry.
const snapshot = {
  visible: true, alive: true, hp: 5, max_hp: 5, weapon: 3,
  magazine: 17, reserve: 60, reload_remaining: 1.2, reload_duration: 2.4,
  slots: [1, 2, 3, 4, 5, 6, 7, 0, 0], selected_slot: 3,
};
check(() => assert.equal(settings.normalizeHud(snapshot).reload_progress, 50));
check(() => assert.deepEqual(settings.normalizeHud(JSON.stringify(snapshot)), settings.normalizeHud(snapshot)));
for (const invalid of ['{', null, [], 1, undefined]) check(() => assert.equal(settings.normalizeHud(invalid), null));
check(() => {
  const sanitized = settings.normalizeHud({ ...snapshot, hp: 99, magazine: -5, reserve: Infinity, reload_remaining: NaN, selected_slot: 9 });
  assert.equal(sanitized.hp, 5); assert.equal(sanitized.magazine, 0); assert.equal(sanitized.reserve, 0);
  assert.equal(sanitized.reload_remaining, 0); assert.equal(sanitized.selected_slot, 0);
});
check(() => {
  const dead = settings.normalizeHud({ ...snapshot, alive: false });
  assert.equal(dead.hp, 0); assert.equal(dead.reload_remaining, 0);
});
check(() => assert.equal(settings.normalizeHud({ ...snapshot, reserve: null }).reserve, null));
check(() => assert.equal(settings.normalizeHud({ ...snapshot, reload_duration: 0 }).reload_remaining, 0));
check(() => assert.equal(settings.normalizeHud({ ...snapshot, reload_remaining: 900 }).reload_progress, 0));
check(() => assert.deepEqual(settings.WEAPONS.map(weapon => weapon.name), ['Empty', 'Sidearm', 'Vityaz', 'AK-47', 'M4', 'Revolver', 'Sniper', 'RPG-7']));

// Execute the actual DOM renderer against a small DOM stand-in. This does not
// claim browser layout, input, GPU or real fullscreen verification.
class Element {
  constructor() { this.textContent = ''; this.hidden = false; this.attrs = new Map(); this.style = {}; this.children = []; }
  append(...children) { this.children.push(...children); }
  getAttribute(name) { return this.attrs.has(name) ? this.attrs.get(name) : null; }
  setAttribute(name, value) { this.attrs.set(name, String(value)); }
}
const ids = ['killshot-hud', 'killshot-slots', 'killshot-reload', ...['life', 'life-max', 'life-state', 'weapon', 'ammo', 'reserve', 'reload-time', 'reload-ring'].map(name => `ks-${name}`)];
const nodes = new Map(ids.map(id => [id, new Element()]));
const fakeDocument = { getElementById: id => nodes.get(id), createElement: () => new Element() };
const renderHud = settings.createHudRenderer(fakeDocument, () => ({ slot3: ['KeyZ'] }));
check(() => assert.equal(nodes.get('killshot-slots').children.length, 9));
check(() => {
  assert.equal(renderHud(snapshot), true);
  assert.equal(nodes.get('killshot-hud').hidden, false);
  assert.equal(nodes.get('ks-ammo').textContent, '17');
  assert.equal(nodes.get('ks-reserve').textContent, '60');
  assert.equal(nodes.get('ks-life').textContent, '5');
  assert.equal(nodes.get('ks-reload-time').textContent, '1.2');
  assert.equal(nodes.get('ks-reload-ring').style.strokeDashoffset, '50');
  assert.equal(nodes.get('killshot-reload').getAttribute('aria-valuenow'), '50');
  const third = nodes.get('killshot-slots').children[2];
  assert.equal(third.children[0].textContent, 'Z');
  assert.equal(third.getAttribute('data-selected'), 'true');
  assert.equal(third.children[1].textContent, 'AK-47');
});
for (let weapon = 1; weapon <= 7; weapon++) {
  check(() => {
    renderHud({ ...snapshot, weapon, selected_slot: weapon, reload_remaining: 0 });
    assert.equal(nodes.get('ks-weapon').textContent, settings.WEAPONS[weapon].name.toUpperCase());
    assert.equal(nodes.get('killshot-reload').hidden, true);
  });
}
check(() => {
  renderHud({ ...snapshot, hp: 1, magazine: 0, reserve: null });
  assert.equal(nodes.get('killshot-hud').getAttribute('data-low-life'), 'true');
  assert.equal(nodes.get('killshot-hud').getAttribute('data-empty'), 'true');
  assert.equal(nodes.get('ks-reserve').textContent, '∞');
});
check(() => {
  renderHud({ ...snapshot, alive: false });
  assert.equal(nodes.get('killshot-reload').hidden, true);
  assert.equal(nodes.get('ks-life-state').textContent, 'RESPAWNING');
});
check(() => { renderHud({ ...snapshot, visible: false }); assert.equal(nodes.get('killshot-hud').hidden, true); });
check(() => { assert.equal(renderHud('{'), false); assert.equal(nodes.get('killshot-hud').hidden, true); });
check(() => assert.equal(settings.createHudRenderer({ getElementById: () => null })(snapshot), false));

// Parse the real page without executing imports, creating a DOM, or starting WASM.
for (const match of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/g)) {
  if (!/\bsrc=/.test(match[1])) {
    check(() => /type="module"/.test(match[1]) ? new vm.SourceTextModule(match[2]) : new vm.Script(match[2]));
  }
}
check(() => {
  const settingsScript = html.match(/<script src="\.\/settings\.js\?v=[^"\s]+"><\/script>/);
  assert(settingsScript && settingsScript.index < html.indexOf('<script type="module">'));
});
check(() => assert(!/3 HP|[♥❤]/.test(html)));
check(() => assert.match(html, /<title>Killshot · v30 · Ember<\/title>/));
check(() => assert.match(html, /#killshot-hud \{[^}]*pointer-events: none/));
check(() => assert.match(html, /id="killshot-hud" hidden/));
check(() => assert.match(html, /prefers-reduced-motion:reduce/));
check(() => assert.match(html, /id="lobby-loadout"/));
check(() => assert.match(html, /id="lobby-starting-weapon" disabled/));

// Execute the actual page's storage/account functions, not an implementation
// copied into the fixture. The DOM rendering and network phases are excluded.
function sourceBetween(start, end) {
  const first = html.indexOf(start);
  const last = html.indexOf(end, first + start.length);
  assert(first >= 0 && last > first, `page fixture boundaries missing: ${start}`);
  return html.slice(first, last);
}
const accountCode = sourceBetween('    const ACCT_KEY', '    function renderAccount')
  + '\nglobalThis.test = { getAccount, readStorage, writeStorage };';
function fresh(storage, getterThrows = false) {
  const context = { crypto: { randomUUID: () => 'visit-id' } };
  if (getterThrows) {
    Object.defineProperty(context, 'localStorage', { get() { throw Error('denied'); } });
  } else {
    context.localStorage = storage;
  }
  vm.createContext(context);
  vm.runInContext(accountCode, context);
  return context;
}

let ctx = fresh(null, true);
check(() => assert.equal(ctx.test.getAccount().id, 'visit-id'));
check(() => assert.equal(ctx.test.getAccount(), ctx.test.getAccount()));
check(() => assert.equal(ctx.test.readStorage('ember-server-url-manual'), null));
check(() => assert.doesNotThrow(() => ctx.test.writeStorage('ember-account', 'x')));
check(() => {
  ctx.test.getAccount().handle = 'renamed';
  assert.equal(ctx.test.getAccount().handle, 'renamed');
});

ctx = fresh({ getItem() { throw Error('read denied'); }, setItem() { throw Error('write denied'); } });
check(() => assert.equal(ctx.test.getAccount(), ctx.test.getAccount()));
const writes = [];
ctx = fresh({
  getItem() { return JSON.stringify({ handle: 'existing', id: 'saved-id' }); },
  setItem(key, value) { writes.push([key, value]); },
});
check(() => assert.equal(ctx.test.getAccount().handle, 'existing'));
check(() => assert.equal(writes.length, 0, 'reading an existing account must not overwrite it'));

ctx = fresh({
  getItem() { return JSON.stringify({ handle: 'existing', id: 'saved-id' }); },
  setItem() { throw Error('quota'); },
});
check(() => {
  const account = ctx.test.getAccount();
  account.handle = 'new-name';
  ctx.test.writeStorage('ember-account', JSON.stringify(account));
  assert.equal(ctx.test.getAccount().handle, 'new-name', 'failed persistence must not undo the rename');
});

ctx = fresh({ getItem() { return '{bad-json'; }, setItem(key, value) { writes.push([key, value]); } });
check(() => assert.equal(ctx.test.getAccount().id, 'visit-id'));
check(() => assert.deepEqual(writes.map(([key]) => key), ['ember-account'], 'no new storage keys'));

const overrideCode = sourceBetween('    const overridden', '    let chosen')
  + '\nglobalThis.overrideResult = overridden;';
ctx = fresh(null, true);
vm.runInContext(overrideCode, ctx);
check(() => assert.equal(ctx.overrideResult, ''));

const pendingCode = sourceBetween('    let pending = null;', '    if (pending && pending.lobby && pending.ws)');
check(() => assert.doesNotThrow(() => vm.runInNewContext(pendingCode, {
  sessionStorage: { getItem() { throw Error('denied'); }, removeItem() { throw Error('denied'); } },
})));
check(() => assert.doesNotThrow(() => vm.runInNewContext(pendingCode, {})));

console.log(JSON.stringify({
  passed: true, checks,
  elapsed_ms: Math.round((performance.now() - started) * 1000) / 1000,
  scope: 'Pure config/migration, HUD renderer with DOM stand-in, inline syntax, denied/quota storage account and host/pending startup; no real browser/GPU execution',
}));
