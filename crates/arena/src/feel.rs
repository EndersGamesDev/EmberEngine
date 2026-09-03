//! The feel pass (arena v18): what a gun does to the camera, the viewmodel,
//! the speakers and the pad, per weapon id, and what the events out of the
//! sim (a hit, a kill, a bonk, a blast) do to the same four channels.
//!
//! Everything here is cosmetic. Nothing in this module ever reaches the
//! wire: the recoil kick moves the camera and the model, never the pitch the
//! client sends, and `online.rs` pins that with `recoil_never_reaches_the_wire`.
//! The numbers are `docs/plans/arena-v18-freight-yard.md` section 6.3; every
//! one ships as written unless a test proves it wrong.

use ember_engine::Rumble;
use ember_engine::glam::Vec3;

use crate::sound::Sfx;

/// The sidearm's glow, the accent every fallback part list inherits.
pub const GLOW_BLUE: Vec3 = Vec3::new(0.20, 0.65, 1.00);

/// How a weapon feels to fire.
///
/// `kick_cam` and `kick_model` are radians of muzzle-up at the recoil peak
/// (camera and viewmodel separately: the model kicks far more than the view,
/// or aiming during a burst is impossible); `push` is metres the model slides
/// back along the look; `rise` is the fraction of the cooldown the kick
/// takes to peak and `settle_pow` shapes the return (1 is linear and never
/// settles between rounds of a full-auto, 3 snaps back); `yaw_alt` is a
/// sideways kick whose sign alternates by shot parity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFeel {
    pub kick_cam: f32,
    pub kick_model: f32,
    pub push: f32,
    pub rise: f32,
    pub settle_pow: f32,
    pub yaw_alt: f32,
    /// The pad's answer to one round.
    pub rumble: Rumble,
    /// Muzzle flash cube edge, and how long it shows.
    pub flash: f32,
    pub flash_ms: f32,
    /// Tracer rod: length, thickness, colour; and the hot head's size.
    pub tracer_len: f32,
    pub tracer_thick: f32,
    pub tracer: Vec3,
    pub head: f32,
    /// The strip colour on the viewmodel and the fallback mesh, so a weapon
    /// whose node is missing still reads as itself.
    pub accent: Vec3,
    pub sound: Sfx,
    pub volume: f32,
    /// Full-autos feed the climb accumulator; the rest do not climb.
    pub full_auto: bool,
    /// Camera shake at the shot (the RPG only).
    pub launch_shake: f32,
    /// The vertical field of view fully aimed down the sights, degrees.
    pub ads_fov: f32,
}

/// The hip-fire vertical field of view, degrees.
pub const HIP_FOV: f32 = 70.0;

const fn rumble(strong: f32, weak: f32, ms: u16) -> Rumble {
    Rumble { strong, weak, ms }
}

/// The feel row for a weapon id. Like `weapon_stats`, a match whose `_` arm
/// is the sidearm, so an id this client does not know feels like the gun it
/// also draws for it.
#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn weapon_feel(id: u8) -> WeaponFeel {
    match id {
        2 => WeaponFeel {
            kick_cam: 0.006,
            kick_model: 0.09,
            push: 0.0,
            rise: 0.15,
            settle_pow: 1.0,
            yaw_alt: 0.004,
            rumble: rumble(0.12, 0.55, 30),
            flash: 0.10,
            flash_ms: 0.035,
            tracer_len: 0.45,
            tracer_thick: 0.06,
            tracer: Vec3::new(1.0, 0.95, 0.70),
            head: 0.16,
            accent: Vec3::new(1.0, 0.90, 0.40),
            sound: Sfx::ShotSmg,
            volume: 0.45,
            full_auto: true,
            launch_shake: 0.0,
            ads_fov: 52.0,
        },
        3 => WeaponFeel {
            kick_cam: 0.028,
            kick_model: 0.24,
            push: 0.0,
            rise: 0.10,
            settle_pow: 1.5,
            yaw_alt: 0.010,
            rumble: rumble(0.55, 0.35, 55),
            flash: 0.18,
            flash_ms: 0.045,
            tracer_len: 0.9,
            tracer_thick: 0.08,
            tracer: Vec3::new(1.0, 0.62, 0.2),
            head: 0.24,
            accent: Vec3::new(1.0, 0.55, 0.15),
            sound: Sfx::ShotRifle,
            volume: 0.5,
            full_auto: true,
            launch_shake: 0.0,
            ads_fov: 48.0,
        },
        4 => WeaponFeel {
            kick_cam: 0.014,
            kick_model: 0.15,
            push: 0.0,
            rise: 0.12,
            settle_pow: 2.0,
            yaw_alt: 0.0,
            rumble: rumble(0.35, 0.40, 40),
            flash: 0.14,
            flash_ms: 0.040,
            tracer_len: 0.7,
            tracer_thick: 0.07,
            tracer: Vec3::new(1.0, 1.0, 1.0),
            head: 0.20,
            accent: Vec3::new(0.90, 0.92, 0.98),
            sound: Sfx::ShotRifle,
            // The M4 shares the rifle's cue at nine tenths of the volume.
            volume: 0.45,
            full_auto: true,
            launch_shake: 0.0,
            ads_fov: 46.0,
        },
        5 => WeaponFeel {
            kick_cam: 0.060,
            kick_model: 0.42,
            push: 0.03,
            rise: 0.08,
            settle_pow: 3.0,
            yaw_alt: 0.0,
            rumble: rumble(0.90, 0.30, 90),
            flash: 0.26,
            flash_ms: 0.050,
            tracer_len: 1.0,
            tracer_thick: 0.11,
            tracer: Vec3::new(1.0, 1.0, 0.95),
            head: 0.28,
            accent: Vec3::new(1.0, 0.25, 0.20),
            sound: Sfx::ShotRevolver,
            volume: 0.55,
            full_auto: false,
            launch_shake: 0.0,
            ads_fov: 50.0,
        },
        6 => WeaponFeel {
            kick_cam: 0.070,
            kick_model: 0.35,
            push: 0.05,
            rise: 0.06,
            settle_pow: 1.5,
            yaw_alt: 0.0,
            rumble: rumble(1.00, 0.20, 110),
            flash: 0.22,
            flash_ms: 0.040,
            tracer_len: 1.6,
            tracer_thick: 0.05,
            tracer: Vec3::new(0.75, 1.0, 1.0),
            head: 0.20,
            accent: Vec3::new(0.40, 0.95, 1.0),
            sound: Sfx::ShotSniper,
            volume: 0.6,
            full_auto: false,
            launch_shake: 0.0,
            ads_fov: 22.0,
        },
        7 => WeaponFeel {
            kick_cam: 0.05,
            kick_model: 0.30,
            push: 0.08,
            rise: 0.10,
            settle_pow: 2.0,
            yaw_alt: 0.0,
            rumble: rumble(1.0, 0.8, 200),
            flash: 0.30,
            flash_ms: 0.060,
            // The rocket is a mesh, not a rod; this is the exhaust behind it.
            tracer_len: 0.6,
            tracer_thick: 0.09,
            tracer: Vec3::new(1.0, 0.50, 0.15),
            head: 0.0,
            accent: Vec3::new(1.0, 0.35, 0.10),
            sound: Sfx::Launch,
            volume: 0.55,
            full_auto: false,
            launch_shake: 0.5,
            ads_fov: 55.0,
        },
        _ => WeaponFeel {
            kick_cam: 0.012,
            kick_model: 0.16,
            push: 0.0,
            rise: 0.16,
            settle_pow: 2.0,
            yaw_alt: 0.0,
            rumble: rumble(0.20, 0.45, 45),
            flash: 0.14,
            flash_ms: 0.045,
            tracer_len: 0.68,
            tracer_thick: 0.075,
            // Today's streak: GLOW_BLUE at 0.55.
            tracer: Vec3::new(0.11, 0.3575, 0.55),
            head: 0.22,
            accent: GLOW_BLUE,
            sound: Sfx::Shot,
            volume: 0.5,
            full_auto: false,
            launch_shake: 0.0,
            ads_fov: 44.0,
        },
    }
}

impl WeaponFeel {
    /// The kick curve at `k`, the fraction of the cooldown elapsed since the
    /// confirmed shot (clamped to `0..=1`): a fast rise to 1 at `rise`, then
    /// a settle shaped by `settle_pow` that reaches exactly 0 at the end of
    /// the cooldown. Today's 0.16/0.84 curve, per weapon now.
    #[must_use]
    pub fn recoil(&self, k: f32) -> f32 {
        let k = k.clamp(0.0, 1.0);
        if k < self.rise {
            k / self.rise
        } else {
            ((1.0 - k) / (1.0 - self.rise)).powf(self.settle_pow)
        }
    }

    /// The vertical field of view at `zoom` (0 hip, 1 fully aimed).
    #[must_use]
    pub fn fov(&self, zoom: f32) -> f32 {
        HIP_FOV + (self.ads_fov - HIP_FOV) * zoom.clamp(0.0, 1.0)
    }
}

/// Which way the sideways kick goes for the `shots`-th confirmed round:
/// alternating, so a burst wanders left-right rather than walking one way.
#[must_use]
pub const fn yaw_side(shots: u32) -> f32 {
    if shots.is_multiple_of(2) { 1.0 } else { -1.0 }
}

/// The full-auto climb: an accumulator every confirmed round of a full-auto
/// adds half its camera kick to, decaying at `exp(-6 dt)`, so the AK climbs
/// under a held trigger and the Vityaz never quite rests.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Climb {
    pub value: f32,
}

impl Climb {
    /// One confirmed round of `feel`; only a full-auto climbs.
    pub const fn shot(&mut self, feel: &WeaponFeel) {
        if feel.full_auto {
            self.value += feel.kick_cam * 0.5;
        }
    }

    pub fn decay(&mut self, dt: f32) {
        self.value *= (-6.0 * dt).exp();
    }
}

/// Camera shake: an amplitude that events raise (never add: `max`, so two
/// blasts on one frame are one shake) and that decays at `exp(-9 dt)`, read
/// each frame as a positional and angular jitter from two fixed sums of
/// sines. Positional and pitch/yaw only: a roll needs a camera up vector,
/// which would be a renderer change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shake {
    pub amp: f32,
}

/// The largest eye displacement a shake may cause, so the eye never crosses
/// the 0.1 near plane into a wall it stands beside.
pub const SHAKE_EYE_CLAMP: f32 = 0.08;

impl Shake {
    pub const fn hit(&mut self, a: f32) {
        self.amp = self.amp.max(a.clamp(0.0, 1.0));
    }

    pub fn decay(&mut self, dt: f32) {
        self.amp *= (-9.0 * dt).exp();
    }

    /// The eye offset and the look-direction offset at time `t`, given the
    /// camera's right vector. Both are zero at zero amplitude.
    #[must_use]
    pub fn offsets(self, t: f32, right: Vec3) -> (Vec3, Vec3) {
        if self.amp <= 1e-4 {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let n1 = (37.1 * t).sin() + 0.5 * (71.3 * t).sin();
        let n2 = (41.7 * t + 1.3).sin() + 0.5 * (67.9 * t).sin();
        let eye = right * (0.03 * n1 * self.amp) + Vec3::Y * (0.02 * n2 * self.amp);
        let eye = if eye.length() > SHAKE_EYE_CLAMP {
            eye.normalize() * SHAKE_EYE_CLAMP
        } else {
            eye
        };
        let look = right * (0.02 * n1 * self.amp) + Vec3::Y * (0.015 * n2 * self.amp);
        (eye, look)
    }
}

/// One event's answer on every channel but the camera: the shake to raise,
/// the rumble to queue, the cue to play. The camera's part (a hitmarker, a
/// dip, a holster drop) is a timer in `online.rs`, because it is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cue {
    pub shake: f32,
    pub rumble: Option<Rumble>,
    pub sfx: Option<(Sfx, f32)>,
}

const fn cue(shake: f32, rumble: Option<Rumble>, sfx: Option<(Sfx, f32)>) -> Cue {
    Cue { shake, rumble, sfx }
}

/// Fire pressed while the magazine is out for a reload: once per press.
/// An empty magazine that is not reloading never exists in play, because
/// the sim starts the reload on the tick the last round leaves, so the
/// click is the one dry pull a player can actually make.
#[must_use]
pub const fn empty_trigger() -> Cue {
    cue(0.0, Some(rumble(0.0, 0.20, 30)), Some((Sfx::Click, 0.35)))
}

#[must_use]
pub const fn reload_start() -> Cue {
    cue(0.0, Some(rumble(0.15, 0.25, 60)), Some((Sfx::Reload, 0.45)))
}

#[must_use]
pub const fn reload_end() -> Cue {
    cue(0.0, Some(rumble(0.25, 0.15, 50)), None)
}

/// A looted gun ran dry and the sidearm came back.
#[must_use]
pub const fn holster() -> Cue {
    cue(0.2, Some(rumble(0.3, 0.3, 80)), Some((Sfx::Holster, 0.5)))
}

/// `S2C::Hit { shooter == me }`; a head hit ticks harder.
#[must_use]
pub const fn hit(head: bool) -> Cue {
    let r = if head {
        rumble(0.0, 0.6, 35)
    } else {
        rumble(0.0, 0.35, 40)
    };
    cue(0.0, Some(r), Some((Sfx::Hit, 0.35)))
}

/// `S2C::Hit { victim == me }`.
#[must_use]
pub const fn hurt() -> Cue {
    cue(0.35, Some(rumble(0.6, 0.2, 120)), Some((Sfx::Hurt, 0.6)))
}

/// `Kill { killer == me }`.
#[must_use]
pub const fn kill() -> Cue {
    cue(0.0, Some(rumble(0.8, 0.8, 180)), Some((Sfx::Kill, 0.5)))
}

/// `Kill { victim == me }`, a self-kill included.
#[must_use]
pub const fn death() -> Cue {
    cue(1.0, Some(rumble(1.0, 0.6, 400)), Some((Sfx::Death, 0.55)))
}

/// A predicted bonk on an armed block.
#[must_use]
pub const fn bonk() -> Cue {
    cue(0.4, Some(rumble(0.5, 1.0, 90)), Some((Sfx::Bonk, 0.5)))
}

/// A predicted bonk on a dead block: a dull click and nothing else, so
/// "nothing happened" is felt too.
#[must_use]
pub const fn bonk_dead() -> Cue {
    cue(0.0, None, Some((Sfx::Click, 0.3)))
}

/// `S2C::Loot`: mine, or someone else's within earshot.
#[must_use]
pub const fn pop(mine: bool) -> Cue {
    if mine {
        cue(0.0, Some(rumble(0.2, 0.6, 120)), Some((Sfx::Pop, 0.55)))
    } else {
        cue(0.0, None, Some((Sfx::Pop, 0.25)))
    }
}

/// How far a pop is still heard when it is not mine, metres.
pub const POP_EARSHOT: f32 = 20.0;

/// `S2C::Blast` at distance `d` from my eye: everything falls off with
/// distance except that a far blast still ticks the pad a little.
#[must_use]
pub fn blast(d: f32) -> Cue {
    let near = (1.0 - d / 14.0).clamp(0.0, 1.0);
    let r = if d < 14.0 {
        // Truncation is the intent: a whole number of milliseconds.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let ms = (350.0 * near) as u16;
        rumble(1.0, 1.0, ms)
    } else {
        rumble(0.3, 0.2, 150)
    };
    let vol = (1.0 - d / 40.0).clamp(0.15, 0.9);
    cue(near, Some(r), Some((Sfx::Blast, vol)))
}

#[cfg(test)]
mod feel_tests {
    use super::*;
    use crate::sound::{BUDGET, prioritize};
    use arena_core::shooter::WEAPON_COUNT;

    #[test]
    fn weapon_feel_table_covers_every_weapon_id() {
        let sidearm = weapon_feel(1);
        for id in 1..=WEAPON_COUNT {
            let f = weapon_feel(id);
            assert!(f.kick_cam > 0.0 && f.kick_model > 0.0, "id {id}: no kick");
            assert!(f.rise > 0.0 && f.rise < 1.0, "id {id}: rise {}", f.rise);
            assert!(f.settle_pow >= 1.0, "id {id}: settle {}", f.settle_pow);
            assert!(f.rumble.ms > 0, "id {id}: silent pad");
            assert!(f.flash > 0.0 && f.flash_ms > 0.0, "id {id}: no flash");
            assert!(
                f.tracer_len > 0.0 && f.tracer_thick > 0.0,
                "id {id}: no tracer"
            );
            assert!(
                f.ads_fov > 0.0 && f.ads_fov < HIP_FOV,
                "id {id}: fov {}",
                f.ads_fov
            );
            assert!(
                f.volume > 0.0 && f.volume <= 1.0,
                "id {id}: volume {}",
                f.volume
            );
            if id != 1 {
                assert_ne!(f, sidearm, "id {id} is the sidearm's row");
            }
        }
        // Ids off the table read as the sidearm, like `weapon_stats`.
        assert_eq!(weapon_feel(0), sidearm);
        assert_eq!(weapon_feel(200), sidearm);
        // The full-autos are exactly the three that climb.
        let autos: Vec<u8> = (1..=WEAPON_COUNT)
            .filter(|&id| weapon_feel(id).full_auto)
            .collect();
        assert_eq!(autos, vec![2, 3, 4]);
    }

    #[test]
    fn recoil_curve_peaks_at_the_rise_and_settles_to_zero() {
        for id in 1..=WEAPON_COUNT {
            let f = weapon_feel(id);
            assert!((f.recoil(f.rise) - 1.0).abs() < 1e-6, "id {id}: peak");
            assert!(f.recoil(1.0).abs() < 1e-6, "id {id}: end");
            assert!(f.recoil(0.0).abs() < 1e-6, "id {id}: start");
            // Rising before the peak, monotone falling after it.
            let mut prev = f.recoil(f.rise);
            let mut k = f.rise;
            while k < 1.0 {
                k += 0.01;
                let r = f.recoil(k);
                assert!(r <= prev + 1e-6, "id {id}: rose at k={k}: {r} > {prev}");
                assert!((0.0..=1.0).contains(&r), "id {id}: out of range at {k}");
                prev = r;
            }
            assert!(f.recoil(f.rise * 0.5) < 1.0 && f.recoil(f.rise * 0.5) > 0.0);
            // Past the cooldown the curve is clamped, never negative.
            assert_eq!(f.recoil(1.5), 0.0);
        }
        // The plan's shapes: the Vityaz settles linearly, the revolver snaps.
        let smg = weapon_feel(2);
        let mid = smg.rise + (1.0 - smg.rise) * 0.5;
        assert!((smg.recoil(mid) - 0.5).abs() < 1e-5, "linear settle");
        let rev = weapon_feel(5);
        let mid = rev.rise + (1.0 - rev.rise) * 0.5;
        assert!((rev.recoil(mid) - 0.125).abs() < 1e-5, "cubic snap");
    }

    #[test]
    fn shake_decays_to_under_one_percent_within_half_a_second() {
        let mut s = Shake::default();
        s.hit(1.0);
        s.hit(0.3);
        assert_eq!(s.amp, 1.0, "max, not sum");
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            s.decay(dt);
        }
        // exp(-4.5) = 0.0111 at exactly half a second; the frame after is
        // under one percent, which is what the ear and the eye call gone.
        assert!(s.amp < 0.0112, "after 0.5 s: {}", s.amp);
        s.decay(dt);
        assert!(s.amp < 0.01, "after 0.517 s: {}", s.amp);
        // The eye never leaves the clamp, whatever the time.
        let mut big = Shake::default();
        big.hit(1.0);
        let mut t = 0.0;
        while t < 2.0 {
            let (eye, _) = big.offsets(t, Vec3::X);
            assert!(
                eye.length() <= SHAKE_EYE_CLAMP + 1e-6,
                "eye off {eye} at {t}"
            );
            t += 0.003;
        }
        assert_eq!(
            Shake::default().offsets(0.7, Vec3::X),
            (Vec3::ZERO, Vec3::ZERO)
        );
    }

    #[test]
    fn yaw_kick_alternates_sides() {
        for n in 0..20u32 {
            assert_eq!(yaw_side(n), -yaw_side(n + 1), "shot {n}");
            assert_eq!(yaw_side(n).abs(), 1.0);
        }
        // Only the full-autos have a sideways kick to alternate.
        assert!(weapon_feel(2).yaw_alt > 0.0 && weapon_feel(3).yaw_alt > 0.0);
        assert_eq!(weapon_feel(5).yaw_alt, 0.0);
    }

    #[test]
    fn the_climb_rises_per_full_auto_round_and_decays() {
        let mut c = Climb::default();
        c.shot(&weapon_feel(5));
        assert_eq!(c.value, 0.0, "a revolver does not climb");
        for _ in 0..10 {
            c.shot(&weapon_feel(3));
        }
        assert!((c.value - 10.0 * 0.028 * 0.5).abs() < 1e-6);
        let before = c.value;
        c.decay(0.5);
        assert!(c.value < before * 0.06, "exp(-3) after half a second");
    }

    #[test]
    fn sfx_priority_keeps_the_boom_under_the_budget() {
        // A crowded frame: eight footfalls of remote fire, then the blast
        // arrives last. Without the sort the blast is the cue that drops.
        let mut queue: Vec<(Sfx, f32)> = (0..8).map(|_| (Sfx::Shot, 0.3)).collect();
        queue.push((Sfx::Hit, 0.35));
        queue.push((Sfx::Blast, 0.9));
        prioritize(&mut queue);
        let played: Vec<Sfx> = queue.iter().take(BUDGET).map(|(s, _)| *s).collect();
        assert_eq!(played[0], Sfx::Blast);
        assert_eq!(
            played[1],
            Sfx::Hit,
            "the hitmarker plays before the remote footfalls"
        );
        assert!(
            played.contains(&Sfx::Shot),
            "the budget still plays the shots"
        );
        assert_eq!(played.len(), BUDGET);
        // The order the plan names, then being hurt and hitting, then the
        // rest; the sort is stable within a rank.
        let order = [
            Sfx::Blast,
            Sfx::Death,
            Sfx::Kill,
            Sfx::Pop,
            Sfx::Bonk,
            Sfx::Hurt,
            Sfx::Shot,
        ];
        for pair in order.windows(2) {
            assert!(pair[0].priority() < pair[1].priority(), "{pair:?}");
        }
        assert_eq!(Sfx::Hurt.priority(), Sfx::Hit.priority());
        // Eight footfalls of remote fire, then the hurt thud arrives last:
        // the thud is the cue the budget keeps, and the footfalls keep
        // their own order behind it.
        let mut crowded: Vec<(Sfx, f32)> = (0..8u8)
            .map(|i| (Sfx::ShotSmg, 0.1 * f32::from(i)))
            .collect();
        crowded.push((Sfx::Hurt, 0.6));
        prioritize(&mut crowded);
        assert_eq!(crowded[0].0, Sfx::Hurt);
        assert!((crowded[1].1 - 0.0).abs() < 1e-6 && (crowded[8].1 - 0.7).abs() < 1e-6);
        let mut same = vec![(Sfx::Reload, 0.1), (Sfx::Shot, 0.2), (Sfx::Upgrade, 0.3)];
        prioritize(&mut same);
        assert_eq!(same[0].0, Sfx::Reload);
        assert_eq!(same[2].0, Sfx::Upgrade);
    }

    #[test]
    fn the_blast_cue_falls_off_with_distance() {
        let near = blast(0.0);
        let mid = blast(7.0);
        let far = blast(30.0);
        assert_eq!(near.shake, 1.0);
        assert!((mid.shake - 0.5).abs() < 1e-6);
        assert_eq!(far.shake, 0.0);
        assert_eq!(near.rumble.unwrap().ms, 350);
        assert_eq!(far.rumble.unwrap(), rumble(0.3, 0.2, 150));
        let vol = |c: Cue| c.sfx.unwrap().1;
        assert!(vol(near) > vol(mid) && vol(mid) > vol(far));
        assert!(vol(blast(100.0)) >= 0.15, "a far blast is still heard");
        assert!(pop(true).rumble.is_some());
        assert_eq!(pop(false).rumble, None);
    }
}
