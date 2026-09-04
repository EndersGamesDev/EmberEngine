//! Cross-slice progressive frame scheduling and browser GPU integration.

mod r#loop;
mod schedule;
mod warp;

#[cfg(target_arch = "wasm32")]
pub use r#loop::BrowserFrameLoop;
pub use schedule::{RefinementSchedule, SceneMode};
