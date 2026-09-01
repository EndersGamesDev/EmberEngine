#![recursion_limit = "256"]
// Angle normalization intentionally advances through floating-point turn boundaries.
#![allow(clippy::while_float)]

//! Ember — a from-scratch 3D engine.
//!
//! Layering (strict one-way dependencies, top depends on bottom):
//!   game code  ->  scene/simulation (soon)  ->  renderer  ->  platform
//!
//! `app` is the platform layer: window, event loop, input.
//! `renderer` owns the GPU; nothing above it touches wgpu directly.

pub mod assets;
#[cfg(not(target_arch = "wasm32"))]
pub mod overlay;
pub mod puppet;
pub mod renderer;
pub mod rig;

mod app;
mod input;

#[cfg(not(target_arch = "wasm32"))]
pub use app::init_diagnostics;
pub use app::{EngineConfig, run};
pub use input::InputState;
pub use renderer::{Camera, Frame, Instance, MeshData, MeshVertex, TextureData};

// Re-exported so game code doesn't need its own winit/glam dependency for
// the common cases.
pub use glam;
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

/// The game side of the engine contract: called once per frame with input
/// and elapsed time; returns what to draw.
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;
}
