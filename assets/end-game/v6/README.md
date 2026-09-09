# V6 warden voice

Four prerecorded English voice cues for the prison warden. These are actual neural speech WAVs usable offline after delivery to the browser; playback does not depend on an installed browser voice or a running synthesis service. The fictional voice blends two stock synthetic male voices and does not target a real person's identity.

| Event | File | Exact requested wording | Duration |
|---|---|---|---:|
| Movement | `warden-movement.wav` | Shut up and go back to your place | 2.27008 s |
| Unlocking | `warden-unlocking.wav` | Where did you get that key from? | 2.06546 s |
| Sword pickup | `warden-sword.wav` | Why you have a sword here? | 1.96713 s |
| Death | `warden-death.wav` | Arrrrrghhhh, I will get revenge! | 2.11633 s |

All files are standard mono WAV, signed little-endian PCM16, 24,000 Hz. Total payload is 404,288 bytes. Peak level is -2.5 dBFS, with no clipped samples. `manifest.json` contains exact frame counts, SHA-256 hashes, source-take hashes, transcripts, generated phonemes, synthesis versions and processing settings. The browser integration owns event selection, gain, interruption, subtitles and playback timing. Existing V1–V5 audio and geometry are untouched.

## Voice source and treatment

Kokoro-82M generated the speech on the existing Adler CPU installation, using 70% `bm_george` and 30% `bm_lewis`, British English phonemization, and seed 60908. The cached model revision is `f3ff3571791e39611d31c381e3a41a3af07b4987`. The source environment reports Kokoro 0.9.4, Misaki 0.9.4, Torch 2.14.0+cpu and NumPy 2.5.3. The existing project TTS provenance identifies Kokoro as Apache 2.0; the model weights and synthesis environment are not part of these shipped assets.

The words are preserved, with punctuation adjusted for short, forceful lines. The death cue starts with the synthesized spelling `Arrrrrghhhh`; it is a voiced grunt syllable rather than a recorded human performance. Kokoro has no explicit anger control in this pipeline. The stern male voice is supported by light saturation, a 2% pitch/rate reduction (-0.350 semitones), modest low-frequency body and 2.5% rasp modulation, increased to 5% briefly at the death cue's beginning. Stronger processing was reduced after an offline recognizer lost clarity. A 160 ms pre-roll allowance protects initial consonants, with short fades and quiet boundaries for click-free playback.

No models were downloaded, no worker service or peer job was changed, and no GPU was used. Generation ran with offline caches, `nice 19`, two CPU threads, and one interop thread. Synthesis took 3.6794 seconds, or 7.34 seconds including SSH/upload/import overhead. Local waveform processing takes approximately 0.04 seconds at Idle priority.

## Reproduce

The three Python scripts and optional Windows dictation script are under `tools/end-game/`. Source takes and logs belong in an explicit scratch directory, not this asset directory. The existing synthesis installation is `/home/ender/asset-forge/envs/tts` on `adler40`, with `HF_HOME=/home/ender/asset-forge/models/kokoro/hf-home`. Use an available worker slot; do not interrupt an existing job or change its services.

Upload `tools/end-game/v6_generate_voice.py` into a dedicated scratch directory on that worker, then run:

```bash
cd /home/ender/asset-forge
nice -n 19 env OMP_NUM_THREADS=2 MKL_NUM_THREADS=2 HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 CUDA_VISIBLE_DEVICES= envs/tts/bin/python outputs/tts/end-game-v6/v6_generate_voice.py --out outputs/tts/end-game-v6
```

Copy the four `*-raw.wav` takes and `generation.json` into a local scratch directory. From the repository root:

```powershell
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v6_finish_voice.py --raw 'C:\Users\end\dev\end-game\runtime-v6-assets\voice'
& 'C:\hy3d\venv\Scripts\python.exe' tools/end-game/v6_verify_voice.py
& tools/end-game/v6_check_dictation.ps1
```

The finish script writes only the four WAVs and manifest here. It uses NumPy and SciPy; the verifier needs NumPy and Python's standard WAV decoder. The optional dictation check uses an already-installed English UK Windows recognizer, takes WAV files as its sole audio input, and never opens a microphone or speaker.

## Verification scope

The offline verifier checks all four requested transcripts against the synthesis graphemes, nonempty phonemes, file hashes, PCM headers, frame counts, durations, payload budget, finite waveform values, silence boundaries, DC offset, signal activity, peak/RMS levels and absence of clipping. All four final files pass.

Free dictation with `MS-2057-80-DESK` recovered the complete movement and unlocking lines, ignoring punctuation. It recovered `Why you have a stored here` for the sword cue and `Had I will get revenge` for the death cue: the recognizer substituted “stored” for “sword” and did not recover the invented groan. This diagnostic helps compare processing choices; it is not a claim of exact automatic transcription for every line. No human listening, speaker playback, or in-game audio review was performed during asset generation.
