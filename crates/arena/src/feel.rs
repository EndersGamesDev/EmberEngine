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
    weapon_stats,
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
    pub from: Vec3,
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

    /// The rods to draw at `now`: the core from the head back, then the
    /// tail behind the core (never overlapping it: one opaque shape inside
    /// another is invisible, so the tail starts where the core ends). The
    /// frame thins both by `fade` over the last `TRACER_LINGER` seconds.
    /// Nothing before the head has left the muzzle or after the streak is
    /// gone.
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
        let dir = self.dir();
        let head = self.head(now);
        let color = weapon_feel(self.weapon).tracer;
        let core = progress.min(TRACER_CORE_LEN);
        rods.push(Rod {
            center: head - dir * (core * 0.5),
            len: core,
            color,
        });
        let tail = progress.min(TRACER_TAIL_LEN) - core;
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

/// One short-lived opaque cube: a spark, a splinter, a puff of dust or
/// smoke. The frame owns the integration; this is the spawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ttl: f32,
    pub size: f32,
    pub color: Vec3,
    /// Downward acceleration; zero for anything that drifts.
    pub gravity: f32,
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
    /// under gravity off metal, six grey-brown dust cubes rising off stone,
    /// six tan splinters off wood, five sand puffs off a sandbag.
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

/// The muzzle plume: four grey cubes off `at` drifting along `dir` and
/// rising, a quarter second, beside the flash star.
#[must_use]
pub fn plume(at: Vec3, dir: Vec3) -> Vec<Puff> {
    let (_, t1, t2) = tangents(dir);
    (0..4u8)
        .map(|k| {
            let a = f32::from(k) * std::f32::consts::FRAC_PI_2 + 0.5;
            let (s, c) = a.sin_cos();
            Puff {
                pos: at + (t1 * c + t2 * s) * 0.04,
                vel: dir * 1.2 + (t1 * c + t2 * s) * 0.35 + Vec3::Y * 0.5,
                ttl: 0.25,
                size: 0.10,
                color: Vec3::new(0.50, 0.48, 0.45),
                gravity: 0.0,
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

#[cfg(test)]
mod feel_tests {
    use super::*;
    use crate::sound::{BUDGET, prioritize};
    use arena_core::shooter::{SHOT_BODY, SHOT_EXPIRED, SHOT_SHIELD, WEAPON_COUNT};

    #[test]
    fn a_tracer_head_travels_at_the_weapons_speed() {
        for id in 1..=WEAPON_COUNT {
            let s = weapon_stats(id);
            let t = Tracer {
                from: Vec3::new(1.0, 1.45, -2.0),
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
        assert_eq!(plume(at, Vec3::Z).len(), 4);
        assert!(
            plume(at, Vec3::Z)
                .iter()
                .all(|p| p.vel.z > 1.0 && p.vel.y > 0.0)
        );
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
