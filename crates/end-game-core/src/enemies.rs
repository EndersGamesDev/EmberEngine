//! Authored castle encounters, fixed-step attacks and contact-driven damage.
use crate::{Dungeon, GuardContact, ImpactKind, STEP, StrikeKind, layout};
use glam::{Vec2, Vec3};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    SwordSoldier,
    SpearSoldier,
    HollowAxeKnight,
    Cyclops,
}
impl EnemyKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::SwordSoldier => "Sword soldier",
            Self::SpearSoldier => "Spear soldier",
            Self::HollowAxeKnight => "Hollow axe knight",
            Self::Cyclops => "The One-Eyed Castellan",
        }
    }
    pub fn height(self) -> f32 {
        match self {
            Self::SwordSoldier => 1.8,
            Self::SpearSoldier => 1.85,
            Self::HollowAxeKnight => 2.05,
            Self::Cyclops => 3.2,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Self::SwordSoldier | Self::SpearSoldier => 0.30,
            Self::HollowAxeKnight => 0.36,
            Self::Cyclops => 0.72,
        }
    }
    pub fn stride_length(self) -> f32 {
        1.2 * self.height() / 1.865
    }
    pub fn max_health(self) -> f32 {
        match self {
            Self::SwordSoldier => 70.,
            Self::SpearSoldier => 75.,
            Self::HollowAxeKnight => 100.,
            Self::Cyclops => 360.,
        }
    }
    fn speed(self) -> f32 {
        match self {
            Self::SwordSoldier => 1.45,
            Self::SpearSoldier => 1.25,
            Self::HollowAxeKnight => 1.1,
            Self::Cyclops => 1.35,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyPhase {
    Idle,
    Hunting,
    Attacking,
    Staggered,
    Dead,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyAttack {
    Slash,
    Thrust,
    Chop,
    Sweep,
    Slam,
}
impl EnemyAttack {
    pub fn windup_time(self) -> f32 {
        match self {
            Self::Slash => 0.45,
            Self::Thrust => 0.60,
            Self::Chop => 0.72,
            Self::Sweep => 0.80,
            Self::Slam => 1.10,
        }
    }
    pub fn contact_time(self) -> f32 {
        match self {
            Self::Slash => 0.56,
            Self::Thrust => 0.72,
            Self::Chop => 0.87,
            Self::Sweep => 0.98,
            Self::Slam => 1.28,
        }
    }
    pub fn follow_end(self) -> f32 {
        match self {
            Self::Slash => 0.73,
            Self::Thrust => 0.90,
            Self::Chop => 1.06,
            Self::Sweep => 1.18,
            Self::Slam => 1.50,
        }
    }
    pub fn duration(self) -> f32 {
        match self {
            Self::Slash => 1.50,
            Self::Thrust => 1.80,
            Self::Chop => 2.10,
            Self::Sweep => 2.30,
            Self::Slam => 2.85,
        }
    }
    pub fn range(self) -> f32 {
        match self {
            Self::Slash => 1.65,
            Self::Thrust => 2.45,
            Self::Chop => 1.95,
            Self::Sweep => 2.95,
            Self::Slam => 3.30,
        }
    }
    pub fn damage(self) -> f32 {
        match self {
            Self::Slash => 12.,
            Self::Thrust => 16.,
            Self::Chop => 20.,
            Self::Sweep => 24.,
            Self::Slam => 34.,
        }
    }
    pub fn block_cost(self) -> f32 {
        match self {
            Self::Slash => 22.,
            Self::Thrust => 25.,
            Self::Chop => 32.,
            Self::Sweep => 38.,
            Self::Slam => 100.,
        }
    }
    pub fn guardable(self) -> bool {
        self != Self::Slam
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Slash => "Sword cut",
            Self::Thrust => "Spear thrust",
            Self::Chop => "Axe chop",
            Self::Sweep => "Castellan sweep",
            Self::Slam => "Crushing slam — dodge",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnemyPose {
    pub phase: EnemyPhase,
    pub elapsed: f32,
    pub attack: EnemyAttack,
    pub walk_phase: f32,
    pub walk_blend: f32,
    pub attack_time: f32,
    pub attack_weight: f32,
    pub flinch_amount: f32,
}
#[derive(Clone, Debug)]
pub struct Enemy {
    pub id: usize,
    pub kind: EnemyKind,
    pub position: Vec3,
    pub spawn: Vec3,
    /// Player convention: zero faces -Z; render with Ry(-yaw).
    pub yaw: f32,
    pub health: f32,
    pub max_health: f32,
    pub phase: EnemyPhase,
    pub elapsed: f32,
    pub walk_phase: f32,
    pub walk_blend: f32,
    pub attack: EnemyAttack,
    pub attack_event: u32,
    pub hit_event: u32,
    pub death_event: u32,
    pub phase_two: bool,
    pub interrupted: Option<EnemyPose>,
    pub flinch_left: f32,
    flinch_origin: f32,
    pub cooldown: f32,
    pub contact_done: bool,
    pub alerted: bool,
}
impl Enemy {
    pub fn new(id: usize, kind: EnemyKind, position: Vec3) -> Self {
        Self {
            id,
            kind,
            position,
            spawn: position,
            yaw: std::f32::consts::PI,
            health: kind.max_health(),
            max_health: kind.max_health(),
            phase: EnemyPhase::Idle,
            elapsed: 0.,
            walk_phase: 0.,
            walk_blend: 0.,
            attack: match kind {
                EnemyKind::SwordSoldier => EnemyAttack::Slash,
                EnemyKind::SpearSoldier => EnemyAttack::Thrust,
                EnemyKind::HollowAxeKnight => EnemyAttack::Chop,
                EnemyKind::Cyclops => EnemyAttack::Sweep,
            },
            attack_event: 0,
            hit_event: 0,
            death_event: 0,
            phase_two: false,
            interrupted: None,
            flinch_left: 0.,
            flinch_origin: 0.,
            cooldown: 0.6,
            contact_done: false,
            alerted: false,
        }
    }
    pub fn alive(&self) -> bool {
        self.health > 0. && self.phase != EnemyPhase::Dead
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0., -self.yaw.cos())
    }
    pub fn flinch_amount(&self) -> f32 {
        if self.flinch_left <= 0.0 {
            return 0.0;
        }
        let age = (0.22 - self.flinch_left).clamp(0.0, 0.22);
        let ease = |a: f32, b: f32| {
            let t = ((age - a) / (b - a)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        (self.flinch_origin + (1.0 - self.flinch_origin) * ease(0.0, 0.045))
            * (1.0 - ease(0.045, 0.22))
    }
    fn flinch(&mut self) {
        self.flinch_origin = self.flinch_amount();
        self.flinch_left = 0.22;
    }
    pub fn attack_pose(&self) -> Option<(EnemyAttack, f32, f32)> {
        if self.phase == EnemyPhase::Attacking {
            return Some((self.attack, self.elapsed, 1.));
        }
        self.interrupted.and_then(|p| {
            let weight = p.attack_weight
                * if self.phase == EnemyPhase::Staggered {
                    (1. - self.elapsed / 0.45).clamp(0., 1.)
                } else {
                    1.
                };
            (weight > 0.).then_some((p.attack, p.attack_time, weight))
        })
    }
    fn snapshot(&self) -> EnemyPose {
        let (attack, attack_time, attack_weight) =
            self.attack_pose().unwrap_or((self.attack, 0., 0.));
        EnemyPose {
            phase: self.phase,
            elapsed: self.elapsed,
            attack,
            walk_phase: self.walk_phase,
            walk_blend: self.walk_blend,
            attack_time,
            attack_weight,
            flinch_amount: self.flinch_amount(),
        }
    }
    pub fn start_attack(&mut self, target: Vec3) {
        let d = target - self.position;
        self.yaw = d.x.atan2(-d.z);
        if self.kind == EnemyKind::Cyclops {
            self.attack = if self.attack_event % 2 == 0 {
                EnemyAttack::Sweep
            } else {
                EnemyAttack::Slam
            };
        }
        self.phase = EnemyPhase::Attacking;
        self.elapsed = 0.;
        self.contact_done = false;
        self.interrupted = None;
        self.attack_event = self.attack_event.wrapping_add(1);
    }
    pub fn take_hit(&mut self, damage: f32, heavy: bool) {
        if !self.alive() {
            return;
        }
        let pose = self.snapshot();
        self.health = (self.health - damage).max(0.);
        self.hit_event = self.hit_event.wrapping_add(1);
        self.flinch();
        self.alerted = true;
        if self.health <= 0. {
            self.interrupted = Some(pose);
            self.phase = EnemyPhase::Dead;
            self.elapsed = 0.;
            self.death_event = self.death_event.wrapping_add(1);
        } else if heavy && self.kind != EnemyKind::Cyclops {
            self.interrupted = Some(pose);
            self.phase = EnemyPhase::Staggered;
            self.elapsed = 0.;
            self.cooldown = 0.5;
        } else if self.phase == EnemyPhase::Idle {
            self.phase = EnemyPhase::Hunting;
            self.elapsed = 0.;
        }
    }
    fn turn_toward(&mut self, target: Vec3) {
        let d = target - self.position;
        let wanted = d.x.atan2(-d.z);
        let delta = (wanted - self.yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        self.yaw += delta.clamp(-2.7 * STEP, 2.7 * STEP);
    }
}

pub fn authored_enemies() -> Vec<Enemy> {
    use EnemyKind::*;
    [
        (SwordSoldier, [-0.8, 0., -18.]),
        (SpearSoldier, [5., 0., -29.]),
        (SwordSoldier, [-5., 0., -34.]),
        (SpearSoldier, [7., 0., -47.]),
        (HollowAxeKnight, [-8., 6., -74.]),
        (HollowAxeKnight, [8., 6., -86.]),
        (HollowAxeKnight, [-24., 14., -81.]),
        (HollowAxeKnight, [24., 14., -73.]),
        (Cyclops, [0., 0., -44.]),
    ]
    .into_iter()
    .enumerate()
    .map(|(id, (kind, p))| Enemy::new(id, kind, Vec3::from_array(p)))
    .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastleEventKind {
    EnemyAttack,
    EnemyHit,
    EnemyDeath,
    BossAwaken,
    BossPhaseTwo,
    BossDeath,
    PlayerHit,
    Seal,
    Clue,
    Sun,
    Wolf,
    Bell,
    Gate,
    Heal,
    Checkpoint,
    Escaped,
}
impl CastleEventKind {
    pub fn key(self) -> &'static str {
        match self {
            Self::BossAwaken => "boss_intro",
            Self::BossPhaseTwo => "boss_phase2",
            Self::BossDeath => "boss_defeat",
            Self::Clue => "escape_clue",
            Self::Escaped => "escape_ending",
            Self::EnemyAttack => "enemy_attack",
            Self::EnemyHit => "enemy_hit",
            Self::EnemyDeath => "enemy_death",
            Self::PlayerHit => "player_hit",
            Self::Seal => "seal",
            Self::Sun => "sun",
            Self::Wolf => "wolf",
            Self::Bell => "bell",
            Self::Gate => "gate",
            Self::Heal => "heal",
            Self::Checkpoint => "checkpoint",
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct CastleEvent {
    pub id: u32,
    pub kind: CastleEventKind,
    pub time: f32,
    pub position: Vec3,
}
#[derive(Clone, Debug, Default)]
pub struct CastleEvents {
    pub serial: u32,
    recent: VecDeque<CastleEvent>,
}
impl CastleEvents {
    pub fn events(&self) -> impl Iterator<Item = &CastleEvent> {
        self.recent.iter()
    }
    pub(crate) fn emit(&mut self, kind: CastleEventKind, time: f32, position: Vec3) {
        self.serial = self.serial.wrapping_add(1);
        self.recent.push_back(CastleEvent {
            id: self.serial,
            kind,
            time,
            position,
        });
        if self.recent.len() > 24 {
            self.recent.pop_front();
        }
    }
}

/// Segment against solid volumes, including slab bottoms and the two animated gates.
pub fn line_clear(from: Vec3, to: Vec3, extra: &[layout::Aabb]) -> bool {
    !layout::castle()
        .solids
        .iter()
        .map(|s| s.bounds)
        .chain(extra.iter().copied())
        .any(|b| {
            let d = to - from;
            let mut lo: f32 = 0.001;
            let mut hi: f32 = 0.999;
            for axis in 0..3 {
                if d[axis].abs() < 0.00001 {
                    if from[axis] < b.min[axis] || from[axis] > b.max[axis] {
                        return false;
                    }
                } else {
                    let a = (b.min[axis] - from[axis]) / d[axis];
                    let z = (b.max[axis] - from[axis]) / d[axis];
                    lo = lo.max(a.min(z));
                    hi = hi.min(a.max(z));
                    if lo > hi {
                        return false;
                    }
                }
            }
            lo <= hi
        })
}

impl Dungeon {
    pub fn focus_enemy(&self) -> Option<&Enemy> {
        self.enemies
            .iter()
            .filter(|e| e.alive() && e.alerted && (e.position.y - self.position.y).abs() < 2.)
            .filter(|e| {
                let d = e.position - self.position;
                d.length()
                    < if e.kind == EnemyKind::Cyclops {
                        22.
                    } else {
                        8.
                    }
                    && self.forward().dot(d.normalize_or_zero()) > 0.35
                    && line_clear(
                        self.position + Vec3::Y * 1.1,
                        e.position + Vec3::Y * 1.1,
                        &[self.exit_gate_bounds(), self.quest.gate_bounds()],
                    )
            })
            .min_by(|a, b| {
                let winding_up = |e: &Enemy| {
                    e.phase == EnemyPhase::Attacking && e.elapsed < e.attack.contact_time()
                };
                // A nearby waiting guard must not hide the committed attack's warning.
                winding_up(b).cmp(&winding_up(a)).then_with(|| {
                    a.position
                        .distance_squared(self.position)
                        .total_cmp(&b.position.distance_squared(self.position))
                })
            })
    }
    pub fn boss(&self) -> Option<&Enemy> {
        self.enemies.iter().find(|e| e.kind == EnemyKind::Cyclops)
    }
    pub fn defeated_count(&self) -> u32 {
        self.enemies.iter().filter(|e| !e.alive()).count() as u32
    }
    pub(crate) fn castle_sword_contact(&mut self, strike: StrikeKind) -> bool {
        if self.stage < 5 {
            return false;
        }
        let gates = [self.exit_gate_bounds(), self.quest.gate_bounds()];
        let eye = self.position + Vec3::Y * 1.1;
        let target = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let d = e.position - self.position;
                e.alive()
                    && d.y.abs() < 1.35
                    && Vec2::new(d.x, d.z).length() < 2.7 + e.kind.radius() * 0.25
                    && self
                        .forward()
                        .dot(Vec3::new(d.x, 0., d.z).normalize_or_zero())
                        > 0.15
                    && line_clear(
                        eye,
                        e.position + Vec3::Y * e.kind.height().min(1.5) * 0.7,
                        &gates,
                    )
            })
            .min_by(|(_, a), (_, b)| {
                a.position
                    .distance_squared(self.position)
                    .total_cmp(&b.position.distance_squared(self.position))
            })
            .map(|(i, _)| i);
        let Some(i) = target else {
            return false;
        };
        let e = &mut self.enemies[i];
        let was_phase_two = e.phase_two;
        e.take_hit(
            strike.damage() * if self.werewolf { 1.25 } else { 1. },
            strike.heavy(),
        );
        let point = e.position + Vec3::Y * e.kind.height().min(1.6) * 0.7;
        let dead = !e.alive();
        let boss = e.kind == EnemyKind::Cyclops;
        if boss && e.health > 0. && e.health <= e.max_health * 0.5 {
            e.phase_two = true;
        }
        self.combat.impact(strike, ImpactKind::Enemy, point);
        self.castle_events.emit(
            if dead {
                if boss {
                    CastleEventKind::BossDeath
                } else {
                    CastleEventKind::EnemyDeath
                }
            } else {
                CastleEventKind::EnemyHit
            },
            self.time,
            point,
        );
        if boss && dead {
            self.quest.boss_defeated = true;
            self.say("The Castellan falls. Take the crown seal from his remains.");
        } else if boss && !was_phase_two && self.enemies[i].phase_two {
            self.castle_events
                .emit(CastleEventKind::BossPhaseTwo, self.time, point);
            self.say("The Castellan roars. His attacks quicken — watch the raised club.");
        } else {
            self.say(if dead {
                "The hollow guard falls."
            } else {
                "Your blade finds its mark."
            });
        }
        true
    }

    pub(crate) fn tick_castle_enemies(&mut self) {
        if self.stage < 5 || self.quest.escaped {
            return;
        }
        self.enemy_attack_wait = (self.enemy_attack_wait - STEP).max(0.);
        let mut occupied = self
            .enemies
            .iter()
            .any(|e| e.alive() && e.phase == EnemyPhase::Attacking);
        let gates = [self.exit_gate_bounds(), self.quest.gate_bounds()];
        let positions: Vec<_> = self
            .enemies
            .iter()
            .filter(|e| e.alive())
            .map(|e| (e.id, e.position, e.kind.radius()))
            .collect();
        let len = self.enemies.len();
        let start_turn = self.enemy_turn;
        for offset in 0..len {
            let i = (start_turn + offset) % len;
            let e = &mut self.enemies[i];
            e.elapsed += STEP;
            e.flinch_left = (e.flinch_left - STEP).max(0.);
            e.cooldown = (e.cooldown - STEP).max(0.);
            e.walk_blend = (e.walk_blend - STEP * 5.).max(0.);
            if !e.alive() {
                continue;
            }
            if e.phase == EnemyPhase::Staggered {
                if e.elapsed >= 0.45 {
                    e.phase = EnemyPhase::Hunting;
                    e.elapsed = 0.;
                    e.interrupted = None;
                }
                continue;
            }
            let d = self.position - e.position;
            let distance = Vec2::new(d.x, d.z).length();
            let same_level = d.y.abs() < 1.1;
            let visible = same_level
                && line_clear(
                    e.position + Vec3::Y * 1.1,
                    self.position + Vec3::Y * 1.1,
                    &gates,
                );
            if e.phase == EnemyPhase::Attacking {
                if !e.contact_done && e.elapsed + 0.00001 >= e.attack.contact_time() {
                    e.contact_done = true;
                    let cone = if e.attack == EnemyAttack::Thrust || e.attack == EnemyAttack::Slam {
                        0.82
                    } else {
                        0.35
                    };
                    let hit = visible
                        && distance <= e.attack.range()
                        && e.forward().dot(Vec3::new(d.x, 0., d.z).normalize_or_zero()) >= cone
                        && self.dodge_time == 0.;
                    if hit {
                        let pitch = self.pitch;
                        let f = Vec3::new(self.yaw.sin(), 0., -self.yaw.cos());
                        let look = Vec3::new(f.x * pitch.cos(), pitch.sin(), f.z * pitch.cos());
                        let incoming = (-d + Vec3::Y * 0.1).normalize_or_zero();
                        let point = self.position + Vec3::Y * 1.25 + look * 0.55;
                        let result = if e.attack.guardable()
                            && self.grounded
                            && look.dot(incoming) >= 0.5
                        {
                            self.guard
                                .receive_cost(&mut self.stamina, point, e.attack.block_cost())
                        } else {
                            GuardContact::Open
                        };
                        if result == GuardContact::Blocked {
                            self.combat.hitstop_left = 3. * STEP;
                            e.flinch();
                        } else {
                            self.health -= e.attack.damage();
                            self.castle_events
                                .emit(CastleEventKind::PlayerHit, self.time, point);
                        }
                    }
                }
                if e.elapsed + 0.00001 >= e.attack.duration() {
                    e.phase = EnemyPhase::Hunting;
                    e.elapsed = 0.;
                    e.cooldown = if e.phase_two { 0.55 } else { 0.9 };
                    self.enemy_attack_wait = 0.38;
                    self.enemy_turn = (i + 1) % len;
                }
                continue;
            }
            let home_distance =
                Vec2::new(self.position.x - e.spawn.x, self.position.z - e.spawn.z).length();
            let zone_match = layout::region_at(self.position.to_array())
                == layout::region_at(e.spawn.to_array());
            let aggro = if e.kind == EnemyKind::Cyclops {
                13.
            } else {
                8.5
            };
            if visible && zone_match && distance < aggro && !e.alerted {
                e.alerted = true;
                if e.kind == EnemyKind::Cyclops {
                    self.castle_events
                        .emit(CastleEventKind::BossAwaken, self.time, e.position);
                }
            }
            let chase = e.alerted
                && same_level
                && zone_match
                && home_distance
                    < if e.kind == EnemyKind::Cyclops {
                        16.
                    } else {
                        12.
                    };
            if !chase {
                e.phase = EnemyPhase::Idle;
            } else {
                e.phase = EnemyPhase::Hunting;
            }
            let target = if chase { self.position } else { e.spawn };
            e.turn_toward(target);
            if chase
                && visible
                && !occupied
                && self.enemy_attack_wait == 0.
                && e.cooldown == 0.
                && distance <= e.attack.range() * 0.90
                && e.forward().dot(d.normalize_or_zero()) > 0.75
            {
                e.start_attack(self.position);
                occupied = true;
                self.castle_events
                    .emit(CastleEventKind::EnemyAttack, self.time, e.position);
                continue;
            }
            let stop = if chase {
                if occupied {
                    e.attack.range() + 0.5
                } else {
                    e.attack.range() * 0.78
                }
            } else {
                0.08
            };
            let flat = Vec3::new(target.x - e.position.x, 0., target.z - e.position.z);
            if flat.length() > stop {
                let direction = flat.normalize_or_zero();
                let delta = direction * e.kind.speed() * if e.phase_two { 1.25 } else { 1. } * STEP;
                let mut body = layout::Body::new(e.position.to_array());
                body.radius = e.kind.radius();
                body.height = e.kind.height();
                let old = e.position;
                layout::advance_body_with(
                    layout::castle(),
                    &gates,
                    &mut body,
                    [delta.x, delta.z],
                    STEP,
                );
                let next = Vec3::from_array(body.position);
                let other_clear = positions.iter().all(|(id, p, r)| {
                    *id == e.id
                        || (p.y - next.y).abs() > 1.
                        || Vec2::new(p.x - next.x, p.z - next.z).length()
                            > e.kind.radius() + r + 0.10
                });
                let player_clear = Vec2::new(next.x - self.position.x, next.z - self.position.z)
                    .length()
                    > e.kind.radius() + layout::PLAYER_RADIUS + 0.10
                    || !same_level;
                // Encounters stay on their authored floor rather than dropping off stairs/walls.
                if body.grounded && (next.y - e.spawn.y).abs() < 0.29 && other_clear && player_clear
                {
                    e.position = next;
                }
                let walked = e.position.distance(old);
                if walked > 0.00001 {
                    e.walk_phase = (e.walk_phase
                        + walked / e.kind.stride_length() * std::f32::consts::TAU)
                        .rem_euclid(std::f32::consts::TAU);
                    e.walk_blend = (e.walk_blend + STEP * 10.).min(1.);
                }
            }
        }
    }
}
