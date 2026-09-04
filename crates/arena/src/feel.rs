//! The feel pass (arena v18): what a gun does to the camera, the viewmodel,
//! the speakers and the pad, per weapon id, and what the events out of the
//! sim (a hit, a kill, a bonk, a blast) do to the same four channels.
//!
//! Everything here is cosmetic. Nothing in this module ever reaches the
//! wire: the recoil kick moves the camera and the model, never the pitch the
//! client sends, and `online.rs` pins that with `recoil_never_reaches_the_wire`.
//! The numbers are `docs/plans/arena-v18-freight-yard.md` section 6.3; every
//! one ships as written unless a test proves it wrong.

use std::collections::VecDeque;

use arena_core::proto::color_for;
use arena_core::shooter::{
    Cover, HILL_CONTESTED, HILL_FREE, Hill, Projectile, SHOT_COVER, SHOT_FLOOR, SHOT_WALL, hash64,
    stance_speed, weapon_stats,
};
use ember_engine::Rumble;
use ember_engine::glam::{Quat, Vec2, Vec3};

use crate::rounds;
use crate::sound::{Dist, SPEED_OF_SOUND, Sfx};

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
    /// Muzzle flash size (the star's petal length; `online::push_flash`
    /// sizes the star from it), and how long it shows.
    pub flash: f32,
    pub flash_ms: f32,
    /// The streak's colour; and the exhaust rod behind a rocket in flight
    /// (length, thickness), the one round still drawn from the state.
    pub tracer_len: f32,
    pub tracer_thick: f32,
    pub tracer: Vec3,
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
            accent: Vec3::new(1.0, 0.90, 0.40),
            sound: Sfx::ShotVityazNear,
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
            accent: Vec3::new(1.0, 0.55, 0.15),
            sound: Sfx::ShotAkNear,
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
            accent: Vec3::new(0.90, 0.92, 0.98),
            sound: Sfx::ShotM4Near,
            // The M4 at nine tenths of the AK's volume.
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
            accent: Vec3::new(1.0, 0.25, 0.20),
            sound: Sfx::ShotRevolverNear,
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
            accent: Vec3::new(0.40, 0.95, 1.0),
            sound: Sfx::ShotSniperNear,
            volume: 0.6,
            full_auto: false,
            launch_shake: 0.0,
            // A 20x scope: the hip field over twenty. The view through it
            // is drawn by `scope_mask`, not by the viewmodel.
            ads_fov: 3.5,
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
            accent: Vec3::new(1.0, 0.35, 0.10),
            sound: Sfx::ShotRpgNear,
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
            accent: GLOW_BLUE,
            sound: Sfx::ShotSidearmNear,
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

/// The least the look may slow to, as a fraction of the hip sensitivity,
/// so a field of view that reads as zero can never freeze the look.
pub const LOOK_SCALE_FLOOR: f32 = 0.03;

/// How much slower the look turns at a vertical field of view, as a
/// fraction of the hip sensitivity. The look slows in the same ratio the
/// view narrows, so a target crossing the screen costs the same mouse
/// travel at every zoom and a 20x scope turns twenty times slower. The pad's
/// right stick reads the same scale.
#[must_use]
pub fn look_scale(fov_deg: f32) -> f32 {
    (fov_deg / HIP_FOV).max(LOOK_SCALE_FLOOR)
}

/// The weapon that is looked through rather than along: the sniper.
pub const SCOPED_WEAPON: u8 = 6;

/// The eased zoom above which the scope view replaces the viewmodel, so
/// the eye sees a brief narrowing before the tube closes round it.
pub const SCOPE_ZOOM: f32 = 0.6;

/// How far in front of the eye the mask and the reticle sit, metres. Three
/// times the 0.1 near plane, so a shake or a dip never clips them, and
/// well inside the 0.6 m the held gun stands at.
pub const SCOPE_DIST: f32 = 0.30;

/// How many slabs close the tube: a 24-gon reads as a circle.
pub const SCOPE_SIDES: usize = 24;

/// The hole's apothem as a fraction of the visible half-height at
/// `SCOPE_DIST`, so the circle nearly fills the shorter screen axis.
pub const SCOPE_APOTHEM: f32 = 0.92;

/// The mask's colour: opaque near-black, since the scene pass has no blend.
pub const SCOPE_BLACK: Vec3 = Vec3::splat(0.02);

/// Whether the scope view is on: the sniper, mostly zoomed.
#[must_use]
pub fn scoped(weapon: u8, zoom: f32) -> bool {
    weapon == SCOPED_WEAPON && zoom > SCOPE_ZOOM
}

/// The visible half-height of the view at `SCOPE_DIST` for a vertical
/// field of view, metres. Everything in the mask is a multiple of it.
#[must_use]
pub fn scope_half_height(fov_deg: f32) -> f32 {
    SCOPE_DIST * (fov_deg.to_radians() * 0.5).tan()
}

/// One opaque slab of the scope mask, in the plane `SCOPE_DIST` in front of
/// the eye: `center` and the unit `tangent`/`normal` pair are in metres in
/// that plane (x along the camera's right, y along its up), the half sizes
/// along them. The outward normal points away from the hole.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slab {
    pub center: Vec2,
    pub tangent: Vec2,
    pub normal: Vec2,
    pub half_len: f32,
    pub half_thick: f32,
}

impl Slab {
    /// Whether a point of the plane lies on this slab. The renderer never
    /// asks (it draws the slab and the depth buffer answers); the coverage
    /// test does, so it exists only there.
    #[cfg(test)]
    #[must_use]
    pub fn contains(&self, p: Vec2) -> bool {
        let d = p - self.center;
        d.dot(self.tangent).abs() <= self.half_len && d.dot(self.normal).abs() <= self.half_thick
    }
}

/// The scope mask for a vertical field of view: the hole's apothem `a` and
/// the slabs that black out everything round it. Slab `k` is a tangent to
/// the regular 24-gon of apothem `a` at angle `k * 2 pi / 24`, eight
/// half-heights long and six thick, standing from the polygon's edge
/// outward. Neighbouring slabs overlap, and their reach (6.92 half-heights
/// from the centre) is past the corner of a 21:9 view (2.54), so the mask
/// is closed at every aspect ratio a screen has; `scope_mask_covers_a_21_9_view`
/// pins it. Every point outside the polygon is on the slab whose angle is
/// nearest, because that slab's inward edge is the polygon's own edge.
#[must_use]
pub fn scope_mask(fov_deg: f32) -> (f32, [Slab; SCOPE_SIDES]) {
    let h = scope_half_height(fov_deg);
    let a = SCOPE_APOTHEM * h;
    let slabs = std::array::from_fn(|k| {
        // Truncation-free: k is at most 23.
        #[allow(clippy::cast_precision_loss)]
        let theta = k as f32 * std::f32::consts::TAU / SCOPE_SIDES as f32;
        let (s, c) = theta.sin_cos();
        let normal = Vec2::new(c, s);
        let tangent = Vec2::new(-s, c);
        Slab {
            center: normal * (a + 3.0 * h),
            tangent,
            normal,
            half_len: 4.0 * h,
            half_thick: 3.0 * h,
        }
    });
    (a, slabs)
}

/// The reticle's two bars for a hole of apothem `a`: full sizes along the
/// camera's right and up, and their depth along the look. Crossing at the
/// centre, spanning the hole, a fiftieth of it thick.
#[must_use]
pub fn scope_reticle(a: f32) -> [Vec3; 2] {
    let thick = 0.02 * a;
    [
        Vec3::new(2.0 * a, thick, 0.001),
        Vec3::new(thick, 2.0 * a, 0.001),
    ]
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

/// The reload's cue is the gun's own (v20): a magazine and a slide for the
/// sidearm, a magazine and a bolt for the three rifles, the cylinder and
/// six rounds for the revolver, the bolt for the sniper, the tube for the
/// launcher. An id off the table reloads like the sidearm it draws as.
#[must_use]
pub const fn reload_start(weapon: u8) -> Cue {
    cue(
        0.0,
        Some(rumble(0.15, 0.25, 60)),
        Some((Sfx::reload(weapon), 0.45)),
    )
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

// ---- v19: modes ----

/// `S2C::RoundOver`: the frag jingle when I or my team won, the death fall
/// otherwise, and one long even rumble either way so the pad marks the
/// end of the round whichever side of it the player was on.
#[must_use]
pub const fn round_over(won: bool) -> Cue {
    let s = if won {
        (Sfx::Kill, 0.55)
    } else {
        (Sfx::Death, 0.5)
    };
    cue(0.0, Some(rumble(0.6, 0.6, 250)), Some(s))
}

/// A team's colour in team deathmatch: blue for team 0, red for team 1,
/// the palette's first two (`color_for`), so a two-player free for all and
/// a team game agree on what blue and red look like. Any other team index
/// reads as red, because the sim only ever assigns 0 or 1 and a value off
/// that range is a bug best seen, not hidden.
#[must_use]
pub const fn team_color(team: u8) -> Vec3 {
    let c = color_for(if team == 0 { 0 } else { 1 });
    Vec3::new(c[0], c[1], c[2])
}

/// The team's name for the status line and the scoreboard header.
#[must_use]
pub const fn team_name(team: u8) -> &'static str {
    if team == 0 { "BLUE" } else { "RED" }
}

/// The hill's edge bars: thin along the footprint's edges, low, lifted a
/// hair above the box top so they never z-fight with the dock or the
/// plinth they sit on.
pub const HILL_BAR_THICK: f32 = 0.06;
pub const HILL_BAR_TALL: f32 = 0.12;
pub const HILL_BAR_LIFT: f32 = 0.06;
/// The marker cube: high enough over the hill to be seen across the map,
/// small enough not to read as cover.
pub const HILL_MARKER_RISE: f32 = 3.0;
pub const HILL_MARKER_EDGE: f32 = 0.3;
/// Nobody on the hill.
pub const HILL_FREE_COLOR: Vec3 = Vec3::ONE;
/// Two or more on it: orange, pulsing at `HILL_PULSE_HZ` so a contested
/// hill is seen to be contested from across the map without a text line.
pub const HILL_CONTESTED_COLOR: Vec3 = Vec3::new(1.0, 0.55, 0.15);
pub const HILL_PULSE_HZ: f32 = 4.0;

/// The colour the hill's bars and marker take for `holder` (a `State.hill`
/// value) at time `t`: white free, the holder's own colour (`holder_color`,
/// which the caller resolves through the same rule that colours bodies, so
/// the king's hill matches the king) when held, orange pulsing between
/// just over half and full brightness when contested. A held hill whose
/// holder this client cannot colour (a state that named a player it has
/// not met) reads as free rather than as a wrong player's.
#[must_use]
pub fn hill_color(holder: u8, holder_color: Option<Vec3>, t: f32) -> Vec3 {
    match holder {
        HILL_FREE => HILL_FREE_COLOR,
        HILL_CONTESTED => {
            let pulse = 0.5 + 0.5 * (std::f32::consts::TAU * HILL_PULSE_HZ * t).sin();
            HILL_CONTESTED_COLOR * (0.55 + 0.45 * pulse)
        }
        _ => holder_color.unwrap_or(HILL_FREE_COLOR),
    }
}

/// The four bars along the hill's edges as (centre, full size) boxes, at
/// `top + HILL_BAR_LIFT`: two along x at the z edges, two along z at the x
/// edges, each the footprint's full length so the corners meet.
#[must_use]
pub fn hill_bars(hill: &Hill) -> [(Vec3, Vec3); 4] {
    let cx = f32::midpoint(hill.min[0], hill.max[0]);
    let cz = f32::midpoint(hill.min[1], hill.max[1]);
    let y = hill.top + HILL_BAR_LIFT;
    let len_x = hill.max[0] - hill.min[0];
    let len_z = hill.max[1] - hill.min[1];
    let along_x = Vec3::new(len_x, HILL_BAR_TALL, HILL_BAR_THICK);
    let along_z = Vec3::new(HILL_BAR_THICK, HILL_BAR_TALL, len_z);
    [
        (Vec3::new(cx, y, hill.min[1]), along_x),
        (Vec3::new(cx, y, hill.max[1]), along_x),
        (Vec3::new(hill.min[0], y, cz), along_z),
        (Vec3::new(hill.max[0], y, cz), along_z),
    ]
}

/// The marker cube over the hill's centre: (centre, full size).
#[must_use]
pub fn hill_marker(hill: &Hill) -> (Vec3, Vec3) {
    (
        Vec3::new(
            f32::midpoint(hill.min[0], hill.max[0]),
            hill.top + HILL_MARKER_RISE,
            f32::midpoint(hill.min[1], hill.max[1]),
        ),
        Vec3::splat(HILL_MARKER_EDGE),
    )
}

// ---- v20: tracers, impacts, marks and spatial cues ----
//
// Everything below is `docs/plans/arena-v20-realism.md` section 5, as pure
// functions: the frame in `online.rs` turns the answers into instances and
// `Audio` calls, and the tests here pin the numbers without a frame.

/// A round's streak, from one `S2C::Shot`: the segment it flew and when it
/// was seen. The head is replayed along the segment at the weapon's own
/// speed from `born`, so a streak reads as something that flew rather than
/// as a line that appeared, and the whole thing outlives the flight by
/// `TRACER_LINGER` while its rods thin out. What the frame draws at the
/// head is the round itself (`rounds::round_for`), and each rod is a
/// tapered streak behind it; the rods here are the where and the how long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tracer {
    /// Where the sim launched the round. The head is replayed from here,
    /// so the round's own body is always on the server's segment.
    pub from: Vec3,
    /// Where the streak is drawn from: the muzzle of the gun the client
    /// draws, which for a remote shooter is about 0.6 m off `from` (the sim
    /// fires from eye height, the drawn weapon is at the hand). Only the
    /// streak reads it — the rods are laid back from the head toward this
    /// point — so a shot leaves the gun that is on screen and rejoins the
    /// server's line within a metre or two of flight. `from` itself
    /// whenever there is no drawn muzzle to start from.
    pub muzzle: Vec3,
    pub to: Vec3,
    pub weapon: u8,
    pub born: f32,
}

/// How far behind the head the core reaches, metres.
pub const TRACER_CORE_LEN: f32 = 2.5;
/// How far behind the head the tail reaches, metres.
pub const TRACER_TAIL_LEN: f32 = 8.0;
/// Seconds the streak stays after the head reaches the end, thinning.
pub const TRACER_LINGER: f32 = 0.12;
/// The tail's brightness against the core's.
pub const TRACER_TAIL_DIM: f32 = 0.45;

/// One opaque rod of a tracer: where its centre is, how long it is along
/// the flight direction, what colour. How thick is not the rod's to say:
/// the frame draws it as a streak (`rounds`) whose base radius is the
/// round's drawn heel times `rounds::STREAK_LEAD` times `Tracer::fade`,
/// a frustum for the core with a cone behind it, or a cone alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rod {
    pub center: Vec3,
    pub len: f32,
    pub color: Vec3,
}

impl Tracer {
    /// The segment's length.
    #[must_use]
    pub fn len(&self) -> f32 {
        (self.to - self.from).length()
    }

    /// The unit flight direction; +X for a segment too short to have one.
    #[must_use]
    pub fn dir(&self) -> Vec3 {
        let d = (self.to - self.from).normalize_or_zero();
        if d == Vec3::ZERO { Vec3::X } else { d }
    }

    /// The speed the head is replayed at: the weapon's top speed, which for
    /// a bullet is its muzzle velocity.
    #[must_use]
    pub const fn speed(&self) -> f32 {
        let top = weapon_stats(self.weapon).speed_max;
        if top < 1.0 { 1.0 } else { top }
    }

    /// How far along the segment the head is at `now`, metres, clamped to
    /// the segment.
    #[must_use]
    pub fn progress(&self, now: f32) -> f32 {
        (self.speed() * (now - self.born)).clamp(0.0, self.len())
    }

    /// Where the head is at `now`.
    #[must_use]
    pub fn head(&self, now: f32) -> Vec3 {
        self.from + self.dir() * self.progress(now)
    }

    /// Seconds from `born` to gone: the flight plus the linger.
    #[must_use]
    pub fn life(&self) -> f32 {
        self.len() / self.speed() + TRACER_LINGER
    }

    #[must_use]
    pub fn alive(&self, now: f32) -> bool {
        now - self.born < self.life()
    }

    /// Whether the round is still flying at `now`: the head has not
    /// reached the end of the segment. The frame draws the round itself
    /// only then; through the linger only the streak remains.
    #[must_use]
    pub fn flying(&self, now: f32) -> bool {
        self.alive(now) && self.progress(now) < self.len()
    }

    /// How much of the streak is left at `now`: 1 through the flight,
    /// falling to 0 over the last `TRACER_LINGER` seconds of the life.
    #[must_use]
    pub fn fade(&self, now: f32) -> f32 {
        let remaining = self.life() - (now - self.born);
        (remaining / TRACER_LINGER).clamp(0.0, 1.0)
    }

    /// Where the streak runs and how long it is: back from the head toward
    /// the `muzzle` the client drew, and the distance between the two. That
    /// is the flight's own direction and the distance flown whenever the
    /// muzzle is the sim's origin; when it is a remote shooter's drawn gun
    /// it starts the streak on that gun and swings onto the flight line as
    /// the round pulls away, since a fixed 0.6 m offset is a degree at
    /// 30 m. `dir` and a zero reach for a segment with no length.
    fn streak(&self, now: f32) -> (Vec3, f32) {
        let back = self.head(now) - self.muzzle;
        let reach = back.length();
        if reach > 1e-4 {
            (back / reach, reach)
        } else {
            (self.dir(), 0.0)
        }
    }

    /// The direction the frame draws the streak's cones along: back toward
    /// the drawn muzzle, which the rods are laid out on.
    #[must_use]
    pub fn streak_dir(&self, now: f32) -> Vec3 {
        self.streak(now).0
    }

    /// The rods to draw at `now`: the core from the head back, then the
    /// tail behind the core (never overlapping it: one opaque shape inside
    /// another is invisible, so the tail starts where the core ends). The
    /// frame thins both by `fade` over the last `TRACER_LINGER` seconds.
    /// Nothing before the head has left the sim's launch point or after the
    /// streak is gone.
    #[must_use]
    pub fn rods(&self, now: f32) -> Vec<Rod> {
        let mut rods = Vec::with_capacity(2);
        if !self.alive(now) {
            return rods;
        }
        let progress = self.progress(now);
        if progress <= 1e-3 {
            return rods;
        }
        let head = self.head(now);
        let (dir, reach) = self.streak(now);
        if reach <= 1e-3 {
            return rods;
        }
        let color = weapon_feel(self.weapon).tracer;
        let core = reach.min(TRACER_CORE_LEN);
        rods.push(Rod {
            center: head - dir * (core * 0.5),
            len: core,
            color,
        });
        let tail = reach.min(TRACER_TAIL_LEN) - core;
        if tail > 1e-3 {
            rods.push(Rod {
                center: head - dir * (core + tail * 0.5),
                len: tail,
                color: color * TRACER_TAIL_DIM,
            });
        }
        rods
    }
}

/// One short-lived opaque particle: a spark, a splinter, a puff of dust
/// or smoke. The frame owns the integration; this is the spawn.
///
/// It was a cube of edge `size` and is now a ball of radius
/// `size * PUFF_BALL` (`rounds::puff`): the same bulk with no corners.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ttl: f32,
    pub size: f32,
    pub color: Vec3,
    /// Downward acceleration; zero for anything that drifts.
    pub gravity: f32,
    /// Seconds before it is born: it neither moves, nor ages, nor is
    /// drawn until this has run out. Zero for every burst; the muzzle
    /// plume alone uses it, to let the flash have the first frames of a
    /// shot to itself (`plume_delay_of`).
    pub delay: f32,
}

/// A puff's drawn radius as a fraction of its `size`: the ball inscribed
/// in the cube it replaced, so nothing grew when the shape changed.
pub const PUFF_BALL: f32 = 0.5;

/// How a particle is drawn `k` of the way through its life, where `k` is
/// the share of its life still to run (1 at birth, 0 at death): the factor
/// on its `size` and the factor on its colour. A falling particle (a
/// spark, a splinter: `gravity` above zero) shrinks away and keeps its
/// colour; a drifting one (smoke, dust) swells by 40% and dims to 40%,
/// which is the only way an opaque pass can thin a puff out.
///
/// The frame draws the ball at `size * factor * PUFF_BALL`; the plume's
/// clearance test reads the same pair, so what the eye is promised and
/// what the frame draws cannot drift apart.
#[must_use]
pub fn puff_draw(gravity: f32, k: f32) -> (f32, f32) {
    if gravity > 0.0 {
        (k, 1.0)
    } else {
        (1.4 - 0.4 * k, 0.4 + 0.6 * k)
    }
}

/// A basis across a surface normal: two unit tangents, so a burst can fan
/// out over the face it hit. A zero normal (a body, a shield, an event from
/// a peer that sends none) fans out about +Y.
fn tangents(normal: Vec3) -> (Vec3, Vec3, Vec3) {
    let n = if normal.length_squared() < 1e-6 {
        Vec3::Y
    } else {
        normal.normalize()
    };
    let helper = if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
    let t1 = n.cross(helper).normalize();
    let t2 = n.cross(t1);
    (n, t1, t2)
}

/// A fan of `count` puffs about `normal` at `at`, each leaving at `speed`
/// along a direction mixed from the normal (weight `out`), a ring
/// direction across the face (weight `across`) and straight up (weight
/// `up`). A fixed fan, not random: there is no RNG on the client and a
/// burst does not need one.
#[allow(clippy::too_many_arguments)]
fn fan(
    at: Vec3,
    normal: Vec3,
    count: u8,
    out: f32,
    across: f32,
    up: f32,
    speed: f32,
    ttl: f32,
    size: f32,
    color: Vec3,
    gravity: f32,
) -> Vec<Puff> {
    let (n, t1, t2) = tangents(normal);
    (0..count)
        .map(|k| {
            let a = f32::from(k) * std::f32::consts::TAU / f32::from(count.max(1)) + 0.3;
            let (s, c) = a.sin_cos();
            let dir = n * out + (t1 * c + t2 * s) * across + Vec3::Y * up;
            Puff {
                pos: at,
                vel: dir.normalize_or_zero() * speed,
                ttl,
                size,
                color,
                gravity,
                // A burst is the impact: it is there the moment the round
                // is.
                delay: 0.0,
            }
        })
        .collect()
}

/// What a round hit, as far as the eye and the ear care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Metal,
    Stone,
    Wood,
    Sand,
}

/// The material at the end of a shot that ended in the world (`hit` one of
/// cover, floor, wall), `None` for any other ending.
///
/// The plan's list, read against the pictures actually on the boxes
/// (`tools/v13/gen_textures.py`): the container and the riveted loot plate
/// are metal; the plinth, the rubble, the tunnel roof, the cobble floor and
/// the arena's limestone balustrade are stone; the crate, the ammunition box
/// and the trench wall (timber revetment over packed earth, not sheet) are
/// wood; the sandbag line is sand. A cover index this build does not know
/// reads as stone, the material most of the map is.
#[must_use]
pub const fn impact_material(hit: u8, cover: u8) -> Option<Material> {
    match hit {
        SHOT_FLOOR | SHOT_WALL => Some(Material::Stone),
        SHOT_COVER => Some(match Cover::from_index(cover) {
            Some(Cover::Container | Cover::Loot) => Material::Metal,
            Some(Cover::Crate | Cover::Ammo | Cover::Wall) => Material::Wood,
            Some(Cover::Sandbag) => Material::Sand,
            Some(Cover::Plinth | Cover::Rubble | Cover::Roof) | None => Material::Stone,
        }),
        _ => None,
    }
}

impl Material {
    /// The impact's sound.
    #[must_use]
    pub const fn sfx(self) -> Sfx {
        match self {
            Self::Metal => Sfx::ImpactMetal,
            Self::Stone => Sfx::ImpactStone,
            Self::Wood => Sfx::ImpactWood,
            Self::Sand => Sfx::ImpactSand,
        }
    }

    /// The impact's base volume, before distance.
    #[must_use]
    pub const fn volume(self) -> f32 {
        match self {
            Self::Metal => 0.35,
            Self::Stone | Self::Wood => 0.30,
            Self::Sand => 0.25,
        }
    }

    /// The burst at `at` off a face with `normal`: eight white-yellow sparks
    /// under gravity off metal, six grey-brown balls of dust rising off
    /// stone, six tan splinters off wood, five sand puffs off a sandbag.
    #[must_use]
    pub fn burst(self, at: Vec3, normal: Vec3) -> Vec<Puff> {
        match self {
            Self::Metal => fan(
                at,
                normal,
                8,
                1.0,
                1.0,
                0.25,
                6.0,
                0.25,
                0.04,
                Vec3::new(1.0, 0.95, 0.60),
                9.81,
            ),
            Self::Stone => fan(
                at,
                normal,
                6,
                0.5,
                0.45,
                0.8,
                1.0,
                0.5,
                0.12,
                Vec3::new(0.46, 0.41, 0.35),
                0.0,
            ),
            Self::Wood => fan(
                at,
                normal,
                6,
                1.0,
                1.0,
                0.4,
                4.0,
                0.35,
                0.05,
                Vec3::new(0.72, 0.55, 0.32),
                9.81,
            ),
            Self::Sand => fan(
                at,
                normal,
                5,
                0.4,
                0.4,
                0.6,
                0.8,
                0.4,
                0.10,
                Vec3::new(0.76, 0.68, 0.48),
                0.0,
            ),
        }
    }
}

/// The sparks off a body at the contact point: red-brown, under gravity.
#[must_use]
pub fn body_sparks(at: Vec3) -> Vec<Puff> {
    fan(
        at,
        Vec3::ZERO,
        6,
        0.8,
        1.0,
        0.0,
        2.8,
        0.3,
        0.08,
        Vec3::new(0.55, 0.12, 0.08),
        9.0,
    )
}

/// The second, shorter spark cone of a ricochet, at a slant off the face.
#[must_use]
pub fn ricochet_sparks(at: Vec3, normal: Vec3) -> Vec<Puff> {
    fan(
        at,
        normal,
        4,
        0.6,
        1.0,
        0.3,
        8.0,
        0.18,
        0.03,
        Vec3::new(1.0, 0.85, 0.45),
        9.81,
    )
}

/// The base volume of the ricochet whine.
pub const RICOCHET_VOLUME: f32 = 0.3;

/// Whether the round that ended at `to` on metal sings off it: one in
/// eight, decided by the end point so every peer that sees the same event
/// hears the same thing, and a burst on one spot is not eight whines.
#[must_use]
pub fn ricochets(to: [f32; 3]) -> bool {
    let bits = |v: f32| u64::from(v.to_bits());
    let h = hash64(bits(to[0]) ^ bits(to[1]).rotate_left(21) ^ bits(to[2]).rotate_left(42));
    h.trailing_zeros() >= 3
}

/// The plume's cube edge, which is twice its drawn ball's radius. Small on
/// purpose: at 0.10 the four balls were 5 cm across at birth and 7 at
/// death, and on a muzzle 0.7 m from the eye they hid the front sight, the
/// barrel tip and part of what was being shot at for the whole quarter
/// second (captured). At 0.04 a ball is 2 cm across at birth and 2.8 at
/// death, which is a puff at arm's length and still a puff from across the
/// yard, where the thing that reads is the spread of the ring rather than
/// any one ball.
pub const PLUME_SIZE: f32 = 0.04;

/// The radius of the ring the four plume puffs are spawned on, metres.
/// Wider than a ball is (`PLUME_CLEAR`), so the ring is a ring: the bore
/// shows through the middle of the smoke, and with it the front sight and
/// the target, instead of standing behind a solid mass of it. Ring plus
/// ball still comes to the 0.09 m it always did, so `plume_reach` and the
/// star floor that reads it (`online::FLASH_CLEAR`) are unmoved and no
/// weapon's flash changes size.
pub const PLUME_RING: f32 = 0.07;

/// How far down the bore from the muzzle the plume starts, metres: the
/// flash star is born at the muzzle itself, so the smoke buds ahead of the
/// star's base ring and grows away down the bore instead of over it. Short
/// of the tip of even the smallest star's forward cone (0.126 m at
/// `online::FLASH_FORWARD` is 0.20 m), so the smoke never buds off the end
/// of the light.
pub const PLUME_LEAD: f32 = 0.08;

/// The drawn radius of one plume puff the instant it is born, metres.
#[must_use]
pub const fn plume_radius() -> f32 {
    PLUME_SIZE * PUFF_BALL
}

/// How far off the bore the plume's outer edge stands when it is born,
/// metres: the spawn ring plus one ball. This is the width of smoke a
/// shot's petals have to reach past to read as the bigger thing on the
/// frame the light hands over to the smoke.
#[must_use]
pub const fn plume_reach() -> f32 {
    PLUME_RING + plume_radius()
}

/// The radius of the hole down the bore the plume leaves when it is born,
/// metres: the spawn ring less one ball, 0.05 m as the two are set. The
/// sight picture lives in this hole — the front sight, the barrel tip and
/// whatever is behind them all sit within a couple of degrees of the bore
/// from where the shooter's eye is — so it has to stay open for the whole
/// life of the smoke. Birth is the tightest moment: the balls leave the
/// ring faster than they swell and faster than they rise, so the hole only
/// widens from here (`the_plume_never_closes_the_sight_line` walks it).
pub const PLUME_CLEAR: f32 = PLUME_RING - plume_radius();

/// A ring no wider than its own ball is not a ring: the four puffs meet
/// over the bore and the sight picture goes back behind solid smoke, which
/// is the defect this pair of numbers was set to fix. Whoever grows
/// `PLUME_SIZE` grows `PLUME_RING` with it or fails here.
const _: () = assert!(PLUME_CLEAR > 0.0);

/// How long a weapon's plume is held back: exactly the life of its flash,
/// so the star has the first 35 to 60 ms of the shot to itself and the
/// smoke arrives as the light goes. A shot reads as light first, smoke
/// second.
#[must_use]
pub const fn plume_delay_of(weapon: u8) -> f32 {
    weapon_feel(weapon).flash_ms
}

/// The muzzle plume of `weapon`: four small dark balls in a ring about the
/// bore, `PLUME_LEAD` down it from `at`, drifting along `dir`, opening out
/// and rising a little, a quarter second, after the flash rather than
/// beside it. Four and a quarter second as they always were; what changed
/// is that they are smaller, darker and spread wide enough to leave the
/// bore clear (`PLUME_CLEAR`), so the smoke is at the edge of the sight
/// picture rather than across it.
///
/// The delay rides on the puff instead of on a queue of pending plumes in
/// the frame: one field and one branch in the retain that already walks
/// every particle, against a second list with a second lifetime, a second
/// expiry and a spawn point that would have gone stale (the local muzzle
/// moves with the view between the shot and the flash's end).
#[must_use]
pub fn plume(at: Vec3, dir: Vec3, weapon: u8) -> Vec<Puff> {
    let (n, t1, t2) = tangents(dir);
    let from = at + n * PLUME_LEAD;
    let delay = plume_delay_of(weapon);
    (0..4u8)
        .map(|k| {
            let a = f32::from(k) * std::f32::consts::FRAC_PI_2 + 0.5;
            let (s, c) = a.sin_cos();
            Puff {
                pos: from + (t1 * c + t2 * s) * PLUME_RING,
                // Out of the ring faster than it rises. The old drift rose
                // at 0.5 m/s and opened at 0.35, so the lower balls floated
                // up into the bore over the quarter second and closed the
                // hole they were born with; at 0.6 out against 0.25 up the
                // hole only widens.
                vel: dir * 1.2 + (t1 * c + t2 * s) * 0.6 + Vec3::Y * 0.25,
                ttl: 0.25,
                size: PLUME_SIZE,
                // Powder smoke, not steam. Every untextured grey in the map
                // is darker than the 0.50 this was: the floor slab is
                // 0.12-0.17, the arena wall 0.26-0.34, gunmetal 0.16-0.20.
                // A colour brighter than every surface in the scene is what
                // made the plume read as white against a rusty container in
                // a golden-hour light, post-tonemap. 0.22 sits just under
                // the wall's grey, so the smoke is a dark thing in front of
                // a lit map from any angle, and it is still well clear of
                // the mark's near-black.
                color: Vec3::new(0.22, 0.21, 0.20),
                gravity: 0.0,
                delay,
            }
        })
        .collect()
}

/// A brass casing: its box, its colour, its life, and the gravity it falls
/// under.
pub const CASING_SIZE: Vec3 = Vec3::new(0.05, 0.02, 0.02);
pub const CASING_COLOR: Vec3 = Vec3::new(0.80, 0.62, 0.25);
pub const CASING_SECS: f32 = 0.6;
pub const CASING_GRAVITY: f32 = 9.81;
/// The tink on the cobbles.
pub const CASING_VOLUME: f32 = 0.18;

/// Where a casing leaves the gun and how fast: out of the ejection port
/// beside the muzzle, to the right and up, a little back.
#[must_use]
pub fn casing_eject(muzzle: Vec3, right: Vec3, look: Vec3) -> (Vec3, Vec3) {
    (
        muzzle - look * 0.25 + right * 0.03,
        right * 2.0 + Vec3::Y * 2.4 - look * 0.4,
    )
}

/// Seconds a casing thrown up at `vy` from `height` above where it lands
/// takes to land: the positive root of the fall. Zero height and no throw
/// is zero.
#[must_use]
pub fn fall_secs(height: f32, vy: f32) -> f32 {
    let height = height.max(0.0);
    (vy + (vy * vy + 2.0 * CASING_GRAVITY * height).sqrt()) / CASING_GRAVITY
}

/// An impact mark: a near-black hole laid flat on the surface a round hit,
/// so a fight leaves its history on the containers. It was a 0.1 m square
/// plate whatever hit; through the sniper's scope at 4.7 m the plate filled
/// the view as one black square. Now it is a disc (`rounds::DISC_OFFSET`)
/// sized by the round that made it (`Mark::diameter`), so the weapon rides
/// along.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub pos: Vec3,
    /// Unit outward normal of the surface.
    pub normal: Vec3,
    /// The weapon that fired the round, from the `Shot` event: what sizes
    /// the hole.
    pub weapon: u8,
    pub born: f32,
}

/// At most this many marks; the oldest goes when a new one arrives.
pub const MARK_CAP: usize = 96;
/// Seconds a mark stays.
pub const MARK_SECS: f32 = 20.0;
/// A bullet's hole is this many calibres across: a 9 mm leaves 27 mm, a
/// .338 Lapua 26 mm, a .454 Casull 35 mm. Real holes in sheet steel are
/// nearer one calibre, but the mark is the only lasting trace of a hit
/// and at one calibre it is under a pixel past a few metres.
pub const MARK_CALIBRES: f32 = 3.0;
/// The rocket's blast mark: its diameter, metres.
pub const ROCKET_MARK: f32 = 0.5;
/// The hole's depth (the disc's thickness along the normal) and how far its
/// front face stands proud of the surface, so it never z-fights the face it
/// sits on: 1 mm. The rest of the disc is SUNK into the obstacle, which is
/// opaque and hides it. It stood the other way round at first, the whole
/// 4 mm slab outside the face; the first capture through the scope showed a
/// hole head-on but a black puck glued to the wall at a grazing angle, so
/// the free variable — where the slab sits along the normal — went the
/// other way. Only the 1 mm shows now, from every angle.
pub const MARK_THICK: f32 = 0.004;
pub const MARK_LIFT: f32 = 0.001;
const _: () = assert!(MARK_LIFT > 0.0 && MARK_LIFT < MARK_THICK);
pub const MARK_COLOR: Vec3 = Vec3::splat(0.04);

/// The event's axis normal as a unit vector; zero (a peer that sent none)
/// reads as up, so the mark lies flat rather than not at all.
#[must_use]
pub fn mark_normal(n: [i8; 3]) -> Vec3 {
    let v = Vec3::new(f32::from(n[0]), f32::from(n[1]), f32::from(n[2]));
    if v.length_squared() < 0.5 {
        Vec3::Y
    } else {
        v.normalize()
    }
}

impl Mark {
    /// The hole's diameter, metres: `MARK_CALIBRES` times the round's real
    /// calibre for a bullet weapon, `ROCKET_MARK` for the rocket.
    #[must_use]
    pub fn diameter(&self) -> f32 {
        rounds::round_for(self.weapon)
            .map_or(ROCKET_MARK, |r| r.calibre_mm().0 * MARK_CALIBRES * 0.001)
    }

    /// Where the hole's disc goes: sunk into the surface so that only
    /// `MARK_LIFT` of its thickness stands proud of the face, the rest
    /// inside the (opaque) obstacle. The disc's own base sits at
    /// `pos - normal * (MARK_THICK - MARK_LIFT)` and it grows along the
    /// normal from there, so its front face is `MARK_LIFT` out and never
    /// fights the face for the pixel while nothing of the slab's side
    /// shows at a grazing angle. Scale applies before rotation, so the
    /// disc — radius 1 in its YZ plane, thick along its own +X from 0 to
    /// 1 — is scaled `(thick, r, r)` and +X is rotated onto the normal.
    #[must_use]
    pub fn placement(&self) -> (Vec3, Vec3, Quat) {
        let r = self.diameter() * 0.5;
        (
            self.pos - self.normal * (MARK_THICK - MARK_LIFT),
            Vec3::new(MARK_THICK, r, r),
            Quat::from_rotation_arc(Vec3::X, self.normal),
        )
    }

    #[must_use]
    pub fn alive(&self, now: f32) -> bool {
        now - self.born < MARK_SECS
    }
}

/// Add a mark, dropping the oldest once the cap is reached.
pub fn add_mark(marks: &mut VecDeque<Mark>, mark: Mark) {
    while marks.len() >= MARK_CAP {
        marks.pop_front();
    }
    marks.push_back(mark);
}

/// Drop the marks that have had their twenty seconds. They are in age
/// order, so this is a pop from the front until the front is young.
pub fn expire_marks(marks: &mut VecDeque<Mark>, now: f32) {
    while marks.front().is_some_and(|m| !m.alive(now)) {
        marks.pop_front();
    }
}

/// How close a round has to pass the eye to crack, metres.
pub const CRACK_RADIUS: f32 = 3.0;

/// The nearest point of the segment `a -> b` to `p`, as a distance.
#[must_use]
pub fn segment_distance(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 < 1e-9 {
        0.0
    } else {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    };
    (a + ab * t - p).length()
}

/// The supersonic crack of a round passing the eye: its volume, or `None`.
/// Only a round faster than sound has a shock cone, and only one within
/// `CRACK_RADIUS` of the eye is close enough to hear it as a crack rather
/// than as the shot; the volume rises with closeness.
#[must_use]
pub fn crack(from: Vec3, to: Vec3, eye: Vec3, speed: f32) -> Option<f32> {
    if speed <= SPEED_OF_SOUND {
        return None;
    }
    let d = segment_distance(from, to, eye);
    (d < CRACK_RADIUS).then(|| 0.25 + 0.55 * (1.0 - d / CRACK_RADIUS))
}

/// One queued sound: what, how loud, where from (a pan of -1 hard left to
/// 1 hard right) and how late it arrives. A cue of the player's own is at
/// the centre with no delay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Play {
    pub sfx: Sfx,
    pub vol: f32,
    pub pan: f32,
    pub delay: f32,
}

impl Play {
    #[must_use]
    pub const fn centre(sfx: Sfx, vol: f32) -> Self {
        Self {
            sfx,
            vol,
            pan: 0.0,
            delay: 0.0,
        }
    }

    /// A cue from `source` heard at `ear` whose right-hand side is `right`.
    /// The volume is the caller's: the fall-off differs by cue.
    #[must_use]
    pub fn spatial(sfx: Sfx, vol: f32, source: Vec3, ear: Vec3, right: Vec3) -> Self {
        let (pan, delay) = spatial(source, ear, right);
        Self {
            sfx,
            vol,
            pan,
            delay,
        }
    }

    /// A `Play` from an event cue's sound.
    #[must_use]
    pub const fn from_cue(sfx: (Sfx, f32)) -> Self {
        Self::centre(sfx.0, sfx.1)
    }
}

/// The pan and the delay of a source heard at `ear`: the pan is the right
/// vector's share of the direction to the source, the delay the distance
/// over the speed of sound. A source at the ear is centred and immediate.
#[must_use]
pub fn spatial(source: Vec3, ear: Vec3, right: Vec3) -> (f32, f32) {
    let d = source - ear;
    let dist = d.length();
    if dist < 1e-3 {
        return (0.0, 0.0);
    }
    let pan = (right.normalize_or_zero().dot(d / dist)).clamp(-1.0, 1.0);
    (pan, dist / SPEED_OF_SOUND)
}

/// The gunshot cue for a weapon at a distance: the sound package's
/// layered shot in the variant `Dist::at` picks, and for an id off the
/// table the near cue of the gun it draws as, which is the feel row's.
#[must_use]
pub fn shot_sfx(weapon: u8, dist: Dist) -> Sfx {
    Sfx::shot(weapon, dist).unwrap_or_else(|| weapon_feel(weapon).sound)
}

/// A remote gunshot's volume at distance `d`: the weapon's own, falling
/// off to a floor over 40 m so a far shot is still heard.
#[must_use]
pub fn remote_shot_volume(feel: &WeaponFeel, d: f32) -> f32 {
    (feel.volume * 0.9 * (1.0 - d / 40.0)).clamp(0.05, feel.volume)
}

/// The fall-off an impact, a ricochet or a body hit takes with distance:
/// full within a few metres, a fifth at sixty and beyond.
#[must_use]
pub fn falloff(d: f32) -> f32 {
    (1.0 - d / 60.0).clamp(0.2, 1.0)
}

/// Order a frame's plays so `take(BUDGET)` keeps the important ones: the
/// same rule as `sound::prioritize`, on the spatial queue. Stable, so two
/// cues of one rank play in the order the frame raised them.
pub fn prioritize_plays(queue: &mut [Play]) {
    queue.sort_by_key(|p| p.sfx.priority());
}

/// Whether a weapon id's rounds are drawn from shot events (bullets) or
/// from the state (the rocket, a mesh in flight).
#[must_use]
pub fn traces(weapon: u8) -> bool {
    weapon_stats(weapon).kind == Projectile::Bullet
}

// ---- v20: footsteps ----

/// The gait a body is heard in. Crouch is not a gait: a crouched body is
/// silent whatever its speed, which is the whole point of crouching.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gait {
    Walk,
    Run,
}

/// Where in the walk cycle the left foot lands.
///
/// `rig::walk_pose` swings the left leg by `sin(phase)` and the right by
/// `sin(phase + PI)`, so the left leg reaches its forward extreme - the
/// heel strike - at `phase = PI/2` and the right at `3PI/2`. Two plants to
/// a cycle, one stride. Nothing here may drift from that constant without
/// the steps sliding off the boots.
const PLANT_OFFSET: f32 = std::f32::consts::FRAC_PI_2;

/// The phase between one plant and the next: half a cycle, one foot.
const PLANT_SPACING: f32 = std::f32::consts::PI;

/// How many feet a body has put down by this phase, which is also its step
/// counter and so what picks the variant. `puppet::advance_anim` only ever
/// adds to a phase, so this only ever rises.
#[must_use]
pub fn plant_index(phase: f32) -> i64 {
    // A phase advances 6 radians per metre walked; a match cannot walk
    // anywhere near 2^63 of them, and a NaN phase reports no plants.
    #[allow(clippy::cast_possible_truncation)]
    let k = ((phase - PLANT_OFFSET) / PLANT_SPACING).floor() as i64;
    k + 1
}

/// How many feet landed while the phase went from `prev` to `now`.
#[must_use]
pub fn plants_crossed(prev: f32, now: f32) -> u32 {
    u32::try_from((plant_index(now) - plant_index(prev)).max(0)).unwrap_or(u32::MAX)
}

/// Under this share of the walking stance speed a body is standing about,
/// not moving. It sits below the crouching speed (a crouch-walk is moving,
/// it is simply silent) and above the jitter a lerped remote position
/// leaves in a one-frame difference.
const STEP_FLOOR_SHARE: f32 = 0.25;

/// Over this multiple of the sprinting stance speed a body did not run,
/// it was moved: a respawn, or a reconciliation snapping a remote across
/// the yard. Nothing is heard, rather than one very loud sprint step.
const TELEPORT_SHARE: f32 = 1.5;

/// The gait a measured horizontal speed reads as, `None` for a body that
/// is barely moving or was teleported.
///
/// The boundary between the two gaits is the midpoint of `arena_core`'s own
/// walking and sprinting stance speeds, so it moves when they do; it is
/// never a guess about how fast a runner looks. There is no sprint flag on
/// the wire, and this is the reason none is needed.
#[must_use]
pub fn gait(speed: f32) -> Option<Gait> {
    let walk = stance_speed(false, false, false);
    let run = stance_speed(true, false, false);
    if speed.is_nan() || speed < walk * STEP_FLOOR_SHARE || speed > run * TELEPORT_SHARE {
        return None;
    }
    if speed >= f32::midpoint(walk, run) {
        Some(Gait::Run)
    } else {
        Some(Gait::Walk)
    }
}

/// The shortest gap between two steps of a walking body, seconds.
///
/// The legs are not a metronome: `puppet::advance_anim` advances the walk
/// phase by 6 radians per metre and a plant falls every PI of them, so at
/// this arena's 9 m/s walk the animation plants about 17 times a second and
/// at a 14.4 m/s sprint about 27. Played straight that is a rattle, not a
/// pair of boots (measured on a scripted client: 7 plants in 0.35 s). The
/// speeds are the game's and are not up for negotiation here, so the ear
/// gets a floor instead: a step sounds only if this body has been quiet for
/// at least this long, and it still lands ON a plant, so the sound is on a
/// foot that is going down. 0.34 s is about three steps a second, a brisk
/// walk.
pub const WALK_GAP: f32 = 0.34;

/// The same for a sprint: about four steps a second, and a fifth quicker
/// than the walk so a runner is heard to be running before the cue itself
/// is recognised.
pub const RUN_GAP: f32 = 0.26;

/// The floor for a gait.
#[must_use]
pub const fn step_gap(g: Gait) -> f32 {
    match g {
        Gait::Walk => WALK_GAP,
        Gait::Run => RUN_GAP,
    }
}

/// A stranger's walking step, at the ear, before distance. Above the
/// casing's tink (0.18) and well under an impact (0.25 to 0.35): a boot
/// next to you is information, a boot is not a bullet.
pub const STEP_WALK_VOLUME: f32 = 0.30;

/// A stranger's sprinting step: half again as loud as their walk, on top
/// of being a heavier cue.
pub const STEP_RUN_VOLUME: f32 = 0.50;

/// My own steps, as a share of a stranger's at the same distance.
///
/// They are at my own feet, so physically they would be the loudest thing
/// in the mix - and they are also the one sound in the game that tells me
/// nothing I do not already know, played twice a stride for as long as I
/// am moving. At two fifths my own walk lands on the hitmarker tick's 0.12
/// and my own sprint on the casing's 0.18: present, and never loud enough
/// to mask the stranger it is my job to hear.
pub const OWN_STEP_SHARE: f32 = 0.4;

/// How far a footstep carries. A rifle is heard across the map (its volume
/// only floors out at 40 m); a boot on cobbles is not, and a step audible
/// from across the yard would turn eight players into weather. Eighteen
/// metres is just inside the pop's own earshot (20 m), the other cue that
/// means "someone is near me right now".
pub const STEP_EARSHOT: f32 = 18.0;

/// How many footsteps one frame may start, before the frame's own
/// `BUDGET`. The walk cycle advances with distance, so a lobby of eight
/// sprinters can put several boots down in one frame; this keeps them from
/// taking the whole budget, and `priority` keeps the survivors behind
/// everything that matters. Under `BUDGET`, deliberately: a frame's worth
/// of footsteps must never be able to drop a gunshot on its own.
pub const STEP_CAP: usize = 3;

/// A body is in the air when its vertical speed is not zero: `step_vertical`
/// zeroes `vy` the moment the feet are supported and the wire carries that
/// value, so this needs no new field and no guess about ground height.
const AIRBORNE_VY: f32 = 0.05;

/// A footstep's volume at `dist` metres, `None` beyond earshot. My own is
/// a flat share of a stranger's: my feet do not get further from my ears.
#[must_use]
pub fn step_volume(g: Gait, dist: f32, own: bool) -> Option<f32> {
    let base = match g {
        Gait::Walk => STEP_WALK_VOLUME,
        Gait::Run => STEP_RUN_VOLUME,
    };
    if own {
        return Some(base * OWN_STEP_SHARE);
    }
    // A NaN distance is out of earshot rather than at the ear.
    if dist.is_nan() || dist >= STEP_EARSHOT {
        return None;
    }
    let k = 1.0 - dist.max(0.0) / STEP_EARSHOT;
    Some(base * k * k)
}

/// How many boots each gait has. Three is enough that a run does not
/// tick like a clock and few enough to keep the kit small; the cue
/// table in `sound` has exactly this many per gait.
pub const STEP_VARIANTS: u8 = 3;

/// Which variant a body's step uses: the sim's own hash finaliser over the
/// player and the step's index. No state is carried between frames and
/// nothing is drawn from a shared generator, so two clients watching one
/// runner pick the same boots and a hitch cannot shift the pattern.
#[must_use]
pub fn step_variant(who: u8, index: i64) -> u8 {
    let h = hash64(
        index
            .unsigned_abs()
            .wrapping_mul(0x100)
            .wrapping_add(u64::from(who)),
    );
    u8::try_from(h % u64::from(STEP_VARIANTS)).unwrap_or(0)
}

/// The cue one body's step plays.
#[must_use]
pub fn step_sfx(g: Gait, who: u8, index: i64) -> Sfx {
    Sfx::step(g == Gait::Run, step_variant(who, index))
}

/// Everything known about one body at the instant a foot might land.
///
/// Nothing here is new on the wire: `PState` carries the position the speed
/// is measured from, `vy`, `crouch` and `alive`, and the phase is the one
/// the body's own legs are already posed from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stepper {
    pub who: u8,
    pub alive: bool,
    pub crouch: bool,
    /// Vertical speed, from the state or from my own prediction.
    pub vy: f32,
    /// Measured horizontal speed in m/s: this frame's displacement over
    /// `dt`, never an intended speed and never a wire field.
    pub speed: f32,
    /// The walk phase before and after this frame advanced it.
    pub prev_phase: f32,
    pub phase: f32,
    /// Seconds since this body last sounded a step, `f32::INFINITY` if it
    /// never has. The cadence floor below reads it.
    pub since_last: f32,
}

/// Whether a cue is a footstep at all, and whether it is a walking one.
/// The tests read a frame's played cues through these rather than listing
/// six variants at every call site; nothing in the game asks, so they are
/// built only for the test profile.
#[cfg(test)]
#[must_use]
pub fn is_step(s: Sfx) -> bool {
    (0..STEP_VARIANTS).any(|v| s == Sfx::step(false, v) || s == Sfx::step(true, v))
}

#[cfg(test)]
#[must_use]
pub fn is_walk_step(s: Sfx) -> bool {
    (0..STEP_VARIANTS).any(|v| s == Sfx::step(false, v))
}

/// The cue and volume a body's frame plays, if any.
///
/// `None` for a crouched body at any speed, a dead one, one off the ground,
/// one that is barely moving or was teleported, one out of earshot, and for
/// any frame in which no foot reached the floor. A frame that crossed
/// several plants at once was a hitch, and plays one step rather than a
/// burst.
#[must_use]
pub fn footstep(b: &Stepper, dist: f32, own: bool) -> Option<(Sfx, f32)> {
    if !b.alive || b.crouch || b.vy.abs() > AIRBORNE_VY {
        return None;
    }
    if plants_crossed(b.prev_phase, b.phase) == 0 {
        return None;
    }
    let g = gait(b.speed)?;
    if b.since_last < step_gap(g) {
        return None;
    }
    let vol = step_volume(g, dist, own)?;
    Some((step_sfx(g, b.who, plant_index(b.phase) - 1), vol))
}

#[cfg(test)]
mod feel_tests {
    use super::*;
    use crate::sound::{BUDGET, prioritize};
    use arena_core::shooter::{SHOT_BODY, SHOT_EXPIRED, SHOT_SHIELD, WEAPON_COUNT};

    /// A body standing still, alive, on the ground.
    fn body() -> Stepper {
        Stepper {
            who: 3,
            alive: true,
            crouch: false,
            vy: 0.0,
            speed: 0.0,
            since_last: f32::INFINITY,
            prev_phase: 0.0,
            phase: 0.0,
        }
    }

    /// Walk `b` at `speed` for `secs` at 60 Hz, advancing its phase through
    /// the SAME `puppet::advance_anim` that poses a remote body's legs, and
    /// collect everything it plays. Returns the cues and the phase it
    /// finished on.
    fn walk_for(
        mut b: Stepper,
        speed: f32,
        secs: f32,
        dist: f32,
        own: bool,
    ) -> (Vec<(Sfx, f32)>, f32) {
        let dt = 1.0 / 60.0;
        let mut slot = (0.0f32, b.phase, 0.0f32);
        let mut out = Vec::new();
        b.speed = speed;
        // A whole number of 60 Hz frames.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let frames = (secs / dt).round().max(0.0) as u32;
        for _ in 0..frames {
            b.prev_phase = slot.1;
            ember_engine::puppet::advance_anim(&mut slot, Vec2::new(speed, 0.0), dt);
            b.phase = slot.1;
            if let Some(c) = footstep(&b, dist, own) {
                out.push(c);
            }
        }
        (out, slot.1)
    }

    fn walk_speed() -> f32 {
        stance_speed(false, false, false)
    }

    fn run_speed() -> f32 {
        stance_speed(true, false, false)
    }

    /// Crouching is silence, at every speed a body can reach - and the same
    /// body standing is heard at the same speed, so it is the crouch that
    /// silences it and not the speed.
    #[test]
    fn a_crouching_body_is_never_heard() {
        for speed in [walk_speed() * 0.5, walk_speed(), run_speed()] {
            let mut crouched = body();
            crouched.crouch = true;
            let (cues, _) = walk_for(crouched, speed, 2.0, 1.0, false);
            assert!(
                cues.is_empty(),
                "heard {} crouched cues at {speed}",
                cues.len()
            );
            let (heard, _) = walk_for(body(), speed, 2.0, 1.0, false);
            assert!(!heard.is_empty(), "nothing at {speed} standing");
            // My own crouch is silent too: the point of a crouch is that
            // the OTHER player cannot hear me, and both peers run this.
            let (mine, _) = walk_for(crouched, speed, 2.0, 0.0, true);
            assert!(mine.is_empty(), "heard my own crouch at {speed}");
        }
    }

    /// A step lands on a foot plant: the left at PI/2, the right at 3PI/2,
    /// two to a stride, and one cue for each.
    #[test]
    fn a_stride_is_two_steps_one_for_each_foot() {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        assert_eq!(plant_index(0.0), 0);
        assert_eq!(plant_index(FRAC_PI_2 - 1e-4), 0);
        assert_eq!(plant_index(FRAC_PI_2 + 1e-4), 1);
        assert_eq!(plant_index(PI), 1);
        assert_eq!(plant_index(3.0 * FRAC_PI_2 + 1e-4), 2);
        assert_eq!(plants_crossed(0.0, TAU), 2, "two feet to a stride");
        assert_eq!(plants_crossed(0.0, 10.0 * TAU), 20);
        assert_eq!(plants_crossed(TAU, 0.0), 0, "a phase never runs back");
        assert_eq!(plants_crossed(1.0, 1.0), 0);
        // And through the animation clock: exactly one cue per plant the
        // walk crossed, no more and no fewer.
        let (cues, phase) = walk_for(body(), walk_speed(), 1.0, 1.0, false);
        assert_eq!(
            u32::try_from(cues.len()).unwrap(),
            plants_crossed(0.0, phase),
            "{} cues over {phase} radians",
            cues.len()
        );
        assert!(
            cues.len() > 4,
            "a second of walking is more than four steps"
        );
    }

    /// A sprint is a different cue at a higher rate, and it is louder.
    #[test]
    fn a_sprint_is_the_run_cue_faster_and_louder() {
        let (walk, walk_phase) = walk_for(body(), walk_speed(), 1.0, 1.0, false);
        let (run, run_phase) = walk_for(body(), run_speed(), 1.0, 1.0, false);
        assert!(run_phase > walk_phase);
        assert!(
            run.len() > walk.len(),
            "{} run steps against {} walk steps",
            run.len(),
            walk.len()
        );
        let walk_row = [Sfx::StepWalkA, Sfx::StepWalkB, Sfx::StepWalkC];
        let run_row = [Sfx::StepRunA, Sfx::StepRunB, Sfx::StepRunC];
        assert!(walk.iter().all(|(s, _)| walk_row.contains(s)), "{walk:?}");
        assert!(run.iter().all(|(s, _)| run_row.contains(s)), "{run:?}");
        assert!(run[0].1 > walk[0].1, "a sprint step is the louder one");
        // The boundary is arena_core's, not a guess: the crouching speed
        // reads as a walk, the sprinting speed as a run, and the midpoint
        // of the two stances is where it turns over.
        assert_eq!(gait(stance_speed(false, true, false)), Some(Gait::Walk));
        assert_eq!(gait(walk_speed()), Some(Gait::Walk));
        assert_eq!(gait(run_speed()), Some(Gait::Run));
        let mid = f32::midpoint(walk_speed(), run_speed());
        assert_eq!(gait(mid), Some(Gait::Run));
        assert_eq!(gait(mid - 0.01), Some(Gait::Walk));
        // A body that is standing about, and one that was teleported.
        assert_eq!(gait(0.0), None);
        assert_eq!(gait(walk_speed() * 0.2), None);
        assert_eq!(gait(run_speed() * 2.0), None);
        assert_eq!(gait(f32::NAN), None);
        // A run does not sound like a machine: more than one variant comes
        // out of it, and the variant is a pure function of who and when.
        let mut kinds: Vec<Sfx> = run.iter().map(|(s, _)| *s).collect();
        kinds.sort_by_key(|s| format!("{s:?}"));
        kinds.dedup();
        assert!(kinds.len() > 1, "one run, one variant: {kinds:?}");
        assert_eq!(step_variant(7, 41), step_variant(7, 41));
        let spread: std::collections::BTreeSet<u8> = (0..60).map(|i| step_variant(2, i)).collect();
        assert_eq!(spread, [0, 1, 2].into_iter().collect());
    }

    /// Off the ground, or dead, and there is nothing to hear.
    #[test]
    fn a_body_in_the_air_or_on_its_back_makes_no_sound() {
        for vy in [4.0, -6.0, 0.2, -0.2] {
            let mut jumping = body();
            jumping.vy = vy;
            let (cues, _) = walk_for(jumping, run_speed(), 2.0, 1.0, false);
            assert!(cues.is_empty(), "heard a body at vy {vy}");
        }
        let mut dead = body();
        dead.alive = false;
        let (buried, _) = walk_for(dead, run_speed(), 2.0, 1.0, false);
        assert!(
            buried.is_empty(),
            "heard {} cues from a corpse",
            buried.len()
        );
        // The grounded epsilon does not silence a body on a floor:
        // `step_vertical` reports vy exactly zero there.
        let (grounded, _) = walk_for(body(), run_speed(), 2.0, 1.0, false);
        assert!(!grounded.is_empty(), "a body on the floor is silent");
    }

    /// Boots, not a rattle: the cadence floor holds however fast the legs
    /// spin.
    ///
    /// `puppet::advance_anim` adds 6 radians of walk phase per metre and a
    /// foot plants every PI of them, so this arena's 9 m/s walk plants
    /// about 17 times a second and its 14.4 m/s sprint about 27. Played
    /// plant for plant that is a machine gun, which is what a scripted
    /// client recorded before this floor existed: 7 steps in 0.35 s. One
    /// second of each gait is walked here through the same phase arithmetic
    /// the animation uses, and what comes out is a step rate a boot could
    /// make, with the sprint quicker than the walk.
    #[test]
    fn the_cadence_is_boots_and_not_the_leg_animation() {
        let walk = stance_speed(false, false, false);
        let run = stance_speed(true, false, false);
        let dt = 1.0 / 120.0;
        let sound = |speed: f32| {
            let mut b = body();
            b.speed = speed;
            let mut phase = 0.0f32;
            let mut last: Option<f32> = None;
            let mut steps = 0;
            let mut t = 0.0f32;
            while t < 1.0 {
                b.prev_phase = phase;
                phase += speed * dt * 6.0;
                b.phase = phase;
                b.since_last = last.map_or(f32::INFINITY, |l: f32| t - l);
                if footstep(&b, 1.0, false).is_some() {
                    steps += 1;
                    last = Some(t);
                }
                t += dt;
            }
            steps
        };
        let plants = plants_crossed(0.0, walk * 6.0);
        assert!(plants > 15, "the legs really do plant that fast: {plants}");
        let heard_walking = sound(walk);
        let heard_running = sound(run);
        assert!(
            (2..=4).contains(&heard_walking),
            "a walk is about three steps a second, not {heard_walking}"
        );
        assert!(
            (3..=5).contains(&heard_running),
            "a sprint is about four, not {heard_running}"
        );
        assert!(
            heard_running > heard_walking,
            "and a runner is heard to be running: {heard_running} against {heard_walking}"
        );
    }

    /// A step falls off with distance and stops entirely at earshot.
    #[test]
    fn a_far_step_is_quieter_and_past_earshot_is_nothing() {
        let (near, _) = walk_for(body(), run_speed(), 1.0, 1.0, false);
        let (far, _) = walk_for(body(), run_speed(), 1.0, 12.0, false);
        assert_eq!(near.len(), far.len(), "distance changes volume, not rate");
        for (n, f) in near.iter().zip(&far) {
            assert_eq!(n.0, f.0, "the same step is the same cue");
            assert!(f.1 < n.1, "{} at 12 m is not under {} at 1 m", f.1, n.1);
        }
        for d in [STEP_EARSHOT, STEP_EARSHOT + 5.0, 100.0, f32::NAN] {
            let (gone, _) = walk_for(body(), run_speed(), 1.0, d, false);
            assert!(gone.is_empty(), "heard a step at {d} m");
        }
        assert_eq!(step_volume(Gait::Walk, STEP_EARSHOT, false), None);
        assert!(step_volume(Gait::Walk, STEP_EARSHOT - 0.01, false).is_some());
    }

    /// My own steps are at my own ears: quieter than a stranger's would be
    /// at the same distance, and they do not fall off.
    #[test]
    fn my_own_steps_are_the_quieter_own_ear_volume() {
        for g in [Gait::Walk, Gait::Run] {
            let mine = step_volume(g, 0.0, true).unwrap();
            assert!(mine < step_volume(g, 0.0, false).unwrap());
            // Quieter than a stranger anywhere inside the room I am in.
            for d in 0..=6u8 {
                let stranger = step_volume(g, f32::from(d), false).unwrap();
                assert!(mine < stranger, "mine {mine} against {stranger} at {d} m");
            }
            // And unchanged by a distance that does not apply to me.
            assert_eq!(step_volume(g, 100.0, true), Some(mine));
        }
        assert!(
            step_volume(Gait::Run, 0.0, true).unwrap()
                > step_volume(Gait::Walk, 0.0, true).unwrap()
        );
        let (mine, _) = walk_for(body(), run_speed(), 1.0, 0.0, true);
        let (stranger, _) = walk_for(body(), run_speed(), 1.0, 0.0, false);
        assert_eq!(mine.len(), stranger.len());
        for (m, s) in mine.iter().zip(&stranger) {
            assert_eq!(m.0, s.0);
            assert!(m.1 < s.1, "my own step {} is not under {}", m.1, s.1);
        }
    }

    /// A frame's worth of footsteps can never cost a player a gunshot: the
    /// cap is under the budget, and the priority puts steps last anyway.
    #[test]
    fn footsteps_cannot_flood_one_frame() {
        // A frame's footsteps can never fill the budget on their own.
        const { assert!(STEP_CAP < BUDGET) }
        let mut queue: Vec<Play> = Vec::new();
        for i in 0..8u8 {
            queue.push(Play::centre(step_sfx(Gait::Run, i, i64::from(i)), 0.5));
        }
        queue.push(Play::centre(Sfx::ShotAkMid, 0.4));
        queue.push(Play::centre(Sfx::Blast, 0.9));
        prioritize_plays(&mut queue);
        let played: Vec<Sfx> = queue.iter().take(BUDGET).map(|p| p.sfx).collect();
        assert_eq!(played[0], Sfx::Blast);
        assert_eq!(played[1], Sfx::ShotAkMid);
        // And the plain queue sorts the same way.
        let mut plain: Vec<(Sfx, f32)> = vec![
            (Sfx::StepRunA, 0.5),
            (Sfx::Crack, 0.7),
            (Sfx::StepWalkB, 0.3),
        ];
        prioritize(&mut plain);
        assert_eq!(plain[0].0, Sfx::Crack);
    }

    #[test]
    fn a_tracer_head_travels_at_the_weapons_speed() {
        for id in 1..=WEAPON_COUNT {
            let s = weapon_stats(id);
            let t = Tracer {
                from: Vec3::new(1.0, 1.45, -2.0),
                muzzle: Vec3::new(1.0, 1.45, -2.0),
                to: Vec3::new(1.0, 1.45, 40.0),
                weapon: id,
                born: 3.0,
            };
            assert!((t.len() - 42.0).abs() < 1e-4);
            assert_eq!(t.dir(), Vec3::Z);
            // Ten milliseconds in, the head is speed_max * 0.01 down the
            // line, and at the end it stops at `to`.
            let head = t.head(3.01);
            assert!(
                (head.z - (-2.0 + s.speed_max * 0.01)).abs() < 1e-2,
                "id {id}: head at {head}"
            );
            assert_eq!(t.head(3.0 + t.len() / s.speed_max + 0.05), t.to);
            assert_eq!(t.head(2.5), t.from, "nothing before it was born");
            // Alive through the flight and the linger, gone after.
            let flight = 42.0 / s.speed_max;
            assert!(t.alive(3.0 + flight));
            assert!(t.alive(3.0 + flight + TRACER_LINGER * 0.9));
            assert!(!t.alive(3.0 + flight + TRACER_LINGER + 1e-3));
            assert!(
                t.rods(3.0 + flight + TRACER_LINGER + 1e-3).is_empty(),
                "no rods once gone"
            );
            // Well into the flight: a core 2.5 m back from the head and a
            // tail behind it to 8 m, never overlapping, never past `from`.
            let now = 3.0 + flight * 0.5;
            let rods = t.rods(now);
            assert_eq!(rods.len(), 2, "id {id}");
            let head = t.head(now);
            let (core, tail) = (rods[0], rods[1]);
            assert!((core.len - TRACER_CORE_LEN).abs() < 1e-4);
            assert!((core.center - (head - Vec3::Z * 1.25)).length() < 1e-3);
            assert_eq!(core.color, weapon_feel(id).tracer);
            assert!((tail.len - (TRACER_TAIL_LEN - TRACER_CORE_LEN)).abs() < 1e-4);
            let tail_front = tail.center + Vec3::Z * (tail.len * 0.5);
            let core_back = core.center - Vec3::Z * (core.len * 0.5);
            assert!((tail_front - core_back).length() < 1e-3, "tail meets core");
            assert_eq!(tail.color, weapon_feel(id).tracer * TRACER_TAIL_DIM);
            // Just after leaving: only the core, and only as long as the
            // head has travelled.
            let early = t.rods(3.0 + 0.5 / s.speed_max);
            assert_eq!(early.len(), 1);
            assert!((early[0].len - 0.5).abs() < 1e-3);
            // The fade: half way through the linger the streak is half as
            // wide, and both rods are still there to be drawn.
            let half = t.rods(3.0 + flight + TRACER_LINGER * 0.5);
            assert_eq!(half.len(), 2, "both rods through the linger");
            assert!((t.fade(3.0 + flight + TRACER_LINGER * 0.5) - 0.5).abs() < 1e-3);
            assert_eq!(t.fade(now), 1.0, "no fade in flight");
            // The round itself flies until the head lands, and not
            // through the linger.
            assert!(t.flying(now));
            assert!(!t.flying(3.0 + flight + 1e-3), "landed");
            assert!(!t.flying(3.0 + flight + TRACER_LINGER + 1.0), "gone");
        }
        // A degenerate segment has a direction and no rods.
        let dot = Tracer {
            from: Vec3::ONE,
            muzzle: Vec3::ONE,
            to: Vec3::ONE,
            weapon: 1,
            born: 0.0,
        };
        assert_eq!(dot.dir(), Vec3::X);
        assert!(dot.rods(0.01).is_empty(), "a dot draws nothing");
        assert!(!dot.alive(TRACER_LINGER + 0.01));
        // Bullets trace, the rocket flies as a mesh.
        for id in 1..=WEAPON_COUNT {
            assert_eq!(traces(id), id != 7, "id {id}");
        }
    }

    #[test]
    fn an_impact_picks_the_material_cue() {
        let kind = |c: Cover| impact_material(SHOT_COVER, c.index());
        assert_eq!(kind(Cover::Container), Some(Material::Metal));
        assert_eq!(kind(Cover::Loot), Some(Material::Metal));
        assert_eq!(kind(Cover::Plinth), Some(Material::Stone));
        assert_eq!(kind(Cover::Rubble), Some(Material::Stone));
        assert_eq!(kind(Cover::Roof), Some(Material::Stone));
        assert_eq!(kind(Cover::Crate), Some(Material::Wood));
        assert_eq!(kind(Cover::Ammo), Some(Material::Wood));
        assert_eq!(kind(Cover::Wall), Some(Material::Wood), "timber revetment");
        assert_eq!(kind(Cover::Sandbag), Some(Material::Sand));
        assert_eq!(impact_material(SHOT_FLOOR, 255), Some(Material::Stone));
        assert_eq!(impact_material(SHOT_WALL, 255), Some(Material::Stone));
        assert_eq!(
            impact_material(SHOT_COVER, 200),
            Some(Material::Stone),
            "an unknown kind reads as stone"
        );
        for hit in [SHOT_EXPIRED, SHOT_BODY, SHOT_SHIELD] {
            assert_eq!(impact_material(hit, 0), None, "hit {hit}");
        }
        assert_eq!(Material::Metal.sfx(), Sfx::ImpactMetal);
        assert_eq!(Material::Stone.sfx(), Sfx::ImpactStone);
        assert_eq!(Material::Wood.sfx(), Sfx::ImpactWood);
        assert_eq!(Material::Sand.sfx(), Sfx::ImpactSand);
        // The bursts: the plan's counts, sparks under gravity, dust rising.
        let at = Vec3::new(2.0, 1.0, 3.0);
        let n = Vec3::X;
        let metal = Material::Metal.burst(at, n);
        assert_eq!(metal.len(), 8);
        assert!(
            metal
                .iter()
                .all(|p| p.gravity > 0.0 && p.vel.x > 0.0 && p.pos == at)
        );
        assert!(
            metal.iter().all(|p| (p.vel.length() - 6.0).abs() < 1e-3),
            "sparks leave at 6 m/s"
        );
        let stone = Material::Stone.burst(at, Vec3::Y);
        assert_eq!(stone.len(), 6);
        assert!(stone.iter().all(|p| p.gravity == 0.0 && p.vel.y > 0.8));
        assert_eq!(Material::Wood.burst(at, n).len(), 6);
        assert_eq!(Material::Sand.burst(at, n).len(), 5);
        assert_eq!(body_sparks(at).len(), 6);
        assert_eq!(ricochet_sparks(at, n).len(), 4);
        assert_eq!(plume(at, Vec3::Z, 3).len(), 4);
        // Every puff goes down the bore, and the ring as a whole rises
        // while opening: the ball on the low side of the ring is pushed
        // out faster than the plume lifts, which is what keeps the hole
        // down the bore open (`the_plume_never_closes_the_sight_line`).
        let ring = plume(at, Vec3::Z, 3);
        assert!(ring.iter().all(|p| p.vel.z > 1.0));
        assert!(ring.iter().map(|p| p.vel.y).sum::<f32>() > 0.0);
        // Every burst is born at once; the plume waits out its weapon's
        // flash and starts a little down the bore, not on the muzzle.
        assert!(
            metal
                .iter()
                .chain(&stone)
                .chain(&body_sparks(at))
                .chain(&ricochet_sparks(at, n))
                .all(|p| p.delay == 0.0)
        );
        for id in 1..=WEAPON_COUNT {
            let row = weapon_feel(id);
            for puff in plume(at, Vec3::Z, id) {
                assert!(
                    (puff.delay - row.flash_ms).abs() < 1e-6,
                    "id {id}: the plume waits out the flash"
                );
                assert!(
                    puff.pos.z >= at.z + PLUME_LEAD - 1e-6,
                    "id {id}: down the bore of the flash"
                );
            }
        }
        // One ricochet in eight, by the end point, the same every time.
        let mut sung = 0;
        for k in 0..4000u32 {
            #[allow(clippy::cast_precision_loss)]
            let to = [k as f32 * 0.37, (k % 7) as f32 * 0.11, k as f32 * -0.53];
            assert_eq!(ricochets(to), ricochets(to), "deterministic");
            if ricochets(to) {
                sung += 1;
            }
        }
        assert!((350..=650).contains(&sung), "{sung} of 4000 ricochet");
        // The reload cue is the gun's.
        assert_eq!(reload_start(1).sfx, Some((Sfx::ReloadPistol, 0.45)));
        assert_eq!(reload_start(6).sfx, Some((Sfx::ReloadSniper, 0.45)));
        assert_eq!(reload_start(7).sfx, Some((Sfx::ReloadRpg, 0.45)));
    }

    /// The shooter must be able to see what is being shot at through and
    /// around the smoke at every moment of its life, so the bore stays
    /// clear: every plume puff, integrated over its whole life at the size
    /// the frame actually draws it (`puff_draw`), keeps its ball off the
    /// line of the shot. Birth is the tightest moment; the balls open out
    /// faster than they swell and faster than they rise, so the hole only
    /// grows. And it is still a puff, not a wisp: the ring stands as wide
    /// off the bore as it ever did.
    #[test]
    fn the_plume_never_closes_the_sight_line() {
        let at = Vec3::new(2.0, 1.4, -3.0);
        // Down the bore, across it, and a near-vertical shot, whose
        // tangent basis is the one `tangents` picks off the world Z.
        for dir in [
            Vec3::Z,
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.05, 0.998, 0.0).normalize(),
        ] {
            for id in 1..=WEAPON_COUNT {
                let mut tightest = f32::MAX;
                for puff in plume(at, dir, id) {
                    for step in 0..=25u8 {
                        let t = puff.ttl * f32::from(step) / 25.0;
                        let k = 1.0 - t / puff.ttl;
                        let pos = puff.pos + puff.vel * t;
                        let (edge, _) = puff_draw(puff.gravity, k);
                        let radius = puff.size * edge * PUFF_BALL;
                        // Distance from the line of the shot, which is
                        // where the sights, the barrel tip and the target
                        // all are.
                        let off = pos - at;
                        let clear = (off - dir * off.dot(dir)).length() - radius;
                        tightest = tightest.min(clear);
                    }
                }
                assert!(
                    tightest > 0.04,
                    "id {id} along {dir}: the smoke closed to {tightest} m of the shot line"
                );
                assert!(
                    (tightest - PLUME_CLEAR).abs() < 1e-3,
                    "id {id}: the tightest moment is birth, at PLUME_CLEAR"
                );
            }
        }
        // Still a puff: the ring reaches as far off the bore as it did
        // before it was hollowed out, so a shot smokes from the outside.
        assert!((plume_reach() - 0.09).abs() < 1e-6, "{}", plume_reach());
        // And it reads as smoke, not steam: darker than every untextured
        // grey the map draws itself with (the arena wall's 0.26 is the
        // brightest).
        for puff in plume(at, Vec3::Z, 3) {
            assert!(
                puff.color.max_element() < 0.26,
                "darker than the map's greys: {}",
                puff.color
            );
        }
    }

    /// A streak is laid back from the head toward the muzzle the client
    /// drew, so a remote shot leaves the gun on screen; the head itself,
    /// and with it the round's own body, stays on the server's segment.
    #[test]
    fn a_streak_starts_at_the_drawn_muzzle_and_rejoins_the_line() {
        let from = Vec3::new(0.0, 1.45, 0.0);
        let muzzle = Vec3::new(0.0, 0.85, 0.6);
        let t = Tracer {
            from,
            muzzle,
            to: Vec3::new(0.0, 1.45, 60.0),
            weapon: 3,
            born: 0.0,
        };
        // Five centimetres of flight: the streak already runs from the
        // drawn muzzle to the head, while the head is on the sim's line.
        let early = 0.05 / t.speed();
        let rods = t.rods(early);
        let rear = rods[0].center - t.streak_dir(early) * (rods[0].len * 0.5);
        assert!(
            (rear - muzzle).length() < 1e-3,
            "the streak leaves the drawn gun: {rear}"
        );
        assert!((t.head(early).x - from.x).abs() < 1e-6 && t.head(early).y > 1.4);
        // Well down range the streak has swung onto the flight line: the
        // 0.6 m offset is under a degree by 40 m.
        let late = 40.0 / t.speed();
        assert!(
            t.streak_dir(late).dot(t.dir()) > 0.9998,
            "{}",
            t.streak_dir(late).dot(t.dir())
        );
        // Nothing before the round has left, and the end is the server's.
        assert!(t.rods(0.0).is_empty(), "no streak before the shot moves");
        assert_eq!(t.to.z, 60.0);
    }

    #[test]
    fn a_near_miss_cracks_only_above_the_speed_of_sound() {
        let eye = Vec3::new(0.0, 1.45, 0.0);
        // A round passing 1 m to the side of the eye, 40 m end to end.
        let from = Vec3::new(1.0, 1.45, -20.0);
        let to = Vec3::new(1.0, 1.45, 20.0);
        assert_eq!(crack(from, to, eye, 280.0), None, "the sidearm is subsonic");
        assert_eq!(
            crack(from, to, eye, SPEED_OF_SOUND),
            None,
            "at the line: no"
        );
        let ak = crack(from, to, eye, 715.0).expect("the AK cracks");
        assert!(ak > 0.5 && ak <= 0.8, "{ak}");
        // Closer is louder; past three metres is silent.
        let grazing = crack(
            Vec3::new(0.1, 1.45, -20.0),
            Vec3::new(0.1, 1.45, 20.0),
            eye,
            900.0,
        )
        .unwrap();
        assert!(grazing > ak);
        assert_eq!(
            crack(
                Vec3::new(3.5, 1.45, -20.0),
                Vec3::new(3.5, 1.45, 20.0),
                eye,
                900.0
            ),
            None
        );
        // A round that stopped before reaching the eye is measured from
        // where it stopped, not from the line it would have followed.
        assert_eq!(
            crack(
                Vec3::new(1.0, 1.45, -20.0),
                Vec3::new(1.0, 1.45, -5.0),
                eye,
                900.0
            ),
            None
        );
        assert!((segment_distance(from, to, eye) - 1.0).abs() < 1e-5);
        assert!((segment_distance(from, from, eye) - from.distance(eye)).abs() < 1e-5);
        // Every bullet but the sidearm's is supersonic.
        for id in 1..=WEAPON_COUNT {
            let s = weapon_stats(id);
            let cracks = crack(from, to, eye, s.speed_max).is_some();
            // The sidearm and the rocket (300 m/s at the top of its
            // sustainer) are the two subsonic rows.
            assert_eq!(cracks, id != 1 && id != 7, "id {id} at {}", s.speed_max);
        }
    }

    #[test]
    fn marks_are_capped() {
        let mut marks = VecDeque::new();
        for k in 0..(MARK_CAP + 10) {
            #[allow(clippy::cast_precision_loss)]
            let m = Mark {
                pos: Vec3::new(k as f32, 0.0, 0.0),
                normal: Vec3::Y,
                weapon: 3,
                born: k as f32 * 0.01,
            };
            add_mark(&mut marks, m);
            assert!(marks.len() <= MARK_CAP);
        }
        assert_eq!(marks.len(), MARK_CAP);
        assert_eq!(marks.front().unwrap().pos.x, 10.0, "the oldest ten went");
        assert_eq!(marks.back().unwrap().pos.x, 105.0);
        // Twenty seconds after the front was born it goes, and only it.
        let front_born = marks.front().unwrap().born;
        expire_marks(&mut marks, front_born + MARK_SECS + 1e-3);
        assert_eq!(marks.len(), MARK_CAP - 1);
        expire_marks(&mut marks, 1000.0);
        assert!(marks.is_empty());
        // The hole is sunk into the face: only MARK_LIFT of it stands
        // proud, the rest inside the box, its thickness along the normal.
        // On a -X face the disc's base is 3 mm INSIDE (at x = 5 + 0.003)
        // and it grows out to x = 5 - 0.001.
        let m = Mark {
            pos: Vec3::new(5.0, 1.0, 2.0),
            normal: -Vec3::X,
            weapon: 3,
            born: 0.0,
        };
        let (pos, scale, rot) = m.placement();
        assert_eq!(pos, Vec3::new(5.0 + (MARK_THICK - MARK_LIFT), 1.0, 2.0));
        assert!(
            (pos + m.normal * MARK_THICK - (m.pos + m.normal * MARK_LIFT)).length() < 1e-6,
            "the front face stands exactly MARK_LIFT proud of the surface"
        );
        let r = m.diameter() * 0.5;
        assert_eq!(scale, Vec3::new(MARK_THICK, r, r));
        assert!(
            (rot * Vec3::X + Vec3::X).length() < 1e-5,
            "thickness on the normal"
        );
        assert_eq!(mark_normal([0, 0, -1]), -Vec3::Z);
        assert_eq!(mark_normal([0, 0, 0]), Vec3::Y, "no normal lies flat");
        assert!(m.alive(MARK_SECS - 0.1) && !m.alive(MARK_SECS));
    }

    /// A mark is a hole three calibres across, not the 0.1 m square plate
    /// it was: through the sniper's 20x scope at 4.7 m the plate filled
    /// the view as one black square. A 9 mm leaves 27 mm, the AK's 7.9 mm
    /// 24 mm, the M4's 5.7 mm 17 mm, the Casull 35 mm, the Lapua 26 mm;
    /// the rocket's blast mark is half a metre; and the disc is thinner
    /// than any hole is wide, sunk into the face with 1 mm of it proud.
    #[test]
    fn a_mark_is_a_hole_three_calibres_wide_not_a_square_plate() {
        let mark = |weapon: u8| Mark {
            pos: Vec3::ZERO,
            normal: Vec3::Y,
            weapon,
            born: 0.0,
        };
        let mm = |weapon: u8| (mark(weapon).diameter() * 1000.0).round();
        assert_eq!(mm(1), 27.0, "the sidearm's 9 mm");
        assert_eq!(mm(2), 27.0, "the Vityaz shares it");
        assert_eq!(mm(3), 24.0, "the AK's 7.9 mm");
        assert_eq!(mm(4), 17.0, "the M4's 5.7 mm");
        assert_eq!(mm(5), 35.0, "the .454 Casull");
        assert_eq!(mm(6), 26.0, "the .338 Lapua");
        assert_eq!(mm(7), 500.0, "the rocket's blast");
        assert_eq!(mm(0), 27.0, "an id off the table is the sidearm's");
        for weapon in 1..=7 {
            let m = mark(weapon);
            let (_, scale, _) = m.placement();
            assert!(scale.x < scale.y, "id {weapon}: thinner than wide");
            assert_eq!(scale.y, scale.z, "id {weapon}: round");
            assert!(
                scale.y * 2.0 < 0.1 || weapon == 7,
                "id {weapon}: a bullet's hole is under the old plate"
            );
        }
    }

    #[test]
    fn spatial_pan_and_delay_follow_the_source() {
        let ear = Vec3::new(0.0, 1.45, 0.0);
        let right = Vec3::X;
        // Dead right at 34.3 m: hard right, a tenth of a second late.
        let (pan, delay) = spatial(Vec3::new(34.3, 1.45, 0.0), ear, right);
        assert!((pan - 1.0).abs() < 1e-5);
        assert!((delay - 0.1).abs() < 1e-5);
        let (pan, _) = spatial(Vec3::new(-10.0, 1.45, 0.0), ear, right);
        assert!((pan + 1.0).abs() < 1e-5, "hard left");
        let (pan, delay) = spatial(Vec3::new(0.0, 1.45, 20.0), ear, right);
        assert!(pan.abs() < 1e-5, "straight ahead is centred");
        assert!((delay - 20.0 / SPEED_OF_SOUND).abs() < 1e-6);
        let (pan, _) = spatial(Vec3::new(5.0, 1.45, 5.0), ear, right);
        assert!(
            (pan - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5,
            "45 degrees"
        );
        assert_eq!(spatial(ear, ear, right), (0.0, 0.0), "at the ear");
        // The right vector turns with the head: the same source swaps sides.
        let (pan, _) = spatial(Vec3::new(5.0, 1.45, 0.0), ear, -Vec3::X);
        assert!((pan + 1.0).abs() < 1e-5);
        // A play built from those, and an own cue at the centre.
        let p = Play::spatial(Sfx::Hit, 0.4, Vec3::new(34.3, 1.45, 0.0), ear, right);
        assert_eq!((p.sfx, p.vol), (Sfx::Hit, 0.4));
        assert!((p.pan - 1.0).abs() < 1e-5 && (p.delay - 0.1).abs() < 1e-5);
        assert_eq!(
            Play::centre(Sfx::Kill, 0.5),
            Play {
                sfx: Sfx::Kill,
                vol: 0.5,
                pan: 0.0,
                delay: 0.0
            }
        );
        assert_eq!(
            Play::from_cue((Sfx::Death, 0.55)),
            Play::centre(Sfx::Death, 0.55)
        );
        // The distance variants: the near one is the feel row's own cue,
        // the mid and far ones differ from it, and an id off the table
        // plays the sidearm's.
        for id in 1..=WEAPON_COUNT {
            assert_eq!(shot_sfx(id, Dist::Near), weapon_feel(id).sound);
            assert_ne!(shot_sfx(id, Dist::Mid), weapon_feel(id).sound);
            assert_ne!(shot_sfx(id, Dist::Far), shot_sfx(id, Dist::Mid));
        }
        assert_eq!(shot_sfx(200, Dist::Near), Sfx::ShotSidearmNear);
        assert_eq!(shot_sfx(0, Dist::Far), Sfx::ShotSidearmNear);
        // The fall-offs.
        let ak = weapon_feel(3);
        assert!((remote_shot_volume(&ak, 0.0) - ak.volume * 0.9).abs() < 1e-6);
        assert!(remote_shot_volume(&ak, 20.0) < remote_shot_volume(&ak, 5.0));
        assert_eq!(remote_shot_volume(&ak, 100.0), 0.05);
        assert_eq!(falloff(0.0), 1.0);
        assert_eq!(falloff(100.0), 0.2);
        // The spatial queue sorts like the plain one.
        let mut q = vec![
            Play::centre(Sfx::Shot, 0.3),
            Play::centre(Sfx::Hit, 0.3),
            Play::centre(Sfx::Blast, 0.9),
        ];
        prioritize_plays(&mut q);
        assert_eq!(q[0].sfx, Sfx::Blast);
        assert_eq!(q[1].sfx, Sfx::Hit);
        // A casing thrown up 2.4 m/s from 1.2 m lands in about 0.76 s;
        // from the floor with no throw, at once.
        let t = fall_secs(1.2, 2.4);
        let landed = 1.2 + 2.4 * t - 0.5 * CASING_GRAVITY * t * t;
        assert!(landed.abs() < 1e-4, "{t}: {landed}");
        assert_eq!(fall_secs(0.0, 0.0), 0.0);
        let (pos, vel) = casing_eject(Vec3::new(0.0, 1.2, 1.0), Vec3::X, Vec3::Z);
        assert!(pos.z < 1.0 && vel.x > 0.0 && vel.y > 0.0);
    }

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
    fn sensitivity_scales_with_the_field_of_view() {
        assert!((look_scale(HIP_FOV) - 1.0).abs() < 1e-6, "hip is the unit");
        let sniper = weapon_feel(SCOPED_WEAPON);
        let scoped = look_scale(sniper.fov(1.0));
        assert!(
            (scoped - 1.0 / 20.0).abs() < 1e-3,
            "a 20x scope turns 20x slower: {scoped}"
        );
        // Every other gun slows by exactly its own narrowing, and none
        // reaches the floor.
        for id in 1..=WEAPON_COUNT {
            let f = weapon_feel(id);
            let s = look_scale(f.fov(1.0));
            assert!((s - f.ads_fov / HIP_FOV).abs() < 1e-6, "id {id}: {s}");
            assert!(s >= LOOK_SCALE_FLOOR, "id {id} froze the look");
        }
        // Half way in, half way slowed, for a linear FOV blend.
        let mid = weapon_feel(3).fov(0.5);
        assert!((look_scale(mid) - (mid / HIP_FOV)).abs() < 1e-6);
        // The floor holds a degenerate field.
        assert_eq!(look_scale(0.0), LOOK_SCALE_FLOOR);
        assert_eq!(look_scale(-5.0), LOOK_SCALE_FLOOR);
    }

    #[test]
    fn the_scope_only_appears_for_the_sniper_above_the_zoom_threshold() {
        assert!(scoped(SCOPED_WEAPON, 1.0));
        assert!(scoped(SCOPED_WEAPON, 0.61));
        assert!(!scoped(SCOPED_WEAPON, 0.6), "the threshold itself is hip");
        assert!(!scoped(SCOPED_WEAPON, 0.0));
        for id in (0..=WEAPON_COUNT).filter(|&id| id != SCOPED_WEAPON) {
            assert!(!scoped(id, 1.0), "id {id} has no scope");
        }
        // The scope's own field of view is the one below the threshold's
        // narrowing, so the mask always opens on a world already zooming.
        assert!(weapon_feel(SCOPED_WEAPON).fov(SCOPE_ZOOM) < HIP_FOV * 0.5);
        assert!((weapon_feel(SCOPED_WEAPON).fov(1.0) - 3.5).abs() < 1e-6);
    }

    #[test]
    fn scope_mask_covers_a_21_9_view() {
        // The threshold's field (a wide hole) and the full scope (a tiny
        // one): the mask is a pure function of the half-height, so both
        // must close.
        for fov in [weapon_feel(SCOPED_WEAPON).fov(SCOPE_ZOOM), 3.5, 70.0] {
            let h = scope_half_height(fov);
            let (a, slabs) = scope_mask(fov);
            assert!((a - SCOPE_APOTHEM * h).abs() < 1e-7);
            // The polygon's corners lie on its circumcircle, and the ring
            // between the apothem and it is inside the hole at the corners
            // and on a slab at the edges; the claim of full cover starts at
            // the circumradius.
            #[allow(clippy::cast_precision_loss)]
            let sides = SCOPE_SIDES as f32;
            let circum = a / (std::f32::consts::PI / sides).cos();
            let half_w = h * 21.0 / 9.0;
            let (nx, ny) = (421, 181);
            let mut outside = 0;
            let mut inside = 0;
            for j in 0..ny {
                for i in 0..nx {
                    #[allow(clippy::cast_precision_loss)]
                    let p = Vec2::new(
                        -half_w + 2.0 * half_w * i as f32 / (nx - 1) as f32,
                        -h + 2.0 * h * j as f32 / (ny - 1) as f32,
                    );
                    let on = slabs.iter().filter(|s| s.contains(p)).count();
                    let r = p.length();
                    if r > circum {
                        assert!(on >= 1, "fov {fov}: {p} at r/h {} is uncovered", r / h);
                        outside += 1;
                    } else if r < 0.9 * a {
                        assert_eq!(on, 0, "fov {fov}: {p} at r/h {} is masked", r / h);
                        inside += 1;
                    }
                }
            }
            assert!(outside > 1000 && inside > 1000, "{outside} / {inside}");
            // The corner of the view, and well past it, are still black.
            let far = Vec2::new(half_w, h) * 1.5;
            assert!(slabs.iter().any(|s| s.contains(far)));
            // The slabs are the plan's boxes: 8h long, 6h thick, standing
            // on the polygon's edge at every one of the 24 angles.
            for (k, s) in slabs.iter().enumerate() {
                assert!((s.half_len - 4.0 * h).abs() < 1e-7);
                assert!((s.half_thick - 3.0 * h).abs() < 1e-7);
                assert!((s.normal.length() - 1.0).abs() < 1e-6);
                assert!(s.tangent.dot(s.normal).abs() < 1e-6, "slab {k} is skewed");
                let inner_edge = s.center.dot(s.normal) - s.half_thick;
                assert!(
                    (inner_edge - a).abs() < 1e-6,
                    "slab {k} does not start at the edge"
                );
            }
        }
        // The reticle spans the hole and is thin against it.
        let [horiz, vert] = scope_reticle(1.0);
        assert_eq!(horiz.x, 2.0);
        assert_eq!(vert.y, 2.0);
        assert!(horiz.y < 0.05 && vert.x < 0.05);
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

    /// The plan's team colours are the palette's first two, by value, so a
    /// palette edit that moved blue or red would be caught here and not on
    /// a player's screen.
    #[test]
    fn team_colours_are_blue_and_red() {
        assert_eq!(team_color(0), Vec3::new(0.25, 0.55, 0.95));
        assert_eq!(team_color(1), Vec3::new(0.92, 0.32, 0.28));
        assert_eq!(team_color(7), team_color(1), "off-range reads as red");
        assert_eq!(team_name(0), "BLUE");
        assert_eq!(team_name(1), "RED");
    }

    #[test]
    fn hill_colour_follows_the_holder_state() {
        let king = Vec3::new(0.3, 0.8, 0.4);
        assert_eq!(hill_color(HILL_FREE, None, 1.0), HILL_FREE_COLOR);
        assert_eq!(
            hill_color(3, Some(king), 1.0),
            king,
            "held: the king's colour"
        );
        assert_eq!(
            hill_color(3, None, 1.0),
            HILL_FREE_COLOR,
            "an unknown king reads as free"
        );
        // Contested: orange, pulsing at 4 Hz between 0.55 and 1.0 of it.
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        let mut t = 0.0;
        while t < 0.25 {
            let c = hill_color(HILL_CONTESTED, Some(king), t);
            let k = c.x / HILL_CONTESTED_COLOR.x;
            assert!(
                (c - HILL_CONTESTED_COLOR * k).length() < 1e-5,
                "hue kept at {t}"
            );
            lo = lo.min(k);
            hi = hi.max(k);
            t += 0.001;
        }
        assert!(
            (lo - 0.55).abs() < 0.01 && (hi - 1.0).abs() < 0.01,
            "{lo}..{hi}"
        );
        // One full pulse takes a quarter second.
        let a = hill_color(HILL_CONTESTED, None, 0.1);
        let b = hill_color(HILL_CONTESTED, None, 0.1 + 1.0 / HILL_PULSE_HZ);
        assert!((a - b).length() < 1e-4, "periodic at 4 Hz");
    }

    #[test]
    fn hill_bars_trace_the_footprint() {
        let dock = Hill {
            min: [-4.0, -2.0],
            max: [4.0, 2.0],
            top: 1.2,
        };
        let bars = hill_bars(&dock);
        for (c, s) in &bars {
            assert!(
                (c.y - (1.2 + HILL_BAR_LIFT)).abs() < 1e-6,
                "lifted off the top"
            );
            assert!((s.y - HILL_BAR_TALL).abs() < 1e-6);
            // Thin one way, the footprint's full length the other, so the
            // corners meet.
            let along_x = *s == Vec3::new(8.0, HILL_BAR_TALL, HILL_BAR_THICK);
            let along_z = *s == Vec3::new(HILL_BAR_THICK, HILL_BAR_TALL, 4.0);
            assert!(along_x || along_z, "bar size {s}");
            // Each bar is centred on an edge.
            let east_or_west = (c.x.abs() - 4.0).abs() < 1e-6 && c.z.abs() < 1e-6;
            let north_or_south = (c.z.abs() - 2.0).abs() < 1e-6 && c.x.abs() < 1e-6;
            assert!(east_or_west || north_or_south, "bar centre {c}");
        }
        let (m, s) = hill_marker(&dock);
        assert_eq!(m, Vec3::new(0.0, 1.2 + HILL_MARKER_RISE, 0.0));
        assert_eq!(s, Vec3::splat(HILL_MARKER_EDGE));
    }

    #[test]
    fn the_round_over_cue_is_a_win_or_a_loss_and_one_rumble() {
        let won = round_over(true);
        let lost = round_over(false);
        assert_eq!(won.sfx.unwrap().0, Sfx::Kill);
        assert_eq!(lost.sfx.unwrap().0, Sfx::Death);
        assert_eq!(won.rumble, Some(rumble(0.6, 0.6, 250)));
        assert_eq!(lost.rumble, won.rumble);
        assert_eq!(won.shake, 0.0);
    }
}
