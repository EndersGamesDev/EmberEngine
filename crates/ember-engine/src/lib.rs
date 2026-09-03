#![recursion_limit = "256"]
// Angle normalization intentionally advances through floating-point turn boundaries.
#![allow(clippy::while_float)]

//! Ember — a from-scratch 3D engine.
//!
//! Layering (strict one-way dependencies, top depends on bottom):
//!   game code  ->  scene/simulation (soon)  ->  renderer  ->  platform
//!
//! `app` is the platform layer: window, event loop, input, haptics.
//! `renderer` owns the GPU; nothing above it touches wgpu directly.

pub mod assets;
pub mod feedback;
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
pub use feedback::{Feedback, Rumble};
pub use input::{InputState, PadButton, PadState};
pub use renderer::{Camera, Fog, Frame, Instance, MeshData, MeshVertex, TextureData};

// Re-exported so game code doesn't need its own winit/glam dependency for
// the common cases.
pub use glam;
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

/// The game side of the engine contract: called once per frame with input
/// and elapsed time; returns what to draw.
pub trait EmberGame {
    fn update(&mut self, input: &InputState, dt: f32) -> Frame;

    /// Called by the platform right after `update`, once per frame, for
    /// what the player should feel: rumble today. The default returns
    /// nothing, so pong, fire, kings and the editor are untouched by the
    /// channel existing; a game opts in by overriding it.
    fn feedback(&mut self) -> Feedback {
        Feedback::default()
    }
}
