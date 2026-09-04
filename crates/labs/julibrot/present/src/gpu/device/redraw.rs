use crate::{PaletteRecord, PresentDataError, PresentError, SceneUniform};

use super::{
    GpuState, Presenter, encode_scene_mesh, ensure_depth, ensure_indices, validate_grid_parts,
    warp_load_color,
};

impl Presenter {
    pub(super) fn prepare_relief_redraw(
        &mut self,
        source: &crate::SceneFrame,
        surface_extent: [u32; 2],
        selected: PaletteRecord,
    ) -> Result<(), PresentError> {
        let grid = self
            .ledger
            .retained_grid()
            .ok_or(PresentError::InvalidGrid {
                width: source.extent[0],
                height: source.extent[1],
                logical_len: 0,
            })?;
        validate_grid_parts(grid, source.iteration_cap, self.gpu.heap_limits)?;
        let uniform = relief_scene_uniform(grid, source, selected)?;
        ensure_indices(&self.device, &mut self.gpu, source.extent)?;
        ensure_depth(&self.device, &mut self.gpu, surface_extent)?;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        Ok(())
    }
}

pub(super) fn encode_relief_redraw(
    encoder: &mut wgpu::CommandEncoder,
    gpu: &GpuState,
    surface_view: &wgpu::TextureView,
    hot_offset: u32,
    selected: PaletteRecord,
) {
    encode_scene_mesh(
        encoder,
        gpu,
        surface_view,
        &gpu.relief_redraw_pipeline,
        hot_offset,
        warp_load_color(selected),
        "Julibrot relief redraw pass",
    );
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
