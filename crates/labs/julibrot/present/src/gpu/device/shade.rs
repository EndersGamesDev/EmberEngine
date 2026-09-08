use crate::PaletteRecord;
use crate::shade_shader::{
    NEAREST_VALUE_BINDING, PRESENTATION_VALUES_BINDING, SHADE_PALETTE_GROUP, SHADE_VALUES_GROUP,
};

use super::{GpuState, SCENE_FORMAT, SceneTexture, extent_3d};

pub(super) fn create_value_target(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    extent: [u32; 2],
) -> SceneTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot presentation value target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let warp_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Julibrot presentation value shade group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: PRESENTATION_VALUES_BINDING,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: NEAREST_VALUE_BINDING,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    SceneTexture {
        _texture: texture,
        view,
        warp_group,
        extent,
    }
}

pub(super) fn ensure_value_target(device: &wgpu::Device, gpu: &mut GpuState, extent: [u32; 2]) {
    if gpu.presentation_values.extent != extent {
        gpu.presentation_values =
            create_value_target(device, &gpu.warp_texture_layout, &gpu.scene_sampler, extent);
    }
}

pub(super) fn create_shade_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    value_layout: &wgpu::BindGroupLayout,
    palette_layout: &wgpu::BindGroupLayout,
    source: &str,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Julibrot sole shade shader"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let bind_group_layouts = shade_bind_group_layouts(value_layout, palette_layout);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Julibrot sole shade pipeline"),
        bind_group_layouts: &bind_group_layouts,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Julibrot sole shade pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("shade_vertex"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("shade_fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn shade_bind_group_layouts<'a>(
    value_layout: &'a wgpu::BindGroupLayout,
    palette_layout: &'a wgpu::BindGroupLayout,
) -> [&'a wgpu::BindGroupLayout; 2] {
    match (SHADE_VALUES_GROUP, SHADE_PALETTE_GROUP) {
        (0, 1) => [value_layout, palette_layout],
        (1, 0) => [palette_layout, value_layout],
        _ => panic!("shade bind groups must be zero and one"),
    }
}

pub(super) fn encode_shade(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot sole shade pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: surface_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_pipeline(&gpu.shade_pipeline);
    pass.set_bind_group(SHADE_VALUES_GROUP, &gpu.presentation_values.warp_group, &[]);
    pass.set_bind_group(SHADE_PALETTE_GROUP, &gpu.palette_group, &[]);
    pass.draw(0..3, 0..1);
}

pub(super) fn write_palette(queue: &wgpu::Queue, gpu: &GpuState, selected: PaletteRecord) {
    queue.write_buffer(&gpu.palette_buffer, 0, bytemuck::bytes_of(&selected));
}
