//! GPU presenter implementation, partitioned by rendering responsibility.

mod device;

pub use device::Presenter;

#[cfg(test)]
pub use device::scene_load_color;
