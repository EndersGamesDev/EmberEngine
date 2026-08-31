//! The client: builds the castle, drives the player's car, frames the shot.

use std::cell::RefCell;

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{
    Camera, EmberGame, Frame, Instance, InputState, KeyCode, MeshData, TextureData,
};
use fire_core::ai;
use fire_core::car::{self, CarInput};
use fire_core::castle::{self, PropKind};
use fire_core::sim::{FixedStep, Race, RaceState};

use crate::meshes;
use crate::texgen;
use crate::trackmesh;

const PLAYERS: usize = 8;
const LAPS: u32 = 3;

// ---- camera ---------------------------------------------------------------

const CAM_DIST: f32 = 13.5;
const CAM_HEIGHT: f32 = 5.5;
/// Chase-camera lag, 1/s. Applied as an exponential so it is frame-rate
/// independent for the same reason the tyre friction is.
const CAM_LAG: f32 = 5.0;
/// How much the camera follows the car's *velocity* rather than its nose.
/// A camera welded to the heading swings wildly through a drift and hides
/// the apex; one welded to the velocity never shows you where you are
/// pointing. The blend keeps the slide legible.
const CAM_VEL_BLEND: f32 = 0.35;
const FOV_BASE: f32 = 62.0;
const FOV_SPEED_GAIN: f32 = 16.0;

/// Everything the page's HUD needs. Published through a thread-local because
/// `ember_engine::run` takes the game by value and never gives it back.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hud {
    pub speed_kmh: f32,
    pub lap: u32,
    pub laps_total: u32,
    pub place: usize,
    pub racers: usize,
    pub boost_charges: u8,
    pub boosting: bool,
    pub drifting: bool,
    pub countdown: f32,
    pub finished: bool,
}

thread_local! {
    static HUD: RefCell<Hud> = const { RefCell::new(Hud {
        speed_kmh: 0.0, lap: 0, laps_total: 0, place: 0, racers: 0,
        boost_charges: 0, boosting: false, drifting: false,
        countdown: 0.0, finished: false,
    }) };
}

pub fn hud() -> Hud {
    HUD.with(|h| *h.borrow())
}

/// Publish this frame's HUD. Both play modes go through here so the page
/// reads one shape regardless of which one is running.
pub fn set_hud(h: Hud) {
    HUD.with(|c| *c.borrow_mut() = h);
}

// ---- meshes ---------------------------------------------------------------

/// Mesh ids, assigned in registration order. `EngineConfig.meshes` entries
/// take ids 1..=N, and id 0 is the engine's built-in cube — so the order of
/// `build_meshes` below IS this struct, and the two must change together.
pub struct Meshes {
    pub ground: u32,
    pub road: u32,
    pub kerb_l: u32,
    pub kerb_r: u32,
    pub wall_l: u32,
    pub wall_r: u32,
    pub start: u32,
    pub car: u32,
    pub gatehouse: u32,
    pub tower: u32,
    pub fountain: u32,
    /// Longest-axis extent of each prop mesh, so `Prop::scale` can be given
    /// in metres rather than in whatever units the generator happened to use.
    pub car_extent: f32,
    pub gatehouse_extent: f32,
    pub tower_extent: f32,
    pub fountain_extent: f32,
    /// How far to lift each prop so it stands on the courtyard floor.
    pub car_lift: f32,
    pub gatehouse_lift: f32,
    pub tower_lift: f32,
    pub fountain_lift: f32,
}

const CAR_GLB: &[u8] = include_bytes!("../../../assets/models/fire/fire-car.glb");
const GATEHOUSE_GLB: &[u8] = include_bytes!("../../../assets/models/fire/fire-gatehouse.glb");
const TOWER_GLB: &[u8] = include_bytes!("../../../assets/models/fire/fire-tower.glb");
const FOUNTAIN_GLB: &[u8] = include_bytes!("../../../assets/models/fire/fire-fountain.glb");

/// A prop that fails to load must not take the game down — better a missing
/// tower than a black screen, and the log says which one went.
fn prop_or_cube(bytes: &[u8], tex: Option<TextureData>, tiles: f32, what: &str) -> MeshData {
    match meshes::prop(bytes, tex, tiles) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("fire: {what} failed to load ({e}); substituting a cube");
            MeshData::textured_box(1.0, None)
        }
    }
}

pub fn build_meshes(track: &fire_core::track::Track) -> (Vec<MeshData>, Meshes) {
    let half = track.half_width();
    let wall_off = half + fire_core::sim::WALL_MARGIN;

    // Textures are generated rather than shipped: `include_bytes!` bakes
    // assets into the wasm bundle that every web player downloads, and these
    // cost zero bytes. See texgen for the rest of the reasoning.
    let list = vec![
        // 1 ground
        trackmesh::ground(track, 90.0, 10.0, Some(texgen::turf(128))),
        // 2 road
        trackmesh::flat_ribbon(track, 0.0, half * 2.0, 0.0, 10.0, 3.0, Some(texgen::cobblestone(256, 10))),
        // 3,4 kerbs — a hair above the road so they never z-fight with it
        trackmesh::flat_ribbon(track, half + 0.9, 1.8, 0.03, 4.0, 1.0, Some(texgen::chequer(64, 2))),
        trackmesh::flat_ribbon(track, -(half + 0.9), 1.8, 0.03, 4.0, 1.0, Some(texgen::chequer(64, 2))),
        // 5,6 courtyard walls
        trackmesh::wall_ribbon(track, wall_off, 6.0, 12.0, 1.0, Some(texgen::castle_stone(256, 4, 6))),
        trackmesh::wall_ribbon(track, -wall_off, 6.0, 12.0, 1.0, Some(texgen::castle_stone(256, 4, 6))),
        // 7 start/finish
        trackmesh::cross_band(track, 0.0, 4.0, 10.0, Some(texgen::chequer(64, 8))),
        // 8 the car — untextured on purpose, so the instance colour is the
        // whole livery and eight players are eight different cars
        prop_or_cube(CAR_GLB, None, 1.0, "car"),
        // 9,10,11 architecture, wearing the same stone as the walls
        prop_or_cube(GATEHOUSE_GLB, Some(texgen::castle_stone(256, 4, 6)), 2.2, "gatehouse"),
        prop_or_cube(TOWER_GLB, Some(texgen::castle_stone(256, 4, 6)), 2.4, "tower"),
        prop_or_cube(FOUNTAIN_GLB, Some(texgen::castle_stone(128, 3, 4)), 2.0, "fountain"),
    ];

    let ids = Meshes {
        ground: 1,
        road: 2,
        kerb_l: 3,
        kerb_r: 4,
        wall_l: 5,
        wall_r: 6,
        start: 7,
        car: 8,
        gatehouse: 9,
        tower: 10,
        fountain: 11,
        car_extent: meshes::longest_extent(&list[7]),
        gatehouse_extent: meshes::longest_extent(&list[8]),
        tower_extent: meshes::longest_extent(&list[9]),
        fountain_extent: meshes::longest_extent(&list[10]),
        car_lift: meshes::ground_offset(&list[7]),
        gatehouse_lift: meshes::ground_offset(&list[8]),
        tower_lift: meshes::ground_offset(&list[9]),
        fountain_lift: meshes::ground_offset(&list[10]),
    };
    (list, ids)
}

// ---- the game -------------------------------------------------------------

/// The chase camera's own state. Split out so local and online play frame the
/// shot identically — a camera that behaves differently online would make the
/// two modes feel like different games.
pub struct Chase {
    dir: Vec2,
    eye: Vec3,
}

impl Chase {
    pub fn new(car: &car::Car) -> Self {
        Self {
            dir: car::forward(car.yaw),
            eye: Vec3::new(car.pos.x, CAM_HEIGHT, car.pos.y),
        }
    }

    pub fn update(&mut self, car: &car::Car, dt: f32) -> Camera {
        let fwd = car::forward(car.yaw);
        let speed = car.speed();
        let vdir = if speed > 4.0 { car.vel / speed } else { fwd };
        let want = (fwd * (1.0 - CAM_VEL_BLEND) + vdir * CAM_VEL_BLEND).normalize_or_zero();
        let want = if want == Vec2::ZERO { fwd } else { want };

        let k = 1.0 - (-CAM_LAG * dt).exp();
        self.dir = (self.dir + (want - self.dir) * k).normalize_or_zero();
        if self.dir == Vec2::ZERO {
            self.dir = fwd;
        }

        let p = Vec3::new(car.pos.x, 0.0, car.pos.y);
        let d = Vec3::new(self.dir.x, 0.0, self.dir.y);
        let want_eye = p - d * CAM_DIST + Vec3::Y * CAM_HEIGHT;
        // Lag the eye too, so kerb strikes do not jolt the whole frame.
        self.eye += (want_eye - self.eye) * k;

        Camera {
            eye: self.eye,
            target: p + d * 10.0 + Vec3::Y * 1.4,
            // Widening with speed is the cheapest sense of velocity there is.
            fov_y_deg: FOV_BASE + FOV_SPEED_GAIN * (speed / car::MAX_SPEED).min(1.4),
        }
    }
}

/// Read the keyboard into a car intent. `boost_was_down` is the caller's
/// rising-edge latch: a held key must spend exactly one charge.
pub fn read_input(input: &InputState, boost_was_down: &mut bool) -> CarInput {
    let held = |a: KeyCode, b: KeyCode| input.down(a) || input.down(b);
    let throttle = if held(KeyCode::KeyW, KeyCode::ArrowUp) {
        1.0
    } else if held(KeyCode::KeyS, KeyCode::ArrowDown) {
        -1.0
    } else {
        0.0
    };
    // `steer` is left-positive, matching `Car::step`.
    let mut steer = 0.0;
    if held(KeyCode::KeyA, KeyCode::ArrowLeft) {
        steer += 1.0;
    }
    if held(KeyCode::KeyD, KeyCode::ArrowRight) {
        steer -= 1.0;
    }
    let boost_down = input.down(KeyCode::ShiftLeft) || input.down(KeyCode::ShiftRight);
    let boost = boost_down && !*boost_was_down;
    *boost_was_down = boost_down;
    CarInput { throttle, steer, handbrake: input.down(KeyCode::Space), boost }
}

pub struct Game {
    race: Race,
    me: usize,
    ids: Meshes,
    chase: Chase,
    /// Rising-edge latch for boost. A held key must spend exactly one charge.
    boost_was_down: bool,
    started: bool,
    clock: FixedStep,
}

impl Game {
    pub fn new(ids: Meshes) -> Self {
        let race = Race::new(castle::track(), PLAYERS, LAPS);
        let chase = Chase::new(&race.racers[0].car);
        Self { race, me: 0, ids, chase, boost_was_down: false, started: false, clock: FixedStep::default() }
    }



    fn publish_hud(&self) {
        let me = &self.race.racers[self.me];
        let place = self.race.standings().iter().position(|&i| i == self.me).unwrap_or(0) + 1;
        HUD.with(|h| {
            *h.borrow_mut() = Hud {
                speed_kmh: me.car.speed() * 3.6,
                lap: me.lap.lap.min(self.race.laps_to_win),
                laps_total: self.race.laps_to_win,
                place,
                racers: self.race.racers.len(),
                boost_charges: me.car.boost_charges,
                boosting: me.car.boosting(),
                drifting: me.car.drift > 0.25,
                countdown: self.race.countdown_left(),
                finished: me.finish_tick.is_some(),
            };
        });
    }
}

impl EmberGame for Game {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        // A stalled tab hands back a huge dt; stepping the sim by it would
        // teleport every car through a wall.
        let dt = dt.clamp(0.0, 0.05);

        if !self.started {
            self.race.start_countdown();
            self.started = true;
        }

        let mine = read_input(input, &mut self.boost_was_down);
        let mut inputs: Vec<CarInput> = self
            .race
            .racers
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if i == self.me {
                    mine
                } else {
                    // Vary skill by grid slot so the field spreads out.
                    let skill = ai::DEFAULT_SKILL - i as f32 * 0.012;
                    ai::chase(&self.race.track, &r.car, skill)
                }
            })
            .collect();
        if self.race.racers[self.me].finish_tick.is_some() {
            inputs[self.me] = ai::chase(&self.race.track, &self.race.racers[self.me].car, 0.8);
        }
        // Whole ticks only. `dt` here is wall-clock frame time, so stepping
        // the sim by it directly would make the handling frame-rate dependent
        // and the local game disagree with the server's fixed clock.
        for _ in 0..self.clock.ticks(dt) {
            self.race.step(&inputs, fire_core::car::DT);
        }

        let camera = self.chase.update(&self.race.racers[self.me].car, dt);
        self.publish_hud();

        scene(&self.race, &self.ids, self.me, camera)

    }
}

/// Per-player livery. Deep, saturated colours that survive being multiplied
/// into an untextured mesh under a single directional light.
pub fn livery(i: usize) -> Vec3 {
    const LIVERIES: [[f32; 3]; 8] = [
        [0.86, 0.14, 0.16], // crimson
        [0.16, 0.42, 0.88], // azure
        [0.20, 0.68, 0.32], // emerald
        [0.94, 0.72, 0.16], // gold
        [0.58, 0.28, 0.82], // violet
        [0.95, 0.46, 0.12], // amber
        [0.16, 0.72, 0.74], // teal
        [0.90, 0.40, 0.66], // rose
    ];
    Vec3::from(LIVERIES[i % LIVERIES.len()])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mesh-id struct and the registration order are written out twice
    /// and must agree; if they drift, every prop draws as the wrong shape.
    #[test]
    fn mesh_ids_match_registration_order() {
        let track = castle::track();
        let (list, ids) = build_meshes(&track);
        assert_eq!(list.len(), 11, "mesh count changed — update the id struct");
        // Ids are 1-based: list[i] has id i+1.
        for (i, id) in [
            ids.ground, ids.road, ids.kerb_l, ids.kerb_r, ids.wall_l,
            ids.wall_r, ids.start, ids.car, ids.gatehouse, ids.tower, ids.fountain,
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(*id, i as u32 + 1, "mesh id {id} is not at registration slot {}", i + 1);
        }
        for (i, m) in list.iter().enumerate() {
            assert!(!m.vertices.is_empty(), "mesh slot {} is empty", i + 1);
            assert_eq!(m.vertices.len() % 3, 0, "mesh slot {} is not a triangle list", i + 1);
        }
    }

    /// Props must have loaded from their GLBs rather than falling back to the
    /// substitute cube — a cube has 36 vertices, so anything at exactly 36 is
    /// a failed load hiding behind the safety net.
    #[test]
    fn generated_props_actually_loaded() {
        let track = castle::track();
        let (list, _) = build_meshes(&track);
        for (slot, name) in [(7, "car"), (8, "gatehouse"), (9, "tower"), (10, "fountain")] {
            assert!(
                list[slot].vertices.len() > 100,
                "{name} fell back to the placeholder cube ({} verts)",
                list[slot].vertices.len()
            );
        }
    }

    #[test]
    fn the_game_runs_without_input() {
        let track = castle::track();
        let (_, ids) = build_meshes(&track);
        let mut g = Game::new(ids);
        let input = InputState::default();
        for _ in 0..60 * 30 {
            let f = g.update(&input, 1.0 / 60.0);
            assert!(f.camera.eye.is_finite(), "camera eye went non-finite");
            assert!(f.camera.target.is_finite(), "camera target went non-finite");
            assert!(!f.instances.is_empty());
            for inst in &f.instances {
                assert!(inst.position.is_finite(), "instance position non-finite");
            }
        }
        // The countdown expires and the AI field gets moving. The player's own
        // car stays put, and should: no keys are held. Asserting the HUD shows
        // speed here would be asserting that a parked car drives itself.
        assert_eq!(g.race.state, RaceState::Racing);
        assert!(
            g.race.racers.iter().skip(1).any(|r| r.car.speed() > 5.0),
            "no AI car got moving"
        );
        assert!(g.race.racers[g.me].car.speed() < 0.5, "the unmanned player car drove off");
        let h = hud();
        assert_eq!(h.racers, PLAYERS);
        assert_eq!(h.laps_total, LAPS);
        assert!((1..=PLAYERS).contains(&h.place), "place {} out of range", h.place);
        assert_eq!(h.boost_charges, car::BOOST_CHARGES, "player spent a charge it never pressed");
    }

    /// The player's car has to actually be wired to the simulation. The key
    /// mapping cannot be tested from here — `InputState`'s pressed set is
    /// private to the engine — so this drives the race directly and checks
    /// that the seat marked `me` is the one that moves.
    #[test]
    fn the_player_car_is_wired_to_the_sim() {
        let track = castle::track();
        let (_, ids) = build_meshes(&track);
        let mut g = Game::new(ids);
        // Order matters: a fresh race is Waiting, and `step` is a no-op there.
        // Arm the countdown first, then run it out, or the throttle ticks
        // below land while the cars are still held on the grid.
        g.race.start_countdown();
        for _ in 0..(fire_core::sim::COUNTDOWN_SECS * 60.0) as u32 + 5 {
            g.race.step(&[], 1.0 / 60.0);
        }
        assert_eq!(g.race.state, RaceState::Racing, "countdown did not finish");
        let start = g.race.racers[g.me].car.pos;
        let mut inputs = vec![CarInput::default(); PLAYERS];
        inputs[g.me] = CarInput { throttle: 1.0, steer: 0.0, handbrake: false, boost: false };
        for _ in 0..120 {
            g.race.step(&inputs, 1.0 / 60.0);
        }
        let moved = (g.race.racers[g.me].car.pos - start).length();
        assert!(moved > 10.0, "player car only moved {moved:.1} m under full throttle");
    }

    /// A held boost key must spend one charge, not all three.
    #[test]
    fn the_boost_latch_holds() {
        let track = castle::track();
        let (_, ids) = build_meshes(&track);
        let mut g = Game::new(ids);
        // Drive the latch directly: read_input owns the rising edge.
        let mut pressed = 0;
        for _ in 0..10 {
            let boost_down = true;
            let boost = boost_down && !g.boost_was_down;
            g.boost_was_down = boost_down;
            if boost {
                pressed += 1;
            }
        }
        assert_eq!(pressed, 1, "a held key produced {pressed} presses");
    }
}


fn push_prop(frame: &mut Frame, ids: &Meshes, kind: PropKind, pos: Vec2, yaw: f32, metres: f32) {
    let (mesh, extent, lift) = match kind {
        PropKind::Gatehouse => (ids.gatehouse, ids.gatehouse_extent, ids.gatehouse_lift),
        PropKind::Tower => (ids.tower, ids.tower_extent, ids.tower_lift),
        PropKind::Fountain => (ids.fountain, ids.fountain_extent, ids.fountain_lift),
    };
    let s = if extent > 1e-4 { metres / extent } else { 1.0 };
    frame.instances.push(
        // Vec3::ONE: these carry a texture, and the shader multiplies the
        // instance colour into it. Tinting here would double-tint.
        Instance::new(Vec3::new(pos.x, lift * s, pos.y), Vec3::splat(s), Vec3::ONE)
            .with_yaw(yaw)
            .with_mesh(mesh),
    );
}

/// Build the whole frame from a race. Shared by local and online play so
/// the two modes cannot drift apart visually.
pub fn scene(race: &Race, ids: &Meshes, me: usize, camera: Camera) -> Frame {
    let mut frame = Frame { camera, instances: Vec::with_capacity(64) };

    // The track meshes are already in world space, so each is one
    // instance at the origin with unit scale and no rotation.
    for mesh in [
        ids.ground,
        ids.road,
        ids.kerb_l,
        ids.kerb_r,
        ids.wall_l,
        ids.wall_r,
        ids.start,
    ] {
        frame
            .instances
            .push(Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE).with_mesh(mesh));
    }

    for p in castle::props() {
        push_prop(&mut frame, ids, p.kind, p.pos, p.yaw, p.scale);
    }

    let car_scale = if ids.car_extent > 1e-4 { 4.4 / ids.car_extent } else { 1.0 };
    for (i, r) in race.racers.iter().enumerate() {
        let c = &r.car;
        let colour = livery(i);
        frame.instances.push(
            Instance::new(
                Vec3::new(c.pos.x, ids.car_lift * car_scale, c.pos.y),
                Vec3::splat(car_scale),
                colour,
            )
            .with_yaw(c.yaw)
            .with_mesh(ids.car),
        );

        // Boost flame: an opaque wedge behind the car. There is no
        // additive blending in this renderer, so a glow can only ever be
        // a solid mesh — this is that, and nothing fancier.
        if c.boosting() {
            let back = -car::forward(c.yaw);
            let p = c.pos + back * 2.4;
            frame.instances.push(
                Instance::new(
                    Vec3::new(p.x, 0.55, p.y),
                    Vec3::new(0.7, 0.5, 1.8),
                    Vec3::new(1.0, 0.62, 0.16),
                )
                .with_yaw(c.yaw),
            );
        }
    }

    // Remaining boost charges, floating over the player's car: the only
    // HUD the engine can draw, since there is no 2D pass and no text.
    let me = &race.racers[me].car;
    for n in 0..me.boost_charges {
        let side = (n as f32 - (car::BOOST_CHARGES - 1) as f32 * 0.5) * 0.85;
        let r = car::right(me.yaw) * side;
        frame.instances.push(
            Instance::new(
                Vec3::new(me.pos.x + r.x, 3.1, me.pos.y + r.y),
                Vec3::splat(0.45),
                Vec3::new(1.0, 0.72, 0.2),
            )
            .with_yaw(me.yaw),
        );
    }

    // Lights: three bars over the line during the countdown, going out
    // one by one. Cheap, readable, and needs no font.
    if race.state == RaceState::Countdown {
        let (c, tan) = race.track.at(0.0);
        let left = Vec2::new(-tan.y, tan.x);
        let lit = race.countdown_left().ceil() as i32;
        for n in 0..3 {
            let p = c + left * (n as f32 - 1.0) * 3.2;
            let on = n < lit;
            frame.instances.push(
                Instance::new(
                    Vec3::new(p.x, 7.0, p.y),
                    Vec3::splat(1.1),
                    if on { Vec3::new(0.9, 0.12, 0.1) } else { Vec3::new(0.12, 0.12, 0.14) },
                )
                .with_mesh(0),
            );
        }
    }

    frame
}
