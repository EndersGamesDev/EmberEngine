/* Arena v31 controls. Install before WASM so menu input is intercepted first.
 * Pure config helpers also export in Node for tests; no browser is launched.
 * Rust settings.rs consumes the same 22 actions, code arrays and revision.
 */
(function (root) {
  'use strict';
  // v31 keeps v30's identical 22-action schema and storage key. Existing
  // sensitivity, slot8 and all other remaps therefore load without resetting.
  const STORAGE_KEY = 'ember-killshot-settings-v1';
  const LEGACY_STORAGE_KEY = 'ember-arena-settings-v1';
  const ACTIONS = [
    ['forward', 'Move forward', ['KeyW']], ['backward', 'Move backward', ['KeyS']],
    ['left', 'Strafe left', ['KeyA']], ['right', 'Strafe right', ['KeyD']],
    ['jump', 'Jump', ['Space']], ['sprint', 'Sprint', ['ShiftLeft', 'ShiftRight']],
    ['crouch', 'Crouch', ['KeyC']], ['fire', 'Fire', ['Mouse0']],
    ['ads', 'Aim down sights', ['Mouse2']], ['reload', 'Reload', ['KeyR']],
    ['melee', 'Melee', ['KeyE']], ['shield', 'Shield', ['KeyQ']],
    ['scoreboard', 'Scoreboard', ['Tab']],
    ...Array.from({ length: 9 }, (_, index) => [`slot${index + 1}`, `Weapon slot ${index + 1}`, [`Digit${index + 1}`]]),
  ].map(([id, label, codes]) => Object.freeze({ id, label, codes: Object.freeze(codes) }));
  const allowedCode = code => typeof code === 'string' && (
    /^Key[A-Z]$/.test(code) || /^Digit[0-9]$/.test(code) || /^Mouse[012]$/.test(code)
    || ['Space', 'Tab', 'ShiftLeft', 'ShiftRight', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(code));
  const defaults = () => ({ sensitivity: 1, bindings: Object.fromEntries(ACTIONS.map(action => [action.id, [...action.codes]])) });
  function validateConfig(value) {
    if (!value || typeof value !== 'object' || !Number.isFinite(value.sensitivity)
      || value.sensitivity < 0.1 || value.sensitivity > 3 || !value.bindings || typeof value.bindings !== 'object') {
      throw new Error('Saved settings are invalid. Defaults have been restored.');
    }
    if (Object.keys(value.bindings).some(id => !ACTIONS.some(action => action.id === id))) throw new Error('Unknown game action.');
    const used = new Set(), bindings = {};
    for (const action of ACTIONS) {
      const codes = value.bindings[action.id];
      if (!Array.isArray(codes) || codes.length < 1 || codes.length > 2 || codes.some(code => !allowedCode(code))) {
        throw new Error(`Invalid binding for ${action.label}.`);
      }
      for (const code of codes) {
        if (used.has(code)) throw new Error(`${keyLabel(code)} is assigned more than once.`);
        used.add(code);
      }
      bindings[action.id] = [...codes];
    }
    return { sensitivity: value.sensitivity, bindings };
  }
  function keyLabel(code) {
    const names = { Mouse0: 'Left mouse', Mouse1: 'Middle mouse', Mouse2: 'Right mouse',
      Space: 'Space', Tab: 'Tab', ShiftLeft: 'Left Shift', ShiftRight: 'Right Shift',
      ArrowUp: 'Up arrow', ArrowDown: 'Down arrow', ArrowLeft: 'Left arrow', ArrowRight: 'Right arrow' };
    return names[code] || code.replace(/^Key|^Digit/, '');
  }
  function rebind(config, actionId, code) {
    const action = ACTIONS.find(item => item.id === actionId);
    if (!action) throw new Error('Unknown game action.');
    if (!allowedCode(code)) throw new Error('That key is reserved or unsupported. Use a letter, number, arrow, Space, Tab, Shift or a mouse button.');
    const other = ACTIONS.find(item => item.id !== actionId && config.bindings[item.id].includes(code));
    if (other) throw new Error(`${keyLabel(code)} is already used for ${other.label}. Change that binding first.`);
    return validateConfig({ sensitivity: config.sensitivity, bindings: { ...config.bindings, [actionId]: [code] } });
  }
  function migrateLegacy(value) {
    // The archived pages still expect exactly thirteen actions. Never write
    // the enlarged table to their key or silently discard customized controls.
    const oldActions = ACTIONS.filter(action => !action.id.startsWith('slot'));
    if (!value || value.version !== 1 || !value.bindings
      || Object.keys(value.bindings).length !== oldActions.length) throw new Error('Invalid legacy controls.');
    const bindings = {}, used = new Set();
    for (const action of oldActions) {
      const codes = value.bindings[action.id];
      if (!Array.isArray(codes) || codes.length < 1 || codes.length > 2) throw new Error('Invalid legacy binding.');
      for (const code of codes) {
        if (!allowedCode(code) || used.has(code)) throw new Error('Invalid legacy binding.');
        used.add(code);
      }
      bindings[action.id] = [...codes];
    }
    const fallback = ['Digit0', ...'ZXVBNMFGHJKLPOIUYTREWQASDC'.split('').map(letter => `Key${letter}`), ...'123456789'.split('').map(n => `Digit${n}`)];
    let adjusted = false;
    for (const action of ACTIONS.filter(action => action.id.startsWith('slot'))) {
      const preferred = action.codes[0];
      const code = used.has(preferred) ? fallback.find(candidate => !used.has(candidate)) : preferred;
      if (!code) throw new Error('No unused weapon-slot key remains.');
      adjusted ||= code !== preferred;
      bindings[action.id] = [code]; used.add(code);
    }
    return { config: validateConfig({ sensitivity: value.sensitivity, bindings }), adjusted };
  }
  const helpers = { STORAGE_KEY, LEGACY_STORAGE_KEY, ACTIONS, allowedCode, defaults, validateConfig, keyLabel, rebind, migrateLegacy };
  if (typeof module === 'object' && module.exports) module.exports = helpers;
  if (!root?.document) return;

  const document = root.document;
  let config = defaults(), initialNote = '', storageUnavailable = false;
  try {
    const saved = root.localStorage.getItem(STORAGE_KEY);
    if (saved !== null) {
      const data = JSON.parse(saved);
      if (data.version !== 1) throw new Error('Saved settings use another version.');
      config = validateConfig(data);
    } else {
      const legacy = root.localStorage.getItem(LEGACY_STORAGE_KEY);
      if (legacy !== null) {
        const migrated = migrateLegacy(JSON.parse(legacy));
        config = migrated.config;
        initialNote = migrated.adjusted
          ? 'Your controls were imported. Some weapon slots use spare keys to preserve your number-key bindings; review them below.'
          : 'Your controls were imported. Weapon slots 1–9 are ready.';
      }
    }
  } catch {
    initialNote = 'Saved controls could not be loaded. Defaults are active.';
  }
  let revision = 0, paused = true, playing = false, capturing = null, previousFocus = null;
  let pendingLock = false, closeGuard = false, ignoredMouseGesture = null;
  const held = new Set(), blockedUntilRelease = new Set();
  function publish(nextPaused = paused) {
    // The platform gate must change first: a slider/key event must never become
    // a game action while the Rust settings snapshot catches up next frame.
    root.__emberInputPaused = Boolean(nextPaused);
    paused = Boolean(nextPaused);
    revision++;
    const bindings = Object.fromEntries(ACTIONS.map(action => [action.id, Object.freeze([...config.bindings[action.id]])]));
    root.__emberArenaSettings = Object.freeze({ revision, paused, sensitivity: config.sensitivity, bindings: Object.freeze(bindings) });
  }
  publish(true);

  const style = document.createElement('style');
  style.textContent = `
    .arena-settings{color-scheme:dark;color:#e5edf8;background:linear-gradient(145deg,#15263c,#091523 70%);border:1px solid #3b5271;border-radius:18px;padding:0;width:min(680px,calc(100vw - 28px));max-height:88vh;max-height:88dvh;box-sizing:border-box;box-shadow:0 28px 100px #000a;font:14px/1.5 system-ui,-apple-system,"Segoe UI",sans-serif}
    .arena-settings::backdrop{background:#020912ba;backdrop-filter:blur(5px)}
    .arena-settings[open]{display:flex;flex-direction:column;overflow:hidden}.arena-settings .settings-inner{padding:24px 26px;min-height:0;overflow-y:auto}.arena-settings .settings-top{display:flex;align-items:start;justify-content:space-between;gap:16px}
    .arena-settings h2{font-size:1.45rem;letter-spacing:-.03em;margin:0;color:#fff}.arena-settings .eyebrow{font-size:.7rem;text-transform:uppercase;letter-spacing:.18em;color:#f0b429;margin:0 0 5px}
    .arena-settings p{margin:6px 0 16px;color:#a8bad3}.arena-settings .settings-note{background:#f0b42910;border:1px solid #f0b42935;border-radius:9px;padding:10px 12px;color:#f0cf84;font-size:.82rem}
    .arena-settings .display-card{display:flex;align-items:center;justify-content:space-between;gap:16px;margin:18px 0;padding:15px;background:linear-gradient(115deg,#23436280,#0b1b2e);border:1px solid #49678c;border-radius:12px}.arena-settings .display-title{display:flex;align-items:center;gap:9px;font-size:1rem;font-weight:650;color:#fff}.arena-settings .display-icon{font-size:1.5rem;color:#f0b429;line-height:1}.arena-settings .display-state{display:block;color:#9cc9ef;font-size:.75rem;margin-top:3px}.arena-settings .display-help{font-size:.75rem;margin:5px 0 0;max-width:36ch}.arena-settings #settings-fullscreen{flex-shrink:0;background:#d8e9fa;color:#0a1a2c;border-color:#d8e9fa;font-weight:700;padding:10px 14px}.arena-settings #settings-fullscreen:hover{background:#fff}.arena-settings #settings-fullscreen:disabled{cursor:default}
    .arena-settings .sensitivity-head{display:flex;justify-content:space-between;align-items:baseline;gap:12px;margin-top:20px}.arena-settings label{opacity:1;color:#e5edf8;font-size:.9rem}.arena-settings output{color:#f0b429;font-weight:650;font-variant-numeric:tabular-nums}
    .arena-settings input[type=range]{padding:8px 0;accent-color:#f0b429;width:100%;border:0;background:transparent}.arena-settings .range-labels{display:flex;justify-content:space-between;color:#8ea4c2;font-size:.75rem}
    .arena-settings fieldset{border:0;padding:0;margin:22px 0 0;min-width:0}.arena-settings legend{padding:0;font-weight:650;font-size:.95rem}.arena-settings .binding-help{font-size:.78rem;margin:4px 0 12px}
    .arena-settings .binding-grid{display:grid;grid-template-columns:1fr 1fr;gap:7px 12px}.arena-settings .binding-row{display:flex;align-items:center;justify-content:space-between;gap:8px;padding:7px 9px;background:#07111e80;border:1px solid #273b56;border-radius:8px;min-width:0}.arena-settings .binding-row>span{font-size:.8rem}
    .arena-settings button{font:inherit;border-radius:7px;border:1px solid #3a5376;background:#1a304e;color:#e5edf8;padding:7px 10px;cursor:pointer}.arena-settings button:hover{background:#254163}.arena-settings button:disabled{opacity:.5;cursor:wait}.arena-settings button:focus-visible,.arena-settings input:focus-visible{outline:2px solid #f0b429;outline-offset:3px}
    .arena-settings .binding-button{font-size:.76rem;white-space:nowrap;max-width:64%;overflow:hidden;text-overflow:ellipsis}.arena-settings .binding-button[aria-pressed=true]{border-color:#f0b429;color:#f0cf84;background:#684a1540}
    .arena-settings .settings-feedback{min-height:2.8em;font-size:.8rem;margin:12px 0;color:#f0cf84}.arena-settings .settings-actions{display:flex;justify-content:space-between;align-items:center;gap:12px;border-top:1px solid #2b3e58;padding:16px 26px;flex-shrink:0;background:#091523}.arena-settings .resume-button{background:#eab336;color:#07101b;border-color:#eab336;font-weight:700;padding:10px 20px}.arena-settings .resume-button:hover{background:#ffd068}.arena-settings .save-caption{font-size:.7rem;color:#8ea4c2;margin:10px 0 0}
    @media(max-width:540px){.arena-settings .settings-inner{padding:18px 16px}.arena-settings .binding-grid{grid-template-columns:1fr}.arena-settings .binding-button{max-width:60%}.arena-settings .settings-actions{align-items:stretch}.arena-settings h2{font-size:1.25rem}}
    @media(max-width:400px){.arena-settings .display-card{flex-wrap:wrap;gap:10px}.arena-settings #settings-fullscreen{width:100%}}
    @media(prefers-reduced-transparency:reduce){.arena-settings::backdrop{backdrop-filter:none;background:#020912ee}}
  `;
  document.head.append(style);
  const dialog = document.createElement('dialog');
  dialog.className = 'arena-settings'; dialog.id = 'arena-settings';
  dialog.setAttribute('aria-labelledby', 'settings-title');
  dialog.setAttribute('aria-describedby', 'settings-notice');
  dialog.innerHTML = `<div class="settings-inner">
    <div class="settings-top"><div><div class="eyebrow">Killshot · Personal setup</div><h2 id="settings-title">Make it feel right.</h2></div></div>
    <p>Display mode, plus mouse sensitivity and controls saved on this browser.</p>
    <div id="settings-notice" class="settings-note">Set up before joining. In multiplayer, opening this menu does not pause the match.</div>
    <section class="display-card" aria-labelledby="settings-display-title"><div><div class="display-title" id="settings-display-title"><span class="display-icon" aria-hidden="true">⛶</span> Fullscreen mode</div><span id="settings-fullscreen-state" class="display-state" role="status">Windowed</span><p id="settings-fullscreen-help" class="display-help">Fill your screen with the game. F11 is the separate browser shortcut on desktop.</p></div><button type="button" id="settings-fullscreen" aria-describedby="settings-fullscreen-help" aria-pressed="false">Enter fullscreen</button></section>
    <div class="sensitivity-head"><label for="arena-sensitivity">Mouse sensitivity</label><output id="arena-sensitivity-value" for="arena-sensitivity">1.00×</output></div>
    <input id="arena-sensitivity" type="range" min="0.1" max="3" step="0.05" value="1" aria-describedby="sensitivity-help">
    <div id="sensitivity-help" class="range-labels"><span>0.10× · precise</span><span>1× default</span><span>3× · fast</span></div>
    <fieldset><legend>Keyboard &amp; mouse</legend><p class="binding-help" id="binding-help">Select a binding, then press one key or mouse button. Esc cancels. Browser shortcut keys and duplicate assignments are not allowed.</p><button id="binding-cancel" type="button" hidden>Cancel binding</button><div class="binding-grid" id="binding-grid"></div></fieldset>
    <p id="settings-feedback" class="settings-feedback" role="status" aria-live="polite" aria-atomic="true"></p>
    <p class="save-caption">Mouse buttons: left, middle or right. Controller bindings are unchanged.</p>
  </div><div class="settings-actions"><button type="button" id="settings-reset">Restore defaults</button><button type="button" id="settings-resume" class="resume-button">Done</button></div>`;
  document.body.append(dialog);
  const $ = id => document.getElementById(id);
  const slider = $('arena-sensitivity'), sensitivityValue = $('arena-sensitivity-value');
  const feedback = $('settings-feedback'), resumeButton = $('settings-resume');
  const fullscreenButton = $('settings-fullscreen'), fullscreenState = $('settings-fullscreen-state');
  const fullscreenHelp = $('settings-fullscreen-help');
  let fullscreenPending = false, fullscreenError = '';
  const browserFullscreen = root.matchMedia?.('(display-mode: fullscreen)');
  function refreshFullscreen() {
    const active = Boolean(document.fullscreenElement);
    const browserMode = !active && Boolean(browserFullscreen?.matches);
    const supported = Boolean(document.documentElement.requestFullscreen) && document.fullscreenEnabled !== false;
    fullscreenState.textContent = active ? 'Fullscreen' : browserMode ? 'Fullscreen · browser' : 'Windowed';
    fullscreenButton.textContent = active ? 'Exit fullscreen' : browserMode ? 'Use F11 to exit' : 'Enter fullscreen';
    fullscreenButton.setAttribute('aria-pressed', String(active || browserMode));
    fullscreenButton.disabled = fullscreenPending || browserMode || (!active && !supported);
    fullscreenHelp.textContent = fullscreenError || (browserMode
      ? 'Browser fullscreen is controlled by F11 or your browser menu.'
      : !supported && !active ? 'Fullscreen is unavailable here. Try your browser’s fullscreen option on desktop.'
      : 'Fill your screen with the game. F11 is the separate browser shortcut on desktop.');
    const quickButton = $('btn-fullscreen');
    if (quickButton) {
      quickButton.textContent = active ? '⛶ Exit fullscreen' : browserMode ? 'F11 to exit fullscreen' : '⛶ Fullscreen';
      quickButton.disabled = fullscreenButton.disabled;
      quickButton.setAttribute('aria-pressed', String(active || browserMode));
    }
  }
  async function toggleFullscreen() {
    if (fullscreenButton.disabled) return;
    capturing = null; fullscreenError = ''; fullscreenPending = true;
    refresh(); refreshFullscreen();
    try {
      // Fullscreen the page so the lobby, HUD and settings remain available.
      // Only this click requests fullscreen; it never resumes game input.
      if (document.fullscreenElement) await document.exitFullscreen();
      else await document.documentElement.requestFullscreen();
    } catch {
      fullscreenError = 'Fullscreen was refused by the browser. Try again, or use F11 on desktop.';
    } finally { fullscreenPending = false; refreshFullscreen(); }
  }
  fullscreenButton.addEventListener('click', toggleFullscreen);
  $('btn-fullscreen')?.addEventListener('click', toggleFullscreen);
  document.addEventListener('fullscreenchange', () => {
    fullscreenError = ''; refreshFullscreen();
    // Keep the modal above a newly fullscreen element in the top layer.
    if (dialog.open) {
      const focused = document.activeElement;
      dialog.close(); dialog.showModal(); focused?.focus?.({ preventScroll: true });
    }
  });
  browserFullscreen?.addEventListener?.('change', refreshFullscreen);
  const buttons = new Map();
  function announce(message) { feedback.textContent = message; }
  function save(message) {
    try {
      root.localStorage.setItem(STORAGE_KEY, JSON.stringify({ version: 1, sensitivity: config.sensitivity, bindings: config.bindings }));
      storageUnavailable = false;
    } catch { storageUnavailable = true; }
    announce(storageUnavailable ? `${message} Active for this visit; browser storage is unavailable.` : `${message} Saved.`);
  }
  function refresh() {
    slider.value = String(config.sensitivity);
    sensitivityValue.textContent = `${config.sensitivity.toFixed(2)}×`;
    slider.setAttribute('aria-valuetext', `${config.sensitivity.toFixed(2)} times default`);
    for (const action of ACTIONS) {
      const button = buttons.get(action.id);
      const labels = config.bindings[action.id].map(keyLabel).join(' / ');
      button.textContent = capturing === action.id ? 'Press a key…' : labels;
      button.setAttribute('aria-pressed', String(capturing === action.id));
      button.setAttribute('aria-label', `Change ${action.label}: ${labels}`);
      button.title = labels;
    }
    resumeButton.textContent = playing ? 'Resume game' : 'Done';
    $('binding-cancel').hidden = !capturing;
    resumeButton.disabled = playing && !document.querySelector('#ember-root canvas');
    $('settings-notice').textContent = playing
      ? 'Your controls are paused. The multiplayer match is still running and other players can hit you.'
      : 'Set up before joining. In multiplayer, opening this menu does not pause the match.';
  }
  for (const action of ACTIONS) {
    const row = document.createElement('div'); row.className = 'binding-row';
    const label = document.createElement('span'); label.textContent = action.label;
    const button = document.createElement('button'); button.type = 'button'; button.className = 'binding-button';
    button.setAttribute('aria-describedby', 'binding-help');
    button.addEventListener('click', () => {
      capturing = action.id; publish(true); refresh();
      announce(`Press a key or mouse button for ${action.label}. Esc cancels.`);
    });
    buttons.set(action.id, button); row.append(label, button); $('binding-grid').append(row);
  }
  function openMenu(message = '') {
    publish(true); pendingLock = false; capturing = null;
    if (!dialog.open) {
      previousFocus = document.activeElement;
      dialog.showModal();
    }
    if (document.pointerLockElement) document.exitPointerLock();
    refresh(); refreshFullscreen(); announce(message || initialNote || (playing ? 'Changes apply immediately. Choose Resume game when ready.' : 'Changes apply immediately. Choose Done when ready.'));
    initialNote = '';
  }
  function closeMenu() {
    capturing = null;
    for (const code of held) blockedUntilRelease.add(code);
    closeGuard = true;
    root.requestAnimationFrame(() => { closeGuard = false; });
    if (dialog.open) dialog.close();
  }
  function resume() {
    // Only this explicit button requests pointer lock. No unlock, focus or
    // settings-change handler may silently recapture the pointer.
    if (!playing) {
      publish(true); closeMenu(); previousFocus?.focus?.(); return;
    }
    const canvas = document.querySelector('#ember-root canvas');
    if (!canvas) { announce('The game is still preparing. Try Resume when it is ready.'); return; }
    if (!canvas.requestPointerLock) { announce('This browser cannot capture the mouse. Use a browser with Pointer Lock support.'); return; }
    publish(false); pendingLock = true; closeMenu();
    canvas.focus();
    try {
      const request = canvas.requestPointerLock();
      request?.catch?.(() => openMenu('Mouse capture was refused. Click Resume game to try again.'));
    } catch { openMenu('Mouse capture was refused. Click Resume game to try again.'); }
  }
  function acceptBinding(code) {
    try {
      const action = ACTIONS.find(item => item.id === capturing);
      config = rebind(config, capturing, code); capturing = null;
      publish(true); refresh(); save(`${action.label}: ${keyLabel(code)}.`);
    } catch (error) { announce(error.message); }
  }
  const stop = (event, prevent = true) => { event.stopImmediatePropagation(); if (prevent) event.preventDefault(); };
  const physicalCode = event => event.type.startsWith('key') ? event.code : `Mouse${event.button}`;
  function inputEvent(event) {
    const code = physicalCode(event), down = event.type === 'keydown' || event.type === 'pointerdown';
    const up = event.type === 'keyup' || event.type === 'pointerup' || event.type === 'pointercancel';
    if (down) held.add(code);
    if (up) held.delete(code);
    if (blockedUntilRelease.has(code)) {
      if (up) blockedUntilRelease.delete(code);
      stop(event); return;
    }
    if (up && ignoredMouseGesture === event.button) {
      const button = event.button;
      root.setTimeout(() => { if (ignoredMouseGesture === button) ignoredMouseGesture = null; }, 0);
    }
    if (closeGuard) { stop(event); return; }
    if (event.type === 'keydown' && event.code === 'Escape') {
      if (dialog.open) {
        if (capturing) { capturing = null; refresh(); announce('Binding unchanged.'); }
        else announce(playing ? 'Choose Resume game to continue.' : 'Choose Done to close settings.');
        stop(event); return;
      }
      if (playing) { openMenu(); stop(event); return; }
    }
    if (dialog.open) {
      const menuAction = event.target?.closest?.('#binding-cancel, #settings-resume, #settings-reset, #settings-fullscreen');
      if (capturing && down && !event.repeat && !menuAction) {
        if (event.altKey || event.ctrlKey || event.metaKey) announce('Browser shortcut combinations cannot be assigned.');
        else acceptBinding(code);
        if (event.type === 'pointerdown') ignoredMouseGesture = event.button;
        stop(event); return;
      }
      // Stop winit's global listeners, but preserve native Tab navigation,
      // range-key editing and primary pointer behavior inside the modal.
      const within = dialog.contains(event.target);
      stop(event, !within || (event.type === 'pointerdown' && event.button !== 0));
      return;
    }
    if (playing && !paused && event.type === 'keydown'
      && Object.values(config.bindings).some(codes => codes.includes(event.code))) event.preventDefault();
  }
  for (const type of ['keydown', 'keyup', 'pointerdown', 'pointerup', 'pointercancel']) root.addEventListener(type, inputEvent, true);
  for (const type of ['pointermove', 'mousemove', 'mousedown', 'mouseup', 'wheel']) root.addEventListener(type, event => {
    if (dialog.open || closeGuard) stop(event, closeGuard || !dialog.contains(event.target));
  }, { capture: true, passive: false });
  for (const type of ['click', 'auxclick']) root.addEventListener(type, event => {
    if (ignoredMouseGesture === event.button || closeGuard) stop(event);
  }, true);
  root.addEventListener('contextmenu', event => {
    if (dialog.open || document.pointerLockElement) stop(event);
  }, true);
  dialog.addEventListener('cancel', event => { event.preventDefault(); capturing = null; refresh(); });
  slider.addEventListener('input', () => {
    const value = Number(slider.value);
    if (!Number.isFinite(value)) return;
    config = { ...config, sensitivity: Math.round(Math.max(0.1, Math.min(3, value)) * 100) / 100 };
    publish(true); refresh(); save(`Sensitivity ${config.sensitivity.toFixed(2)}×.`);
  });
  $('settings-reset').addEventListener('click', () => {
    config = defaults(); capturing = null; publish(true); refresh(); save('Default controls restored.');
  });
  $('binding-cancel').addEventListener('click', () => { capturing = null; refresh(); announce('Binding unchanged.'); });
  resumeButton.addEventListener('click', resume);
  for (const id of ['btn-settings', 'btn-game-settings']) $(id)?.addEventListener('click', () => openMenu());
  document.addEventListener('pointerlockchange', () => {
    if (!playing) return;
    if (document.pointerLockElement && !dialog.open) { pendingLock = false; publish(false); }
    else if (!document.pointerLockElement && !pendingLock) openMenu();
  });
  document.addEventListener('pointerlockerror', () => {
    if (playing) openMenu('Mouse capture was refused. Click Resume game to try again.');
  });
  root.addEventListener('blur', () => { held.clear(); if (playing) openMenu('Game controls are paused while this window is inactive.'); });
  document.addEventListener('visibilitychange', () => { if (document.hidden && playing) openMenu('Game controls are paused while this page is hidden.'); });
  new root.MutationObserver(() => { if (dialog.open) refresh(); }).observe($('ember-root'), { childList: true });
  root.emberArenaSettingsUI = Object.freeze({
    open: () => openMenu(),
    setPlaying(value) {
      playing = Boolean(value);
      document.documentElement.classList.toggle('arena-playing', playing);
      if (playing) openMenu('Choose Resume game to capture your mouse.');
      else { publish(true); pendingLock = false; closeMenu(); }
      refresh();
    },
  });
  refresh(); refreshFullscreen();
})(typeof window === 'undefined' ? null : window);

/* Killshot's presentation-only HUD. Rust sends one authoritative snapshot at
 * most twenty times per second; no ammo/health/reload gameplay is predicted in
 * JavaScript. Keeping this in settings.js preserves the four-file web release.
 */
(function (root) {
  'use strict';
  const WEAPONS = Object.freeze([
    { name: 'Empty', short: '—' }, { name: 'Sidearm', short: 'Sidearm' },
    { name: 'Vityaz', short: 'Vityaz' }, { name: 'AK-47', short: 'AK-47' },
    { name: 'M4', short: 'M4' }, { name: 'Revolver', short: 'Revolver' },
    { name: 'Sniper', short: 'Sniper' }, { name: 'RPG-7', short: 'RPG-7' },
    { name: 'Breach-12', short: 'Breach-12' },
  ].map(weapon => Object.freeze(weapon)));
  const bounded = (value, max, fallback = 0) => typeof value === 'number' && Number.isFinite(value)
    ? Math.min(max, Math.max(0, value)) : fallback;
  const count = value => Math.floor(bounded(value, 9999));
  const weaponId = value => Number.isInteger(value) && value >= 1 && value <= 8 ? value : 0;
  function normalizeHud(raw) {
    try { if (typeof raw === 'string') raw = JSON.parse(raw); } catch { return null; }
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
    const maxHp = Math.max(1, count(raw.max_hp) || 5);
    const alive = raw.alive === true;
    const duration = bounded(raw.reload_duration, 30);
    const remaining = alive && duration > 0 ? bounded(raw.reload_remaining, duration) : 0;
    const slots = Array.from({ length: 9 }, (_, index) => weaponId(raw.slots?.[index]));
    const selected = Number.isInteger(raw.selected_slot) && raw.selected_slot >= 1 && raw.selected_slot <= 9
      ? raw.selected_slot : 0;
    return {
      visible: raw.visible === true, alive, hp: alive ? Math.min(maxHp, count(raw.hp)) : 0,
      max_hp: maxHp, weapon: weaponId(raw.weapon), magazine: count(raw.magazine),
      reserve: raw.reserve === null ? null : count(raw.reserve), slots,
      selected_slot: selected && slots[selected - 1] ? selected : 0,
      reload_remaining: remaining, reload_duration: duration,
      reload_progress: remaining > 0 ? Math.min(100, Math.max(0, (1 - remaining / duration) * 100)) : 100,
    };
  }
  function createHudRenderer(document, getBindings = () => ({})) {
    const $ = id => document.getElementById(id);
    const hud = $('killshot-hud');
    if (!hud) return () => false;
    const slots = [];
    for (let number = 1; number <= 9; number++) {
      const item = document.createElement('div'); item.className = 'ks-slot';
      const key = document.createElement('span'); key.className = 'ks-slot-key';
      const name = document.createElement('span'); name.className = 'ks-slot-name';
      item.append(key, name); $('killshot-slots').append(item);
      slots.push({ item, key, name, number });
    }
    const text = (node, value) => { if (node.textContent !== String(value)) node.textContent = String(value); };
    const attr = (node, name, value) => { if (node.getAttribute(name) !== String(value)) node.setAttribute(name, String(value)); };
    const nodes = Object.fromEntries(['life', 'life-max', 'life-state', 'weapon', 'ammo', 'reserve', 'reload-time', 'reload-ring'].map(name => [name, $(`ks-${name}`)]));
    const reload = $('killshot-reload');
    return raw => {
      const state = normalizeHud(raw);
      hud.hidden = !state?.visible;
      if (!state?.visible) { reload.hidden = true; return Boolean(state); }
      text(nodes.life, state.hp); text(nodes['life-max'], state.max_hp);
      text(nodes['life-state'], state.alive ? 'ALIVE' : 'RESPAWNING');
      text(nodes.weapon, WEAPONS[state.weapon].name.toUpperCase());
      text(nodes.ammo, state.magazine); text(nodes.reserve, state.reserve === null ? '∞' : state.reserve);
      attr(hud, 'data-low-life', state.alive && state.hp <= 2);
      attr(hud, 'data-empty', state.magazine === 0);
      const reloading = state.reload_remaining > 0;
      reload.hidden = !reloading;
      if (reloading) {
        const remaining = state.reload_remaining.toFixed(1);
        text(nodes['reload-time'], remaining);
        attr(reload, 'aria-valuenow', Math.round(state.reload_progress));
        attr(reload, 'aria-valuetext', `Reloading: ${remaining} seconds remaining`);
        nodes['reload-ring'].style.strokeDashoffset = String(100 - state.reload_progress);
      }
      const bindings = getBindings();
      for (const { item, key, name, number } of slots) {
        const weapon = state.slots[number - 1];
        const code = bindings?.[`slot${number}`]?.[0] || `Digit${number}`;
        const keyName = ({ Mouse0: 'M1', Mouse1: 'M3', Mouse2: 'M2', Space: 'SPACE', ShiftLeft: 'LSHFT', ShiftRight: 'RSHFT', ArrowUp: '↑', ArrowDown: '↓', ArrowLeft: '←', ArrowRight: '→' })[code] || code.replace(/^Key|^Digit/, '');
        text(key, keyName); text(name, WEAPONS[weapon].short);
        attr(item, 'data-owned', weapon !== 0); attr(item, 'data-selected', state.selected_slot === number);
        attr(item, 'aria-label', `Slot ${number}, ${WEAPONS[weapon].name}, key ${keyName}${state.selected_slot === number ? ', selected' : ''}`);
      }
      return true;
    };
  }
  if (typeof module === 'object' && module.exports) Object.assign(module.exports, { WEAPONS, normalizeHud, createHudRenderer });
  if (!root?.document) return;
  root.emberKillshotWeapons = WEAPONS;
  root.emberUpdateKillshotHud = createHudRenderer(root.document, () => root.__emberArenaSettings?.bindings);
})(typeof window === 'undefined' ? null : window);
