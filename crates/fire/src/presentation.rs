//! Fire-only scene detail. All geometry uses the ordinary Ember scene pass.

use ember_engine::glam::{Quat, Vec2, Vec3};
use ember_engine::{Frame, Instance};
use fire_core::car::{self, Car};
use fire_core::sim::Race;

use super::Meshes;

fn part(
    frame: &mut Frame,
    origin: Vec3,
    rot: Quat,
    offset: Vec3,
    scale: Vec3,
    colour: Vec3,
    mesh: u32,
) {
    frame.instances.push(
        Instance::new(origin + rot * offset, scale, colour)
            .with_rot(rot)
            .with_mesh(mesh),
    );
}

/// Consistent wheelbase and lights, with three distinct body/roof silhouettes.
#[allow(clippy::too_many_lines)] // Authored body details share one local transform.
pub fn car(frame: &mut Frame, ids: &Meshes, car: &Car, colour: Vec3, time: f32, distance: f32) {
    let vehicle = usize::from(car.vehicle.min(2));
    let yaw = Quat::from_rotation_y(car.yaw);
    let p = Vec3::new(car.pos.x, 0.0, car.pos.y);
    let lateral = car.vel.dot(car::right(car.yaw));
    let body_rot = yaw * Quat::from_rotation_z((lateral * -0.009).clamp(-0.045, 0.045));
    let paint = if car.hit_left > 0.0 {
        colour.lerp(Vec3::ONE, 0.45)
    } else {
        colour
    };
    let black = Vec3::new(0.024, 0.028, 0.035);
    let silver = Vec3::new(0.61, 0.65, 0.70);
    let (length, width, axle, roof_y, roof_z) = match vehicle {
        1 => (4.0, 0.89, 1.31, 1.13, -0.20),
        2 => (4.84, 0.99, 1.57, 1.43, -0.31),
        _ => (4.60, 0.96, 1.48, 1.33, -0.28),
    };
    part(
        frame,
        p,
        yaw,
        Vec3::new(0.0, 0.012, 0.0),
        Vec3::new(width + 0.15, 1.0, length * 0.54),
        Vec3::splat(0.055),
        ids.disc,
    );
    part(
        frame,
        p,
        body_rot,
        Vec3::ZERO,
        Vec3::ONE,
        paint,
        ids.bodies[vehicle],
    );
    part(
        frame,
        p,
        body_rot,
        Vec3::ZERO,
        Vec3::ONE,
        Vec3::ONE,
        ids.glass[vehicle],
    );
    // Painted roof, black floor/splitter and a restrained bonnet stripe.
    part(
        frame,
        p,
        body_rot,
        Vec3::new(0.0, roof_y, roof_z),
        Vec3::new(
            if vehicle == 1 { 0.75 } else { 1.02 },
            0.025,
            if vehicle == 1 { 0.28 } else { 0.68 },
        ),
        paint * 0.90,
        0,
    );
    part(
        frame,
        p,
        body_rot,
        Vec3::new(0.0, 0.29, 0.0),
        Vec3::new(width * 1.88, 0.09, length * 0.91),
        black,
        0,
    );
    part(
        frame,
        p,
        body_rot,
        Vec3::new(0.0, if vehicle == 2 { 0.865 } else { 0.70 }, length * 0.33),
        Vec3::new(0.33, 0.017, 0.63),
        black * 1.7,
        0,
    );
    for side in [-1.0_f32, 1.0] {
        for z in [-axle, axle] {
            let steer = if z > 0.0 {
                -car.steer_angle * 0.44
            } else {
                0.0
            };
            let spin = distance / 0.36;
            let wheel_rot = yaw * Quat::from_rotation_y(steer) * Quat::from_rotation_x(spin);
            let centre = p + yaw * Vec3::new(width * side, 0.36, z);
            frame.instances.push(
                Instance::new(centre, Vec3::ONE, black)
                    .with_rot(wheel_rot)
                    .with_mesh(ids.tyre),
            );
            let outer = centre + (yaw * Quat::from_rotation_y(steer)) * Vec3::X * side * 0.16;
            frame.instances.push(
                Instance::new(outer, Vec3::ONE, silver)
                    .with_rot(wheel_rot)
                    .with_mesh(ids.rim),
            );
            // Five dark spokes make wheel rotation readable at low speed.
            for n in 0_u8..5 {
                let rot =
                    wheel_rot * Quat::from_rotation_x(f32::from(n) * std::f32::consts::TAU / 5.0);
                part(
                    frame,
                    outer,
                    rot,
                    Vec3::new(side * 0.022, 0.10, 0.0),
                    Vec3::new(0.022, 0.19, 0.035),
                    black,
                    0,
                );
            }
        }
        let nose = length * 0.5 - 0.035;
        part(
            frame,
            p,
            body_rot,
            Vec3::new(
                side * width * 0.66,
                if vehicle == 2 { 0.69 } else { 0.53 },
                nose,
            ),
            Vec3::new(0.43, 0.065, 0.07),
            Vec3::new(1.0, 0.98, 0.79),
            0,
        );
        part(
            frame,
            p,
            body_rot,
            Vec3::new(
                side * width * 0.63,
                if vehicle == 2 { 0.72 } else { 0.61 },
                -nose,
            ),
            Vec3::new(0.48, 0.085, 0.07),
            Vec3::new(1.0, 0.025, 0.015),
            0,
        );
        part(
            frame,
            p,
            body_rot,
            Vec3::new(side * (width + 0.05), 0.86, 0.33),
            Vec3::new(0.19, 0.12, 0.24),
            paint,
            0,
        );
        // Exhaust outlets and low, sharp boost jets.
        part(
            frame,
            p,
            yaw,
            Vec3::new(side * 0.52, 0.35, -length * 0.5),
            Vec3::new(0.17, 0.13, 0.14),
            silver,
            0,
        );
        if car.boosting() {
            let flicker = 0.8 + (time * 38.0).sin().abs() * 0.35;
            part(
                frame,
                p,
                yaw,
                Vec3::new(side * 0.52, 0.35, -length * 0.5 - 0.42 * flicker),
                Vec3::new(0.13, 0.12, 0.85 * flicker),
                Vec3::new(0.12, 0.71, 1.0),
                0,
            );
            part(
                frame,
                p,
                yaw,
                Vec3::new(side * 0.52, 0.35, -length * 0.5 - 0.13),
                Vec3::new(0.09, 0.085, 0.3),
                Vec3::new(0.95, 0.98, 1.0),
                0,
            );
        }
        if car.drift > 0.25 && car.speed() > 7.0 {
            part(
                frame,
                p,
                yaw,
                Vec3::new(side * width, 0.014, -axle - 1.7),
                Vec3::new(0.24, 0.008, 3.4),
                Vec3::splat(0.095),
                0,
            );
            let hot = if car.drift_charge > 0.65 {
                Vec3::new(1.0, 0.64, 0.04)
            } else {
                Vec3::new(0.12, 0.73, 1.0)
            };
            part(
                frame,
                p,
                yaw,
                Vec3::new(side * (width + 0.12), 0.15, -axle - 0.50),
                Vec3::new(0.08, 0.07, 0.42),
                hot,
                0,
            );
        }
    }
    // GT wing, lightweight low ducktail, muscle rear lip.
    let wing_y = if vehicle == 0 {
        1.12
    } else if vehicle == 1 {
        0.76
    } else {
        0.95
    };
    let wing_z = -length * 0.42;
    part(
        frame,
        p,
        body_rot,
        Vec3::new(0.0, wing_y, wing_z),
        Vec3::new(width * 1.95, 0.07, 0.25),
        if vehicle == 2 { paint } else { black },
        0,
    );
    if vehicle == 0 {
        for side in [-0.6, 0.6] {
            part(
                frame,
                p,
                body_rot,
                Vec3::new(side, 0.95, wing_z),
                Vec3::new(0.065, 0.30, 0.12),
                black,
                0,
            );
        }
    }
    // Open rings keep the car visible: no fake transparent shield material.
    if car.shield_left > 0.0 || car.grip_left > 0.0 {
        let tint = if car.shield_left > 0.0 {
            Vec3::new(0.15, 0.75, 1.0)
        } else {
            Vec3::new(0.34, 1.0, 0.29)
        };
        for n in 0_u8..12 {
            let angle = f32::from(n) * std::f32::consts::TAU / 12.0 + time;
            part(
                frame,
                p,
                Quat::IDENTITY,
                Vec3::new(angle.cos() * 1.5, 0.20, angle.sin() * 2.7),
                Vec3::new(0.13, 0.055, 0.13),
                tint,
                0,
            );
        }
    }
}

/// Trackside objects stay behind the same wall boundary as simulation.
#[allow(clippy::too_many_lines)] // Fixed circuit dressing uses a single scene builder.
pub fn circuit(frame: &mut Frame, race: &Race, ids: &Meshes) {
    let track = &race.track;
    let half = track.half_width();
    let white = Vec3::new(0.87, 0.91, 0.91);
    let dark = Vec3::new(0.045, 0.065, 0.08);
    // Shared irregular canopy geometry and contact shadows frame the circuit.
    for sample in 0_u16..26 {
        let (p, tangent) = track.at(f32::from(sample) * track.length() / 26.0);
        let left = Vec2::new(-tangent.y, tangent.x);
        let side = if sample % 3 == 0 { -1.0 } else { 1.0 };
        let p = p + left * side * (half + 20.0 + f32::from(sample % 4) * 3.0);
        let origin = Vec3::new(p.x, 0.0, p.y);
        let tall = 6.5 + f32::from(sample % 5) * 0.8;
        part(
            frame,
            origin,
            Quat::IDENTITY,
            Vec3::new(0.0, 0.01, 0.0),
            Vec3::new(4.5, 1.0, 4.5),
            Vec3::new(0.08, 0.11, 0.06),
            ids.disc,
        );
        part(
            frame,
            origin,
            Quat::IDENTITY,
            Vec3::new(0.0, tall * 0.38, 0.0),
            Vec3::new(0.62, tall * 0.76, 0.62),
            Vec3::new(0.16, 0.105, 0.06),
            0,
        );
        for (x, y, z, scale) in [
            (0.0, tall, 0.0, 3.4),
            (-1.9, tall * 0.77, 0.7, 2.6),
            (1.6, tall * 0.84, -0.6, 2.7),
        ] {
            part(
                frame,
                origin,
                Quat::from_rotation_y(f32::from(sample)),
                Vec3::new(x, y, z),
                Vec3::new(scale, scale * 1.1, scale),
                Vec3::new(0.61, 0.77, 0.50),
                ids.foliage,
            );
        }
    }
    // Two covered grandstands give the straight a spectator/paddock side.
    for distance in [65.0, 150.0] {
        let (p, tangent) = track.at(distance);
        let left = Vec2::new(-tangent.y, tangent.x);
        let p = p + left * (half + 23.0);
        let origin = Vec3::new(p.x, 0.0, p.y);
        let rot = Quat::from_rotation_y(tangent.x.atan2(tangent.y));
        for row in 0_u8..5 {
            let y = 0.9 + f32::from(row) * 0.65;
            let x = f32::from(row) * 1.25;
            part(
                frame,
                origin,
                rot,
                Vec3::new(x, y * 0.5, 0.0),
                Vec3::new(1.35, y, 48.0),
                Vec3::new(0.35, 0.37, 0.38),
                0,
            );
            part(
                frame,
                origin,
                rot,
                Vec3::new(x, y + 0.18, 0.0),
                Vec3::new(0.55, 0.32, 46.0),
                if row % 2 == 0 {
                    Vec3::new(0.17, 0.24, 0.26)
                } else {
                    Vec3::new(0.60, 0.29, 0.16)
                },
                0,
            );
        }
        part(
            frame,
            origin,
            rot,
            Vec3::new(2.5, 6.6, 0.0),
            Vec3::new(9.8, 0.24, 53.0),
            Vec3::new(0.20, 0.24, 0.28),
            0,
        );
        for z in [-23.0, 0.0, 23.0] {
            part(
                frame,
                origin,
                rot,
                Vec3::new(6.4, 3.3, z),
                Vec3::new(0.28, 6.6, 0.28),
                Vec3::new(0.42, 0.45, 0.47),
                0,
            );
        }
    }
    // Rail caps and regular stakes create speed cues without a corridor of walls.
    for sample in 0_u16..100 {
        let s = f32::from(sample) * 18.0;
        if s >= track.length() {
            break;
        }
        let (p, tangent) = track.at(s);
        let left = Vec2::new(-tangent.y, tangent.x);
        let rot = Quat::from_rotation_y(tangent.x.atan2(tangent.y));
        for side in [-1.0, 1.0] {
            let edge = p + left * side * (half + fire_core::sim::WALL_MARGIN + 0.15);
            let origin = Vec3::new(edge.x, 0.0, edge.y);
            part(
                frame,
                origin,
                rot,
                Vec3::new(0.0, 0.66, 0.0),
                Vec3::new(0.16, 1.32, 0.16),
                dark,
                0,
            );
            part(
                frame,
                origin,
                rot,
                Vec3::new(0.0, 1.15, 0.0),
                Vec3::new(0.28, 0.11, 3.2),
                white * 0.75,
                0,
            );
        }
    }
    // Corner entry markers: 3/2/1 stripes count down the braking zone.
    for fraction in [0.23, 0.41, 0.60, 0.77, 0.91] {
        for bars in 1_u8..=3 {
            let (p, tangent) = track.at(track.length() * fraction - f32::from(bars) * 17.0);
            let left = Vec2::new(-tangent.y, tangent.x);
            let edge = p + left * (half + fire_core::sim::WALL_MARGIN + 0.55);
            let rot = Quat::from_rotation_y(tangent.x.atan2(tangent.y));
            let origin = Vec3::new(edge.x, 0.0, edge.y);
            part(
                frame,
                origin,
                rot,
                Vec3::new(0.0, 1.65, 0.0),
                Vec3::new(1.25, 1.2, 0.10),
                white,
                0,
            );
            for stripe in 0..bars {
                part(
                    frame,
                    origin,
                    rot,
                    Vec3::new(
                        (f32::from(stripe) - f32::from(bars - 1) * 0.5) * 0.29,
                        1.65,
                        -0.062,
                    ),
                    Vec3::new(0.16, 0.82, 0.02),
                    dark,
                    0,
                );
            }
        }
    }
    // Grid brackets, start gantry and track-direction arrows.
    let (start, tangent) = track.at(0.0);
    let rot = Quat::from_rotation_y(tangent.x.atan2(tangent.y));
    let origin = Vec3::new(start.x, 0.0, start.y);
    for side in [-1.0, 1.0] {
        part(
            frame,
            origin,
            rot,
            Vec3::new(side * (half + 2.1), 3.6, 0.0),
            Vec3::new(0.50, 7.2, 0.50),
            dark,
            0,
        );
    }
    part(
        frame,
        origin,
        rot,
        Vec3::new(0.0, 7.0, 0.0),
        Vec3::new(half * 2.0 + 4.7, 0.90, 0.75),
        dark,
        0,
    );
    part(
        frame,
        origin,
        rot,
        Vec3::new(0.0, 7.3, -0.4),
        Vec3::new(half * 2.0 + 3.4, 0.09, 0.05),
        Vec3::new(1.0, 0.36, 0.06),
        0,
    );
    for slot in 0_u8..8 {
        let distance = track.length() - 12.0 - f32::from(slot / 2) * 9.0;
        let (p, tangent) = track.at(distance);
        let rot = Quat::from_rotation_y(tangent.x.atan2(tangent.y));
        let left = Vec2::new(-tangent.y, tangent.x);
        let p = p + left
            * if slot % 2 == 0 {
                half * 0.42
            } else {
                -half * 0.42
            };
        let origin = Vec3::new(p.x, 0.018, p.y);
        part(
            frame,
            origin,
            rot,
            Vec3::new(0.0, 0.0, 2.5),
            Vec3::new(3.0, 0.012, 0.12),
            white * 0.75,
            0,
        );
        for x in [-1.5, 1.5] {
            part(
                frame,
                origin,
                rot,
                Vec3::new(x, 0.0, 1.9),
                Vec3::new(0.12, 0.012, 1.3),
                white * 0.75,
                0,
            );
        }
    }
}

pub fn items(frame: &mut Frame, race: &Race, ids: &Meshes) {
    let time = race.elapsed_seconds();
    for pickup in &race.pickups {
        if pickup.respawn_left > 0.0 {
            continue;
        }
        let p = Vec3::new(
            pickup.pos.x,
            1.12 + (time * 2.4 + f32::from(pickup.id)).sin() * 0.16,
            pickup.pos.y,
        );
        let rot =
            Quat::from_rotation_y(time * 0.85 + f32::from(pickup.id)) * Quat::from_rotation_z(0.13);
        frame.instances.push(
            Instance::new(p, Vec3::splat(1.22), Vec3::ONE)
                .with_rot(rot)
                .with_mesh(ids.pickup),
        );
        part(
            frame,
            p,
            Quat::IDENTITY,
            Vec3::new(0.0, -p.y + 0.02, 0.0),
            Vec3::new(0.78, 1.0, 0.78),
            Vec3::new(0.035, 0.14, 0.18),
            ids.disc,
        );
    }
    for pulse in &race.projectiles {
        let p = Vec3::new(pulse.pos.x, 0.72, pulse.pos.y);
        let target = race
            .racers
            .get(usize::from(pulse.target))
            .map_or(pulse.pos + Vec2::Y, |r| r.car.pos);
        let d = target - pulse.pos;
        let rot = Quat::from_rotation_y(d.x.atan2(d.y));
        part(
            frame,
            p,
            rot,
            Vec3::ZERO,
            Vec3::new(0.45, 0.45, 0.8),
            Vec3::new(1.0, 0.20, 0.04),
            0,
        );
        part(
            frame,
            p,
            rot,
            Vec3::new(0.0, 0.0, -0.65),
            Vec3::new(0.18, 0.18, 0.65),
            Vec3::new(1.0, 0.74, 0.15),
            0,
        );
    }
    for oil in &race.hazards {
        let p = Vec3::new(oil.pos.x, 0.025, oil.pos.y);
        part(
            frame,
            p,
            Quat::IDENTITY,
            Vec3::ZERO,
            Vec3::new(2.7, 1.0, 2.7),
            Vec3::new(0.024, 0.020, 0.042),
            ids.disc,
        );
        part(
            frame,
            p,
            Quat::IDENTITY,
            Vec3::new(-0.4, 0.002, 0.3),
            Vec3::new(1.5, 1.0, 1.0),
            Vec3::new(0.085, 0.035, 0.12),
            ids.disc,
        );
    }
}
