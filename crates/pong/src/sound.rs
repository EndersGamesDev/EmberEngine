//! Procedurally-synthesized sound effects — no asset files. The same
//! waveforms are generated on both platforms; playback is rodio on native
//! and Web Audio on wasm (context created lazily, after the first user
//! gesture, as browsers require).

const SAMPLE_RATE: u32 = 44_100;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Shot,
    Hit,
    Hurt,
    Kill,
    Death,
    Respawn,
}

const ALL: [Sfx; 6] = [Sfx::Shot, Sfx::Hit, Sfx::Hurt, Sfx::Kill, Sfx::Death, Sfx::Respawn];

/// Mono f32 samples at 44.1 kHz.
fn synth(sfx: Sfx) -> Vec<f32> {
    let sr = SAMPLE_RATE as f32;
    let mut rng: u32 = 0x1234_5678;
    let mut noise = move || -> f32 {
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        (rng >> 16) as f32 / 32768.0 - 1.0
    };
    // Pitch sweep f0->f1 over dur with exponential decay; shape morphs
    // between sine (0.0) and square (1.0).
    let mut sweep = |dur: f32, f0: f32, f1: f32, square: f32, decay: f32, noise_amt: f32| -> Vec<f32> {
        let n = (dur * sr) as usize;
        let mut phase = 0.0f32;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr;
                let f = f0 + (f1 - f0) * (t / dur);
                phase += std::f32::consts::TAU * f / sr;
                let s = phase.sin();
                let sq = if s >= 0.0 { 1.0 } else { -1.0 };
                let osc = s * (1.0 - square) + sq * square;
                let env = (-t * decay).exp();
                (osc + noise() * noise_amt) * env * 0.4
            })
            .collect()
    };
    match sfx {
        // Laser pew: fast downward square sweep with a noisy attack.
        Sfx::Shot => sweep(0.09, 950.0, 160.0, 0.7, 28.0, 0.25),
        // Hitmarker: short bright blip.
        Sfx::Hit => sweep(0.05, 1250.0, 1400.0, 0.2, 45.0, 0.0),
        // Taking damage: low thud.
        Sfx::Hurt => sweep(0.13, 170.0, 70.0, 0.4, 22.0, 0.5),
        // Frag: quick two-tone rise.
        Sfx::Kill => {
            let mut a = sweep(0.07, 520.0, 520.0, 0.3, 18.0, 0.0);
            a.extend(sweep(0.11, 780.0, 900.0, 0.3, 16.0, 0.0));
            a
        }
        // Dying: long fall.
        Sfx::Death => sweep(0.32, 420.0, 70.0, 0.5, 9.0, 0.15),
        // Respawn: soft rise.
        Sfx::Respawn => sweep(0.2, 240.0, 640.0, 0.0, 10.0, 0.0),
    }
}

pub use platform::Audio;

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::{synth, Sfx, ALL, SAMPLE_RATE};
    use std::collections::HashMap;

    pub struct Audio {
        // Field order matters: handle before stream so playback stops
        // cleanly; stream must stay alive for audio to play at all.
        handle: rodio::OutputStreamHandle,
        _stream: rodio::OutputStream,
        samples: HashMap<Sfx, Vec<f32>>,
    }

    impl Audio {
        pub fn new() -> Option<Audio> {
            let (stream, handle) = rodio::OutputStream::try_default().ok()?;
            let samples = ALL.iter().map(|&s| (s, synth(s))).collect();
            Some(Audio { handle, _stream: stream, samples })
        }

        pub fn play(&self, sfx: Sfx, vol: f32) {
            use rodio::Source;
            let Some(data) = self.samples.get(&sfx) else { return };
            let buf = rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, data.clone());
            let _ = self.handle.play_raw(buf.amplify(vol.clamp(0.0, 1.0)));
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{synth, Sfx, ALL, SAMPLE_RATE};
    use std::cell::RefCell;
    use std::collections::HashMap;

    pub struct Audio {
        ctx: RefCell<Option<web_sys::AudioContext>>,
        buffers: RefCell<HashMap<Sfx, web_sys::AudioBuffer>>,
    }

    impl Audio {
        pub fn new() -> Option<Audio> {
            // Context creation is deferred to the first play(): browsers
            // only allow audio after a user gesture.
            Some(Audio { ctx: RefCell::new(None), buffers: RefCell::new(HashMap::new()) })
        }

        pub fn play(&self, sfx: Sfx, vol: f32) {
            let mut ctx_slot = self.ctx.borrow_mut();
            if ctx_slot.is_none() {
                *ctx_slot = web_sys::AudioContext::new().ok();
                if let Some(ctx) = ctx_slot.as_ref() {
                    let mut buffers = self.buffers.borrow_mut();
                    for &s in ALL.iter() {
                        let mut data = synth(s);
                        if let Ok(buf) =
                            ctx.create_buffer(1, data.len() as u32, SAMPLE_RATE as f32)
                        {
                            let _ = buf.copy_to_channel(&mut data, 0);
                            buffers.insert(s, buf);
                        }
                    }
                }
            }
            let Some(ctx) = ctx_slot.as_ref() else { return };
            // A suspended context resumes after a gesture; fire-and-forget.
            let _ = ctx.resume();
            let buffers = self.buffers.borrow();
            let Some(buf) = buffers.get(&sfx) else { return };
            let (Ok(src), Ok(gain)) = (ctx.create_buffer_source(), ctx.create_gain()) else {
                return;
            };
            src.set_buffer(Some(buf));
            gain.gain().set_value(vol.clamp(0.0, 1.0));
            let _ = src.connect_with_audio_node(&gain);
            let _ = gain.connect_with_audio_node(&ctx.destination());
            let _ = src.start();
        }
    }
}
