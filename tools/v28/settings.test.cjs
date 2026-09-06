/* Pure settings/startup regression checks; no browser, DOM input or GPU.
 * Run: node --experimental-vm-modules tools/v28/settings.test.cjs
 */
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const pageRoot = path.resolve(__dirname, '../../web/games/arena/v28');
const html = fs.readFileSync(path.join(pageRoot, 'index.html'), 'utf8');
const settings = require(path.join(pageRoot, 'settings.js'));
if (!vm.SourceTextModule) {
  throw new Error('Run with node --experimental-vm-modules tools/v28/settings.test.cjs');
}

let checks = 0;
const check = fn => { fn(); checks++; };
const base = settings.defaults();
check(() => assert.equal(settings.ACTIONS.length, 13));
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
  scope: 'Pure config, inline syntax, denied/quota storage account and host/pending startup; no DOM/GPU execution',
}));
