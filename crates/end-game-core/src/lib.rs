//! End Game's local, fixed-step dungeon simulation. All lengths are metres.
use glam::{Vec2, Vec3};
pub mod combat;
pub mod dialogue;
pub mod guard;
#[cfg(test)]
mod guard_tests;
pub mod interaction;
pub mod warden;
pub use combat::{Combat, ImpactKind, Strike, StrikeKind};
pub use dialogue::{Dialogue, VoiceEvent, VoiceKind};
pub use guard::{Guard, GuardContact};
pub use interaction::{Interaction, InteractionKind};
pub use warden::{Warden, WardenPhase};

pub const STEP: f32 = 1.0 / 60.0;
pub const GRAVITY: f32 = 9.81;

const COT_MIN: Vec2 = Vec2::new(-2.9, 2.35);
const COT_MAX: Vec2 = Vec2::new(-1.6, 4.75);
// The rotated base encloses the upper block of cell.rs's sword monolith.
const PLINTH_CENTER: Vec2 = Vec2::new(2.6, -4.6);
const PLINTH_HALF: Vec2 = Vec2::new(0.50, 0.45);
const PLINTH_YAW: f32 = -0.23;
const WARDEN_RADIUS: f32 = 0.28;
const WARDEN_MIN: Vec2 = Vec2::new(-4.40, -6.37);
const WARDEN_MAX: Vec2 = Vec2::new(4.40, -0.66);

fn plinth_local(point: Vec2) -> Vec2 {
    let delta = point - PLINTH_CENTER;
    let (sin, cos) = PLINTH_YAW.sin_cos();
    Vec2::new(cos * delta.x - sin * delta.y, sin * delta.x + cos * delta.y)
}

fn plinth_world(point: Vec2) -> Vec2 {
    let (sin, cos) = PLINTH_YAW.sin_cos();
    PLINTH_CENTER
        + Vec2::new(
            cos * point.x + sin * point.y,
            -sin * point.x + cos * point.y,
        )
}

fn warden_walk_clear(from: Vec2, to: Vec2) -> bool {
    let inside = |p: Vec2| p.cmpge(WARDEN_MIN).all() && p.cmple(WARDEN_MAX).all();
    let half = PLINTH_HALF + Vec2::splat(WARDEN_RADIUS);
    inside(from)
        && inside(to)
        && !segment_hits_rect(plinth_local(from), plinth_local(to), -half, half)
}

/// Six-node visibility graph around the only solid inside the corridor. The
/// four corners include body clearance; every movement segment is rechecked.
fn warden_waypoint(from: Vec2, player: Vec2) -> Option<Vec2> {
    let mut goal = player.clamp(WARDEN_MIN, WARDEN_MAX);
    let half = PLINTH_HALF + Vec2::splat(WARDEN_RADIUS + 0.04);
    let mut local = plinth_local(goal);
    if local.abs().cmple(half).all() {
        let clearance = half - local.abs();
        let axis = if clearance.x < clearance.y { 0 } else { 1 };
        local[axis] = if local[axis] < 0.0 {
            -half[axis]
        } else {
            half[axis]
        };
        goal = plinth_world(local);
    }
    if warden_walk_clear(from, goal) {
        return Some(goal);
    }
    let nodes = [
        from,
        goal,
        plinth_world(Vec2::new(-half.x, -half.y)),
        plinth_world(Vec2::new(half.x, -half.y)),
        plinth_world(Vec2::new(half.x, half.y)),
        plinth_world(Vec2::new(-half.x, half.y)),
    ];
    let mut distance = [f32::INFINITY; 6];
    let mut previous = [usize::MAX; 6];
    let mut visited = [false; 6];
    distance[0] = 0.0;
    for _ in 0..6 {
        let next = (0..6)
            .filter(|&i| !visited[i] && distance[i].is_finite())
            .min_by(|&a, &b| distance[a].total_cmp(&distance[b]));
        let Some(current) = next else { break };
        if current == 1 {
            break;
        }
        visited[current] = true;
        for candidate in 1..6 {
            if visited[candidate] || !warden_walk_clear(nodes[current], nodes[candidate]) {
                continue;
            }
            let cost = distance[current] + nodes[current].distance(nodes[candidate]);
            if cost < distance[candidate] {
                distance[candidate] = cost;
                previous[candidate] = current;
            }
        }
    }
    let mut next = 1;
    for _ in 0..6 {
        match previous[next] {
            0 => return Some(nodes[next]),
            usize::MAX => return None,
            parent => next = parent,
        }
    }
    None
}

fn segment_hits_plinth(from: Vec2, to: Vec2) -> bool {
    segment_hits_rect(
        plinth_local(from),
        plinth_local(to),
        -PLINTH_HALF,
        PLINTH_HALF,
    )
}

fn segment_hits_rect(from: Vec2, to: Vec2, min: Vec2, max: Vec2) -> bool {
    let direction = to - from;
    let mut enter: f32 = 0.0;
    let mut leave: f32 = 1.0;
    for axis in 0..2 {
        if direction[axis] == 0.0 {
            if from[axis] < min[axis] || from[axis] > max[axis] {
                return false;
            }
        } else {
            let a = (min[axis] - from[axis]) / direction[axis];
            let b = (max[axis] - from[axis]) / direction[axis];
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
            if enter > leave {
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Material {
    Oak,
    Iron,
    Stone,
    Leather,
    Cloth,
}

#[derive(Clone, Copy, Debug)]
pub struct Properties {
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub roughness: f32,
    pub metallic: f32,
}
impl Material {
    pub const fn properties(self) -> Properties {
        match self {
            Self::Oak => Properties {
                density: 700.0,
                friction: 0.55,
                restitution: 0.22,
                roughness: 0.86,
                metallic: 0.0,
            },
            Self::Iron => Properties {
                density: 7870.0,
                friction: 0.45,
                restitution: 0.12,
                roughness: 0.38,
                metallic: 0.9,
            },
            Self::Stone => Properties {
                density: 2600.0,
                friction: 0.7,
                restitution: 0.05,
                roughness: 0.95,
                metallic: 0.0,
            },
            Self::Leather => Properties {
                density: 860.0,
                friction: 0.65,
                restitution: 0.08,
                roughness: 0.72,
                metallic: 0.0,
            },
            Self::Cloth => Properties {
                density: 300.0,
                friction: 0.6,
                restitution: 0.0,
                roughness: 1.0,
                metallic: 0.0,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Body {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: Vec3,
    pub material: Material,
    pub wear: f32,
}
impl Body {
    pub fn mass(&self) -> f32 {
        self.size.x * self.size.y * self.size.z * self.material.properties().density
    }
    pub fn impulse(&mut self, impulse: Vec3) {
        self.velocity += impulse / self.mass().max(0.01);
    }
    pub fn step(&mut self) {
        let p = self.material.properties();
        self.velocity.y -= GRAVITY * STEP;
        self.position += self.velocity * STEP;
        if self.position.y < self.size.y * 0.5 {
            self.position.y = self.size.y * 0.5;
            let impact = -self.velocity.y;
            self.velocity.y = if impact > 0.4 {
                impact * p.restitution
            } else {
                0.0
            };
            let horizontal = Vec2::new(self.velocity.x, self.velocity.z);
            let speed = horizontal.length();
            let reduced = (speed - p.friction * GRAVITY * STEP).max(0.0);
            let v = horizontal.normalize_or_zero() * reduced;
            self.velocity.x = v.x;
            self.velocity.z = v.y;
            self.wear = (self.wear + impact * 0.0005).min(1.0);
        }
        self.position.x = self.position.x.clamp(-2.5, 2.5);
        self.position.z = self.position.z.clamp(0.5, 4.4);
    }
}

#[derive(Default, Clone, Copy)]
pub struct Controls {
    pub movement: Vec2,
    pub sprint: bool,
    pub crouch: bool,
    pub jump: bool,
    pub interact: bool,
    pub attack: bool,
    pub block: bool,
    pub dodge: bool,
    pub transform: bool,
}

#[derive(Clone, Debug)]
pub struct Dungeon {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub velocity_y: f32,
    pub stamina: f32,
    pub health: f32,
    pub stage: u8,
    pub gate_open: f32,
    pub board_open: f32,
    pub interaction: Option<Interaction>,
    pub werewolf: bool,
    pub transformation: f32,
    pub combat: Combat,
    pub guard: Guard,
    /// Compatibility timer: active sword duration remaining, or unarmed cooldown.
    pub attack_time: f32,
    pub crouched: bool,
    pub alert: f32,
    pub warden: Vec2,
    pub warden_health: f32,
    pub warden_ai: Warden,
    pub dialogue: Dialogue,
    pub time: f32,
    pub body: Body,
    pub event: u32,
    pub message: &'static str,
    pub footsteps: u32,
    step_distance: f32,
    dodge_time: f32,
    hit_cooldown: f32,
}

impl Default for Dungeon {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 3.25),
            yaw: 0.0,
            pitch: 0.0,
            velocity_y: 0.0,
            stamina: 100.0,
            health: 100.0,
            stage: 0,
            gate_open: 0.0,
            board_open: 0.0,
            interaction: None,
            werewolf: false,
            transformation: 0.0,
            combat: Combat::default(),
            guard: Guard::default(),
            attack_time: 0.0,
            crouched: false,
            alert: 0.0,
            warden: Vec2::new(-2.9, -3.7),
            warden_health: 100.0,
            warden_ai: Warden::default(),
            dialogue: Dialogue::default(),
            time: 0.0,
            event: 0,
            message: "Cold iron. Old wood. You are still alive.",
            footsteps: 0,
            step_distance: 0.0,
            dodge_time: 0.0,
            hit_cooldown: 0.0,
            body: Body {
                position: Vec3::new(1.8, 0.55, 3.4),
                velocity: Vec3::ZERO,
                size: Vec3::splat(0.65),
                material: Material::Oak,
                wear: 0.12,
            },
        }
    }
}

impl Dungeon {
    /// The final impact and sword recovery remain visible before completion UI.
    pub fn finished(&self) -> bool {
        self.stage == 5 && self.combat.finished()
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos())
    }
    pub fn objective(&self) -> &'static str {
        match self.stage {
            0 => "Search the cell for a way out",
            1 => "Take the key beneath the loose board",
            2 => "Unlock the cell door",
            3 => "Pass the warden. Find the greatsword",
            4 => "Break the chain on the far gate",
            _ => "The dungeon is behind you",
        }
    }
    pub fn distance(&self, x: f32, z: f32) -> f32 {
        Vec2::new(self.position.x - x, self.position.z - z).length()
    }
    pub fn prompt(&self) -> &'static str {
        if self.interaction.is_some() {
            return "";
        }
        match self.stage {
            0 if self.distance(-1.25, 2.1) < 1.6 => "Lift the loose floorboard",
            1 if self.distance(-1.25, 2.1) < 1.6 => "Take the iron key",
            2 if self.distance(0.0, 0.0) < 1.6 => "Unlock the cell",
            3 if self.distance(2.6, -4.6) < 1.9 || self.distance(2.6, -2.95) < 1.25 => {
                "Draw the greatsword from stone"
            }
            4 if self.distance(0.0, -6.5) < 2.0 => "Strike the gate's chain",
            _ => "",
        }
    }
    fn say(&mut self, message: &'static str) {
        self.message = message;
        self.event += 1;
    }
    pub fn interact(&mut self) {
        // Approaches are authored on the floor. Let an airborne player finish
        // their jump instead of interpolating them down through the furniture.
        if self.position.y != 0.0 || self.velocity_y != 0.0 || self.prompt().is_empty() {
            return;
        }
        let kind = match self.stage {
            0 => InteractionKind::Board,
            1 => InteractionKind::Key,
            2 => InteractionKind::Lock,
            3 => InteractionKind::Sword,
            _ => return,
        };
        let approach = kind.approach();
        // Check the whole segment against furniture: a short corner crossing can
        // fall between preflight samples and then intersect an animation tick.
        if segment_hits_rect(
            Vec2::new(self.position.x, self.position.z),
            Vec2::new(approach.x, approach.z),
            COT_MIN,
            COT_MAX,
        ) || segment_hits_plinth(
            Vec2::new(self.position.x, self.position.z),
            Vec2::new(approach.x, approach.z),
        ) {
            self.say("Move to the clear side of the object.");
            return;
        }
        for step in 0..=24 {
            let p = self.position.lerp(approach, step as f32 / 24.0);
            if !self.can_stand(p.x, p.z) {
                self.say("Move to the clear side of the object.");
                return;
            }
        }
        // The interaction owns the arms and the approach from its first tick.
        // A dodge started on an earlier tick must not add motion to that path.
        self.dodge_time = 0.0;
        self.attack_time = 0.0;
        self.combat.cancel();
        self.interaction = Some(Interaction {
            kind,
            elapsed: 0.0,
            origin: self.position,
            committed: false,
        });
    }
    fn commit_interaction(&mut self, kind: InteractionKind) {
        match kind {
            InteractionKind::Board => {
                self.stage = 1;
                self.say("A key, hidden beneath the grain.");
            }
            InteractionKind::Key => {
                self.stage = 2;
                self.say("An iron key. Keep quiet.");
            }
            InteractionKind::Lock => {
                self.stage = 3;
                if self.warden_health > 0.0 && self.warden_ai.phase != WardenPhase::Dead {
                    self.dialogue.emit(VoiceKind::GateUnlocked, self.time);
                }
                self.say("The lock gives. The cell door slides aside.");
            }
            InteractionKind::Sword => {
                self.stage = 4;
                self.werewolf = true;
                self.transformation = 0.0;
                if self.warden_health > 0.0 && self.warden_ai.phase != WardenPhase::Dead {
                    self.dialogue.emit(VoiceKind::SwordClaimed, self.time);
                }
                self.say("The blade remembers. The wolf awakens.");
            }
        }
    }
    fn strike_contact(&mut self, kind: StrikeKind) {
        let forward = self.forward();
        let chain_distance = self.distance(0.0, -6.5);
        let chain_in_reach = chain_distance < 2.0 && forward.z < -0.25;
        let to_warden = Vec3::new(
            self.warden.x - self.position.x,
            0.0,
            self.warden.y - self.position.z,
        );
        let warden_distance = to_warden.length();
        // One contact sample hits the nearest eligible target, never both.
        if self.warden_health > 0.0
            && warden_distance < 2.7
            && forward.dot(to_warden.normalize_or_zero()) > 0.15
            && (!chain_in_reach || warden_distance < chain_distance)
        {
            self.warden_health = (self.warden_health
                - kind.damage() * if self.werewolf { 1.25 } else { 1.0 })
            .max(0.0);
            self.alert = 1.0;
            self.warden_ai
                .on_sword_hit(kind.heavy(), self.warden_health == 0.0);
            if self.warden_health == 0.0 {
                self.dialogue.emit(VoiceKind::WardenDeath, self.time);
            }
            self.combat.impact(
                kind,
                ImpactKind::Warden,
                Vec3::new(self.warden.x, 1.1, self.warden.y),
            );
            self.say(if self.warden_health == 0.0 {
                "The warden falls."
            } else {
                "Iron meets iron."
            });
        } else if chain_in_reach {
            self.stage = 5;
            self.combat
                .impact(kind, ImpactKind::Chain, Vec3::new(0.0, 1.05, -6.88));
            self.combat.clear_queue();
            self.say("Beyond the iron, the hunt begins.");
        }
    }
    fn can_stand(&self, x: f32, z: f32) -> bool {
        if !(-6.65..=4.68).contains(&z) {
            return false;
        }
        if x.abs() > if z > 0.0 { 2.68 } else { 4.68 } {
            return false;
        }
        if z.abs() < 0.38 && (self.stage < 3 || x.abs() > 0.64) {
            return false;
        }
        // The physical aperture follows the sliding gate during its opening motion.
        if z.abs() < 0.38 && x + 0.14 > -0.72 + self.gate_open * 1.55 {
            return false;
        }
        // The cot has a physical footprint, not only a picture.
        if (COT_MIN.x..=COT_MAX.x).contains(&x) && (COT_MIN.y..=COT_MAX.y).contains(&z) {
            return false;
        }
        if plinth_local(Vec2::new(x, z)).abs().cmple(PLINTH_HALF).all() {
            return false;
        }
        // V2 furnishings stay in the corners, with clearance around their visible solids.
        if ((2.18..=3.0).contains(&x) && z > 4.10) || ((-1.89..=-0.97).contains(&x) && z > 4.10) {
            return false;
        }
        true
    }

    fn knife_line_clear(&self, from: Vec2, to: Vec2) -> bool {
        if segment_hits_plinth(from, to) || segment_hits_rect(from, to, COT_MIN, COT_MAX) {
            return false;
        }
        // Test whole segments, including the cell's solid sides and the portion
        // of the sliding bars that still occupies the aperture.
        for (min, max) in [
            (Vec2::new(-4.68, -0.38), Vec2::new(-0.64, 0.38)),
            (Vec2::new(0.64, -0.38), Vec2::new(4.68, 0.38)),
        ] {
            if segment_hits_rect(from, to, min, max) {
                return false;
            }
        }
        let bars_left = if self.stage < 3 {
            -0.64
        } else {
            (-0.72 + self.gate_open * 1.55 - 0.14).max(-0.64)
        };
        bars_left >= 0.64
            || !segment_hits_rect(from, to, Vec2::new(bars_left, -0.38), Vec2::new(0.64, 0.38))
    }

    fn tick_warden(&mut self, running: bool) {
        if self.warden_health <= 0.0 {
            self.dialogue.emit(VoiceKind::WardenDeath, self.time);
        }
        if self.warden_health <= 0.0 && self.warden_ai.phase != WardenPhase::Dead {
            self.warden_ai.on_sword_hit(false, true);
        }
        let phase = self.warden_ai.phase;
        let contact = self.warden_ai.advance();
        if phase == WardenPhase::Dead {
            return;
        }
        let player = Vec2::new(self.position.x, self.position.z);
        let offset = player - self.warden;
        let distance = offset.length();
        if self.stage >= 3 && phase == WardenPhase::Sleeping {
            let noise = distance < 1.0
                || (running && distance < 4.5)
                || (self.attack_time > 0.0 && distance < 6.0);
            if noise {
                self.alert = (self.alert + STEP * 1.4).min(1.0);
            } else if self.alert < 1.0 {
                self.alert = (self.alert - STEP * 0.2).max(0.0);
            }
            if self.alert >= 1.0 {
                self.warden_ai.threaten();
            }
        }
        if contact
            && Vec3::new(offset.x, self.position.y, offset.y).length() <= 1.65
            && self.warden_ai.forward().dot(offset.normalize_or_zero()) >= 0.55
            && self.dodge_time == 0.0
            && self.knife_line_clear(self.warden, player)
        {
            let eye = self.position + Vec3::Y * if self.crouched { 1.0 } else { 1.65 };
            let forward = self.forward();
            let look = Vec3::new(
                forward.x * self.pitch.cos(),
                self.pitch.sin(),
                forward.z * self.pitch.cos(),
            );
            let incoming =
                (Vec3::new(self.warden.x, 1.25, self.warden.y) - eye).normalize_or_zero();
            let result = if look.dot(incoming) >= 0.5 && self.position.y <= 0.001 {
                self.guard
                    .receive(&mut self.stamina, eye + look * 0.65 - Vec3::Y * 0.22)
            } else {
                GuardContact::Open
            };
            if result == GuardContact::Blocked {
                self.combat.hitstop_left = 3.0 * STEP;
                self.warden_ai.on_sword_hit(false, false);
                self.say("Steel catches the knife. Keep your guard toward him.");
            } else {
                self.health -= 15.0;
                self.hit_cooldown = warden::KNIFE_DURATION;
                self.warden_ai.hit_player();
                self.say(if result == GuardContact::Broken {
                    "Your guard breaks. Lower the blade to recover stamina."
                } else {
                    "The knife finds you. Guard toward him or step aside."
                });
            }
        }
        // A phase transition consumes this tick. In particular, the entire
        // wake, knife recovery, or stagger finishes before pursuit can resume.
        if phase != WardenPhase::Hunting {
            return;
        }
        if distance <= 1.25 && self.knife_line_clear(self.warden, player) {
            self.warden_ai.turn_toward(offset);
            if self.warden_ai.forward().dot(offset.normalize_or_zero()) >= 0.90 {
                self.warden_ai.begin_attack();
            }
            return;
        }
        let Some(waypoint) = warden_waypoint(self.warden, player) else {
            self.warden_ai.turn_toward(offset);
            return;
        };
        let toward = waypoint - self.warden;
        self.warden_ai.turn_toward(toward);
        let direction = toward.normalize_or_zero();
        if self.warden_ai.forward().dot(direction) < 0.75 {
            return;
        }
        let travel = toward.length().min(warden::WALK_SPEED * STEP);
        let next = self.warden + direction * travel;
        if warden_walk_clear(self.warden, next) {
            self.warden_ai.walked(self.warden.distance(next));
            self.warden = next;
        }
    }

    pub fn tick(&mut self, mut input: Controls) {
        if self.finished() {
            return;
        }
        if self.stage == 5 {
            let progress = self.combat.tick(false, &mut self.stamina);
            self.attack_time = self.combat.remaining();
            if !progress.frozen {
                self.time += STEP;
                self.dialogue.advance();
            }
            return;
        }
        if input.interact && self.interaction.is_none() {
            self.interact();
        }
        let was_interacting = self.interaction.is_some();
        if let Some(mut action) = self.interaction {
            action.elapsed += STEP;
            self.position = action.origin.lerp(
                action.kind.approach(),
                interaction::ease(0.0, 0.35, action.elapsed),
            );
            self.velocity_y = 0.0;
            if action.kind == InteractionKind::Board {
                self.board_open = action.manipulate();
            }
            if !action.committed && action.elapsed >= action.kind.commit_time() {
                self.commit_interaction(action.kind);
                action.committed = true;
            }
            self.interaction = if action.elapsed >= action.kind.duration() {
                None
            } else {
                Some(action)
            };
            input = Controls::default();
        }
        let old_swing = self.combat.swing_event;
        let progress = self.combat.tick(
            self.stage == 4 && !was_interacting && input.attack && !input.block,
            &mut self.stamina,
        );
        if self.combat.swing_event != old_swing {
            self.event += 1;
        }
        self.attack_time = if self.stage >= 4 {
            self.combat.remaining()
        } else {
            (self.attack_time - STEP).max(0.0)
        };
        if progress.frozen {
            return;
        }
        if let Some(kind) = progress.contact {
            self.strike_contact(kind);
            if self.combat.hitstop_left > 0.0 {
                return;
            }
        }
        self.time += STEP;
        self.dialogue.advance();
        self.guard.tick(
            input.block,
            self.stage == 4
                && !was_interacting
                && self.position.y <= 0.001
                && !input.jump
                && !input.dodge
                && self.dodge_time == 0.0
                && self.combat.finished(),
        );
        self.gate_open = (self.gate_open
            + if self.stage >= 3 {
                STEP / 0.85
            } else {
                -STEP * 3.0
            })
        .clamp(0.0, 1.0);
        if !was_interacting {
            self.board_open = (self.board_open
                + if self.stage > 0 {
                    STEP * 2.0
                } else {
                    -STEP * 3.0
                })
            .clamp(0.0, 1.0);
        }
        if !was_interacting {
            self.crouched = input.crouch;
        }
        self.transformation = (self.transformation - STEP).max(0.0);
        self.dodge_time = (self.dodge_time - STEP).max(0.0);
        self.hit_cooldown = (self.hit_cooldown - STEP).max(0.0);
        if input.transform && self.stage >= 4 && self.transformation == 0.0 {
            self.werewolf = !self.werewolf;
            self.transformation = 1.2;
            self.say(if self.werewolf {
                "Let the wolf in."
            } else {
                "Hold on to what is human."
            });
        }
        let movement = if input.movement.is_finite() {
            input.movement.clamp_length_max(1.0)
        } else {
            Vec2::ZERO
        };
        let forward = self.forward();
        let right = Vec3::new(forward.z * -1.0, 0.0, forward.x);
        let direction = right * movement.x + forward * movement.y;
        let guarding = self.guard.amount > 0.0;
        let running = input.sprint
            && self.stamina > 4.0
            && !input.crouch
            && !guarding
            && movement.length() > 0.1;
        if input.dodge && self.stamina > 24.0 && self.dodge_time == 0.0 {
            self.dodge_time = 0.22;
            self.stamina -= 24.0;
        }
        let speed = if guarding {
            1.0
        } else if input.crouch {
            1.1
        } else if running {
            4.1
        } else {
            2.1
        };
        let delta = if self.dodge_time > 0.0 {
            if direction.length() > 0.1 {
                direction.normalize()
            } else {
                forward
            }
        } else {
            direction
        } * if self.dodge_time > 0.0 { 6.5 } else { speed }
            * STEP;
        let old = self.position;
        if self.can_stand(self.position.x + delta.x, self.position.z)
            && !segment_hits_plinth(
                Vec2::new(self.position.x, self.position.z),
                Vec2::new(self.position.x + delta.x, self.position.z),
            )
        {
            self.position.x += delta.x;
        }
        if self.can_stand(self.position.x, self.position.z + delta.z)
            && !segment_hits_plinth(
                Vec2::new(self.position.x, self.position.z),
                Vec2::new(self.position.x, self.position.z + delta.z),
            )
        {
            self.position.z += delta.z;
        }
        self.stamina = (self.stamina
            + if running {
                -18.0
            } else if guarding {
                0.0
            } else {
                16.0
            } * STEP)
            .clamp(0.0, 100.0);
        if input.jump && self.position.y == 0.0 && self.stamina > 12.0 {
            self.velocity_y = 4.1;
            self.stamina -= 12.0;
        }
        self.velocity_y -= GRAVITY * STEP;
        self.position.y = (self.position.y + self.velocity_y * STEP).max(0.0);
        if self.position.y == 0.0 {
            self.velocity_y = 0.0;
        }
        let walked = Vec2::new(self.position.x - old.x, self.position.z - old.z).length();
        self.step_distance += walked;
        if self.step_distance > 0.65 && self.position.y == 0.0 {
            self.step_distance = 0.0;
            self.footsteps += 1;
            if walked > 0.0
                && !self.crouched
                && self.position.z > 0.38
                && self.warden_health > 0.0
                && self.warden_ai.phase != WardenPhase::Dead
            {
                self.dialogue.emit(VoiceKind::Movement, self.time);
            }
        }
        if self
            .position
            .distance(self.body.position - Vec3::Y * self.body.size.y * 0.5)
            < 0.72
            && walked > 0.0
        {
            self.body.impulse(direction * 22.0);
        }
        self.body.step();
        if self.stage < 4 && input.attack && self.attack_time == 0.0 && self.stamina >= 20.0 {
            self.attack_time = 0.7;
            self.stamina -= 20.0;
            self.event += 1;
            self.say("Your blade is somewhere beyond these bars.");
        }
        self.tick_warden(running);
        if self.health <= 0.0 {
            let next_life = self.dialogue.life.wrapping_add(1);
            *self = Self::default();
            self.dialogue = Dialogue::new(next_life);
            self.say("The dark takes you. Try again.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finish_interaction(s: &mut Dungeon) {
        assert!(s.interaction.is_some(), "interaction did not start");
        for _ in 0..180 {
            s.tick(Controls::default());
            if s.interaction.is_none() {
                return;
            }
        }
        panic!("interaction did not finish: {:?}", s.interaction);
    }

    fn perform_interaction(s: &mut Dungeon, kind: InteractionKind) {
        let stage = s.stage;
        s.tick(Controls {
            interact: true,
            ..Controls::default()
        });
        assert_eq!(s.interaction.map(|action| action.kind), Some(kind));
        assert_eq!(
            s.stage, stage,
            "interaction committed before reaching the object"
        );
        finish_interaction(s);
    }

    fn finish_attack(s: &mut Dungeon) {
        assert!(s.combat.active.is_some());
        for _ in 0..300 {
            s.tick(Controls::default());
            if s.combat.finished() {
                return;
            }
        }
        panic!("combat did not recover: {:?}", s.combat);
    }

    #[test]
    fn locked_bars_and_cot_are_solid() {
        let mut s = Dungeon::default();
        for _ in 0..300 {
            s.tick(Controls {
                movement: Vec2::Y,
                ..Controls::default()
            });
        }
        assert!(s.position.z >= 0.38);
        assert!(!s.can_stand(-2.1, 3.2));
        assert!(!s.can_stand(1.2, 0.0));
    }
    #[test]
    fn complete_progression_and_blade_gate() {
        let mut s = Dungeon::default();
        s.interact();
        assert_eq!(s.stage, 0);
        s.position = Vec3::new(-1.25, 0.0, 2.1);
        perform_interaction(&mut s, InteractionKind::Board);
        assert_eq!(s.stage, 1);
        perform_interaction(&mut s, InteractionKind::Key);
        assert_eq!(s.stage, 2);
        s.position = Vec3::new(0.0, 0.0, 0.8);
        s.interact();
        assert!(!s.can_stand(0.0, 0.0));
        finish_interaction(&mut s);
        for _ in 0..60 {
            s.tick(Controls::default());
        }
        assert!(s.can_stand(0.0, 0.0));
        s.position = Vec3::new(2.6, 0.0, -4.0);
        perform_interaction(&mut s, InteractionKind::Sword);
        assert!(s.werewolf);
        s.position = Vec3::new(0.0, 0.0, -5.4);
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        assert_eq!(s.stage, 4, "chain broke on the click instead of contact");
        finish_attack(&mut s);
        assert_eq!(s.stage, 5);
        assert!(s.finished());
    }

    #[test]
    fn interactions_commit_once_after_contact_and_manipulation() {
        for (stage, kind) in [
            InteractionKind::Board,
            InteractionKind::Key,
            InteractionKind::Lock,
            InteractionKind::Sword,
        ]
        .into_iter()
        .enumerate()
        {
            let mut s = Dungeon {
                stage: stage as u8,
                position: kind.approach(),
                board_open: if stage > 0 { 1.0 } else { 0.0 },
                gate_open: if stage >= 3 { 1.0 } else { 0.0 },
                ..Dungeon::default()
            };
            s.interact();
            let mut previous_elapsed = 0.0;
            let mut saw_contact = false;
            let mut saw_commit = false;
            for _ in 0..180 {
                // Repeated requests cannot restart an action, repeat its effect,
                // or start the next stage while the hand is still recovering.
                s.tick(Controls {
                    interact: true,
                    ..Controls::default()
                });
                if let Some(action) = s.interaction {
                    assert_eq!(action.kind, kind);
                    assert!(action.elapsed > previous_elapsed);
                    previous_elapsed = action.elapsed;
                    if action.elapsed < kind.commit_time() {
                        assert_eq!(s.stage, stage as u8);
                        assert_eq!(s.event, 0);
                        if action.elapsed >= kind.contact_time() {
                            saw_contact = true;
                            assert_eq!(action.reach(), 1.0);
                        }
                        if kind == InteractionKind::Lock {
                            assert_eq!(s.gate_open, 0.0);
                        }
                        if kind == InteractionKind::Sword {
                            assert!(!s.werewolf);
                        }
                    } else {
                        saw_commit = true;
                        assert!(action.committed);
                        assert_eq!(action.grasp(), 1.0);
                        assert_eq!(action.manipulate(), 1.0);
                        assert_eq!(s.stage, stage as u8 + 1);
                        assert_eq!(s.event, 1);
                    }
                } else {
                    break;
                }
            }
            assert!(
                saw_contact && saw_commit,
                "missing contact or commit for {kind:?}"
            );
            assert!(s.interaction.is_none());
            assert_eq!(s.stage, stage as u8 + 1);
            assert_eq!(s.event, 1);
            assert_eq!(s.position, kind.approach());
            for _ in 0..120 {
                s.tick(Controls::default());
            }
            assert_eq!(s.stage, stage as u8 + 1);
            assert_eq!(s.event, 1, "{kind:?} effect repeated after recovery");
        }
    }

    #[test]
    fn interaction_suppresses_competing_controls_from_start_through_recovery() {
        let mut s = Dungeon {
            position: Vec3::new(2.6, 0.0, -3.6),
            stage: 3,
            gate_open: 1.0,
            board_open: 1.0,
            ..Dungeon::default()
        };
        let origin = s.position;
        let mut elapsed = 0.0;
        for _ in 0..180 {
            s.tick(Controls {
                movement: Vec2::ONE,
                sprint: true,
                crouch: true,
                jump: true,
                interact: true,
                attack: true,
                block: true,
                dodge: true,
                transform: true,
            });
            elapsed += STEP;
            let expected = origin.lerp(
                InteractionKind::Sword.approach(),
                interaction::ease(0.0, 0.35, elapsed),
            );
            assert!(s.position.distance(expected) < 0.00001);
            assert_eq!(s.velocity_y, 0.0);
            assert_eq!(s.stamina, 100.0);
            assert_eq!(s.attack_time, 0.0);
            assert!(s.combat.active.is_none());
            assert_eq!(s.combat.queued_count(), 0);
            assert_eq!(s.dodge_time, 0.0);
            assert_eq!(s.transformation, 0.0);
            assert_eq!(s.footsteps, 0);
            assert_eq!(s.alert, 0.0);
            if s.stage == 4 {
                assert!(s.werewolf, "transform input overrode the sword interaction");
            }
            if s.interaction.is_none() {
                break;
            }
        }
        assert!(s.interaction.is_none());
        assert_eq!(s.stage, 4);
        assert_eq!(s.event, 1);

        // Movement resumes on the next tick, without replaying any held edge.
        s.tick(Controls {
            movement: Vec2::X,
            ..Controls::default()
        });
        assert!(s.position.x > InteractionKind::Sword.approach().x);
        assert_eq!(s.attack_time, 0.0);
    }

    #[test]
    fn accepted_interaction_cancels_existing_dodge_and_attack_motion() {
        let mut s = Dungeon {
            position: Vec3::new(-1.25, 0.0, 2.1),
            ..Dungeon::default()
        };
        s.tick(Controls {
            dodge: true,
            attack: true,
            ..Controls::default()
        });
        assert!(s.dodge_time > 0.0 && s.attack_time > 0.0);
        s.interact();
        assert_eq!(s.dodge_time, 0.0);
        assert_eq!(s.attack_time, 0.0);
        let origin = s.position;
        for _ in 0..30 {
            s.tick(Controls::default());
            let action = s.interaction.unwrap();
            let expected = origin.lerp(
                action.kind.approach(),
                interaction::ease(0.0, 0.35, action.elapsed),
            );
            assert!(s.position.distance(expected) < 0.00001);
        }
        finish_interaction(&mut s);
        let recovered = s.position;
        s.tick(Controls::default());
        assert_eq!(s.position, recovered);
    }

    #[test]
    fn airborne_interaction_waits_for_a_grounded_request() {
        let mut s = Dungeon {
            position: Vec3::new(-1.25, 0.0, 2.1),
            ..Dungeon::default()
        };
        s.tick(Controls {
            jump: true,
            ..Controls::default()
        });
        assert!(s.position.y > 0.0);
        let mut expected = s.clone();
        expected.tick(Controls::default());
        s.tick(Controls {
            interact: true,
            ..Controls::default()
        });
        assert!(s.interaction.is_none());
        assert_eq!(s.position, expected.position);
        assert_eq!(s.velocity_y, expected.velocity_y);
        assert_eq!(s.stage, 0);
        for _ in 0..120 {
            s.tick(Controls::default());
        }
        assert_eq!(s.position.y, 0.0);
        assert!(
            s.interaction.is_none(),
            "an airborne request was incorrectly queued"
        );
        perform_interaction(&mut s, InteractionKind::Board);
        assert_eq!(s.stage, 1);
    }

    #[test]
    fn approach_rejects_a_cot_crossing_even_when_both_endpoints_are_clear() {
        for origin in [
            Vec3::new(-2.5, 0.0, 2.2),
            // This corner crossing falls between the old 24 preflight samples.
            Vec3::new(-2.01, 0.0, 1.49),
        ] {
            let mut s = Dungeon {
                position: origin,
                ..Dungeon::default()
            };
            let approach = InteractionKind::Board.approach();
            assert!(s.can_stand(origin.x, origin.z));
            assert!(s.can_stand(approach.x, approach.z));
            assert!(!s.prompt().is_empty());
            s.tick(Controls {
                interact: true,
                ..Controls::default()
            });
            assert!(
                s.interaction.is_none(),
                "approach crossed the cot from {origin:?}"
            );
            assert_eq!(s.position, origin);
            assert_eq!(s.stage, 0);
            assert_eq!(s.message, "Move to the clear side of the object.");
            for _ in 0..120 {
                s.tick(Controls::default());
            }
            assert_eq!(s.stage, 0);
            assert_eq!(s.position, origin);
        }
    }

    #[test]
    fn sword_plinth_blocks_far_side_approaches_and_walking() {
        let mut s = Dungeon {
            position: Vec3::new(2.6, 0.0, -5.5),
            stage: 3,
            gate_open: 1.0,
            ..Dungeon::default()
        };
        let origin = s.position;
        let approach = InteractionKind::Sword.approach();
        assert!(!s.can_stand(2.6, -4.6));
        // Outside the rotated solid, even though inside its axis-aligned bounds.
        assert!(s.can_stand(2.04, -4.10));
        assert!(s.can_stand(origin.x, origin.z));
        assert!(s.can_stand(approach.x, approach.z));
        assert!(!s.prompt().is_empty());
        s.tick(Controls {
            interact: true,
            ..Controls::default()
        });
        assert!(s.interaction.is_none());
        assert_eq!(s.position, origin);
        assert_eq!(s.stage, 3);
        assert_eq!(s.message, "Move to the clear side of the object.");

        s.position = Vec3::new(2.6, 0.0, -3.8);
        for _ in 0..120 {
            s.tick(Controls {
                movement: Vec2::Y,
                ..Controls::default()
            });
            assert!(s.can_stand(s.position.x, s.position.z));
            assert!(s.position.z > -4.15, "walked through the monolith");
        }
        perform_interaction(&mut s, InteractionKind::Sword);
        assert_eq!(s.stage, 4);
    }

    #[test]
    fn dodge_cannot_skip_a_rotated_plinth_corner() {
        let mut s = Dungeon {
            position: Vec3::new(2.94, 0.0, -4.05),
            stage: 3,
            gate_open: 1.0,
            ..Dungeon::default()
        };
        let origin = s.position;
        assert!(s.can_stand(origin.x, origin.z));
        assert!(s.can_stand(origin.x + 6.5 * STEP, origin.z));
        s.tick(Controls {
            movement: Vec2::X,
            dodge: true,
            ..Controls::default()
        });
        assert_eq!(
            s.position, origin,
            "dodge tunneled through the stone corner"
        );
    }

    #[test]
    fn accepted_approach_remains_in_clear_space_and_finishes_before_contact() {
        for (stage, kind, origin) in [
            (0, InteractionKind::Board, Vec3::new(-0.5, 0.0, 2.2)),
            (1, InteractionKind::Key, Vec3::new(-1.25, 0.0, 3.12)),
            (2, InteractionKind::Lock, Vec3::new(0.0, 0.0, 1.4)),
            (3, InteractionKind::Sword, Vec3::new(2.2, 0.0, -3.3)),
        ] {
            let mut s = Dungeon {
                position: origin,
                stage,
                gate_open: if stage >= 3 { 1.0 } else { 0.0 },
                ..Dungeon::default()
            };
            s.interact();
            assert!(s.interaction.is_some(), "{kind:?} approach rejected");
            for _ in 0..180 {
                let previous = s.position;
                s.tick(Controls::default());
                assert!(s.can_stand(s.position.x, s.position.z));
                assert_eq!(s.position.y, 0.0);
                assert!(
                    s.position.distance(previous) < 0.15,
                    "{kind:?} approach teleported"
                );
                if let Some(action) = s.interaction {
                    if action.elapsed >= 0.35 {
                        assert_eq!(s.position, kind.approach());
                        assert!(action.elapsed >= kind.contact_time() || s.stage == stage);
                    }
                } else {
                    break;
                }
            }
            assert!(s.interaction.is_none());
            assert_eq!(s.stage, stage + 1);
        }
    }
    #[test]
    fn density_controls_impulse_and_gravity_is_mass_independent() {
        let a = Dungeon::default().body;
        let mut oak = a.clone();
        let mut iron = a;
        iron.material = Material::Iron;
        oak.impulse(Vec3::X * 100.0);
        iron.impulse(Vec3::X * 100.0);
        assert!(oak.velocity.x > iron.velocity.x * 10.0);
        oak.position.y = 10.0;
        iron.position.y = 10.0;
        oak.step();
        iron.step();
        assert_eq!(oak.velocity.y, iron.velocity.y);
        for _ in 0..600 {
            oak.step();
        }
        assert!((oak.position.y - oak.size.y / 2.0).abs() < 0.001);
        assert!(oak.velocity.length() < 0.01);
    }
    #[test]
    fn diagonal_is_not_faster_and_crouching_stays_quiet() {
        let mut a = Dungeon::default();
        let mut b = a.clone();
        a.tick(Controls {
            movement: Vec2::Y,
            ..Controls::default()
        });
        b.tick(Controls {
            movement: Vec2::ONE,
            ..Controls::default()
        });
        let start = Dungeon::default().position;
        assert!((a.position.distance(start) - b.position.distance(start)).abs() < 0.001);
        a.stage = 3;
        a.position = Vec3::new(-2.9, 0.0, -1.9);
        for _ in 0..120 {
            a.tick(Controls {
                crouch: true,
                ..Controls::default()
            });
        }
        assert_eq!(a.alert, 0.0);
    }

    #[test]
    fn chapter_is_traversable_without_teleporting() {
        fn walk(s: &mut Dungeon, x: f32, z: f32) {
            for _ in 0..1200 {
                let delta = Vec2::new(x - s.position.x, z - s.position.z);
                if delta.length() < 0.10 {
                    return;
                }
                s.yaw = delta.x.atan2(-delta.y);
                s.tick(Controls {
                    movement: Vec2::Y,
                    crouch: true,
                    ..Controls::default()
                });
            }
            panic!("path blocked at {:?}, target {x}, {z}", s.position);
        }
        let mut s = Dungeon::default();
        walk(&mut s, -1.25, 2.1);
        perform_interaction(&mut s, InteractionKind::Board);
        perform_interaction(&mut s, InteractionKind::Key);
        walk(&mut s, 0.0, 0.8);
        perform_interaction(&mut s, InteractionKind::Lock);
        walk(&mut s, 0.0, -1.0);
        walk(&mut s, 2.6, -3.5);
        perform_interaction(&mut s, InteractionKind::Sword);
        assert_eq!(s.stage, 4);
        walk(&mut s, 0.0, -5.5);
        s.yaw = 0.0;
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        finish_attack(&mut s);
        assert_eq!(s.stage, 5);
        assert_eq!(s.health, 100.0);
    }

    #[test]
    fn striking_a_warden_wakes_him() {
        let mut s = Dungeon::default();
        s.stage = 4;
        s.position = Vec3::new(-2.9, 0.0, -1.5);
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        assert_eq!(s.warden_health, 100.0);
        assert_eq!(s.combat.impact_event, 0);
        for _ in 0..60 {
            s.tick(Controls::default());
            if s.combat.impact_event > 0 {
                break;
            }
        }
        assert_eq!(s.alert, 1.0);
        assert_eq!(s.warden_health, 72.0);
        assert_eq!(s.warden_ai.phase, WardenPhase::Waking);
        assert_eq!(s.warden_ai.stand_amount(), 0.0);
    }

    #[test]
    fn each_strike_damages_once_at_contact_with_the_current_form_multiplier() {
        for kind in [
            StrikeKind::Cut,
            StrikeKind::Backhand,
            StrikeKind::Finisher,
            StrikeKind::Overhead,
            StrikeKind::Rising,
        ] {
            for werewolf in [false, true] {
                let mut s = Dungeon {
                    stage: 4,
                    position: Vec3::new(-2.9, 0.0, -1.5),
                    werewolf,
                    ..Dungeon::default()
                };
                s.combat.active = Some(Strike::new(kind));
                for tick in 1..=180 {
                    s.tick(Controls::default());
                    if tick as f32 * STEP + 0.00001 < kind.contact_time() {
                        assert_eq!(s.warden_health, 100.0);
                        assert_eq!(s.combat.impact_event, 0);
                    }
                }
                assert_eq!(
                    s.warden_health,
                    100.0 - kind.damage() * if werewolf { 1.25 } else { 1.0 }
                );
                assert_eq!(s.combat.impact_event, 1);
                assert_eq!(s.combat.impact_kind, Some(ImpactKind::Warden));
                assert_eq!(s.combat.impact_strike, Some(kind));
            }
        }
    }

    #[test]
    fn a_miss_has_no_hitstop_or_impact_and_does_not_hit_late() {
        let mut s = Dungeon {
            stage: 4,
            position: Vec3::new(0.0, 0.0, -3.0),
            warden: Vec2::new(0.0, -1.0),
            ..Dungeon::default()
        };
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        for _ in 0..25 {
            s.tick(Controls::default());
        }
        assert!(s.combat.active.unwrap().contact_done);
        assert_eq!(s.combat.hitstop_left, 0.0);
        assert_eq!(s.combat.impact_event, 0);
        // Moving into the arc during recovery cannot turn an earlier miss into a hit.
        s.warden = Vec2::new(0.0, -4.8);
        finish_attack(&mut s);
        assert_eq!(s.warden_health, 100.0);
        assert_eq!(s.combat.impact_event, 0);
    }

    #[test]
    fn contact_uses_target_position_at_impact_not_at_the_click() {
        let mut s = Dungeon {
            stage: 4,
            position: Vec3::new(0.0, 0.0, -3.0),
            warden: Vec2::new(3.0, -3.0),
            ..Dungeon::default()
        };
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        for _ in 0..8 {
            s.tick(Controls::default());
        }
        assert_eq!(s.warden_health, 100.0);
        s.warden = Vec2::new(0.0, -4.8);
        for _ in 0..20 {
            s.tick(Controls::default());
            if s.combat.impact_event > 0 {
                break;
            }
        }
        assert_eq!(s.warden_health, 72.0);
        assert_eq!(s.combat.impact_point, Vec3::new(0.0, 1.1, -4.8));
    }

    #[test]
    fn real_hitstop_freezes_world_motion_while_buffering_the_next_strike() {
        let mut s = Dungeon {
            stage: 4,
            position: Vec3::new(-2.9, 0.0, -1.5),
            velocity_y: 0.4,
            ..Dungeon::default()
        };
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        for _ in 0..30 {
            s.tick(Controls::default());
            if s.combat.impact_event > 0 {
                break;
            }
        }
        assert_eq!(s.combat.impact_event, 1);
        let frozen = s.clone();
        let mut frozen_ticks = 0;
        while s.combat.hitstop_left > 0.0 {
            s.tick(Controls {
                attack: frozen_ticks == 0,
                movement: Vec2::X,
                sprint: true,
                jump: true,
                dodge: true,
                ..Controls::default()
            });
            frozen_ticks += 1;
            assert_eq!(s.position, frozen.position);
            assert_eq!(s.velocity_y, frozen.velocity_y);
            assert_eq!(s.warden, frozen.warden);
            assert_eq!(s.warden_ai, frozen.warden_ai);
            assert_eq!(s.dialogue, frozen.dialogue);
            assert_eq!(s.body.position, frozen.body.position);
            assert_eq!(s.body.velocity, frozen.body.velocity);
            assert_eq!(s.combat.active, frozen.combat.active);
            assert_eq!(s.stamina, frozen.stamina);
            assert_eq!(s.time, frozen.time);
            assert_eq!(s.health, frozen.health);
        }
        assert_eq!(frozen_ticks, 4);
        assert_eq!(s.combat.queued(), [Some(StrikeKind::Backhand), None]);
        s.tick(Controls {
            movement: Vec2::X,
            ..Controls::default()
        });
        assert!(s.position.x > frozen.position.x);
        assert!(s.combat.active.unwrap().elapsed > frozen.combat.active.unwrap().elapsed);
    }

    #[test]
    fn gate_impact_clears_buffer_and_recovers_before_completion() {
        let mut s = Dungeon {
            stage: 4,
            position: Vec3::new(0.0, 0.0, -5.4),
            ..Dungeon::default()
        };
        for _ in 0..3 {
            s.tick(Controls {
                attack: true,
                ..Controls::default()
            });
        }
        assert_eq!(s.combat.queued_count(), 2);
        assert_eq!(s.stage, 4);
        for _ in 0..30 {
            s.tick(Controls::default());
            if s.stage == 5 {
                break;
            }
        }
        assert_eq!(s.stage, 5);
        assert!(!s.finished());
        assert_eq!(s.combat.queued_count(), 0);
        assert_eq!(s.combat.impact_kind, Some(ImpactKind::Chain));
        assert_eq!(s.combat.hitstop_left, 0.10);
        let mut stopped = 0;
        while s.combat.hitstop_left > 0.0 {
            s.tick(Controls {
                attack: true,
                ..Controls::default()
            });
            stopped += 1;
            assert!(!s.finished());
        }
        assert_eq!(stopped, 6);
        finish_attack(&mut s);
        assert!(s.finished());
        assert_eq!(s.combat.swing_event, 1);
        assert_eq!(s.combat.impact_event, 1);
        let event = s.event;
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        assert_eq!(s.event, event);
    }

    #[test]
    fn pickup_cancels_stale_combat_and_cannot_queue_a_sword_strike() {
        let mut s = Dungeon {
            position: InteractionKind::Board.approach(),
            ..Dungeon::default()
        };
        for _ in 0..3 {
            s.combat.tick(true, &mut s.stamina);
        }
        s.combat
            .impact(StrikeKind::Cut, ImpactKind::Warden, Vec3::X);
        s.interact();
        assert!(s.combat.finished());
        let event = s.combat.swing_event;
        for _ in 0..80 {
            s.tick(Controls {
                attack: true,
                ..Controls::default()
            });
            assert!(s.combat.active.is_none());
            assert_eq!(s.combat.queued_count(), 0);
            assert_eq!(s.combat.swing_event, event);
        }
    }

    fn knife_fixture() -> Dungeon {
        let mut s = Dungeon {
            stage: 4,
            gate_open: 1.0,
            position: Vec3::new(0.0, 0.0, -1.85),
            warden: Vec2::new(0.0, -3.0),
            ..Dungeon::default()
        };
        s.warden_ai.phase = WardenPhase::Hunting;
        s.tick(Controls::default());
        assert_eq!(s.warden_ai.phase, WardenPhase::Attacking);
        assert_eq!(s.warden_ai.elapsed, 0.0);
        assert_eq!(s.warden_ai.attack_event, 1);
        s
    }

    #[test]
    fn alerted_warden_finishes_waking_before_pursuit_or_damage() {
        let mut s = Dungeon {
            stage: 3,
            gate_open: 1.0,
            alert: 1.0,
            position: Vec3::new(-2.9, 0.0, -1.5),
            ..Dungeon::default()
        };
        let origin = s.warden;
        s.tick(Controls::default());
        assert_eq!(s.warden_ai.phase, WardenPhase::Waking);
        let mut stand = 0.0;
        for _ in 0..99 {
            s.tick(Controls::default());
            assert_eq!(s.warden, origin);
            assert_eq!(s.health, 100.0);
            assert_eq!(s.warden_ai.attack_event, 0);
            assert!(s.warden_ai.stand_amount() >= stand);
            stand = s.warden_ai.stand_amount();
        }
        assert_eq!(s.warden_ai.phase, WardenPhase::Hunting);
        assert_eq!(s.warden_ai.knife_draw(), 1.0);
        s.tick(Controls::default());
        assert!(s.warden.y > origin.y);
        assert!(s.warden_ai.walk_phase > 0.0);
    }

    #[test]
    fn close_player_wakes_warden_without_proximity_damage() {
        let mut s = Dungeon {
            stage: 3,
            position: Vec3::new(-2.9, 0.0, -2.9),
            ..Dungeon::default()
        };
        for _ in 0..43 {
            s.tick(Controls::default());
        }
        assert_eq!(s.warden_ai.phase, WardenPhase::Waking);
        assert_eq!(s.health, 100.0);
        for _ in 0..99 {
            s.tick(Controls::default());
            assert_eq!(s.health, 100.0);
        }
        assert_eq!(s.warden_ai.attack_event, 0);
    }

    #[test]
    fn knife_hits_once_at_contact_and_finishes_recovery_before_reattacking() {
        let mut s = knife_fixture();
        let yaw = s.warden_ai.yaw;
        for _ in 0..37 {
            s.tick(Controls::default());
            assert_eq!(s.health, 100.0);
            assert_eq!(s.warden_ai.hit_event, 0);
            assert_eq!(s.warden_ai.yaw, yaw);
        }
        assert!(s.warden_ai.elapsed < warden::KNIFE_CONTACT);
        s.tick(Controls::default());
        assert_eq!(s.health, 85.0);
        assert_eq!(s.warden_ai.hit_event, 1);
        assert_eq!(s.warden_ai.hit_left, warden::HIT_DURATION);
        for _ in 38..86 {
            s.tick(Controls::default());
            assert_eq!(s.health, 85.0);
            assert_eq!(s.warden_ai.hit_event, 1);
            assert_eq!(s.warden_ai.attack_event, 1);
        }
        assert_eq!(s.warden_ai.phase, WardenPhase::Hunting);
        s.tick(Controls::default());
        assert_eq!(s.warden_ai.phase, WardenPhase::Attacking);
        assert_eq!(s.warden_ai.elapsed, 0.0);
        assert_eq!(s.warden_ai.attack_event, 2);
    }

    #[test]
    fn walking_out_during_windup_misses_and_cannot_hit_late() {
        let mut s = knife_fixture();
        for _ in 0..12 {
            s.tick(Controls::default());
        }
        let origin = s.position;
        for _ in 12..38 {
            s.tick(Controls {
                movement: -Vec2::Y,
                ..Controls::default()
            });
        }
        assert!(s.position.z > origin.z + 0.8);
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.hit_event, 0);
        s.position = origin;
        for _ in 38..86 {
            s.tick(Controls::default());
            assert_eq!(s.health, 100.0);
            assert_eq!(s.warden_ai.hit_event, 0);
        }
    }

    #[test]
    fn dodging_at_contact_avoids_a_knife_still_inside_range() {
        let mut s = knife_fixture();
        for _ in 0..36 {
            s.tick(Controls::default());
        }
        s.tick(Controls {
            dodge: true,
            movement: Vec2::X,
            ..Controls::default()
        });
        s.tick(Controls::default());
        assert!(s.dodge_time > 0.0);
        assert!(s.distance(s.warden.x, s.warden.y) < 1.65);
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.hit_event, 0);
    }

    #[test]
    fn knife_direction_locks_at_start_and_cannot_hit_behind_warden() {
        let mut s = knife_fixture();
        let yaw = s.warden_ai.yaw;
        for _ in 0..25 {
            s.tick(Controls::default());
        }
        s.position = Vec3::new(0.0, 0.0, -4.1);
        for _ in 25..38 {
            s.tick(Controls::default());
            assert_eq!(s.warden_ai.yaw, yaw);
        }
        assert!(s.distance(s.warden.x, s.warden.y) < 1.65);
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.hit_event, 0);
    }

    #[test]
    fn light_hits_flinch_but_heavy_hits_cancel_the_pending_knife_contact() {
        for heavy in [false, true] {
            let mut s = knife_fixture();
            for _ in 0..24 {
                s.tick(Controls::default());
            }
            let elapsed = s.warden_ai.elapsed;
            s.strike_contact(if heavy {
                StrikeKind::Overhead
            } else {
                StrikeKind::Cut
            });
            assert!(s.warden_ai.flinch_left > 0.0);
            if heavy {
                assert_eq!(s.warden_ai.phase, WardenPhase::Staggered);
                assert_eq!(s.warden_ai.elapsed, 0.0);
            } else {
                assert_eq!(s.warden_ai.phase, WardenPhase::Attacking);
                assert_eq!(s.warden_ai.elapsed, elapsed);
            }
            for _ in 0..40 {
                s.tick(Controls::default());
            }
            assert_eq!(s.health, if heavy { 100.0 } else { 85.0 });
            assert_eq!(s.warden_ai.hit_event, if heavy { 0 } else { 1 });
        }
    }

    #[test]
    fn death_cancels_a_pending_knife_and_advances_collapse_after_hitstop() {
        let mut s = knife_fixture();
        s.warden_health = 10.0;
        for _ in 0..36 {
            s.tick(Controls::default());
        }
        s.strike_contact(StrikeKind::Cut);
        assert_eq!(s.warden_health, 0.0);
        assert_eq!(s.warden_ai.phase, WardenPhase::Dead);
        assert_eq!(s.warden_ai.stand_amount(), 1.0);
        while s.combat.hitstop_left > 0.0 {
            s.tick(Controls::default());
            assert_eq!(s.warden_ai.elapsed, 0.0);
        }
        for _ in 0..180 {
            s.tick(Controls::default());
        }
        assert!(s.warden_ai.elapsed > 2.99);
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.hit_event, 0);
        assert_eq!(s.warden_ai.attack_event, 1);
    }

    #[test]
    fn knife_cannot_cross_closed_bars_or_cell_walls_but_open_aperture_allows_contact() {
        for (stage, x, expected) in [(2, 0.0, 100.0), (4, 1.0, 100.0), (4, 0.0, 85.0)] {
            let mut s = Dungeon {
                stage,
                gate_open: if stage >= 3 { 1.0 } else { 0.0 },
                warden: Vec2::new(x, -0.66),
                position: Vec3::new(x, 0.0, 0.50),
                ..Dungeon::default()
            };
            s.warden_ai.begin_attack();
            for _ in 0..38 {
                s.tick(Controls::default());
            }
            assert_eq!(s.health, expected, "stage {stage}, x {x}");
        }
    }

    #[test]
    fn knife_cannot_cross_the_sword_plinth() {
        let from = plinth_world(Vec2::new(0.0, -0.70));
        let to = plinth_world(Vec2::new(0.0, 0.70));
        let delta = to - from;
        let mut s = Dungeon {
            stage: 4,
            gate_open: 1.0,
            warden: from,
            position: Vec3::new(to.x, 0.0, to.y),
            ..Dungeon::default()
        };
        s.warden_ai.yaw = delta.x.atan2(-delta.y);
        s.warden_ai.begin_attack();
        for _ in 0..38 {
            s.tick(Controls::default());
        }
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.hit_event, 0);
    }

    #[test]
    fn pursuit_routes_around_plinth_without_crossing_solids_or_sliding_feet() {
        for direction in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
            let from = plinth_world(direction * -1.7);
            let to = plinth_world(direction * 1.7);
            let mut s = Dungeon {
                stage: 4,
                gate_open: 1.0,
                warden: from,
                position: Vec3::new(to.x, 0.0, to.y),
                ..Dungeon::default()
            };
            s.warden_ai.phase = WardenPhase::Hunting;
            let mut traveled = 0.0;
            for _ in 0..600 {
                let before = s.warden;
                let walk = s.warden_ai.walk_phase;
                s.tick(Controls::default());
                assert!(
                    warden_walk_clear(before, s.warden),
                    "{direction:?}: {before:?} -> {:?}",
                    s.warden
                );
                let step = s.warden.distance(before);
                assert!(step <= warden::WALK_SPEED * STEP + 0.00001);
                if step == 0.0 {
                    assert_eq!(s.warden_ai.walk_phase, walk);
                }
                traveled += step;
                if s.warden_ai.attack_event > 0 {
                    break;
                }
            }
            assert_eq!(
                s.warden_ai.attack_event, 1,
                "did not reach {to:?} from {from:?}, ended {:?}",
                s.warden
            );
            assert!(traveled > 2.0);
        }
    }

    #[test]
    fn hunting_stays_in_corridor_when_player_retreats_into_cell() {
        let mut s = Dungeon {
            stage: 3,
            gate_open: 1.0,
            warden: Vec2::new(0.0, -2.0),
            position: Vec3::new(0.0, 0.0, 2.0),
            ..Dungeon::default()
        };
        s.warden_ai.phase = WardenPhase::Hunting;
        for _ in 0..240 {
            let before = s.warden;
            s.tick(Controls::default());
            assert!(warden_walk_clear(before, s.warden));
            assert!(s.warden.y <= WARDEN_MAX.y);
        }
        assert!(s.warden.y > -0.70);
        assert_eq!(s.health, 100.0);
        assert_eq!(s.warden_ai.attack_event, 0);
    }

    #[test]
    fn global_hitstop_freezes_pending_knife_contact_and_feedback() {
        let mut s = knife_fixture();
        for _ in 0..37 {
            s.tick(Controls::default());
        }
        s.warden_ai.hit_left = 0.10;
        s.combat
            .impact(StrikeKind::Cut, ImpactKind::Warden, Vec3::ZERO);
        let frozen = s.warden_ai.clone();
        while s.combat.hitstop_left > 0.0 {
            s.tick(Controls::default());
            assert_eq!(s.warden_ai, frozen);
            assert_eq!(s.health, 100.0);
        }
        s.tick(Controls::default());
        assert_eq!(s.health, 85.0);
        assert_eq!(s.warden_ai.hit_event, 1);
    }

    #[test]
    fn movement_voice_needs_audible_footsteps_not_looking_crouching_or_a_blocked_wall() {
        let mut quiet = Dungeon::default();
        for _ in 0..180 {
            quiet.yaw += STEP;
            quiet.tick(Controls::default());
        }
        assert_eq!(quiet.footsteps, 0);
        assert_eq!(quiet.dialogue.sequence, 0);
        quiet.yaw = 0.0;
        for _ in 0..180 {
            quiet.tick(Controls {
                movement: Vec2::X,
                crouch: true,
                ..Controls::default()
            });
        }
        assert!(quiet.footsteps > 0);
        assert_eq!(quiet.dialogue.sequence, 0);

        let mut audible = Dungeon::default();
        for _ in 0..100 {
            audible.tick(Controls {
                movement: Vec2::X,
                ..Controls::default()
            });
        }
        assert_eq!(audible.dialogue.sequence, 1);
        assert_eq!(
            audible.dialogue.events().next().unwrap().kind,
            VoiceKind::Movement
        );
        let stopped = audible.position;
        let footsteps = audible.footsteps;
        for _ in 0..900 {
            audible.tick(Controls {
                movement: Vec2::X,
                ..Controls::default()
            });
        }
        assert_eq!(audible.position, stopped);
        assert_eq!(audible.footsteps, footsteps);
        assert_eq!(audible.dialogue.sequence, 1);
    }

    #[test]
    fn cell_movement_voice_repeats_after_cooldown_without_waking_the_warden() {
        let mut s = Dungeon::default();
        let mut direction = 1.0;
        for _ in 0..810 {
            if s.position.x > 0.8 {
                direction = -1.0;
            }
            if s.position.x < -0.8 {
                direction = 1.0;
            }
            s.tick(Controls {
                movement: Vec2::X * direction,
                ..Controls::default()
            });
        }
        let events: Vec<_> = s.dialogue.events().copied().collect();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|e| e.kind == VoiceKind::Movement));
        assert!(events[1].time - events[0].time >= 11.999);
        assert_eq!(s.alert, 0.0);
        assert_eq!(s.warden_ai.phase, WardenPhase::Sleeping);

        let mut corridor = Dungeon {
            stage: 3,
            position: Vec3::new(0.0, 0.0, -2.0),
            ..Dungeon::default()
        };
        for _ in 0..60 {
            corridor.tick(Controls {
                movement: Vec2::X,
                ..Controls::default()
            });
        }
        assert!(corridor.footsteps > 0);
        assert_eq!(corridor.dialogue.sequence, 0);
    }

    #[test]
    fn story_voice_fires_at_the_lock_and_sword_commit_not_the_press_or_reach() {
        for (kind, stage, voice) in [
            (InteractionKind::Lock, 2, VoiceKind::GateUnlocked),
            (InteractionKind::Sword, 3, VoiceKind::SwordClaimed),
        ] {
            let mut s = Dungeon {
                stage,
                position: kind.approach(),
                ..Dungeon::default()
            };
            s.tick(Controls {
                interact: true,
                ..Controls::default()
            });
            assert_eq!(s.dialogue.sequence, 0);
            for _ in 0..180 {
                s.tick(Controls {
                    interact: true,
                    ..Controls::default()
                });
                if let Some(action) = s.interaction {
                    if action.elapsed < kind.commit_time() {
                        assert_eq!(s.dialogue.sequence, 0, "early {kind:?} reaction");
                        assert_eq!(s.stage, stage);
                    } else {
                        assert_eq!(s.dialogue.sequence, 1);
                        assert_eq!(s.stage, stage + 1);
                    }
                }
            }
            assert_eq!(s.dialogue.sequence, 1);
            let events: Vec<_> = s.dialogue.events().collect();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].kind, voice);
            assert!(events[0].time >= kind.commit_time() - STEP - 0.00001);
            assert!(events[0].time < kind.commit_time() + STEP);
        }
    }

    #[test]
    fn fatal_sword_voice_is_once_only_and_retained_through_hitstop_and_collapse() {
        let mut s = knife_fixture();
        s.strike_contact(StrikeKind::Cut);
        assert!(s.warden_health > 0.0);
        assert_eq!(s.dialogue.sequence, 0);
        s.warden_health = 10.0;
        s.strike_contact(StrikeKind::Cut);
        assert_eq!(s.dialogue.sequence, 1);
        assert!(s.dialogue.dead());
        assert_eq!(
            s.dialogue.events().next().unwrap().kind,
            VoiceKind::WardenDeath
        );
        let frozen = s.dialogue.clone();
        while s.combat.hitstop_left > 0.0 {
            s.tick(Controls::default());
            assert_eq!(s.dialogue, frozen);
        }
        for _ in 0..180 {
            s.tick(Controls::default());
        }
        s.strike_contact(StrikeKind::Cut);
        assert_eq!(s.dialogue.sequence, 1);
        s.stage = 5;
        for _ in 0..180 {
            s.tick(Controls {
                movement: Vec2::X,
                attack: true,
                ..Controls::default()
            });
        }
        assert_eq!(s.dialogue.sequence, 1);
    }

    #[test]
    fn observed_warden_death_emits_on_tick_and_blocks_later_live_lines() {
        let mut s = Dungeon {
            stage: 3,
            warden_health: 0.0,
            ..Dungeon::default()
        };
        s.tick(Controls::default());
        assert_eq!(s.warden_ai.phase, WardenPhase::Dead);
        assert!(s.dialogue.dead());
        assert_eq!(
            s.dialogue.events().next().unwrap().kind,
            VoiceKind::WardenDeath
        );
        s.commit_interaction(InteractionKind::Lock);
        s.commit_interaction(InteractionKind::Sword);
        for _ in 0..900 {
            s.tick(Controls {
                movement: Vec2::X,
                ..Controls::default()
            });
        }
        assert!(s.footsteps > 0);
        assert_eq!(s.dialogue.sequence, 1);
    }

    #[test]
    fn prisoner_death_changes_voice_life_and_clears_old_story_history() {
        let mut s = Dungeon::default();
        s.commit_interaction(InteractionKind::Lock);
        s.commit_interaction(InteractionKind::Sword);
        let old_life = s.dialogue.life;
        let old_first = *s.dialogue.events().next().unwrap();
        assert_eq!(s.dialogue.sequence, 2);
        s.health = 0.0;
        s.tick(Controls::default());
        assert_eq!(s.stage, 0);
        assert_eq!(s.dialogue.life, old_life + 1);
        assert_eq!(s.dialogue.sequence, 0);
        assert_eq!(s.dialogue.events().count(), 0);
        assert!(!s.dialogue.dead());
        s.commit_interaction(InteractionKind::Lock);
        let new_first = *s.dialogue.events().next().unwrap();
        assert_eq!(new_first.id, old_first.id);
        assert_ne!((s.dialogue.life, new_first.id), (old_life, old_first.id));
        assert_eq!(new_first.kind, VoiceKind::GateUnlocked);
    }
}
