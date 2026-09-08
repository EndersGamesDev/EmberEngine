# Trailer tools

How the Killshot trailer on [`web/games/arena/`](../../web/games/arena/) was made, and how to remake it. Nothing here runs during a build or a release; these are authoring tools, run by hand.

The trailer mixes three sources: real gameplay recorded from the shipped client, cinematic shots and stills from the LAN asset workers, and a title card drawn by the browser. The narration is spoken by a text-to-speech model and describes only what the game actually does — every claim in it was checked against `crates/arena-core` before it was recorded.

## The pieces

[`capture-gameplay.cjs`](capture-gameplay.cjs) records real gameplay as a numbered JPEG sequence. It owns a private loopback web server, a private `arena-server`, and a disposable headless browser, and refuses to start if either port is already listening. Input is authored DOM events dispatched inside that document — never `page.keyboard`, never OS input, never foreground activation — because someone is sitting at this machine.

[`contact-sheet.cjs`](contact-sheet.cjs) composes evenly spaced frames of a take into one picture, so a whole take can be judged at a glance. No image library is installed on this host and none is needed: the montage is drawn in the same headless browser.

[`title-card.cjs`](title-card.cjs) renders the title card. Text is drawn by the browser rather than generated, because an image model cannot spell a wordmark reliably.

[`probe-look.cjs`](probe-look.cjs) is a diagnostic kept for the next person: it reports which authored mouse-motion event the real client accepts. It exists because of the trap below.

[`build-trailer.sh`](build-trailer.sh) assembles the film. It runs **on falke64**, which has the only ffmpeg on this network and is where the Wan video worker stores its shots.

## Running it

```bash
cargo build --target wasm32-unknown-unknown --release -p arena --lib
wasm-bindgen --target web --no-typescript --out-dir web/pkg target/wasm32-unknown-unknown/release/arena.wasm
cargo build --release -p arena-server

EMBER_QA_PLAYWRIGHT=<absolute path to a playwright module> \
  node tools/trailer/capture-gameplay.cjs --shot breach-12
```

`--shot` names one of the takes defined at the top of the capture script; `--map` instead assembles an ad-hoc scouting take from flags. `--out` chooses the frame directory, `--chrome visible` keeps the page's own navigation in frame, and `--width`/`--height` set the capture size.

Frames, the staged audio and the title card are then copied to `~/killshot-trailer` on falke64, and `build-trailer.sh` is run there with the four Wan job ids in the environment:

```bash
ssh falke64 'cd ~/killshot-trailer && WAN_OPERATOR=<id> WAN_HARBOR=<id> \
  WAN_FREIGHT=<id> WAN_SHOTGUN=<id> bash build-trailer.sh'
```

It writes `trailer.mp4` and `cues.json`. The caption file is generated from `cues.json` rather than written by hand, so recutting a shot cannot silently drift the captions away from the voice.

## Traps this cost a bug each

**A synthetic mouse move turns the camera by exactly zero.** winit reads the motion out of `getCoalescedEvents()`, and Chromium returns `[]` for an untrusted event. The look event must be a `PointerEvent('pointermove')` with `movementX`/`movementY` *and* `getCoalescedEvents` defined on it, exactly as `tools/v31/browser-killshot.cjs` does it. Nothing errors when this is wrong — the take is simply six seconds of a frozen view.

**A fractional `movementX` is truncated to zero.** A slow pan divided into 16 ms steps is well under one pixel per step. `glide()` therefore carries a remainder and only dispatches whole pixels. Same silent failure.

**A match begins paused**, behind the personal-setup dialog, and a paused client still sends input packets — they are simply neutral. Waiting for the paused state and clicking Resume is not optional, and sampling for the dialog instead of waiting for it races.

Because all three fail silently and identically, the capture measures the client's own aim packets and **fails a take whose camera never moved**, over the recorded portion only — counting the framing preroll would let a frozen take pass on the strength of its own setup move.

**The concat demuxer resolves relative paths against the list file's directory**, so entries are written absolute.

**Playwright's `recordVideo` needs a separate ffmpeg download.** The Chrome DevTools screencast needs nothing, delivers frames at the browser's real rate, and leaves the muxing to an ffmpeg we already have. That is why the capture writes frames and not a video.
