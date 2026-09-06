//! Readable, grounded supply cases; collision and collection remain server-owned.

use arena_core::shooter::{SupplyKind, SupplySpawn};
use ember_engine::glam::Vec3;
use ember_engine::{Frame, Instance};

pub fn push(frame: &mut Frame, spawns: &[SupplySpawn], active: &[bool]) {
    for (index, spawn) in spawns.iter().enumerate() {
        let origin = Vec3::new(spawn.pos[0], 0.0, spawn.pos[1]);
        let ready = active.get(index).copied().unwrap_or(false);
        let health = spawn.kind == SupplyKind::Health;
        let accent = if health {
            Vec3::new(0.18, 0.72, 0.48)
        } else {
            Vec3::new(0.92, 0.65, 0.20)
        };
        let mut part = |at: Vec3, scale: Vec3, color: Vec3| {
            frame
                .instances
                .push(Instance::new(origin + at, scale, color).with_surface(0.72, 0.0));
        };
        // A low, dark cradle marks the supply's return point when depleted.
        part(
            Vec3::new(0.0, 0.035, 0.0),
            Vec3::new(0.96, 0.07, 0.68),
            Vec3::splat(0.16),
        );
        if !ready {
            continue;
        }
        let shell = if health {
            Vec3::new(0.78, 0.82, 0.78)
        } else {
            Vec3::new(0.25, 0.31, 0.20)
        };
        part(
            Vec3::new(0.0, 0.28, 0.0),
            Vec3::new(0.78, 0.46, 0.50),
            shell,
        );
        part(
            Vec3::new(0.0, 0.53, 0.0),
            Vec3::new(0.83, 0.07, 0.55),
            shell * 0.75,
        );
        part(
            Vec3::new(0.0, 0.60, 0.0),
            Vec3::new(0.25, 0.07, 0.06),
            Vec3::splat(0.13),
        );
        for x in [-0.28, 0.28] {
            part(
                Vec3::new(x, 0.35, 0.262),
                Vec3::new(0.065, 0.12, 0.035),
                Vec3::splat(0.22),
            );
            part(
                Vec3::new(x, 0.35, -0.262),
                Vec3::new(0.065, 0.12, 0.035),
                Vec3::splat(0.22),
            );
        }
        for z in [-0.256, 0.256] {
            if health {
                part(
                    Vec3::new(0.0, 0.29, z),
                    Vec3::new(0.30, 0.09, 0.018),
                    accent,
                );
                part(
                    Vec3::new(0.0, 0.29, z),
                    Vec3::new(0.09, 0.28, 0.020),
                    accent,
                );
            } else {
                for x in [-0.12, 0.0, 0.12] {
                    part(Vec3::new(x, 0.29, z), Vec3::new(0.055, 0.20, 0.018), accent);
                }
            }
        }
        // Top marking remains readable from a roof without a floating icon.
        if health {
            part(
                Vec3::new(0.0, 0.570, 0.0),
                Vec3::new(0.30, 0.01, 0.09),
                accent,
            );
            part(
                Vec3::new(0.0, 0.571, 0.0),
                Vec3::new(0.09, 0.01, 0.30),
                accent,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_supplies_only_show_empty_cradles() {
        let spawns = [
            SupplySpawn {
                pos: [2.0, 4.0],
                kind: SupplyKind::Health,
            },
            SupplySpawn {
                pos: [-2.0, 4.0],
                kind: SupplyKind::Ammo,
            },
        ];
        let mut empty = Frame::default();
        push(&mut empty, &spawns, &[]);
        assert_eq!(empty.instances.len(), 2);
        let mut ready = Frame::default();
        push(&mut ready, &spawns, &[true, true]);
        assert!(ready.instances.len() > 20);
        assert!(ready.instances.len() < 40, "bounded draw instance budget");
    }
}
