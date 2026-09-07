//! Champion combat identity, composed from the existing nine scene meshes.
//!
//! Call `draw_projectile` for each authoritative projectile and `draw_fx`
//! before the legacy FX renderer. The latter returns false for generic events
//! the old renderer still owns. FX age is normalized 0..1; projectile time is
//! match seconds. All animation is presentation-only and has no mutable RNG.
//!
//! Large effects use open outlines, thin blades and sparse alpha sparks. They
//! never put an opaque sphere, cylinder or filled disc over a target. The tests
//! bound draw cost and compare geometry without colors, so identity cannot be
//! reduced to five recolors of the same attack.

#![allow(
    clippy::suboptimal_flops,
    reason = "Analytic effect formulas stay readable; finite-transform fixtures guard their output"
)]

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::{Frame, Instance, Particle};
use league_core::proto::ProjSnap;

use crate::scene::{
    MESH_BLADE, MESH_CONE, MESH_FRUSTUM, MESH_GEAR, MESH_OCTA, MESH_RING, MESH_SPHERE,
};
use crate::world::{FxLite, UnitLite};

const CYAN: Vec3 = Vec3::new(0.22, 1.0, 1.3);
const BRONZE: Vec3 = Vec3::new(0.58, 0.29, 0.10);
const FLAME: Vec3 = Vec3::new(1.3, 0.34, 0.035);
const HOT: Vec3 = Vec3::new(1.3, 0.92, 0.28);
const IVORY: Vec3 = Vec3::new(1.0, 0.93, 0.65);
const MINT: Vec3 = Vec3::new(0.52, 1.0, 0.78);
const SLUDGE: Vec3 = Vec3::new(0.32, 0.56, 0.11);
const IRON: Vec3 = Vec3::new(0.31, 0.25, 0.19);
const VIOLET: Vec3 = Vec3::new(0.7, 0.42, 1.2);
const SILVER: Vec3 = Vec3::new(0.76, 0.89, 1.0);

fn put(frame: &mut Frame, mesh: u32, pos: Vec3, scale: Vec3, color: Vec3, rot: Quat) {
    if pos.is_finite() && scale.is_finite() && color.is_finite() && rot.is_finite() {
        frame.instances.push(
            Instance::new(pos, scale.abs().max(Vec3::splat(0.001)), color)
                .with_mesh(mesh)
                .with_rot(rot)
                .without_shadow(),
        );
    }
}

fn rod(frame: &mut Frame, from: Vec3, to: Vec3, radius: f32, color: Vec3) {
    let delta = to - from;
    let length = delta.length();
    if length > 0.001 && length.is_finite() {
        put(
            frame,
            MESH_FRUSTUM,
            from + delta * 0.5,
            Vec3::new(radius, length * 0.5, radius),
            color,
            Quat::from_rotation_arc(Vec3::Y, delta / length),
        );
    }
}

fn ring(frame: &mut Frame, pos: Vec3, radius: f32, color: Vec3, rotation: Quat) {
    put(
        frame,
        MESH_RING,
        pos,
        Vec3::new(radius, 1.0, radius),
        color,
        rotation,
    );
}

fn polar(center: Vec3, radius: f32, angle: f32) -> Vec3 {
    center + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius)
}

fn arc(frame: &mut Frame, center: Vec3, radius: f32, angles: [f32; 2], color: Vec3, width: f32) {
    for index in 0..10u8 {
        let start = angles[0] + (angles[1] - angles[0]) * f32::from(index) / 10.0;
        let end = angles[0] + (angles[1] - angles[0]) * f32::from(index + 1) / 10.0;
        rod(
            frame,
            polar(center, radius, start),
            polar(center, radius, end),
            width,
            color,
        );
    }
}

fn blade(frame: &mut Frame, pos: Vec3, yaw: f32, length: f32, width: f32, color: Vec3) {
    put(
        frame,
        MESH_BLADE,
        pos,
        Vec3::new(length, 0.075, width),
        color,
        Quat::from_rotation_y(-yaw),
    );
}

fn sparks(frame: &mut Frame, center: Vec3, color: Vec3, age: f32, count: u8) {
    for index in 0..count {
        let angle = TAU * f32::from(index) / f32::from(count) + age * 2.0;
        let radius = 0.12 + age * (0.7 + f32::from(index % 3) * 0.18);
        let pos = polar(center, radius, angle) + Vec3::Y * (age * 1.2 - age * age * 0.7);
        frame.particles.push(Particle {
            position: pos,
            color,
            size: Vec2::splat(0.24 * (1.0 - age) + 0.06),
            opacity: 0.7 * (1.0 - age),
        });
    }
}

fn drone(frame: &mut Frame, pos: Vec3, yaw: f32, phase: f32, size: f32) {
    put(
        frame,
        MESH_OCTA,
        pos,
        Vec3::new(0.40, 0.18, 0.24) * size,
        BRONZE,
        Quat::from_rotation_y(-yaw),
    );
    let wing_axis = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
    for sign in [-1.0, 1.0] {
        let rotor = pos + wing_axis * (0.32 * sign * size);
        ring(frame, rotor, 0.19 * size, BRONZE, Quat::IDENTITY);
        blade(
            frame,
            rotor + Vec3::Y * 0.02,
            phase * 11.0,
            0.24 * size,
            0.12 * size,
            CYAN,
        );
    }
    let front = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    rod(
        frame,
        pos + front * (0.16 * size),
        pos + front * (0.49 * size),
        0.055 * size,
        CYAN,
    );
}

fn petals(frame: &mut Frame, center: Vec3, radius: f32, phase: f32, count: u8, color: Vec3) {
    for index in 0..count {
        let angle = TAU * f32::from(index) / f32::from(count) + phase;
        blade(
            frame,
            polar(center, radius, angle),
            angle + 0.35,
            radius * 0.75,
            0.32,
            color,
        );
    }
}

fn diamond(frame: &mut Frame, center: Vec3, radius: f32, yaw: f32, color: Vec3) {
    let side = Vec3::new(-yaw.sin(), 0.0, yaw.cos()) * radius;
    let up = Vec3::Y * radius;
    let points = [center + up, center + side, center - up, center - side];
    for index in 0..4 {
        rod(frame, points[index], points[(index + 1) % 4], 0.045, color);
    }
}

fn clock(frame: &mut Frame, center: Vec3, radius: f32, phase: f32) {
    ring(frame, center, radius, VIOLET, Quat::IDENTITY);
    put(
        frame,
        MESH_GEAR,
        center,
        Vec3::new(radius * 0.46, 0.16, radius * 0.46),
        BRONZE,
        Quat::from_rotation_y(phase * 2.0),
    );
    blade(
        frame,
        center + Vec3::Y * 0.08,
        phase,
        radius * 0.82,
        0.13,
        SILVER,
    );
    blade(
        frame,
        center + Vec3::Y * 0.12,
        -phase * 0.6,
        radius * 0.57,
        0.2,
        IVORY,
    );
}

fn hook(frame: &mut Frame, center: Vec3, yaw: f32, phase: f32) {
    let dir = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    for index in 0..5u8 {
        let link = center - dir * (0.34 + f32::from(index) * 0.31);
        ring(
            frame,
            link,
            0.18,
            IRON,
            Quat::from_rotation_x(if index % 2 == 0 { FRAC_PI_2 } else { 0.0 }),
        );
    }
    blade(frame, center, yaw, 0.75, 0.48, IRON);
    blade(frame, center + dir * 0.48, yaw + 2.1, 0.43, 0.35, IVORY);
    put(
        frame,
        MESH_SPHERE,
        center - dir * 0.1 + Vec3::Y * (phase.sin() * 0.05),
        Vec3::new(0.16, 0.10, 0.16),
        SLUDGE,
        Quat::IDENTITY,
    );
}

/// Draw a shot at its authoritative position, with only local trail animation.
///
/// No interpolation moves a projectile ahead of the snapshot. Unknown owners
/// retain a compact team-colored dart; minion shots are never misidentified.
pub fn draw_projectile(frame: &mut Frame, projectile: &ProjSnap, time: f32) {
    if ![projectile.x, projectile.z, projectile.dx, projectile.dz]
        .iter()
        .all(|v| v.is_finite())
    {
        return;
    }
    let phase = if time.is_finite() {
        time.rem_euclid(120.0)
    } else {
        0.0
    };
    let yaw = projectile.dz.atan2(projectile.dx);
    let pos = Vec3::new(projectile.x, 1.05, projectile.z);
    let dir = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    if projectile.k == 3 {
        hook(frame, pos, yaw, phase * 9.0);
        return;
    }
    match projectile.champ {
        0 => {
            drone(
                frame,
                pos,
                yaw,
                phase,
                if projectile.k == 1 { 0.9 } else { 0.68 },
            );
            rod(
                frame,
                pos - dir * 0.3,
                pos - dir * (0.85 + 0.12 * (phase * 22.0).sin()),
                0.06,
                CYAN,
            );
        }
        1 => {
            blade(frame, pos - dir * 0.3, yaw, 0.8, 0.27, HOT);
            for index in 0..3u8 {
                let back = pos - dir * (0.25 + f32::from(index) * 0.25);
                put(
                    frame,
                    MESH_OCTA,
                    back + Vec3::Y * (0.12 * (phase * 18.0 + f32::from(index)).sin()),
                    Vec3::splat(0.16 - f32::from(index) * 0.03),
                    FLAME,
                    Quat::from_rotation_y(phase * 7.0),
                );
            }
        }
        2 => {
            petals(frame, pos, 0.18, phase * 10.0, 3, IVORY);
            diamond(frame, pos, 0.24, yaw + phase * 3.0, MINT);
            rod(frame, pos - dir * 0.25, pos - dir * 0.65, 0.035, MINT);
        }
        3 => {
            for offset in [-0.22, 0.0, 0.22] {
                let side = Vec3::new(-yaw.sin(), 0.0, yaw.cos()) * offset;
                blade(
                    frame,
                    pos + side,
                    yaw + 0.15 * (phase * 5.0).sin(),
                    0.58,
                    0.18,
                    IVORY,
                );
            }
            put(
                frame,
                MESH_SPHERE,
                pos - dir * 0.2,
                Vec3::new(0.29, 0.15, 0.24),
                SLUDGE,
                Quat::IDENTITY,
            );
        }
        4 => {
            clock(
                frame,
                pos,
                if projectile.k == 2 { 0.42 } else { 0.29 },
                phase * 12.0,
            );
            for index in 1..=2u8 {
                let back = pos - dir * (f32::from(index) * 0.33);
                put(
                    frame,
                    MESH_OCTA,
                    back,
                    Vec3::splat(0.09),
                    VIOLET,
                    Quat::from_rotation_y(-phase * 8.0),
                );
            }
        }
        _ => {
            let color = if projectile.t == 0 { SILVER } else { FLAME };
            blade(frame, pos - dir * 0.15, yaw, 0.42, 0.16, color);
        }
    }
}

struct Cast {
    origin: Vec3,
    target: Vec3,
    yaw: f32,
    age: f32,
    pulse: f32,
}

impl Cast {
    fn new(fx: &FxLite, age: f32) -> Self {
        let origin = Vec3::new(fx.x, 0.18, fx.z);
        let target = Vec3::new(fx.x2, 0.18, fx.z2);
        Self {
            origin,
            target,
            yaw: (target.z - origin.z).atan2(target.x - origin.x),
            age,
            pulse: (PI * age).sin().max(0.0),
        }
    }

    fn forward(&self, distance: f32) -> Vec3 {
        self.origin + Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin()) * distance
    }
}

fn swarm_cast(frame: &mut Frame, cast: &Cast, slot: u8) {
    let center = cast.origin + Vec3::Y * 0.85;
    match slot {
        0 => {
            for index in 0..3u8 {
                let angle = cast.yaw + (f32::from(index) - 1.0) * 0.7;
                let pos = polar(center, 0.55 + cast.age * 1.4, angle);
                drone(frame, pos, angle, cast.age * 8.0, 0.8);
            }
            arc(
                frame,
                cast.origin,
                1.2 + cast.age * 0.4,
                [cast.yaw - 1.1, cast.yaw + 1.1],
                CYAN,
                0.035,
            );
        }
        1 => {
            // Three coaxial lenses focus at the emitter; the real k1 event
            // supplies the full laser's authoritative endpoints.
            let dir = Vec3::new(cast.yaw.cos(), 0.0, cast.yaw.sin());
            let rotation = Quat::from_rotation_arc(Vec3::Y, dir);
            for index in 0..3u8 {
                let pos = center + dir * (0.45 + f32::from(index) * 0.45 + cast.age * 0.25);
                ring(frame, pos, 0.45 - f32::from(index) * 0.09, CYAN, rotation);
            }
            rod(
                frame,
                center,
                center + dir * 1.8,
                0.045 + cast.pulse * 0.06,
                SILVER,
            );
        }
        2 => {
            for sign in [-1.0, 1.0] {
                let side = Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos());
                let pos = center + side * (sign * (0.55 + cast.age));
                diamond(frame, pos, 0.75, cast.yaw, CYAN * (0.6 + cast.pulse * 0.4));
                ring(frame, pos - Vec3::Y * 0.6, 0.47, BRONZE, Quat::IDENTITY);
                rod(
                    frame,
                    pos - Vec3::Y * 0.5,
                    pos + Vec3::Y * (0.1 + cast.age),
                    0.035,
                    CYAN,
                );
            }
        }
        3 => {
            for index in 0..4u8 {
                let angle = TAU * f32::from(index) / 4.0 + cast.age * 0.8;
                let pos = polar(center, 1.25 + cast.pulse * 0.25, angle);
                drone(frame, pos, angle + PI, cast.age * 9.0, 0.7);
                rod(frame, pos, center + Vec3::Y * 0.5, 0.045, CYAN);
            }
            put(
                frame,
                MESH_OCTA,
                center + Vec3::Y * 0.5,
                Vec3::splat(0.25 + cast.pulse * 0.12),
                SILVER,
                Quat::from_rotation_y(cast.age * 5.0),
            );
        }
        _ => {}
    }
}

fn flame_sweep(frame: &mut Frame, center: Vec3, yaw: f32, age: f32, radius: f32) {
    let swing = yaw - 1.35 + age * 2.7;
    blade(frame, center, swing, radius, 0.38 * (1.0 - age) + 0.12, HOT);
    arc(
        frame,
        center,
        radius * 0.84,
        [swing - 0.75, swing],
        FLAME,
        0.06 * (1.0 - age) + 0.02,
    );
    for index in 0..3u8 {
        let pos = polar(center, radius * (0.45 + f32::from(index) * 0.2), swing);
        put(
            frame,
            MESH_CONE,
            pos + Vec3::Y * (0.15 + age * 0.2),
            Vec3::new(0.10, 0.22 + 0.08 * f32::from(index), 0.10),
            FLAME,
            Quat::IDENTITY,
        );
    }
}

fn knight_cast(frame: &mut Frame, cast: &Cast, slot: u8) {
    match slot {
        0 => {
            // Open rising corkscrew; no filled funnel hides the caster.
            for index in 0..3u8 {
                let level = f32::from(index);
                arc(
                    frame,
                    cast.origin + Vec3::Y * (0.25 + level * 0.55),
                    0.45 + level * 0.30,
                    [cast.age * 8.0 + level, cast.age * 8.0 + level + PI * 1.4],
                    if index == 1 { HOT } else { FLAME },
                    0.045,
                );
            }
            sparks(frame, cast.origin + Vec3::Y, HOT, cast.age, 5);
        }
        1 => {
            let center = cast.forward(0.8) + Vec3::Y * 1.0;
            diamond(frame, center, 1.1 + cast.pulse * 0.1, cast.yaw, HOT);
            let side = Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos());
            for sign in [-1.0, 1.0] {
                let foot = center + side * sign * 0.65 - Vec3::Y * 0.35;
                rod(
                    frame,
                    foot,
                    center + side * sign * 0.55 + Vec3::Y * (0.7 + cast.pulse * 0.15),
                    0.10,
                    FLAME,
                );
            }
            blade(frame, center, cast.yaw, 0.42, 0.38, HOT);
        }
        2 => {
            flame_sweep(
                frame,
                cast.origin + Vec3::Y * 1.25,
                cast.yaw,
                cast.age,
                2.25,
            );
            // Crossguard announces an enchanted weapon, unlike the auto arc.
            let center = cast.origin + Vec3::Y * 1.25;
            let side = Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos());
            rod(
                frame,
                center - side * 0.36,
                center + side * 0.36,
                0.08,
                BRONZE,
            );
        }
        3 => {
            for sign in [-1.0, 1.0] {
                let side = Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos()) * sign;
                let shoulder = cast.origin + Vec3::Y * 1.4 + side * 0.35;
                let wing = shoulder + side * (1.0 + cast.pulse * 0.4) + Vec3::Y * 0.9;
                let tip = wing + side * 0.5 - Vec3::Y * 0.55;
                rod(frame, shoulder, wing, 0.095, FLAME);
                rod(frame, wing, tip, 0.07, HOT);
                rod(
                    frame,
                    wing,
                    shoulder + side * 0.65 - Vec3::Y * 0.45,
                    0.055,
                    FLAME,
                );
                put(
                    frame,
                    MESH_CONE,
                    shoulder + Vec3::Y * 0.9,
                    Vec3::new(0.16, 0.45, 0.16),
                    HOT,
                    Quat::from_rotation_z(sign * 0.45),
                );
            }
            arc(
                frame,
                cast.origin,
                1.3 + cast.age * 0.6,
                [0.0, TAU],
                FLAME,
                0.05,
            );
        }
        _ => {}
    }
}

fn hallow_cast(frame: &mut Frame, cast: &Cast, slot: u8) {
    let center = cast.origin + Vec3::Y * 0.55;
    match slot {
        0 => {
            petals(
                frame,
                cast.origin + Vec3::Y * (2.35 + cast.age * 0.7),
                0.85 + cast.pulse * 0.2,
                cast.age,
                6,
                IVORY,
            );
            rod(
                frame,
                cast.origin + Vec3::Y * 2.35 - Vec3::X * 0.44,
                cast.origin + Vec3::Y * 2.35 + Vec3::X * 0.44,
                0.07,
                MINT,
            );
            rod(
                frame,
                cast.origin + Vec3::Y * 2.35 - Vec3::Z * 0.44,
                cast.origin + Vec3::Y * 2.35 + Vec3::Z * 0.44,
                0.07,
                MINT,
            );
            sparks(frame, cast.origin + Vec3::Y * 2.35, MINT, cast.age, 4);
        }
        1 => {
            let side = Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos());
            for index in 0..3u8 {
                let tip =
                    cast.forward(1.25 + f32::from(index) * 0.62 + cast.age * 0.7) + Vec3::Y * 0.18;
                let back = Vec3::new(cast.yaw.cos(), 0.0, cast.yaw.sin()) * 0.42;
                rod(frame, tip - back + side * 0.38, tip, 0.065, MINT);
                rod(frame, tip - back - side * 0.38, tip, 0.065, IVORY);
            }
            petals(frame, center, 0.3, -cast.age * 3.0, 2, IVORY);
        }
        2 => {
            // Two perpendicular open diamonds create a woven ward, never a
            // solid bubble. The shrinking waist gives the lattice its snap.
            diamond(
                frame,
                center + Vec3::Y * 1.25,
                1.30 + cast.pulse * 0.2,
                cast.yaw,
                MINT,
            );
            diamond(
                frame,
                center + Vec3::Y * 1.25,
                1.30 + cast.pulse * 0.2,
                cast.yaw + FRAC_PI_2,
                IVORY,
            );
            ring(
                frame,
                center + Vec3::Y * (1.0 + cast.age * 0.8),
                1.02 - cast.age * 0.18,
                IVORY,
                Quat::IDENTITY,
            );
        }
        3 => {
            let crown = cast.origin + Vec3::Y * (1.7 + cast.age * 0.5);
            ring(frame, crown, 0.70 + cast.pulse * 0.2, IVORY, Quat::IDENTITY);
            petals(frame, crown, 0.65, -cast.age * 1.8, 4, IVORY);
            for sign in [-1.0, 1.0] {
                let foot = cast.origin + Vec3::X * sign * 0.75;
                rod(frame, foot, crown + Vec3::X * sign * 0.55, 0.04, MINT);
            }
            diamond(frame, crown + Vec3::Y * 0.3, 0.28, cast.yaw, SILVER);
        }
        _ => {}
    }
}

fn claw_strike(frame: &mut Frame, center: Vec3, yaw: f32, age: f32) {
    let side = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
    let dir = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    for index in 0..3u8 {
        let from = center + side * ((f32::from(index) - 1.0) * 0.35) - dir * (age * 0.6);
        blade(
            frame,
            from,
            yaw - 0.18 + age * 0.55,
            1.25 + 0.2 * f32::from(index),
            0.25,
            IVORY,
        );
        rod(
            frame,
            from - Vec3::Y * 0.12,
            from + dir * (0.7 + age),
            0.05,
            SLUDGE,
        );
    }
}

fn maw_cast(frame: &mut Frame, cast: &Cast, slot: u8) {
    match slot {
        0 => {
            hook(
                frame,
                cast.forward(0.75 + cast.age) + Vec3::Y * 0.7,
                cast.yaw,
                cast.age * 8.0,
            );
            claw_strike(frame, cast.origin + Vec3::Y * 0.7, cast.yaw, cast.age * 0.4);
        }
        1 => {
            // Three interrupted sludge pools orbit an open, legible center.
            for index in 0..3u8 {
                let angle = TAU * f32::from(index) / 3.0 + cast.age;
                let pool = polar(cast.origin, 0.95 + cast.pulse * 0.4, angle);
                ring(frame, pool, 0.45, SLUDGE, Quat::IDENTITY);
                put(
                    frame,
                    MESH_SPHERE,
                    pool + Vec3::Y * (0.18 + cast.age * 0.3),
                    Vec3::new(0.22, 0.12, 0.22),
                    SLUDGE * 0.65,
                    Quat::IDENTITY,
                );
            }
            sparks(frame, cast.origin + Vec3::Y * 0.4, SLUDGE, cast.age, 6);
        }
        2 => {
            for index in 0..4u8 {
                let step = f32::from(index);
                let point = cast.forward(1.2 + step * 0.5 + cast.age * 0.7);
                put(
                    frame,
                    MESH_OCTA,
                    point + Vec3::Y * (0.15 + cast.pulse * (0.3 + step * 0.1)),
                    Vec3::new(0.22, 0.3, 0.16),
                    IRON,
                    Quat::from_rotation_y(cast.yaw + step),
                );
                blade(frame, point, cast.yaw + PI, 0.45, 0.40, SLUDGE);
            }
        }
        3 => {
            // Six angular fault lines split outward; each ends in a raised
            // tooth instead of a solid ground disc or a generic shock ring.
            for index in 0..6u8 {
                let angle = TAU * f32::from(index) / 6.0;
                let bend = polar(cast.origin, 0.7 + cast.age, angle + 0.2);
                let tip = polar(cast.origin, 1.45 + cast.age * 1.8, angle);
                rod(frame, polar(cast.origin, 0.4, angle), bend, 0.065, SLUDGE);
                rod(frame, bend, tip, 0.065, IVORY * 0.7);
                put(
                    frame,
                    MESH_CONE,
                    tip + Vec3::Y * (0.12 + cast.pulse * 0.4),
                    Vec3::new(0.22, 0.15 + cast.pulse * 0.5, 0.22),
                    IRON,
                    Quat::from_rotation_z(0.25 * angle.sin()),
                );
            }
        }
        _ => {}
    }
}

fn tessera_cast(frame: &mut Frame, cast: &Cast, slot: u8) {
    match slot {
        0 => {
            let center = cast.forward(0.8) + Vec3::Y * 0.75;
            clock(frame, center, 0.7 + cast.pulse * 0.1, cast.age * 8.0);
            for index in 0..3u8 {
                let angle = TAU * f32::from(index) / 3.0 - cast.age * 4.0;
                put(
                    frame,
                    MESH_GEAR,
                    polar(center, 0.95, angle),
                    Vec3::new(0.19, 0.15, 0.19),
                    BRONZE,
                    Quat::from_rotation_y(-cast.age * 11.0),
                );
            }
        }
        1 => {
            let center = cast.origin + Vec3::Y * 0.08;
            for index in 0..4u8 {
                let angle = TAU * f32::from(index) / 4.0 + PI * 0.25;
                let tooth = polar(center, 0.7 + 0.3 * (1.0 - cast.age), angle);
                blade(frame, tooth, angle + PI, 0.58, 0.34, VIOLET);
                put(
                    frame,
                    MESH_GEAR,
                    tooth + Vec3::Y * 0.12,
                    Vec3::new(0.19, 0.11, 0.19),
                    BRONZE,
                    Quat::from_rotation_y(cast.age * 4.0),
                );
            }
            diamond(frame, center + Vec3::Y * 0.5, 0.38, cast.yaw, SILVER);
        }
        2 => {
            for index in 0..3u8 {
                let center = cast.forward(f32::from(index) * 0.8 + cast.age * 0.4) + Vec3::Y * 0.5;
                diamond(
                    frame,
                    center,
                    0.75 - f32::from(index) * 0.12,
                    cast.yaw,
                    VIOLET,
                );
                blade(frame, center, cast.yaw - cast.age * 6.0, 0.48, 0.14, SILVER);
            }
            clock(frame, cast.origin, 0.42, -cast.age * 9.0);
        }
        3 => {
            let center = cast.origin + Vec3::Y * 0.12;
            clock(frame, center, 1.55 + cast.pulse * 0.35, cast.age * 3.0);
            for index in 0..8u8 {
                let angle = TAU * f32::from(index) / 8.0;
                let from = polar(center, 1.4 + cast.pulse * 0.2, angle);
                let to = polar(center, 1.68 + cast.pulse * 0.2, angle);
                rod(
                    frame,
                    from,
                    to + Vec3::Y * (0.18 + cast.age * 0.3),
                    0.065,
                    IVORY,
                );
            }
            ring(
                frame,
                center + Vec3::Y * (1.8 - cast.age),
                0.62,
                VIOLET,
                Quat::from_rotation_x(FRAC_PI_2),
            );
        }
        _ => {}
    }
}

fn cast_effect(frame: &mut Frame, cast: &Cast, champ: u8, ability: u8) {
    match champ {
        0 => swarm_cast(frame, cast, ability),
        1 => knight_cast(frame, cast, ability),
        2 => hallow_cast(frame, cast, ability),
        3 => maw_cast(frame, cast, ability),
        4 => tessera_cast(frame, cast, ability),
        _ => {}
    }
}

fn strike(frame: &mut Frame, fx: &FxLite, cast: &Cast) {
    let start = fx.v.rem_euclid(8.0) >= 4.0;
    let pos = if start && matches!(fx.champ, 2 | 3) {
        cast.forward(1.15) + Vec3::Y * 1.65
    } else {
        cast.origin + Vec3::Y * 0.9
    };
    match fx.champ {
        0 => {
            for index in 0..3u8 {
                let angle = TAU * f32::from(index) / 3.0 + cast.age * 4.0;
                let start = polar(pos, 0.15 + cast.age * 0.45, angle);
                rod(
                    frame,
                    start,
                    polar(pos, 0.5 + cast.age * 0.6, angle),
                    0.045,
                    CYAN,
                );
            }
            ring(
                frame,
                pos,
                0.22 + cast.age * 0.45,
                BRONZE,
                Quat::from_rotation_x(FRAC_PI_2),
            );
        }
        1 => flame_sweep(
            frame,
            pos,
            cast.yaw,
            cast.age,
            if fx.v.rem_euclid(2.0) >= 1.0 {
                2.1
            } else {
                1.65
            },
        ),
        2 => {
            petals(frame, pos, 0.58 + cast.age * 0.7, -cast.age * 2.0, 4, IVORY);
            diamond(frame, pos, 0.42 + cast.pulse * 0.3, cast.yaw, MINT);
        }
        3 => claw_strike(frame, pos, cast.yaw, cast.age),
        4 => {
            clock(frame, pos, 0.3 + cast.age * 0.55, cast.age * 9.0);
            for index in 0..3u8 {
                let angle = TAU * f32::from(index) / 3.0 + cast.age;
                put(
                    frame,
                    MESH_GEAR,
                    polar(pos, 0.45 + cast.age * 0.6, angle),
                    Vec3::new(0.13, 0.10, 0.13),
                    VIOLET,
                    Quat::from_rotation_y(-cast.age * 12.0),
                );
            }
        }
        _ => {}
    }
    let color = [CYAN, HOT, IVORY, SLUDGE, VIOLET][usize::from(fx.champ)];
    sparks(frame, pos, color, cast.age, 3);
}

fn teleport(frame: &mut Frame, fx: &FxLite, cast: &Cast) {
    for (index, point) in [cast.origin, cast.target].into_iter().enumerate() {
        let age = if index == 0 { cast.age } else { 1.0 - cast.age };
        match fx.champ {
            0 => {
                diamond(
                    frame,
                    point + Vec3::Y * 0.8,
                    0.65 + age * 0.5,
                    cast.yaw,
                    CYAN,
                );
                ring(frame, point, 0.55 + age * 0.3, BRONZE, Quat::IDENTITY);
            }
            1 => {
                for sign in [-1.0, 1.0] {
                    let pos = point + Vec3::new(-cast.yaw.sin(), 0.0, cast.yaw.cos()) * sign * 0.45;
                    blade(
                        frame,
                        pos + Vec3::Y * (0.2 + age),
                        cast.yaw + sign * 0.5,
                        0.9,
                        0.3,
                        FLAME,
                    );
                }
                sparks(frame, point + Vec3::Y * 0.6, HOT, cast.age, 4);
            }
            3 => {
                petals(
                    frame,
                    point + Vec3::Y * 0.1,
                    1.25 + age * 0.5,
                    cast.yaw,
                    5,
                    SLUDGE,
                );
                arc(
                    frame,
                    point + Vec3::Y * 0.12,
                    1.3 + age * 0.5,
                    [cast.yaw - PI * 0.7, cast.yaw + PI * 0.7],
                    SLUDGE,
                    0.075,
                );
                put(
                    frame,
                    MESH_OCTA,
                    point + Vec3::Y * (0.2 + age * 0.3),
                    Vec3::new(0.20, 0.30, 0.20),
                    IRON,
                    Quat::from_rotation_y(cast.age * 7.0),
                );
            }
            4 => clock(frame, point, 0.48 + age * 0.65, age * 6.0),
            _ => diamond(
                frame,
                point + Vec3::Y * 0.7,
                0.6 + age * 0.4,
                cast.yaw,
                IVORY,
            ),
        }
    }
}

/// Draw attributed champion FX; return false for the legacy generic fallback.
///
/// `age` is normalized across the effect's lifetime. Malformed attributed
/// coordinates are consumed without drawing, preventing invalid transforms
/// from reaching either this renderer or the legacy fallback.
pub fn draw_fx(frame: &mut Frame, fx: &FxLite, age: f32) -> bool {
    if fx.champ > 4 {
        return false;
    }
    if ![fx.x, fx.z, fx.x2, fx.z2, age]
        .iter()
        .all(|value| value.is_finite())
    {
        return true;
    }
    let age = age.clamp(0.0, 1.0);
    if age >= 1.0 {
        return true;
    }
    let cast = Cast::new(fx, age);
    match fx.k {
        13 if fx.ability < 4 => cast_effect(frame, &cast, fx.champ, fx.ability),
        0 => strike(frame, fx, &cast),
        1 if fx.champ == 0 => {
            // One coherent cyan beam with a thin white core. Its endpoints
            // come from the server, including each clone's separate laser.
            let start = cast.origin + Vec3::Y * 0.87;
            let end = cast.target + Vec3::Y * 0.87;
            rod(frame, start, end, 0.08 + 0.07 * (1.0 - age), CYAN);
            rod(
                frame,
                start + Vec3::Y * 0.015,
                end + Vec3::Y * 0.015,
                0.035,
                SILVER,
            );
            sparks(frame, end, CYAN, age, 4);
        }
        8 => teleport(frame, fx, &cast),
        9 | 12 if fx.champ == 2 && fx.ability < 4 => {
            hallow_cast(frame, &cast, fx.ability);
        }
        10 if fx.champ == 3 => {
            let delta = cast.target - cast.origin;
            let length = delta.length().min(14.0);
            let dir = delta.normalize_or_zero();
            for index in 0..10u8 {
                let pos = cast.origin + dir * (length * f32::from(index) / 10.0) + Vec3::Y * 0.75;
                ring(
                    frame,
                    pos,
                    0.16,
                    IRON,
                    Quat::from_rotation_x(if index % 2 == 0 { FRAC_PI_2 } else { 0.0 }),
                );
            }
            hook(
                frame,
                cast.origin + dir * length + Vec3::Y * 0.75,
                cast.yaw,
                age * 8.0,
            );
        }
        _ => return false,
    }
    true
}

/// A small model transform driven by an actual authoritative attack start.
///
/// Add `offset` in world space; multiply the model's local rotation by
/// `rotation` and its scale by `stretch`. Collider, selection and health-bar
/// positions stay authoritative. No skeleton or gameplay timing is changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackPose {
    pub offset: Vec3,
    pub rotation: Quat,
    pub stretch: Vec3,
}

impl Default for AttackPose {
    fn default() -> Self {
        Self {
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            stretch: Vec3::ONE,
        }
    }
}

fn attack_start(fx: &FxLite) -> bool {
    // v's bit 2 distinguishes launches from point impacts, even when a real
    // target is at the world origin. Bits 0/1 remain crit and spell flags.
    fx.k == 0 && fx.ability == 4 && fx.v.is_finite() && fx.v.rem_euclid(8.0) >= 4.0
}

/// Recoil, swing or lunge for the latest nearby start belonging to this actor.
pub fn attack_pose(unit: &UnitLite, effects: &[FxLite]) -> AttackPose {
    if unit.dead || unit.def > 4 || ![unit.x, unit.z, unit.fa].iter().all(|v| v.is_finite()) {
        return AttackPose::default();
    }
    let belongs = |fx: &&FxLite| {
        fx.champ == unit.def
            && fx.life.is_finite()
            && fx.life > 0.0
            && fx.left.is_finite()
            && fx.left > 0.0
            && [fx.x, fx.z, fx.x2, fx.z2].iter().all(|v| v.is_finite())
            && (fx.x - unit.x).powi(2) + (fx.z - unit.z).powi(2) < 0.8 * 0.8
    };
    // A cast owns its short visual window, including a cooldown-ready auto
    // released later in the same simulation tick. Damage and shots survive.
    if effects
        .iter()
        .filter(belongs)
        .any(|fx| fx.k == 13 && fx.ability < 4)
    {
        return AttackPose::default();
    }
    let Some(fx) = effects
        .iter()
        .rev()
        .filter(belongs)
        .find(|fx| attack_start(fx))
    else {
        return AttackPose::default();
    };
    let age = (1.0 - fx.left / fx.life).clamp(0.0, 1.0);
    let pulse = (PI * age).sin();
    let yaw = (fx.z2 - fx.z).atan2(fx.x2 - fx.x);
    let dir = Vec3::new(yaw.cos(), 0.0, yaw.sin());
    match unit.def {
        0 => AttackPose {
            offset: -dir * (0.16 * pulse) + Vec3::Y * (0.08 * pulse),
            rotation: Quat::from_rotation_z(0.08 * pulse),
            stretch: Vec3::ONE,
        },
        1 => AttackPose {
            offset: dir * (0.30 * pulse),
            rotation: Quat::from_rotation_y(-0.28 * pulse) * Quat::from_rotation_z(-0.14 * pulse),
            stretch: Vec3::new(1.0 + pulse * 0.06, 1.0, 1.0),
        },
        2 => AttackPose {
            offset: -dir * (0.08 * pulse) + Vec3::Y * (0.18 * pulse),
            rotation: Quat::from_rotation_y(0.16 * pulse),
            stretch: Vec3::ONE,
        },
        3 => AttackPose {
            offset: dir * (0.28 * pulse) - Vec3::Y * (0.12 * pulse),
            rotation: Quat::from_rotation_z(-0.12 * pulse),
            stretch: Vec3::new(1.0 + pulse * 0.08, 1.0 - pulse * 0.10, 1.0 + pulse * 0.05),
        },
        4 => AttackPose {
            offset: -dir * (0.12 * pulse),
            rotation: Quat::from_rotation_y(-0.35 * pulse),
            stretch: Vec3::new(1.0, 1.0 + pulse * 0.05, 1.0),
        },
        _ => AttackPose::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use league_core::proto::UnitSnap;

    fn effect(champ: u8, ability: u8, kind: u8) -> FxLite {
        FxLite {
            k: kind,
            champ,
            ability,
            x: -4.0,
            z: 1.0,
            x2: 2.0,
            z2: 3.0,
            v: if kind == 0 { 4.0 } else { 0.0 },
            life: 0.45,
            left: 0.30,
        }
    }

    fn shot(champ: u8, kind: u8) -> ProjSnap {
        ProjSnap {
            id: 51,
            k: kind,
            t: 0,
            champ,
            x: -3.0,
            z: 2.0,
            dx: 0.8,
            dz: 0.6,
        }
    }

    fn fx_frame(champ: u8, ability: u8, kind: u8, age: f32) -> Frame {
        let mut frame = Frame::default();
        assert!(draw_fx(&mut frame, &effect(champ, ability, kind), age));
        frame
    }

    fn valid(frame: &Frame, max_instances: usize) {
        assert!(
            frame.instances.len() <= max_instances,
            "{} instances exceeds {max_instances}",
            frame.instances.len()
        );
        assert!(frame.particles.len() <= 8);
        for item in &frame.instances {
            assert!((1..=9).contains(&item.mesh));
            assert!(
                item.position.is_finite()
                    && item.scale.is_finite()
                    && item.color.is_finite()
                    && item.rot.is_finite()
            );
            assert!(item.scale.min_element() > 0.0);
            assert!((item.rot.length() - 1.0).abs() < 0.001);
            assert!(
                !item.casts_shadow,
                "transient lights should not cast opaque shadows"
            );
        }
        for particle in &frame.particles {
            assert!(
                particle.position.is_finite()
                    && particle.color.is_finite()
                    && particle.size.is_finite()
            );
            assert!(particle.size.min_element() > 0.0);
            assert!((0.0..=1.0).contains(&particle.opacity));
        }
    }

    // Geometry-only signature: excludes color so recoloring an identical
    // effect cannot satisfy the champion/ability identity contract.
    fn geometry(frame: &Frame) -> Vec<u32> {
        let mut signature = Vec::new();
        for item in &frame.instances {
            signature.push(item.mesh);
            signature.extend(item.position.to_array().map(f32::to_bits));
            signature.extend(item.scale.to_array().map(f32::to_bits));
            signature.extend(item.rot.to_array().map(f32::to_bits));
        }
        signature
    }

    #[test]
    fn twenty_casts_have_distinct_geometry_and_animate_with_bounded_cost() {
        let mut signatures = Vec::new();
        for champ in 0..5 {
            for ability in 0..4 {
                let early = fx_frame(champ, ability, 13, 0.17);
                let late = fx_frame(champ, ability, 13, 0.69);
                valid(&early, 40);
                valid(&late, 40);
                assert!(
                    !early.instances.is_empty(),
                    "champ {champ}, ability {ability}"
                );
                let signature = geometry(&early);
                assert_ne!(
                    signature,
                    geometry(&late),
                    "static cast for champ {champ}, ability {ability}"
                );
                assert!(
                    !signatures.contains(&signature),
                    "duplicate cast geometry for champ {champ}, ability {ability}"
                );
                signatures.push(signature);
            }
        }
        assert_eq!(signatures.len(), 20);
    }

    #[test]
    fn five_autos_have_distinct_launch_hit_and_projectile_geometry() {
        let mut strikes = Vec::new();
        let mut projectiles = Vec::new();
        for champ in 0..5 {
            let strike = fx_frame(champ, 4, 0, 0.23);
            valid(&strike, 20);
            let signature = geometry(&strike);
            assert!(
                !strikes.contains(&signature),
                "duplicate auto strike for champ {champ}"
            );
            strikes.push(signature);
            let mut moving = Frame::default();
            draw_projectile(&mut moving, &shot(champ, 0), 1.3);
            valid(&moving, 12);
            let mut later = Frame::default();
            draw_projectile(&mut later, &shot(champ, 0), 1.47);
            assert_ne!(
                geometry(&moving),
                geometry(&later),
                "static projectile for champ {champ}"
            );
            let signature = geometry(&moving);
            assert!(
                !projectiles.contains(&signature),
                "duplicate auto projectile for champ {champ}"
            );
            projectiles.push(signature);
        }
    }

    #[test]
    fn laser_uses_server_endpoints_and_never_an_opaque_target_shell() {
        let fx = effect(0, 1, 1);
        let frame = fx_frame(0, 1, 1, 0.3);
        assert_eq!(frame.instances.len(), 2);
        let start = Vec3::new(fx.x, 1.05, fx.z);
        let end = Vec3::new(fx.x2, 1.05, fx.z2);
        let beam = frame.instances[0];
        let half = beam.rot * Vec3::Y * beam.scale.y;
        assert!((beam.position - half - start).length() < 0.001);
        assert!((beam.position + half - end).length() < 0.001);
        assert!(beam.scale.x < 0.2 && beam.scale.z < 0.2);
    }

    #[test]
    fn extreme_ages_stationary_shots_and_generic_effects_stay_safe() {
        for age in [-3.0, 0.0, 0.999, 1.0, 4.0, f32::NAN] {
            for champ in 0..5 {
                for ability in 0..4 {
                    valid(&fx_frame(champ, ability, 13, age), 40);
                }
                for kind in [0, 8, 10] {
                    let mut frame = Frame::default();
                    draw_fx(&mut frame, &effect(champ, 4, kind), age);
                    valid(&frame, 40);
                }
            }
        }
        for kind in 0..4 {
            let mut stationary = shot(0, kind);
            stationary.dx = 0.0;
            stationary.dz = 0.0;
            let mut frame = Frame::default();
            draw_projectile(&mut frame, &stationary, f32::NAN);
            valid(&frame, 12);
        }
        let mut frame = Frame::default();
        assert!(!draw_fx(&mut frame, &effect(255, 255, 6), 0.2));
        assert!(!draw_fx(&mut frame, &effect(0, 255, 6), 0.2));
        let mut invalid = effect(0, 0, 13);
        invalid.x = f32::INFINITY;
        assert!(draw_fx(&mut frame, &invalid, 0.2));
        let mut invalid_shot = shot(0, 0);
        invalid_shot.x = f32::NAN;
        draw_projectile(&mut frame, &invalid_shot, 0.0);
        assert_eq!(frame.instances.len(), 0);
    }

    #[test]
    fn body_poses_follow_actual_starts_and_ignore_impacts_or_other_actors() {
        let mut poses = Vec::new();
        for champ in 0..5 {
            let mut world = World::new(1);
            world.set_units(&[UnitSnap {
                id: 1,
                k: 0,
                def: champ,
                x: -4.0,
                z: 1.0,
                ..UnitSnap::default()
            }]);
            let unit = world.units[0];
            let mut fx = effect(champ, 4, 0);
            // A valid target at (0,0) must still count as an attack launch.
            fx.x2 = 0.0;
            fx.z2 = 0.0;
            let pose = attack_pose(&unit, &[fx]);
            assert_ne!(pose, AttackPose::default());
            assert!(
                pose.offset.is_finite() && pose.rotation.is_finite() && pose.stretch.is_finite()
            );
            assert!(pose.offset.length() < 0.5 && pose.stretch.min_element() > 0.8);
            assert!(!poses.contains(&pose));
            poses.push(pose);
            fx.v = 0.0;
            assert_eq!(attack_pose(&unit, &[fx]), AttackPose::default());
            fx.v = 4.0;
            fx.x += 5.0;
            assert_eq!(attack_pose(&unit, &[fx]), AttackPose::default());
            fx.x = unit.x;
            fx.champ = (champ + 1) % 5;
            assert_eq!(attack_pose(&unit, &[fx]), AttackPose::default());
        }
    }

    #[test]
    fn accepted_abilities_own_recovery_window_including_same_tick_attacks() {
        for champ in 0..5 {
            let mut world = World::new(1);
            world.set_units(&[UnitSnap {
                id: 1,
                k: 0,
                def: champ,
                x: -4.0,
                z: 1.0,
                ..UnitSnap::default()
            }]);
            let unit = world.units[0];
            let attack = effect(champ, 4, 0);
            let original = attack_pose(&unit, &[attack]);
            assert_ne!(original, AttackPose::default());
            for ability in 0..4 {
                let mut cast = effect(champ, ability, 13);
                assert_eq!(attack_pose(&unit, &[attack, cast]), AttackPose::default());
                assert_eq!(attack_pose(&unit, &[cast, attack]), AttackPose::default());
                cast.x += 5.0;
                assert_eq!(attack_pose(&unit, &[attack, cast]), original);
                cast.x = unit.x;
                cast.left = 0.0;
                assert_eq!(attack_pose(&unit, &[attack, cast]), original);
            }
        }
    }
}
