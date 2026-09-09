// Write the trailer's caption file from the mix's own cue list.
//
//   node tools/trailer/make-captions.cjs <cues.json> <out.vtt>
//
// The times come from cues.json, which build-trailer.sh emits from the exact
// offsets it fed to the mix, and the text comes from script.json, which is also
// what was spoken. Neither is retyped here, because a caption file written by
// hand drifts away from the audio the first time a shot is recut.
'use strict';
const fs = require('node:fs');
const path = require('node:path');

const cuesPath = path.resolve(process.argv[2] || 'cues.json');
const outPath = path.resolve(process.argv[3] || 'trailer.vtt');
const script = JSON.parse(fs.readFileSync(path.join(__dirname, 'script.json'), 'utf8'));
const cues = JSON.parse(fs.readFileSync(cuesPath, 'utf8'));

const stamp = (seconds) => {
  const whole = Math.floor(seconds);
  const ms = Math.round((seconds - whole) * 1000);
  const h = String(Math.floor(whole / 3600)).padStart(2, '0');
  const m = String(Math.floor((whole % 3600) / 60)).padStart(2, '0');
  const s = String(whole % 60).padStart(2, '0');
  return `${h}:${m}:${s}.${String(ms).padStart(3, '0')}`;
};

const text = script.captions || script.lines;
if (text.length < cues.cues.length) {
  throw new Error(`script.json has ${text.length} captions for ${cues.cues.length} cues`);
}

const out = ['WEBVTT', ''];
cues.cues.forEach((cue, i) => {
  // A caption that vanishes the instant the speaker stops is hard to finish
  // reading, so each one is held a little past its line — but never into the
  // next cue, and never past the end of the film.
  const start = cue.startSeconds;
  const natural = start + cue.durationSeconds + 0.6;
  const next = cues.cues[i + 1];
  const end = Math.min(natural, next ? next.startSeconds - 0.05 : cues.totalSeconds);
  out.push(String(i + 1));
  out.push(`${stamp(start)} --> ${stamp(Math.max(end, start + 0.8))}`);
  out.push(text[i]);
  out.push('');
});

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, `${out.join('\n')}`, 'utf8');
console.log(JSON.stringify({ out: outPath, cues: cues.cues.length, bytes: fs.statSync(outPath).size }));
