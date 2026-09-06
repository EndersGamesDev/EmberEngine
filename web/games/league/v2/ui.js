import { chooseHost, listLobbies, renderChip } from '../../../hosts.js';

const ROOT = new URL('../../../', location);
const $ = (id) => document.getElementById(id);
const esc = (value) => String(value ?? '').replace(/[&<>"']/g, (c) => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const readSaved = (key, fallback, store = localStorage) => { try { return JSON.parse(store.getItem(key)) ?? fallback; } catch { return fallback; } };
const save = (key, value) => { try { localStorage.setItem(key, JSON.stringify(value)); } catch {} };
const fmtTime = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;

const ART = ['art/swarm.webp', 'art/emberknight.webp', 'art/hallow.webp', 'art/bogmaw.webp', 'art/tessera.webp'];
const EMBLEM = ['◈', '♜', '✦', '⚓', '⌛'];
const CHAMP_RGB = (c) => c.colour.map((x) => Math.round(x * 255)).join(',');

let resolveLeagueReady;
window.leagueReady = new Promise((resolve) => { resolveLeagueReady = resolve; });
document.body.dataset.leagueBoot = 'loading';

let wasm = null, PROTO = 0, DATA = null;
let chosen = null, candidates = [], wrongProto = [];
let launched = false, mode = 3, shopOpen = false, lastPhase = '', local = false, latest = null, lastAim = null, lastShop = '', pendingPick = null;

try {
  const m = await import('./pkg/league.js');
  await m.default();
  wasm = m;
  PROTO = wasm.proto_version();
  DATA = JSON.parse(wasm.data_json());
} catch (e) {
  wasm = null;
  console.error(e);
  const n = $('engine-note');
  n.classList.add('bad');
  n.textContent = 'The game could not load. Reload the page to try again.';
  for (const id of ['btn-practice', 'btn-practice3', 'btn-create', 'btn-quick']) $(id).disabled = true;
}

// ---- keybindings -----------------------------------------------------------

const ACTIONS = [
  ['q', 'Q ability'], ['w', 'W ability'], ['e', 'E ability'], ['r', 'R ability'],
  ['d', 'D spell'], ['f', 'F spell'],
  ['item1', 'Item 1'], ['item2', 'Item 2'], ['item3', 'Item 3'],
  ['item4', 'Item 4'], ['item5', 'Item 5'], ['item6', 'Item 6'],
  ['stop', 'Stop moving'],
];
const DEFAULT_KEYS = {
  q: 'KeyQ', w: 'KeyW', e: 'KeyE', r: 'KeyR',
  d: 'KeyD', f: 'KeyF',
  item1: 'Digit1', item2: 'Digit2', item3: 'Digit3',
  item4: 'Digit4', item5: 'Digit5', item6: 'Digit6',
  stop: 'KeyS',
};
const RESERVED = new Set(['KeyB', 'Escape', 'ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight']);
const RESERVED_MSG = {
  KeyB: 'B is fixed as the shop key and cannot be remapped.',
  Escape: 'Esc is fixed for menus and cannot be remapped.',
  ShiftLeft: 'Shift is the rank modifier and cannot be remapped.',
  ShiftRight: 'Shift is the rank modifier and cannot be remapped.',
  ControlLeft: 'Ctrl is the rank modifier and cannot be remapped.',
  ControlRight: 'Ctrl is the rank modifier and cannot be remapped.',
};

function sanitizeKeys(raw) {
  const out = { ...DEFAULT_KEYS };
  if (raw && typeof raw === 'object' && !Array.isArray(raw)) {
    for (const act of Object.keys(DEFAULT_KEYS)) {
      const v = raw[act];
      if (typeof v === 'string' && v.length > 0 && v.length < 32) out[act] = v;
    }
  }
  return out;
}
let keys = sanitizeKeys(readSaved('ember-league-v2-keys', {}));
const actionLabel = (act) => (ACTIONS.find(([a]) => a === act) ?? [act, act])[1];

let keyOpts = null;
function optionLabels() {
  if (!keyOpts && wasm?.binding_options_json) {
    try {
      const v = JSON.parse(wasm.binding_options_json());
      if (Array.isArray(v)) keyOpts = Object.fromEntries(v.map((o) => [o.code, o.label]));
      else if (v && typeof v === 'object') keyOpts = Object.fromEntries(Object.entries(v).map(([c, o]) => [c, o && typeof o === 'object' ? o.label : o]));
    } catch { keyOpts = null; }
  }
  return keyOpts;
}
function keyLabel(code) {
  const opts = optionLabels();
  if (opts?.[code]) return opts[code];
  if (code.startsWith('Key')) return code.slice(3);
  if (code.startsWith('Digit')) return code.slice(5);
  if (code.startsWith('Numpad')) return code.slice(6);
  return code;
}
const hasBindingApi = () => typeof wasm?.set_bindings_json === 'function';
const setEnabled = (v) => { try { wasm?.set_input_enabled?.(v); } catch {} };

let keysOpen = false, listening = null, conflict = null;
const setKeyStatus = (text, bad = false) => {
  const el = $('key-status');
  el.textContent = text || '';
  el.classList.toggle('bad', bad);
};
function applyKeys(partial) {
  if (!hasBindingApi()) throw new Error('the engine binding update is not shipped in this build yet');
  wasm.set_bindings_json(JSON.stringify(partial));
}
const persistKeys = () => save('ember-league-v2-keys', keys);

function syncEngine() {
  if (!hasBindingApi()) {
    setKeyStatus('The engine binding update is not in this build yet — default keys are active.');
    return;
  }
  try {
    applyKeys({ ...keys });
    setKeyStatus('');
  } catch (e) {
    setKeyStatus(`Engine rejected the saved bindings: ${String(e.message ?? e)}. Defaults are active.`, true);
  }
}

function paintKeys() {
  $('key-rows').innerHTML = ACTIONS.map(([act, label]) => `
    <div class="krow${listening === act ? ' listen' : ''}" data-act="${act}">
      <span class="klabel">${label}</span>
      <span class="kchip">${listening === act ? 'press…' : esc(keyLabel(keys[act]))}</span>
      <button class="kset" data-act="${act}">${listening === act ? 'listening' : 'change'}</button>
    </div>`).join('');
  $('key-rows').querySelectorAll('.kset').forEach((b) => { b.onclick = () => startListening(b.dataset.act); });
  const cf = $('key-conflict');
  if (conflict) {
    cf.classList.remove('hidden');
    $('key-conflict-text').textContent = `${keyLabel(conflict.code)} is already bound to ${actionLabel(conflict.other)}.`;
  } else {
    cf.classList.add('hidden');
  }
}
function startListening(act) {
  listening = act;
  conflict = null;
  setKeyStatus(`Press any key for ${actionLabel(act)}…  (Esc cancels)`);
  paintKeys();
}
function cancelListening() {
  listening = null;
  setKeyStatus('');
  paintKeys();
}
function bindAction(act, code) {
  if (keys[act] === code) {
    listening = null;
    setKeyStatus(`${actionLabel(act)} is already on ${keyLabel(code)}.`);
    paintKeys();
    return;
  }
  const old = keys[act];
  keys[act] = code;
  try {
    applyKeys({ [act]: code });
    persistKeys();
    setKeyStatus(`${actionLabel(act)} → ${keyLabel(code)}`);
  } catch (e) {
    keys[act] = old;
    setKeyStatus(`Engine rejected: ${String(e.message ?? e)}`, true);
  }
  listening = null;
  paintKeys();
  paintHudKeys();
}
function resolveConflict(take) {
  if (!conflict) return;
  const { act, code, other } = conflict;
  const oldAct = keys[act];
  if (take) {
    keys[act] = code;
    keys[other] = oldAct;
    try {
      applyKeys({ [act]: code, [other]: oldAct });
      persistKeys();
      setKeyStatus(`Swapped: ${actionLabel(act)} → ${keyLabel(code)}, ${actionLabel(other)} → ${keyLabel(oldAct)}.`);
    } catch (e) {
      keys[act] = oldAct;
      setKeyStatus(`Engine rejected the swap: ${String(e.message ?? e)}`, true);
    }
  } else {
    setKeyStatus('Kept the previous bindings.');
  }
  conflict = null;
  paintKeys();
  paintHudKeys();
}
function resetKeys() {
  keys = { ...DEFAULT_KEYS };
  persistKeys();
  listening = null;
  conflict = null;
  try {
    applyKeys({});
    setKeyStatus('Reset to defaults.');
  } catch (e) {
    setKeyStatus(`Engine rejected the reset: ${String(e.message ?? e)}`, true);
  }
  paintKeys();
  paintHudKeys();
}
function paintHudKeys() {
  const abilKey = ['q', 'w', 'e', 'r'], spellKey = ['d', 'f'];
  document.querySelectorAll('#abils .slot').forEach((el) => {
    const k = el.querySelector('.key');
    if (k) k.textContent = keyLabel(keys[abilKey[Number(el.dataset.abil)]]);
  });
  document.querySelectorAll('#spells .slot').forEach((el) => {
    const k = el.querySelector('.key');
    if (k) k.textContent = keyLabel(keys[spellKey[Number(el.dataset.spell)]]);
  });
  document.querySelectorAll('#islots .slot').forEach((el) => {
    const k = el.querySelector('.key');
    if (k) k.textContent = keyLabel(keys[`item${Number(el.dataset.item) + 1}`]);
  });
  renderHelpKeys();
}
function openKeys() {
  keysOpen = true;
  listening = null;
  conflict = null;
  syncEngine();
  paintKeys();
  $('keys').classList.remove('hidden');
  setEnabled(false);
}
function closeKeys() {
  if (!keysOpen) return;
  keysOpen = false;
  listening = null;
  conflict = null;
  $('keys').classList.add('hidden');
  setKeyStatus(hasBindingApi() ? '' : 'The engine binding update is not in this build yet — default keys are active.');
  setEnabled(true);
}
$('btn-keys').onclick = openKeys;
$('btn-keys2').onclick = openKeys;
$('btn-keys3').onclick = openKeys;
$('btn-keys-close').onclick = closeKeys;
$('btn-keys-reset').onclick = resetKeys;
$('btn-conflict-take').onclick = () => resolveConflict(true);
$('btn-conflict-keep').onclick = () => resolveConflict(false);

addEventListener('keydown', (e) => {
  if (!keysOpen) return;
  e.preventDefault();
  e.stopPropagation();
  if (listening) {
    if (e.code === 'Escape') { cancelListening(); return; }
    if (RESERVED.has(e.code)) {
      listening = null;
      setKeyStatus(RESERVED_MSG[e.code], true);
      paintKeys();
      return;
    }
    const other = Object.keys(keys).find((a) => a !== listening && keys[a] === e.code);
    if (other) {
      conflict = { act: listening, code: e.code, other };
      listening = null;
      setKeyStatus('');
      paintKeys();
      return;
    }
    bindAction(listening, e.code);
    return;
  }
  if (e.code === 'Escape') closeKeys();
}, { capture: true });

// ---- page keys (shop, menus, scroll) ---------------------------------------

const typing = (e) => e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT');
addEventListener('keydown', (e) => {
  if (keysOpen) return;
  if (typing(e)) return;
  if ([' ', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.key)) e.preventDefault();
  if (e.repeat || !launched) return;
  if (e.code === 'KeyB') { e.preventDefault(); setShop(!shopOpen); return; }
  if (e.code === 'Escape') {
    if (!$('help').classList.contains('hidden')) { closeOverlay('help'); return; }
    if (!$('pause').classList.contains('hidden')) { closePause(); return; }
    if (shopOpen) { setShop(false); return; }
    if (latest?.phase === 'live') openPause();
  }
});

document.addEventListener('pointermove', (e) => {
  if (!e.target.matches('#ember-root canvas')) return;
  const r = e.target.getBoundingClientRect();
  lastAim = [2 * (e.clientX - r.left) / r.width - 1, 1 - 2 * (e.clientY - r.top) / r.height];
});
document.addEventListener('contextmenu', (e) => { if (e.target.matches('#ember-root canvas')) e.preventDefault(); });

// ---- menus ------------------------------------------------------------------

const showStatus = (text) => { $('status-text').textContent = text; $('status').classList.toggle('hidden', !text); };
$('btn-leave').onclick = $('btn-recover').onclick = $('btn-leave2').onclick = () => location.reload();
const fail = (msg) => showStatus('Unable to start the game: ' + (msg || 'see browser console'));
window.addEventListener('error', (e) => fail(e.message));
window.addEventListener('unhandledrejection', (e) => fail(e.reason));

function openPause() { $('pause').classList.remove('hidden'); setEnabled(false); }
function closePause() { $('pause').classList.add('hidden'); setEnabled(true); }
function openOverlay(id) { $(id).classList.remove('hidden'); setEnabled(false); }
function closeOverlay(id) { $(id).classList.add('hidden'); setEnabled(true); }
$('btn-resume').onclick = closePause;
$('btn-help').onclick = () => openOverlay('help');
$('btn-help-close').onclick = () => closeOverlay('help');

function renderHelpKeys() {
  const k = (a) => `<b>${esc(keyLabel(keys[a]))}</b>`;
  $('help-keys').innerHTML = `
    <li>Click the ground to walk; click an enemy to attack it.</li>
    <li>${k('q')} ${k('w')} ${k('e')} ${k('r')} cast at the cursor.</li>
    <li>${k('d')} ${k('f')} summoner spells at the cursor.</li>
    <li>${k('item1')}–${k('item6')} use item slots (potions drink).</li>
    <li>${k('stop')} stops moving.</li>
    <li><b>Shift</b>/<b>Ctrl</b> + ability key ranks it up; the <b>+</b> on a slot does the same.</li>
    <li><b>B</b> opens the shop (fixed). <b>Esc</b> closes the shop or opens this menu.</li>`;
}

// ---- shop -------------------------------------------------------------------

function setShop(open) {
  shopOpen = !!open && latest?.phase === 'live';
  $('shop').classList.toggle('hidden', !shopOpen);
  $('btn-shop').textContent = shopOpen ? 'Close shop [B]' : 'Shop [B]';
  wasm_cmd({ shop: shopOpen });
  if (shopOpen) renderShop(latest?.me);
}
$('btn-shop').onclick = () => setShop(!shopOpen);

// ---- engine commands --------------------------------------------------------

function wasm_cmd(obj) {
  try { wasm?.cmd_json(JSON.stringify(obj)); } catch (e) { console.error(e); }
}
function aimedCommand(kind, slot) {
  const c = document.querySelector('#ember-root canvas');
  wasm_cmd({ [kind]: slot, aim: lastAim, aspect: c ? c.clientWidth / c.clientHeight : 16 / 9 });
}

// ---- lobby ------------------------------------------------------------------

const handleValue = () => {
  const h = ($('handle').value || '').trim().slice(0, 20) || 'summoner';
  $('handle').value = h;
  try { localStorage.setItem('ember-league-handle', h); } catch {}
  return h;
};
try { $('handle').value = localStorage.getItem('ember-league-handle') || 'summoner'; } catch { $('handle').value = 'summoner'; }

async function discover() {
  try {
    const r = await chooseHost(ROOT.href, { game: 'league', proto: PROTO || null });
    if (launched) return;
    chosen = r.chosen;
    candidates = r.candidates;
    wrongProto = r.wrongProto || [];
    for (const id of ['btn-create', 'btn-quick']) $(id).disabled = !wasm || !(chosen || candidates[0]);
    renderChip($('host-chip'), chosen, { wrongProto, proto: PROTO });
  } catch (e) { console.error(e); }
}

async function showLobbies() {
  const list = $('lobbies');
  if (!candidates.length) {
    $('lobby-note').textContent = 'no league server is published right now — practice mode still works';
    list.innerHTML = '';
    return;
  }
  const lists = await Promise.all(candidates.map((c) => listLobbies(c.url, { proto: PROTO || 0 }).catch(() => [])));
  if (launched) return;
  const all = [];
  candidates.forEach((host, i) => { for (const l of lists[i] || []) all.push({ host, lobby: l }); });
  list.innerHTML = '';
  if (!all.length) $('lobby-note').textContent = 'no open lobbies — create one';
  for (const { host, lobby: l } of all) {
    const li = document.createElement('li');
    const go = document.createElement('button');
    go.className = 'go';
    go.textContent = l.racing ? 'in match' : l.players >= l.cap ? 'full' : 'join';
    go.disabled = !!l.racing || l.players >= l.cap || !wasm;
    go.onclick = () => {
      const pass = l.has_password ? prompt(`Password for "${l.name}"`) : null;
      if (l.has_password && pass === null) return;
      mode = l.mode || 3;
      launchOnline(l.name, pass, false, host);
    };
    const name = document.createElement('b');
    name.textContent = (l.name || '?') + (l.has_password ? ' 🔒' : '');
    const meta = document.createElement('span');
    meta.className = 'meta';
    meta.textContent = `${l.players}/${l.cap} · mode ${l.mode}v${l.mode} · host ${l.host || '?'}${l.racing ? ' · in match' : ''}`;
    const tag = document.createElement('span');
    tag.className = 'pill';
    tag.textContent = host.name || 'server';
    tag.title = host.url || '';
    li.append(name, meta, tag, go);
    list.appendChild(li);
  }
}

function onlyHost(url) {
  const entry = { name: '', league_ws: url, league_version: '', league_commit: '', updated: '' };
  return { chosen: { ...entry, url }, candidates: [{ ...entry, url }], wrongProto: [] };
}

function launchOnline(lobby, password, create, host) {
  if (!wasm || launched) return;
  launched = true;
  local = false;
  $('stage').classList.add('drafting');
  mode = mode === 1 ? 1 : 3;
  $('menu').classList.add('hidden');
  $('draft').classList.remove('hidden');
  try {
    wasm.start_online(JSON.stringify({
      ws: host.url, handle: handleValue(), lobby,
      password: password || '', create: !!create, mode,
    }));
  } catch (e) {
    launched = false;
    $('menu').classList.remove('hidden');
    $('draft').classList.add('hidden');
    $('stage').classList.remove('drafting');
    const n = $('lobby-note');
    n.classList.add('bad');
    n.textContent = String(e);
    return;
  }
  pollHud();
}

$('btn-create').onclick = () => {
  const name = ($('newlobby').value || '').trim();
  if (!name) { $('lobby-note').textContent = 'name the lobby first'; return; }
  const host = chosen || candidates[0];
  if (!host) { $('lobby-note').textContent = 'no server to host on'; return; }
  mode = Number($('newmode').value);
  launchOnline(name, $('newpass').value || '', true, host);
};
$('btn-refresh').onclick = async () => { await discover(); await showLobbies(); };
$('btn-practice').onclick = () => launchLocal(1);
$('btn-practice3').onclick = () => launchLocal(3);
$('btn-quick').onclick = async () => {
  const host = chosen || candidates[0];
  if (!host) { $('lobby-note').textContent = 'no server right now'; return; }
  const rows = await listLobbies(host.url, { proto: PROTO });
  const open = (rows || []).filter((l) => !l.racing && !l.has_password && l.players < l.cap && l.mode === Number($('newmode').value));
  if (!open.length) {
    mode = Number($('newmode').value);
    launchOnline('Quick ' + Math.random().toString(36).slice(2, 8), '', true, host);
    return;
  }
  mode = open[0].mode;
  launchOnline(open[0].name, null, false, host);
};

function launchLocal(m) {
  if (!wasm || launched) return;
  launched = true;
  local = true;
  mode = m;
  $('stage').classList.add('drafting');
  $('menu').classList.add('hidden');
  $('draft').classList.remove('hidden');
  try { wasm.start_local(m); pollHud(); } catch (e) { fail(e); }
}

// ---- draft ------------------------------------------------------------------

let mySlot = 0, roster = [], picked = null, dSel = 0, fSel = 1, runes = [0, 1, 2];
let pages = readSaved('ember-league-pages', { A: [0, 1, 2], B: [3, 4, 0], C: [5, 6, 7] });
if (!pages || typeof pages !== 'object' || Array.isArray(pages)) pages = {};
let curPage = 'A';
let cardsBuilt = false;

function buildDraftChrome() {
  const opt = (v, t) => `<option value="${v}">${t}</option>`;
  $('pickd').innerHTML = DATA.spells.map((s, i) => opt(i, s.name)).join('');
  $('pickf').innerHTML = DATA.spells.map((s, i) => opt(i, s.name)).join('');
  $('runebox').innerHTML = DATA.runes.map((r, i) =>
    `<button class="rune" data-r="${i}" title="${esc(r.desc)}">${esc(r.name)}</button>`).join('');
  $('runebox').querySelectorAll('.rune').forEach((el) => {
    el.onclick = () => {
      const i = Number(el.dataset.r);
      runes = runes.includes(i) ? runes.filter((x) => x !== i) : [...(runes.length === 3 ? runes.slice(1) : runes), i];
      pages[curPage] = [...runes];
      save('ember-league-pages', pages);
      sendPick();
      paintRunes();
    };
  });
  for (const p of ['A', 'B', 'C']) {
    $('pg' + p).onclick = () => { curPage = p; runes = cleanRunes(pages[p] || [0, 1, 2]); sendPick(); paintRunes(); };
  }
  $('btn-start').onclick = () => { sendPick(); wasm_cmd({ start: true }); };
  $('pickd').onchange = () => { dSel = Number($('pickd').value); if (dSel === fSel) fSel = (dSel + 1) % DATA.spells.length; $('pickf').value = fSel; sendPick(); };
  $('pickf').onchange = () => { fSel = Number($('pickf').value); if (fSel === dSel) dSel = (fSel + 1) % DATA.spells.length; $('pickd').value = dSel; sendPick(); };
  runes = cleanRunes(pages.A || [0, 1, 2]);
  $('pickd').value = dSel;
  $('pickf').value = fSel;
}
function cleanRunes(values) {
  return Array.isArray(values) ? [...new Set(values)].filter((n) => Number.isInteger(n) && n >= 0 && n < DATA.runes.length).slice(0, 3) : [];
}
function paintRunes() {
  for (const p of ['A', 'B', 'C']) $('pg' + p).classList.toggle('go', curPage === p);
  $('runebox').querySelectorAll('.rune').forEach((el) => el.classList.toggle('on', runes.includes(Number(el.dataset.r))));
}
function sendPick() {
  if (picked === null || runes.length !== 3 || !launched) return;
  pendingPick = { champ: picked, d: dSel, f: fSel, runes: [...runes], at: performance.now() };
  wasm_cmd({ pick: { champ: picked, d: dSel, f: fSel, runes } });
}

function buildCards() {
  $('cards').innerHTML = DATA.champs.map((c, i) => `
    <button class="card" data-c="${i}">
      <span class="art"><span class="emb" style="color:rgb(${CHAMP_RGB(c)})" aria-hidden="true">${EMBLEM[i]}</span>
      <img src="${ART[i]}" alt="" onerror="this.style.display='none'"></span>
      <b style="color:rgb(${CHAMP_RGB(c)})">${c.name}</b>
      <span class="title">${c.title}</span>
      <div class="kit"><b>Q</b> ${c.q.name} — ${c.q.desc}<br><b>W</b> ${c.w.name} — ${c.w.desc}<br>
      <b>E</b> ${c.e.name} — ${c.e.desc}<br><b>R</b> ${c.r.name} — ${c.r.desc}</div>
    </button>`).join('');
  $('cards').querySelectorAll('.card').forEach((el) => {
    el.onclick = () => { if (el.disabled) return; picked = Number(el.dataset.c); buildCardsSel(); buildDetail(); sendPick(); };
  });
  cardsBuilt = true;
}
function buildCardsSel() {
  if (!cardsBuilt) return;
  const team = roster.find((r) => r.slot === mySlot)?.team;
  const taken = new Set(roster.filter((r) => r.team === team && r.picked && r.slot !== mySlot).map((r) => r.champ));
  $('cards').querySelectorAll('.card').forEach((el) => {
    const i = Number(el.dataset.c);
    el.classList.toggle('sel', i === picked);
    el.classList.toggle('taken', taken.has(i));
    el.disabled = taken.has(i);
    el.setAttribute('aria-pressed', String(i === picked));
  });
}
function buildDetail() {
  const box = $('detail');
  if (picked === null || !DATA.champs[picked]) {
    box.innerHTML = '<p class="note">Pick a champion to inspect their kit.</p>';
    return;
  }
  const c = DATA.champs[picked];
  const row = (k, a) => `<div><b>${k}</b> ${a.name} <span class="cd">${a.cd.join(' / ')}s</span> — ${a.desc}</div>`;
  box.innerHTML = `
    <div class="dport"><span class="emb" style="color:rgb(${CHAMP_RGB(c)})" aria-hidden="true">${EMBLEM[picked]}</span>
      <img src="${ART[picked]}" alt="" onerror="this.style.display='none'"></div>
    <div>
      <h3 class="dname" style="color:rgb(${CHAMP_RGB(c)})">${c.name}</h3>
      <p class="dtitle">${c.title}</p>
      <div class="dstat">
        <span>HP <b>${c.hp}</b></span><span>Mana <b>${c.mana}</b></span><span>Move <b>${c.ms}</b></span>
        <span>Damage <b>${c.ad}</b></span><span>Ability power <b>${c.ap}</b></span>
        <span>Range <b>${c.range}</b></span><span>Attack cd <b>${c.atkCd}s</b></span>
      </div>
      <div class="dkit">${row('Q', c.q)}${row('W', c.w)}${row('E', c.e)}${row('R', c.r)}</div>
    </div>`;
}

// ---- HUD --------------------------------------------------------------------

const mini = $('mini'), mctx = mini.getContext('2d');

function render(h) {
  latest = h;
  mySlot = h.slot ?? 0;
  showStatus(h.notice || (!local && !h.connected ? 'Connecting to the server…' : ''));
  $('stage-tools').classList.toggle('hidden', h.phase === 'select');
  $('stage').classList.toggle('drafting', h.phase === 'select');
  if (h.phase === 'select') {
    $('menu').classList.add('hidden');
    $('stage').classList.remove('hidden');
    $('result').classList.add('hidden');
    if (lastPhase !== 'select') { setShop(false); picked = null; pendingPick = null; }
    $('draft').classList.remove('hidden');
    if (!cardsBuilt) { buildCards(); buildDetail(); }
    roster = h.roster || [];
    mySlot = h.slot ?? 0;
    const me = roster.find((r) => r.slot === mySlot);
    if (pendingPick && me?.picked && me.champ === pendingPick.champ && me.d === pendingPick.d && me.f === pendingPick.f && JSON.stringify(me.runes) === JSON.stringify(pendingPick.runes)) pendingPick = null;
    if (pendingPick && (h.notice || performance.now() - pendingPick.at > 3000)) { pendingPick = null; picked = me?.picked ? me.champ : null; }
    if (me?.picked && !pendingPick && runes.length === 3) {
      picked = me.champ;
      dSel = me.d;
      fSel = me.f;
      runes = cleanRunes(me.runes);
    }
    $('pickd').value = dSel;
    $('pickf').value = fSel;
    $('draft-left').textContent = Math.ceil(h.left || 0) + 's';
    $('teams').innerHTML = [0, 1].map((t) => {
      const row = roster.filter((r) => r.team === t).map((r) => {
        const who = r.handle + (r.bot ? ' [bot]' : '') + (r.picked ? ' — ' + (DATA.champs[r.champ]?.name ?? '?') : ' — picking…');
        return `<li>${esc(who)}${r.slot === mySlot ? ' (you)' : ''}</li>`;
      }).join('');
      return `<div><b style="color:${t ? 'var(--red)' : 'var(--blue)'}">${t ? 'RED' : 'BLUE'}</b><ul>${row}</ul></div>`;
    }).join('');
    const host = roster.filter((r) => !r.bot && r.connected).reduce((min, r) => Math.min(min, r.slot), Infinity);
    $('btn-start').disabled = !me || mySlot !== host || picked === null || runes.length !== 3 || (!local && !h.connected);
    $('draft-note').textContent = h.notice || (runes.length !== 3 ? `Choose ${3 - runes.length} more runes.`
      : picked === null ? 'Pick a champion. Each team may pick a champion once; rivals may mirror picks.'
      : mySlot !== host ? 'Ready. The lobby host can start; empty seats become bots.'
      : 'Ready to start. Empty seats become bots; the countdown starts automatically.');
    buildCardsSel();
    paintRunes();
    lastPhase = 'select';
    return;
  }
  if (lastPhase !== h.phase) {
    setShop(false);
    closePause();
    if (h.phase === 'live') requestAnimationFrame(() => document.querySelector('#ember-root canvas')?.focus({ preventScroll: true }));
  }
  $('draft').classList.add('hidden');
  $('menu').classList.add('hidden');
  $('stage').classList.remove('hidden');
  $('result').classList.toggle('hidden', h.phase !== 'over');
  if (h.phase === 'over') {
    $('result-who').textContent = h.winner === 0 ? 'BLUE wins' : 'RED wins';
    $('result-who').style.color = h.winner === 0 ? 'var(--blue)' : 'var(--red)';
    $('result-stats').textContent = `kills ${h.kills[0]} — ${h.kills[1]}`;
    $('result-countdown').textContent = `Next draft in ${Math.ceil(h.left || 0)}s`;
  }
  $('timer').textContent = fmtTime(h.secs || 0);
  $('kills').textContent = `${h.kills?.[0] ?? 0} : ${h.kills?.[1] ?? 0}`;
  const cores = h.cores || [];
  const paintCore = (i, t) => {
    const c = cores.find((x) => x.t === t);
    if (!c) return;
    const fill = i === 0 ? $('corehp0') : $('corehp1');
    const txt = i === 0 ? $('coretxt0') : $('coretxt1');
    fill.style.width = `${Math.max(0, 100 * c.hp / Math.max(1, c.mh))}%`;
    txt.textContent = `${Math.max(0, Math.round(c.hp))}/${Math.round(c.mh)}`;
  };
  paintCore(0, 0);
  paintCore(1, 1);
  $('boons').textContent = (h.boon || []).map((b, i) =>
    b ? `${b === 1 ? '⚔' : '⏲'} ${fmtTime(h.boonLeft?.[i] || 0)}` : '').join('  ').trim();
  const me = h.me && typeof h.me === 'object' ? h.me : null;
  if (me) {
    const champ = me.champ ?? 0;
    const img = $('portrait');
    if (img.getAttribute('src') !== ART[champ]) img.src = ART[champ];
    $('portrait-emb').textContent = EMBLEM[champ];
    $('hp').style.width = `${Math.max(0, 100 * me.hp / Math.max(1, me.mh))}%`;
    $('mn').style.width = `${Math.max(0, 100 * me.mn / Math.max(1, me.mm))}%`;
    $('vitals').textContent = `${Math.ceil(me.hp)} HP · ${Math.floor(me.mn)} mana`;
    $('lvl').textContent = `lv ${me.lv} · ${me.g} g` + (me.pt ? ` · ${me.pt} pts` : '');
    $('respawn').classList.toggle('hidden', me.alive);
    if (!me.alive) $('respawn').textContent = `respawn ${Math.ceil(me.resp)}s`;
    document.querySelectorAll('#abils .slot').forEach((el) => {
      const i = Number(el.dataset.abil);
      const rank = me.rk[i];
      const ability = DATA.champs[champ]?.[['q', 'w', 'e', 'r'][i]];
      el.classList.toggle('unlearned', !rank);
      el.title = `${['Q', 'W', 'E', 'R'][i]} · ${ability?.name || ''} — ${ability?.desc || ''} · ${ability?.mana?.[Math.max(0, rank - 1)] ?? '?'} mana · rank ${rank}/3. Click to cast at your last field cursor; + or rank-modifier + key ranks up.`;
      el.querySelector('.rk').textContent = rank ? '●'.repeat(rank) : '';
      const cd = el.querySelector('.cd');
      if (!rank) { cd.classList.remove('hidden'); cd.textContent = i === 3 ? 'Lv 6' : 'Learn'; }
      else if (me.cd[i] > 0.05) { cd.classList.remove('hidden'); cd.textContent = me.cd[i].toFixed(1); }
      else cd.classList.add('hidden');
      el.querySelector('.up').classList.toggle('hidden', !(me.pt > 0 && me.rk[i] < 3 && (i !== 3 || me.lv >= (DATA.ultimateLevels || [6, 9, 12])[me.rk[i]])));
    });
    document.querySelectorAll('#spells .slot').forEach((el) => {
      const i = Number(el.dataset.spell);
      const id = i === 0 ? me.d : me.f;
      el.querySelector('.face').textContent = i === 0 ? 'D' : 'F';
      el.title = `${DATA.spells[id]?.name} — ${DATA.spells[id]?.desc}`;
      const cd = el.querySelector('.cd');
      if (me.scd[i] > 0.5) { cd.classList.remove('hidden'); cd.textContent = Math.ceil(me.scd[i]); }
      else cd.classList.add('hidden');
    });
    document.querySelectorAll('#islots .slot').forEach((el) => {
      const i = Number(el.dataset.item);
      const it = DATA.items.find((x) => x.id === me.items[i]);
      el.innerHTML = `<span class="key">${el.querySelector('.key')?.textContent ?? ''}</span>${it ? esc(it.name.split(' ').map((x) => x[0]).join('').slice(0, 2)) : '·'}${it?.charges ? `<small>${me.charges[i]}</small>` : ''}`;
      el.title = it ? `${it.name}${it.charges ? ' x' + me.charges[i] : ''}` : 'empty';
    });
    const st = me.stats;
    $('stats').textContent = st ? `AD ${Math.round(st.ad)} · AP ${Math.round(st.ap)} · attacks/s ${st.attackSpeed.toFixed(2)} · crit ${Math.round(st.crit * 100)}% / ${Math.round(st.critd)}% · haste ${Math.round(st.haste * 100)}%` : '';
  }
  if (shopOpen) renderShop(me);
  $('score-body').innerHTML = (h.roster || []).map((r) => {
    const name = (r.handle || '?') + (r.bot ? ' [bot]' : '');
    return `<tr><td style="color:${r.team ? 'var(--red)' : 'var(--blue)'}">${esc(name)}</td><td>${DATA.champs[r.champ]?.name ?? '—'}</td></tr>`;
  }).join('');
  $('feed').classList.toggle('hidden', !(h.feed || []).length);
  $('feed').innerHTML = (h.feed || []).map((t) => `• ${esc(t)}`).join('<br>');
  drawMini(h);
  lastPhase = h.phase;
}

function renderShop(me) {
  const s = $('shop');
  if (!me) { s.textContent = 'The shop needs a live match.'; return; }
  const signature = JSON.stringify([me.g, me.alive, me.items, me.charges]);
  if (signature === lastShop) return;
  lastShop = signature;
  s.innerHTML = `<h4>Item shop</h4> <span class="kbd">${me.g} gold · buy anywhere · B closes</span><p class="note">Permanent items grant passive stats. Use potion slots with your item keys.</p>` + DATA.items.map((it) => {
    const owned = me.items.includes(it.id);
    const refill = it.charges > 0 && me.items.some((id, i) => id === it.id && me.charges[i] < it.charges);
    const room = me.items.includes(0) || refill;
    const can = me.alive && me.g >= it.cost && room && (!owned || it.charges > 0);
    const label = !me.alive ? 'Respawning' : owned && !it.charges ? 'Owned' : !room ? 'Full' : `${it.cost}g`;
    return `<div class="it"><button ${can ? '' : 'disabled'} data-buy="${it.id}">${refill ? 'Refill ' : ''}${label}</button><span class="d"><b>${esc(it.name)}</b> <span class="tier">TIER ${it.tier}</span><br>${esc(it.desc)}</span></div>`;
  }).join('');
  s.querySelectorAll('[data-buy]').forEach((b) => { b.onclick = () => wasm_cmd({ buy: Number(b.dataset.buy) }); });
}

function drawMini(h) {
  mctx.clearRect(0, 0, mini.width, mini.height);
  mctx.fillStyle = '#0d1017';
  mctx.fillRect(0, 0, mini.width, mini.height);
  const X = (x) => ((x + 70) / 140) * mini.width;
  const Z = (z) => ((z + 40) / 80) * mini.height;
  mctx.fillStyle = '#1d2233';
  mctx.fillRect(X(-70), Z(-7), mini.width, Z(7) - Z(-7));
  for (const u of (h.units || [])) {
    const [k, t, x, z, hp, slot] = u;
    if (k === 3) continue;
    mctx.fillStyle = t === 2 ? '#b59a4a' : (t === 0 ? '#6ea0ff' : '#ff7a70');
    if (k === 6 || k === 7) mctx.fillStyle = '#cfd6ff';
    const r = (k === 0 ? 3 : k === 1 || k === 2 ? 1.6 : 4);
    mctx.globalAlpha = k === 4 || k === 5 || k === 6 || k === 7 ? (hp / 100) : 1;
    if (hp <= 0) mctx.globalAlpha = 0.15;
    mctx.fillRect(X(x) - r / 2, Z(z) - r / 2, r, r);
    mctx.globalAlpha = 1;
    if (k === 0 && slot === mySlot) { mctx.strokeStyle = '#fff'; mctx.strokeRect(X(x) - 4, Z(z) - 4, 8, 8); }
  }
}

function pollHud() {
  if (pollHud.started) return;
  pollHud.started = true;
  let previous = 0;
  const tick = (now) => {
    if (now - previous < 50) { requestAnimationFrame(tick); return; }
    previous = now;
    try {
      const h = JSON.parse(wasm.state_json());
      render(h);
    } catch (e) {
      if (!pollHud.warned) { pollHud.warned = true; console.error(e); }
    }
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

document.querySelectorAll('#abils .slot').forEach((el) => {
  el.addEventListener('click', (e) => {
    const i = Number(el.dataset.abil);
    if (e.target.closest('.up') || !latest?.me?.rk[i]) wasm_cmd({ rank: i });
    else aimedCommand('cast', i);
  });
});
document.querySelectorAll('#islots .slot').forEach((el) => {
  el.addEventListener('click', () => wasm_cmd({ use: Number(el.dataset.item) }));
});
document.querySelectorAll('#spells .slot').forEach((el) => {
  el.addEventListener('click', () => aimedCommand('spell', Number(el.dataset.spell)));
});

if (DATA) {
  buildDraftChrome();
  buildDetail();
  for (const id of ['btn-practice', 'btn-practice3']) $(id).disabled = false;
  $('engine-note').textContent = '';
  document.body.dataset.leagueBoot = 'ready';
  syncEngine();
  paintHudKeys();
} else {
  document.body.dataset.leagueBoot = 'failed';
}
resolveLeagueReady(wasm);

// ---- boot -------------------------------------------------------------------

const pending = readSaved('ember-pending', null, sessionStorage);
try { sessionStorage.removeItem('ember-pending'); } catch {}
await discover();
if (pending && pending.ws && wasm) {
  const h = onlyHost(pending.ws).chosen;
  mode = pending.mode ? Number(pending.mode) : 3;
  launchOnline(pending.lobby, pending.password, pending.action === 'create', h);
} else {
  await showLobbies();
  setInterval(() => { if (!launched) showLobbies(); }, 10000);
}
