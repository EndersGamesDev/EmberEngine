use crate::{Pose, PresentDataError, PresentError, SceneUniform};

use super::{
    GpuState, Presenter, encode_scene_mesh, ensure_depth, ensure_indices, validate_grid_parts,
    warp_load_color,
};

/// Measured excess-stretch candidate for generalized relief redraws, in destination pixels.
pub(super) const RELIEF_STRETCH_GUARD_CANDIDATE: f64 = 1.0;

/// Optional excess-stretch allowance for generalized relief redraws, in destination pixels.
///
/// `None` leaves the measured candidate guard disabled while its coverage trade-off is decided.
pub(super) const RELIEF_STRETCH_GUARD: Option<f64> = None;

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
        let Ok(uniform) = relief_scene_uniform(grid, source, destination, surface_extent) else {
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
                && relief_scene_uniform(
                    grid,
                    source,
                    destination,
                    [destination.grid_width, destination.grid_height],
                )
                .is_ok()
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
    surface_extent: [u32; 2],
) -> Result<SceneUniform, PresentError> {
    relief_scene_uniform_with_guard(
        grid,
        source,
        destination,
        surface_extent,
        RELIEF_STRETCH_GUARD,
    )
}

pub(super) fn relief_scene_uniform_with_guard(
    grid: &ember_julibrot_kernels::EscapeGrid,
    source: &crate::SceneFrame,
    destination: &Pose,
    surface_extent: [u32; 2],
    stretch_guard: Option<f64>,
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
    let mut uniform = SceneUniform::new(
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
    })?;
    if !crate::planner::exact_relief_redraw_family(&source.pose, destination) {
        let Some(excess_px) = stretch_guard else {
            return Ok(uniform);
        };
        #[allow(
            clippy::cast_precision_loss,
            reason = "the checked presentation extent is narrowed once into the scene GPU ABI"
        )]
        let presentation_extent = [surface_extent[0] as f32, surface_extent[1] as f32];
        if presentation_extent
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(PresentError::Device {
                operation: "pack relief redraw presentation extent",
            });
        }
        #[allow(
            clippy::cast_possible_truncation,
            reason = "the finite guard allowance is narrowed once into the scene GPU ABI"
        )]
        let excess_px = excess_px as f32;
        if !excess_px.is_finite() || excess_px < 0.0 {
            return Err(PresentError::Device {
                operation: "pack relief redraw stretch allowance",
            });
        }
        uniform.reserved_0 = [
            1.0,
            presentation_extent[0],
            presentation_extent[1],
            excess_px,
        ];
    }
    Ok(uniform)
}
