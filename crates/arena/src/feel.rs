//! The feel pass (arena v18): what a gun does to the camera, the viewmodel,
//! the speakers and the pad, per weapon id, and what the events out of the
//! sim (a hit, a kill, a bonk, a blast) do to the same four channels.
//!
//! Everything here is cosmetic. Nothing in this module ever reaches the
//! wire: the recoil kick moves the camera and the model, never the pitch the
//! client sends, and `online.rs` pins that with `recoil_never_reaches_the_wire`.
//! The numbers are `docs/plans/arena-v18-freight-yard.md` section 6.3; every
//! one ships as written unless a test proves it wrong.

use arena_core::proto::color_for;
use arena_core::shooter::{HILL_CONTESTED, HILL_FREE, Hill};
use ember_engine::Rumble;
use ember_engine::glam::{Vec2, Vec3};

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
