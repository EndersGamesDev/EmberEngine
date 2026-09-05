use ember_lab_heap::DialectLimits;
use wgpu::util::DeviceExt as _;

use crate::{PaletteRecord, PresentError, PresentMain, exterior_zero, scene_indices};

use super::{
    BACKDROP_STENCIL, DEPTH_FORMAT, DepthTarget, GpuState, IndexTarget, SCENE_DEPTH_COMPARE,
    SCENE_FORMAT, SceneLayer, SceneTexture, color, scene_depth_range, scene_draw_order,
    scene_stencil, stencil_reference, viewport_dimension,
};

mod submit;

pub(super) fn create_scene_texture(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    extent: [u32; 2],
) -> SceneTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot scene texture slot"),
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
        label: Some("Julibrot immutable scene warp group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
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

pub(super) fn create_depth_target(device: &wgpu::Device, extent: [u32; 2]) -> DepthTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot scene depth target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    DepthTarget {
        _texture: texture,
        view,
        extent,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_scene_pipeline(
    device: &wgpu::Device,
    label: &'static str,
    source: &str,
    vertex_entry: &'static str,
    fragment_entry: &'static str,
    heap_layout: &wgpu::BindGroupLayout,
    scene_layout: &wgpu::BindGroupLayout,
    color_format: wgpu::TextureFormat,
    depth: Option<SceneLayer>,
) -> wgpu::RenderPipeline {
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[heap_layout, scene_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some(vertex_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some(fragment_entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: depth.map(|layer| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: SCENE_DEPTH_COMPARE,
            stencil: scene_stencil(layer),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
pub(super) fn ensure_scene_texture(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    index: usize,
    extent: [u32; 2],
) -> Result<bool, PresentError> {
    validate_extent(device, extent)?;
    if gpu.scene_textures[index].extent == extent {
        return Ok(false);
    }
    gpu.scene_textures[index] =
        create_scene_texture(device, &gpu.warp_texture_layout, &gpu.scene_sampler, extent);
    Ok(true)
}

pub(super) fn ensure_depth(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: [u32; 2],
) -> Result<(), PresentError> {
    validate_extent(device, extent)?;
    if gpu.depth.extent != extent {
        gpu.depth = create_depth_target(device, extent);
    }
    Ok(())
}

pub(super) fn ensure_indices(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: [u32; 2],
) -> Result<(), PresentError> {
    if gpu
        .indices
        .as_ref()
        .is_some_and(|indices| indices.extent == extent)
    {
        return Ok(());
    }
    let values = scene_indices(extent).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let count = u32::try_from(values.len()).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let contents = if values.is_empty() {
        &[0_u32][..]
    } else {
        &values
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Julibrot scene u32 index buffer"),
        contents: bytemuck::cast_slice(contents),
        usage: wgpu::BufferUsages::INDEX,
    });
    gpu.indices = Some(IndexTarget {
        buffer,
        count,
        extent,
    });
    Ok(())
}

pub(super) fn ensure_backdrop_indices(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    extent: Option<[u32; 2]>,
) -> Result<(), PresentError> {
    let Some(extent) = extent else {
        return Ok(());
    };
    if gpu
        .backdrop_indices
        .as_ref()
        .is_some_and(|indices| indices.extent == extent)
    {
        return Ok(());
    }
    let values = scene_indices(extent).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let count = u32::try_from(values.len()).map_err(|_| PresentError::IndexCountOverflow {
        width: extent[0],
        height: extent[1],
    })?;
    let contents = if values.is_empty() {
        &[0_u32][..]
    } else {
        &values
    };
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Julibrot backdrop u32 index buffer"),
        contents: bytemuck::cast_slice(contents),
        usage: wgpu::BufferUsages::INDEX,
    });
    gpu.backdrop_indices = Some(IndexTarget {
        buffer,
        count,
        extent,
    });
    Ok(())
}

pub(super) fn encode_scene(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    texture_index: usize,
    hot_offset: u32,
    selected: PaletteRecord,
    has_backdrop: bool,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot backdrop then main scene pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.scene_textures[texture_index].view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(scene_load_color(selected)),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(scene_depth_attachment(&gpu.depth.view)),
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    for layer in scene_draw_order(has_backdrop) {
        let [minimum_depth, maximum_depth] = scene_depth_range(*layer, has_backdrop);
        pass.set_viewport(
            0.0,
            0.0,
            viewport_dimension(gpu.depth.extent[0]),
            viewport_dimension(gpu.depth.extent[1]),
            minimum_depth,
            maximum_depth,
        );
        let (pipeline, group, indices) = match layer {
            SceneLayer::Main => (
                &gpu.scene_pipeline,
                &gpu.scene_groups[0],
                gpu.indices.as_ref(),
            ),
            SceneLayer::Backdrop => (
                &gpu.backdrop_pipeline,
                &gpu.scene_groups[1],
                gpu.backdrop_indices.as_ref(),
            ),
        };
        pass.set_pipeline(pipeline);
        pass.set_stencil_reference(stencil_reference(*layer));
        draw_scene_mesh(&mut pass, group, indices, hot_offset);
    }
}

/// The scene depth-stencil attachment: depth cleared to the far plane, the stamp cleared to none.
pub(super) const fn scene_depth_attachment(
    view: &wgpu::TextureView,
) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(1.0),
            store: wgpu::StoreOp::Discard,
        }),
        stencil_ops: Some(wgpu::Operations {
            load: wgpu::LoadOp::Clear(BACKDROP_STENCIL),
            store: wgpu::StoreOp::Discard,
        }),
    }
}
#[allow(
    clippy::too_many_arguments,
    reason = "the shared mesh encoder names its target, pipeline, bindings, palette, and label"
)]
pub(super) fn encode_scene_mesh(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    color_view: &wgpu::TextureView,
    hot_offset: u32,
    load_color: wgpu::Color,
    has_backdrop: bool,
    label: &'static str,
) {
    let depth_attachment = Some(scene_depth_attachment(&gpu.depth.view));
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: color_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(load_color),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: depth_attachment,
        occlusion_query_set: None,
        timestamp_writes: None,
    });
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    for layer in scene_draw_order(has_backdrop) {
        let [minimum_depth, maximum_depth] = scene_depth_range(*layer, has_backdrop);
        pass.set_viewport(
            0.0,
            0.0,
            viewport_dimension(gpu.depth.extent[0]),
            viewport_dimension(gpu.depth.extent[1]),
            minimum_depth,
            maximum_depth,
        );
        let (pipeline, group, indices) = match layer {
            SceneLayer::Main => (
                &gpu.relief_redraw_pipeline,
                &gpu.scene_groups[0],
                gpu.indices.as_ref(),
            ),
            SceneLayer::Backdrop => (
                &gpu.relief_redraw_backdrop_pipeline,
                &gpu.scene_groups[1],
                gpu.backdrop_indices.as_ref(),
            ),
        };
        pass.set_pipeline(pipeline);
        pass.set_stencil_reference(stencil_reference(*layer));
        draw_scene_mesh(&mut pass, group, indices, hot_offset);
    }
}

pub(super) fn draw_scene_mesh<'pass>(
    pass: &mut wgpu::RenderPass<'pass>,
    scene_group: &'pass wgpu::BindGroup,
    indices: Option<&'pass IndexTarget>,
    hot_offset: u32,
) {
    pass.set_bind_group(1, scene_group, &[hot_offset]);
    if let Some(indices) = indices {
        pass.set_index_buffer(indices.buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.count, 0, 0..1);
    }
}

pub fn scene_load_color(selected: PaletteRecord) -> wgpu::Color {
    color(exterior_zero(selected))
}
pub(super) fn validate_grid(main: &PresentMain, limits: DialectLimits) -> Result<(), PresentError> {
    validate_grid_parts(&main.grid, main.state.delivered_iter_cap, limits)
}

pub(super) fn validate_grid_parts(
    grid: &ember_julibrot_kernels::EscapeGrid,
    iteration_cap: u32,
    limits: DialectLimits,
) -> Result<(), PresentError> {
    let active_len = grid
        .width
        .checked_mul(grid.height)
        .filter(|length| *length > 0 && *length <= grid.span.logical_len)
        .ok_or(PresentError::InvalidGrid {
            width: grid.width,
            height: grid.height,
            logical_len: grid.span.logical_len,
        })?;
    if grid.span.directory_index >= limits.span_capacity
        || grid.span.page_count > limits.handle_capacity
        || grid
            .span
            .handles()
            .iter()
            .any(|handle| handle.index() >= limits.descriptor_capacity)
    {
        return Err(PresentError::StaleSpan {
            directory_index: grid.span.directory_index,
        });
    }
    if active_len == 0 || iteration_cap == 0 {
        return Err(PresentError::InvalidGrid {
            width: grid.width,
            height: grid.height,
            logical_len: grid.span.logical_len,
        });
    }
    Ok(())
}

pub(super) fn validate_backdrop(
    backdrop: &crate::PresentBackdrop,
    limits: DialectLimits,
) -> Result<(), PresentError> {
    validate_grid_parts(&backdrop.grid, backdrop.iteration_cap, limits)
}

pub(super) fn validate_extent(device: &wgpu::Device, extent: [u32; 2]) -> Result<(), PresentError> {
    let limit = device.limits().max_texture_dimension_2d;
    if extent[0] == 0 || extent[1] == 0 || extent[0] > limit || extent[1] > limit {
        return Err(PresentError::ExtentAllocation {
            width: extent[0],
            height: extent[1],
        });
    }
    Ok(())
}

pub(super) const fn extent_3d(extent: [u32; 2]) -> wgpu::Extent3d {
    wgpu::Extent3d {
        width: extent[0],
        height: extent[1],
        depth_or_array_layers: 1,
    }
}
