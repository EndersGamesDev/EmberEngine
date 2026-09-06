//! GPU presenter implementation, partitioned by rendering responsibility.

mod device;

pub use device::Presenter;
pub use device::readback::FrameReadback;

#[cfg(test)]
pub use device::scene_load_color;
