//! GPU presenter implementation, partitioned by rendering responsibility.

mod device;

pub use device::Presenter;
pub use device::readback::{FrameReadback, FrameReadbackRoute, frame_readback_route};

#[cfg(test)]
pub use device::scene_load_color;
