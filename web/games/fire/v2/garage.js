export const vehicles = [
  { name: 'GT-01', word: 'PRECISION.', type: 'GRAND TOURING', style: 'BALANCED', color: '#ea6335', accent: '#ffba8c', stats: [78, 79, 80], desc: 'Balanced through the apex. Composed on the straight. Your introduction to a different kind of racing.', brief: 'The complete all-rounder.' },
  { name: 'APEX R', word: 'INSTINCT.', type: 'LIGHTWEIGHT', style: 'AGILE', color: '#c6d9cf', accent: '#f6fff9', stats: [65, 92, 96], desc: 'Light on its feet. Quick to change direction. Trade outright speed for the confidence to brake later.', brief: 'Lightweight. Corner hungry.' },
  { name: 'V8-R', word: 'PRESENCE.', type: 'MUSCLE', style: 'POWERFUL', color: '#647c9c', accent: '#bddbff', stats: [97, 85, 64], desc: 'Long bonnet. Serious momentum. Unleash the power on the straight, then earn every metre under braking.', brief: 'Big power. Bigger personality.' },
];
let selected = 0;
try { selected = Math.max(0, Math.min(2, Number(localStorage.getItem('fire-v2-car')) || 0)); } catch {}
export const selectedVehicle = () => selected;

// Original vector illustrations, deliberately light enough to show before the game downloads.
function carArt(v, id, hero = false) {
  const c = vehicles[v], p = `${id}-${v}`;
  const roof = v === 1 ? 'M255 127 Q292 68 374 73 L455 121' : v === 2 ? 'M242 127 L292 73 L402 77 L459 126' : 'M237 130 Q295 65 387 76 L463 125';
  const glass = v === 1 ? 'M267 125 Q299 81 337 82 L337 122Z M349 82 L374 83 L432 122 L350 122Z' : v === 2 ? 'M257 125 L299 84 L340 86 L340 123Z M350 86 L397 89 L438 123 L350 123Z' : 'M255 126 Q289 86 338 86 L338 124Z M349 85 Q369 84 383 89 L441 124 L350 124Z';
  let wheels = '';
  for (const [x, y, r] of [[192, 190, 43], [502, 190, 43]]) {
    wheels += `<g><circle cx="${x}" cy="${y}" r="${r+4}" fill="#070a0d"/><circle cx="${x}" cy="${y}" r="${r}" fill="url(#${p}-tyre)"/><circle cx="${x}" cy="${y}" r="29" fill="#879296"/><circle cx="${x}" cy="${y}" r="25" fill="#131b20"/><circle cx="${x}" cy="${y}" r="19" fill="#4b565e"/>`;
    for (let n=0;n<5;n++) wheels += `<path d="M${x-3} ${y-24} L${x+3} ${y-24} L${x+5} ${y+6} L${x-2} ${y+7}Z" fill="#c1cbd0" transform="rotate(${n*72} ${x} ${y})"/>`;
    wheels += `<circle cx="${x}" cy="${y}" r="7" fill="#121b20"/><path d="M${x+16} ${y-14}L${x+22} ${y-7}L${x+19} ${y+9}" fill="none" stroke="#f66b36" stroke-width="5"/></g>`;
  }
  return `<svg viewBox="0 0 700 280" role="img" aria-label="${c.name} vehicle illustration"><defs><linearGradient id="${p}-paint" x1="0" y1="0" x2="0.2" y2="1"><stop stop-color="${c.accent}"/><stop offset=".31" stop-color="${c.color}"/><stop offset=".64" stop-color="${c.color}"/><stop offset="1" stop-color="#1d252b"/></linearGradient><linearGradient id="${p}-glass" x2=".6" y2="1"><stop stop-color="#6d858e"/><stop offset=".4" stop-color="#20333e"/><stop offset="1" stop-color="#0b141b"/></linearGradient><radialGradient id="${p}-tyre"><stop offset=".66" stop-color="#070b0e"/><stop offset=".74" stop-color="#30383c"/><stop offset=".9" stop-color="#11171b"/><stop offset="1" stop-color="#293034"/></radialGradient></defs><ellipse cx="350" cy="237" rx="282" ry="15" fill="#03080c" opacity=".6"/>${v===1?'<path d="M511 120V101H548V125" stroke="#252e34" stroke-width="7"/><path d="M486 98L562 94L565 102L490 108Z" fill="#242d34"/>':''}<path d="${roof} L494 148 L208 145Z" fill="url(#${p}-paint)"/><path d="${glass}" fill="url(#${p}-glass)" stroke="#81918f" stroke-width="1"/><path d="M88 161 Q115 144 220 130 L451 126 Q477 121 527 126 L591 146 L612 165 L608 196 L565 204 Q555 146 503 145 Q450 145 447 205 L245 208 Q243 145 191 145 Q139 145 140 207 L92 204 L80 187Z" fill="url(#${p}-paint)" stroke="#0b141a" stroke-width="2"/><path d="M104 159L220 141L451 138L535 137" fill="none" stroke="${c.accent}" stroke-width="2" opacity=".65"/><path d="M261 145L260 189L430 186L444 140" fill="none" stroke="#0b192b" opacity=".7"/><path d="M96 184L126 179L126 190L91 193Z M576 174L607 178L605 192L577 188Z" fill="#0a1117"/><path d="M92 166L132 158L127 166L91 173Z" fill="#efffff"/><path d="M564 144L590 150L601 160L571 154Z" fill="#ff402e"/><path d="M251 201L442 198L441 208L247 214Z" fill="#101c23"/><path d="M87 200L130 202M572 201L609 196" stroke="#0a1116" stroke-width="5"/><path d="M383 149L400 149" stroke="#0b1b25" stroke-width="3"/><path d="M274 128L261 134L250 128L257 119Z" fill="#25353d"/>${wheels}<path d="M292 182L350 180" stroke="${c.accent}" opacity=".4"/>${hero?`<text x="310" y="174" font-family="sans-serif" font-size="10" letter-spacing="5" fill="#0b1520" opacity=".65">F I R E</text>`:''}</svg>`;
}

function selectVehicle(id) {
  selected = id;
  try { localStorage.setItem('fire-v2-car', String(id)); } catch {}
  const v = vehicles[id];
  document.getElementById('hero-name').innerHTML = `${v.name}<br>${v.word}`;
  document.getElementById('hero-desc').textContent = v.desc;
  document.getElementById('hero-style').textContent = v.style;
  document.getElementById('hero-class').textContent = v.type;
  document.getElementById('hero-index').textContent = `0${id+1} / 03`;
  document.querySelector('.hero-number').textContent = `0${id+1}`;
  document.getElementById('hero-car').innerHTML = carArt(id, 'hero', true);
  document.querySelectorAll('.car-card').forEach((el, i) => {
    el.setAttribute('aria-pressed', String(id===i));
    el.querySelector('.selected').textContent = id===i ? '●' : '○';
  });
}
vehicles.forEach((v, id) => {
  const b = document.createElement('button');
  b.className = 'car-card';
  b.setAttribute('aria-label', `Select ${v.name}, ${v.type.toLowerCase()}`);
  b.innerHTML = `<span class="car-top"><span>0${id+1} / ${v.type}</span><span class="selected">○</span></span>${carArt(id,'card')}<strong>${v.name}</strong><small>${v.brief}</small>${['SPEED','ACCEL.','HANDLING'].map((s,i)=>`<span class="stat"><span>${s}</span><i><b style="width:${v.stats[i]}%"></b></i></span>`).join('')}`;
  b.onclick = () => selectVehicle(id);
  document.getElementById('cars').appendChild(b);
});
selectVehicle(selected);

export const formatTime = seconds => {
  const ticks = Math.round(Math.max(0, Number(seconds) || 0) * 100);
  return `${String(Math.floor(ticks/6000)).padStart(2,'0')}:${String(Math.floor(ticks/100)%60).padStart(2,'0')}.${String(ticks%100).padStart(2,'0')}`;
};

export class EngineAudio {
  constructor() { this.enabled = true; this.ctx = null; this.previous = {}; }
  async start() {
    if (!this.ctx) {
      const Audio = window.AudioContext || window.webkitAudioContext;
      if (!Audio) return;
      this.ctx = new Audio();
      this.master = this.ctx.createGain(); this.master.gain.value = .18; this.master.connect(this.ctx.destination);
      this.filter = this.ctx.createBiquadFilter(); this.filter.type='lowpass'; this.filter.frequency.value=550; this.filter.connect(this.master);
      this.motor = this.ctx.createGain(); this.motor.gain.value=.05; this.motor.connect(this.filter);
      this.oscs = [1, 1.5, 2].map((ratio,i) => { const o=this.ctx.createOscillator(); o.type=i===1?'triangle':'sawtooth'; o.frequency.value=40*ratio; o.connect(this.motor); o.start(); return o; });
      const buffer=this.ctx.createBuffer(1,this.ctx.sampleRate*2,this.ctx.sampleRate), data=buffer.getChannelData(0);
      let last=0; for(let i=0;i<data.length;i++){ last=(last+(Math.random()*2-1)*.12)/1.06; data[i]=last*3; }
      const road=this.ctx.createBufferSource(); road.buffer=buffer; road.loop=true;
      this.tyres=this.ctx.createGain(); this.tyres.gain.value=0; road.connect(this.tyres); this.tyres.connect(this.master); road.start();
    }
    if(this.enabled) await this.ctx.resume();
  }
  toggle() { this.enabled=!this.enabled; if(this.ctx) this.master.gain.setTargetAtTime(this.enabled?.18:0,this.ctx.currentTime,.08); return this.enabled; }
  cue(hz,duration=.12) {
    if(!this.ctx||!this.enabled)return;
    const o=this.ctx.createOscillator(),g=this.ctx.createGain(),t=this.ctx.currentTime;
    o.type='sine';o.frequency.setValueAtTime(hz,t);o.frequency.exponentialRampToValueAtTime(hz*.65,t+duration);
    g.gain.setValueAtTime(.12,t);g.gain.exponentialRampToValueAtTime(.001,t+duration);o.connect(g);g.connect(this.master);o.start(t);o.stop(t+duration);
  }
  update(h) {
    if(!this.ctx)return;
    const t=this.ctx.currentTime, gear=Math.max(1,h.gear||1), rpm=34+(Math.max(0,h.speed)/gear)*1.9;
    this.oscs.forEach((o,i)=>o.frequency.setTargetAtTime(rpm*[1,1.5,2][i],t,.09));
    this.motor.gain.setTargetAtTime(h.finished?.015:.055+Math.min(.06,h.speed/2400),t,.12);
    this.filter.frequency.setTargetAtTime(h.boosting?1700:450+rpm*3,t,.1);
    this.tyres.gain.setTargetAtTime(h.finished?0:Math.min(.12,h.speed*.0004)+(h.drifting?.20:0),t,.1);
    if(h.item && h.item!==this.previous.item)this.cue(880,.22);
    if(h.hit>.1 && !(this.previous.hit>.1))this.cue(95,.28);
    if(h.countdown>0 && Math.ceil(h.countdown)!==Math.ceil(this.previous.countdown||0))this.cue(440,.13);
    if(h.countdown===0 && this.previous.countdown>0)this.cue(880,.4);
    this.previous=h;
  }
}
