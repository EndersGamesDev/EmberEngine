//! Cross-slice progressive frame scheduling and browser GPU integration.

mod r#loop;
mod schedule;

#[cfg(target_arch = "wasm32")]
pub use r#loop::BrowserFrameLoop;
pub use schedule::{RefinementSchedule, SceneMode};
