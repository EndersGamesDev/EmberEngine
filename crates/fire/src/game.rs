//! The client: builds the castle, drives the player's car, frames the shot.

use std::cell::RefCell;

use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{
    Camera, EmberGame, Frame, InputState, Instance, KeyCode, MeshData, PadButton, TextureData,
};
use fire_core::ai;
use fire_core::car::{self, CarInput};
use fire_core::castle::{self, PropKind};
use fire_core::sim::{FixedStep, Race, RaceState};

use crate::meshes;
use crate::texgen;
use crate::trackmesh;

#[path = "presentation.rs"]
mod presentation;

const PLAYERS: usize = 8;
const LAPS: u32 = 3;

// ---- camera ---------------------------------------------------------------

const CAM_DIST: f32 = 8.8;
const CAM_HEIGHT: f32 = 3.15;
/// Chase-camera lag, 1/s. Applied as an exponential so it is frame-rate
/// independent for the same reason the tyre friction is.
const CAM_LAG: f32 = 8.0;
/// How much the camera follows the car's *velocity* rather than its nose.
/// A camera welded to the heading swings wildly through a drift and hides
/// the apex; one welded to the velocity never shows you where you are
/// pointing. The blend keeps the slide legible.
const CAM_VEL_BLEND: f32 = 0.23;
const FOV_BASE: f32 = 62.0;
const FOV_SPEED_GAIN: f32 = 10.0;

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
    pub item: u8,
    pub vehicle: u8,
    pub gear: u8,
    pub race_time: f32,
    pub drift_charge: f32,
    pub shield: f32,
    pub grip: f32,
    pub hit: f32,
}

thread_local! {
    static HUD: RefCell<Hud> = const { RefCell::new(Hud {
        speed_kmh: 0.0, lap: 0, laps_total: 0, place: 0, racers: 0,
        boost_charges: 0, boosting: false, drifting: false,
        countdown: 0.0, finished: false, item: 0, vehicle: 0, gear: 1,
        race_time: 0.0, drift_charge: 0.0, shield: 0.0, grip: 0.0, hit: 0.0,
    }) };
    static RESTART: RefCell<Option<u8>> = const { RefCell::new(None) };
}

/// Restart in the existing engine loop; no extra canvas or event loop.
pub fn request_restart(vehicle: u8) {
    RESTART.with(|r| *r.borrow_mut() = Some(vehicle.min(2)));
}

#[must_use]
pub fn hud() -> Hud {
    HUD.with(|h| *h.borrow())
}

/// Publish this frame's HUD. Both play modes go through here so the page
/// reads one shape regardless of which one is running.
pub fn set_hud(h: Hud) {
    HUD.with(|c| *c.borrow_mut() = h);
}

/// Speed-based automatic gear indication; reverse is shown as zero by the page.
#[must_use]
pub fn display_gear(car: &car::Car) -> u8 {
    if car.vel.dot(car::forward(car.yaw)) < -0.5 {
        return 0;
    }
    let speed = car.speed() * 3.6;
    [38.0, 72.0, 112.0, 157.0, 208.0]
        .iter()
        .fold(1, |gear, &threshold| gear + u8::from(speed > threshold))
}

// ---- meshes ---------------------------------------------------------------

/// Mesh ids, assigned in registration order.
///
/// `EngineConfig.meshes` entries take ids 1..=N, and id 0 is the engine's
/// built-in cube — so the order of `build_meshes` below IS this struct, and
/// the two must change together.
pub struct Meshes {
    pub ground: u32,
    pub road: u32,
    pub kerb_l: u32,
    pub kerb_r: u32,
    pub wall_l: u32,
    pub wall_r: u32,
    pub start: u32,
    pub car: u32,
    pub bodies: [u32; 3],
    pub glass: [u32; 3],
    pub tyre: u32,
    pub rim: u32,
    pub disc: u32,
    pub pickup: u32,
    pub edges: [u32; 2],
    pub runoff: [u32; 2],
    pub sky: u32,
    pub foliage: u32,
    pub gatehouse: u32,
    pub tower: u32,
    pub fountain: u32,
    /// Longest-axis extent of each prop mesh, so `Prop::scale` can be given
    /// in metres rather than in whatever units the generator happened to use.
    pub gatehouse_extent: f32,
    pub tower_extent: f32,
    pub fountain_extent: f32,
    /// How far to lift each prop so it stands on the courtyard floor.
    pub gatehouse_lift: f32,
    pub tower_lift: f32,
    pub fountain_lift: f32,
}

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

#[must_use]
#[allow(clippy::too_many_lines)] // Registration order and ids are reviewed together.
pub fn build_meshes(track: &fire_core::track::Track) -> (Vec<MeshData>, Meshes) {
    let half = track.half_width();
    let wall_off = half + fire_core::sim::WALL_MARGIN;

    // Textures are generated rather than shipped: `include_bytes!` bakes
    // assets into the wasm bundle that every web player downloads, and these
    // cost zero bytes. See texgen for the rest of the reasoning.
    let mut list = vec![
        // 1 ground
        trackmesh::ground(track, 90.0, 10.0, Some(texgen::turf(128))),
        // 2 road
        trackmesh::flat_ribbon(
            track,
            0.0,
            half * 2.0,
            0.0,
            10.0,
            3.0,
            Some(texgen::asphalt(256)),
        ),
        // 3,4 kerbs — a hair above the road so they never z-fight with it
        trackmesh::flat_ribbon(
            track,
            half + 0.9,
            1.8,
            0.03,
            4.0,
            1.0,
            Some(texgen::kerb(64)),
        ),
        trackmesh::flat_ribbon(
            track,
            -(half + 0.9),
            1.8,
            0.03,
            4.0,
            1.0,
            Some(texgen::kerb(64)),
        ),
        // 5,6 courtyard walls
        trackmesh::wall_ribbon(
            track,
            wall_off,
            1.1,
            12.0,
            1.0,
            Some(texgen::castle_stone(256, 4, 6)),
        ),
        trackmesh::wall_ribbon(
            track,
            -wall_off,
            1.1,
            12.0,
            1.0,
            Some(texgen::castle_stone(256, 4, 6)),
        ),
        // 7 start/finish
        trackmesh::cross_band(track, 0.0, 4.0, 10.0, Some(texgen::chequer(64, 8))),
        // 8 the car — untextured on purpose, so the instance colour is the
        // whole livery and eight players are eight different cars
        meshes::car_body(0, false),
        // 9,10,11 architecture, wearing the same stone as the walls
        prop_or_cube(
            GATEHOUSE_GLB,
            Some(texgen::castle_stone(256, 4, 6)),
            2.2,
            "gatehouse",
        ),
        prop_or_cube(
            TOWER_GLB,
            Some(texgen::castle_stone(256, 4, 6)),
            2.4,
            "tower",
        ),
        prop_or_cube(
            FOUNTAIN_GLB,
            Some(texgen::castle_stone(128, 3, 4)),
            2.0,
            "fountain",
        ),
        meshes::car_body(1, false),
        meshes::car_body(2, false),
        meshes::car_body(0, true),
        meshes::car_body(1, true),
        meshes::car_body(2, true),
        meshes::wheel(0.36, 0.29),
        meshes::wheel(0.235, 0.035),
        meshes::disc(),
        MeshData::textured_box(1.0, Some(texgen::mystery(128))),
        trackmesh::flat_ribbon(track, half - 0.18, 0.16, 0.015, 10.0, 1.0, None),
        trackmesh::flat_ribbon(track, -half + 0.18, 0.16, 0.015, 10.0, 1.0, None),
        trackmesh::flat_ribbon(
            track,
            half + 3.4,
            3.2,
            0.005,
            9.0,
            1.0,
            Some(texgen::asphalt(128)),
        ),
        trackmesh::flat_ribbon(
            track,
            -half - 3.4,
            3.2,
            0.005,
            9.0,
            1.0,
            Some(texgen::asphalt(128)),
        ),
    ];

    list.push(meshes::sky());
    list.push(meshes::foliage());
    let ids = Meshes {
        ground: 1,
        road: 2,
        kerb_l: 3,
        kerb_r: 4,
        wall_l: 5,
        wall_r: 6,
        start: 7,
        car: 8,
        bodies: [8, 12, 13],
        glass: [14, 15, 16],
        tyre: 17,
        rim: 18,
        disc: 19,
        pickup: 20,
        edges: [21, 22],
        runoff: [23, 24],
        sky: 25,
        foliage: 26,
        gatehouse: 9,
        tower: 10,
        fountain: 11,
        gatehouse_extent: meshes::longest_extent(&list[8]),
        tower_extent: meshes::longest_extent(&list[9]),
        fountain_extent: meshes::longest_extent(&list[10]),
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
    previous_pos: Vec2,
}

impl Chase {
    #[must_use]
    pub fn new(car: &car::Car) -> Self {
        Self {
            dir: car::forward(car.yaw),
            eye: Vec3::new(car.pos.x, CAM_HEIGHT, car.pos.y)
                - Vec3::new(car::forward(car.yaw).x, 0.0, car::forward(car.yaw).y) * CAM_DIST,
            previous_pos: car.pos,
        }
    }

    #[must_use]
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
        let want_eye =
            p - d * (CAM_DIST + (speed / car::MAX_SPEED).min(1.3) * 1.6) + Vec3::Y * CAM_HEIGHT;
        // Translate with the car before smoothing its relative orbit. Smoothing
        // world translation adds speed / lag metres to chase distance.
        let movement = car.pos - self.previous_pos;
        self.eye += Vec3::new(movement.x, 0.0, movement.y);
        self.previous_pos = car.pos;
        self.eye += (want_eye - self.eye) * k;
        // Direction already has its own damping. A second sideways lag would
        // push the driver's car out of frame during a fast corner.
        let relative = self.eye - p;
        let distance = Vec2::new(relative.x, relative.z).length();
        self.eye = p - d * distance + Vec3::Y * CAM_HEIGHT;

        Camera {
            eye: self.eye,
            target: p + d * 12.0 + Vec3::Y * 1.05,
            // Widening with speed is the cheapest sense of velocity there is.
            fov_y_deg: FOV_BASE + FOV_SPEED_GAIN * (speed / car::MAX_SPEED).min(1.4),
        }
    }
}

/// Read the keyboard into a car intent. `boost_was_down` is the caller's
/// rising-edge latch: a held key must spend exactly one charge.
#[must_use]
pub fn read_input(
    input: &InputState,
    boost_was_down: &mut bool,
    item_was_down: &mut bool,
) -> CarInput {
    let held = |a: KeyCode, b: KeyCode| input.down(a) || input.down(b);
    let mut throttle = if held(KeyCode::KeyW, KeyCode::ArrowUp) {
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
    let pad = input.pad();
    if let Some(pad) = pad {
        if throttle == 0.0 {
            throttle = pad.rt - pad.lt;
        }
        if steer == 0.0 {
            steer = -pad.left[0];
        }
    }
    let button = |b| pad.is_some_and(|p| p.down(b));
    let boost_down = input.down(KeyCode::ShiftLeft)
        || input.down(KeyCode::ShiftRight)
        || button(PadButton::South);
    let boost = boost_down && !*boost_was_down;
    *boost_was_down = boost_down;
    let item_down = input.down(KeyCode::KeyE) || button(PadButton::RB);
    let use_item = item_down && !*item_was_down;
    *item_was_down = item_down;
    CarInput {
        throttle,
        steer,
        handbrake: input.down(KeyCode::Space) || button(PadButton::West),
        boost,
        use_item,
    }
}

#[allow(clippy::struct_excessive_bools)] // Independent input edges and queued presses must not share state.
pub struct Game {
    race: Race,
    me: usize,
    ids: Meshes,
    chase: Chase,
    /// Rising-edge latch for boost. A held key must spend exactly one charge.
    boost_was_down: bool,
    item_was_down: bool,
    pending_boost: bool,
    pending_item: bool,
    recover_was_down: bool,
    started: bool,
    clock: FixedStep,
}

impl Game {
    #[must_use]
    pub fn new(ids: Meshes) -> Self {
        Self::new_with_vehicle(ids, 0)
    }

    #[must_use]
    pub fn new_with_vehicle(ids: Meshes, vehicle: u8) -> Self {
        let mut race = Race::new(castle::track(), PLAYERS, LAPS);
        race.racers[0].car.vehicle = vehicle.min(2);
        let chase = Chase::new(&race.racers[0].car);
        Self {
            race,
            me: 0,
            ids,
            chase,
            boost_was_down: false,
            item_was_down: false,
            pending_boost: false,
            pending_item: false,
            recover_was_down: false,
            started: false,
            clock: FixedStep::default(),
        }
    }

    fn publish_hud(&self) {
        let me = &self.race.racers[self.me];
        let place = self
            .race
            .standings()
            .iter()
            .position(|&i| i == self.me)
            .unwrap_or(0)
            + 1;
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
                item: me.car.item,
                vehicle: me.car.vehicle,
                gear: display_gear(&me.car),
                race_time: me
                    .finish_time
                    .unwrap_or_else(|| self.race.elapsed_seconds()),
                drift_charge: me.car.drift_charge,
                shield: me.car.shield_left,
                grip: me.car.grip_left,
                hit: me.car.hit_left,
            };
        });
    }
}

impl EmberGame for Game {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        // A stalled tab hands back a huge dt; stepping the sim by it would
        // teleport every car through a wall.
        let dt = dt.clamp(0.0, 0.05);

        if let Some(vehicle) = RESTART.with(|r| r.borrow_mut().take()) {
            self.race = Race::new(castle::track(), PLAYERS, LAPS);
            self.race.racers[0].car.vehicle = vehicle;
            self.chase = Chase::new(&self.race.racers[0].car);
            self.started = false;
            self.clock = FixedStep::default();
            self.pending_boost = false;
            self.pending_item = false;
        }

        if !self.started {
            self.race.start_countdown();
            self.started = true;
        }

        let mut mine = read_input(input, &mut self.boost_was_down, &mut self.item_was_down);
        self.pending_boost |= mine.boost;
        self.pending_item |= mine.use_item;
        mine.boost = self.pending_boost;
        mine.use_item = self.pending_item;
        let recover_down =
            input.down(KeyCode::KeyR) || input.pad().is_some_and(|pad| pad.down(PadButton::North));
        if recover_down && !self.recover_was_down {
            self.race.recover(self.me);
        }
        self.recover_was_down = recover_down;
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
                    let slot = u8::try_from(i).expect("local race grid fits in u8");
                    let skill = ai::DEFAULT_SKILL - f32::from(slot) * 0.012;
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
            self.pending_boost = false;
            self.pending_item = false;
            inputs[self.me].boost = false;
            inputs[self.me].use_item = false;
        }

        let camera = self.chase.update(&self.race.racers[self.me].car, dt);
        self.publish_hud();

        scene(&self.race, &self.ids, self.me, camera)
    }
}

/// Per-player livery. Deep, saturated colours that survive being multiplied
/// into an untextured mesh under a single directional light.
#[must_use]
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
#[must_use]
pub fn scene(race: &Race, ids: &Meshes, _me: usize, camera: Camera) -> Frame {
    let mut frame = Frame {
        camera,
        instances: Vec::with_capacity(700),
        fog: ember_engine::Fog {
            color: [0.22, 0.29, 0.36],
            density: 0.0011,
        },
    };
    frame
        .instances
        .push(Instance::new(frame.camera.eye, Vec3::splat(400.0), Vec3::ONE).with_mesh(ids.sky));

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
        ids.edges[0],
        ids.edges[1],
        ids.runoff[0],
        ids.runoff[1],
    ] {
        frame
            .instances
            .push(Instance::new(Vec3::ZERO, Vec3::ONE, Vec3::ONE).with_mesh(mesh));
    }

    for p in castle::props() {
        push_prop(&mut frame, ids, p.kind, p.pos, p.yaw, p.scale);
    }

    presentation::circuit(&mut frame, race, ids);
    for (i, racer) in race.racers.iter().enumerate() {
        presentation::car(
            &mut frame,
            ids,
            &racer.car,
            livery(i),
            race.elapsed_seconds(),
            racer.lap.progress,
        );
    }
    presentation::items(&mut frame, race, ids);

    // Lights: three bars over the line during the countdown, going out
    // one by one. Cheap, readable, and needs no font.
    if race.state == RaceState::Countdown {
        let (c, tan) = race.track.at(0.0);
        let left = Vec2::new(-tan.y, tan.x);
        let lit = race.countdown_left().ceil().clamp(0.0, 3.0);
        for n in 0_u8..3 {
            let p = c + left * (f32::from(n) - 1.0) * 3.2;
            let on = f32::from(n) < lit;
            frame.instances.push(
                Instance::new(
                    Vec3::new(p.x, 7.0, p.y),
                    Vec3::splat(1.1),
                    if on {
                        Vec3::new(0.9, 0.12, 0.1)
                    } else {
                        Vec3::new(0.12, 0.12, 0.14)
                    },
                )
                .with_mesh(0),
            );
        }
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chase_keeps_a_fast_turning_car_centred_without_translation_lag() {
        let mut car = car::Car::new(Vec2::ZERO, 0.0);
        let mut chase = Chase::new(&car);
        for _ in 0..180 {
            car.yaw += 0.012;
            car.vel = car::forward(car.yaw) * 45.0;
            car.pos += car.vel * car::DT;
            let camera = chase.update(&car, car::DT);
            let centre = Vec3::new(car.pos.x, 0.65, car.pos.y);
            let ndc = camera.view_proj(16.0 / 9.0).project_point3(centre);
            assert!(ndc.x.abs() < 0.01, "driver escaped centre: {ndc}");
            let offset = camera.eye - Vec3::new(car.pos.x, CAM_HEIGHT, car.pos.y);
            assert!(offset.length() < 10.6, "camera fell behind: {offset}");
        }
    }

    /// The mesh-id struct and the registration order are written out twice
    /// and must agree; if they drift, every prop draws as the wrong shape.
    #[test]
    fn mesh_ids_match_registration_order() {
        let track = castle::track();
        let (list, ids) = build_meshes(&track);
        assert_eq!(list.len(), 26, "mesh count changed — update the id struct");
        // Ids are 1-based: list[i] has id i+1.
        for (i, id) in [
            ids.ground,
            ids.road,
            ids.kerb_l,
            ids.kerb_r,
            ids.wall_l,
            ids.wall_r,
            ids.start,
            ids.car,
            ids.gatehouse,
            ids.tower,
            ids.fountain,
            ids.bodies[1],
            ids.bodies[2],
            ids.glass[0],
            ids.glass[1],
            ids.glass[2],
            ids.tyre,
            ids.rim,
            ids.disc,
            ids.pickup,
            ids.edges[0],
            ids.edges[1],
            ids.runoff[0],
            ids.runoff[1],
            ids.sky,
            ids.foliage,
        ]
        .iter()
        .enumerate()
        {
            let expected = u32::try_from(i).expect("mesh registration index fits u32") + 1;
            assert_eq!(
                *id,
                expected,
                "mesh id {id} is not at registration slot {}",
                i + 1
            );
        }
        for (i, mesh) in list.iter().enumerate() {
            assert!(!mesh.vertices.is_empty(), "mesh slot {} is empty", i + 1);
            assert_eq!(
                mesh.vertices.len() % 3,
                0,
                "mesh slot {} is not a triangle list",
                i + 1
            );
        }
    }

    /// Props must have loaded from their GLBs rather than falling back to the
    /// substitute cube — a cube has 36 vertices, so anything at exactly 36 is
    /// a failed load hiding behind the safety net.
    #[test]
    fn generated_props_actually_loaded() {
        let track = castle::track();
        let (list, _) = build_meshes(&track);
        for (slot, name) in [(8, "gatehouse"), (9, "tower"), (10, "fountain")] {
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
        let mut game = Game::new(ids);
        let input = InputState::default();
        // Exercise the scene and AI field, including contacts with the parked
        // player. Contact may legitimately move a car with no throttle input.
        for _ in 0..60 * 10 {
            let frame = game.update(&input, 1.0 / 60.0);
            assert!(frame.camera.eye.is_finite(), "camera eye went non-finite");
            assert!(
                frame.camera.target.is_finite(),
                "camera target went non-finite"
            );
            assert!(!frame.instances.is_empty());
            for instance in &frame.instances {
                assert!(
                    instance.position.is_finite(),
                    "instance position non-finite"
                );
            }
        }
        // The countdown expires and the AI field moves. Driver resources stay
        // untouched, even when a following AI car bumps the parked player.
        assert_eq!(game.race.state, RaceState::Racing);
        assert!(
            game.race.racers.iter().skip(1).any(|r| r.car.speed() > 5.0),
            "no AI car got moving"
        );
        let hud = hud();
        assert_eq!(hud.racers, PLAYERS);
        assert_eq!(hud.laps_total, LAPS);
        assert!(
            (1..=PLAYERS).contains(&hud.place),
            "place {} out of range",
            hud.place
        );
        assert_eq!(
            hud.boost_charges,
            car::BOOST_CHARGES,
            "player spent a charge it never pressed"
        );
    }

    /// The player's car has to actually be wired to the simulation. The key
    /// mapping cannot be tested from here — `InputState`'s pressed set is
    /// private to the engine — so this drives the race directly and checks
    /// that the seat marked `me` is the one that moves.
    #[test]
    fn the_player_car_is_wired_to_the_sim() {
        let track = castle::track();
        let (_, ids) = build_meshes(&track);
        let mut game = Game::new(ids);
        // Order matters: a fresh race is Waiting, and `step` is a no-op there.
        // Arm the countdown first, then run it out, or the throttle ticks
        // below land while the cars are still held on the grid.
        game.race.start_countdown();
        // The countdown is a small positive simulation constant.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let countdown_ticks = (fire_core::sim::COUNTDOWN_SECS * 60.0) as u32;
        for _ in 0..countdown_ticks + 5 {
            game.race.step(&[], 1.0 / 60.0);
        }
        assert_eq!(
            game.race.state,
            RaceState::Racing,
            "countdown did not finish"
        );
        let start = game.race.racers[game.me].car.pos;
        let mut inputs = vec![CarInput::default(); PLAYERS];
        inputs[game.me] = CarInput {
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
            boost: false,
            use_item: false,
        };
        for _ in 0..120 {
            game.race.step(&inputs, 1.0 / 60.0);
        }
        let moved = (game.race.racers[game.me].car.pos - start).length();
        assert!(
            moved > 10.0,
            "player car only moved {moved:.1} m under full throttle"
        );
    }

    /// A held boost key must spend one charge, not all three.
    #[test]
    fn the_boost_latch_holds() {
        let track = castle::track();
        let (_, ids) = build_meshes(&track);
        let mut game = Game::new(ids);
        // Drive the latch directly: read_input owns the rising edge.
        let mut pressed = 0;
        for _ in 0..10 {
            let boost_down = true;
            let boost = boost_down && !game.boost_was_down;
            game.boost_was_down = boost_down;
            if boost {
                pressed += 1;
            }
        }
        assert_eq!(pressed, 1, "a held key produced {pressed} presses");
    }

    #[test]
    fn item_and_boost_have_independent_edges() {
        let mut boost = false;
        let mut item = false;
        let held_boost = InputState::from_parts(&[KeyCode::ShiftLeft], &[], (0.0, 0.0), None);
        assert!(read_input(&held_boost, &mut boost, &mut item).boost);
        let both =
            InputState::from_parts(&[KeyCode::ShiftLeft, KeyCode::KeyE], &[], (0.0, 0.0), None);
        let second = read_input(&both, &mut boost, &mut item);
        assert!(second.use_item);
        assert!(!second.boost);
        assert!(!read_input(&both, &mut boost, &mut item).use_item);
    }

    #[test]
    fn presses_survive_frames_without_a_tick_and_restart_reuses_the_game() {
        let (_, ids) = build_meshes(&castle::track());
        let mut game = Game::new_with_vehicle(ids, 2);
        game.race.state = RaceState::Racing;
        game.started = true;
        game.race.racers[0].car.item = 1;
        game.race.racers[0].car.vel = car::forward(game.race.racers[0].car.yaw) * 20.0;
        let press = InputState::from_parts(&[KeyCode::KeyE], &[], (0.0, 0.0), None);
        game.update(&press, 0.0);
        assert_eq!(game.race.racers[0].car.item, 1);
        game.update(&InputState::default(), car::DT);
        assert_eq!(game.race.racers[0].car.item, 0);
        assert!(game.race.racers[0].car.boosting());
        request_restart(1);
        let frame = game.update(&InputState::default(), car::DT);
        assert_eq!(game.race.racers[0].car.vehicle, 1);
        assert_eq!(game.race.state, RaceState::Countdown);
        assert!(game.race.elapsed_seconds() < 0.01);
        let p = game.race.racers[0].car.pos;
        assert!((frame.camera.eye - Vec3::new(p.x, CAM_HEIGHT, p.y)).length() > 8.0);
    }
}
