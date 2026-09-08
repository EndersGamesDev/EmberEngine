//! End Game's local, fixed-step dungeon simulation. All lengths are metres.
use glam::{Vec2, Vec3};

pub const STEP: f32 = 1.0 / 60.0;
pub const GRAVITY: f32 = 9.81;

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
    pub werewolf: bool,
    pub transformation: f32,
    pub attack_time: f32,
    pub crouched: bool,
    pub alert: f32,
    pub warden: Vec2,
    pub warden_health: f32,
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
            werewolf: false,
            transformation: 0.0,
            attack_time: 0.0,
            crouched: false,
            alert: 0.0,
            warden: Vec2::new(-2.9, -3.7),
            warden_health: 100.0,
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
        match self.stage {
            0 if self.distance(-1.25, 2.1) < 1.6 => "Lift the loose floorboard",
            1 if self.distance(-1.25, 2.1) < 1.6 => "Take the iron key",
            2 if self.distance(0.0, 0.0) < 1.6 => "Unlock the cell",
            3 if self.distance(2.6, -4.6) < 1.9 => "Draw the greatsword from stone",
            4 if self.distance(0.0, -6.5) < 2.0 => "Strike the gate's chain",
            _ => "",
        }
    }
    fn say(&mut self, message: &'static str) {
        self.message = message;
        self.event += 1;
    }
    pub fn interact(&mut self) {
        if self.prompt().is_empty() {
            return;
        }
        match self.stage {
            0 => {
                self.stage = 1;
                self.say("A key, hidden beneath the grain.");
            }
            1 => {
                self.stage = 2;
                self.say("An iron key. Keep quiet.");
            }
            2 => {
                self.stage = 3;
                self.say("The lock gives. The warden still sleeps.");
            }
            3 => {
                self.stage = 4;
                self.werewolf = true;
                self.transformation = 2.6;
                self.say("The blade remembers. The wolf awakens.");
            }
            _ => {}
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
        // The cot has a physical footprint, not only a picture.
        if (-2.8..=-1.7).contains(&x) && (2.55..=4.55).contains(&z) {
            return false;
        }
        true
    }
    pub fn tick(&mut self, input: Controls) {
        if self.stage == 5 {
            return;
        }
        self.time += STEP;
        self.crouched = input.crouch;
        self.attack_time = (self.attack_time - STEP).max(0.0);
        self.transformation = (self.transformation - STEP).max(0.0);
        self.dodge_time = (self.dodge_time - STEP).max(0.0);
        self.hit_cooldown = (self.hit_cooldown - STEP).max(0.0);
        if input.interact {
            self.interact();
        }
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
        let running =
            input.sprint && self.stamina > 4.0 && !input.crouch && movement.length() > 0.1;
        if input.dodge && self.stamina > 24.0 && self.dodge_time == 0.0 {
            self.dodge_time = 0.22;
            self.stamina -= 24.0;
        }
        let speed = if input.crouch {
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
        if self.can_stand(self.position.x + delta.x, self.position.z) {
            self.position.x += delta.x;
        }
        if self.can_stand(self.position.x, self.position.z + delta.z) {
            self.position.z += delta.z;
        }
        self.stamina = (self.stamina + if running { -18.0 } else { 16.0 } * STEP).clamp(0.0, 100.0);
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
        if input.attack && self.attack_time == 0.0 && self.stamina >= 20.0 {
            self.attack_time = 0.7;
            self.stamina -= 20.0;
            self.event += 1;
            if self.stage >= 4 {
                if self.distance(0.0, -6.5) < 2.0 && forward.z < -0.25 {
                    self.stage = 5;
                    self.say("Beyond the iron, the hunt begins.");
                }
                let to_warden = Vec3::new(
                    self.warden.x - self.position.x,
                    0.0,
                    self.warden.y - self.position.z,
                );
                if to_warden.length() < 2.7 && forward.dot(to_warden.normalize_or_zero()) > 0.15 {
                    self.warden_health =
                        (self.warden_health - if self.werewolf { 60.0 } else { 35.0 }).max(0.0);
                    self.alert = 1.0;
                    self.say(if self.warden_health == 0.0 {
                        "The warden falls."
                    } else {
                        "Iron meets iron."
                    });
                }
            } else {
                self.say("Your blade is somewhere beyond these bars.");
            }
        }
        if self.stage >= 3 && self.warden_health > 0.0 {
            let dist = self.distance(self.warden.x, self.warden.y);
            let noise =
                dist < 1.0 || (running && dist < 4.5) || (self.attack_time > 0.0 && dist < 6.0);
            if noise {
                self.alert = (self.alert + STEP * 1.4).min(1.0);
            } else if self.alert < 1.0 {
                self.alert = (self.alert - STEP * 0.2).max(0.0);
            }
            if self.alert >= 1.0 {
                let dir =
                    (Vec2::new(self.position.x, self.position.z) - self.warden).normalize_or_zero();
                if dist > 1.1 && self.position.z < -0.6 {
                    self.warden += dir * STEP * 1.25;
                }
                if dist < 1.5 && self.hit_cooldown == 0.0 && self.dodge_time == 0.0 {
                    self.health -= 15.0;
                    self.hit_cooldown = 1.4;
                    self.say("The warden has found you. Move.");
                }
            }
        }
        if self.health <= 0.0 {
            *self = Self::default();
            self.say("The dark takes you. Try again.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        s.interact();
        s.interact();
        assert_eq!(s.stage, 2);
        s.position = Vec3::new(0.0, 0.0, 0.8);
        s.interact();
        assert!(s.can_stand(0.0, 0.0));
        s.position = Vec3::new(2.6, 0.0, -4.0);
        s.interact();
        assert!(s.werewolf);
        s.position = Vec3::new(0.0, 0.0, -5.4);
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
        assert_eq!(s.stage, 5);
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
        s.interact();
        s.interact();
        walk(&mut s, 0.0, 0.8);
        s.interact();
        walk(&mut s, 0.0, -1.0);
        walk(&mut s, 2.6, -3.5);
        s.interact();
        assert_eq!(s.stage, 4);
        walk(&mut s, 0.0, -5.5);
        s.yaw = 0.0;
        s.tick(Controls {
            attack: true,
            ..Controls::default()
        });
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
        assert_eq!(s.alert, 1.0);
        assert!(s.warden_health < 100.0);
    }
}
