//! Cross-slice progressive frame scheduling and browser GPU integration.

mod r#loop;

#[cfg(target_arch = "wasm32")]
pub use r#loop::BrowserFrameLoop;
pub use r#loop::{RefinementSchedule, SceneMode};
