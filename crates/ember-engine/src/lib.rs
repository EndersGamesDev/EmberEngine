//! Ember — a from-scratch 3D engine.
//!
//! Layering (strict one-way dependencies, top depends on bottom):
//!   game code  ->  scene/simulation (soon)  ->  renderer  ->  platform
//!
//! `app` is the platform layer: window, event loop, input.
//! `renderer` owns the GPU; nothing above it touches wgpu directly.

pub mod renderer;

mod app;
mod input;

pub use app::{run, EngineConfig};
pub use input::InputState;
pub use renderer::{Camera, Frame, Instance};

// Re-exported so game code doesn't need its own winit/glam dependency for
// the common cases.
pub use glam;
pub use winit::keyboard::KeyCode;

/// The game side of the engine contract: called once per frame with input
/// and elapsed time; returns what to draw.
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
}
