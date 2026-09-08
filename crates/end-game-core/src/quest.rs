//! Ordered castle seals, finite shrines and the physical final escape.
use crate::{
    Combat, Dungeon, Guard, STEP,
    enemies::{CastleEventKind, Enemy},
    layout,
};
use glam::Vec3;

pub const CHECKPOINT: Vec3 = Vec3::new(0.0, 0.0, -9.2);
pub const INSCRIPTION: &str =
    "Sun at the dry fountain, wolf on the wall, bell above; crown breaks the final chain.";
pub const SALLY_LIFT: f32 = 3.2;
pub const SALLY_OPEN_TIME: f32 = 1.8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestKind {
    Inscription,
    Sun,
    Wolf,
    Bell,
    SallyPort,
    Shrine(u8),
}
#[derive(Clone, Copy, Debug)]
pub struct QuestSite {
    pub kind: QuestKind,
    pub position: Vec3,
    pub label: &'static str,
    pub text: &'static str,
}
pub const SITES: [QuestSite; 8] = [
    QuestSite {
        kind: QuestKind::Inscription,
        position: Vec3::new(0., 1.5, -24.),
        label: "Read the hall inscription",
        text: INSCRIPTION,
    },
    QuestSite {
        kind: QuestKind::Sun,
        position: Vec3::new(-7., 7.22, -78.9),
        label: "Touch the Sun seal",
        text: "The dry fountain bears a sun: the first light.",
    },
    QuestSite {
        kind: QuestKind::Wolf,
        position: Vec3::new(-24., 15., -83.),
        label: "Touch the Wolf seal",
        text: "The wolf watches from the walls. It follows the sun.",
    },
    QuestSite {
        kind: QuestKind::Bell,
        position: Vec3::new(21., 23., -97.),
        label: "Ring the summit Bell",
        text: "The bell above answers the wolf below. Let it sound last.",
    },
    QuestSite {
        kind: QuestKind::SallyPort,
        position: Vec3::new(-24.75, 7.1, -80.),
        label: "Present the crown at the west gate",
        text: "Three vows wake the lock; the Castellan's crown breaks the final chain.",
    },
    QuestSite {
        kind: QuestKind::Shrine(0),
        position: Vec3::new(-8., 0.8, -50.),
        label: "Drink from the healing shrine",
        text: "One draught remains in this shrine.",
    },
    QuestSite {
        kind: QuestKind::Shrine(1),
        position: Vec3::new(3., 6.8, -70.),
        label: "Drink from the healing shrine",
        text: "One draught remains in this shrine.",
    },
    QuestSite {
        kind: QuestKind::Shrine(2),
        position: Vec3::new(16., 22.8, -97.),
        label: "Drink from the healing shrine",
        text: "One draught remains in this shrine.",
    },
];

#[derive(Clone, Debug, Default)]
pub struct Quest {
    pub seal: bool,
    pub sequence: u8,
    pub gate_open: f32,
    pub gate_unlocked: bool,
    pub escaped: bool,
    pub checkpoint: bool,
    pub inscription_read: bool,
    pub shrines_used: u8,
    pub boss_defeated: bool,
    pub clue_seen: u8,
    pub latest_clue: &'static str,
}
impl Quest {
    pub fn gate_bounds(&self) -> layout::Aabb {
        let bottom = 6. + self.gate_open * SALLY_LIFT;
        layout::Aabb::new([-25.6, bottom, -81.2], [-25., bottom + 3., -78.8])
    }
    pub fn shrine_available(&self, id: u8) -> bool {
        id < 3 && self.shrines_used & (1 << id) == 0
    }
    pub fn healing_left(&self) -> u32 {
        3 - (self.shrines_used & 7).count_ones()
    }
    pub fn active(&self, kind: QuestKind) -> bool {
        match kind {
            QuestKind::Inscription => self.inscription_read,
            QuestKind::Sun => self.sequence >= 1,
            QuestKind::Wolf => self.sequence >= 2,
            QuestKind::Bell => self.sequence >= 3,
            QuestKind::SallyPort => self.gate_unlocked,
            QuestKind::Shrine(id) => !self.shrine_available(id),
        }
    }
    pub fn objective(&self) -> &'static str {
        if self.escaped {
            "You escaped the castle"
        } else if self.gate_unlocked {
            "Walk through the west garden gate to escape"
        } else if !self.inscription_read {
            "Read the inscription at the great hall entrance"
        } else if !self.boss_defeated {
            "Defeat the One-Eyed Castellan and claim his crown seal"
        } else if !self.seal {
            "Take the crown seal from the fallen Castellan"
        } else {
            match self.sequence {
                0 => "Awaken the Sun at the dry garden fountain",
                1 => "Find the Wolf seal on the western castle wall",
                2 => "Ring the Bell at the tower summit",
                _ => "Bring the crown seal to the west garden gate",
            }
        }
    }
    pub fn journal(&self) -> Vec<&'static str> {
        let mut lines = vec![
            "Castle entry checkpoint: defeated enemies and discovered clues survive death.",
            "Three shrines each restore full health once. Their spent draughts remain spent.",
        ];
        if self.inscription_read {
            lines.push(INSCRIPTION);
        }
        if self.clue_seen & 1 != 0 {
            lines.push(SITES[1].text);
        }
        if self.clue_seen & 2 != 0 {
            lines.push(SITES[2].text);
        }
        if self.clue_seen & 4 != 0 {
            lines.push(SITES[3].text);
        }
        if self.boss_defeated {
            lines.push("The One-Eyed Castellan is defeated. His crown is the final key.");
        }
        if self.seal {
            lines.push("The crown seal is yours.");
        }
        if self.sequence >= 1 {
            lines.push("Sun awakened at the dry fountain.");
        }
        if self.sequence >= 2 {
            lines.push("Wolf awakened on the western wall.");
        }
        if self.sequence >= 3 {
            lines.push("The summit bell has answered. The three vows are complete.");
        }
        if self.gate_unlocked {
            lines.push("The west garden sally-port is opening. Follow the short path beyond it.");
        }
        if self.escaped {
            lines.push("Beyond the final chain, dawn belongs to you.");
        }
        lines
    }
}

impl Dungeon {
    pub fn crown_position(&self) -> Option<Vec3> {
        (!self.quest.seal)
            .then(|| {
                self.boss()
                    .filter(|b| !b.alive())
                    .map(|b| b.position + Vec3::Y * 0.2)
            })
            .flatten()
    }
    fn crown_in_reach(&self) -> bool {
        self.crown_position().is_some_and(|p| {
            p.distance(self.position) < 1.9
                && crate::enemies::line_clear(
                    self.position + Vec3::Y * 1.1,
                    p + Vec3::Y * 0.3,
                    &[self.exit_gate_bounds(), self.quest.gate_bounds()],
                )
        })
    }
    pub fn quest_site(&self) -> Option<&'static QuestSite> {
        if self.stage < 5 || !self.grounded {
            return None;
        }
        let chest = self.position + Vec3::Y * 1.1;
        let gates = [self.exit_gate_bounds(), self.quest.gate_bounds()];
        SITES
            .iter()
            .filter(|s| s.position.distance(chest) < 1.75)
            .filter(|s| {
                crate::enemies::line_clear(chest, s.position, &gates)
                    || (s.kind == QuestKind::SallyPort
                        && crate::enemies::line_clear(
                            chest,
                            s.position,
                            &[self.exit_gate_bounds()],
                        ))
            })
            .min_by(|a, b| {
                a.position
                    .distance_squared(chest)
                    .total_cmp(&b.position.distance_squared(chest))
            })
    }
    pub fn quest_prompt(&self) -> &'static str {
        if self.stage < 5 || !self.grounded || self.quest.escaped {
            return "";
        }
        if self.crown_in_reach() {
            return "Take the Castellan's crown seal";
        }
        self.quest_site().map_or("", |s| match s.kind {
            QuestKind::Shrine(id) if !self.quest.shrine_available(id) => {
                "The healing shrine is empty"
            }
            QuestKind::Shrine(_) if self.health >= 100. => "The shrine waits until you are wounded",
            QuestKind::SallyPort if self.quest.gate_unlocked => {
                "The way beyond the final chain is open"
            }
            _ => s.label,
        })
    }
    pub(crate) fn interact_castle(&mut self) {
        if self.health <= 0.0
            || self.combat.hitstop_left > 0.0
            || !self.grounded
            || !self.combat.finished()
            || self.dodge_time > 0.
            || self.quest.escaped
        {
            return;
        }
        if self.crown_in_reach() {
            self.quest.seal = true;
            self.quest.boss_defeated = true;
            self.castle_events
                .emit(CastleEventKind::Seal, self.time, self.position);
            self.say("The crown seal is yours. Sun, Wolf, Bell — then the west garden gate.");
            return;
        }
        let Some(site) = self.quest_site() else {
            return;
        };
        let mut event = None;
        let message = match site.kind {
            QuestKind::Inscription => {
                if !self.quest.inscription_read {
                    event = Some(CastleEventKind::Clue);
                }
                self.quest.inscription_read = true;
                self.quest.latest_clue = INSCRIPTION;
                INSCRIPTION
            }
            QuestKind::Sun | QuestKind::Wolf | QuestKind::Bell => {
                let index = match site.kind {
                    QuestKind::Sun => 0,
                    QuestKind::Wolf => 1,
                    _ => 2,
                };
                self.quest.clue_seen |= 1 << index;
                self.quest.latest_clue = site.text;
                if self.quest.sequence == index {
                    self.quest.sequence += 1;
                    event = Some(match index {
                        0 => CastleEventKind::Sun,
                        1 => CastleEventKind::Wolf,
                        _ => CastleEventKind::Bell,
                    });
                    match index {
                        0 => "Sun awakened. Seek the Wolf on the western wall.",
                        1 => "Wolf awakened. Ring the Bell above the tower.",
                        _ => "The Bell answers. Bring the crown seal to the west garden gate.",
                    }
                } else if self.quest.sequence > index {
                    site.text
                } else {
                    self.quest.sequence = 0;
                    self.quest.latest_clue = "The vows fade. Begin again: Sun at the dry fountain, Wolf on the wall, Bell above.";
                    self.quest.latest_clue
                }
            }
            QuestKind::SallyPort => {
                if self.quest.seal && self.quest.sequence == 3 {
                    if !self.quest.gate_unlocked {
                        event = Some(CastleEventKind::Gate);
                    }
                    self.quest.gate_unlocked = true;
                    "The final chain breaks. Walk west through the rising gate."
                } else {
                    self.quest.latest_clue = site.text;
                    "The gate needs the Castellan's crown and the vows in order: Sun, Wolf, Bell."
                }
            }
            QuestKind::Shrine(id) => {
                if !self.quest.shrine_available(id) {
                    "The shrine is empty."
                } else if self.health >= 100. {
                    "Keep its draught. You are not wounded."
                } else {
                    self.quest.shrines_used |= 1 << id;
                    self.health = 100.;
                    event = Some(CastleEventKind::Heal);
                    "The shrine restores your health. Its single draught is spent."
                }
            }
        };
        if let Some(event) = event {
            self.castle_events.emit(event, self.time, site.position);
        }
        self.say(message);
    }
    pub(crate) fn tick_quest(&mut self) {
        if self.stage < 5 {
            return;
        }
        if !self.quest.checkpoint && self.position.z < -7.7 && self.grounded && self.health > 0. {
            self.quest.checkpoint = true;
            self.castle_events
                .emit(CastleEventKind::Checkpoint, self.time, CHECKPOINT);
            self.say("Castle entry secured. Defeated enemies and your journal survive death.");
        }
        if self.quest.gate_unlocked {
            self.quest.gate_open = (self.quest.gate_open + STEP / SALLY_OPEN_TIME).min(1.);
        }
        if !self.quest.escaped
            && self.quest.gate_unlocked
            && self.quest.gate_open > 0.95
            && self.position.x < -28.
            && (-81.2..=-78.8).contains(&self.position.z)
            && (self.position.y - 6.).abs() < 0.2
            && self.grounded
        {
            self.quest.escaped = true;
            self.castle_events
                .emit(CastleEventKind::Escaped, self.time, self.position);
            self.combat.cancel();
            self.guard.amount = 0.;
            self.say("Beyond the final chain, dawn belongs to you. You escaped.");
        }
    }
    pub(crate) fn respawn_castle(&mut self) {
        self.position = CHECKPOINT;
        self.yaw = 0.;
        self.pitch = 0.;
        self.velocity_y = 0.;
        self.grounded = true;
        self.health = 100.;
        self.stamina = 100.;
        self.crouched = false;
        self.dodge_time = 0.;
        self.hit_cooldown = 0.;
        self.interaction = None;
        self.attack_time = 0.;
        let sword_events = (self.combat.swing_event, self.combat.impact_event);
        self.combat = Combat::default();
        self.combat.swing_event = sword_events.0;
        self.combat.impact_event = sword_events.1;
        self.surface_impacts.clear();
        self.air_heavy_used = false;
        self.strike_frame = None;
        self.guard = Guard::default();
        self.enemy_attack_wait = 1.;
        self.step_distance = 0.;
        // Dead bodies, seal location, shrine use, journal, gate state and event serials persist.
        for e in &mut self.enemies {
            if e.alive() {
                *e = Enemy::new(e.id, e.kind, e.spawn);
            }
        }
        self.say("You return to the castle entry. Your victories and vows remain.");
    }
}
