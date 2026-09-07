/* UltimateLegue landing — page behaviour. No dependencies, no tracking. */

const $ = (s) => document.querySelector(s);

// ---- hero art: media/hero.webp, falling back to the v2 arena picture ----

const heroArt = $('#hero-art');
heroArt.addEventListener('error', () => {
  if (!heroArt.dataset.fb) {
    heroArt.dataset.fb = '1';
    heroArt.src = './v2/art/arena.webp';
  } else {
    heroArt.remove();
  }
});

// ---- trailer: surface a note if the video cannot load ----

const video = $('#trailer-video');
const trailerNote = $('#trailer-note');
video.addEventListener('error', () => { trailerNote.hidden = false; }, true);

// ---- reveal on scroll (skipped for reduced motion and when JS is off) ----

if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches && 'IntersectionObserver' in window) {
  document.documentElement.classList.add('reveal');
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (e.isIntersecting) { e.target.classList.add('in'); io.unobserve(e.target); }
    }
  }, { rootMargin: '0px 0px -8% 0px' });
  document.querySelectorAll('.fade').forEach((n) => io.observe(n));
}

// ---- champion reader ----

const reader = $('#reader');
const readerClose = $('#reader-close');
let activeCard = null;

const text = (id, value) => { const el = $(id); if (el && typeof value === 'string') el.textContent = value; };

function fillReader(card) {
  const d = card.dataset;
  const img = card.querySelector('.c-art img');
  text('#reader-name', d.name);
  text('#reader-title', d.title);
  text('#reader-origin', d.origin);
  text('#reader-motive', d.motive);
  text('#reader-flaw', d.flaw);
  text('#reader-relationship', d.relationship);
  const rimg = $('#reader-img');
  if (img) { rimg.src = img.src; rimg.alt = img.alt; rimg.style.display = ''; }
  const emb = card.querySelector('.c-emb');
  text('#reader-emb', emb ? emb.textContent : '');
  reader.style.setProperty('--accent', getComputedStyle(card).getPropertyValue('--accent'));
}

function openReader(card) {
  fillReader(card);
  reader.classList.remove('hidden');
  activeCard = card;
  document.querySelectorAll('.champ').forEach((c) => c.setAttribute('aria-expanded', String(c === card)));
  reader.focus({ preventScroll: true });
}

function closeReader() {
  if (reader.classList.contains('hidden')) return;
  reader.classList.add('hidden');
  const back = activeCard;
  activeCard = null;
  document.querySelectorAll('.champ').forEach((c) => c.setAttribute('aria-expanded', 'false'));
  if (back) back.focus({ preventScroll: true });
}

document.querySelectorAll('.champ').forEach((card) => {
  card.addEventListener('click', () => {
    if (activeCard === card && !reader.classList.contains('hidden')) closeReader();
    else openReader(card);
  });
});
readerClose.addEventListener('click', closeReader);
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && !reader.classList.contains('hidden') && !e.defaultPrevented) closeReader();
});

// ---- story.json: replace the draft copy at runtime when valid ----

const isStr = (v, max = 4000) => typeof v === 'string' && v.trim().length > 0 && v.length <= max;
const KEYS = ['swarm', 'emberknight', 'hallow', 'bogmaw', 'tessera'];
const el = (tag, cls, value) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (typeof value === 'string') n.textContent = value;
  return n;
};

function applyWorld(w) {
  if (!w || typeof w !== 'object') return;
  if (isStr(w.premise)) $('#world-premise').textContent = w.premise.trim();
  if (isStr(w.tagline)) $('#hero-title').textContent = w.tagline.trim();
  if (isStr(w.title)) $('.hero .eyebrow').textContent = `Ember original · ${w.title.trim()}`;
}

function applyChampions(list) {
  if (!Array.isArray(list) || !list.length) return;
  const cards = {};
  document.querySelectorAll('.champ').forEach((c) => { cards[c.dataset.key] = c; });
  list.forEach((entry, i) => {
    if (!entry || typeof entry !== 'object') return;
    const key = isStr(entry.key) && cards[entry.key.trim()] ? entry.key.trim() : KEYS[i];
    const card = cards[key];
    if (!card || !isStr(entry.name)) return;
    const d = card.dataset;
    d.name = entry.name.trim();
    if (isStr(entry.title)) d.title = entry.title.trim();
    if (isStr(entry.desc) || isStr(entry.oneLiner) || isStr(entry.blurb)) d.desc = (entry.desc || entry.oneLiner || entry.blurb).trim();
    if (isStr(entry.origin)) d.origin = entry.origin.trim();
    if (isStr(entry.motive)) d.motive = entry.motive.trim();
    if (isStr(entry.flaw)) d.flaw = entry.flaw.trim();
    if (isStr(entry.relationship)) d.relationship = entry.relationship.trim();
    card.querySelector('.c-name').textContent = d.name;
    card.querySelector('.c-title').textContent = d.title;
    card.querySelector('.c-desc').textContent = d.desc;
    const img = card.querySelector('.c-art img');
    if (img) img.alt = `Painted portrait of ${d.name}, ${d.title}`;
    if (isStr(entry.accent) && /^#[0-9a-fA-F]{3,8}$/.test(entry.accent.trim())) {
      card.style.setProperty('--accent', entry.accent.trim());
    }
    if (isStr(entry.emblem, 8)) card.querySelector('.c-emb').textContent = entry.emblem.trim();
  });
  if (activeCard) fillReader(activeCard);
}

function applyChapters(list) {
  if (!Array.isArray(list) || !list.length) return;
  const ol = $('#chapters');
  ol.innerHTML = '';
  list.slice(0, 5).forEach((c, i) => {
    if (!c || typeof c !== 'object') return;
    const li = el('li', 'chapter');
    li.id = `chapter-${i + 1}`;
    li.append(el('span', 'cnum', String(i + 1).padStart(2, '0')));
    const body = el('div', 'cbody');
    if (isStr(c.title)) body.append(el('h3', null, c.title.trim()));
    const paras = Array.isArray(c.body)
      ? c.body.filter((p) => isStr(p))
      : (isStr(c.body) ? [c.body] : []);
    paras.forEach((p, j) => {
      const last = j === paras.length - 1 && paras.length > 1;
      body.append(el('p', last ? 'hook' : null, p.trim()));
    });
    li.append(body);
    ol.append(li);
  });
}

async function loadStory() {
  try {
    const r = await fetch('./story.json', { cache: 'no-cache' });
    if (!r.ok) return;
    const s = await r.json();
    if (!s || typeof s !== 'object') return;
    applyWorld(s.world);
    applyChampions(s.champions);
    applyChapters(s.chapters);
  } catch {
    // story.json missing or invalid: the draft copy in the HTML stays.
  }
}
loadStory();
