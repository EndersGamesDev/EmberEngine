use crate::{PaletteRecord, PresentDataError, PresentError, SceneUniform};

use super::{
    GpuState, Presenter, encode_scene_mesh, ensure_backdrop_indices, ensure_depth, ensure_indices,
    validate_backdrop, validate_grid_parts, warp_load_color,
};

impl Presenter {
    pub(super) fn prepare_relief_redraw(
        &mut self,
        source: &crate::SceneFrame,
        surface_extent: [u32; 2],
        selected: PaletteRecord,
    ) -> Result<Option<bool>, PresentError> {
        let Some(grid) = self.ledger.retained_grid() else {
            return Ok(None);
        };
        if validate_grid_parts(grid, source.iteration_cap, self.gpu.heap_limits).is_err() {
            return Ok(None);
        }
        let Ok(uniform) = relief_scene_uniform(grid, source, selected) else {
            return Ok(None);
        };
        let backdrop = self.main.as_ref().and_then(|main| main.backdrop.as_ref());
        let backdrop_uniform = backdrop.and_then(|backdrop| {
            validate_backdrop(backdrop, self.gpu.heap_limits)
                .ok()
                .and_then(|()| backdrop_scene_uniform(backdrop, selected).ok())
        });
        let backdrop_extent = backdrop_uniform
            .as_ref()
            .and_then(|_| backdrop.map(|backdrop| [backdrop.grid.width, backdrop.grid.height]));
        ensure_indices(&self.device, &mut self.gpu, source.extent)?;
        ensure_backdrop_indices(&self.device, &mut self.gpu, backdrop_extent)?;
        ensure_depth(&self.device, &mut self.gpu, surface_extent)?;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        if let Some(backdrop_uniform) = backdrop_uniform {
            self.queue.write_buffer(
                &self.gpu.scene_buffers[1],
                0,
                bytemuck::bytes_of(&backdrop_uniform),
            );
        }
        Ok(Some(backdrop_extent.is_some()))
    }

    pub(super) fn retained_records_support_relief_redraw(
        &self,
        source: &crate::SceneFrame,
        selected: PaletteRecord,
    ) -> bool {
        self.ledger.retained_grid().is_some_and(|grid| {
            validate_grid_parts(grid, source.iteration_cap, self.gpu.heap_limits).is_ok()
                && relief_scene_uniform(grid, source, selected).is_ok()
        })
    }
}

pub(super) fn encode_relief_redraw(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
    hot_offset: u32,
    selected: PaletteRecord,
    has_backdrop: bool,
) {
    encode_scene_mesh(
        encoder,
        gpu,
        surface_view,
        hot_offset,
        warp_load_color(selected),
        has_backdrop,
        "Julibrot relief redraw pass",
    );
}

fn backdrop_scene_uniform(
    backdrop: &crate::PresentBackdrop,
    selected: PaletteRecord,
) -> Result<SceneUniform, PresentError> {
    SceneUniform::new(
        [backdrop.grid.width, backdrop.grid.height],
        backdrop.grid.level as u32,
        backdrop.iteration_cap,
        backdrop.grid.span.directory_index,
        backdrop.grid.span.logical_len,
        backdrop.plane,
        backdrop.map,
        selected,
    )
    .map_err(|error| match error {
        PresentDataError::InvalidMap => PresentError::Device {
            operation: "pack relief redraw backdrop map",
        },
        _ => PresentError::InvalidGrid {
            width: backdrop.grid.width,
            height: backdrop.grid.height,
            logical_len: backdrop.grid.span.logical_len,
        },
    })
}

pub(super) fn relief_scene_uniform(
    grid: &ember_julibrot_kernels::EscapeGrid,
    source: &crate::SceneFrame,
    selected: PaletteRecord,
) -> Result<SceneUniform, PresentError> {
    if [grid.width, grid.height] != source.extent {
        return Err(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: grid.span.logical_len,
        });
    }
    SceneUniform::new(
        source.extent,
        source.level as u32,
        source.iteration_cap,
        grid.span.directory_index,
        grid.span.logical_len,
        source.pose.plane,
        source.pose.map,
        selected,
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
