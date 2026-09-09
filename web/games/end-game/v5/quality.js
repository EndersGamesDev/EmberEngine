// Only render budget changes: simulation remains fixed at 60 Hz.
export class Quality {
  constructor({width, height, dpr = 1, memory = 4, mobile = false, maxDimension = 8192}) {
    this.display = {width, height, dpr, maxDimension};
    this.mobile = mobile;
    this.mode = 'auto';
    this.samples = [];
    this.stable = 0;
    this.scale = this.limit() * (memory <= 4 || mobile ? 0.72 : 1);
    this.clamp();
  }
  limit() {
    const {width, height, dpr, maxDimension} = this.display;
    const pixels = width * height * dpr * dpr;
    return Math.min(1, 5120 / Math.max(width * dpr, 1), maxDimension / Math.max(width * dpr, height * dpr, 1), Math.sqrt(14745600 / Math.max(pixels, 1)));
  }
  clamp() { this.scale = Math.min(this.limit(), Math.max(0.25, this.scale)); }
  set(mode) { this.mode = mode; this.scale = this.limit() * (mode === 'performance' ? 0.60 : mode === 'auto' && this.mobile ? 0.72 : 1); this.samples = []; this.stable = 0; }
  sample(ms) {
    if (!Number.isFinite(ms) || ms < 2) return;
    this.samples.push(Math.min(ms, 1000));
    if (this.samples.length < 120 && this.samples.reduce((a, b) => a + b, 0) < 2000) return;
    const mean = this.samples.reduce((a, b) => a + b, 0) / this.samples.length;
    this.samples = [];
    this.fps = Math.round(1000 / mean);
    if (this.mode !== 'auto') return;
    if (mean > 22) { this.scale *= 0.88; this.stable = 0; }
    else if (mean < 17.3 && ++this.stable >= 4) { this.scale *= 1.07; this.stable = 0; }
    else if (mean >= 17.3) this.stable = 0;
    this.clamp();
  }
}
