use std::num::NonZeroU64;

use ember_lab_heap::DialectLimits;

use crate::shade_shader::{
    NEAREST_VALUE_BINDING, PALETTE_UNIFORM_BINDING, PRESENTATION_VALUES_BINDING,
    palette_uniform_bytes,
};
use crate::{HOT_PAYLOAD_BYTES, PresentError, SCENE_PAYLOAD_BYTES};

pub(super) fn create_heap_layout(
    device: &wgpu::Device,
    limits: DialectLimits,
) -> Result<wgpu::BindGroupLayout, PresentError> {
    let entries = heap_layout_entries(limits)?;
    Ok(
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Julibrot immutable heap presentation layout"),
            entries: &entries,
        }),
    )
}

pub(super) fn heap_layout_entries(
    limits: DialectLimits,
) -> Result<[wgpu::BindGroupLayoutEntry; 3], PresentError> {
    let directory_records = limits
        .span_capacity
        .checked_add(limits.handle_capacity.div_ceil(4))
        .ok_or(PresentError::Device {
            operation: "compute heap directory binding size",
        })?;
    Ok([
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
        static_uniform_entry(1, u64::from(limits.descriptor_capacity) * 16),
        static_uniform_entry(2, u64::from(directory_records) * 16),
    ])
}

fn static_uniform_entry(binding: u32, bytes: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    }
}

fn hot_uniform_entry(binding: u32, bytes: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: NonZeroU64::new(bytes),
        },
        count: None,
    }
}

pub(super) fn scene_layout_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        static_uniform_entry(0, u64::from(SCENE_PAYLOAD_BYTES)),
        hot_uniform_entry(1, u64::from(HOT_PAYLOAD_BYTES)),
    ]
}

pub(super) fn warp_hot_layout_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        hot_uniform_entry(0, u64::from(HOT_PAYLOAD_BYTES)),
        static_uniform_entry(1, u64::from(SCENE_PAYLOAD_BYTES)),
    ]
}

pub(super) fn palette_layout_entries() -> [wgpu::BindGroupLayoutEntry; 1] {
    [static_uniform_entry(
        PALETTE_UNIFORM_BINDING,
        palette_uniform_bytes(),
    )]
}

pub(super) fn create_scene_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries = scene_layout_entries();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot scene and HOT layout"),
        entries: &entries,
    })
}

pub(super) fn create_warp_hot_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries = warp_hot_layout_entries();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot warp HOT layout"),
        entries: &entries,
    })
}

pub(super) fn create_warp_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot retained scene texture layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: PRESENTATION_VALUES_BINDING,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: NEAREST_VALUE_BINDING,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                count: None,
            },
        ],
    })
}

pub(super) fn create_palette_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let entries = palette_layout_entries();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Julibrot present-time palette layout"),
        entries: &entries,
    })
}
