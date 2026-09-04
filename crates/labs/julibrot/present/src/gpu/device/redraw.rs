use crate::{PaletteRecord, PresentDataError, PresentError, PresentMain, SceneUniform};

use super::{
    GpuState, Presenter, encode_scene_mesh, ensure_depth, ensure_indices, validate_grid,
    warp_load_color,
};

impl Presenter {
    pub(super) fn prepare_relief_redraw(
        &mut self,
        source: &crate::SceneFrame,
        surface_extent: [u32; 2],
        selected: PaletteRecord,
    ) -> Result<(), PresentError> {
        let main = self.main.as_ref().ok_or(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: 0,
        })?;
        validate_grid(main, self.gpu.heap_limits)?;
        let uniform = relief_scene_uniform(main, source, selected)?;
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
    main: &PresentMain,
    source: &crate::SceneFrame,
    selected: PaletteRecord,
) -> Result<SceneUniform, PresentError> {
    if [main.grid.width, main.grid.height] != source.extent {
        return Err(PresentError::InvalidGrid {
            width: source.extent[0],
            height: source.extent[1],
            logical_len: main.grid.span.logical_len,
        });
    }
    SceneUniform::new(
        source.extent,
        source.level as u32,
        source.iteration_cap,
        main.grid.span.directory_index,
        main.grid.span.logical_len,
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
            logical_len: main.grid.span.logical_len,
        },
    })
}
