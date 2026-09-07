//! Cross-slice progressive frame scheduling and browser GPU integration.

mod r#loop;
mod schedule;
mod warp;

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) use r#loop::BrowserRefreshOrder;
#[cfg(target_arch = "wasm32")]
pub use r#loop::BrowserFrameLoop;
pub use schedule::{RefinementSchedule, SceneMode};
