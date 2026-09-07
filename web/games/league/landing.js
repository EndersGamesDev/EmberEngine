/* UltimateLegue landing — page behaviour. No dependencies, no tracking.
 *
 * story.json follows schemaVersion 1 (board 191): campaign, world,
 * champions, chapters, features. Any invalid or missing file leaves the
 * HTML draft copy untouched. */

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

// ---- trailer: poster-first, never a fake play control ----

const video = $('#trailer-video');
const trailerNote = $('#trailer-note');

function toPoster() {
  if (!video || video.dataset.posterized) return;
  video.dataset.posterized = '1';
  const img = document.createElement('img');
  img.src = video.poster || './media/trailer-poster.webp';
  img.alt = 'Trailer poster: the Crystalforge lane at dusk';
  img.className = 'trailer-poster';
  img.onerror = () => { img.remove(); trailerNote.hidden = false; };
  video.replaceWith(img);
  trailerNote.hidden = false;
}

video.addEventListener('error', toPoster, true);
(async () => {
  try {
    const r = await fetch('./media/trailer.mp4', { method: 'HEAD' });
    if (r.status === 404 || r.status === 403 || r.status === 410) toPoster();
  } catch {
    // Network failure: assume the file may exist; the error event covers it.
  }
})();

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
const showIf = (id, value) => {
  const el = $(id);
  if (!el) return;
  const has = typeof value === 'string' && value.trim().length > 0;
  el.hidden = !has;
  if (has) el.textContent = value;
};

function fillOrigin(value) {
  const box = $('#reader-origin');
  if (!box) return;
  box.innerHTML = '';
  if (typeof value !== 'string') return;
  for (const part of value.split('\n\n')) {
    if (part.trim()) box.append(Object.assign(document.createElement('p'), { textContent: part.trim() }));
  }
}

function fillReader(card) {
  const d = card.dataset;
  text('#reader-name', d.name);
  text('#reader-title', d.title);
  showIf('#reader-quote', d.quote);
  showIf('#reader-role', d.role);
  fillOrigin(d.origin);
  text('#reader-motive', d.motive);
  text('#reader-flaw', d.flaw);
  text('#reader-bond', d.relationship);
  const rimg = $('#reader-img');
  const src = d.storyImage || d.portrait || (card.querySelector('.c-art img')?.src);
  if (src) {
    rimg.src = src;
    rimg.alt = `Story art for ${d.name}`;
    rimg.style.display = '';
  }
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

// ---- story.json (schemaVersion 1) ----

const isStr = (v, max = 4000) => typeof v === 'string' && v.trim().length > 0 && v.length <= max;
const isStrArr = (v, max = 4000) => Array.isArray(v) && v.length > 0 && v.every((x) => isStr(x, max));
const KEYS = ['swarm', 'emberknight', 'hallow', 'bogmaw', 'tessera'];
const FEATURE_ICONS = ['dagger', 'swarm-q', 'crystal', 'coin', 'storm', 'aegis'];

const el = (tag, cls, value) => {
  const n = document.createElement(tag);
  if (cls) n.className = cls;
  if (typeof value === 'string') n.textContent = value;
  return n;
};

function applyCampaign(c) {
  if (!c || typeof c !== 'object') return;
  if (isStr(c.eyebrow)) $('.hero .eyebrow').textContent = c.eyebrow.trim();
  if (isStr(c.headline)) $('#hero-title').textContent = c.headline.trim();
  showIf('#campaign-tagline', c.tagline);
  if (isStr(c.description)) $('#hero-lede').textContent = c.description.trim();
}

function applyWorld(w) {
  if (!w || typeof w !== 'object') return;
  if (isStr(w.title)) $('#world-title').textContent = w.title.trim();
  showIf('#world-subtitle', w.subtitle);
  const box = $('#world-para');
  if (isStrArr(w.paragraphs)) {
    box.innerHTML = '';
    for (const p of w.paragraphs) box.append(el('p', null, p.trim()));
  }
}

function applyChampions(list) {
  if (!Array.isArray(list) || !list.length) return;
  const names = {};
  for (const c of list) if (c && isStr(c.id) && isStr(c.name)) names[c.id.trim()] = c.name.trim();
  const cards = {};
  document.querySelectorAll('.champ').forEach((c) => { cards[c.dataset.key] = c; });
  list.forEach((entry, i) => {
    if (!entry || typeof entry !== 'object') return;
    const key = isStr(entry.id) && cards[entry.id.trim()] ? entry.id.trim() : KEYS[i];
    const card = cards[key];
    if (!card || !isStr(entry.name)) return;
    const d = card.dataset;
    d.name = entry.name.trim();
    if (isStr(entry.epithet)) d.title = entry.epithet.trim();
    if (isStr(entry.hook)) d.desc = entry.hook.trim();
    if (isStr(entry.quote)) d.quote = entry.quote.trim();
    if (isStr(entry.role)) d.role = entry.role.trim();
    if (isStrArr(entry.origin)) d.origin = entry.origin.join('\n\n');
    if (isStr(entry.motivation)) d.motive = entry.motivation.trim();
    if (isStr(entry.flaw)) d.flaw = entry.flaw.trim();
    if (entry.bond && typeof entry.bond === 'object') {
      const who = isStr(entry.bond.champion) && names[entry.bond.champion.trim()]
        ? names[entry.bond.champion.trim()]
        : '';
      d.relationship = [isStr(entry.bond.text) ? entry.bond.text.trim() : '', who].filter(Boolean).join(' — ');
    }
    card.querySelector('.c-name').textContent = d.name;
    card.querySelector('.c-title').textContent = d.title;
    card.querySelector('.c-desc').textContent = d.desc;
    const img = card.querySelector('.c-art img');
    if (img) {
      img.alt = `Painted portrait of ${d.name}, ${d.title}`;
      if (isStr(entry.portrait, 200)) {
        d.portrait = entry.portrait.trim();
        img.src = d.portrait;
      }
    }
    if (isStr(entry.storyImage, 200)) d.storyImage = entry.storyImage.trim();
    else if (img) d.storyImage = d.portrait || img.src;
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
    li.id = isStr(c.id, 60) ? c.id.trim() : `chapter-${i + 1}`;
    const num = Number.isFinite(c.number) ? Math.floor(c.number) : i + 1;
    li.append(el('span', 'cnum', String(num).padStart(2, '0')));
    const body = el('div', 'cbody');
    if (isStr(c.image, 200)) {
      const img = el('img', 'cimg');
      img.src = c.image.trim();
      img.alt = '';
      img.loading = 'lazy';
      img.onerror = () => img.remove();
      body.append(img);
    }
    if (isStr(c.title)) body.append(el('h3', null, c.title.trim()));
    if (isStr(c.subtitle)) body.append(el('p', 'csub', c.subtitle.trim()));
    const paras = isStrArr(c.body) ? c.body : [];
    paras.forEach((p, j) => {
      const last = j === paras.length - 1 && paras.length > 1;
      body.append(el('p', last ? 'hook' : null, p.trim()));
    });
    li.append(body);
    ol.append(li);
  });
}

function applyFeatures(list) {
  if (!Array.isArray(list) || !list.length) return;
  const ul = $('#features');
  ul.innerHTML = '';
  list.slice(0, 8).forEach((f, i) => {
    if (!f || typeof f !== 'object' || !isStr(f.title) || !isStr(f.text)) return;
    const li = el('li', 'feature');
    if (FEATURE_ICONS[i]) {
      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('class', 'ficon');
      svg.setAttribute('viewBox', '0 0 64 64');
      svg.setAttribute('aria-hidden', 'true');
      const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
      use.setAttribute('href', `./v2/art/icons.svg#${FEATURE_ICONS[i]}`);
      svg.append(use);
      li.append(svg);
    }
    li.append(el('h3', null, f.title.trim()));
    li.append(el('p', null, f.text.trim()));
    ul.append(li);
  });
}

async function loadStory() {
  try {
    const r = await fetch('./story.json', { cache: 'no-cache' });
    if (!r.ok) return;
    const s = await r.json();
    if (!s || typeof s !== 'object') return;
    applyCampaign(s.campaign);
    applyWorld(s.world);
    applyChampions(s.champions);
    applyChapters(s.chapters);
    applyFeatures(s.features);
  } catch {
    // story.json missing or invalid: the draft copy in the HTML stays.
  }
}
loadStory();
