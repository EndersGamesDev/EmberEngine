//! Breakwater Harbor: fixed metric geometry for an eight-player tactical port.
//!
//! All placements are authored tables, never a seed or a prop-scattering rule.
//! West is the covered service warehouse, centre is staggered cargo, east is
//! the open quay. The renderer reads these same boxes before dressing them.

use crate::shooter::{Cover, Hill, Level, Obstacle};

pub const HARBOR_HALF: f32 = 48.0;
pub const CONTAINER_W: f32 = 2.438;
pub const CONTAINER_H: f32 = 2.591;
pub const CONTAINER_20: f32 = 6.06;
pub const CONTAINER_40: f32 = 12.19;
pub const CRANE_CENTERS_Z: [f32; 2] = [-18.0, 18.0];
pub const WAREHOUSE_MIN: [f32; 2] = [-44.0, -28.0];
pub const WAREHOUSE_MAX: [f32; 2] = [-28.0, 28.0];
pub const WAREHOUSE_ROOF_BASE: f32 = 4.2;

/// A real 20/40-foot cargo box, aligned to one world axis.
#[derive(Clone, Copy, Debug)]
pub struct ContainerPlacement {
    pub center: [f32; 2],
    pub length: f32,
    pub along_x: bool,
    pub tiers: u8,
}

impl ContainerPlacement {
    #[must_use]
    pub fn obstacle(self) -> Obstacle {
        let [x, z] = self.center;
        let [hx, hz] = if self.along_x {
            [self.length * 0.5, CONTAINER_W * 0.5]
        } else {
            [CONTAINER_W * 0.5, self.length * 0.5]
        };
        Obstacle::boxed(
            Cover::Container,
            [x - hx, z - hz],
            [x + hx, z + hz],
            0.0,
            CONTAINER_H * f32::from(self.tiers),
        )
    }
}

const fn cargo(x: f32, z: f32, length: f32, along_x: bool, tiers: u8) -> ContainerPlacement {
    ContainerPlacement {
        center: [x, z],
        length,
        along_x,
        tiers,
    }
}

/// Staggered crosswise outer rows and lengthwise inner stacks interrupt long
/// angles while leaving 2.81 m outer cargo gaps and generous cross streets.
pub const CONTAINERS: &[ContainerPlacement] = &[
    cargo(-15.0, -27.0, CONTAINER_40, true, 1),
    cargo(0.0, -27.0, CONTAINER_40, true, 2),
    cargo(15.0, -27.0, CONTAINER_40, true, 1),
    cargo(-15.0, -13.0, CONTAINER_40, false, 2),
    cargo(0.0, -13.0, CONTAINER_20, true, 1),
    cargo(15.0, -13.0, CONTAINER_40, false, 1),
    cargo(-15.0, 13.0, CONTAINER_40, false, 1),
    cargo(0.0, 13.0, CONTAINER_20, true, 1),
    cargo(15.0, 13.0, CONTAINER_40, false, 2),
    cargo(-15.0, 27.0, CONTAINER_40, true, 1),
    cargo(0.0, 27.0, CONTAINER_40, true, 2),
    cargo(15.0, 27.0, CONTAINER_40, true, 1),
    cargo(-22.0, -3.0, CONTAINER_20, false, 1),
    cargo(22.0, 3.0, CONTAINER_20, false, 1),
    cargo(27.0, -31.0, CONTAINER_20, false, 1),
    cargo(27.0, 31.0, CONTAINER_20, false, 1),
];

/// Alternating north/south in FFA; the existing z-sign split gives each TDM
/// side four separate pockets. Three-sided screens open towards the boundary.
pub const SPAWNS: [[f32; 2]; 8] = [
    [-37.0, 42.0],
    [-37.0, -42.0],
    [-13.0, 42.0],
    [-13.0, -42.0],
    [11.0, 42.0],
    [11.0, -42.0],
    [35.0, 42.0],
    [35.0, -42.0],
];

pub const HARBOR_HILL: Hill = Hill {
    min: [-3.0, -3.0],
    max: [3.0, 3.0],
    top: 0.0,
};

/// Warehouse wall/roof solids. Doorways are genuine gaps, not painted doors.
pub const WAREHOUSE: &[Obstacle] = &[
    Obstacle::boxed(Cover::Wall, [-44.0, -28.0], [-43.5, 28.0], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-28.5, -28.0], [-28.0, -16.0], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-28.5, -10.0], [-28.0, 10.0], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-28.5, 16.0], [-28.0, 28.0], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-43.5, -28.0], [-38.0, -27.5], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-34.0, -28.0], [-28.5, -27.5], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-43.5, 27.5], [-38.0, 28.0], 0.0, 4.2),
    Obstacle::boxed(Cover::Wall, [-34.0, 27.5], [-28.5, 28.0], 0.0, 4.2),
    Obstacle::boxed(
        Cover::Roof,
        WAREHOUSE_MIN,
        WAREHOUSE_MAX,
        WAREHOUSE_ROOF_BASE,
        4.55,
    ),
    // Interior service partition forces a deliberate dogleg around x=-31.
    Obstacle::boxed(Cover::Wall, [-39.0, -0.4], [-33.0, 0.4], 0.0, 3.0),
    Obstacle::boxed(Cover::Crate, [-42.0, -19.0], [-40.0, -17.0], 0.0, 1.2),
    Obstacle::boxed(Cover::Crate, [-42.0, 17.0], [-40.0, 19.0], 0.0, 1.2),
];

/// Four narrow solid legs per gantry, with a ten-metre lane between x=32/44.
pub const CRANE_LEGS: &[Obstacle] = &[
    Obstacle::boxed(Cover::Wall, [31.4, -21.2], [32.6, -20.0], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [43.4, -21.2], [44.6, -20.0], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [31.4, -16.0], [32.6, -14.8], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [43.4, -16.0], [44.6, -14.8], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [31.4, 14.8], [32.6, 16.0], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [43.4, 14.8], [44.6, 16.0], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [31.4, 20.0], [32.6, 21.2], 0.0, 12.0),
    Obstacle::boxed(Cover::Wall, [43.4, 20.0], [44.6, 21.2], 0.0, 12.0),
];

/// Low cover outside the central hill, plus two safe ammo/crate/roof chains.
pub const LOW_COVER: &[Obstacle] = &[
    Obstacle::boxed(Cover::Crate, [-7.0, -5.0], [-5.0, -3.0], 0.0, 1.2),
    Obstacle::boxed(Cover::Crate, [5.0, 3.0], [7.0, 5.0], 0.0, 1.2),
    Obstacle::boxed(Cover::Sandbag, [5.0, -5.0], [8.0, -4.2], 0.0, 1.1),
    Obstacle::boxed(Cover::Sandbag, [-8.0, 4.2], [-5.0, 5.0], 0.0, 1.1),
    Obstacle::boxed(Cover::Ammo, [-15.6, -22.6], [-14.4, -21.4], 0.0, 0.55),
    Obstacle::boxed(Cover::Crate, [-15.6, -24.6], [-14.4, -23.4], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [14.4, 21.4], [15.6, 22.6], 0.0, 0.55),
    Obstacle::boxed(Cover::Crate, [14.4, 23.4], [15.6, 24.6], 0.0, 1.2),
    Obstacle::boxed(Cover::Crate, [34.0, -5.0], [36.0, -3.0], 0.0, 1.2),
    Obstacle::boxed(Cover::Crate, [40.0, 3.0], [42.0, 5.0], 0.0, 1.2),
];

pub const LOOT_CENTERS: [[f32; 2]; 5] = [
    [-25.0, -13.0],
    [-25.0, 13.0],
    [38.0, -22.0],
    [38.0, 22.0],
    [0.0, 0.0],
];

impl Level {
    /// Breakwater Harbor: a fixed 96 by 96 metre, three-route cargo port.
    #[must_use]
    pub fn harbor() -> Self {
        let mut obstacles: Vec<_> = CONTAINERS.iter().map(|p| p.obstacle()).collect();
        obstacles.extend_from_slice(WAREHOUSE);
        obstacles.extend_from_slice(CRANE_LEGS);
        obstacles.extend_from_slice(LOW_COVER);
        for [x, z] in SPAWNS {
            let sign = z.signum();
            let a = 37.6 * sign;
            let b = 38.4 * sign;
            let rear = 44.5 * sign;
            obstacles.push(Obstacle::boxed(
                Cover::Wall,
                [x - 4.5, a.min(b)],
                [x + 4.5, a.max(b)],
                0.0,
                2.8,
            ));
            for dx in [-4.5, 3.7] {
                obstacles.push(Obstacle::boxed(
                    Cover::Wall,
                    [x + dx, b.min(rear)],
                    [x + dx + 0.8, b.max(rear)],
                    0.0,
                    2.8,
                ));
            }
        }
        for [x, z] in LOOT_CENTERS {
            obstacles.push(Obstacle::boxed(
                Cover::Loot,
                [x - 0.5, z - 0.5],
                [x + 0.5, z + 0.5],
                2.3,
                3.3,
            ));
        }
        Self {
            arena_half: HARBOR_HALF,
            obstacles,
            spawns: SPAWNS.to_vec(),
            pads: Vec::new(),
            decor: Vec::new(),
            hill: Some(HARBOR_HILL),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    use super::*;
    use crate::shooter::{
        AIR_MOVE_SPEED, BODY_H_STAND, EYE_STAND, FIXED_DT, GameMode, MAP_HARBOR, MOVE_SPEED,
        PLAYER_R, PlayerIn, SHOT_COVER, SHOT_WALL, Sim, blocked_in, move_circle, move_circle_in,
        movement_speed, segment_hits_cover, step_vertical, support_height, weapon_stats,
    };

    #[test]
    fn harbor_is_authored_metric_and_seed_independent() {
        let level = Level::harbor();
        assert_eq!(level.arena_half, 48.0);
        assert_eq!(level.obstacles.len(), 75);
        assert_eq!(level.spawns, SPAWNS);
        assert_eq!(level.hill, Some(HARBOR_HILL));
        for seed in [0, 1, 27, u64::MAX] {
            assert_eq!(Level::named(MAP_HARBOR, seed), level);
        }
        let encoded = serde_json::to_string(&level).unwrap();
        assert_eq!(serde_json::from_str::<Level>(&encoded).unwrap(), level);
        for o in &level.obstacles {
            assert!(o.min[0] < o.max[0] && o.min[1] < o.max[1] && o.base < o.h && o.base >= 0.0);
            assert!(o.min.iter().all(|v| v.is_finite() && *v >= -HARBOR_HALF));
            assert!(o.max.iter().all(|v| v.is_finite() && *v <= HARBOR_HALF));
        }
        for p in CONTAINERS {
            let b = p.obstacle();
            let dimensions = [b.max[0] - b.min[0], b.max[1] - b.min[1]];
            let (length, width): (f32, f32) = if p.along_x {
                dimensions.into()
            } else {
                (dimensions[1], dimensions[0])
            };
            assert!((width - CONTAINER_W).abs() < 1e-5);
            assert!((length - p.length).abs() < 1e-5);
            assert!([CONTAINER_20, CONTAINER_40].contains(&p.length));
            assert_eq!(b.h, CONTAINER_H * f32::from(p.tiers));
        }
    }

    #[test]
    fn all_eight_spawn_pockets_are_clear_and_screened_in_both_directions() {
        let level = Level::harbor();
        assert_eq!(level.spawns_for(0).len(), 4);
        assert_eq!(level.spawns_for(1).len(), 4);
        for (i, a) in SPAWNS.iter().enumerate() {
            assert!(!blocked_in(
                *a,
                0.0,
                PLAYER_R + 0.1,
                &level.obstacles,
                HARBOR_HALF
            ));
            for (j, b) in SPAWNS.iter().enumerate() {
                if i == j {
                    continue;
                }
                let from = [a[0], EYE_STAND, a[1]];
                let to = [b[0], EYE_STAND, b[1]];
                assert!(
                    !segment_hits_cover(from, to, &[]),
                    "empty control must see any distance"
                );
                assert!(
                    segment_hits_cover(from, to, &level.obstacles),
                    "spawn {i} sees {j}"
                );
            }
        }
    }

    #[test]
    fn every_spawn_and_route_and_reward_share_walkable_floor() {
        // Half-metre grid with an extra 5 cm clearance, using the actual
        // bounded body collision. One connected component proves both ways.
        let level = Level::harbor();
        let n = 95_i32;
        let side = (2 * n + 1) as usize;
        let idx = |i: i32, j: i32| ((i + n) as usize) * side + (j + n) as usize;
        let at = |i: i32, j: i32| [i as f32 * 0.5, j as f32 * 0.5];
        let mut free = vec![false; side * side];
        for i in -n..=n {
            for j in -n..=n {
                free[idx(i, j)] = !blocked_in(
                    at(i, j),
                    0.0,
                    PLAYER_R + 0.05,
                    &level.obstacles,
                    HARBOR_HALF,
                );
            }
        }
        let mut seen = vec![false; free.len()];
        let mut stack = vec![(-74_i32, 84_i32)];
        seen[idx(-74, 84)] = true;
        while let Some((i, j)) = stack.pop() {
            for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (a, b) = (i + di, j + dj);
                if a.abs() > n || b.abs() > n {
                    continue;
                }
                let k = idx(a, b);
                if free[k] && !seen[k] {
                    seen[k] = true;
                    stack.push((a, b));
                }
            }
        }
        let routes = [
            [0.0, 0.0],
            [-36.0, 15.0],
            [-36.0, -15.0],
            [38.0, 15.0],
            [38.0, -15.0],
            [7.5, 32.0],
            [7.5, -32.0],
        ];
        for p in SPAWNS
            .iter()
            .chain(LOOT_CENTERS.iter())
            .chain(routes.iter())
        {
            let k = idx((p[0] * 2.0).round() as i32, (p[1] * 2.0).round() as i32);
            assert!(free[k] && seen[k], "unreachable floor {p:?}");
        }
    }

    fn walk_route(level: &Level, route: &[[f32; 2]]) -> f32 {
        let mut pos = route[0];
        let mut ticks = 0_u32;
        for target in &route[1..] {
            for _ in 0..1800 {
                let mv = [
                    (target[0] - pos[0]) / (MOVE_SPEED * FIXED_DT),
                    (target[1] - pos[1]) / (MOVE_SPEED * FIXED_DT),
                ];
                pos = move_circle_in(
                    pos,
                    0.0,
                    mv,
                    MOVE_SPEED,
                    FIXED_DT,
                    &level.obstacles,
                    level.arena_half,
                );
                ticks += 1;
                if (pos[0] - target[0]).abs() + (pos[1] - target[1]).abs() < 1e-3 {
                    break;
                }
            }
            assert!(
                (pos[0] - target[0]).abs() + (pos[1] - target[1]).abs() < 1e-3,
                "stuck at {pos:?}, target {target:?}"
            );
        }
        ticks as f32 * FIXED_DT
    }

    #[test]
    fn three_route_families_walk_at_four_metres_per_second_with_crosslinks() {
        let level = Level::harbor();
        assert_eq!(MOVE_SPEED, 4.0);
        assert_eq!(AIR_MOVE_SPEED, 9.0);
        let west = [
            [-36.0, 32.0],
            [-36.0, 4.0],
            [-31.0, 4.0],
            [-31.0, -4.0],
            [-36.0, -4.0],
            [-36.0, -32.0],
        ];
        let center = [
            [7.5, 32.0],
            [7.5, 8.0],
            [10.0, 8.0],
            [10.0, -8.0],
            [7.5, -8.0],
            [7.5, -32.0],
        ];
        let east = [[38.0, 32.0], [38.0, -32.0]];
        for (route, seconds) in [(&west[..], 18.5), (&center[..], 17.25), (&east[..], 16.0)] {
            let measured = walk_route(&level, route);
            assert!(
                (measured - seconds).abs() < 0.15,
                "{measured}s vs {seconds}s"
            );
        }
        // Door -> west reward -> cargo cross street -> quay, both banks.
        for sign in [-1.0, 1.0] {
            let route = [
                [-36.0, 13.0 * sign],
                [-25.0, 13.0 * sign],
                [-25.0, 35.5 * sign],
                [38.0, 35.5 * sign],
                [38.0, 32.0 * sign],
            ];
            walk_route(&level, &route);
        }
    }

    #[test]
    fn bodies_and_bullets_use_harbor_walls_not_the_legacy_boundary() {
        let level = Level::harbor();
        let mut sim = Sim::from_level(&level, 4, GameMode::Ffa);
        sim.add_player(0);
        sim.players[0].pos = [38.0, -10.0];
        for _ in 0..60 {
            sim.step(&|_| PlayerIn {
                mv: [0.0, 1.0],
                ..Default::default()
            });
        }
        assert!((sim.players[0].pos[1] + 6.0).abs() < 0.01);
        assert_eq!(
            move_circle([38.0, 0.0], 0.0, [0.0, 1.0], 4.0, FIXED_DT, &[]),
            [38.0, 0.0]
        );
        sim.players[0].pos = [38.0, -10.0];
        sim.step(&|_| PlayerIn {
            aim: [0.0, 1.0],
            fire: true,
            ..Default::default()
        });
        assert!(
            !sim.bullets.is_empty(),
            "round outside |24| remains in flight"
        );
        sim.bullets.clear();
        sim.players[0].cooldown = 0.0;
        sim.players[0].pos = [46.0, 0.0];
        sim.step(&|_| PlayerIn {
            aim: [1.0, 0.0],
            fire: true,
            ..Default::default()
        });
        assert_eq!(sim.shots[0].hit, SHOT_WALL);
        assert_eq!(sim.shots[0].to[0], HARBOR_HALF - weapon_stats(1).radius);
        sim.players[0].cooldown = 0.0;
        sim.players[0].pos = [40.0, -20.6];
        sim.step(&|_| PlayerIn {
            aim: [1.0, 0.0],
            fire: true,
            ..Default::default()
        });
        assert_eq!(sim.shots[0].hit, SHOT_COVER, "crane leg is real cover");
        assert!((sim.shots[0].to[0] - 43.4).abs() < 1e-4);
        sim.players[0].pos = [HARBOR_HALF - PLAYER_R - 0.01, 0.0];
        for _ in 0..10 {
            sim.step(&|_| PlayerIn {
                mv: [1.0, 0.0],
                ..Default::default()
            });
        }
        assert!(sim.players[0].pos[0] <= HARBOR_HALF - PLAYER_R);
    }

    #[test]
    fn warehouse_is_walked_under_and_supports_roof_landings() {
        let level = Level::harbor();
        let p = [-36.0, 15.0];
        assert!(!blocked_in(p, 0.0, PLAYER_R, &level.obstacles, HARBOR_HALF));
        assert_eq!(support_height(p, PLAYER_R, 0.0, &level.obstacles), 0.0);
        assert_eq!(support_height(p, PLAYER_R, 4.55, &level.obstacles), 4.55);
        assert!(!segment_hits_cover(
            [-36.0, 1.45, 10.0],
            [-36.0, 1.45, 20.0],
            &level.obstacles
        ));
        assert!(segment_hits_cover(
            [-36.0, 3.9, 15.0],
            [-36.0, 4.8, 15.0],
            &level.obstacles
        ));
        let v = step_vertical(p, 2.3, 4.0, false, FIXED_DT, &level.obstacles);
        assert_eq!(v.y, WAREHOUSE_ROOF_BASE - BODY_H_STAND);
        assert!(v.bonked.is_some());
        let v = step_vertical(p, 4.56, -2.0, false, FIXED_DT, &level.obstacles);
        assert_eq!(v.y, 4.55);
        assert!(v.grounded);
    }

    #[test]
    fn both_container_climbing_chains_preserve_safe_authored_jumps() {
        let level = Level::harbor();
        for sign in [-1.0, 1.0] {
            let (mut p, mut y, mut vy, mut ground) = ([15.0 * sign, 20.0 * sign], 0.0, 0.0, true);
            for (z, height) in [(22.0, 0.55), (24.0, 1.2), (27.0, CONTAINER_H)] {
                let target = [15.0 * sign, z * sign];
                for _ in 0..600 {
                    let jump = ground && y < height - 0.001;
                    let speed =
                        movement_speed(p, y, vy, jump, false, false, false, &level.obstacles);
                    let mv = [
                        (target[0] - p[0]) / (speed * FIXED_DT),
                        (target[1] - p[1]) / (speed * FIXED_DT),
                    ];
                    p = move_circle_in(p, y, mv, speed, FIXED_DT, &level.obstacles, HARBOR_HALF);
                    let v = step_vertical(p, y, vy, jump, FIXED_DT, &level.obstacles);
                    (y, vy, ground) = (v.y, v.vy, v.grounded);
                    if ground && (y - height).abs() < 0.001 && (p[1] - target[1]).abs() < 0.05 {
                        break;
                    }
                }
                assert!(
                    ground && (y - height).abs() < 0.001,
                    "chain {sign}: {p:?} at {y}, wanted {height}"
                );
            }
        }
    }
}
