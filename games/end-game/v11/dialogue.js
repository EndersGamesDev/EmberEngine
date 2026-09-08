export const LINES = Object.freeze({
  movement: {file: 'warden-movement.wav', text: 'Shut up and go back to your place!', duration: 2.27008},
  key: {file: 'warden-unlocking.wav', text: 'Where did you get that key from?', duration: 2.06546},
  sword: {file: 'warden-sword.wav', text: 'Why you have a sword here?', duration: 1.96713},
  death: {file: 'warden-death.wav', text: 'Arrrrrghhhh, I will get revenge!', duration: 2.11633},
});

// One speaker, with a per-life cursor. The simulation retains recent events so
// several fixed updates between rendered frames cannot swallow a story line.
export class WardenDialogue {
  constructor(output, caption = () => {}) {
    this.output = output;
    this.caption = caption;
    this.life = null;
    this.cursor = 0;
    this.queue = [];
    this.active = null;
    this.paused = false;
    this.dead = false;
    this.finished = false;
    this.now = 0;
    this.waiting = 0;
    this.justResumed = false;
  }
  clear() {
    this.output.stop();
    this.active = null;
    this.queue = [];
    this.waiting = 0;
    this.caption(null);
  }
  ingest(snapshot, now) {
    if (!snapshot) return;
    if (snapshot.life !== this.life) {
      this.clear();
      this.life = snapshot.life;
      this.cursor = 0;
      this.dead = false;
      this.finished = false;
    }
    this.now = now;
    const incoming = (snapshot.events || []).filter(e => e.id > this.cursor).sort((a,b) => a.id-b.id);
    for (const event of incoming) this.cursor = Math.max(this.cursor, event.id);
    if (this.finished) return;
    const death = [...incoming].reverse().find(e => e.kind === 'death');
    if (death) {
      this.clear();
      this.dead = true;
      if (now - death.time <= 10) this.queue.push(death);
    } else if (snapshot.dead || this.dead) {
      this.dead = true;
      this.queue = this.queue.filter(e => e.kind === 'death');
      if (this.active && this.active.event.kind !== 'death') this.clear();
    } else {
      for (const event of incoming) {
        if (!LINES[event.kind] || now-event.time > (event.kind === 'movement' ? 3 : 12)) continue;
        if (event.kind === 'movement') {
          if (!this.active && this.queue.length === 0) this.queue.push(event);
        } else {
          if (this.active?.event.kind === 'movement') {
            this.output.stop();
            this.active = null;
            this.caption(null);
          }
          this.queue = this.queue.filter(e => e.kind !== 'movement');
          this.queue.push(event);
        }
      }
    }
  }
  tick(seconds) {
    if (this.paused) return;
    // A resume can happen before RAF measures its first frame after background
    // throttling. That frame's delta includes the paused interval.
    const dt = this.justResumed ? 0 : Math.max(0, Number.isFinite(seconds) ? seconds : 0);
    this.justResumed = false;
    if (this.active) {
      this.active.elapsed += dt;
      if (this.active.elapsed >= Math.max(3.2, this.active.line.duration + 0.4)) {
        this.output.stop();
        this.active = null;
        this.caption(null);
      }
    }
    if (this.active) return;
    while (this.queue.length && this.now-this.queue[0].time > (this.queue[0].kind === 'movement' ? 3 : 12)) {
      this.queue.shift();
    }
    if (!this.queue.length) { this.waiting = 0; return; }
    this.waiting += dt;
    const event = this.queue[0], line = LINES[event.kind];
    // Briefly wait for decoding; unavailable audio still gets a timed caption.
    if (!this.output.ready(event.kind) && this.waiting < 1.2) return;
    this.queue.shift();
    this.waiting = 0;
    this.active = {event, line, elapsed: 0};
    this.caption(line.text);
    this.output.play(event.kind);
  }
  setPaused(value) {
    if (this.paused === value) return;
    this.paused = value;
    if (value) { this.output.pause(); this.caption(null); }
    else { this.justResumed = true; this.output.resume(); this.caption(this.active?.line.text || null); }
  }
  finish() {
    this.finished = true;
    this.queue = this.queue.filter(e => e.kind === 'death');
    if (this.active?.event.kind !== 'death') {
      this.output.stop(); this.active = null; this.caption(null);
    }
  }
  get speaking() { return !!this.active && !this.paused; }
}

// Decode once after the same user gesture that unlocks the game's AudioContext.
// Sources are recreated at their saved offset when the pause menu is closed.
export class VoiceAudio {
  constructor(getContext, lines = LINES) {
    this.getContext = getContext;
    this.lines = lines;
    this.buffers = new Map();
    this.failed = new Set();
    this.loading = false;
    this.clip = null;
    this.source = null;
    this.gain = null;
    this.volume = 0.55;
  }
  preload() {
    const context = this.getContext();
    if (!context || this.loading) return;
    this.loading = true;
    for (const [kind, line] of Object.entries(this.lines)) {
      fetch(new URL(line.file, import.meta.url))
        .then(response => { if (!response.ok) throw new Error('Voice asset unavailable'); return response.arrayBuffer(); })
        .then(data => context.decodeAudioData(data))
        .then(buffer => this.buffers.set(kind, buffer))
        .catch(() => this.failed.add(kind));
    }
  }
  ready(kind) { return this.buffers.has(kind) || this.failed.has(kind); }
  play(kind) {
    this.stop();
    const buffer = this.buffers.get(kind), context = this.getContext();
    // No delayed surprise playback if the browser blocked audio or decoding.
    if (!buffer || !context || context.state !== 'running') return;
    this.clip = {buffer, offset: 0, started: 0};
    this.resume();
  }
  disconnect() {
    if (this.source) { try { this.source.stop(); } catch {} this.source.disconnect(); }
    this.gain?.disconnect();
    this.source = null; this.gain = null;
  }
  stop() { this.disconnect(); this.clip = null; }
  pause() {
    if (this.source && this.clip) this.clip.offset += this.getContext().currentTime - this.clip.started;
    this.disconnect();
  }
  resume() {
    const context = this.getContext(), clip = this.clip;
    if (!clip || this.source || !context || context.state !== 'running' || clip.offset >= clip.buffer.duration) return;
    try {
      this.source = context.createBufferSource();
      this.gain = context.createGain();
      this.source.buffer = clip.buffer;
      this.gain.gain.value = this.volume;
      this.source.connect(this.gain).connect(context.destination);
      clip.started = context.currentTime;
      this.source.start(0, clip.offset);
    } catch { this.stop(); }
  }
  setVolume(value) {
    this.volume = Math.max(0, Math.min(1, value));
    if (this.gain) this.gain.gain.value = this.volume;
  }
}
