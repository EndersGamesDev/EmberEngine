use crate::fence::FenceLedger;
use crate::{HotSlot, PresentDataError, PresentError, PresentStatus, SceneUniform, SubmissionKind};

use super::super::{
    FENCE_BYTES, Presenter, arm_fence, arm_glitch_readback, encode_glitch_count, encode_scene,
    ensure_backdrop_indices, ensure_depth, ensure_glitch_count_resources, ensure_indices,
    ensure_scene_texture, extent_3d, validate_backdrop, validate_extent, validate_grid,
};

impl Presenter {
    /// Submits the one image scene pass, optional Final census, and completion fence.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for absent/invalid state, an occupied target, or checked overflow.
    pub fn submit_scene(&mut self, hot_slot: HotSlot, now_ms: f64) -> Result<u64, PresentError> {
        let result = self.try_submit_scene(hot_slot, now_ms);
        if let Err(error) = &result {
            self.facts.status = PresentStatus::Refused(error.clone());
        }
        result
    }

    #[allow(
        clippy::too_many_lines,
        reason = "scene submission keeps validation and its ordered GPU transaction together"
    )]
    fn try_submit_scene(&mut self, hot_slot: HotSlot, now_ms: f64) -> Result<u64, PresentError> {
        let main = self.main.as_ref().ok_or(PresentError::InvalidGrid {
            width: 0,
            height: 0,
            logical_len: 0,
        })?;
        validate_grid(main, self.gpu.heap_limits)?;
        if let Some(backdrop) = &main.backdrop {
            validate_backdrop(backdrop, self.gpu.heap_limits)?;
        }
        let pose = self.hot[hot_slot.index() as usize].ok_or(PresentError::Device {
            operation: "select unwritten HOT slot",
        })?;
        let (palette_id, palette_record) = main.selected_palette().ok_or(PresentError::Device {
            operation: "decode palette identifier",
        })?;
        let precision_mode = main.precision_mode().ok_or(PresentError::Device {
            operation: "decode precision policy",
        })?;
        let extent = [main.grid.width, main.grid.height];
        validate_extent(&self.device, extent)?;
        let uniform = SceneUniform::new(
            extent,
            main.grid.level as u32,
            main.state.delivered_iter_cap,
            main.grid.span.directory_index,
            main.grid.span.logical_len,
            main.plane,
            main.map,
            palette_record,
        )
        .map_err(|error| match error {
            PresentDataError::InvalidMap => PresentError::Device {
                operation: "pack scene screen map",
            },
            _ => PresentError::InvalidGrid {
                width: extent[0],
                height: extent[1],
                logical_len: main.grid.span.logical_len,
            },
        })?;
        let backdrop_uniform = main
            .backdrop
            .as_ref()
            .map(|backdrop| {
                SceneUniform::new(
                    [backdrop.grid.width, backdrop.grid.height],
                    backdrop.grid.level as u32,
                    backdrop.iteration_cap,
                    backdrop.grid.span.directory_index,
                    backdrop.grid.span.logical_len,
                    backdrop.plane,
                    backdrop.map,
                    palette_record,
                )
            })
            .transpose()
            .map_err(|error| match error {
                PresentDataError::InvalidMap => PresentError::Device {
                    operation: "pack backdrop screen map",
                },
                _ => PresentError::InvalidGrid {
                    width: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.width),
                    height: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.height),
                    logical_len: main
                        .backdrop
                        .as_ref()
                        .map_or(0, |backdrop| backdrop.grid.span.logical_len),
                },
            })?;
        let scene_id = self.next_scene_id;
        let next_scene_id = scene_id.checked_add(1).ok_or(PresentError::Device {
            operation: "advance scene identity",
        })?;
        let device = &self.device;
        let gpu = &mut self.gpu;
        let facts = &mut self.facts;
        let scene_samples = &mut self.scene_samples;
        let warp_samples = &mut self.warp_samples;
        let texture_index = self.ledger.begin(|texture_index| {
            let reallocated = ensure_scene_texture(device, gpu, texture_index as usize, extent)?;
            if reallocated {
                facts.texture_reallocations = facts.texture_reallocations.saturating_add(1);
                scene_samples.reset();
                warp_samples.reset();
            }
            ensure_indices(device, gpu, extent)?;
            ensure_backdrop_indices(
                device,
                gpu,
                main.backdrop
                    .as_ref()
                    .map(|backdrop| [backdrop.grid.width, backdrop.grid.height]),
            )?;
            ensure_depth(device, gpu, extent)?;
            ensure_glitch_count_resources(device, gpu, extent);
            Ok(crate::state::PendingScene {
                scene_id,
                pose,
                palette: palette_id,
                iteration_cap: main.state.delivered_iter_cap,
                level: main.grid.level,
                extent,
                texture_index,
                centre_revision: main.state.centre_revision,
                plane_origin_f64: main.state.plane_origin_f64,
                precision_mode: precision_mode.as_str(),
                drop_reason: None,
            })
        })?;
        self.next_scene_id = next_scene_id;
        self.queue
            .write_buffer(&self.gpu.scene_buffers[0], 0, bytemuck::bytes_of(&uniform));
        if let Some(backdrop_uniform) = backdrop_uniform {
            self.queue.write_buffer(
                &self.gpu.scene_buffers[1],
                0,
                bytemuck::bytes_of(&backdrop_uniform),
            );
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot scene and fence"),
            });
        encode_scene(
            &mut encoder,
            &self.gpu,
            texture_index as usize,
            hot_slot.dynamic_offset(),
            palette_record,
            main.backdrop.is_some(),
        );
        let readback = &self.gpu.glitch_readback;
        encode_glitch_count(&mut encoder, &self.gpu, hot_slot.dynamic_offset());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.gpu.glitch_count_target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(readback.bytes_per_row),
                    rows_per_image: Some(readback.extent[1]),
                },
            },
            extent_3d(readback.extent),
        );
        encoder.clear_buffer(&self.gpu.scene_fence, 0, Some(FENCE_BYTES));
        self.queue.submit([encoder.finish()]);
        let glitch_readback = Some(arm_glitch_readback(&self.gpu.glitch_readback.buffer));
        self.scene_fence = Some(arm_fence(
            &self.gpu.scene_fence,
            glitch_readback,
            FenceLedger::new(
                SubmissionKind::Scene,
                scene_id,
                None,
                precision_mode.as_str(),
                self.scene_samples.next(),
                now_ms,
                self.config.fence_deadline_ms,
                self.config.max_fence_polls,
            ),
        ));
        self.facts.in_flight_scene_id = Some(scene_id);
        Ok(scene_id)
    }
}
