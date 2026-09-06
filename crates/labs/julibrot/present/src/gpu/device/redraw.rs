use crate::{Pose, PresentDataError, PresentError, SceneUniform};

use super::{GpuState, Presenter, encode_scene_mesh, ensure_depth, ensure_indices, validate_grid_parts, warp_load_color};

impl Presenter {
    pub(super) fn prepare_relief_redraw(
        &mut self,
        source: &crate::SceneFrame,
        destination: &Pose,
        surface_extent: [u32; 2],
    ) -> Result<bool, PresentError> {
        let Some(grid) = self.ledger.retained_grid() else {
            return Ok(false);
        };
        if validate_grid_parts(grid, source.iteration_cap, self.gpu.heap_limits).is_err() {
            return Ok(false);
        }
        let Ok(uniform) = relief_scene_uniform(grid, source, destination) else {
            return Ok(false);
        };
        ensure_indices(&self.device, &mut self.gpu, source.extent)?;
        ensure_depth(&self.device, &mut self.gpu, surface_extent)?;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        Ok(true)
    }

    pub(super) fn retained_records_support_relief_redraw(
        &self,
        source: &crate::SceneFrame,
        destination: &Pose,
    ) -> bool {
        self.ledger.retained_grid().is_some_and(|grid| {
            validate_grid_parts(grid, source.iteration_cap, self.gpu.heap_limits).is_ok()
                && relief_scene_uniform(grid, source, destination).is_ok()
        })
    }
}

pub(super) fn encode_relief_redraw(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
    hot_offset: u32,
) {
    encode_scene_mesh(
        encoder,
        gpu,
        surface_view,
        hot_offset,
        warp_load_color(),
        false,
        "Julibrot relief redraw pass",
    );
}

pub(super) fn relief_scene_uniform(
    grid: &ember_julibrot_kernels::EscapeGrid,
    source: &crate::SceneFrame,
    destination: &Pose,
) -> Result<SceneUniform, PresentError> {
    if [grid.width, grid.height] != source.extent {
        return Err(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: grid.span.logical_len,
        });
    }
    let redraw = crate::relief_redraw_source_pose(&source.pose, source.extent, destination).ok_or(
        PresentError::Device {
            operation: "compose relief redraw source lattice",
        },
    )?;
    SceneUniform::new(
        source.extent,
        source.level as u32,
        source.iteration_cap,
        grid.span.directory_index,
        grid.span.logical_len,
        redraw.plane,
        redraw.map,
    )
    .map_err(|error| match error {
        PresentDataError::InvalidMap => PresentError::Device {
            operation: "pack relief redraw source map",
        },
        _ => PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: grid.span.logical_len,
        },
    })
}
