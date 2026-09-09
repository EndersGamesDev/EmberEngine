//! Shared blade contacts, anatomical targets and bounded surface feedback.
use crate::{Dungeon, ImpactKind, Strike, WardenPhase, blade, layout};
use glam::{Quat, Vec3};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Head,
    LeftTorso,
    RightTorso,
    LeftLeg,
    RightLeg,
}
impl HitZone {
    pub const ALL: [Self; 5] = [
        Self::Head,
        Self::LeftTorso,
        Self::RightTorso,
        Self::LeftLeg,
        Self::RightLeg,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Head => "Head",
            Self::LeftTorso => "Left torso",
            Self::RightTorso => "Right torso",
            Self::LeftLeg => "Left leg",
            Self::RightLeg => "Right leg",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitReaction {
    pub id: u32,
    pub time: f32,
    pub zone: HitZone,
    pub point: Vec3,
    /// World-space motion of the sharp edge at contact.
    pub direction: Vec3,
    pub strength: f32,
    pub elapsed: f32,
    pub origin: [Vec3; 5],
}
impl HitReaction {
    pub fn vectors(self) -> [Vec3; 5] {
        let ease = |a: f32, b: f32| {
            let t = ((self.elapsed - a) / (b - a)).clamp(0., 1.);
            t * t * (3. - 2. * t)
        };
        let mut out = self.origin.map(|v| v * (1. - ease(0., 0.08)));
        out[self.zone as usize] +=
            self.direction * self.strength * ease(0., 0.055) * (1. - ease(0.055, 0.34));
        out
    }
}
#[derive(Clone, Copy, Debug)]
pub struct AimTarget {
    pub enemy_id: Option<usize>,
    pub zone: HitZone,
    pub point: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwordFrame {
    pub eye: Vec3,
    pub rotation: Quat,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwordSweep {
    pub strike: Strike,
    pub start: f32,
    pub end: f32,
    pub from: SwordFrame,
    pub to: SwordFrame,
    pub contact_fraction: Option<f32>,
}
impl SwordSweep {
    pub fn frame(self, fraction: f32) -> SwordFrame {
        let f = fraction.clamp(0., 1.);
        SwordFrame {
            eye: self.from.eye.lerp(self.to.eye, f),
            rotation: self.from.rotation.slerp(self.to.rotation, f),
        }
    }
    pub fn world_sample(self, fraction: f32) -> blade::BladePose {
        let f = fraction.clamp(0., 1.);
        let frame = self.frame(f);
        let p = blade::strike_sample(self.strike, self.start + (self.end - self.start) * f);
        blade::BladePose {
            position: frame.eye + frame.rotation * p.position,
            rotation: frame.rotation * p.rotation,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct SurfaceImpact {
    pub id: u32,
    pub time: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub material: layout::Surface,
    pub surface_id: u32,
    pub persistent: bool,
    pub strength: f32,
}
#[derive(Clone, Debug, Default)]
pub struct SurfaceImpacts {
    pub serial: u32,
    recent: VecDeque<SurfaceImpact>,
}
impl SurfaceImpacts {
    pub fn impacts(&self) -> impl Iterator<Item = &SurfaceImpact> {
        self.recent.iter()
    }
    pub fn clear(&mut self) {
        self.recent.clear();
    }
    fn push(&mut self, mut impact: SurfaceImpact) {
        self.serial = self.serial.wrapping_add(1);
        impact.id = self.serial;
        self.recent.push_back(impact);
        if self.recent.len() > 64 {
            self.recent.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Contact {
    fraction: f32,
    point: Vec3,
    normal: Vec3,
}
/// A slab test returns the actual plane, not the inflated blade centre.
fn segment_box(from: Vec3, to: Vec3, bounds: layout::Aabb) -> Option<Contact> {
    let delta = to - from;
    let mut lo: f32 = 0.;
    let mut hi: f32 = 1.;
    let mut normal = Vec3::ZERO;
    for axis in 0..3 {
        if delta[axis].abs() < 0.000001 {
            if from[axis] < bounds.min[axis] || from[axis] > bounds.max[axis] {
                return None;
            }
        } else {
            let a = (bounds.min[axis] - from[axis]) / delta[axis];
            let b = (bounds.max[axis] - from[axis]) / delta[axis];
            let near = a.min(b);
            if near > lo {
                lo = near;
                normal = Vec3::ZERO;
                normal[axis] = if a < b { -1. } else { 1. };
            }
            hi = hi.min(a.max(b));
            if lo > hi {
                return None;
            }
        }
    }
    if hi < 0. || lo > 1. {
        return None;
    }
    let mut point = from + delta * lo;
    if normal == Vec3::ZERO {
        let mut nearest = f32::INFINITY;
        for axis in 0..3 {
            for (plane, sign) in [(bounds.min[axis], -1.), (bounds.max[axis], 1.)] {
                let distance = (from[axis] - plane).abs();
                if distance < nearest {
                    nearest = distance;
                    normal = Vec3::ZERO;
                    normal[axis] = sign;
                    point = from;
                    point[axis] = plane;
                }
            }
        }
    }
    Some(Contact {
        fraction: lo,
        point,
        normal,
    })
}

#[derive(Clone, Copy)]
struct Target {
    enemy: Option<usize>,
    position: Vec3,
    rotation: Quat,
    height: f32,
    seated: f32,
}
impl Target {
    fn boxes(self) -> [(HitZone, layout::Aabb); 5] {
        let h = self.height;
        let torso_drop = self.seated * 0.20 * h;
        let head_drop = self.seated * 0.16 * h;
        let z = self.seated * 0.15 * h;
        [
            (
                HitZone::Head,
                layout::Aabb::new(
                    [-0.12 * h, 0.82 * h - head_drop, z - 0.13 * h],
                    [0.12 * h, 1.02 * h - head_drop, z + 0.13 * h],
                ),
            ),
            (
                HitZone::LeftTorso,
                layout::Aabb::new(
                    [-0.19 * h, 0.43 * h - torso_drop, z - 0.15 * h],
                    [0., 0.82 * h - torso_drop, z + 0.15 * h],
                ),
            ),
            (
                HitZone::RightTorso,
                layout::Aabb::new(
                    [0., 0.43 * h - torso_drop, z - 0.15 * h],
                    [0.19 * h, 0.82 * h - torso_drop, z + 0.15 * h],
                ),
            ),
            (
                HitZone::LeftLeg,
                layout::Aabb::new([-0.125 * h, 0.025 * h, -0.12 * h], [0., 0.43 * h, 0.12 * h]),
            ),
            (
                HitZone::RightLeg,
                layout::Aabb::new([0., 0.025 * h, -0.12 * h], [0.125 * h, 0.43 * h, 0.12 * h]),
            ),
        ]
    }
    fn cast(self, from: Vec3, to: Vec3) -> Option<(HitZone, Contact)> {
        let inv = self.rotation.conjugate();
        let a = inv * (from - self.position);
        let b = inv * (to - self.position);
        self.boxes()
            .into_iter()
            .filter_map(|(zone, bounds)| {
                segment_box(a, b, bounds).map(|mut hit| {
                    hit.point = self.position + self.rotation * hit.point;
                    hit.normal = self.rotation * hit.normal;
                    (zone, hit)
                })
            })
            .min_by(|a, b| a.1.fraction.total_cmp(&b.1.fraction))
    }
}
#[derive(Clone, Copy)]
struct Surface {
    id: u32,
    bounds: layout::Aabb,
    material: layout::Surface,
    persistent: bool,
}
#[derive(Clone, Copy)]
enum Victim {
    Body(Target, HitZone),
    Chain,
    Surface(Surface),
}

impl Dungeon {
    pub(crate) fn sword_frame(&self) -> SwordFrame {
        SwordFrame {
            eye: self.position + Vec3::Y * if self.crouched { 1.0 } else { 1.65 },
            rotation: Quat::from_rotation_y(-self.yaw) * Quat::from_rotation_x(self.pitch),
        }
    }
    /// Collision and hands share the gradual return from a caught blade to
    /// the player's current physical frame. Motion time freezes with hitstop.
    pub fn sword_motion_frame(&self) -> SwordFrame {
        let physical = self.sword_frame();
        if let (Some(strike), Some(contact)) = (self.combat.active, self.combat.impact_frame) {
            if let Some(at) = strike.surface_stop.or(strike.contact_at) {
                let amount = blade::smooth((strike.elapsed - at) / 0.10);
                if amount <= 0.0 {
                    return contact;
                }
                if amount >= 1.0 {
                    return physical;
                }
                return SwordFrame {
                    eye: contact.eye.lerp(physical.eye, amount),
                    rotation: contact.rotation.slerp(physical.rotation, amount),
                };
            }
        }
        physical
    }
    fn sword_targets(&self) -> Vec<Target> {
        let mut targets = Vec::new();
        if self.warden_health > 0. && self.warden_ai.phase != WardenPhase::Dead {
            targets.push(Target {
                enemy: None,
                position: Vec3::new(self.warden.x, 0., self.warden.y),
                rotation: Quat::from_rotation_y(-self.warden_ai.yaw),
                height: 1.865,
                seated: 1. - self.warden_ai.stand_amount(),
            });
        }
        if self.stage >= 5 {
            targets.extend(
                self.enemies
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.alive())
                    .map(|(i, e)| Target {
                        enemy: Some(i),
                        position: e.position,
                        rotation: Quat::from_rotation_y(-e.yaw),
                        height: e.kind.height(),
                        seated: 0.,
                    }),
            );
        }
        targets
    }
    fn sword_surfaces(&self) -> Vec<Surface> {
        let mut out: Vec<_> = layout::castle()
            .solids
            .iter()
            .map(|s| Surface {
                id: s.index as u32,
                bounds: s.bounds,
                material: s.material,
                persistent: s.kind != layout::SolidKind::PropCollider
                    && s.material != layout::Surface::Grass,
            })
            .collect();
        out.extend(
            layout::basement_strike_surfaces()
                .iter()
                .enumerate()
                .map(|(i, s)| Surface {
                    id: 10_000 + i as u32,
                    bounds: s.bounds,
                    material: s.material,
                    persistent: s.persistent,
                }),
        );
        // While the chain is locked, its explicit target represents the blade
        // contact through the bars. The navigation gate's full opaque AABB
        // would falsely stop every diagonal cut in the empty gaps above it.
        if self.stage != 4 {
            out.push(Surface {
                id: 20_000,
                bounds: self.exit_gate_bounds(),
                material: layout::Surface::Iron,
                persistent: false,
            });
        }
        out.push(Surface {
            id: 20_001,
            bounds: self.quest.gate_bounds(),
            material: layout::Surface::Iron,
            persistent: false,
        });
        if self.gate_open < 0.99 {
            out.push(Surface {
                id: 20_002,
                bounds: layout::Aabb::new(
                    [-0.72 + self.gate_open * 1.55, 0., -0.10],
                    [0.72 + self.gate_open * 1.55, 3.05, 0.10],
                ),
                material: layout::Surface::Iron,
                persistent: false,
            });
        }
        out
    }
    pub fn aim_target(&self) -> Option<AimTarget> {
        if self.stage < 4 {
            return None;
        }
        let eye = self.position + Vec3::Y * if self.crouched { 1.0 } else { 1.65 };
        let rotation = Quat::from_rotation_y(-self.yaw) * Quat::from_rotation_x(self.pitch);
        let eye = eye + rotation * Vec3::new(0., -0.25, -0.47);
        let end = eye + rotation * Vec3::NEG_Z * 1.85;
        let wall = self
            .sword_surfaces()
            .into_iter()
            .filter_map(|s| segment_box(eye, end, s.bounds))
            .map(|h| h.fraction)
            .min_by(f32::total_cmp)
            .unwrap_or(1.);
        self.sword_targets()
            .into_iter()
            .filter_map(|t| t.cast(eye, end).map(|(zone, h)| (t, zone, h)))
            .filter(|(_, _, h)| h.fraction < wall)
            .min_by(|a, b| a.2.fraction.total_cmp(&b.2.fraction))
            .map(|(t, zone, h)| AimTarget {
                enemy_id: t.enemy.map(|i| self.enemies[i].id),
                zone,
                point: h.point,
            })
    }
    pub fn aimed_zone(&self) -> Option<HitZone> {
        self.aim_target().map(|a| a.zone)
    }
    /// Project this world point for the honest neutral-cut cursor. It retains
    /// the blade/eye parallax when no enemy is under the cutting axis.
    pub fn sword_aim_point(&self) -> Vec3 {
        if let Some(target) = self.aim_target() {
            return target.point;
        }
        let view = Quat::from_rotation_y(-self.yaw) * Quat::from_rotation_x(self.pitch);
        let start = self.position
            + Vec3::Y * if self.crouched { 1.0 } else { 1.65 }
            + view * Vec3::new(0., -0.25, -0.47);
        let end = start + view * Vec3::NEG_Z * 1.85;
        self.sword_surfaces()
            .into_iter()
            .filter_map(|s| segment_box(start, end, s.bounds))
            .min_by(|a, b| a.fraction.total_cmp(&b.fraction))
            .map_or(end, |h| h.point)
    }

    /// Sweep the shared blade with sixteen substeps per 60 Hz tick. Both sharp
    /// edges and the centre are tested; connecting tracks catch thin geometry
    /// between poses, including a fast turn of the point.
    pub(crate) fn sword_contact_window(&mut self, strike: Strike, start: f32, end: f32) {
        if start > end || strike.surface_stop.is_some() {
            return;
        }
        let targets = self.sword_targets();
        let surfaces = self.sword_surfaces();
        let to = self.sword_motion_frame();
        let sweep = SwordSweep {
            strike,
            start,
            end,
            from: self.strike_frame.unwrap_or(to),
            to,
            contact_fraction: None,
        };
        self.combat.sweep = Some(sweep);
        let pose = |f: f32| {
            let p = sweep.world_sample(f);
            (p.position, p.rotation)
        };
        let point = |(p, r): (Vec3, Quat), x: f32, z: f32| p + r * Vec3::new(x, 0., z);
        let mut previous = pose(0.);
        for sub in 0..=16 {
            let fraction = sub as f32 / 16.;
            let at = start + (end - start) * fraction;
            let eye = sweep.frame(fraction).eye;
            let current = pose(fraction);
            let mut nearest: Option<(f32, Victim, Contact, Vec3)> = None;
            let mut rays = Vec::with_capacity(18);
            for z in [-0.11, 0., 0.11] {
                rays.push((point(current, 0.02, z), point(current, 1.85, z), 1.0));
                for x in [0.02, 0.48, 0.94, 1.40, 1.85] {
                    rays.push((point(previous, x, z), point(current, x, z), 0.0));
                }
            }
            for (from, to, rank) in rays {
                let motion = (point(pose((fraction + 0.01).min(1.)), 1., 0.)
                    - point(pose((fraction - 0.01).max(0.)), 1., 0.))
                .normalize_or_zero();
                let mut consider = |victim: Victim, h: Contact| {
                    let score = rank + h.fraction;
                    if nearest.as_ref().is_none_or(|n| score < n.0) {
                        nearest = Some((score, victim, h, motion));
                    }
                };
                for s in &surfaces {
                    if let Some(h) = segment_box(from, to, s.bounds) {
                        consider(Victim::Surface(*s), h);
                    }
                }
                if !strike.contact_done {
                    let visible = |h: Contact| {
                        !surfaces.iter().any(|s| {
                            segment_box(eye, h.point, s.bounds).is_some_and(|w| w.fraction < 0.999)
                        })
                    };
                    for target in &targets {
                        if let Some((zone, h)) = target.cast(from, to) {
                            if visible(h) {
                                consider(Victim::Body(*target, zone), h);
                            }
                        }
                    }
                    if self.stage == 4 {
                        if let Some(h) = segment_box(
                            from,
                            to,
                            layout::Aabb::new([-0.28, 0.72, -6.88], [0.28, 1.56, -6.68]),
                        ) {
                            if visible(h) {
                                consider(Victim::Chain, h);
                            }
                        }
                    }
                }
            }
            if let Some((_, victim, hit, direction)) = nearest {
                if let Some(s) = &mut self.combat.sweep {
                    s.contact_fraction = Some(fraction);
                }
                self.resolve_sword_hit(strike, at, victim, hit, direction);
                return;
            }
            previous = current;
        }
    }
    fn resolve_sword_hit(
        &mut self,
        strike: Strike,
        at: f32,
        victim: Victim,
        hit: Contact,
        direction: Vec3,
    ) {
        let strength = if strike.kind.heavy() { 0.85 } else { 0.45 };
        let tangent = (direction - hit.normal * direction.dot(hit.normal)).normalize_or_zero();
        let tangent = if tangent.length_squared() > 0.01 {
            tangent
        } else {
            hit.normal.any_orthonormal_vector()
        };
        if let Some(active) = &mut self.combat.active {
            active.elapsed = at;
            active.contact_done = true;
            active.contact_at.get_or_insert(at);
            if matches!(victim, Victim::Surface(_)) {
                active.surface_stop = Some(at);
            }
        }
        match victim {
            Victim::Body(target, zone) => {
                let previous = target
                    .enemy
                    .and_then(|i| self.enemies[i].reaction)
                    .or_else(|| {
                        if target.enemy.is_none() {
                            self.warden_ai.reaction
                        } else {
                            None
                        }
                    });
                let reaction = HitReaction {
                    id: self.combat.impact_event.wrapping_add(1),
                    time: self.time,
                    zone,
                    point: hit.point,
                    direction,
                    strength,
                    elapsed: 0.,
                    origin: previous.map_or([Vec3::ZERO; 5], HitReaction::vectors),
                };
                if let Some(i) = target.enemy {
                    self.apply_enemy_sword_hit(i, strike.kind, hit.point, reaction);
                } else {
                    self.warden_health = (self.warden_health
                        - strike.kind.damage() * if self.werewolf { 1.25 } else { 1. })
                    .max(0.);
                    self.warden_ai.reaction = Some(reaction);
                    self.warden_ai
                        .on_sword_hit(strike.kind.heavy(), self.warden_health == 0.);
                    self.alert = 1.;
                    if self.warden_health == 0. {
                        self.dialogue.emit(crate::VoiceKind::WardenDeath, self.time);
                    }
                    self.combat
                        .impact(strike.kind, ImpactKind::Warden, hit.point);
                    self.say(if self.warden_health == 0. {
                        "The warden falls."
                    } else {
                        "Your blade finds its mark."
                    });
                }
                self.combat.impact_zone = Some(zone);
            }
            Victim::Chain => {
                if let Some(active) = &mut self.combat.active {
                    active.surface_stop = Some(at);
                }
                self.combat
                    .impact(strike.kind, ImpactKind::Chain, hit.point);
                if self.warden_health <= 0. {
                    self.stage = 5;
                    self.combat.clear_queue();
                    self.say("The chain breaks. The far gate rises.");
                } else {
                    self.say("The warden still holds this dungeon. Defeat him first.");
                }
            }
            Victim::Surface(s) => {
                self.combat
                    .impact(strike.kind, ImpactKind::Surface, hit.point);
                self.combat.impact_surface = Some(s.material);
                self.combat.hitstop_left = if strike.kind.heavy() { 0.075 } else { 0.045 };
                self.surface_impacts.push(SurfaceImpact {
                    id: 0,
                    time: self.time,
                    point: hit.point,
                    normal: hit.normal,
                    tangent,
                    material: s.material,
                    surface_id: s.id,
                    persistent: s.persistent,
                    strength,
                });
            }
        }
        self.combat.impact_normal = hit.normal;
        self.combat.impact_tangent = tangent;
        self.combat.impact_frame = self
            .combat
            .sweep
            .and_then(|s| s.contact_fraction.map(|f| s.frame(f)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Controls, Enemy, EnemyKind, EnemyPhase, STEP, StrikeKind};
    fn arena() -> Dungeon {
        let mut game = Dungeon::default();
        game.stage = 5;
        game.exit_open = 1.;
        game.warden_health = 0.;
        game.position = Vec3::new(0., 0., -30.);
        game.enemies = vec![Enemy::new(
            0,
            EnemyKind::SwordSoldier,
            Vec3::new(0., 0., -31.55),
        )];
        game.enemies[0].cooldown = 100.;
        game
    }
    fn tick(game: &mut Dungeon, n: usize) {
        for _ in 0..n {
            game.tick(Controls::default());
        }
    }
    #[test]
    fn anatomical_zones_rotate_with_the_enemy_not_the_observer() {
        for yaw in [0., std::f32::consts::PI, 0.83] {
            let target = Target {
                enemy: Some(0),
                position: Vec3::new(2., 6., -10.),
                rotation: Quat::from_rotation_y(-yaw),
                height: 1.8,
                seated: 0.,
            };
            for (zone, bounds) in target.boxes() {
                let center = Vec3::from_array(bounds.center());
                let from = target.position + target.rotation * (center + Vec3::NEG_Z * 2.);
                let to = target.position + target.rotation * center;
                assert_eq!(
                    target.cast(from, to).unwrap().0,
                    zone,
                    "yaw{yaw} zone{zone:?}"
                );
            }
        }
    }
    #[test]
    fn view_pitch_and_yaw_can_select_all_five_real_zones() {
        let mut game = arena();
        let mut seen = [false; 5];
        for pitch in -70..=35 {
            for yaw in -16..=16 {
                game.pitch = (pitch as f32).to_radians();
                game.yaw = (yaw as f32).to_radians();
                if let Some(zone) = game.aimed_zone() {
                    seen[zone as usize] = true;
                }
            }
        }
        assert!(
            seen.into_iter().all(|x| x),
            "unreachable aim zone: {seen:?}"
        );
        game.enemies[0].position.x = 12.;
        assert!(game.aimed_zone().is_none());
    }
    #[test]
    fn actual_linked_cuts_can_hit_each_zone_with_aim_and_never_twice() {
        let mut seen = [false; 5];
        for kind in [StrikeKind::Cut, StrikeKind::Backhand] {
            for pitch in [-40., -30., -20., -10., 0., 10., 20., 30.] {
                for yaw in [-12., -6., 0., 6., 12.] {
                    let mut game = arena();
                    game.pitch = f32::to_radians(pitch);
                    game.yaw = f32::to_radians(yaw);
                    game.combat.active = Some(Strike::new(kind));
                    tick(&mut game, 60);
                    if let Some(reaction) = game.enemies[0].reaction {
                        let zone = reaction.zone;
                        if !seen[zone as usize]
                            && std::env::var_os("END_GAME_ZONE_REPORT").is_some()
                        {
                            eprintln!(
                                "ZONE {zone:?}: kind={kind:?} player=(0,0,-30) yaw_deg={yaw} pitch_deg={pitch} crouch=false target_initial=(0,0,-31.55) target_yaw=PI contact={:?} phase={:?}",
                                reaction.point, game.combat.active
                            );
                        }
                        seen[zone as usize] = true;
                        assert_eq!(game.enemies[0].health, 70. - kind.damage());
                        assert_eq!(game.enemies[0].hit_event, 1);
                        assert!(game.enemies[0].reaction.unwrap().direction.is_finite());
                    }
                }
            }
        }
        assert!(
            seen.into_iter().all(|x| x),
            "actual blade missed zones: {seen:?}"
        );
    }
    #[test]
    fn chain_has_a_reachable_physical_blade_approach_inside_the_prompt_range() {
        let mut results = Vec::new();
        for z in [-4.7, -4.8, -4.9, -5.0, -5.2, -5.4, -5.8, -6.1] {
            let mut game = arena();
            game.stage = 4;
            game.position = Vec3::new(0., 0., z);
            game.enemies.clear();
            game.tick(Controls {
                attack: true,
                ..Controls::default()
            });
            tick(&mut game, 80);
            results.push((
                z,
                game.stage,
                game.combat.impact_kind,
                game.combat.impact_point,
            ));
        }
        assert!(
            results.iter().any(|r| r.1 == 5),
            "chain approaches: {results:?}"
        );
        assert!(results.iter().filter(|r| r.0 >= -5.0).all(|r| r.1 == 5));
        assert!(
            results
                .iter()
                .filter(|r| r.0 <= -5.2)
                .all(|r| r.2 == Some(ImpactKind::Surface)),
            "close cuts must meet the visible jamb"
        );
    }
    #[test]
    fn intervening_wall_blocks_enemy_and_emits_flush_oriented_surface_impact() {
        let mut game = arena();
        game.position = Vec3::new(1.55, 0., -14.);
        game.yaw = std::f32::consts::FRAC_PI_2;
        game.enemies[0].position = Vec3::new(3., 0., -14.);
        game.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        tick(&mut game, 70);
        assert_eq!(game.enemies[0].health, 70.);
        assert!(game.aimed_zone().is_none());
        let hit = game
            .surface_impacts
            .impacts()
            .next()
            .expect("wall was visible to the blade but generated no impact");
        assert!(hit.persistent);
        assert_eq!(hit.material, layout::Surface::Stone);
        assert!((hit.point.x - 2.).abs() < 0.001);
        assert!(hit.normal.dot(Vec3::NEG_X) > 0.99);
        assert!(hit.tangent.dot(hit.normal).abs() < 0.001);
        assert_eq!(game.surface_impacts.impacts().count(), 1);
    }
    #[test]
    fn thin_plane_between_sample_positions_is_not_tunneled() {
        let hit = segment_box(
            Vec3::new(-2., 1., 0.),
            Vec3::new(2., 1., 0.),
            layout::Aabb::new([0., 0., -1.], [0.002, 2., 1.]),
        )
        .unwrap();
        assert_eq!(hit.point, Vec3::Y);
        assert_eq!(hit.normal, Vec3::NEG_X);
        let inside = segment_box(
            Vec3::new(0.001, 1., 0.),
            Vec3::new(0.001, 1., 0.),
            layout::Aabb::new([0., 0., -1.], [0.002, 2., 1.]),
        )
        .unwrap();
        assert!(inside.normal.length() > 0.99);
        assert!(inside.point.x == 0. || inside.point.x == 0.002);
    }
    #[test]
    fn direct_ground_heavy_and_atomic_jump_heavy_are_distinct() {
        let mut ground = arena();
        ground.enemies.clear();
        ground.tick(Controls {
            heavy: true,
            attack: true,
            ..Controls::default()
        });
        assert_eq!(ground.combat.active.unwrap().kind, StrikeKind::Overhead);
        assert_eq!(ground.combat.queued_count(), 0);
        assert!(ground.grounded);
        let mut jump = arena();
        jump.enemies.clear();
        jump.tick(Controls {
            heavy: true,
            jump: true,
            attack: true,
            ..Controls::default()
        });
        assert_eq!(jump.combat.active.unwrap().kind, StrikeKind::JumpHeavy);
        assert!(!jump.grounded);
        assert!(jump.velocity_y > 3.9);
        assert!((jump.stamina - (48. + 16. * STEP)).abs() < 0.001);
        let mut tired = arena();
        tired.enemies.clear();
        tired.stamina = 51.;
        tired.tick(Controls {
            heavy: true,
            jump: true,
            ..Controls::default()
        });
        assert!(tired.combat.active.is_none());
        assert!(tired.grounded);
        assert_eq!(tired.combat.swing_event, 0);
    }
    #[test]
    fn falling_heavy_holds_recovery_without_hovering_or_repeat_landing_damage() {
        let mut game = arena();
        game.enemies.clear();
        game.position.y = 5.;
        game.grounded = false;
        game.tick(Controls {
            heavy: true,
            ..Controls::default()
        });
        tick(&mut game, 40);
        let active = game.combat.active.unwrap();
        assert!(active.landing_wait);
        assert!((active.elapsed - StrikeKind::JumpHeavy.follow_end()).abs() < 0.001);
        assert!(game.position.y < 3.);
        assert!(game.velocity_y < -6.);
        assert!(active.contact_done);
        assert_eq!(game.combat.impact_event, 0);
        game.tick(Controls {
            heavy: true,
            ..Controls::default()
        });
        assert_eq!(game.combat.swing_event, 1);
        tick(&mut game, 100);
        assert!(game.grounded);
        assert!(game.combat.finished());
        assert_eq!(game.combat.swing_event, 1);
        assert!(game.enemies.is_empty());
    }
    #[test]
    fn jump_heavy_hitstop_freezes_gravity_and_recovery_together() {
        let mut game = arena();
        game.enemies.clear();
        game.tick(Controls {
            jump: true,
            heavy: true,
            ..Controls::default()
        });
        game.combat
            .impact(StrikeKind::JumpHeavy, ImpactKind::Enemy, Vec3::ZERO);
        let before = game.clone();
        game.tick(Controls::default());
        assert_eq!(game.position, before.position);
        assert_eq!(game.velocity_y, before.velocity_y);
        assert_eq!(game.combat.active, before.combat.active);
    }
    #[test]
    fn fast_look_and_body_motion_sweep_between_physical_tick_frames() {
        for translated in [false, true] {
            let mut game = arena();
            let eye = game.position + Vec3::Y * 1.65;
            game.strike_frame = Some(SwordFrame {
                eye: eye + if translated { Vec3::NEG_X } else { Vec3::ZERO },
                rotation: Quat::from_rotation_y(if translated { 0. } else { 0.9 }),
            });
            if translated {
                game.position.x = 1.;
            } else {
                game.yaw = 0.9;
            }
            let strike = Strike::new(StrikeKind::Cut);
            game.combat.active = Some(strike);
            game.sword_contact_window(
                strike,
                strike.kind.contact_time(),
                strike.kind.contact_time(),
            );
            assert_eq!(
                game.enemies[0].hit_event, 1,
                "frame motion omitted: translated{translated}"
            );
            let sweep = game.combat.sweep.unwrap();
            let fraction = sweep.contact_fraction.unwrap();
            assert_eq!(game.combat.impact_frame, Some(sweep.frame(fraction)));
            assert!(fraction > 0. && fraction < 1.);
            assert_eq!(
                game.combat.active.unwrap().elapsed,
                strike.kind.contact_time()
            );
            assert!(sweep.world_sample(fraction).position.is_finite());
        }
    }
    #[test]
    fn rejected_jump_heavy_never_falls_back_to_a_light_attack() {
        let mut game = arena();
        game.stamina = 51.;
        game.tick(Controls {
            jump: true,
            heavy: true,
            attack: true,
            ..Controls::default()
        });
        assert_eq!(game.combat.swing_event, 0);
        assert!(game.grounded);
        assert!(game.combat.active.is_none());
    }
    #[test]
    fn body_contact_frame_returns_gradually_and_remains_the_collision_frame() {
        let mut game = arena();
        game.enemies.clear();
        let physical = game.sword_frame();
        let start = SwordFrame {
            eye: physical.eye - Vec3::X * 0.30,
            rotation: Quat::from_rotation_y(-0.50),
        };
        let mut strike = Strike::new(StrikeKind::Cut);
        strike.elapsed = 0.34;
        strike.contact_at = Some(strike.elapsed);
        strike.contact_done = true;
        let contact = SwordSweep {
            strike,
            start: 0.32,
            end: 0.37,
            from: start,
            to: physical,
            contact_fraction: Some(0.4),
        }
        .frame(0.4);
        game.combat.active = Some(strike);
        game.combat.impact_frame = Some(contact);
        game.combat.hitstop_left = 2.0 * STEP;
        game.strike_frame = Some(contact);
        tick(&mut game, 2);
        assert_eq!(game.sword_motion_frame(), contact);
        let mut previous = contact;
        for step in 1..=6 {
            game.tick(Controls::default());
            let sweep = game.combat.sweep.expect("active return must remain swept");
            let current = game.sword_motion_frame();
            assert_eq!(sweep.from, previous);
            assert_eq!(sweep.to, current);
            assert_eq!(game.strike_frame, Some(current));
            assert!(current.eye.distance(previous.eye) < 0.06);
            let turn = 2.0
                * current
                    .rotation
                    .dot(previous.rotation)
                    .abs()
                    .clamp(0., 1.)
                    .acos();
            assert!(turn < 0.10, "return snapped by {turn} on tick {step}");
            if step == 1 {
                assert!(current.eye.distance(contact.eye) < 0.01);
                assert!(current.eye.distance(physical.eye) > 0.15);
            }
            assert!(
                current.eye.distance(physical.eye) <= previous.eye.distance(physical.eye) + 0.00001
            );
            previous = current;
        }
        assert!(previous.eye.distance(physical.eye) < 0.00001);
        assert!(previous.rotation.dot(physical.rotation).abs() > 0.99999);
    }
    #[test]
    fn reaction_restart_preserves_each_zone_and_dead_snapshot() {
        let mut game = arena();
        let first = HitReaction {
            id: 1,
            time: 0.,
            zone: HitZone::LeftTorso,
            point: Vec3::ZERO,
            direction: Vec3::X,
            strength: 0.8,
            elapsed: 0.07,
            origin: [Vec3::ZERO; 5],
        };
        let second = HitReaction {
            id: 2,
            time: 0.1,
            zone: HitZone::Head,
            point: Vec3::Y,
            direction: Vec3::NEG_Y,
            strength: 1.,
            elapsed: 0.,
            origin: first.vectors(),
        };
        assert_eq!(second.vectors(), first.vectors());
        game.enemies[0].reaction = Some(second);
        game.enemies[0].take_hit(999., true);
        assert_eq!(game.enemies[0].phase, EnemyPhase::Dead);
        assert_eq!(game.enemies[0].interrupted.unwrap().reaction, Some(second));
    }
    #[test]
    fn surface_history_is_bounded_and_checkpoint_clears_marks_without_reusing_ids() {
        let mut game = arena();
        for _ in 0..90 {
            game.surface_impacts.push(SurfaceImpact {
                id: 0,
                time: 0.,
                point: Vec3::ZERO,
                normal: Vec3::Y,
                tangent: Vec3::X,
                material: layout::Surface::Stone,
                surface_id: 1,
                persistent: true,
                strength: 1.,
            });
        }
        assert_eq!(game.surface_impacts.impacts().count(), 64);
        assert_eq!(game.surface_impacts.impacts().next().unwrap().id, 27);
        game.combat.swing_event = 12;
        game.combat.impact_event = 10;
        game.respawn_castle();
        assert_eq!(game.surface_impacts.impacts().count(), 0);
        assert_eq!(game.surface_impacts.serial, 90);
        assert_eq!(game.combat.swing_event, 12);
        assert_eq!(game.combat.impact_event, 10);
        assert!(game.combat.finished());
    }
}
