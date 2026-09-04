# The slot for recorded sound effects

Every cue the arena client plays is synthesised in `crates/arena/src/sound.rs`; nothing in this folder ships in v20. This folder is where a recorded sample goes when the project has one it is licensed to ship, and this file is the contract for dropping one in.

## The format

One file per cue, named `<cue>.wav` where `<cue>` is the name `Sfx::file_name()` gives that variant (`shot_ak_near`, `impact_metal`, `reload_revolver`, and so on; the full list is the `file_name` match in `sound.rs`). The file is RIFF WAVE, PCM, 16-bit, mono, 44.1 kHz. The reader (`decode_wav`) refuses anything else by name: stereo, 8- or 24-bit, another rate, a compressed codec. Convert at the source; the client does not resample or mix down, so a wrong file is caught in the test and not degraded in silence.

Normalise the recording to a peak near -1 dBFS and trim the leading silence: the client plays a cue the frame its event lands, and the synthesised set has its attack in the first 25 ms, so a recording with 40 ms of lead-in arrives late against the tracer.

## The two steps

1. Copy the file here as `<cue>.wav`.
2. Add its line to the `RECORDED` table in `sound.rs`: `(Sfx::ShotAkNear, include_bytes!("../assets/sfx/shot_ak_near.wav"))`.

The line is what makes the recording replace the synth. `include_bytes!` needs a literal path and a file that exists, so there is no build script scanning this folder: a missing file fails the build loudly instead of falling back to the synth without anyone noticing. The test `the_slot_folder_holds_only_registered_wavs` walks this folder and fails for a WAV that is not a cue name, does not decode, or has no line in the table, so a file dropped in without step 2 is caught by `cargo test -p arena --lib`.

## What it costs

`include_bytes!` bakes the file into the wasm bundle; every byte here is a byte every web player downloads. A 44.1 kHz PCM16 mono second is 88 KB in the bundle and 176 KB decoded (four bytes a sample) on both platforms, and the web keeps a second decoded copy inside the audio context. The synthesised set of 52 cues is 3.1 MB decoded (the `summary.csv` the plot helper writes carries the exact total) and the plan's ceiling is 24 MB.

## Licensing

A recording is an asset like a model: its licence has to allow shipping in a public web page and in a native build. Nothing in this repo today carries such a licence for sound, which is why the folder is empty and the slot is the next tier, not this one.
