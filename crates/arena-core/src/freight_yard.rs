//! Freight Yard, the authored v18 arena: `docs/plans/arena-v18-freight-yard.md`
//! section 4.
//!
//! Its own module because `shooter.rs` is the sim and is long enough; the
//! sim reads a `Level` and never this file. A rail freight yard at golden
//! hour: two trains of closed containers run east-west, a loading dock
//! stands in the open yard between them with the king block hanging over
//! it, a walled backlot behind each train is where you spawn, and `?`
//! blocks hang in the train zone like signal boxes, two more over the
//! middle containers where only the roof network reaches them.
//!
//! Quadrant, then mirror. The north-east quadrant is authored once and
//! reflected across `x = 0` and `z = 0`; pieces that straddle one axis are
//! authored in a spine list and reflected across the other axis only; the
//! dock and the king block sit on the origin. Mirrors and not Trench City's
//! quarter turn because of the lanes: a rotation would turn the trains
//! into a ring, and mirrors keep every container parallel to x, which is
//! what gives the yard its two 48 m sightlines and boxes the train zone
//! off from the yard and the backlot. The train zone is itself a lane: its
//! blocks hang above eye level and nothing else in it stands taller than
//! 1.45, so it runs 48 m end to end like the yard, and what separates the
//! two is that a body in one cannot see into the other. That is what makes
//! the sniper and the SMG different guns here.
//!
//! Every number below is the plan's, and the invariants at the bottom are
//! what decide it: a sightline or a clearance the tests reject is moved by
//! the smallest amount and the move is recorded in the plan's deviations.

use crate::shooter::{ARENA_HALF, Cover, Decor, DecorKind, Hill, LOOT_SIZE, Level, Obstacle};

/// The dock: a jumpable stage in the open yard, `Cover::Plinth` because the
/// client draws a plinth as a flat stone slab, which is what a loading dock
/// looks like.
const DOCK: Obstacle = Obstacle::boxed(Cover::Plinth, [-4.0, -2.0], [4.0, 2.0], 0.0, 1.2);

/// The hill (v19): the dock's top, the spot the king block hangs over.
/// Spelled out rather than derived from `DOCK` so the rule and the box can
/// be moved apart on purpose and never drift apart by accident.
const YARD_HILL: Hill = Hill {
    min: [-4.0, -2.0],
    max: [4.0, 2.0],
    top: 1.2,
};

/// The king block's bottom. From the dock a 0.39 m hop; from the floor a
/// running jump that has to clear the dock's step-up line before it can
/// get under the block, so the bonk lands near the apex, a floaty and
/// deliberate jump that leaves you hanging in the open.
const KING_BASE: f32 = 3.45;

/// The roof blocks' bottom. From C2's roof (feet 2.6, head 4.46) it is
/// walked under and bonked with a 0.44 m rise; from the floor the head
/// reaches 3.55 at most, so it is a roof-only reward, and nothing in the
/// yard reaches its top (feet would need 5.55).
const ROOF_BASE: f32 = 4.9;

/// The train-zone blocks' bottom. Walked under with 0.44 m of headroom
/// (`BODY_H_STAND` 1.86), and a floor jump meets it a few ticks after the
/// press: the Mario snap.
const TRAIN_BASE: f32 = 2.30;

/// A loot block: one metre on a side, centred on `(x, z)`, hung with its
/// bottom at `base`.
const fn loot(x: f32, z: f32, base: f32) -> Obstacle {
    Obstacle::boxed(
        Cover::Loot,
        [x - LOOT_SIZE * 0.5, z - LOOT_SIZE * 0.5],
        [x + LOOT_SIZE * 0.5, z + LOOT_SIZE * 0.5],
        base,
        base + LOOT_SIZE,
    )
}

/// Section 4.2, emitted once: the dock, and the king block over it.
const CENTRE: [Obstacle; 2] = [DOCK, loot(0.0, 0.0, KING_BASE)];

/// Section 4.3, the spine along x: authored for `x > 0`, straddling
/// `z = 0`, mirrored in x. The yard wagon splits each yard lane and blocks
/// the `x = 5.5` spawn-to-spawn line; the crate and the ammo box are its
/// climbing chain (gaps 0.4); the rubble dresses the flank.
const SPINE_X: [Obstacle; 4] = [
    Obstacle::boxed(Cover::Container, [4.5, -1.2], [10.5, 1.2], 0.0, 2.6),
    Obstacle::boxed(Cover::Crate, [10.9, -0.6], [12.1, 0.6], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [12.5, -0.4], [13.5, 0.4], 0.0, 0.55),
    Obstacle::boxed(Cover::Rubble, [19.0, -1.0], [21.0, 1.0], 0.0, 0.7),
];

/// Section 4.4, the spine along z: authored for `z > 0`, straddling
/// `x = 0`, mirrored in z. C1 blocks the yard's centre from the backlot
/// diagonals; C2 carries the roof block; C3 stops the two spawns of a
/// backlot seeing each other across `x = 0`.
const SPINE_Z: [Obstacle; 4] = [
    Obstacle::boxed(Cover::Container, [-3.6, 5.0], [3.6, 7.4], 0.0, 2.6),
    Obstacle::boxed(Cover::Container, [-3.6, 14.0], [3.6, 16.4], 0.0, 2.6),
    loot(0.0, 15.2, ROOF_BASE),
    Obstacle::boxed(Cover::Container, [-1.2, 18.6], [1.2, 22.6], 0.0, 2.6),
];

/// Section 4.5, the north-east quadrant, mirrored into all four. In the
/// plan's order: Q1 and the two chains beside the crossing, Q2 and its
/// chain, the stacked pair, Q4 and its chain, C3's chain, the two
/// sandbags, the flank rubble, the train-zone block. Eighteen rows: the
/// plan's tally says nineteen and 94 boxes, but its table has eighteen
/// and the yard is 90.
///
/// One box is not where the plan put it. The plan hung Q4's ammo box at
/// `10.8..11.6 x 16.4..17.2`, flush against Q2's north face and 0.4 from
/// the chain crate at `z 17.6`: a 1.2 m slot between two boxes that both
/// stop a body standing on the ammo (`blocked` is judged on the standing
/// head against a container and on `STEP_UP` against the crate), and a
/// body is 1.2 m wide, so the step could be stood on along one line in
/// exact arithmetic and nowhere in play. The climbing-chain invariant is
/// what caught it. It now sits beside the crate instead of between the
/// crate and the train, 0.4 from the crate's west face and 1.0 from Q2, and
/// the chain reads floor, ammo, crate, Q4 exactly as the other six do.
const QUADRANT: [Obstacle; 18] = [
    Obstacle::boxed(Cover::Container, [7.6, 5.0], [13.6, 7.4], 0.0, 2.6),
    Obstacle::boxed(Cover::Crate, [4.0, 6.2], [5.2, 7.4], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [4.0, 7.8], [5.0, 8.6], 0.0, 0.55),
    Obstacle::boxed(Cover::Crate, [14.0, 6.2], [15.2, 7.4], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [14.0, 7.8], [15.0, 8.6], 0.0, 0.55),
    Obstacle::boxed(Cover::Container, [7.6, 14.0], [14.0, 16.4], 0.0, 2.6),
    Obstacle::boxed(Cover::Crate, [14.4, 15.2], [15.6, 16.4], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [14.4, 16.8], [15.4, 17.6], 0.0, 0.55),
    Obstacle::boxed(Cover::Container, [17.6, 14.0], [23.6, 16.4], 0.0, 5.2),
    Obstacle::boxed(Cover::Container, [10.2, 19.0], [12.6, 23.0], 0.0, 2.6),
    Obstacle::boxed(Cover::Ammo, [9.6, 17.4], [10.4, 18.2], 0.0, 0.55),
    Obstacle::boxed(Cover::Crate, [10.8, 17.6], [12.0, 18.8], 0.0, 1.2),
    Obstacle::boxed(Cover::Crate, [1.6, 18.6], [2.8, 19.8], 0.0, 1.2),
    Obstacle::boxed(Cover::Ammo, [3.0, 18.6], [3.8, 19.4], 0.0, 0.55),
    Obstacle::boxed(Cover::Sandbag, [4.0, 18.0], [8.0, 18.8], 0.0, 1.1),
    Obstacle::boxed(Cover::Sandbag, [15.0, 18.0], [19.0, 18.8], 0.0, 1.1),
    Obstacle::boxed(Cover::Rubble, [20.0, 8.0], [22.0, 10.0], 0.0, 0.7),
    loot(6.5, 10.1, TRAIN_BASE),
];

/// The north-east quadrant's spawns, mirrored into the other three. Listed
/// so that slots `0..4` land in four different quadrants: the x-mirror pair
/// of one spawn is 11.0 apart with the backlot divider between them, and
/// the same-backlot pair 13.5 apart.
const QUADRANT_SPAWNS: [[f32; 2]; 2] = [[5.5, 21.5], [19.0, 21.5]];

/// The four images of a quadrant piece, in the order the spawns need: the
/// first four spawns are one per quadrant only if consecutive images land
/// in different quadrants.
const QUADRANT_SIGNS: [[f32; 2]; 4] = [[1.0, 1.0], [-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0]];

/// How many boxes the yard emits: the centre once, each spine twice, the
/// quadrant four times.
const BOX_COUNT: usize = CENTRE.len() + 2 * SPINE_X.len() + 2 * SPINE_Z.len() + 4 * QUADRANT.len();

/// A reflection of a box, `sx` and `sz` each `1.0` or `-1.0`. Both corners
/// are mapped and min/max re-derived: negating an axis swaps min and max on
/// it, so mapping the corners componentwise would emit a box with
/// `min > max` (the trap `docs/asset-pipeline.md` warns about, and the same
/// lesson as Trench City's `rot90_box`).
const fn reflect(o: &Obstacle, sx: f32, sz: f32) -> Obstacle {
    let a = [o.min[0] * sx, o.min[1] * sz];
    let b = [o.max[0] * sx, o.max[1] * sz];
    Obstacle {
        min: [a[0].min(b[0]), a[1].min(b[1])],
        max: [a[0].max(b[0]), a[1].max(b[1])],
        ..*o
    }
}

/// The box's image across `x = 0`.
const fn mirror_x(o: &Obstacle) -> Obstacle {
    reflect(o, -1.0, 1.0)
}

/// The box's image across `z = 0`.
const fn mirror_z(o: &Obstacle) -> Obstacle {
    reflect(o, 1.0, -1.0)
}

impl Level {
    /// The authored v18 arena: section 4's tables transcribed above,
    /// mirror-symmetric in x and in z. Obstacle order is the emission
    /// order (centre, spine x, spine z, quadrant), and it matters: every
    /// `Cover::Loot` box becomes a `LootBlock` in this order and
    /// `State.loot` is index-aligned with it on the wire. The dock is
    /// obstacle 0 and the king block obstacle 1.
    #[must_use]
    pub fn freight_yard() -> Self {
        let mut obstacles = Vec::with_capacity(BOX_COUNT);
        obstacles.extend_from_slice(&CENTRE);
        for o in &SPINE_X {
            obstacles.push(*o);
            obstacles.push(mirror_x(o));
        }
        for o in &SPINE_Z {
            obstacles.push(*o);
            obstacles.push(mirror_z(o));
        }
        for o in &QUADRANT {
            for [sx, sz] in QUADRANT_SIGNS {
                obstacles.push(reflect(o, sx, sz));
            }
        }
        let mut spawns = Vec::with_capacity(2 * QUADRANT_SPAWNS.len());
        for s in QUADRANT_SPAWNS {
            for [sx, sz] in QUADRANT_SIGNS {
                spawns.push([s[0] * sx, s[1] * sz]);
            }
        }
        Self {
            arena_half: ARENA_HALF,
            obstacles,
            spawns,
            pads: Vec::new(),
            decor: freight_yard_decor(),
            hill: Some(YARD_HILL),
        }
    }
}

/// Freight Yard's decor, plan section 4.5.
///
/// Twelve facades on the radius-44 ring every 30 degrees facing in, the
/// cathedral's slot filled because there is no cathedral and no statue;
/// lamps at `(+-26, 0, +-8)` and `(+-8, 0, +-26)`; wrecks in the four
/// corners and two more on the flanks at `(+-27, 0, 0)` turned a quarter,
/// where they read as rolling stock. Sky, ground, cobble floor and city
/// wall are v13's; nothing new is generated. `sin_cos` is fine for
/// determinism: decor is client-only and never reaches the sim.
fn freight_yard_decor() -> Vec<Decor> {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
    const RING_R: f32 = 44.0;
    let mut decor = Vec::with_capacity(12 + 8 + 6);
    for slot in 0..12u8 {
        let phi = f32::from(slot) * (TAU / 12.0);
        let (s, c) = phi.sin_cos();
        decor.push(Decor {
            kind: if slot.is_multiple_of(2) {
                DecorKind::FacadeA
            } else {
                DecorKind::FacadeB
            },
            pos: [RING_R * s, 0.0, RING_R * c],
            yaw: (phi + PI) % TAU,
            scale: 18.0,
        });
    }
    for [x, z] in [[26.0, 8.0], [8.0, 26.0]] {
        for [sx, sz] in QUADRANT_SIGNS {
            decor.push(Decor {
                kind: DecorKind::Lamp,
                pos: [x * sx, 0.0, z * sz],
                yaw: 0.0,
                scale: 5.0,
            });
        }
    }
    for [sx, sz] in QUADRANT_SIGNS {
        decor.push(Decor {
            kind: DecorKind::Wreck,
            pos: [27.0 * sx, 0.0, 27.0 * sz],
            yaw: FRAC_PI_4,
            scale: 1.5,
        });
    }
    for sx in [1.0, -1.0] {
        decor.push(Decor {
            kind: DecorKind::Wreck,
            pos: [27.0 * sx, 0.0, 0.0],
            yaw: FRAC_PI_2,
            scale: 1.5,
        });
    }
    decor
}

/// The drivers the map invariants are proven with: a player made of the
/// sim's own `move_circle` and `step_vertical`, and a shot made of the
/// sim's own round. `climb`, `hop`, `centre`, `gap`, `contains` and `dist`
/// began as Trench City's (v13) drivers and live here now; `shooter.rs`'s
/// test module imports them from this module, so both maps run one driver.
/// Only `shot_over` exists twice: the one here takes a `Level`, and the
/// one in `shooter.rs` takes a bare obstacle list and a target height,
/// which its tests need and the map invariants do not.
///
/// New here: `perch`, which finds where a body can actually stand on a
/// box, because a chain step hemmed by a taller neighbour within a body's
/// radius is not a step; `floor_start`, which finds open floor beside a
/// box in the first free direction; and `standable_floor`, which asks
/// `move_circle` (rather than a copy of the private `blocked`) whether the
/// sim lets a body stand at a point.
#[cfg(test)]
pub(crate) mod level_helpers {
    // Grid indices are small and non-negative by construction.
    #![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]

    use crate::shooter::{
        BODY_H_STAND, FIXED_DT, GameMode, Level, MAX_HP, MOVE_SPEED, Obstacle, PLAYER_R, PlayerIn,
        STEP_UP, Sim, VStep, move_circle, step_vertical, support_height,
    };
    use std::collections::HashMap;

    pub fn centre(o: &Obstacle) -> [f32; 2] {
        [
            f32::midpoint(o.min[0], o.max[0]),
            f32::midpoint(o.min[1], o.max[1]),
        ]
    }

    pub fn contains(o: &Obstacle, p: [f32; 2]) -> bool {
        p[0] >= o.min[0] && p[0] <= o.max[0] && p[1] >= o.min[1] && p[1] <= o.max[1]
    }

    /// Axis-separated distance between two footprints; 0 when they touch
    /// or overlap.
    pub fn gap(a: &Obstacle, b: &Obstacle) -> f32 {
        let gx = (a.min[0] - b.max[0]).max(b.min[0] - a.max[0]);
        let gz = (a.min[1] - b.max[1]).max(b.min[1] - a.max[1]);
        gx.max(gz).max(0.0)
    }

    pub fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
        let (dx, dz) = (a[0] - b[0], a[1] - b[1]);
        (dx * dx + dz * dz).sqrt()
    }

    /// Distance from a point to a footprint; 0 inside it. A body of radius
    /// `r` overlaps the box exactly when this is below `r`, which is the
    /// sim's `overlaps` written as a distance.
    pub fn box_dist(p: [f32; 2], o: &Obstacle) -> f32 {
        let cx = p[0].clamp(o.min[0], o.max[0]);
        let cz = p[1].clamp(o.min[1], o.max[1]);
        dist(p, [cx, cz])
    }

    /// Whether the sim lets a standing body occupy `p` on the floor, asked
    /// of `move_circle` itself: a one-step move of exactly 0.2 m from west
    /// of `p` that arrives is a `p` the sim's `blocked` accepts at `y = 0`.
    /// The speed is chosen so `speed * FIXED_DT` is the grid step.
    pub fn standable_floor(p: [f32; 2], obs: &[Obstacle]) -> bool {
        let from = [p[0] - 0.2, p[1]];
        let to = move_circle(from, 0.0, [1.0, 0.0], 12.0, FIXED_DT, obs);
        to[0] > from[0]
    }

    /// Open floor with a margin: inside the wall and at least a body's
    /// radius plus 0.1 from every box a standing body could hit. The
    /// margin is what keeps a test start off the knife edge where two
    /// floats decide whether a circle touches a corner.
    pub fn open_floor(p: [f32; 2], obs: &[Obstacle]) -> bool {
        let inside = crate::shooter::ARENA_HALF - PLAYER_R - 0.1;
        p[0].abs() <= inside
            && p[1].abs() <= inside
            && obs
                .iter()
                .filter(|o| o.base < BODY_H_STAND)
                .all(|o| box_dist(p, o) >= PLAYER_R + 0.1)
    }

    /// Open floor near `from`: 1.4 out along the first of `dirs` that is
    /// open, then 2.0 out. Panics when none is, because a chain nobody can
    /// walk up to is not a chain.
    pub fn floor_start(from: [f32; 2], dirs: &[[f32; 2]], obs: &[Obstacle]) -> [f32; 2] {
        for reach in [1.4, 2.0] {
            for d in dirs {
                let p = [from[0] + d[0] * reach, from[1] + d[1] * reach];
                if open_floor(p, obs) {
                    return p;
                }
            }
        }
        panic!("no open floor within 2.0 of {from:?} in {dirs:?}");
    }

    /// Where a body can stand on `b`: the point nearest its centre, on a
    /// 0.1 m grid over the footprint grown by 0.3, whose circle is well
    /// over the box, is supported by exactly this box's top, and is not
    /// hemmed by a neighbour that `blocked` would stop a body of this
    /// height against (a taller box within a radius, or a raised box its
    /// head would meet). Deterministic: the scan order breaks ties.
    pub fn perch(b: &Obstacle, obs: &[Obstacle]) -> [f32; 2] {
        let c = centre(b);
        let grow = 0.3;
        let step = 0.1;
        let nx = ((b.max[0] - b.min[0] + 2.0 * grow) / step).round() as i32;
        let nz = ((b.max[1] - b.min[1] + 2.0 * grow) / step).round() as i32;
        let mut best: Option<([f32; 2], f32)> = None;
        for i in 0..=nx {
            for j in 0..=nz {
                let p = [
                    b.min[0] - grow + step * i as f32,
                    b.min[1] - grow + step * j as f32,
                ];
                if box_dist(p, b) > PLAYER_R - 0.05 {
                    continue;
                }
                if (support_height(p, PLAYER_R, b.h, obs) - b.h).abs() > 1e-6 {
                    continue;
                }
                let hemmed = obs.iter().any(|o| {
                    o != b
                        && box_dist(p, o) < PLAYER_R + 0.05
                        && b.h < o.h - STEP_UP
                        && b.h + BODY_H_STAND > o.base
                });
                if hemmed {
                    continue;
                }
                let d = dist(p, c);
                if best.is_none_or(|(_, bd)| d < bd - 1e-6) {
                    best = Some((p, d));
                }
            }
        }
        best.unwrap_or_else(|| panic!("no standable point on {b:?}"))
            .0
    }

    /// Drives the sim's own `move_circle` / `step_vertical` toward `target`
    /// at `target_h`, hopping whenever grounded below that height, exactly
    /// as a player would. Returns the resting (pos, y) once standing at the
    /// target height within reach of the point, or `None` after `ticks`.
    pub fn climb(
        mut pos: [f32; 2],
        mut y: f32,
        target: [f32; 2],
        target_h: f32,
        ticks: u32,
        obs: &[Obstacle],
    ) -> Option<([f32; 2], f32)> {
        let (mut vy, mut grounded) = (0.0f32, true);
        let per_tick = MOVE_SPEED * FIXED_DT;
        for _ in 0..ticks {
            // Intent scaled so the last step lands on the point instead of
            // overshooting it; move_circle clamps anything longer to unit.
            let mv = [
                (target[0] - pos[0]) / per_tick,
                (target[1] - pos[1]) / per_tick,
            ];
            let jump = grounded && y < target_h - 1e-3;
            pos = move_circle(pos, y, mv, MOVE_SPEED, FIXED_DT, obs);
            let VStep {
                y: ny,
                vy: nvy,
                grounded: g,
                ..
            } = step_vertical(pos, y, vy, jump, FIXED_DT, obs);
            y = ny;
            vy = nvy;
            grounded = g;
            if dist(pos, target) < 0.05 && grounded && (y - target_h).abs() < 1e-3 {
                return Some((pos, y));
            }
        }
        None
    }

    /// `climb` on the floor with no jumping: where a body walking toward
    /// `target` at `y 0` ends up, or `None` when it never arrives.
    pub fn walk(
        mut pos: [f32; 2],
        target: [f32; 2],
        ticks: u32,
        obs: &[Obstacle],
    ) -> Option<[f32; 2]> {
        let per_tick = MOVE_SPEED * FIXED_DT;
        for _ in 0..ticks {
            let mv = [
                (target[0] - pos[0]) / per_tick,
                (target[1] - pos[1]) / per_tick,
            ];
            pos = move_circle(pos, 0.0, mv, MOVE_SPEED, FIXED_DT, obs);
            if dist(pos, target) < 0.05 {
                return Some(pos);
            }
        }
        None
    }

    /// One jump from a standing start: take off on tick 0 with the stick
    /// held toward `target`, let go once past it, and report where the body
    /// comes to rest. Unlike `climb` it never jumps again, so whatever it
    /// reaches, one hop reaches.
    pub fn hop(
        mut pos: [f32; 2],
        mut y: f32,
        target: [f32; 2],
        obs: &[Obstacle],
    ) -> ([f32; 2], f32) {
        let d = dist(pos, target);
        let dir = [(target[0] - pos[0]) / d, (target[1] - pos[1]) / d];
        let mut vy = 0.0f32;
        for tick in 0..300 {
            let ahead = (target[0] - pos[0]) * dir[0] + (target[1] - pos[1]) * dir[1];
            let mv = if ahead > 0.0 { dir } else { [0.0, 0.0] };
            pos = move_circle(pos, y, mv, MOVE_SPEED, FIXED_DT, obs);
            let VStep {
                y: ny,
                vy: nvy,
                grounded,
                ..
            } = step_vertical(pos, y, vy, tick == 0, FIXED_DT, obs);
            y = ny;
            vy = nvy;
            if tick > 0 && grounded {
                break;
            }
        }
        (pos, y)
    }

    /// The highest the feet get on one jump from open floor, as the sim's
    /// integrator computes it (1.6867, not the continuous 1.7633).
    pub fn jump_apex() -> f32 {
        let (mut y, mut vy, mut peak) = (0.0f32, 0.0f32, 0.0f32);
        for tick in 0..120 {
            let v = step_vertical([0.0, 0.0], y, vy, tick == 0, FIXED_DT, &[]);
            y = v.y;
            vy = v.vy;
            peak = peak.max(y);
        }
        peak
    }

    pub fn step_with(sim: &mut Sim, inputs: &HashMap<u8, PlayerIn>) {
        sim.step(&|id| inputs.get(&id).copied().unwrap_or_default());
    }

    /// One shot on `level` from `from` at `from_y`, elevation `pitch`, at a
    /// target standing on the floor at `to`; both pinned every tick.
    /// Whether it connected within two seconds. The sidearm's round, so
    /// the ray is exactly the aim: spread would make a sightline a die
    /// roll.
    ///
    /// The shooter holds the sidearm, whose round expires at 54 m
    /// (`BULLET_TTL` 1.6 s at `BULLET_SPEED`), which is short of the far
    /// spawn pairs of a 48 m arena; a test that could not reach a pair could
    /// not fail for it. So every round in flight is held at 4 s of life on
    /// every tick, which is to say it never expires inside the driver's
    /// 180-tick window, and the window itself is the range: 3 s at 34 m/s
    /// is 102 m, half again the diagonal. This asks whether the geometry
    /// blocks the line, not whether the sidearm reaches it.
    pub fn shot_over(level: &Level, from: [f32; 2], from_y: f32, pitch: f32, to: [f32; 2]) -> bool {
        let mut sim = Sim::from_level(level, 0, GameMode::Ffa);
        sim.add_player(0);
        sim.add_player(1);
        let aim = [(to[0] - from[0]), (to[1] - from[1])];
        let mut inputs = HashMap::new();
        inputs.insert(
            0,
            PlayerIn {
                aim,
                pitch,
                fire: true,
                ..Default::default()
            },
        );
        inputs.insert(1, PlayerIn::default());
        for _ in 0..180 {
            sim.players.iter_mut().for_each(|p| match p.id {
                0 if p.alive => {
                    p.pos = from;
                    p.y = from_y;
                    p.vy = 0.0;
                }
                1 if p.alive => {
                    p.pos = to;
                    p.y = 0.0;
                    p.vy = 0.0;
                }
                _ => {}
            });
            step_with(&mut sim, &inputs);
            for b in &mut sim.bullets {
                b.ttl = 4.0;
            }
            if sim.players.iter().find(|p| p.id == 1).unwrap().hp < MAX_HP {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    // Test-only casts use small fixed ranges.
    #![allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]

    use super::level_helpers::{
        box_dist, centre, climb, contains, dist, floor_start, gap, hop, jump_apex, open_floor,
        perch, shot_over, standable_floor, walk,
    };
    use super::*;
    use crate::shooter::{
        BODY_H_STAND, FIXED_DT, GameMode, MAP_FREIGHT_YARD, MOVE_SPEED, PLAYER_R, STEP_UP, Sim,
        move_circle, stance_speed, step_vertical, support_height,
    };

    /// The frozen v18 Trench City, written once from `Level::trench_city()`
    /// with `serde_json::to_string_pretty` and checked in, so that a change
    /// to the sim's types or to the serialiser is caught as loudly as a
    /// moved box. Regenerated by `write_trench_city_fixture` when the map
    /// is meant to change, and that is a protocol question every time: the
    /// v18 blocks replaced the v13 pads here, and that is what took
    /// `PROTO_VERSION` from 14 to 15.
    const TRENCH_CITY_V18: &str = include_str!("../tests/fixtures/trench-city-v18.json");

    fn yard() -> Level {
        Level::freight_yard()
    }

    fn find(level: &Level, kind: Cover, min: [f32; 2]) -> (usize, &Obstacle) {
        level
            .obstacles
            .iter()
            .enumerate()
            .find(|(_, o)| o.kind == kind && dist(o.min, min) < 1e-4)
            .unwrap_or_else(|| panic!("no {kind:?} at {min:?}"))
    }

    fn loot_blocks(level: &Level) -> Vec<(usize, &Obstacle)> {
        level
            .obstacles
            .iter()
            .enumerate()
            .filter(|(_, o)| o.kind == Cover::Loot)
            .collect()
    }

    /// The v18 skeleton's check, kept: the name resolves, the dock is
    /// obstacle 0, the king block obstacle 1, and the sim arms one block
    /// per `Cover::Loot` in obstacle order with no pads.
    #[test]
    fn the_yard_is_named_and_carries_its_centre() {
        let level = yard();
        assert_eq!(Level::named(MAP_FREIGHT_YARD, 0), level);
        assert_eq!(level.arena_half, ARENA_HALF);
        let dock = &level.obstacles[0];
        assert_eq!(dock.kind, Cover::Plinth);
        assert_eq!((dock.base, dock.h), (0.0, 1.2));
        let king = &level.obstacles[1];
        assert_eq!(king.kind, Cover::Loot);
        assert_eq!(king.min, [-0.5, -0.5]);
        assert_eq!(king.max, [0.5, 0.5]);
        assert_eq!((king.base, king.h), (KING_BASE, KING_BASE + LOOT_SIZE));
        let sim = Sim::from_level(&level, 7, GameMode::Ffa);
        assert_eq!(sim.seed, 7);
        let blocks = loot_blocks(&level);
        assert_eq!(sim.loot.len(), blocks.len());
        for (slot, (index, _)) in sim.loot.iter().zip(&blocks) {
            assert_eq!(slot.obstacle, *index);
            assert_eq!(slot.respawn_t, 0.0);
        }
        assert!(sim.pads.is_empty(), "the yard has no pads");
    }

    // 1
    #[test]
    fn yard_has_eight_clear_spawns() {
        let level = yard();
        assert_eq!(level.spawns.len(), 8);
        let inside = ARENA_HALF - PLAYER_R;
        for (i, s) in level.spawns.iter().enumerate() {
            assert!(
                s[0].abs() < inside && s[1].abs() < inside,
                "spawn {i} {s:?} is outside the arena"
            );
            for o in &level.obstacles {
                let d = box_dist(*s, o);
                assert!(d >= PLAYER_R, "spawn {i} {s:?} overlaps {o:?}");
                assert!(d >= 2.0, "spawn {i} {s:?} is only {d} from {o:?}");
            }
            for (j, t) in level.spawns.iter().enumerate().skip(i + 1) {
                let d = dist(*s, *t);
                assert!(d >= 11.0, "spawns {i} and {j} are only {d} apart");
            }
        }
        // The sim places players there in slot order, and the first four
        // slots land in four different quadrants.
        let mut sim = Sim::from_level(&level, 0, GameMode::Ffa);
        for id in 0..8u8 {
            sim.add_player(id);
        }
        for (p, s) in sim.players.iter().zip(&level.spawns) {
            assert_eq!(p.pos, *s);
        }
        let mut quadrants: Vec<(bool, bool)> = level.spawns[..4]
            .iter()
            .map(|s| (s[0] > 0.0, s[1] > 0.0))
            .collect();
        quadrants.sort_unstable();
        quadrants.dedup();
        assert_eq!(quadrants.len(), 4, "slots 0..4 must be one per quadrant");
    }

    // 2
    #[test]
    fn yard_boxes_are_inside_and_well_formed() {
        let level = yard();
        assert_eq!(level.obstacles.len(), BOX_COUNT);
        assert_eq!(level.obstacles.len(), 90);
        assert_eq!(loot_blocks(&level).len(), 7);
        assert!(
            level.pads.is_empty(),
            "the yard has no pads: {:?}",
            level.pads
        );
        for o in &level.obstacles {
            assert!(
                o.min[0] < o.max[0] && o.min[1] < o.max[1],
                "inverted or degenerate box {o:?}"
            );
            assert!(
                o.min[0] >= -ARENA_HALF
                    && o.min[1] >= -ARENA_HALF
                    && o.max[0] <= ARENA_HALF
                    && o.max[1] <= ARENA_HALF,
                "{o:?} leaves the arena"
            );
            assert!(o.base < o.h, "{o:?} has its bottom above its top");
            assert!(o.base >= 0.0, "{o:?} starts below the floor");
            if o.kind == Cover::Loot {
                assert!(
                    (o.max[0] - o.min[0] - LOOT_SIZE).abs() < 1e-5
                        && (o.max[1] - o.min[1] - LOOT_SIZE).abs() < 1e-5
                        && (o.h - o.base - LOOT_SIZE).abs() < 1e-5,
                    "block {o:?} is not a unit cube"
                );
                assert!(
                    [TRAIN_BASE, KING_BASE, ROOF_BASE].contains(&o.base),
                    "block {o:?} hangs at an unplanned height"
                );
            } else {
                assert_eq!(o.base, 0.0, "{o:?}: only blocks leave the floor");
            }
        }
        // No two boxes share any volume: footprints may nest (a block over
        // the dock, a block over C2) only when their height ranges do not.
        for (i, a) in level.obstacles.iter().enumerate() {
            for b in &level.obstacles[i + 1..] {
                let xz = a.min[0] < b.max[0]
                    && b.min[0] < a.max[0]
                    && a.min[1] < b.max[1]
                    && b.min[1] < a.max[1];
                let y = a.base < b.h && b.base < a.h;
                assert!(!(xz && y), "{a:?} and {b:?} overlap");
            }
        }
    }

    /// Section 4.5's decor list, counted: twelve facades facing in, eight
    /// lamps, six wrecks, nothing else.
    #[test]
    fn the_yard_decor_is_facades_lamps_and_wrecks() {
        use std::f32::consts::PI;
        let decor = yard().decor;
        let count = |k: DecorKind| decor.iter().filter(|d| d.kind == k).count();
        assert_eq!(count(DecorKind::FacadeA), 6);
        assert_eq!(count(DecorKind::FacadeB), 6);
        assert_eq!(count(DecorKind::Lamp), 8);
        assert_eq!(count(DecorKind::Wreck), 6);
        assert_eq!(count(DecorKind::Statue), 0);
        assert_eq!(count(DecorKind::Cathedral), 0);
        assert_eq!(decor.len(), 26);
        for d in &decor {
            let r = (d.pos[0] * d.pos[0] + d.pos[2] * d.pos[2]).sqrt();
            assert!(r > ARENA_HALF, "{d:?} is inside the arena");
            assert_eq!(d.pos[1], 0.0, "{d:?} is off the ground");
            if matches!(d.kind, DecorKind::FacadeA | DecorKind::FacadeB) {
                assert!((r - 44.0).abs() < 1e-3, "{d:?} is off the ring");
                // Facing in: the facing direction (sin yaw, cos yaw) points
                // back at the origin.
                let facing = [d.yaw.sin(), d.yaw.cos()];
                let to_centre = [-d.pos[0] / r, -d.pos[2] / r];
                let dot = facing[0] * to_centre[0] + facing[1] * to_centre[1];
                assert!(dot > 0.999, "{d:?} does not face the centre");
            }
        }
        let wrecks: Vec<&Decor> = decor
            .iter()
            .filter(|d| d.kind == DecorKind::Wreck)
            .collect();
        assert_eq!(
            wrecks
                .iter()
                .filter(|d| (d.yaw - PI / 2.0).abs() < 1e-6)
                .count(),
            2,
            "two wrecks read as rolling stock on the flanks"
        );
    }

    // 3
    #[test]
    fn yard_is_mirror_symmetric_in_x_and_z() {
        let level = yard();
        // The mirrors are written out here rather than borrowed from the
        // builder, so a builder that reflects wrongly cannot agree with
        // itself. Both corners are mapped and min/max re-derived.
        let flip = |o: &Obstacle, sx: f32, sz: f32| {
            let (ax, az) = (o.min[0] * sx, o.min[1] * sz);
            let (bx, bz) = (o.max[0] * sx, o.max[1] * sz);
            Obstacle {
                min: [ax.min(bx), az.min(bz)],
                max: [ax.max(bx), az.max(bz)],
                ..*o
            }
        };
        let key = |o: &Obstacle| -> [f32; 7] {
            [
                f32::from(o.kind as u8),
                o.min[0],
                o.min[1],
                o.max[0],
                o.max[1],
                o.base,
                o.h,
            ]
        };
        let order = |x: &[f32; 7], y: &[f32; 7]| {
            x.iter()
                .zip(y)
                .map(|(p, q)| p.total_cmp(q))
                .find(|c| c.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        };
        let closed_under = |boxes: &[&Obstacle], sx: f32, sz: f32, what: &str| {
            let mut a: Vec<[f32; 7]> = boxes.iter().map(|o| key(o)).collect();
            let mut b: Vec<[f32; 7]> = boxes.iter().map(|o| key(&flip(o, sx, sz))).collect();
            a.sort_by(order);
            b.sort_by(order);
            for (x, y) in a.iter().zip(&b) {
                for k in 0..7 {
                    assert!(
                        (x[k] - y[k]).abs() < 1e-4,
                        "{what} is not closed under the mirror ({sx}, {sz}): {x:?} vs {y:?}"
                    );
                }
            }
        };
        let all: Vec<&Obstacle> = level.obstacles.iter().collect();
        let loot: Vec<&Obstacle> = loot_blocks(&level).into_iter().map(|(_, o)| o).collect();
        for (sx, sz) in [(-1.0, 1.0), (1.0, -1.0)] {
            closed_under(&all, sx, sz, "the obstacle multiset");
            closed_under(&loot, sx, sz, "the loot subset");
            for s in &level.spawns {
                let q = [s[0] * sx, s[1] * sz];
                assert!(
                    level.spawns.iter().any(|r| dist(*r, q) < 1e-4),
                    "spawn {s:?} mirrored to {q:?} is not a spawn"
                );
            }
        }
    }

    // 4
    #[test]
    fn yard_lists_are_in_their_half_planes() {
        for o in &CENTRE {
            assert!(
                o.min[0] < 0.0 && o.max[0] > 0.0 && o.min[1] < 0.0 && o.max[1] > 0.0,
                "centre piece {o:?} does not sit on the origin"
            );
        }
        for o in &SPINE_X {
            assert!(o.min[0] > 0.0, "spine-x piece {o:?} is not in x > 0");
            assert!(
                o.min[1] < 0.0 && o.max[1] > 0.0,
                "spine-x piece {o:?} does not straddle z = 0"
            );
        }
        for o in &SPINE_Z {
            assert!(o.min[1] > 0.0, "spine-z piece {o:?} is not in z > 0");
            assert!(
                o.min[0] < 0.0 && o.max[0] > 0.0,
                "spine-z piece {o:?} does not straddle x = 0"
            );
        }
        for o in &QUADRANT {
            assert!(
                o.min[0] > 0.0 && o.min[1] > 0.0,
                "quadrant piece {o:?} is not in the north-east quadrant"
            );
        }
        for s in &QUADRANT_SPAWNS {
            assert!(s[0] > 0.0 && s[1] > 0.0, "spawn {s:?} is not north-east");
        }
        assert_eq!(BOX_COUNT, 2 + 8 + 8 + 72);
        // And so no box is emitted twice.
        let level = yard();
        for (i, a) in level.obstacles.iter().enumerate() {
            for b in &level.obstacles[i + 1..] {
                assert_ne!(a, b, "{a:?} was emitted twice");
            }
        }
    }

    // 5
    #[test]
    fn no_two_raised_boxes_share_a_footprint() {
        // Both shipped levels: the ceiling clamp reports the lowest-base
        // box of those over a head, and it is bit-identical to the old
        // "last in list order" rule only while no two raised boxes ever
        // overlap in plan.
        for (name, level) in [
            ("trench-city", Level::trench_city()),
            ("freight-yard", yard()),
        ] {
            let raised: Vec<&Obstacle> = level.obstacles.iter().filter(|o| o.base > 0.0).collect();
            assert!(!raised.is_empty(), "{name} has no raised box");
            for (i, a) in raised.iter().enumerate() {
                for b in &raised[i + 1..] {
                    assert!(
                        gap(a, b) > 0.0,
                        "{name}: raised boxes {a:?} and {b:?} share a footprint"
                    );
                }
            }
        }
    }

    /// Climbs `c` by its chain (floor -> ammo -> crate -> roof) with the
    /// shared driver and returns where the body rests on the roof, or
    /// panics naming the step that failed. Also proves the floor alone
    /// never gets on top.
    fn climb_by_chain(level: &Level, c: &Obstacle) -> ([f32; 2], f32) {
        let obs = &level.obstacles;
        let chain = obs
            .iter()
            .filter(|k| k.kind == Cover::Crate && gap(k, c) <= 0.5)
            .find_map(|k| {
                obs.iter()
                    .find(|a| a.kind == Cover::Ammo && gap(a, k) <= 0.5)
                    .map(|a| (k, a))
            });
        let Some((step, ammo)) = chain else {
            panic!("container {c:?} has no crate within 0.5 with an ammo box within 0.5 of it");
        };
        let (ac, kc, cc) = (perch(ammo, obs), perch(step, obs), perch(c, obs));
        // Start on open floor beyond the ammo box, away from the crate if
        // that is open and beside it otherwise.
        let d = dist(ac, kc);
        let away = [(ac[0] - kc[0]) / d, (ac[1] - kc[1]) / d];
        let side = [-away[1], away[0]];
        let start = floor_start(
            ac,
            &[away, side, [-side[0], -side[1]], [-away[0], -away[1]]],
            obs,
        );
        let (pos, y) = climb(start, 0.0, ac, ammo.h, 600, obs)
            .unwrap_or_else(|| panic!("floor -> ammo failed for {c:?} from {start:?}"));
        let (pos, y) = climb(pos, y, kc, step.h, 600, obs)
            .unwrap_or_else(|| panic!("ammo -> crate failed for {c:?}"));
        let (top, y) = climb(pos, y, cc, c.h, 600, obs)
            .unwrap_or_else(|| panic!("crate -> container failed for {c:?}"));
        assert!((y - c.h).abs() < 1e-3, "ended with feet at {y}");

        // And the floor alone is not enough: from open floor on the far
        // side of the container (or beside it when the far side is not
        // open floor), the same driver never gets on top.
        let toward = [-away[0], -away[1]];
        let half = |dir: [f32; 2]| {
            if dir[0].abs() > dir[1].abs() {
                (c.max[0] - c.min[0]) * 0.5
            } else {
                (c.max[1] - c.min[1]) * 0.5
            }
        };
        let far = [toward, side, [-side[0], -side[1]]]
            .into_iter()
            .map(|dir| {
                let reach = half(dir) + 1.4;
                [cc[0] + dir[0] * reach, cc[1] + dir[1] * reach]
            })
            .find(|p| open_floor(*p, obs))
            .unwrap_or_else(|| panic!("no open floor beside {c:?}"));
        assert!(
            climb(far, 0.0, cc, c.h, 600, obs).is_none(),
            "container {c:?} was climbed from the floor at {far:?} without its chain"
        );
        (top, y)
    }

    // 6
    #[test]
    fn every_yard_container_roof_is_reached_by_a_climbing_chain() {
        let level = yard();
        let obs = &level.obstacles;
        let containers: Vec<&Obstacle> = obs
            .iter()
            .filter(|o| o.kind == Cover::Container && (o.h - 2.6).abs() < 1e-6)
            .collect();
        assert_eq!(
            containers.len(),
            20,
            "the stacked pair is the only taller container"
        );
        // C2 carries the roof block and has no chain of its own: its roof
        // is one hop from Q2's, which the chain reaches.
        let is_c2 = |o: &Obstacle| {
            o.min[0] < 0.0 && o.max[0] > 0.0 && (centre(o)[1].abs() - 15.2).abs() < 1e-4
        };
        for c in containers.iter().filter(|c| !is_c2(c)) {
            climb_by_chain(&level, c);
        }
        for c2 in containers.iter().filter(|c| is_c2(c)) {
            let cz = centre(c2)[1];
            let q2 = obs
                .iter()
                .find(|o| {
                    o.kind == Cover::Container
                        && (o.min[0] - 7.6).abs() < 1e-4
                        && (centre(o)[1] - cz).abs() < 1e-4
                })
                .expect("Q2 beside C2");
            let (pos, y) = climb_by_chain(&level, q2);
            // Walk to Q2's west edge and hop the 4 m backlot entrance.
            let (pos, y) =
                climb(pos, y, [q2.min[0] + 0.6, cz], q2.h, 300, obs).expect("walk along Q2's roof");
            let (on_c2, y) = hop(pos, y, [c2.max[0] - 1.6, cz], obs);
            assert!(
                (y - c2.h).abs() < 1e-3,
                "rested at {y} at {on_c2:?}, not on C2's roof"
            );
            assert!(contains(c2, on_c2), "{on_c2:?} is not over C2");
            // And never from the floor beside it.
            let far = [0.0, cz + (cz.signum() * (1.2 + 1.4))];
            assert!(open_floor(far, obs), "{far:?} is not open floor");
            assert!(
                climb(far, 0.0, centre(c2), c2.h, 600, obs).is_none(),
                "C2 {c2:?} was climbed from the floor"
            );
        }
        // Q3 at 5.2 has no chain and the floor never reaches it.
        let stacked: Vec<&Obstacle> = obs
            .iter()
            .filter(|o| o.kind == Cover::Container && o.h > 5.0)
            .collect();
        assert_eq!(stacked.len(), 4);
        for q3 in stacked {
            assert!(
                !obs.iter()
                    .any(|k| k.kind == Cover::Crate && gap(k, q3) <= 0.5),
                "the stacked pair {q3:?} has a chain"
            );
        }
    }

    // 7
    #[test]
    fn no_yard_spawn_sees_another() {
        // Level fire from the floor, both ways round, because where a
        // round's per-tick samples fall depends on which end it leaves
        // from. Every diagonal class is decided here, not by hand.
        let level = yard();
        // The driver must be able to fail: the far-corner pairs are 57 m
        // apart, past the sidearm's own 54 m, so first prove its round
        // reaches the far corner of an empty arena.
        let mut open = level.clone();
        open.obstacles.clear();
        assert!(
            shot_over(&open, [-22.0, -22.0], 0.0, 0.0, [22.0, 22.0]),
            "the driver's round does not cross an empty arena"
        );
        for (i, a) in level.spawns.iter().enumerate() {
            for (j, b) in level.spawns.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert!(
                    !shot_over(&level, *a, 0.0, 0.0, *b),
                    "spawn {i} {a:?} shoots spawn {j} {b:?} on level fire"
                );
            }
        }
    }

    // 8
    //
    // The yard's lane is the long sightline; the train zone is boxed in by
    // the trains. The plan's second line was the alley itself, (-12, 10) to
    // (12, 10), and it connects: it runs under the two train-zone blocks,
    // which hang at 2.3 and stop nothing at eye level, and nothing else in
    // the alley reaches 1.45. None of the plan's knobs (Q2's length, the
    // sandbags) can touch a line at z 10, so the alley is left as authored
    // and the finding is in the plan's deviations; what is pinned here is
    // the contrast the geometry does deliver, that a body in the alley
    // sees neither the yard nor the backlot.
    #[test]
    fn the_yard_has_a_long_sightline_and_the_train_zone_does_not() {
        let level = yard();
        assert!(
            shot_over(&level, [-20.0, 3.0], 0.0, 0.0, [20.0, 3.0]),
            "the yard lane at z 3 must connect end to end"
        );
        let alley = [-12.0, 10.0];
        for (name, out) in [("the yard", [-12.0, 3.0]), ("the backlot", [-12.0, 21.5])] {
            assert!(
                !shot_over(&level, alley, 0.0, 0.0, out),
                "the alley at {alley:?} sees {name} at {out:?}"
            );
            assert!(
                !shot_over(&level, out, 0.0, 0.0, alley),
                "{name} at {out:?} sees the alley at {alley:?}"
            );
        }
    }

    /// One jump under a block from `pos` at `y0`, jumping on tick 0 and
    /// nothing after: the ticks on which the clamp reported `k`, and the
    /// highest the feet got.
    fn bonk_ticks(
        pos: [f32; 2],
        y0: f32,
        k: usize,
        ticks: u32,
        obs: &[Obstacle],
    ) -> (Vec<u32>, f32) {
        let (mut y, mut vy, mut peak) = (y0, 0.0f32, y0);
        let mut hits = Vec::new();
        for tick in 0..ticks {
            let v = step_vertical(pos, y, vy, tick == 0, FIXED_DT, obs);
            y = v.y;
            vy = v.vy;
            peak = peak.max(y);
            if v.bonked == Some(k) {
                hits.push(tick);
            } else {
                assert!(v.bonked.is_none(), "bonked {:?} instead of {k}", v.bonked);
            }
        }
        (hits, peak)
    }

    // 9
    #[test]
    fn every_train_zone_block_is_bonked_from_the_floor_and_walked_under() {
        let level = yard();
        let obs = &level.obstacles;
        let trains: Vec<(usize, &Obstacle)> = loot_blocks(&level)
            .into_iter()
            .filter(|(_, o)| (o.base - TRAIN_BASE).abs() < 1e-6)
            .collect();
        assert_eq!(trains.len(), 4);
        for (k, block) in trains {
            let c = centre(block);
            assert!(
                standable_floor(c, obs),
                "block {block:?} cannot be stood under"
            );
            assert_eq!(support_height(c, PLAYER_R, 0.0, obs), 0.0);
            // One press from the floor: the head meets the block once, on
            // the first tick the integrator lifts it past the bottom (the
            // fourth step after the press: 0.55 m of rise against 0.44 of
            // headroom; the third gives 0.42 and misses), and the feet stop
            // exactly at bottom minus body.
            let (hits, peak) = bonk_ticks(c, 0.0, k, 90, obs);
            assert_eq!(hits, [3], "block {block:?}: bonk ticks (0-based)");
            assert!(
                (peak - (block.base - BODY_H_STAND)).abs() < 1e-4,
                "block {block:?}: feet peaked at {peak}"
            );
            // Walked under, 3.9 m through and back, never touched.
            let (mut pos, mut y, mut vy) = ([c[0], c[1] - 2.0], 0.0f32, 0.0f32);
            for tick in 0..52u32 {
                let dir = if tick < 26 { 1.0 } else { -1.0 };
                pos = move_circle(pos, y, [0.0, dir], MOVE_SPEED, FIXED_DT, obs);
                let v = step_vertical(pos, y, vy, false, FIXED_DT, obs);
                y = v.y;
                vy = v.vy;
                assert_eq!(v.bonked, None, "walking under {block:?} bonked at {pos:?}");
                assert_eq!(y, 0.0, "lifted off the floor under {block:?} at {pos:?}");
            }
            assert!(
                (pos[1] - (c[1] - 2.0)).abs() < 0.2,
                "did not walk under {block:?} and back: ended at {pos:?}"
            );
        }
    }

    // 10
    #[test]
    fn the_king_block_is_bonked_from_the_dock_and_from_the_floor() {
        let level = yard();
        let obs = &level.obstacles;
        let (k, king) = find(&level, Cover::Loot, [-0.5, -0.5]);
        let dock = &level.obstacles[0];
        assert_eq!(dock.kind, Cover::Plinth);
        assert!(
            contains(dock, centre(king)),
            "the king block hangs over the dock"
        );

        // From the dock: a 0.39 m hop, met on the third step after the
        // press (0.42 m of rise).
        assert_eq!(support_height([0.0, 0.0], PLAYER_R, dock.h, obs), dock.h);
        let (hits, peak) = bonk_ticks([0.0, 0.0], dock.h, k, 90, obs);
        assert_eq!(hits, [2], "from the dock: bonk ticks (0-based)");
        assert!((peak - (king.base - BODY_H_STAND)).abs() < 1e-4);

        // From the floor: a sprinting jump from beside the dock. The dock
        // blocks the run-in until the feet clear its step-up line, then the
        // body sails over it and the head meets the block near the apex,
        // before the feet have touched anything but the floor.
        let start = [0.0, dock.max[1] + 0.7];
        assert!(open_floor(start, obs), "{start:?} is not open floor");
        assert_eq!(support_height(start, PLAYER_R, 0.0, obs), 0.0);
        let speed = stance_speed(true, false, false);
        let (mut pos, mut y, mut vy) = (start, 0.0f32, 0.0f32);
        let mut hits = Vec::new();
        let mut landed_on = None;
        for tick in 0..90u32 {
            let mv = if pos[1] > 0.0 {
                [0.0, -1.0]
            } else {
                [0.0, 0.0]
            };
            pos = move_circle(pos, y, mv, speed, FIXED_DT, obs);
            let v = step_vertical(pos, y, vy, tick == 0, FIXED_DT, obs);
            y = v.y;
            vy = v.vy;
            if v.bonked == Some(k) {
                hits.push((tick, pos, y));
            } else {
                assert!(v.bonked.is_none());
            }
            if tick > 0 && v.grounded && landed_on.is_none() {
                landed_on = Some((tick, y));
            }
        }
        assert_eq!(hits.len(), 1, "from the floor: {hits:?}");
        let (bonk_tick, at, feet) = hits[0];
        assert!(
            box_dist(at, king) < PLAYER_R,
            "bonked from {at:?}, not under the block"
        );
        assert!((feet - (king.base - BODY_H_STAND)).abs() < 1e-4);
        let (land_tick, land_y) = landed_on.expect("came down");
        assert!(land_tick > bonk_tick, "touched down before the bonk");
        assert_eq!(land_y, dock.h, "came down on the dock");
    }

    // 11
    #[test]
    fn the_roof_block_is_bonked_from_its_container_and_not_from_the_floor() {
        let level = yard();
        let obs = &level.obstacles;
        let apex = jump_apex();
        assert!(
            (apex - 1.6867).abs() < 1e-3,
            "the integrator's apex is {apex}"
        );
        let roofs: Vec<(usize, &Obstacle)> = loot_blocks(&level)
            .into_iter()
            .filter(|(_, o)| (o.base - ROOF_BASE).abs() < 1e-6)
            .collect();
        assert_eq!(roofs.len(), 2);
        for (k, block) in roofs {
            let c = centre(block);
            let c2 = obs
                .iter()
                .find(|o| o.kind == Cover::Container && contains(o, c))
                .expect("C2 under the roof block");
            // From C2's roof, walked under (head 4.46 against 4.9) and
            // bonked with a 0.44 m rise.
            assert!(c2.h + BODY_H_STAND < block.base, "not walked under");
            assert_eq!(support_height(c, PLAYER_R, c2.h, obs), c2.h);
            let (hits, peak) = bonk_ticks(c, c2.h, k, 90, obs);
            assert_eq!(hits, [3], "from the roof: bonk ticks (0-based)");
            assert!((peak - (block.base - BODY_H_STAND)).abs() < 1e-4);
            // From the floor the head reaches 3.55 at most, and nothing on
            // the floor beside C2 gets a body onto it: 200 ticks of jumping
            // at the block from the open floor beside the container never
            // report it.
            assert!(apex + BODY_H_STAND < block.base);
            let sign = c[1].signum();
            let outer = if sign > 0.0 { c2.max[1] } else { c2.min[1] };
            let start = [c[0], outer + sign * 0.8];
            assert!(open_floor(start, obs), "{start:?} is not open floor");
            let (mut pos, mut y, mut vy, mut grounded) = (start, 0.0f32, 0.0f32, true);
            for _ in 0..200 {
                pos = move_circle(pos, y, [0.0, -sign], MOVE_SPEED, FIXED_DT, obs);
                let v = step_vertical(pos, y, vy, grounded, FIXED_DT, obs);
                y = v.y;
                vy = v.vy;
                grounded = v.grounded;
                assert_eq!(
                    v.bonked, None,
                    "the roof block was bonked from the floor at {pos:?}"
                );
                // Feet this far under C2's top are still a body the box
                // stops: never on it.
                assert!(y < c2.h - STEP_UP, "got onto C2 from the floor at {pos:?}");
            }
        }
    }

    // 12
    #[test]
    fn the_backlot_reaches_the_train_zone_through_two_gaps() {
        let level = yard();
        let obs = &level.obstacles;
        let (_, c2) = find(&level, Cover::Container, [-3.6, 14.0]);
        let (_, q2) = find(&level, Cover::Container, [7.6, 14.0]);
        let (_, q2_crate) = find(&level, Cover::Crate, [14.4, 15.2]);
        let (_, q3) = find(&level, Cover::Container, [17.6, 14.0]);
        assert!((q2.min[0] - c2.max[0] - 4.0).abs() < 1e-4, "the 4 m gap");
        assert!(
            (q3.min[0] - q2_crate.max[0] - 2.0).abs() < 1e-4,
            "the 2 m gap"
        );
        let wide = f32::midpoint(c2.max[0], q2.min[0]);
        let narrow = f32::midpoint(q2_crate.max[0], q3.min[0]);
        // Each spawn walks (no jumping: the sandbags are gone round, not
        // over) to the mouth of its gap and through it into the alley.
        let routes: [([f32; 2], &[[f32; 2]]); 2] = [
            (
                [5.5, 21.5],
                &[[8.7, 21.5], [8.7, 17.2], [wide, 17.2], [wide, 12.0]],
            ),
            (
                [19.0, 21.5],
                &[[19.8, 21.5], [19.8, 17.2], [narrow, 17.2], [narrow, 12.0]],
            ),
        ];
        for (spawn, waypoints) in routes {
            assert!(level.spawns.contains(&spawn));
            let mut pos = spawn;
            for w in waypoints {
                pos = walk(pos, *w, 600, obs)
                    .unwrap_or_else(|| panic!("from {spawn:?}: stuck before {w:?} at {pos:?}"));
            }
            assert!(pos[1] < c2.min[1], "did not reach the alley from {spawn:?}");
        }
    }

    /// Where a block is "reached" on foot: its own footprint when nothing
    /// is under it (it is walked under), otherwise the footprint of the
    /// floor box it hangs over (the dock under the king block, C2 under a
    /// roof block), because that is what a body has to get beside to climb
    /// toward it.
    fn approach<'a>(obs: &'a [Obstacle], block: &'a Obstacle) -> &'a Obstacle {
        obs.iter()
            .find(|o| o.base == 0.0 && contains(o, centre(block)))
            .unwrap_or(block)
    }

    // 13
    #[test]
    fn every_spawn_reaches_every_block_footprint() {
        let level = yard();
        let obs = &level.obstacles;
        // A 0.2 m grid over the arena, each cell asked of the sim whether a
        // standing body may occupy it, flooded 4-way from each spawn.
        let n: i32 = 116;
        let side = (2 * n + 1) as usize;
        let at = |i: i32, j: i32| [0.2 * i as f32, 0.2 * j as f32];
        let idx = |i: i32, j: i32| ((i + n) as usize) * side + (j + n) as usize;
        let mut free = vec![false; side * side];
        for i in -n..=n {
            for j in -n..=n {
                free[idx(i, j)] = standable_floor(at(i, j), obs);
            }
        }
        // A cell counts as reaching a footprint when its circle plus one
        // grid step touches it.
        let mut targets: Vec<(String, &Obstacle)> = loot_blocks(&level)
            .into_iter()
            .map(|(k, b)| (format!("block {k}"), approach(obs, b)))
            .collect();
        targets.push(("the dock".to_string(), &level.obstacles[0]));
        for (s, spawn) in level.spawns.iter().enumerate() {
            let (si, sj) = (
                (spawn[0] / 0.2).round() as i32,
                (spawn[1] / 0.2).round() as i32,
            );
            assert!(
                free[idx(si, sj)],
                "spawn {s} {spawn:?} is not on free floor"
            );
            let mut seen = vec![false; side * side];
            let mut stack = vec![(si, sj)];
            seen[idx(si, sj)] = true;
            let mut reached = vec![false; targets.len()];
            while let Some((i, j)) = stack.pop() {
                let p = at(i, j);
                for (t, (_, o)) in targets.iter().enumerate() {
                    if !reached[t] && box_dist(p, o) <= PLAYER_R + 0.2 + 1e-3 {
                        reached[t] = true;
                    }
                }
                for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (ni, nj) = (i + di, j + dj);
                    if ni.abs() > n || nj.abs() > n {
                        continue;
                    }
                    let q = idx(ni, nj);
                    if free[q] && !seen[q] {
                        seen[q] = true;
                        stack.push((ni, nj));
                    }
                }
            }
            for (t, (name, o)) in targets.iter().enumerate() {
                assert!(reached[t], "spawn {s} {spawn:?} never reaches {name} {o:?}");
            }
        }
    }

    // 14
    #[test]
    fn yard_survives_serde() {
        let level = yard();
        let json = serde_json::to_string(&level).unwrap();
        let back: Level = serde_json::from_str(&json).unwrap();
        assert_eq!(back, level);
        assert!(json.contains("\"Loot\""), "the blocks travel by kind name");
        // And the round trip keeps the blocks armed in the same order.
        let a = Sim::from_level(&level, 3, GameMode::Ffa);
        let b = Sim::from_level(&back, 3, GameMode::Ffa);
        assert_eq!(
            a.loot.iter().map(|l| l.obstacle).collect::<Vec<_>>(),
            b.loot.iter().map(|l| l.obstacle).collect::<Vec<_>>()
        );
    }

    // 14
    #[test]
    fn a_v13_json_level_still_decodes_without_loot() {
        // The v17 map as a v13 editor would have written it: Trench City
        // with its blocks stripped and its four pads back under the
        // roofs. Every kind it names predates `Cover::Loot`, and a sim
        // built from it arms no block and four pads.
        let mut v17 = Level::trench_city();
        v17.obstacles.retain(|o| o.kind != Cover::Loot);
        v17.pads = vec![[0.0, 12.5], [-12.5, 0.0], [0.0, -12.5], [12.5, 0.0]];
        let json = serde_json::to_string(&v17).unwrap();
        assert!(!json.contains("Loot"));
        let old: Level = serde_json::from_str(&json).unwrap();
        assert_eq!(old.obstacles.len(), 1 + 4 * 21);
        assert!(old.obstacles.iter().all(|o| o.kind != Cover::Loot));
        let sim = Sim::from_level(&old, 0, GameMode::Ffa);
        assert!(
            sim.loot.is_empty(),
            "a level without blocks plays without blocks"
        );
        assert_eq!(sim.pads.len(), 4);
        // And a hand-written v13 box with a raised base is a roof, not a
        // block: the kind is carried, never inferred from the base.
        let v13 = r#"{"arena_half":24.0,
            "obstacles":[{"min":[-6.0,11.0],"max":[6.0,14.0],"h":2.9,"base":2.5,"kind":"Roof"}],
            "spawns":[[5.0,6.0]]}"#;
        let roofed: Level = serde_json::from_str(v13).unwrap();
        assert_eq!(roofed.obstacles[0].kind, Cover::Roof);
        assert_eq!(roofed.obstacles[0].base, 2.5);
        assert!(
            Sim::from_level(&roofed, 0, GameMode::Ffa).loot.is_empty(),
            "a raised roof must not arm a block"
        );
    }

    // 15
    #[test]
    fn trench_city_matches_its_fixture() {
        let frozen: Level = serde_json::from_str(TRENCH_CITY_V18).unwrap();
        let live = Level::trench_city();
        assert_eq!(live.obstacles.len(), frozen.obstacles.len());
        for (i, (a, b)) in live.obstacles.iter().zip(&frozen.obstacles).enumerate() {
            assert_eq!(a, b, "trench city obstacle {i} moved");
        }
        assert_eq!(live.spawns, frozen.spawns);
        assert!(frozen.pads.is_empty(), "the v18 fixture carries no pads");
        assert_eq!(live.pads, frozen.pads);
        assert_eq!(live.decor, frozen.decor);
        assert_eq!(live, frozen);
        assert_eq!(
            frozen
                .obstacles
                .iter()
                .filter(|o| o.kind == Cover::Loot)
                .count(),
            4,
            "the v18 fixture carries the four blocks"
        );
        // The fixture is what serde writes today, so a serialisation change
        // is caught as loudly as a map change.
        assert_eq!(
            serde_json::to_string_pretty(&live).unwrap() + "\n",
            TRENCH_CITY_V18.replace("\r\n", "\n")
        );
    }

    /// Writes the fixture. Ignored: run once by hand
    /// (`cargo test -p arena-core write_trench_city_fixture -- --ignored`)
    /// when the frozen map is meant to change, and never otherwise; a
    /// changed map is a protocol bump (`PROTO_VERSION`), so the run comes
    /// with one.
    #[test]
    #[ignore = "regenerates the frozen fixture; run by hand only"]
    fn write_trench_city_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/trench-city-v18.json"
        );
        let json = serde_json::to_string_pretty(&Level::trench_city()).unwrap() + "\n";
        std::fs::write(path, json).unwrap();
    }
}
