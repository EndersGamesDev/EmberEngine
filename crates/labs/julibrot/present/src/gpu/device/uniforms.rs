use std::num::NonZeroU64;

use ember_lab_heap::DialectLimits;

use crate::{HOT_PAYLOAD_BYTES, PresentError, SCENE_PAYLOAD_BYTES};

pub(super) fn create_heap_layout(
    device: &wgpu::Device,
    limits: DialectLimits,
) -> Result<wgpu::BindGroupLayout, PresentError> {
    let directory_records = limits
        .span_capacity
        .checked_add(limits.handle_capacity.div_ceil(4))
        .ok_or(PresentError::Device {
            operation: "compute heap directory binding size",
        })?;
    let uniform = |binding, bytes| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    };
    Ok(
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Julibrot immutable heap presentation layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                uniform(1, u64::from(limits.descriptor_capacity) * 16),
                uniform(2, u64::from(directory_records) * 16),
            ],
        }),
    )
}

pub(super) fn create_scene_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot scene and HOT layout"),
        entries: &[
            uniform_entry(0, false, u64::from(SCENE_PAYLOAD_BYTES)),
            uniform_entry(1, true, u64::from(HOT_PAYLOAD_BYTES)),
        ],
    })
}

pub(super) fn create_warp_hot_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot warp HOT layout"),
        entries: &[
            uniform_entry(0, true, u64::from(HOT_PAYLOAD_BYTES)),
            uniform_entry(1, false, u64::from(SCENE_PAYLOAD_BYTES)),
        ],
    })
}

pub(super) fn uniform_entry(binding: u32, dynamic: bool, bytes: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    }
}

pub(super) fn create_warp_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot retained scene texture layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}
