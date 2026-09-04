//! Sound effects: a synthesis kit, every cue built from it, and playback.
//!
//! Every cue is synthesised at 44.1 kHz from a small kit of pure,
//! deterministic functions, so the client ships no audio asset and both
//! platforms build the same waveforms; a recorded sample dropped into
//! `assets/sfx/` replaces one cue's synth through the `RECORDED` table (the
//! folder's README is the contract). Playback is rodio on native and Web
//! Audio on wasm (context created lazily, after the first user gesture, as
//! browsers require); a spatial play carries a pan and a delay so a remote
//! shot arrives late from the side it was fired from.
//!
//! The DSP is written as the textbook formulas on purpose: a `mul_add`
//! rewrite would hide the filter shape from the next reader, and the cues
//! are short enough that the rounding does not reach the ear.
#![allow(clippy::imprecise_flops)]

use std::borrow::Cow;
use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_4, TAU};

const SAMPLE_RATE: u32 = 44_100;
const SAMPLE_RATE_F32: f32 = 44_100.0;

/// Metres per second: a remote cue is played `distance / SPEED_OF_SOUND`
/// late, and a round above it cracks as it passes.
#[allow(dead_code)] // Read by the v20 client's spatial routing (plan section 5).
pub const SPEED_OF_SOUND: f32 = 343.0;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    /// The sidearm's laser pew, the v13 shot; the v20 client plays
    /// `ShotSidearmNear` instead and this stays for the frozen pages' feel.
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
    // The v20 layered gunshots: one row of `GUNS` in three distance
    // variants each. `Sfx::shot(weapon, dist)` picks one.
    ShotSidearmNear,
    ShotSidearmMid,
    ShotSidearmFar,
    ShotVityazNear,
    ShotVityazMid,
    ShotVityazFar,
    ShotAkNear,
    ShotAkMid,
    ShotAkFar,
    ShotM4Near,
    ShotM4Mid,
    ShotM4Far,
    ShotRevolverNear,
    ShotRevolverMid,
    ShotRevolverFar,
    ShotSniperNear,
    ShotSniperMid,
    ShotSniperFar,
    ShotRpgNear,
    ShotRpgMid,
    ShotRpgFar,
    /// A supersonic round passing close: the shock cone, no delay.
    Crack,
    /// A round meeting a container, the city wall or the arena wall.
    ImpactMetal,
    /// A round meeting a plinth, rubble, the cobbles or a roof.
    ImpactStone,
    /// A round meeting a crate or an ammo box.
    ImpactWood,
    /// A round meeting a sandbag.
    ImpactSand,
    /// A round meeting a body.
    ImpactBody,
    /// One metal hit in eight: the whine off the plate.
    Ricochet,
    /// A brass casing landing on the cobbles, two bounces.
    Casing,
    /// Mag out, mag in, slide.
    ReloadPistol,
    /// Mag out, mag in, bolt.
    ReloadRifle,
    /// Cylinder out, six rounds dropped, cylinder in.
    ReloadRevolver,
    /// Bolt back, forward.
    ReloadSniper,
    /// A hollow tube slide.
    ReloadRpg,
}

/// Every variant, so both platforms synthesise the same set once and the
/// tests walk every cue.
const ALL: [Sfx; 52] = [
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
    Sfx::ShotSidearmNear,
    Sfx::ShotSidearmMid,
    Sfx::ShotSidearmFar,
    Sfx::ShotVityazNear,
    Sfx::ShotVityazMid,
    Sfx::ShotVityazFar,
    Sfx::ShotAkNear,
    Sfx::ShotAkMid,
    Sfx::ShotAkFar,
    Sfx::ShotM4Near,
    Sfx::ShotM4Mid,
    Sfx::ShotM4Far,
    Sfx::ShotRevolverNear,
    Sfx::ShotRevolverMid,
    Sfx::ShotRevolverFar,
    Sfx::ShotSniperNear,
    Sfx::ShotSniperMid,
    Sfx::ShotSniperFar,
    Sfx::ShotRpgNear,
    Sfx::ShotRpgMid,
    Sfx::ShotRpgFar,
    Sfx::Crack,
    Sfx::ImpactMetal,
    Sfx::ImpactStone,
    Sfx::ImpactWood,
    Sfx::ImpactSand,
    Sfx::ImpactBody,
    Sfx::Ricochet,
    Sfx::Casing,
    Sfx::ReloadPistol,
    Sfx::ReloadRifle,
    Sfx::ReloadRevolver,
    Sfx::ReloadSniper,
    Sfx::ReloadRpg,
];

/// How many cues one frame may start. A backlogged burst (a hidden tab
/// catching up) must not blast every buffered cue at once. Spatial plays
/// count against it like centred ones.
pub const BUDGET: usize = 6;

/// How far a remote shot was fired from, which picks the gunshot variant:
/// the near one carries the mechanism and every high, the mid one loses
/// the mechanism, the far one is the low boom and its tail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dist {
    Near,
    Mid,
    Far,
}

impl Dist {
    /// The variant for a source this many metres away: near under 12 m,
    /// mid under 40 m, far beyond.
    #[must_use]
    pub const fn at(metres: f32) -> Self {
        if metres < 12.0 {
            Self::Near
        } else if metres < 40.0 {
            Self::Mid
        } else {
            Self::Far
        }
    }
}

impl Sfx {
    /// Playback priority, lower first. The frame budget drops the tail of
    /// the queue, so the cues that carry information a player cannot get
    /// any other way (a rocket went off, someone died, a round just missed
    /// my head, my round hit flesh, a block paid) sort before the ones the
    /// next event repeats anyway (another footfall of a burst, another
    /// remote shot). Being hurt and landing a hit sit between those: each
    /// happens once per event and a crowded frame is exactly when a player
    /// needs to hear that they were shot, so they must not queue behind a
    /// remote burst's footfalls. Impacts on the world, ricochets and
    /// casings are decoration and go last, so a crowded frame drops a
    /// casing before a shot.
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::Blast => 0,
            Self::Death => 1,
            Self::Kill => 2,
            Self::Crack => 3,
            Self::ImpactBody => 4,
            Self::Pop => 5,
            Self::Bonk => 6,
            Self::Hurt | Self::Hit => 7,
            Self::ImpactMetal
            | Self::ImpactStone
            | Self::ImpactWood
            | Self::ImpactSand
            | Self::Ricochet
            | Self::Casing => 9,
            _ => 8,
        }
    }

    /// The layered shot of weapon id 1..=7 at a distance, `None` for an id
    /// that is not a weapon.
    #[must_use]
    pub const fn shot(weapon: u8, dist: Dist) -> Option<Self> {
        use Dist::{Far, Mid, Near};
        Some(match (weapon, dist) {
            (1, Near) => Self::ShotSidearmNear,
            (1, Mid) => Self::ShotSidearmMid,
            (1, Far) => Self::ShotSidearmFar,
            (2, Near) => Self::ShotVityazNear,
            (2, Mid) => Self::ShotVityazMid,
            (2, Far) => Self::ShotVityazFar,
            (3, Near) => Self::ShotAkNear,
            (3, Mid) => Self::ShotAkMid,
            (3, Far) => Self::ShotAkFar,
            (4, Near) => Self::ShotM4Near,
            (4, Mid) => Self::ShotM4Mid,
            (4, Far) => Self::ShotM4Far,
            (5, Near) => Self::ShotRevolverNear,
            (5, Mid) => Self::ShotRevolverMid,
            (5, Far) => Self::ShotRevolverFar,
            (6, Near) => Self::ShotSniperNear,
            (6, Mid) => Self::ShotSniperMid,
            (6, Far) => Self::ShotSniperFar,
            (7, Near) => Self::ShotRpgNear,
            (7, Mid) => Self::ShotRpgMid,
            (7, Far) => Self::ShotRpgFar,
            _ => return None,
        })
    }

    /// The reload that belongs to weapon id 1..=7: the sidearm's pistol
    /// reload, the three magazine rifles' one, the revolver's, the
    /// sniper's bolt and the tube; anything else gets the v18 clicks.
    #[must_use]
    pub const fn reload(weapon: u8) -> Self {
        match weapon {
            1 => Self::ReloadPistol,
            2..=4 => Self::ReloadRifle,
            5 => Self::ReloadRevolver,
            6 => Self::ReloadSniper,
            7 => Self::ReloadRpg,
            _ => Self::Reload,
        }
    }

    /// The name a recorded sample of this cue carries in `assets/sfx/`
    /// (`<name>.wav`), which is also the CSV name the plot helper writes.
    /// The slot's tests and README are its readers; the player never is.
    #[must_use]
    #[allow(dead_code)]
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Shot => "shot",
            Self::Hit => "hit",
            Self::Hurt => "hurt",
            Self::Kill => "kill",
            Self::Death => "death",
            Self::Respawn => "respawn",
            Self::Upgrade => "upgrade",
            Self::Reload => "reload",
            Self::ShotSmg => "shot_smg",
            Self::ShotRifle => "shot_rifle",
            Self::ShotRevolver => "shot_revolver",
            Self::ShotSniper => "shot_sniper",
            Self::Launch => "launch",
            Self::Blast => "blast",
            Self::Bonk => "bonk",
            Self::Pop => "pop",
            Self::Click => "click",
            Self::Holster => "holster",
            Self::ShotSidearmNear => "shot_sidearm_near",
            Self::ShotSidearmMid => "shot_sidearm_mid",
            Self::ShotSidearmFar => "shot_sidearm_far",
            Self::ShotVityazNear => "shot_vityaz_near",
            Self::ShotVityazMid => "shot_vityaz_mid",
            Self::ShotVityazFar => "shot_vityaz_far",
            Self::ShotAkNear => "shot_ak_near",
            Self::ShotAkMid => "shot_ak_mid",
            Self::ShotAkFar => "shot_ak_far",
            Self::ShotM4Near => "shot_m4_near",
            Self::ShotM4Mid => "shot_m4_mid",
            Self::ShotM4Far => "shot_m4_far",
            Self::ShotRevolverNear => "shot_revolver_near",
            Self::ShotRevolverMid => "shot_revolver_mid",
            Self::ShotRevolverFar => "shot_revolver_far",
            Self::ShotSniperNear => "shot_sniper_near",
            Self::ShotSniperMid => "shot_sniper_mid",
            Self::ShotSniperFar => "shot_sniper_far",
            Self::ShotRpgNear => "shot_rpg_near",
            Self::ShotRpgMid => "shot_rpg_mid",
            Self::ShotRpgFar => "shot_rpg_far",
            Self::Crack => "crack",
            Self::ImpactMetal => "impact_metal",
            Self::ImpactStone => "impact_stone",
            Self::ImpactWood => "impact_wood",
            Self::ImpactSand => "impact_sand",
            Self::ImpactBody => "impact_body",
            Self::Ricochet => "ricochet",
            Self::Casing => "casing",
            Self::ReloadPistol => "reload_pistol",
            Self::ReloadRifle => "reload_rifle",
            Self::ReloadRevolver => "reload_revolver",
            Self::ReloadSniper => "reload_sniper",
            Self::ReloadRpg => "reload_rpg",
        }
    }
}

/// Order a frame's queued cues so that `take(BUDGET)` keeps the important
/// ones. A stable sort, so two cues of one priority still play in the order
/// the frame raised them. The v20 client sorts its spatial queue by the
/// same `priority` through `feel::prioritize_plays`; this is the plain
/// form the tests pin the rule on.
#[allow(dead_code)]
pub fn prioritize(queue: &mut [(Sfx, f32)]) {
    queue.sort_by_key(|(s, _)| s.priority());
}

/// The constant-power pan law: left and right gains for `pan` in -1..1.
///
/// `theta = (pan + 1) * pi / 4`, left `cos`, right `sin`, so a centred cue
/// sits at -3 dB on both sides and a hard pan puts all of it on one side.
/// The web panner does the same thing inside `StereoPannerNode`; native
/// builds the two channels from this.
#[must_use]
#[allow(dead_code)] // The web build leaves the law to its panner node.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let theta = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (theta.cos(), theta.sin())
}

// Sound durations are finite and nonnegative, and truncation selects a whole sample count.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn sample_count(duration: f32) -> usize {
    (duration.max(0.0) * SAMPLE_RATE_F32) as usize
}

// A sample index is far below 2^24, so the f32 holds it exactly.
#[allow(clippy::cast_precision_loss)]
fn secs(i: usize) -> f32 {
    i as f32 / SAMPLE_RATE_F32
}

// ---------------------------------------------------------------------------
// 6.1 The kit
// ---------------------------------------------------------------------------

/// White noise in -1..1 for `dur` seconds from a seeded LCG. The seed is
/// the cue's, so the same cue is bit-identical on every build and platform
/// and two cues never share a burst.
#[must_use]
pub fn noise(dur: f32, seed: u32) -> Vec<f32> {
    let mut rng = seed;
    (0..sample_count(dur))
        .map(|_| {
            rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            // The shifted value is 16 bits wide, so the fallback never runs.
            let upper = u16::try_from(rng >> 16).unwrap_or(u16::MAX);
            f32::from(upper) / 32768.0 - 1.0
        })
        .collect()
}

/// A sine sweeping exponentially from `f0` to `f1` over `dur` seconds, at
/// unit amplitude; a sweep is how a body tone falls and a ricochet whines.
#[must_use]
pub fn sweep(f0: f32, f1: f32, dur: f32) -> Vec<f32> {
    let n = sample_count(dur);
    let ratio = if f0 > 0.0 { f1 / f0 } else { 1.0 };
    let mut phase = 0.0f32;
    (0..n)
        .map(|i| {
            let u = if n > 1 { secs(i) / dur } else { 0.0 };
            let f = f0 * ratio.powf(u);
            phase += TAU * f / SAMPLE_RATE_F32;
            phase.sin()
        })
        .collect()
}

/// A steady sine at `f` for `dur` seconds.
#[must_use]
pub fn sine(f: f32, dur: f32) -> Vec<f32> {
    sweep(f, f, dur)
}

/// A gain curve: rise over `attack`, `hold` at one, fall over `decay`.
///
/// The fall is `(1 - u)^curve`, so `curve` 1 is a straight line and 3
/// drops fast and lands softly, which is how a percussive hit decays
/// without the tail of an exponential that never reaches silence.
#[must_use]
pub fn envelope(attack: f32, hold: f32, decay: f32, curve: f32) -> Vec<f32> {
    let na = sample_count(attack);
    let nh = sample_count(hold);
    let nd = sample_count(decay);
    let mut env = Vec::with_capacity(na + nh + nd);
    env.extend((0..na).map(|i| secs(i) / attack));
    env.extend(std::iter::repeat_n(1.0, nh));
    env.extend((0..nd).map(|i| (1.0 - secs(i) / decay).max(0.0).powf(curve)));
    env
}

/// `buf` multiplied by `env` sample by sample, as long as the shorter.
#[must_use]
pub fn shaped(buf: &[f32], env: &[f32]) -> Vec<f32> {
    buf.iter().zip(env).map(|(s, e)| s * e).collect()
}

/// One pole at `cutoff` in the bilinear form, whose low-pass is exactly
/// silent at Nyquist and whose high-pass is exactly unity there; the naive
/// `y += a * (x - y)` recurrence leaks a quarter of the top octave through
/// both, which is what the octave test measures.
struct OnePole {
    alpha: f32,
    low_gain: f32,
    high_gain: f32,
    x1: f32,
    y1: f32,
}

impl OnePole {
    fn new(cutoff: f32) -> Self {
        let k = (std::f32::consts::PI * cutoff.clamp(1.0, 20_000.0) / SAMPLE_RATE_F32).tan();
        let alpha = (1.0 - k) / (1.0 + k);
        Self {
            alpha,
            low_gain: (1.0 - alpha) * 0.5,
            high_gain: 0.5 + alpha * 0.5,
            x1: 0.0,
            y1: 0.0,
        }
    }

    fn low(&mut self, x: f32) -> f32 {
        let y = self.alpha * self.y1 + self.low_gain * (x + self.x1);
        self.x1 = x;
        self.y1 = y;
        y
    }

    fn high(&mut self, x: f32) -> f32 {
        let y = self.alpha * self.y1 + self.high_gain * (x - self.x1);
        self.x1 = x;
        self.y1 = y;
        y
    }
}

/// Six dB per octave, low or high; the public filters run it twice for
/// twelve.
fn one_pole(buf: &[f32], cutoff: f32, high: bool) -> Vec<f32> {
    let mut f = OnePole::new(cutoff);
    buf.iter()
        .map(|&x| if high { f.high(x) } else { f.low(x) })
        .collect()
}

/// Twelve dB per octave above `cutoff`: the distance, the far variant,
/// the tail's dullness.
#[must_use]
pub fn lowpass(buf: &[f32], cutoff: f32) -> Vec<f32> {
    one_pole(&one_pole(buf, cutoff, false), cutoff, false)
}

/// Twelve dB per octave below `cutoff`: a click's brightness, the shock
/// cone's hiss.
#[must_use]
pub fn highpass(buf: &[f32], cutoff: f32) -> Vec<f32> {
    one_pole(&one_pole(buf, cutoff, true), cutoff, true)
}

/// A band-pass whose centre sweeps exponentially from `f0` to `f1`.
///
/// The cookbook biquad (constant zero dB peak gain), its coefficients
/// recomputed each sample, which is cheap at these lengths and is what the
/// rocket's rising whoosh needs.
#[must_use]
#[allow(clippy::many_single_char_names, clippy::similar_names)]
pub fn bandpass_sweep(buf: &[f32], f0: f32, f1: f32, q: f32) -> Vec<f32> {
    let n = buf.len();
    let ratio = if f0 > 0.0 { f1 / f0 } else { 1.0 };
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    buf.iter()
        .enumerate()
        .map(|(i, &x)| {
            let u = if n > 1 { secs(i) / secs(n - 1) } else { 0.0 };
            let w0 = TAU * f0 * ratio.powf(u) / SAMPLE_RATE_F32;
            let alpha = w0.sin() / (2.0 * q);
            let a0 = 1.0 + alpha;
            let a1 = -2.0 * w0.cos();
            let a2 = 1.0 - alpha;
            let y = (alpha * x - alpha * x2 - a1 * y1 - a2 * y2) / a0;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            y
        })
        .collect()
}

/// The band around `centre` with quality `q`: a blast's colour, a stone
/// hit's dryness.
#[must_use]
pub fn bandpass(buf: &[f32], centre: f32, q: f32) -> Vec<f32> {
    bandpass_sweep(buf, centre, centre, q)
}

/// The gain-weighted sum of layers, as long as the longest.
#[must_use]
pub fn mix(layers: &[(&[f32], f32)]) -> Vec<f32> {
    let n = layers.iter().map(|(b, _)| b.len()).max().unwrap_or(0);
    let mut out = vec![0.0f32; n];
    for (buf, gain) in layers {
        for (o, s) in out.iter_mut().zip(buf.iter()) {
            *o += s * gain;
        }
    }
    out
}

/// Add `src` into `dst` starting `at` seconds in, growing `dst` as needed:
/// how a reload's clicks are laid out in time.
pub fn place(dst: &mut Vec<f32>, src: &[f32], at: f32, gain: f32) {
    let start = sample_count(at);
    if dst.len() < start + src.len() {
        dst.resize(start + src.len(), 0.0);
    }
    for (o, s) in dst[start..].iter_mut().zip(src) {
        *o += s * gain;
    }
}

/// The longest tail `delay_tail` will grow past its input.
const TAIL_CAP: f32 = 2.0;

/// The reverberant tail of a yard between containers.
///
/// A comb whose loop delays `secs` and feeds back `feedback` of itself
/// through a low-pass at `cutoff`, so each return is duller than the last.
/// Only the returns are produced (the dry signal is the caller's own
/// layer); the buffer runs until the loop has decayed sixty dB, capped at
/// `TAIL_CAP` seconds.
#[must_use]
pub fn delay_tail(buf: &[f32], secs_: f32, feedback: f32, cutoff: f32) -> Vec<f32> {
    let d = sample_count(secs_).max(1);
    let fb = feedback.clamp(0.0, 0.98);
    let loops = if fb > 0.0 {
        (-3.0 / fb.log10()).ceil()
    } else {
        1.0
    };
    let extra = sample_count(loops * secs(d)).min(sample_count(TAIL_CAP));
    let n = buf.len() + extra;
    let mut lp = OnePole::new(cutoff);
    let mut out = vec![0.0f32; n];
    for i in 0..n {
        let back = if i >= d {
            buf.get(i - d).copied().unwrap_or(0.0) + out[i - d]
        } else {
            0.0
        };
        out[i] = fb * lp.low(back);
    }
    out
}

/// `tanh(x * drive) / tanh(drive)`: rounds the peaks of a layered shot so
/// the sum stays inside the buffer without a hard edge.
#[must_use]
pub fn soft_clip(buf: &[f32], drive: f32) -> Vec<f32> {
    let scale = 1.0 / drive.tanh();
    buf.iter().map(|&x| (x * drive).tanh() * scale).collect()
}

/// Scale so the loudest sample is `peak` (silence stays silence).
#[must_use]
pub fn normalize(buf: &[f32], peak: f32) -> Vec<f32> {
    let max = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if max <= 0.0 {
        return buf.to_vec();
    }
    let g = peak / max;
    buf.iter().map(|&x| x * g).collect()
}

// ---------------------------------------------------------------------------
// 6.2 A gunshot
// ---------------------------------------------------------------------------

/// One gun's voice.
///
/// The blast band is the colour of the report, the body is the low tone
/// that carries the calibre, the tail is how long the yard answers; the two
/// flags are the sniper's whipcrack and the rocket's launch whoosh, which
/// no other row has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunParams {
    pub blast_hz: f32,
    pub body_hz: f32,
    pub body_ms: f32,
    pub tail_ms: f32,
    pub whipcrack: bool,
    pub whoosh: bool,
    /// The noise seed, distinct per row so no two guns share a burst.
    pub seed: u32,
}

/// The seven rows of the v20 plan, indexed by weapon id minus one.
pub const GUNS: [GunParams; 7] = [
    // 1 Sidearm: a flat, snappy .45.
    GunParams {
        blast_hz: 1400.0,
        body_hz: 140.0,
        body_ms: 90.0,
        tail_ms: 260.0,
        whipcrack: false,
        whoosh: false,
        seed: 0x1000_0001,
    },
    // 2 Vityaz: quick and dry, the SMG chatter.
    GunParams {
        blast_hz: 1800.0,
        body_hz: 160.0,
        body_ms: 60.0,
        tail_ms: 200.0,
        whipcrack: false,
        whoosh: false,
        seed: 0x1000_0002,
    },
    // 3 AK-47: the heavy intermediate crack with a long yard tail.
    GunParams {
        blast_hz: 900.0,
        body_hz: 110.0,
        body_ms: 140.0,
        tail_ms: 420.0,
        whipcrack: false,
        whoosh: false,
        seed: 0x1000_0003,
    },
    // 4 M4: sharper than the AK.
    GunParams {
        blast_hz: 1600.0,
        body_hz: 130.0,
        body_ms: 100.0,
        tail_ms: 340.0,
        whipcrack: false,
        whoosh: false,
        seed: 0x1000_0004,
    },
    // 5 Revolver: the deepest, longest.
    GunParams {
        blast_hz: 700.0,
        body_hz: 90.0,
        body_ms: 200.0,
        tail_ms: 520.0,
        whipcrack: false,
        whoosh: false,
        seed: 0x1000_0005,
    },
    // 6 Sniper: a boom with a whipcrack layered.
    GunParams {
        blast_hz: 500.0,
        body_hz: 70.0,
        body_ms: 260.0,
        tail_ms: 700.0,
        whipcrack: true,
        whoosh: false,
        seed: 0x1000_0006,
    },
    // 7 RPG-7: the launch whoosh over a low body.
    GunParams {
        blast_hz: 300.0,
        body_hz: 60.0,
        body_ms: 400.0,
        tail_ms: 900.0,
        whipcrack: false,
        whoosh: true,
        seed: 0x1000_0007,
    },
];

/// The three comb delays of the yard tail: round trips off containers
/// three to seven metres away, at lengths that do not share a period so
/// the returns do not pile into one flutter.
const YARD_DELAYS: [f32; 3] = [0.019, 0.029, 0.041];

/// The feedback that lands a comb of loop `delay` at sixty dB down after
/// `tail` seconds.
fn feedback_for(delay: f32, tail: f32) -> f32 {
    10f32.powf(-3.0 * delay / tail.max(delay)).min(0.95)
}

/// The near variant of a gun's shot: the plan's name for the voice, which
/// the cue dispatcher reaches through `gunshot_at`.
#[must_use]
#[allow(dead_code)]
pub fn gunshot(p: &GunParams) -> Vec<f32> {
    gunshot_at(p, Dist::Near)
}

/// The layers of one gun's shot before they are mixed.
///
/// Each sits at its own natural level; `gunshot_at` mixes them, and they
/// are public so a probe can write them out and read a variant's balance
/// off a picture.
pub struct GunLayers {
    /// A 3 ms click of high-passed noise, peak one.
    pub mechanism: Vec<f32>,
    /// 8 to 20 ms of noise in the gun's band, longer for a lower band,
    /// with a 0.3 ms attack, peak one: the loudest layer.
    pub blast: Vec<f32>,
    /// A sine falling half an octave over `body_ms`, decaying sixty dB by
    /// its end, peak one.
    pub body: Vec<f32>,
    /// The blast through three yard combs, low-passed at 2.5 kHz, at the
    /// level the yard returns it.
    pub tail: Vec<f32>,
    /// The sniper's 2 ms of 4 kHz, to be placed 8 ms after the blast.
    pub whip: Option<Vec<f32>>,
    /// The rocket's noise band rising from 300 Hz over 250 ms.
    pub whoosh: Option<Vec<f32>>,
}

/// The decay curve of a body: `(1 - u)^8` is seven dB down a tenth of the
/// way in and sixty dB down near the end, the way a struck body rings
/// out, so a 400 ms body has fallen well under its blast by the time the
/// yard's returns arrive and the shot's peak stays at its front.
const BODY_CURVE: f32 = 8.0;

#[must_use]
pub fn gunshot_layers(p: &GunParams) -> GunLayers {
    let blast_secs = (20.0 - p.blast_hz / 150.0).clamp(8.0, 20.0) * 0.001;
    let body_secs = p.body_ms * 0.001;
    let tail_secs = p.tail_ms * 0.001;

    let mechanism = normalize(
        &shaped(
            &highpass(&noise(0.003, p.seed), 2000.0),
            &envelope(0.0, 0.0005, 0.0025, 1.5),
        ),
        1.0,
    );
    let blast = normalize(
        &shaped(
            &bandpass(
                &noise(blast_secs + 0.0003, p.seed ^ 0x5bd1_e995),
                p.blast_hz,
                0.7,
            ),
            &envelope(0.0003, 0.0, blast_secs, 2.0),
        ),
        1.0,
    );
    let body = shaped(
        &sweep(p.body_hz, p.body_hz * FRAC_1_SQRT_2, body_secs),
        &envelope(0.001, 0.0, body_secs - 0.001, BODY_CURVE),
    );
    // Each comb is weighted by the root of what it does not feed back: a
    // long tail is a big yard whose first return is weaker, so the three
    // returns of a 900 ms tail (which overlap, the blast being longer
    // than their spacing) do not pile up over the direct blast. The first
    // return of the AK's tail lands thirteen dB under its blast up close.
    let combs: Vec<Vec<f32>> = YARD_DELAYS
        .iter()
        .map(|&d| {
            let fb = feedback_for(d, tail_secs);
            let comb = delay_tail(&blast, d, fb, 2500.0);
            mix(&[(&comb, (1.0 - fb).sqrt())])
        })
        .collect();
    let tail = lowpass(
        &mix(&[(&combs[0], 0.6), (&combs[1], 0.5), (&combs[2], 0.4)]),
        2500.0,
    );
    let whip = p
        .whipcrack
        .then(|| shaped(&sine(4000.0, 0.002), &envelope(0.0002, 0.0, 0.0018, 1.0)));
    // The whoosh peaks with the blast and falls away under it, so the
    // launch reads as one event and not as a second one 40 ms later.
    let whoosh = p.whoosh.then(|| {
        normalize(
            &shaped(
                &bandpass_sweep(&noise(0.25, p.seed ^ 0x27d4_eb2f), 300.0, 1200.0, 2.0),
                &envelope(0.002, 0.005, 0.243, 4.0),
            ),
            1.0,
        )
    });
    GunLayers {
        mechanism,
        blast,
        body,
        tail,
        whip,
        whoosh,
    }
}

/// A gun's shot at a distance: the layers of `gunshot_layers` mixed.
///
/// Near carries everything; mid drops the mechanism, raises the tail six
/// dB and low-passes at 3 kHz; far halves the blast, keeps the tail six dB
/// over the halved blast's own (the yard answers the blast it got, so the
/// tail layer follows the blast gain) and low-passes at 900 Hz, so what
/// is left at range is the body and the yard. Then a soft clip at unit
/// drive, gentle enough to keep the peak where the layers put it, and a
/// normalise to 0.9.
#[must_use]
pub fn gunshot_at(p: &GunParams, dist: Dist) -> Vec<f32> {
    let l = gunshot_layers(p);
    let (mech_g, blast_g, body_g, tail_g, cutoff) = match dist {
        Dist::Near => (0.35, 1.0, 0.7, 1.0, None),
        Dist::Mid => (0.0, 1.0, 0.7, 2.0, Some(3000.0)),
        Dist::Far => (0.0, 0.5, 0.7, 1.0, Some(900.0)),
    };
    let mut out = mix(&[
        (&l.mechanism, mech_g),
        (&l.blast, blast_g),
        (&l.body, body_g),
        (&l.tail, tail_g),
    ]);
    if let Some(whip) = &l.whip {
        place(&mut out, whip, 0.008, 0.7 * blast_g);
    }
    if let Some(whoosh) = &l.whoosh {
        place(&mut out, whoosh, 0.0, 0.3);
    }
    if let Some(c) = cutoff {
        out = lowpass(&out, c);
    }
    normalize(&soft_clip(&out, 1.0), 0.9)
}

// ---------------------------------------------------------------------------
// 6.3 The rest
// ---------------------------------------------------------------------------

/// A cue's own noise seed: its position in `ALL` spread by a constant, so
/// two cues never share a burst and every build gets the same one.
fn seed_of(sfx: Sfx) -> u32 {
    let i = ALL.iter().position(|&s| s == sfx).unwrap_or(0);
    u32::try_from(i).unwrap_or(0).wrapping_mul(0x9E37_79B9) ^ 0x1234_5678
}

/// A mechanical click: `ms` of noise high-passed at `hz`, normalised.
fn click(seed: u32, hz: f32, ms: f32) -> Vec<f32> {
    let dur = ms * 0.001;
    normalize(
        &shaped(
            &highpass(&noise(dur, seed), hz),
            &envelope(0.0, 0.0003, dur - 0.0003, 2.0),
        ),
        1.0,
    )
}

/// A thunk: a low sine at `hz` with a little low-passed noise, `ms` long.
fn thunk(seed: u32, hz: f32, ms: f32) -> Vec<f32> {
    let dur = ms * 0.001;
    let tone = shaped(
        &sweep(hz, hz * 0.8, dur),
        &envelope(0.001, 0.0, dur - 0.001, 2.5),
    );
    let grit = shaped(
        &lowpass(&noise(dur, seed), hz * 3.0),
        &envelope(0.0, 0.0, dur, 3.0),
    );
    normalize(&mix(&[(&tone, 1.0), (&grit, 0.5)]), 1.0)
}

/// A slide: noise in a band around `hz`, `ms` long, with a soft attack.
fn slide(seed: u32, hz: f32, ms: f32) -> Vec<f32> {
    let dur = ms * 0.001;
    normalize(
        &shaped(
            &bandpass(&noise(dur, seed), hz, 1.2),
            &envelope(dur * 0.2, dur * 0.3, dur * 0.5, 1.5),
        ),
        1.0,
    )
}

/// A tink: a short bright sine with an inharmonic partial, `ms` long.
fn tink(hz: f32, ms: f32) -> Vec<f32> {
    let dur = ms * 0.001;
    let env = envelope(0.0002, 0.0, dur - 0.0002, 3.0);
    let a = shaped(&sine(hz, dur), &env);
    let b = shaped(&sine(hz * 1.58, dur), &env);
    normalize(&mix(&[(&a, 1.0), (&b, 0.4)]), 1.0)
}

fn finish(out: &[f32]) -> Vec<f32> {
    normalize(&soft_clip(out, 1.2), 0.9)
}

/// The supersonic crack: a 1.5 ms click, then 20 ms of high-passed noise
/// decaying, the shock cone passing the ear.
fn crack(seed: u32) -> Vec<f32> {
    let mut out = shaped(&noise(0.0015, seed), &envelope(0.0, 0.0005, 0.001, 1.0));
    let cone = shaped(
        &highpass(&noise(0.02, seed ^ 0x1111), 3000.0),
        &envelope(0.0, 0.0, 0.02, 2.0),
    );
    place(&mut out, &cone, 0.0015, 0.8);
    finish(&out)
}

/// Metal: a 4 kHz ring with a second inharmonic partial over 120 ms, plus
/// the click of the strike.
fn impact_metal(seed: u32) -> Vec<f32> {
    let env = envelope(0.0005, 0.0, 0.1195, 3.0);
    let ring = mix(&[
        (&shaped(&sine(4000.0, 0.12), &env), 1.0),
        (&shaped(&sine(6350.0, 0.12), &env), 0.6),
    ]);
    let mut out = ring;
    place(&mut out, &click(seed, 2500.0, 3.0), 0.0, 0.8);
    finish(&out)
}

/// Stone: band-passed noise at 800 Hz, 60 ms, dry.
fn impact_stone(seed: u32) -> Vec<f32> {
    finish(&shaped(
        &bandpass(&noise(0.06, seed), 800.0, 1.2),
        &envelope(0.0005, 0.0, 0.0595, 2.0),
    ))
}

/// Wood: a 400 Hz thump with a 2 kHz crack on top, 80 ms.
fn impact_wood(seed: u32) -> Vec<f32> {
    let mut out = shaped(&sine(400.0, 0.08), &envelope(0.001, 0.0, 0.079, 2.5));
    place(&mut out, &click(seed, 2000.0, 12.0), 0.0, 0.7);
    finish(&out)
}

/// Sand: a low-passed noise thud at 200 Hz, 70 ms.
fn impact_sand(seed: u32) -> Vec<f32> {
    finish(&shaped(
        &lowpass(&noise(0.07, seed), 200.0),
        &envelope(0.002, 0.0, 0.068, 1.5),
    ))
}

/// Body: a wet low thud at 150 Hz, 90 ms.
fn impact_body(seed: u32) -> Vec<f32> {
    let tone = shaped(&sweep(150.0, 90.0, 0.09), &envelope(0.002, 0.0, 0.088, 2.0));
    let wet = shaped(
        &lowpass(&noise(0.04, seed), 400.0),
        &envelope(0.001, 0.0, 0.039, 2.0),
    );
    finish(&mix(&[(&tone, 1.0), (&wet, 0.5)]))
}

/// Ricochet: a whine descending from 3 kHz to 800 Hz over 220 ms, with
/// the click of the strike.
fn ricochet(seed: u32) -> Vec<f32> {
    let mut out = shaped(
        &sweep(3000.0, 800.0, 0.22),
        &envelope(0.005, 0.02, 0.195, 1.5),
    );
    out = mix(&[(&out, 0.8)]);
    place(&mut out, &click(seed, 2500.0, 3.0), 0.0, 1.0);
    finish(&out)
}

/// Casing: a 5 kHz tink with two bounces 90 ms apart, quieter each.
fn casing() -> Vec<f32> {
    let hit = tink(5000.0, 30.0);
    let mut out = Vec::new();
    place(&mut out, &hit, 0.0, 1.0);
    place(&mut out, &hit, 0.09, 0.55);
    place(&mut out, &hit, 0.18, 0.3);
    finish(&out)
}

/// Pistol: mag out click, mag in thunk, slide.
fn reload_pistol(seed: u32) -> Vec<f32> {
    let mut out = Vec::new();
    place(&mut out, &click(seed, 2000.0, 8.0), 0.0, 0.8);
    place(&mut out, &thunk(seed ^ 1, 180.0, 50.0), 0.20, 1.0);
    place(&mut out, &slide(seed ^ 2, 1500.0, 50.0), 0.40, 0.6);
    place(&mut out, &click(seed ^ 3, 1800.0, 6.0), 0.45, 0.9);
    finish(&out)
}

/// Rifle: mag out, mag in, bolt back and forward.
fn reload_rifle(seed: u32) -> Vec<f32> {
    let mut out = Vec::new();
    place(&mut out, &click(seed, 1800.0, 8.0), 0.0, 0.8);
    place(&mut out, &thunk(seed ^ 1, 150.0, 60.0), 0.25, 1.0);
    place(&mut out, &slide(seed ^ 2, 1200.0, 60.0), 0.50, 0.6);
    place(&mut out, &click(seed ^ 3, 1500.0, 7.0), 0.62, 1.0);
    finish(&out)
}

/// Revolver: cylinder out, six rounds dropped as fast clicks, cylinder in.
fn reload_revolver(seed: u32) -> Vec<f32> {
    let mut out = Vec::new();
    place(&mut out, &click(seed, 2000.0, 6.0), 0.0, 0.8);
    place(&mut out, &tink(2500.0, 40.0), 0.0, 0.4);
    for k in 0..6u32 {
        let hz = if k % 2 == 0 { 4800.0 } else { 5200.0 };
        place(&mut out, &tink(hz, 15.0), 0.15 + 0.06 * secs_of(k), 0.6);
    }
    place(&mut out, &thunk(seed ^ 1, 220.0, 40.0), 0.65, 1.0);
    place(&mut out, &click(seed ^ 2, 1800.0, 5.0), 0.66, 0.8);
    finish(&out)
}

// A small count as a time multiplier.
#[allow(clippy::cast_precision_loss)]
const fn secs_of(k: u32) -> f32 {
    k as f32
}

/// Sniper: bolt back, bolt forward, each a slide ending in a click.
fn reload_sniper(seed: u32) -> Vec<f32> {
    let mut out = Vec::new();
    place(&mut out, &click(seed, 1800.0, 6.0), 0.0, 0.9);
    place(&mut out, &slide(seed ^ 1, 1200.0, 80.0), 0.01, 0.6);
    place(&mut out, &slide(seed ^ 2, 1000.0, 80.0), 0.30, 0.6);
    place(&mut out, &thunk(seed ^ 3, 200.0, 40.0), 0.36, 0.8);
    place(&mut out, &click(seed ^ 4, 1600.0, 6.0), 0.36, 1.0);
    finish(&out)
}

/// RPG: a hollow tube slide, then the round seating.
fn reload_rpg(seed: u32) -> Vec<f32> {
    let mut out = shaped(
        &bandpass_sweep(&noise(0.30, seed), 400.0, 700.0, 3.0),
        &envelope(0.03, 0.15, 0.12, 1.5),
    );
    out = normalize(&out, 0.7);
    place(&mut out, &thunk(seed ^ 1, 120.0, 60.0), 0.30, 1.0);
    place(&mut out, &click(seed ^ 2, 1200.0, 6.0), 0.31, 0.6);
    finish(&out)
}

/// The v18 sweep voice, kept bit for bit for the eighteen cues that
/// predate the kit: a pitch sweep with exponential decay whose shape morphs
/// between sine (0) and square (1), with a noise share.
struct V18 {
    rng: u32,
}

impl V18 {
    const fn new() -> Self {
        Self { rng: 0x1234_5678 }
    }

    fn noise(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let upper = u16::try_from(self.rng >> 16).expect("the shifted RNG value fits in u16");
        f32::from(upper) / 32768.0 - 1.0
    }

    fn sweep(
        &mut self,
        dur: f32,
        f0: f32,
        f1: f32,
        square: f32,
        decay: f32,
        noise_amt: f32,
    ) -> Vec<f32> {
        let sr = SAMPLE_RATE_F32;
        let n = sample_count(dur);
        let mut phase = 0.0f32;
        (0..n)
            .map(|i| {
                let sample =
                    u16::try_from(i).expect("sound effects contain fewer than 65k samples");
                let t = f32::from(sample) / sr;
                let f = f0 + (f1 - f0) * (t / dur);
                phase += TAU * f / sr;
                let s = phase.sin();
                let sq = if s >= 0.0 { 1.0 } else { -1.0 };
                let osc = s * (1.0 - square) + sq * square;
                let env = (-t * decay).exp();
                (osc + self.noise() * noise_amt) * env * 0.4
            })
            .collect()
    }
}

/// Mono f32 samples at 44.1 kHz for a cue.
#[allow(clippy::too_many_lines)]
fn synth(sfx: Sfx) -> Vec<f32> {
    let seed = seed_of(sfx);
    let mut v = V18::new();
    match sfx {
        // Laser pew: fast downward square sweep with a noisy attack.
        Sfx::Shot => v.sweep(0.09, 950.0, 160.0, 0.7, 28.0, 0.25),
        // Hitmarker: short bright blip.
        Sfx::Hit => v.sweep(0.05, 1250.0, 1400.0, 0.2, 45.0, 0.0),
        // Taking damage: low thud.
        Sfx::Hurt => v.sweep(0.13, 170.0, 70.0, 0.4, 22.0, 0.5),
        // Frag: quick two-tone rise.
        Sfx::Kill => {
            let mut a = v.sweep(0.07, 520.0, 520.0, 0.3, 18.0, 0.0);
            a.extend(v.sweep(0.11, 780.0, 900.0, 0.3, 16.0, 0.0));
            a
        }
        // Dying: long fall.
        Sfx::Death => v.sweep(0.32, 420.0, 70.0, 0.5, 9.0, 0.15),
        // Respawn: soft rise.
        Sfx::Respawn => v.sweep(0.2, 240.0, 640.0, 0.0, 10.0, 0.0),
        // Weapon upgrade: bright triumphant rise.
        Sfx::Upgrade => {
            let mut a = v.sweep(0.08, 420.0, 420.0, 0.2, 14.0, 0.0);
            a.extend(v.sweep(0.16, 640.0, 1050.0, 0.2, 10.0, 0.0));
            a
        }
        // Reload: two mechanical clicks with a gap.
        Sfx::Reload => {
            let mut a = v.sweep(0.035, 1900.0, 1500.0, 0.85, 70.0, 0.2);
            a.extend(std::iter::repeat_n(0.0, sample_count(0.08)));
            a.extend(v.sweep(0.045, 1300.0, 900.0, 0.85, 60.0, 0.2));
            a
        }
        // The SMG: short and buzzy, decaying fast so a 12.5 Hz burst reads
        // as rounds, not as a tone.
        Sfx::ShotSmg => v.sweep(0.05, 700.0, 300.0, 0.8, 40.0, 0.3),
        // The rifle: a low hammer with a noisy edge.
        Sfx::ShotRifle => v.sweep(0.11, 230.0, 90.0, 0.7, 24.0, 0.45),
        // The revolver: the hammer's click, then a long low report.
        Sfx::ShotRevolver => {
            let mut a = v.sweep(0.012, 2400.0, 1800.0, 1.0, 120.0, 0.0);
            a.extend(v.sweep(0.16, 150.0, 55.0, 0.6, 16.0, 0.5));
            a
        }
        // The sniper: a supersonic crack over a boom.
        Sfx::ShotSniper => {
            let mut a = v.sweep(0.04, 1800.0, 400.0, 0.9, 60.0, 0.6);
            a.extend(v.sweep(0.22, 120.0, 45.0, 0.5, 12.0, 0.35));
            a
        }
        // The rocket leaving the tube: a rising whoosh, mostly noise.
        Sfx::Launch => v.sweep(0.25, 90.0, 400.0, 0.2, 10.0, 0.9),
        // The detonation: sub-bass and noise, with a rumbling tail.
        Sfx::Blast => {
            let mut a = v.sweep(0.45, 70.0, 28.0, 0.5, 7.0, 0.95);
            a.extend(v.sweep(0.30, 40.0, 25.0, 0.2, 8.0, 0.6));
            a
        }
        // The bonk: a rising sine boing, the Mario note.
        Sfx::Bonk => v.sweep(0.09, 260.0, 820.0, 0.0, 22.0, 0.05),
        // The pop: two bright bell tones a fifth apart.
        Sfx::Pop => {
            let mut a = v.sweep(0.06, 1480.0, 1480.0, 0.1, 30.0, 0.0);
            a.extend(v.sweep(0.14, 1975.0, 1975.0, 0.1, 18.0, 0.0));
            a
        }
        // The dry click: hard, short, dull.
        Sfx::Click => v.sweep(0.025, 900.0, 700.0, 1.0, 120.0, 0.3),
        // The holster: a slap down, then the sidearm's rising draw.
        Sfx::Holster => {
            let mut a = v.sweep(0.05, 600.0, 300.0, 0.6, 50.0, 0.4);
            a.extend(v.sweep(0.08, 380.0, 700.0, 0.2, 20.0, 0.0));
            a
        }
        Sfx::ShotSidearmNear | Sfx::ShotSidearmMid | Sfx::ShotSidearmFar => {
            gunshot_at(&GUNS[0], dist_of(sfx))
        }
        Sfx::ShotVityazNear | Sfx::ShotVityazMid | Sfx::ShotVityazFar => {
            gunshot_at(&GUNS[1], dist_of(sfx))
        }
        Sfx::ShotAkNear | Sfx::ShotAkMid | Sfx::ShotAkFar => gunshot_at(&GUNS[2], dist_of(sfx)),
        Sfx::ShotM4Near | Sfx::ShotM4Mid | Sfx::ShotM4Far => gunshot_at(&GUNS[3], dist_of(sfx)),
        Sfx::ShotRevolverNear | Sfx::ShotRevolverMid | Sfx::ShotRevolverFar => {
            gunshot_at(&GUNS[4], dist_of(sfx))
        }
        Sfx::ShotSniperNear | Sfx::ShotSniperMid | Sfx::ShotSniperFar => {
            gunshot_at(&GUNS[5], dist_of(sfx))
        }
        Sfx::ShotRpgNear | Sfx::ShotRpgMid | Sfx::ShotRpgFar => gunshot_at(&GUNS[6], dist_of(sfx)),
        Sfx::Crack => crack(seed),
        Sfx::ImpactMetal => impact_metal(seed),
        Sfx::ImpactStone => impact_stone(seed),
        Sfx::ImpactWood => impact_wood(seed),
        Sfx::ImpactSand => impact_sand(seed),
        Sfx::ImpactBody => impact_body(seed),
        Sfx::Ricochet => ricochet(seed),
        Sfx::Casing => casing(),
        Sfx::ReloadPistol => reload_pistol(seed),
        Sfx::ReloadRifle => reload_rifle(seed),
        Sfx::ReloadRevolver => reload_revolver(seed),
        Sfx::ReloadSniper => reload_sniper(seed),
        Sfx::ReloadRpg => reload_rpg(seed),
    }
}

/// Which distance variant a shot cue is, near for anything that is not a
/// mid or far shot.
const fn dist_of(sfx: Sfx) -> Dist {
    match sfx {
        Sfx::ShotSidearmMid
        | Sfx::ShotVityazMid
        | Sfx::ShotAkMid
        | Sfx::ShotM4Mid
        | Sfx::ShotRevolverMid
        | Sfx::ShotSniperMid
        | Sfx::ShotRpgMid => Dist::Mid,
        Sfx::ShotSidearmFar
        | Sfx::ShotVityazFar
        | Sfx::ShotAkFar
        | Sfx::ShotM4Far
        | Sfx::ShotRevolverFar
        | Sfx::ShotSniperFar
        | Sfx::ShotRpgFar => Dist::Far,
        _ => Dist::Near,
    }
}

// ---------------------------------------------------------------------------
// 6.5 The slot for recorded samples
// ---------------------------------------------------------------------------

/// Recorded samples, one line per WAV in `crates/arena/assets/sfx/`, each
/// `(Sfx::X, include_bytes!("../assets/sfx/<X.file_name()>.wav"))`. A
/// line is what makes the file replace the synth at build time: there is
/// no build script scanning the folder, because `include_bytes!` needs a
/// literal path and a file that exists, and a missing file must fail the
/// build loudly rather than fall back to the synth in silence. The test
/// `the_slot_folder_holds_only_registered_wavs` catches a WAV dropped in
/// without its line. Nothing is shipped in v20.
const RECORDED: &[(Sfx, &[u8])] = &[];

/// Decode a RIFF WAVE file in the slot's format: PCM, 16-bit, mono, 44.1 kHz.
///
/// # Errors
///
/// Anything else is refused with a reason rather than resampled or mixed
/// down, because a sample that reached the tree in the wrong format should
/// be fixed at the source, not silently degraded here; so is a file whose
/// chunks do not add up.
pub fn decode_wav(bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("not a RIFF WAVE file");
    }
    let le16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    let le32 =
        |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    let mut pos = 12;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = usize::try_from(le32(pos + 4)).map_err(|_| "a chunk size overflows")?;
        let body = pos + 8;
        if body + size > bytes.len() {
            return Err("a chunk runs past the end of the file");
        }
        match id {
            b"fmt " => {
                if size < 16 {
                    return Err("the fmt chunk is short");
                }
                fmt = Some((le16(body), le16(body + 2), le32(body + 4), le16(body + 14)));
            }
            b"data" => {
                let (format, channels, rate, bits) =
                    fmt.ok_or("the data chunk comes before the fmt chunk")?;
                if format != 1 {
                    return Err("not PCM");
                }
                if channels != 1 {
                    return Err("not mono");
                }
                if rate != SAMPLE_RATE {
                    return Err("not 44.1 kHz");
                }
                if bits != 16 {
                    return Err("not 16-bit");
                }
                return Ok(bytes[body..body + size]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| f32::from(i16::from_le_bytes(*c)) / 32768.0)
                    .collect());
            }
            _ => {}
        }
        // Chunks are word-aligned; an odd size carries a pad byte.
        pos = body + size + (size & 1);
    }
    Err("no data chunk")
}

/// The samples for a cue from a table of recordings, else the synth. A
/// decoded recording and a synthesised buffer are both owned today; the
/// `Cow` is the shape a static, pre-decoded table would borrow through.
fn source_from(sfx: Sfx, recorded: &[(Sfx, &[u8])]) -> Cow<'static, [f32]> {
    recorded
        .iter()
        .find(|(s, _)| *s == sfx)
        .and_then(|(_, bytes)| decode_wav(bytes).ok())
        .map_or_else(|| Cow::Owned(synth(sfx)), Cow::Owned)
}

/// The samples every player hears for a cue: the recording in the slot
/// when there is one, the synth otherwise.
fn source(sfx: Sfx) -> Cow<'static, [f32]> {
    source_from(sfx, RECORDED)
}

pub use platform::Audio;

#[cfg(not(target_arch = "wasm32"))]
mod platform {
    use super::{ALL, SAMPLE_RATE, Sfx, pan_gains, source};
    use std::collections::HashMap;
    use std::time::Duration;

    pub struct Audio {
        // Field order matters: handle before stream so playback stops
        // cleanly; stream must stay alive for audio to play at all.
        handle: rodio::OutputStreamHandle,
        _stream: rodio::OutputStream,
        samples: HashMap<Sfx, Vec<f32>>,
    }

    impl Audio {
        #[must_use]
        pub fn new() -> Option<Self> {
            let (stream, handle) = rodio::OutputStream::try_default().ok()?;
            let samples = ALL.iter().map(|&s| (s, source(s).into_owned())).collect();
            Some(Self {
                handle,
                _stream: stream,
                samples,
            })
        }

        /// A cue at the centre, now.
        pub fn play(&self, sfx: Sfx, vol: f32) {
            use rodio::Source;
            let Some(data) = self.samples.get(&sfx) else {
                return;
            };
            let buf = rodio::buffer::SamplesBuffer::new(1, SAMPLE_RATE, data.clone());
            let _ = self.handle.play_raw(buf.amplify(vol.clamp(0.0, 1.0)));
        }

        /// A cue panned by `pan` (-1 left .. 1 right) and started
        /// `delay_secs` late: a two-channel buffer built by the pan law,
        /// through rodio's `delay`. The delay is clamped to five seconds
        /// so a bad distance cannot park a cue for a minute.
        #[allow(dead_code)] // Wired by the v20 client (plan section 5).
        pub fn play_spatial(&self, sfx: Sfx, vol: f32, pan: f32, delay_secs: f32) {
            use rodio::Source;
            let Some(data) = self.samples.get(&sfx) else {
                return;
            };
            let (l, r) = pan_gains(pan);
            let stereo: Vec<f32> = data.iter().flat_map(|&s| [s * l, s * r]).collect();
            let delay = if delay_secs.is_finite() {
                delay_secs.clamp(0.0, 5.0)
            } else {
                0.0
            };
            let buf = rodio::buffer::SamplesBuffer::new(2, SAMPLE_RATE, stereo)
                .delay(Duration::from_secs_f32(delay))
                .amplify(vol.clamp(0.0, 1.0));
            let _ = self.handle.play_raw(buf);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod platform {
    use super::{ALL, SAMPLE_RATE, Sfx, source};
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
        /// call stack at least once (Safari and autoplay policies): the
        /// pointerdown listener below guarantees that.
        fn ensure(&self) {
            let mut ctx_slot = self.ctx.borrow_mut();
            if ctx_slot.is_none() {
                *ctx_slot = web_sys::AudioContext::new().ok();
                if let Some(ctx) = ctx_slot.as_ref() {
                    let mut buffers = self.buffers.borrow_mut();
                    for &s in &ALL {
                        let mut data = source(s).into_owned();
                        let Ok(len) = u32::try_from(data.len()) else {
                            continue;
                        };
                        if let Ok(buf) = ctx.create_buffer(1, len, SAMPLE_RATE as f32) {
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
        #[must_use]
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

        /// A cue at the centre, now.
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

        /// A cue panned by `pan` (-1 left .. 1 right) through a
        /// `StereoPannerNode` (the same constant-power law as native) and
        /// started `delay_secs` after the context's clock, clamped to
        /// five seconds.
        #[allow(dead_code)] // Wired by the v20 client (plan section 5).
        pub fn play_spatial(&self, sfx: Sfx, vol: f32, pan: f32, delay_secs: f32) {
            let ctx_slot = self.inner.ctx.borrow();
            let Some(ctx) = ctx_slot.as_ref() else { return };
            let buffers = self.inner.buffers.borrow();
            let Some(buf) = buffers.get(&sfx) else { return };
            let (Ok(src), Ok(panner), Ok(gain)) = (
                ctx.create_buffer_source(),
                ctx.create_stereo_panner(),
                ctx.create_gain(),
            ) else {
                return;
            };
            src.set_buffer(Some(buf));
            panner.pan().set_value(pan.clamp(-1.0, 1.0));
            gain.gain().set_value(vol.clamp(0.0, 1.0));
            drop(src.connect_with_audio_node(&panner));
            drop(panner.connect_with_audio_node(&gain));
            drop(gain.connect_with_audio_node(&ctx.destination()));
            let delay = if delay_secs.is_finite() {
                delay_secs.clamp(0.0, 5.0)
            } else {
                0.0
            };
            drop(src.start_with_when(ctx.current_time() + f64::from(delay)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    /// The buffer memory every player pays for the synthesised set, all
    /// cues at four bytes a sample. The web keeps a second copy inside
    /// the audio context, and native clones one buffer per play.
    fn total_bytes() -> usize {
        ALL.iter().map(|&s| source(s).len() * 4).sum()
    }

    fn zero_crossings_per_second(data: &[f32]) -> f32 {
        let crossings = data
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        secs_of(u32::try_from(crossings).unwrap()) / secs(data.len())
    }

    fn peak_ms(data: &[f32]) -> f32 {
        let (i, _) = data
            .iter()
            .enumerate()
            .fold((0, 0.0f32), |(bi, bv), (i, &v)| {
                if v.abs() > bv { (i, v.abs()) } else { (bi, bv) }
            });
        secs(i) * 1000.0
    }

    fn rms(data: &[f32]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }
        (data.iter().map(|x| x * x).sum::<f32>() / secs_of(u32::try_from(data.len()).unwrap()))
            .sqrt()
    }

    /// Every variant synthesises to a non-empty, finite, bounded buffer,
    /// and the whole set stays under the 24 MB the plan budgets, so a new
    /// cue that runs too long fails here and not on a player's first
    /// shot.
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
        let bytes = total_bytes();
        assert!(bytes < 24 * 1024 * 1024, "{bytes} bytes of cues");
    }

    #[test]
    fn every_kit_stage_is_finite_and_bounded() {
        let n = noise(0.1, 7);
        assert_eq!(n.len(), 4410);
        let stages: Vec<(&str, Vec<f32>)> = vec![
            ("noise", n.clone()),
            ("sine", sine(440.0, 0.1)),
            ("sweep", sweep(2000.0, 200.0, 0.1)),
            ("envelope", envelope(0.01, 0.02, 0.05, 2.0)),
            ("shaped", shaped(&n, &envelope(0.01, 0.0, 0.09, 2.0))),
            ("lowpass", lowpass(&n, 1000.0)),
            ("highpass", highpass(&n, 1000.0)),
            ("bandpass", bandpass(&n, 800.0, 1.0)),
            ("bandpass_sweep", bandpass_sweep(&n, 300.0, 1200.0, 2.0)),
            ("mix", mix(&[(&n, 0.5), (&sine(100.0, 0.05), 0.5)])),
            ("delay_tail", delay_tail(&n, 0.02, 0.6, 2500.0)),
            ("soft_clip", soft_clip(&n, 2.0)),
            ("normalize", normalize(&n, 0.5)),
        ];
        // A filter may ring a little past its input's peak; a stage is
        // bounded when it stays within twice the unit input, and the
        // finished cues are held to one by the sample-limit test.
        for (name, buf) in &stages {
            assert!(!buf.is_empty(), "{name}: empty");
            for v in buf {
                assert!(v.is_finite() && v.abs() <= 2.0, "{name}: sample {v}");
            }
        }
        // The envelope is one exactly through its hold and lands on zero.
        let env = envelope(0.01, 0.02, 0.05, 2.0);
        assert!((env[sample_count(0.01) + 1] - 1.0).abs() < 1e-6);
        assert!(env[env.len() - 1].abs() < 1e-3);
        // The mix runs as long as its longest layer and the tail grows
        // past its input, until the loop is sixty dB down.
        assert_eq!(mix(&[(&n, 1.0), (&sine(100.0, 0.05), 1.0)]).len(), n.len());
        let tail = delay_tail(&n, 0.02, 0.5, 2500.0);
        assert!(tail.len() > n.len());
        assert!(tail.len() <= n.len() + sample_count(TAIL_CAP));
        assert!(rms(&tail[tail.len() - 200..]) < 0.02 * rms(&tail[..2000]));
        // A hard-panned cue is on one side; the centre is equal power.
        let (l, r) = pan_gains(-1.0);
        assert!((l - 1.0).abs() < 1e-6 && r.abs() < 1e-6);
        let (l, r) = pan_gains(0.0);
        assert!((l - r).abs() < 1e-6 && (l * l + r * r - 1.0).abs() < 1e-6);
        let (l, r) = pan_gains(1.0);
        assert!(l.abs() < 1e-6 && (r - 1.0).abs() < 1e-6);
        // Normalize hits the peak it was asked for.
        let peak = normalize(&n, 0.5)
            .iter()
            .fold(0.0f32, |m, &x| m.max(x.abs()));
        assert!((peak - 0.5).abs() < 1e-6);
        // A cue is the same buffer every time: the seed, not the clock.
        assert_eq!(noise(0.01, 99), noise(0.01, 99));
        assert_ne!(noise(0.01, 99), noise(0.01, 100));
    }

    #[test]
    fn lowpass_removes_the_top_octave() {
        // A tone in the top octave through a 4 kHz low-pass loses almost
        // everything; a tone two octaves under the cutoff keeps almost
        // everything; the mirror holds for the high-pass.
        let top = sine(20_000.0, 0.2);
        let low = sine(1000.0, 0.2);
        let settled = |b: &[f32]| rms(&b[b.len() / 2..]);
        let top_through_low = lowpass(&top, 4000.0);
        let low_through_low = lowpass(&low, 4000.0);
        assert!(settled(&top_through_low) < 0.2 * settled(&top));
        assert!(settled(&low_through_low) > 0.8 * settled(&low));
        let top_through_high = highpass(&top, 4000.0);
        let low_through_high = highpass(&low, 4000.0);
        assert!(settled(&top_through_high) > 0.8 * settled(&top));
        assert!(settled(&low_through_high) < 0.2 * settled(&low));
        // The band-pass keeps its centre and drops both far sides.
        let centre = bandpass(&sine(800.0, 0.2), 800.0, 1.0);
        let below = bandpass(&sine(100.0, 0.2), 800.0, 1.0);
        let above = bandpass(&sine(6400.0, 0.2), 800.0, 1.0);
        assert!(settled(&centre) > 0.8 * settled(&low));
        assert!(settled(&below) < 0.2 * settled(&low));
        assert!(settled(&above) < 0.2 * settled(&low));
    }

    #[test]
    fn every_gunshot_has_a_sharper_attack_than_its_tail() {
        for (i, p) in GUNS.iter().enumerate() {
            let weapon = u8::try_from(i + 1).unwrap();
            let near = gunshot(p);
            let mid = gunshot_at(p, Dist::Mid);
            let far = gunshot_at(p, Dist::Far);
            assert_eq!(near, gunshot_at(p, Dist::Near));
            for (name, buf) in [("near", &near), ("mid", &mid), ("far", &far)] {
                let peak = peak_ms(buf);
                assert!(peak < 25.0, "weapon {weapon} {name}: peak at {peak} ms");
                let len = secs(buf.len());
                assert!(
                    len > p.tail_ms * 0.001,
                    "weapon {weapon} {name}: {len} s is shorter than its tail"
                );
            }
            let (zn, zm, zf) = (
                zero_crossings_per_second(&near),
                zero_crossings_per_second(&mid),
                zero_crossings_per_second(&far),
            );
            // The far variant is duller than both others; mid and near
            // share the 2.5 kHz tail that sets most of their crossings,
            // so the plan only pins far against near.
            assert!(zf < zn, "weapon {weapon}: far {zf} crossings/s, near {zn}");
            assert!(zf < zm, "weapon {weapon}: far {zf} crossings/s, mid {zm}");
            // The three variants are what the enum hands out.
            let sfx = |d| Sfx::shot(weapon, d).unwrap();
            assert_eq!(synth(sfx(Dist::Near)), near);
            assert_eq!(synth(sfx(Dist::Mid)), mid);
            assert_eq!(synth(sfx(Dist::Far)), far);
        }
        // Twenty-one distinct cues, and no shot for a non-weapon.
        let mut all: Vec<Sfx> = (1..=7u8)
            .flat_map(|w| [Dist::Near, Dist::Mid, Dist::Far].map(|d| Sfx::shot(w, d).unwrap()))
            .collect();
        all.sort_by_key(|s| ALL.iter().position(|a| a == s));
        all.dedup();
        assert_eq!(all.len(), 21);
        assert!(all.iter().all(|s| ALL.contains(s)));
        assert_eq!(Sfx::shot(0, Dist::Near), None);
        assert_eq!(Sfx::shot(8, Dist::Far), None);
        assert_eq!(Dist::at(0.0), Dist::Near);
        assert_eq!(Dist::at(11.9), Dist::Near);
        assert_eq!(Dist::at(12.0), Dist::Mid);
        assert_eq!(Dist::at(39.9), Dist::Mid);
        assert_eq!(Dist::at(40.0), Dist::Far);
        // The rocket's sustainer is the only accelerating round and its
        // shot is the only one with a whoosh; the sniper alone cracks.
        assert!(GUNS.iter().filter(|g| g.whoosh).count() == 1 && GUNS[6].whoosh);
        assert!(GUNS.iter().filter(|g| g.whipcrack).count() == 1 && GUNS[5].whipcrack);
        assert_eq!(Sfx::reload(1), Sfx::ReloadPistol);
        assert_eq!(Sfx::reload(3), Sfx::ReloadRifle);
        assert_eq!(Sfx::reload(5), Sfx::ReloadRevolver);
        assert_eq!(Sfx::reload(6), Sfx::ReloadSniper);
        assert_eq!(Sfx::reload(7), Sfx::ReloadRpg);
        assert_eq!(Sfx::reload(0), Sfx::Reload);
    }

    #[test]
    fn the_priority_order_keeps_the_v20_cues_where_the_plan_puts_them() {
        // Blast, Death, Kill, Crack, ImpactBody first, then the v18 order,
        // then the shots, and the decoration last.
        let order = [
            Sfx::Blast,
            Sfx::Death,
            Sfx::Kill,
            Sfx::Crack,
            Sfx::ImpactBody,
            Sfx::Pop,
            Sfx::Bonk,
            Sfx::Hurt,
            Sfx::ShotAkNear,
            Sfx::Casing,
        ];
        for pair in order.windows(2) {
            assert!(pair[0].priority() < pair[1].priority(), "{pair:?}");
        }
        for s in [
            Sfx::ImpactMetal,
            Sfx::ImpactStone,
            Sfx::ImpactWood,
            Sfx::ImpactSand,
            Sfx::Ricochet,
        ] {
            assert_eq!(s.priority(), Sfx::Casing.priority(), "{s:?}");
        }
        assert_eq!(Sfx::ShotAkNear.priority(), Sfx::ReloadRifle.priority());
        // A frame of eight remote AK rounds, their eight metal impacts and
        // eight casings, then a crack and a body hit arrive last: the
        // budget keeps the crack and the body hit, then shots, and every
        // impact and casing drops.
        let mut queue: Vec<(Sfx, f32)> = Vec::new();
        for _ in 0..8 {
            queue.push((Sfx::ShotAkMid, 0.4));
            queue.push((Sfx::ImpactMetal, 0.3));
            queue.push((Sfx::Casing, 0.2));
        }
        queue.push((Sfx::Crack, 0.7));
        queue.push((Sfx::ImpactBody, 0.5));
        prioritize(&mut queue);
        let played: Vec<Sfx> = queue.iter().take(BUDGET).map(|(s, _)| *s).collect();
        assert_eq!(played[0], Sfx::Crack);
        assert_eq!(played[1], Sfx::ImpactBody);
        assert!(played[2..].iter().all(|s| *s == Sfx::ShotAkMid));
        // Every cue has a name for the slot and no two share one.
        let mut names: Vec<&str> = ALL.iter().map(|s| s.file_name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }

    /// A little-endian PCM16 mono 44.1 kHz WAV around `samples`, the
    /// format the slot's README names.
    fn encode_wav(samples: &[i16], channels: u16, bits: u16) -> Vec<u8> {
        let data: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let data_len = u32::try_from(data.len()).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        let block = channels * bits / 8;
        out.extend_from_slice(&(SAMPLE_RATE * u32::from(block)).to_le_bytes());
        out.extend_from_slice(&block.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    #[test]
    fn a_wav_in_the_slot_replaces_the_synth() {
        let pcm: Vec<i16> = [0i16, 16384, -16384, 32767, -32768, 1].to_vec();
        let wav = encode_wav(&pcm, 1, 16);
        let recorded: &[(Sfx, &[u8])] = &[(Sfx::ShotAkNear, &wav)];
        let got = source_from(Sfx::ShotAkNear, recorded);
        let want: Vec<f32> = pcm.iter().map(|&s| f32::from(s) / 32768.0).collect();
        assert_eq!(got.as_ref(), want.as_slice());
        assert_ne!(got.as_ref(), synth(Sfx::ShotAkNear).as_slice());
        // A cue without a recording still synthesises, and an empty table
        // is what ships.
        assert_eq!(
            source_from(Sfx::ShotAkFar, recorded).as_ref(),
            synth(Sfx::ShotAkFar).as_slice()
        );
        // Nothing ships in the slot in v20, so every cue is its synth.
        for s in ALL {
            assert_eq!(source(s).as_ref(), synth(s).as_slice(), "{s:?}");
        }
        // The reader refuses what the slot does not take, by name.
        assert_eq!(decode_wav(&encode_wav(&pcm, 2, 16)), Err("not mono"));
        assert_eq!(decode_wav(&encode_wav(&pcm, 1, 8)), Err("not 16-bit"));
        assert_eq!(decode_wav(b"RIFF\0\0\0\0WAVEjunk"), Err("no data chunk"));
        assert_eq!(decode_wav(b"OggS"), Err("not a RIFF WAVE file"));
        let mut truncated = wav.clone();
        truncated.truncate(wav.len() - 4);
        assert_eq!(
            decode_wav(&truncated),
            Err("a chunk runs past the end of the file")
        );
        // A recording that fails to decode falls back to the synth rather
        // than playing silence.
        let bad: &[(Sfx, &[u8])] = &[(Sfx::Crack, b"OggS")];
        assert_eq!(
            source_from(Sfx::Crack, bad).as_ref(),
            synth(Sfx::Crack).as_slice()
        );
    }

    /// A WAV dropped into `assets/sfx/` must carry a cue's name and a line
    /// in `RECORDED`, or it does nothing and nobody notices.
    #[test]
    fn the_slot_folder_holds_only_registered_wavs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/sfx");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wav") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let sfx = ALL.iter().find(|s| s.file_name() == stem);
            assert!(sfx.is_some(), "{} is not a cue name", path.display());
            let bytes = std::fs::read(&path).unwrap();
            let decoded = decode_wav(&bytes);
            assert!(
                decoded.is_ok(),
                "{}: {}",
                path.display(),
                decoded.unwrap_err()
            );
            assert!(
                RECORDED.iter().any(|(s, _)| Some(s) == sfx),
                "{} has no line in RECORDED",
                path.display()
            );
        }
    }

    /// With `EMBER_SFX_PLOT=<dir>` set, write every cue as a CSV of time,
    /// sample and a one-millisecond RMS envelope, plus `summary.csv` with
    /// the numbers the plan asks to look at (the peak's time and the
    /// zero-crossing rate) and the memory total. Otherwise nothing.
    #[test]
    fn plot_every_cue_when_asked() {
        let Ok(dir) = std::env::var("EMBER_SFX_PLOT") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut summary = String::from("name,samples,seconds,bytes,peak_ms,crossings_per_s\n");
        let window = sample_count(0.001);
        for s in ALL {
            let data = source(s);
            let mut csv = String::from("t_ms,sample,env\n");
            for (i, v) in data.iter().enumerate() {
                let lo = i.saturating_sub(window / 2);
                let hi = (i + window / 2).min(data.len());
                let env = rms(&data[lo..hi]);
                writeln!(csv, "{:.4},{v:.5},{env:.5}", secs(i) * 1000.0).unwrap();
            }
            std::fs::write(dir.join(format!("{}.csv", s.file_name())), csv).unwrap();
            writeln!(
                summary,
                "{},{},{:.4},{},{:.2},{:.0}",
                s.file_name(),
                data.len(),
                secs(data.len()),
                data.len() * 4,
                peak_ms(&data),
                zero_crossings_per_second(&data)
            )
            .unwrap();
        }
        writeln!(summary, "total,,,{},,", total_bytes()).unwrap();
        std::fs::write(dir.join("summary.csv"), summary).unwrap();
    }
}
