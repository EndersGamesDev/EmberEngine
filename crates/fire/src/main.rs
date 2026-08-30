use ember_engine::{Camera, EmberGame, EngineConfig, Frame, InputState, KeyCode, Instance};
use glam::Vec3;
use tracing::info;

const CAR_COLOR: f32 = 0.95;
const TRACK_COLOR: f32 = 0.1;
const RACING_STRIPE_COLOR: f32 = 0.8;

const PLAYER_COUNT: usize = 8;

fn rotate_y(v: Vec3, angle: f32) -> Vec3 {
    let sin = angle.sin();
    let cos = angle.cos();
    Vec3::new(
        v.x * cos - v.z * sin,
        v.y,
        v.x * sin + v.z * cos,
    )
}

struct Car {
    pos: Vec3,
    angle: f32,
    speed: f32,
    max_speed: f32,
    turn_speed: f32,
    score: u32,
}

struct GameState {
    cars: [Car; PLAYER_COUNT],
    camera_target: usize,
    camera_angle: f32,
    lap_times: Vec<f32>,
}

impl EmberGame for GameState {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame {
        // Update car physics for each player
        for i in 0..PLAYER_COUNT {
            let car = &mut self.cars[i];
            
            // Acceleration
            if input.down(KeyCode::KeyW) || input.down(KeyCode::ArrowUp) {
                car.speed = car.speed.min(car.max_speed);
            }
            if input.down(KeyCode::KeyS) || input.down(KeyCode::ArrowDown) {
                car.speed = car.speed.max(-car.max_speed * 0.3);
            }
            
            // Turning
            if car.speed.abs() > 0.1 {
                if input.down(KeyCode::KeyA) || input.down(KeyCode::ArrowLeft) {
                    car.angle += car.turn_speed * (car.speed / car.max_speed) * dt;
                }
                if input.down(KeyCode::KeyD) || input.down(KeyCode::ArrowRight) {
                    car.angle -= car.turn_speed * (car.speed / car.max_speed) * dt;
                }
            }
            
            // Move car
            let forward = rotate_y(Vec3::new(0.0, 0.0, 1.0), car.angle);
            car.pos += forward * car.speed * dt;
            
            // Simple lap tracking
            if car.pos.length() < 10.0 {
                car.score += 1;
                info!("player {} completed a lap!", i);
            }
            
            // Track boundary (circular track)
            car.pos.x = car.pos.x.clamp(-30.0, 30.0);
            car.pos.z = car.pos.z.clamp(-30.0, 30.0);
        }
        
        // Update camera to follow the target car
        let target_car = &self.cars[self.camera_target];
        self.camera_angle = target_car.angle;
        
        let camera_pos = target_car.pos + Vec3::new(0.0, 15.0, 20.0);
        
        // Build scene
        let camera = Camera {
            eye: camera_pos,
            target: target_car.pos,
            fov_y_deg: 80.0,
        };
        
        let mut frame = Frame {
            camera,
            instances: Vec::with_capacity(128),
        };
        
        let inst = |frame: &mut Frame, pos: Vec3, scale: Vec3, color: Vec3| {
            frame.instances.push(Instance::new(pos, scale, color));
        };
        
        // Track surface
        let track_radius = 30.0;
        for ring in 0..5 {
            let radius = track_radius - ring as f32 * 8.0;
            inst(
                &mut frame,
                Vec3::new(0.0, -0.2, 0.0),
                Vec3::new(radius * 2.0, 0.2, radius * 2.0),
                Vec3::new(TRACK_COLOR, TRACK_COLOR, TRACK_COLOR),
            );
            
            // Racing stripes
            let stripe_count = 12;
            for i in 0..stripe_count {
                let theta = (i as f32 / stripe_count as f32) * 2.0 * std::f32::consts::PI;
                let stripe_x = radius * 1.01 * theta.cos();
                let stripe_z = radius * 1.01 * theta.sin();
                inst(
                    &mut frame,
                    Vec3::new(stripe_x, 0.01, stripe_z),
                    Vec3::new(0.1, 0.01, 0.8),
                    Vec3::new(RACING_STRIPE_COLOR, RACING_STRIPE_COLOR, RACING_STRIPE_COLOR),
                );
            }
        }
        
        // Cars
        for i in 0..PLAYER_COUNT {
            let car = &self.cars[i];
            
            // Car body
            inst(
                &mut frame,
                car.pos + Vec3::new(0.0, 0.3, 0.0),
                Vec3::new(1.5, 0.6, 3.0),
                Vec3::new(CAR_COLOR, CAR_COLOR, CAR_COLOR),
            );
            
            // Number
            inst(
                &mut frame,
                car.pos + Vec3::new(0.0, 0.6, 0.0),
                Vec3::splat(0.4),
                Vec3::new(0.0, 0.0, 1.0),
            );
        }
        
        // Leaderboard text (in world position)
        for i in 0..PLAYER_COUNT {
            let car = &self.cars[i];
            let pos = car.pos + Vec3::new(0.0, 1.0, 0.0);
            
            inst(
                &mut frame,
                pos,
                Vec3::splat(0.8),
                Vec3::new(1.0, 1.0, 0.0),
            );
        }
        
        frame
    }
}

pub fn run_local() {
    let cars = [
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
        Car {
            pos: Vec3::new(0.0, 0.0, 0.0),
            angle: 0.0,
            speed: 0.0,
            max_speed: 15.0,
            turn_speed: 2.0,
            score: 0,
        },
    ];
    
    let game = GameState {
        cars,
        camera_target: 0,
        camera_angle: 0.0,
        lap_times: vec![],
    };
    
    ember_engine::run(
        EngineConfig {
            title: format!("ember race — {} players racing!", PLAYER_COUNT),
            capture_mouse: false,
            ..Default::default()
        },
        game,
    );
}

// ---- native entry point ----

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run_local();
}

// ---- wasm entry points ----

#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn wasm_init() {
        console_error_panic_hook_lite();
    }

    fn console_error_panic_hook_lite() {
    }

    #[wasm_bindgen]
    pub fn start_local() {
        super::run_local();
    }
}