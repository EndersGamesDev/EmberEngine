// Story clips share one voice channel. Event ids survive castle checkpoint deaths.
// Captions are driven independently of successful audio decoding.
export class CastleDialogue {
  constructor(output, lines, caption = () => {}) {
    this.output = output;
    this.lines = lines;
    this.caption = caption;
    this.cursor = 0;
    this.queue = [];
    this.active = null;
    this.paused = false;
    this.resumed = false;
    this.waiting = 0;
  }
  ingest(events = []) {
    for (const event of [...events].sort((a, b) => a.id - b.id)) {
      if (event.id <= this.cursor) continue;
      this.cursor = event.id;
      if (!this.lines[event.kind]) continue;
      if (['boss_defeat', 'escape_ending'].includes(event.kind)) {
        this.output.stop(); this.active = null; this.queue = []; this.caption(null);
      }
      this.queue.push(event);
    }
  }
  tick(seconds, channelBusy = false) {
    if (this.paused) return;
    const dt = this.resumed ? 0 : Math.min(0.25, Math.max(0, Number.isFinite(seconds) ? seconds : 0));
    this.resumed = false;
    if (this.active) {
      this.active.elapsed += dt;
      if (this.active.elapsed >= Math.max(4, this.active.line.duration + 0.6)) {
        this.output.stop(); this.active = null; this.caption(null);
      }
    }
    if (this.active || channelBusy || !this.queue.length) return;
    this.waiting += dt;
    const event = this.queue[0], line = this.lines[event.kind];
    if (!this.output.ready(event.kind) && this.waiting < 1.2) return;
    this.queue.shift(); this.waiting = 0;
    this.active = {event, line, elapsed: 0};
    this.caption(line); this.output.play(event.kind);
  }
  setPaused(value) {
    if (this.paused === value) return;
    this.paused = value;
    if (value) { this.output.pause(); this.caption(null); }
    else { this.resumed = true; this.output.resume(); this.caption(this.active?.line || null); }
  }
  get speaking() { return !!this.active && !this.paused; }
}
