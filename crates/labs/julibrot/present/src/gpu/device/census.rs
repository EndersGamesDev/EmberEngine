use std::sync::{Arc, Mutex};

use crate::fence::{FenceDecision, FenceLedger};

use super::{
    FENCE_BYTES, GLITCH_RECORDS_PER_TEXEL, GlitchCountTarget, GlitchReadback, GpuState,
    PendingFence, PendingGlitchReadback, RGBA8_BYTES_PER_TEXEL, SCENE_FORMAT, SceneCensus,
    extent_3d,
};

pub(super) fn create_glitch_count_target(
    device: &wgpu::Device,
    extent: [u32; 2],
) -> GlitchCountTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Julibrot status-one census target"),
        size: extent_3d(extent),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SCENE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    GlitchCountTarget {
        texture,
        view,
        extent,
    }
}
pub(super) fn ensure_glitch_count_resources(
    device: &wgpu::Device,
    gpu: &mut GpuState,
    scene_extent: [u32; 2],
) {
    let extent = [
        scene_extent[0],
        scene_extent[1].div_ceil(GLITCH_RECORDS_PER_TEXEL),
    ];
    if gpu.glitch_count_target.extent != extent {
        gpu.glitch_count_target = create_glitch_count_target(device, extent);
        gpu.glitch_readback = create_glitch_readback(device, extent);
    }
}
pub(super) fn encode_glitch_count(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    hot_offset: u32,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Julibrot status-one census pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &gpu.glitch_count_target.view,
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
    pass.set_pipeline(&gpu.glitch_count_pipeline);
    pass.set_bind_group(0, &gpu.heap_group, &[]);
    // The census counts the MAIN grid's records. Slot one is the backdrop and never enters it: a
    // Final main must publish the same glitch count with a backdrop attached as without one.
    pass.set_bind_group(1, &gpu.scene_groups[0], &[hot_offset]);
    pass.draw(0..3, 0..1);
}
pub(super) fn create_fence(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: FENCE_BYTES,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

pub(super) fn create_glitch_readback(device: &wgpu::Device, extent: [u32; 2]) -> GlitchReadback {
    let packed_row = extent[0] * RGBA8_BYTES_PER_TEXEL;
    let bytes_per_row = packed_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Julibrot status-one census readback"),
        size: u64::from(bytes_per_row) * u64::from(extent[1]),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    GlitchReadback {
        buffer,
        extent,
        bytes_per_row,
    }
}

pub(super) fn arm_glitch_readback(buffer: &wgpu::Buffer) -> PendingGlitchReadback {
    let signal = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&signal);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = callback.lock() {
                *slot = Some(result.map_err(|_| ()));
            }
        });
    PendingGlitchReadback { signal }
}

pub(super) fn arm_fence(
    buffer: &wgpu::Buffer,
    glitch_readback: Option<PendingGlitchReadback>,
    ledger: FenceLedger,
) -> PendingFence {
    let signal = Arc::new(Mutex::new(None));
    let callback = Arc::clone(&signal);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            if let Ok(mut slot) = callback.lock() {
                *slot = Some(result.map_err(|_| ()));
            }
        });
    PendingFence {
        ledger,
        signal,
        signal_result: None,
        glitch_readback,
    }
}

pub(super) fn observe_fence(pending: &mut PendingFence, now_ms: f64) -> FenceDecision {
    if pending.signal_result.is_none() {
        pending.signal_result = pending.signal.lock().ok().and_then(|mut slot| slot.take());
    }
    pending.ledger.observe(now_ms, pending.signal_result)
}

pub(super) fn take_glitch_readback_result(pending: &mut PendingFence) -> Option<Result<(), ()>> {
    pending
        .glitch_readback
        .as_mut()?
        .signal
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub(super) fn census_if_ready(
    result: Option<Result<(), ()>>,
    census: impl FnOnce() -> SceneCensus,
) -> Option<SceneCensus> {
    matches!(result, Some(Ok(()))).then(census)
}

pub(super) fn mapped_census(readback: &GlitchReadback) -> SceneCensus {
    let bytes = readback.buffer.slice(..).get_mapped_range();
    let census = census_bytes(&bytes, readback.extent, readback.bytes_per_row);
    drop(bytes);
    census
}

/// Decodes one census readback: the exact status-one sum and the deterministic best candidate.
///
/// The candidate is the record of highest encoded iteration rank; equal ranks keep the lowest
/// record index, so the same completed grid always names the same reference point.
pub(super) fn census_bytes(bytes: &[u8], extent: [u32; 2], bytes_per_row: u32) -> SceneCensus {
    let packed_row = extent[0] as usize * RGBA8_BYTES_PER_TEXEL as usize;
    let mut census = SceneCensus::EMPTY;
    let mut best_rank = 0;
    for row in 0..extent[1] {
        let start = row as usize * bytes_per_row as usize;
        let texels = bytes[start..start + packed_row]
            .as_chunks::<{ RGBA8_BYTES_PER_TEXEL as usize }>()
            .0;
        for (column, rgba) in texels.iter().enumerate() {
            census.glitch_pixel_count =
                census.glitch_pixel_count.saturating_add(u32::from(rgba[0]));
            let located =
                rgba[3] != 0 && (census.reference_sample.is_none() || rgba[1] > best_rank);
            if !located {
                continue;
            }
            let Some(index) = u32::try_from(column)
                .ok()
                .and_then(|column| row.checked_mul(extent[0])?.checked_add(column))
                .and_then(|group| group.checked_mul(GLITCH_RECORDS_PER_TEXEL))
                .and_then(|base| base.checked_add(u32::from(rgba[2])))
            else {
                continue;
            };
            best_rank = rgba[1];
            census.reference_sample = Some(index);
        }
    }
    census
}
