//! Presentation-only action feedback shared by practice and online play.
//!
//! Orders describe submitted intent, never predicted acceptance. Damage values
//! are net HP loss between authoritative snapshots, grouped for ten simulation
//! ticks; they are not attributed to an attacker or relabelled as crit damage.
//! Heal/crit/cast labels come from actual FX. The page deduplicates sound and
//! announcements by (session, id), but updates a grouped number on every poll.

use std::sync::atomic::{AtomicU64, Ordering};

use ember_engine::{Frame, Instance};
use glam::{Quat, Vec3};
use league_core::{data, proto::Cmd};
use serde::Serialize;

use crate::scene::{self, MESH_RING};
use crate::world::{FxLite, UnitLite, World};

const MAX_EVENTS: usize = 48;
static SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub id: u64,
    pub kind: &'static str,
    pub text: String,
    pub amount: Option<f32>,
    pub unit: u32,
    pub source: u32,
    pub ability: u8,
    pub x: f32,
    pub z: f32,
    pub age: f32,
    pub left: f32,
    pub confirmed: bool,
    #[serde(skip)]
    tick: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Order {
    pub kind: &'static str,
    pub x: f32,
    pub z: f32,
    pub target: u32,
    pub age: f32,
    pub left: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Unavailable {
    pub id: u64,
    pub text: String,
    pub slot: Option<u8>,
    pub left: f32,
}

pub struct Presentation {
    pub session: u64,
    pub events: Vec<Event>,
    pub order: Option<Order>,
    pub target: Option<u32>,
    pub unavailable: Option<Unavailable>,
    pub impact: f32,
    next_id: u64,
    last_tick: u64,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            session: SESSION.fetch_add(1, Ordering::Relaxed),
            events: Vec::new(),
            order: None,
            target: None,
            unavailable: None,
            impact: 0.0,
            next_id: 1,
            last_tick: 0,
        }
    }
}

impl Presentation {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn snapshot_tick(&mut self, tick: u64) {
        if tick < self.last_tick {
            self.reset();
        }
        self.last_tick = tick;
    }

    const fn id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    fn push(&mut self, mut event: Event) {
        if !event.x.is_finite() || !event.z.is_finite() {
            return;
        }
        event.id = self.id();
        if self.events.len() >= MAX_EVENTS {
            self.events.remove(0);
        }
        self.events.push(event);
    }

    pub fn tick(&mut self, dt: f32) {
        for event in &mut self.events {
            event.age += dt;
            event.left -= dt;
        }
        self.events.retain(|event| event.left > 0.0);
        if let Some(order) = &mut self.order {
            order.age += dt;
            order.left -= dt;
            if order.left <= 0.0 {
                self.order = None;
            }
        }
        if let Some(hint) = &mut self.unavailable {
            hint.left -= dt;
            if hint.left <= 0.0 {
                self.unavailable = None;
            }
        }
        self.impact = dt.mul_add(-3.0, self.impact).max(0.0);
    }

    pub fn damage(&mut self, unit: &UnitLite, amount: f32, tick: u64, mine: bool) {
        if !amount.is_finite() || amount <= 0.0 || !unit.x.is_finite() || !unit.z.is_finite() {
            return;
        }
        if mine {
            self.impact = self
                .impact
                .max((amount / unit.mh.max(1.0) * 4.0).clamp(0.12, 0.7));
        }
        if let Some(event) = self.events.iter_mut().rev().find(|event| {
            event.kind == "damage" && event.unit == unit.id && tick.saturating_sub(event.tick) <= 10
        }) {
            let total = event.amount.unwrap_or_default() + amount;
            event.amount = Some(total);
            event.text = format!("−{total:.0}");
            event.x = unit.x;
            event.z = unit.z;
            return;
        }
        self.push(Event {
            id: 0,
            kind: "damage",
            text: format!("−{amount:.0}"),
            amount: Some(amount),
            unit: unit.id,
            source: 0,
            ability: u8::MAX,
            x: unit.x,
            z: unit.z,
            age: 0.0,
            left: 1.05,
            confirmed: true,
            tick,
        });
    }

    pub fn effect(&mut self, effect: &FxLite, units: &[UnitLite], my_id: u32, tick: u64) {
        let (kind, label) = match effect.k {
            9 => ("heal", "HEAL".to_string()),
            6 => ("level", "LEVEL UP".to_string()),
            13 if effect.ability < 4 => {
                if effect.source == my_id && my_id != 0 {
                    self.unavailable = None;
                }
                (
                    "cast",
                    ["Q", "W", "E", "R"][usize::from(effect.ability)].to_string(),
                )
            }
            0 if effect.v.is_finite() && effect.v >= 0.0 && effect.v <= 7.0 => {
                // An airborne crit roll is not a confirmed impact. Melee starts
                // have already resolved their hit; objective attacks do too.
                // The flag word is integral by contract; compare bit patterns so
                // no float equality is involved.
                let critical = [1.0_f32, 3.0, 5.0, 7.0]
                    .iter()
                    .any(|flag| flag.to_bits() == effect.v.to_bits());
                let start = effect.v >= 4.0;
                let melee = data::CHAMPS
                    .get(usize::from(effect.champ))
                    .is_some_and(|c| c.range < 2.5);
                let objective = units.iter().any(|u| {
                    u.k >= 4 && (u.x - effect.x2).abs() < 0.1 && (u.z - effect.z2).abs() < 0.1
                });
                if start {
                    self.push(Event {
                        id: 0,
                        kind: "attack",
                        text: String::new(),
                        amount: None,
                        unit: effect.source,
                        source: effect.source,
                        ability: effect.ability,
                        x: effect.x,
                        z: effect.z,
                        age: 0.0,
                        left: 0.35,
                        confirmed: true,
                        tick,
                    });
                    if !critical || (!melee && !objective) {
                        return;
                    }
                }
                if critical {
                    ("crit", "CRIT".to_string())
                } else {
                    ("hit", String::new())
                }
            }
            _ => return,
        };
        let target_point = effect.k == 0 && effect.v >= 4.0;
        self.push(Event {
            id: 0,
            kind,
            text: label,
            amount: None,
            unit: 0,
            source: effect.source,
            ability: effect.ability,
            x: if target_point { effect.x2 } else { effect.x },
            z: if target_point { effect.z2 } else { effect.z },
            age: 0.0,
            left: if kind == "cast" { 0.65 } else { 1.1 },
            confirmed: true,
            tick,
        });
    }

    pub fn command(&mut self, cmd: &Cmd, position: Option<(f32, f32)>, hint: Option<String>) {
        if let Some(text) = hint {
            let id = self.id();
            let slot = match cmd {
                Cmd::Cast { slot, .. } => Some(*slot),
                _ => None,
            };
            self.unavailable = Some(Unavailable {
                id,
                text,
                slot,
                left: 1.6,
            });
        } else if matches!(cmd, Cmd::Cast { .. } | Cmd::Spell { .. } | Cmd::Rank { .. }) {
            self.unavailable = None;
        }
        let next = match *cmd {
            Cmd::Move { x, z } => Some(("move", x, z, 0)),
            Cmd::AttackMove { x, z } => Some(("attack_move", x, z, 0)),
            Cmd::Attack { target } => position.map(|(x, z)| ("attack", x, z, target)),
            _ => None,
        };
        if let Some((kind, x, z, target)) = next
            && x.is_finite()
            && z.is_finite()
        {
            self.order = Some(Order {
                kind,
                x,
                z,
                target,
                age: 0.0,
                left: 0.85,
            });
            self.target = (target != 0).then_some(target);
        }
    }
}

/// Explain only conditions visible in the latest authoritative snapshot.
/// This is a short-lived hint and never decides whether a command is sent.
#[must_use]
pub fn unavailable(world: &World, cmd: &Cmd) -> Option<String> {
    if !matches!(cmd, Cmd::Cast { .. } | Cmd::Spell { .. } | Cmd::Rank { .. }) {
        return None;
    }
    let champion = world.champs.iter().find(|c| c.slot == world.my_slot)?;
    let unit = world.my_unit()?;
    if !champion.alive || unit.dead {
        return Some("Wait until you respawn.".into());
    }
    match *cmd {
        Cmd::Cast { slot, .. } if slot < 4 => {
            let index = usize::from(slot);
            let key = ["Q", "W", "E", "R"][index];
            let rank = champion.ranks[index];
            if rank == 0 {
                return Some(format!("Learn {key} first — spend a skill point with +."));
            }
            // The snapshot exposes remaining Knight blinks. R recasts are free
            // and remain legal while the ordinary ultimate cooldown runs.
            if unit.def == data::KNIGHT && slot == 3 && world.my_blinks > 0 {
                return None;
            }
            if champion.cds[index] > 0.0 {
                return Some(format!("{key} is ready in {:.1}s.", champion.cds[index]));
            }
            let definition = data::CHAMPS.get(usize::from(unit.def))?;
            let ability = [&definition.q, &definition.w, &definition.e, &definition.r][index];
            let required = ability.mana[usize::from(rank.clamp(1, 3) - 1)];
            if unit.mn < required {
                return Some(format!("Not enough mana for {key} — need {required:.0}."));
            }
            None
        }
        Cmd::Spell { slot, .. } if slot < 2 => {
            let remaining = champion.scds[usize::from(slot)];
            (remaining > 0.0).then(|| format!("Spell is ready in {remaining:.1}s."))
        }
        Cmd::Rank { slot } if slot < 4 => {
            let rank = champion.ranks[usize::from(slot)];
            if rank >= 3 {
                return Some("This ability is already fully ranked.".into());
            }
            if slot == 3 && champion.level < data::R_LEVELS[usize::from(rank)] {
                return Some(format!(
                    "Ultimate rank unlocks at level {}.",
                    data::R_LEVELS[usize::from(rank)]
                ));
            }
            (champion.points == 0).then(|| "No skill points — gain a level first.".into())
        }
        _ => None,
    }
}

fn ring(frame: &mut Frame, x: f32, z: f32, radius: f32, colour: Vec3) {
    frame.instances.push(
        Instance::new(
            Vec3::new(x, 0.16, z),
            Vec3::new(radius, 1.0, radius),
            colour,
        )
        .with_mesh(MESH_RING)
        .without_shadow(),
    );
}

/// Append at most fourteen open marker pieces, without altering camera/picking.
pub fn draw(frame: &mut Frame, world: &World) {
    if let Some(order) = &world.feedback.order {
        let shrink = (1.0 - order.age / 0.85).clamp(0.0, 1.0);
        let radius = shrink.mul_add(0.45, 0.65);
        let colour = if order.kind == "move" {
            Vec3::new(0.22, 1.0, 0.8)
        } else {
            Vec3::new(1.0, 0.56, 0.14)
        };
        ring(
            frame,
            order.x,
            order.z,
            radius * 1.18,
            Vec3::new(0.035, 0.08, 0.08),
        );
        ring(frame, order.x, order.z, radius, colour);
        for angle in [
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            3.0 * std::f32::consts::FRAC_PI_2,
        ] {
            let (sin, cos) = angle.sin_cos();
            frame.instances.push(
                Instance::new(
                    Vec3::new(cos.mul_add(1.2, order.x), 0.19, sin.mul_add(1.2, order.z)),
                    Vec3::new(0.35, 0.04, 0.07),
                    colour,
                )
                .with_rot(Quat::from_rotation_y(-angle))
                .without_shadow(),
            );
        }
    }
    if let Some(unit) = world.feedback.target.and_then(|id| {
        world
            .units
            .iter()
            .find(|u| u.id == id && !u.dead && u.x.is_finite() && u.z.is_finite())
    }) {
        let radius = if unit.k >= 4 { 2.35 } else { 1.15 };
        let colour = Vec3::new(1.0, 0.25, 0.13);
        for dx in [-1.0_f32, 1.0] {
            for dz in [-1.0_f32, 1.0] {
                let at = Vec3::new(dx.mul_add(radius, unit.x), 0.22, dz.mul_add(radius, unit.z));
                for scale in [Vec3::new(0.4, 0.045, 0.07), Vec3::new(0.07, 0.045, 0.4)] {
                    frame
                        .instances
                        .push(Instance::new(at, scale, colour).without_shadow());
                }
            }
        }
    }
}

#[must_use]
pub fn json(world: &World) -> serde_json::Value {
    let feedback = &world.feedback;
    let target = feedback
        .target
        .and_then(|id| world.units.iter().find(|u| u.id == id && !u.dead))
        .map(|u| {
            let name = match u.k {
                0 | 3 => data::CHAMPS
                    .get(usize::from(u.def))
                    .map_or("Champion", |c| c.name),
                1 | 2 => "Minion",
                4 => "North Court",
                5 => "South Court",
                _ => "Core",
            };
            serde_json::json!({"id":u.id,"x":u.x,"z":u.z,"hp":u.hp,"mh":u.mh,"name":name})
        });
    let camera = scene::camera_for(world.cam);
    let events: Vec<_> = feedback
        .events
        .iter()
        .filter_map(|event| {
            if !world.view_aspect.is_finite()
                || world.view_aspect <= 0.0
                || event.amount.is_some_and(|amount| amount < 1.0)
            {
                return None;
            }
            let (x, y) = scene::project(&camera, world.view_aspect, event.x, 1.8, event.z);
            if !x.is_finite()
                || !y.is_finite()
                || !(-1.0..=1.0).contains(&x)
                || !(-1.0..=1.0).contains(&y)
            {
                return None;
            }
            let mut value = serde_json::to_value(event).ok()?;
            value["sx"] = serde_json::json!(f32::midpoint(x, 1.0));
            value["sy"] = serde_json::json!((1.0 - y) * 0.5);
            Some(value)
        })
        .collect();
    serde_json::json!({
        "session":feedback.session,"events":events,"order":feedback.order,
        "target":target,"unavailable":feedback.unavailable,"impact":feedback.impact,
        "camera":{"x":world.cam.0,"z":world.cam.1,"height":scene::CAM_HEIGHT,"back":scene::CAM_BACK,"fov":scene::CAM_FOV}
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use league_core::proto::{Phase, S2C, UnitSnap};

    fn row(hp: f32) -> UnitSnap {
        UnitSnap {
            id: 17,
            hp,
            mh: 500,
            x: -40.0,
            k: 0,
            slot: 0,
            ..UnitSnap::default()
        }
    }

    fn effect(kind: u8, flags: f32) -> FxLite {
        FxLite {
            k: kind,
            source: 17,
            champ: data::SWARM,
            ability: 4,
            x: -40.0,
            z: 0.0,
            x2: -38.0,
            z2: 0.0,
            v: flags,
            life: 0.0,
            left: 0.0,
        }
    }

    #[test]
    fn snapshots_group_only_confirmed_net_hp_loss_and_never_invent_an_attacker() {
        let mut world = World::new(1);
        world.set_units(&[row(500.0)]);
        assert!(world.feedback.events.is_empty());
        world.tick = 1;
        world.set_units(&[row(470.0)]);
        let id = world.feedback.events[0].id;
        world.tick = 6;
        world.set_units(&[row(450.0)]);
        world.set_units(&[row(450.0)]); // Repeat polling cannot make another hit.
        assert_eq!(world.feedback.events.len(), 1);
        let event = &world.feedback.events[0];
        assert_eq!(event.id, id);
        assert_eq!(event.amount, Some(50.0));
        assert_eq!((event.source, event.unit, event.kind), (0, 17, "damage"));
        world.tick = 17;
        world.set_units(&[row(400.0)]);
        assert_eq!(world.feedback.events.len(), 2);
        assert_ne!(world.feedback.events[1].id, id);
        assert!(world.feedback.impact > 0.0);
    }

    #[test]
    fn regen_respawn_stat_changes_and_new_match_do_not_masquerade_as_hits() {
        let mut world = World::new(1);
        world.set_units(&[row(400.0)]);
        world.set_units(&[row(450.0)]);
        let mut changed = row(350.0);
        changed.mh = 600;
        world.set_units(&[changed]);
        changed.dead = true;
        world.set_units(&[changed]);
        world.set_units(&[row(300.0)]);
        assert!(world.feedback.events.is_empty());
        world.tick = 20;
        world.set_units(&[row(250.0)]);
        assert_eq!(world.feedback.events.len(), 1);
        let old_session = world.feedback.session;
        world.tick = 0;
        world.set_units(&[]);
        assert_ne!(world.feedback.session, old_session);
        assert!(world.feedback.events.is_empty());
    }

    #[test]
    fn ranged_crit_launch_is_not_a_hit_and_impacts_preserve_exact_source() {
        let mut feedback = Presentation::default();
        feedback.effect(&effect(0, 5.0), &[], 17, 1);
        assert_eq!(feedback.events.len(), 1);
        assert_eq!(feedback.events[0].kind, "attack");
        feedback.effect(&effect(0, 0.0), &[], 17, 2);
        feedback.effect(&effect(0, 1.0), &[], 17, 3);
        assert_eq!(feedback.events[1].kind, "hit");
        assert_eq!(feedback.events[2].kind, "crit");
        assert!(
            feedback
                .events
                .iter()
                .all(|event| event.amount.is_none() && event.source == 17)
        );
        assert_eq!(feedback.events[2].x.to_bits(), (-40.0_f32).to_bits());
    }

    #[test]
    fn other_actors_cannot_clear_your_unavailable_hint() {
        let mut feedback = Presentation::default();
        feedback.command(
            &Cmd::Cast {
                slot: 0,
                x: 0.0,
                z: 0.0,
            },
            None,
            Some("Learn Q first".into()),
        );
        let mut cast = effect(13, 0.0);
        cast.ability = 0;
        cast.source = 18;
        feedback.effect(&cast, &[], 17, 1);
        assert!(feedback.unavailable.is_some());
        cast.source = 17;
        feedback.effect(&cast, &[], 17, 2);
        assert!(feedback.unavailable.is_none());
        feedback.effect(&effect(9, 0.0), &[], 17, 2);
        assert_eq!(feedback.events.last().unwrap().kind, "heal");
        assert!(feedback.events.last().unwrap().amount.is_none());
    }

    #[test]
    fn feedback_is_bounded_expires_and_has_finite_projection_without_edge_pinning() {
        let mut world = World::new(1);
        for _ in 0..100 {
            world.push_fx(effect(9, 0.0));
        }
        assert_eq!(world.feedback.events.len(), MAX_EVENTS);
        let value = json(&world);
        assert_eq!(value["events"].as_array().unwrap().len(), MAX_EVENTS);
        for event in value["events"].as_array().unwrap() {
            assert!((0.0..=1.0).contains(&event["sx"].as_f64().unwrap()));
            assert!((0.0..=1.0).contains(&event["sy"].as_f64().unwrap()));
        }
        world.cam = (4000.0, 0.0);
        let offscreen = json(&world)["events"].as_array().unwrap().clone();
        assert_eq!(offscreen, Vec::<serde_json::Value>::new());
        world.feedback.tick(2.0);
        assert!(world.feedback.events.is_empty());
        assert_eq!(world.feedback.impact.to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn selection_and_order_draws_are_bounded_open_markers_on_the_existing_camera() {
        let mut world = World::new(1);
        world.set_units(&[row(500.0)]);
        world
            .feedback
            .command(&Cmd::Attack { target: 17 }, Some((-40.0, 0.0)), None);
        let frame = world.decorate_frame(Frame::default());
        assert_eq!(frame.instances.len(), 14);
        assert!(frame.instances.iter().all(|i| i.position.is_finite()
            && i.scale.is_finite()
            && i.rot.is_finite()
            && i.mesh <= MESH_RING));
        world
            .feedback
            .command(&Cmd::Move { x: -35.0, z: 0.0 }, None, None);
        assert!(world.feedback.target.is_none());
        assert_eq!(world.decorate_frame(Frame::default()).instances.len(), 6);
        world.feedback.command(
            &Cmd::Move {
                x: f32::NAN,
                z: 0.0,
            },
            None,
            None,
        );
        assert!(
            world
                .decorate_frame(Frame::default())
                .instances
                .iter()
                .all(|i| i.position.is_finite())
        );
    }

    #[test]
    fn hints_follow_learned_cooldown_mana_and_free_knight_recast_snapshot_rules() {
        let mut simulation = league_core::sim::Match::new(1, 3);
        simulation.join("hint fixture");
        simulation.set_pick(0, data::KNIGHT, 0, 1, [0, 1, 2]);
        simulation.start();
        let S2C::State { units, champs, .. } = simulation.snapshot() else {
            panic!("state");
        };
        let mut world = World::new(1);
        world.phase = Phase::Live;
        world.set_units(&units);
        world.champs = champs;
        let cast = Cmd::Cast {
            slot: 0,
            x: 0.0,
            z: 0.0,
        };
        assert!(unavailable(&world, &cast).unwrap().contains("Learn Q"));
        world.champs[0].ranks[0] = 1;
        world.champs[0].cds[0] = 2.5;
        assert!(unavailable(&world, &cast).unwrap().contains("2.5s"));
        world.champs[0].cds[0] = 0.0;
        world
            .units
            .iter_mut()
            .find(|unit| unit.slot == 0)
            .unwrap()
            .mn = 0.0;
        assert!(unavailable(&world, &cast).unwrap().contains("mana"));
        world.champs[0].ranks[3] = 1;
        world.champs[0].cds[3] = 90.0;
        world.my_blinks = 2;
        assert!(
            unavailable(
                &world,
                &Cmd::Cast {
                    slot: 3,
                    x: 0.0,
                    z: 0.0
                }
            )
            .is_none()
        );
        world.my_blinks = 0;
        assert!(
            unavailable(
                &world,
                &Cmd::Cast {
                    slot: 3,
                    x: 0.0,
                    z: 0.0
                }
            )
            .unwrap()
            .contains("90.0s")
        );
    }
}
