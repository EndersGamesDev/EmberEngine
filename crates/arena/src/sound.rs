//! Procedurally-synthesized sound effects — no asset files. The same
//! waveforms are generated on both platforms; playback is rodio on native
//! and Web Audio on wasm (context created lazily, after the first user
//! gesture, as browsers require).

const SAMPLE_RATE: u32 = 44_100;
const SAMPLE_RATE_F32: f32 = 44_100.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    /// The sidearm's laser pew, today's shot.
    Shot,
    Hit,
    Hurt,
    Kill,
    Death,
    Respawn,
    Upgrade,
    Reload,
    /// The Vityaz: a short buzz that never settles between rounds.
    ShotSmg,
    /// The AK-47 (and the M4 at nine tenths): a low hammer.
    ShotRifle,
    /// The revolver: a hammer click, then the report.
    ShotRevolver,
    /// The sniper: a crack and a boom.
    ShotSniper,
    /// The RPG-7: a rising whoosh.
    Launch,
    /// A rocket detonating: the one cue that must survive a crowded frame.
    Blast,
    /// A head meeting an armed loot block from below.
    Bonk,
    /// A weapon popping out of a block.
    Pop,
    /// A trigger pulled during a reload, or a bonk on a dead block:
    /// "nothing happened", felt.
    Click,
    /// A looted gun running dry and the sidearm coming back.
    Holster,
}

/// Every variant, so both platforms synthesise the same set once.
const ALL: [Sfx; 18] = [
    Sfx::Shot,
    Sfx::Hit,
    Sfx::Hurt,
    Sfx::Kill,
    Sfx::Death,
    Sfx::Respawn,
    Sfx::Upgrade,
    Sfx::Reload,
    Sfx::ShotSmg,
    Sfx::ShotRifle,
    Sfx::ShotRevolver,
    Sfx::ShotSniper,
    Sfx::Launch,
    Sfx::Blast,
    Sfx::Bonk,
    Sfx::Pop,
    Sfx::Click,
    Sfx::Holster,
];

/// How many cues one frame may start. A backlogged burst (a hidden tab
/// catching up) must not blast every buffered cue at once.
pub const BUDGET: usize = 6;

impl Sfx {
    /// Playback priority, lower first. The frame budget drops the tail of
    /// the queue, so the cues that carry information a player cannot get
    /// any other way (a rocket went off, someone died, a block paid) sort
    /// before the ones the next state repeats anyway (another footfall of
    /// a burst, another remote shot). Being hurt and landing a hit sit
    /// between those: each happens once per event and a crowded frame is
    /// exactly when a player needs to hear that they were shot, so they
    /// must not queue behind a remote burst's footfalls.
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::Blast => 0,
            Self::Death => 1,
            Self::Kill => 2,
            Self::Pop => 3,
            Self::Bonk => 4,
            Self::Hurt | Self::Hit => 5,
            _ => 6,
        }
    }
}

/// Order a frame's queued cues so that `take(BUDGET)` keeps the important
/// ones. A stable sort, so two cues of one priority still play in the order
/// the frame raised them.
pub fn prioritize(queue: &mut [(Sfx, f32)]) {
    queue.sort_by_key(|(s, _)| s.priority());
}

// Sound durations are finite and nonnegative, and truncation selects a whole sample count.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn sample_count(duration: f32) -> usize {
    (duration * SAMPLE_RATE_F32) as usize
}

/// Mono f32 samples at 44.1 kHz.
#[allow(clippy::too_many_lines)]
fn synth(sfx: Sfx) -> Vec<f32> {
    let sr = SAMPLE_RATE_F32;
    let mut rng: u32 = 0x1234_5678;
    let mut noise = move || -> f32 {
        rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let upper = u16::try_from(rng >> 16).expect("the shifted RNG value fits in u16");
        f32::from(upper) / 32768.0 - 1.0
    };
    // Pitch sweep f0->f1 over dur with exponential decay; shape morphs
    // between sine (0.0) and square (1.0).
    let mut sweep =
        |dur: f32, f0: f32, f1: f32, square: f32, decay: f32, noise_amt: f32| -> Vec<f32> {
            let n = sample_count(dur);
            let mut phase = 0.0f32;
            (0..n)
                .map(|i| {
                    let sample =
                        u16::try_from(i).expect("sound effects contain fewer than 65k samples");
                    let t = f32::from(sample) / sr;
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
        // Weapon upgrade: bright triumphant rise.
        Sfx::Upgrade => {
            let mut a = sweep(0.08, 420.0, 420.0, 0.2, 14.0, 0.0);
            a.extend(sweep(0.16, 640.0, 1050.0, 0.2, 10.0, 0.0));
            a
        }
        // Reload: two mechanical clicks with a gap.
        Sfx::Reload => {
            let mut a = sweep(0.035, 1900.0, 1500.0, 0.85, 70.0, 0.2);
            a.extend(std::iter::repeat_n(0.0, sample_count(0.08)));
            a.extend(sweep(0.045, 1300.0, 900.0, 0.85, 60.0, 0.2));
            a
        }
        // The SMG: short and buzzy, decaying fast so a 12.5 Hz burst reads
        // as rounds, not as a tone.
        Sfx::ShotSmg => sweep(0.05, 700.0, 300.0, 0.8, 40.0, 0.3),
        // The rifle: a low hammer with a noisy edge.
        Sfx::ShotRifle => sweep(0.11, 230.0, 90.0, 0.7, 24.0, 0.45),
        // The revolver: the hammer's click, then a long low report.
        Sfx::ShotRevolver => {
            let mut a = sweep(0.012, 2400.0, 1800.0, 1.0, 120.0, 0.0);
            a.extend(sweep(0.16, 150.0, 55.0, 0.6, 16.0, 0.5));
            a
        }
        // The sniper: a supersonic crack over a boom.
        Sfx::ShotSniper => {
            let mut a = sweep(0.04, 1800.0, 400.0, 0.9, 60.0, 0.6);
            a.extend(sweep(0.22, 120.0, 45.0, 0.5, 12.0, 0.35));
            a
        }
        // The rocket leaving the tube: a rising whoosh, mostly noise.
        Sfx::Launch => sweep(0.25, 90.0, 400.0, 0.2, 10.0, 0.9),
        // The detonation: sub-bass and noise, with a rumbling tail.
        Sfx::Blast => {
            let mut a = sweep(0.45, 70.0, 28.0, 0.5, 7.0, 0.95);
            a.extend(sweep(0.30, 40.0, 25.0, 0.2, 8.0, 0.6));
            a
        }
        // The bonk: a rising sine boing, the Mario note.
        Sfx::Bonk => sweep(0.09, 260.0, 820.0, 0.0, 22.0, 0.05),
        // The pop: two bright bell tones a fifth apart.
        Sfx::Pop => {
            let mut a = sweep(0.06, 1480.0, 1480.0, 0.1, 30.0, 0.0);
            a.extend(sweep(0.14, 1975.0, 1975.0, 0.1, 18.0, 0.0));
            a
        }
        // The dry click: hard, short, dull.
        Sfx::Click => sweep(0.025, 900.0, 700.0, 1.0, 120.0, 0.3),
        // The holster: a slap down, then the sidearm's rising draw.
        Sfx::Holster => {
            let mut a = sweep(0.05, 600.0, 300.0, 0.6, 50.0, 0.4);
            a.extend(sweep(0.08, 380.0, 700.0, 0.2, 20.0, 0.0));
            a
        }
    }
}

pub use platform::Audio;

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::{ALL, SAMPLE_RATE, Sfx, synth};
    use std::collections::HashMap;

    pub struct Audio {
        // Field order matters: handle before stream so playback stops
        // cleanly; stream must stay alive for audio to play at all.
        handle: rodio::OutputStreamHandle,
        _stream: rodio::OutputStream,
        samples: HashMap<Sfx, Vec<f32>>,
    }

    impl Audio {
        pub fn new() -> Option<Self> {
            let (stream, handle) = rodio::OutputStream::try_default().ok()?;
            let samples = ALL.iter().map(|&s| (s, synth(s))).collect();
            Some(Self {
                handle,
                _stream: stream,
                samples,
            })
        }

        pub fn play(&self, sfx: Sfx, vol: f32) {
            use rodio::Source;
            let Some(data) = self.samples.get(&sfx) else {
                return;
            };
            let buf = rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, data.clone());
            let _ = self.handle.play_raw(buf.amplify(vol.clamp(0.0, 1.0)));
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{ALL, SAMPLE_RATE, Sfx, synth};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    struct Inner {
        ctx: RefCell<Option<web_sys::AudioContext>>,
        buffers: RefCell<HashMap<Sfx, web_sys::AudioBuffer>>,
    }

    impl Inner {
        /// Create/resume the context. MUST be called from a user-gesture
        /// call stack at least once (Safari and autoplay policies) — the
        /// pointerdown listener below guarantees that.
        fn ensure(&self) {
            let mut ctx_slot = self.ctx.borrow_mut();
            if ctx_slot.is_none() {
                *ctx_slot = web_sys::AudioContext::new().ok();
                if let Some(ctx) = ctx_slot.as_ref() {
                    let mut buffers = self.buffers.borrow_mut();
                    for &s in ALL.iter() {
                        let mut data = synth(s);
                        if let Ok(buf) = ctx.create_buffer(1, data.len() as u32, SAMPLE_RATE as f32)
                        {
                            drop(buf.copy_to_channel(&mut data, 0));
                            buffers.insert(s, buf);
                        }
                    }
                }
            }
            if let Some(ctx) = ctx_slot.as_ref() {
                drop(ctx.resume());
            }
        }
    }

    pub struct Audio {
        inner: Rc<Inner>,
        /// Kept alive for the page's lifetime: every click re-ensures the
        /// context (also un-suspends it after tab switches).
        _gesture: Closure<dyn FnMut(web_sys::Event)>,
    }

    impl Audio {
        pub fn new() -> Option<Audio> {
            let inner = Rc::new(Inner {
                ctx: RefCell::new(None),
                buffers: RefCell::new(HashMap::new()),
            });
            let gesture = {
                let inner = Rc::clone(&inner);
                Closure::<dyn FnMut(web_sys::Event)>::new(move |_| inner.ensure())
            };
            let doc = web_sys::window()?.document()?;
            doc.add_event_listener_with_callback("pointerdown", gesture.as_ref().unchecked_ref())
                .ok()?;
            Some(Audio {
                inner,
                _gesture: gesture,
            })
        }

        pub fn play(&self, sfx: Sfx, vol: f32) {
            // No creation here: outside a gesture stack it would be blocked
            // anyway; the pointerdown listener owns initialization.
            let ctx_slot = self.inner.ctx.borrow();
            let Some(ctx) = ctx_slot.as_ref() else { return };
            let buffers = self.inner.buffers.borrow();
            let Some(buf) = buffers.get(&sfx) else { return };
            let (Ok(src), Ok(gain)) = (ctx.create_buffer_source(), ctx.create_gain()) else {
                return;
            };
            src.set_buffer(Some(buf));
            gain.gain().set_value(vol.clamp(0.0, 1.0));
            drop(src.connect_with_audio_node(&gain));
            drop(gain.connect_with_audio_node(&ctx.destination()));
            drop(src.start());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant synthesises to a non-empty, finite, bounded buffer
    /// under 65k samples (the sweep's own sample index limit), so a new cue
    /// that runs too long fails here and not on a player's first shot.
    #[test]
    fn every_cue_synthesises_within_the_sample_limit() {
        for s in ALL {
            let data = synth(s);
            assert!(!data.is_empty(), "{s:?}: empty");
            assert!(data.len() < 65_536 * 2, "{s:?}: {} samples", data.len());
            for v in &data {
                assert!(v.is_finite() && v.abs() <= 1.0, "{s:?}: sample {v}");
            }
        }
    }
}
