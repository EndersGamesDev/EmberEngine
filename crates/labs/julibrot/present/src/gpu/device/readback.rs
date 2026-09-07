//! One requested copy of a presented frame, taken where the pass already owns the frame texture.
//!
//! A picture that can only be read back through a context flag set in a private copy of the page is
//! a picture measured on an edited page, and the edit is exactly the sort of thing a proof is meant
//! to exclude. The copy below needs no such edit. The cost is stated rather than hidden — one full
//! copy of the surface, four bytes a pixel, so 2,073,600 bytes at 960 by 540, plus the buffer's own
//! allocation for as long as the copy is in flight — and it is paid on request only, never per
//! frame.
//!
//! There are two routes to those bytes, because there are two kinds of device.
//!
//! The direct route copies the surface image itself. It is the better one and it is unavailable on
//! the floor this lab targets: a WebGL2 swapchain surface does not offer `COPY_SRC`, measured on
//! ANGLE over Mesa Intel, so a lab that only had this route would answer a typed refusal to every
//! request on the device class it is for.
//!
//! The fallback route draws the shade pass a second time into an offscreen colour target that is a
//! copy source, and copies from that. It is pixel-identical by construction rather than by hope:
//! the second encode is the same call over the already reprojected value target, with the same
//! pipeline, bind groups and palette, appended to the same command encoder as the first before that
//! encoder is submitted. Nothing writes those inputs between the two encodes. The one thing
//! that could separate them is an extent disagreement between the surface the pass was drawn at and
//! the target the copy is taken from, and that is refused with a reason rather than resized.
//!
//! Rows come back top-down, in the order the texture holds them, packed and in RGBA order whatever
//! the surface's own channel order is: a caller counting colours must not have to know which of the
//! two eight-bit surface formats the browser handed the device.

use std::sync::{Arc, Mutex};

use crate::PresentError;

use super::shade::encode_shade;
use super::{MapSignal, Presenter, RGBA8_BYTES_PER_TEXEL};

/// Which of the two routes produced, or will produce, a frame copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameReadbackRoute {
    /// The surface image itself was the copy source.
    Surface,
    /// The presentation pass was drawn once more into an offscreen copy source.
    OffscreenRerender,
}

impl FrameReadbackRoute {
    /// Returns the sentence the page publishes for this route.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Surface => "copied from the surface",
            Self::OffscreenRerender => "copied from an offscreen re-render of the shade pass",
        }
    }
}

/// Selects the route an armed capture must take on a device exposing these surface usages.
///
/// The direct route is chosen wherever it exists, because copying the image the browser presented
/// answers the question outright; the fallback is chosen only where the surface refuses to be a
/// copy source at all, which is the WebGL2 floor's ordinary answer.
#[must_use]
pub const fn frame_readback_route(surface_usages: wgpu::TextureUsages) -> FrameReadbackRoute {
    if surface_usages.contains(wgpu::TextureUsages::COPY_SRC) {
        FrameReadbackRoute::Surface
    } else {
        FrameReadbackRoute::OffscreenRerender
    }
}

/// One copy of a presented frame: packed RGBA, four bytes a pixel, rows top-down.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameReadback {
    /// Copied width in pixels.
    pub width: u32,
    /// Copied height in pixels.
    pub height: u32,
    /// Which route produced these bytes.
    pub route: FrameReadbackRoute,
    /// The completed scene the presentation pass was reading when the copy was encoded.
    pub scene_id: Option<u64>,
    /// Packed RGBA bytes, `width * height * 4` of them, first row first.
    pub rgba: Vec<u8>,
}

/// The channel order the surface texture holds, and therefore whether the copy is reordered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ChannelOrder {
    /// Already the returned order.
    Rgba,
    /// Blue and red exchanged on the way out.
    Bgra,
}

/// A copy whose commands are encoded and whose buffer is not yet mapped.
pub(super) struct EncodedFrameReadback {
    buffer: wgpu::Buffer,
    extent: [u32; 2],
    bytes_per_row: u32,
    order: ChannelOrder,
    route: FrameReadbackRoute,
    scene_id: Option<u64>,
}

impl EncodedFrameReadback {
    /// Asks for the map, which may only be done once the commands have been submitted.
    fn map(self) -> PendingFrameReadback {
        let signal: MapSignal = Arc::new(Mutex::new(None));
        let callback = Arc::clone(&signal);
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if let Ok(mut slot) = callback.lock() {
                    *slot = Some(result.map_err(|_| ()));
                }
            });
        PendingFrameReadback {
            buffer: self.buffer,
            extent: self.extent,
            bytes_per_row: self.bytes_per_row,
            order: self.order,
            route: self.route,
            scene_id: self.scene_id,
            signal,
        }
    }
}

pub(super) struct PendingFrameReadback {
    buffer: wgpu::Buffer,
    extent: [u32; 2],
    bytes_per_row: u32,
    order: ChannelOrder,
    route: FrameReadbackRoute,
    scene_id: Option<u64>,
    signal: MapSignal,
}

/// The offscreen colour target the fallback route draws into, kept across captures at one extent.
pub(super) struct ReadbackTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    extent: [u32; 2],
}

impl Presenter {
    /// Copies one presented frame texture into a mapped buffer, on request only.
    ///
    /// This is the direct route: the texture is the image the surface is about to present, so what
    /// is read back is the frame itself. The copy is submitted here and completes on a later turn;
    /// [`Self::take_frame_readback`] returns it.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal when a copy is already in flight, when the texture cannot be a copy
    /// source, when its extent is zero, or when its format is not an eight-bit four-channel one.
    pub fn request_frame_readback(&mut self, texture: &wgpu::Texture) -> Result<(), PresentError> {
        if self.frame_readback.is_some() {
            return Err(PresentError::Device {
                operation: "request a frame copy while one is in flight",
            });
        }
        if !texture.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return Err(PresentError::Device {
                operation: "copy a frame texture that is not a copy source",
            });
        }
        let extent = [texture.width(), texture.height()];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot presented frame copy"),
            });
        let copy = encode_copy(
            &self.device,
            &mut encoder,
            texture,
            extent,
            FrameReadbackRoute::Surface,
            self.facts.completed_scene_id,
        )?;
        self.queue.submit([encoder.finish()]);
        self.frame_readback = Some(copy.map());
        Ok(())
    }

    /// Arms one copy taken by drawing the shade pass a second time into a copy source.
    ///
    /// This is the fallback route, for a surface that is not itself a copy source. Arming is not a
    /// copy: the second draw is appended to the presentation pass's own command encoder on the next
    /// frame submission, so the two draws cannot be separated by anything at all.
    pub const fn arm_offscreen_frame_readback(&mut self) {
        self.frame_readback_armed = true;
    }

    /// Reports whether an offscreen copy has been armed and not yet encoded.
    #[must_use]
    pub const fn offscreen_frame_readback_armed(&self) -> bool {
        self.frame_readback_armed
    }

    /// Reports whether a requested frame copy is still in flight.
    #[must_use]
    pub const fn frame_readback_pending(&self) -> bool {
        self.frame_readback.is_some()
    }

    /// Abandons an in-flight copy whose map has not completed.
    ///
    /// A map that never fires is not a slow copy but one that is never coming, and holding it costs
    /// a surface-sized buffer, refuses every later request, and keeps the loop turning for nothing.
    /// The buffer is dropped; a callback that arrives afterwards writes into storage nobody reads.
    pub fn abandon_frame_readback(&mut self) -> bool {
        self.frame_readback.take().is_some()
    }

    /// Takes the reason an armed copy was not encoded, leaving nothing behind.
    ///
    /// A capture that could not be drawn is a refusal with a cause, never a silently absent
    /// picture; the frame it was armed on was presented normally.
    pub const fn take_frame_readback_refusal(&mut self) -> Option<PresentError> {
        self.frame_readback_refusal.take()
    }

    /// Returns the requested frame copy once its map has completed.
    ///
    /// Answers `Ok(None)` while the copy is still in flight, which is the ordinary case for the
    /// turns between the request and its completion. It never answers an empty picture: a copy
    /// that failed is a typed refusal, a copy short of its own extent is a typed refusal, and a
    /// copy that has not landed is nothing at all.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal when the map failed or the assembled bytes are short of the extent;
    /// the request is retired either way, so a failed copy does not wedge the next one.
    pub fn take_frame_readback(&mut self) -> Result<Option<FrameReadback>, PresentError> {
        let Some(pending) = self.frame_readback.as_mut() else {
            return Ok(None);
        };
        self.device.poll(wgpu::Maintain::Poll);
        let result = pending.signal.lock().ok().and_then(|mut slot| slot.take());
        match result {
            None => Ok(None),
            Some(Err(())) => {
                self.frame_readback = None;
                Err(PresentError::Device {
                    operation: "map a presented frame copy",
                })
            }
            Some(Ok(())) => {
                let bytes = pending.buffer.slice(..).get_mapped_range();
                let rgba = pack_rows(&bytes, pending.extent, pending.bytes_per_row, pending.order);
                drop(bytes);
                pending.buffer.unmap();
                let extent = pending.extent;
                let route = pending.route;
                let scene_id = pending.scene_id;
                self.frame_readback = None;
                if rgba.len() != packed_bytes(extent) {
                    return Err(PresentError::Device {
                        operation: "assemble a complete frame copy",
                    });
                }
                Ok(Some(FrameReadback {
                    width: extent[0],
                    height: extent[1],
                    route,
                    scene_id,
                    rgba,
                }))
            }
        }
    }

    /// Draws the shade pass once more into a copy source and encodes the copy.
    ///
    /// Called from the warp submission, after the pass has been encoded into the surface view and
    /// before that encoder is submitted, so the second draw reads exactly the state the first one
    /// read. Nothing is scheduled, deferred or re-planned: the plan is the one the presentation
    /// call was made with, and there is no turn of the loop between the two encodes for a scene
    /// promotion or a control move to land in.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal when a copy is already in flight, when the extent is zero, when the
    /// target's extent is not the presented extent, or when the surface format cannot be copied.
    pub(super) fn encode_offscreen_capture(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        extent: [u32; 2],
    ) -> Result<EncodedFrameReadback, PresentError> {
        if self.frame_readback.is_some() {
            return Err(PresentError::Device {
                operation: "request a frame copy while one is in flight",
            });
        }
        if extent[0] == 0 || extent[1] == 0 {
            return Err(PresentError::Device {
                operation: "copy a frame of zero extent",
            });
        }
        self.ensure_readback_target(extent);
        let target = self
            .frame_readback_target
            .as_ref()
            .ok_or(PresentError::Device {
                operation: "allocate an offscreen frame copy target",
            })?;
        // The presented extent and the copied extent are the same or the copy is refused. A target
        // of another size would be a second draw of a different picture, which is the one way the
        // two encodes could disagree about anything.
        if target.extent != extent {
            return Err(PresentError::Device {
                operation: "copy a frame whose target extent is not the presented extent",
            });
        }
        encode_shade(encoder, &self.gpu, &target.view);
        encode_copy(
            &self.device,
            encoder,
            &target.texture,
            extent,
            FrameReadbackRoute::OffscreenRerender,
            self.facts.completed_scene_id,
        )
    }

    /// Retires the arming and takes ownership of an encoded copy once its commands are submitted.
    pub(super) fn retain_encoded_readback(&mut self, copy: EncodedFrameReadback) {
        self.frame_readback_armed = false;
        self.frame_readback = Some(copy.map());
    }

    /// Clears an arming whose capture could not be encoded, so the next one is not blocked.
    pub(super) const fn disarm_offscreen_frame_readback(&mut self) {
        self.frame_readback_armed = false;
    }

    fn ensure_readback_target(&mut self, extent: [u32; 2]) {
        if self
            .frame_readback_target
            .as_ref()
            .is_some_and(|target| target.extent == extent)
        {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Julibrot offscreen frame capture target"),
            size: wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // The surface's own format, so the presentation pipelines draw into it unchanged: a
            // second pipeline for a second format would be a second picture's worth of rounding.
            format: self.config.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.frame_readback_target = Some(ReadbackTarget {
            texture,
            view,
            extent,
        });
    }
}

/// Encodes one texture-to-buffer copy with padded rows into a freshly sized readback buffer.
fn encode_copy(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    extent: [u32; 2],
    route: FrameReadbackRoute,
    scene_id: Option<u64>,
) -> Result<EncodedFrameReadback, PresentError> {
    let order = channel_order(texture.format()).ok_or(PresentError::Device {
        operation: "copy a frame whose format is not eight-bit four-channel",
    })?;
    if extent[0] == 0 || extent[1] == 0 {
        return Err(PresentError::Device {
            operation: "copy a frame of zero extent",
        });
    }
    let bytes_per_row = padded_bytes_per_row(extent[0]).ok_or(PresentError::Device {
        operation: "size a frame copy row",
    })?;
    let size = u64::from(bytes_per_row)
        .checked_mul(u64::from(extent[1]))
        .ok_or(PresentError::Device {
            operation: "size a frame copy buffer",
        })?;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Julibrot presented frame readback"),
        size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(extent[1]),
            },
        },
        wgpu::Extent3d {
            width: extent[0],
            height: extent[1],
            depth_or_array_layers: 1,
        },
    );
    Ok(EncodedFrameReadback {
        buffer,
        extent,
        bytes_per_row,
        order,
        route,
        scene_id,
    })
}

/// The number of bytes a complete packed copy of this extent holds.
const fn packed_bytes(extent: [u32; 2]) -> usize {
    extent[0] as usize * extent[1] as usize * RGBA8_BYTES_PER_TEXEL as usize
}

/// The copy's row stride: the packed row rounded up to the copy alignment the API requires.
///
/// At 960 pixels the packed row is already 3,840 bytes and a whole number of 256-byte units, so the
/// lab's own frame is copied with no padding at all; the arithmetic is here for every other extent.
fn padded_bytes_per_row(width: u32) -> Option<u32> {
    let packed = width.checked_mul(RGBA8_BYTES_PER_TEXEL)?;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    packed.div_ceil(alignment).checked_mul(alignment)
}

/// Whether an eight-bit four-channel surface format needs its red and blue exchanged.
///
/// Both sRGB and linear spellings appear, because which one a browser hands the device is the
/// browser's choice; the bytes are the same eight-bit values either way, which is exactly what a
/// screenshot of the canvas shows.
const fn channel_order(format: wgpu::TextureFormat) -> Option<ChannelOrder> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            Some(ChannelOrder::Rgba)
        }
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
            Some(ChannelOrder::Bgra)
        }
        _ => None,
    }
}

/// Drops the row padding and puts the channels in RGBA order.
///
/// A row shorter than its packed width is dropped rather than half-copied: a truncated buffer is a
/// refusal to answer, not a picture with a ragged edge, and the caller checks the assembled length
/// against the extent before publishing it.
fn pack_rows(bytes: &[u8], extent: [u32; 2], bytes_per_row: u32, order: ChannelOrder) -> Vec<u8> {
    let packed = extent[0] as usize * RGBA8_BYTES_PER_TEXEL as usize;
    let stride = bytes_per_row as usize;
    let mut rgba = Vec::with_capacity(packed * extent[1] as usize);
    for row in 0..extent[1] as usize {
        let start = row * stride;
        let Some(line) = bytes.get(start..start + packed) else {
            break;
        };
        match order {
            ChannelOrder::Rgba => rgba.extend_from_slice(line),
            ChannelOrder::Bgra => {
                for texel in line.as_chunks::<{ RGBA8_BYTES_PER_TEXEL as usize }>().0 {
                    rgba.extend_from_slice(&[texel[2], texel[1], texel[0], texel[3]]);
                }
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::{
        ChannelOrder, FrameReadbackRoute, channel_order, frame_readback_route, pack_rows,
        packed_bytes, padded_bytes_per_row,
    };

    #[test]
    fn the_lab_frame_width_needs_no_padding_and_a_ragged_one_is_rounded_up() {
        assert_eq!(padded_bytes_per_row(960), Some(3_840));
        assert_eq!(padded_bytes_per_row(64), Some(256));
        assert_eq!(padded_bytes_per_row(65), Some(512));
        assert_eq!(padded_bytes_per_row(1), Some(256));
        assert_eq!(padded_bytes_per_row(u32::MAX), None);
    }

    #[test]
    fn padding_is_dropped_row_by_row_and_the_rows_stay_top_down() {
        let extent = [2, 3];
        let stride = 12_usize;
        let mut bytes = vec![0_u8; stride * 3];
        for row in 0..3_u8 {
            for texel in 0..2_u8 {
                let at = usize::from(row) * stride + usize::from(texel) * 4;
                bytes[at] = row * 10 + texel;
                bytes[at + 1] = 1;
                bytes[at + 2] = 2;
                bytes[at + 3] = 255;
            }
        }
        let packed = pack_rows(&bytes, extent, 12, ChannelOrder::Rgba);
        assert_eq!(packed.len(), packed_bytes(extent));
        assert_eq!(packed[0], 0);
        assert_eq!(packed[4], 1);
        assert_eq!(packed[8], 10);
        assert_eq!(packed[16], 20);
        assert_eq!(packed[3], 255);
    }

    #[test]
    fn a_blue_first_surface_is_returned_red_first() {
        let bytes = vec![7_u8, 8, 9, 255];
        let packed = pack_rows(&bytes, [1, 1], 4, ChannelOrder::Bgra);
        assert_eq!(packed, vec![9, 8, 7, 255]);
    }

    /// A short buffer must not become a picture. The packer stops, and the assembled length no
    /// longer matches the extent, which is what the caller refuses on rather than publishing a
    /// frame with rows nobody copied.
    #[test]
    fn a_short_buffer_ends_the_copy_and_fails_its_length_check() {
        let bytes = vec![1_u8; 4];
        let packed = pack_rows(&bytes, [1, 3], 4, ChannelOrder::Rgba);
        assert_eq!(packed.len(), 4);
        assert_ne!(packed.len(), packed_bytes([1, 3]));
        assert_eq!(packed_bytes([960, 540]), 2_073_600);
    }

    #[test]
    fn only_the_eight_bit_four_channel_surface_formats_are_copied() {
        assert_eq!(
            channel_order(wgpu::TextureFormat::Rgba8UnormSrgb),
            Some(ChannelOrder::Rgba)
        );
        assert_eq!(
            channel_order(wgpu::TextureFormat::Bgra8UnormSrgb),
            Some(ChannelOrder::Bgra)
        );
        assert_eq!(channel_order(wgpu::TextureFormat::Rgba32Float), None);
        assert_eq!(channel_order(wgpu::TextureFormat::R8Unorm), None);
    }

    /// The route is chosen from what the surface offers and from nothing else.
    ///
    /// A WebGL2 swapchain surface offers render-attachment usage alone, which is the case measured
    /// on the floor this lab targets; a surface that also offers the copy usage is copied directly,
    /// because copying the image the browser presented answers the question outright.
    #[test]
    fn the_fallback_is_selected_exactly_when_the_surface_is_not_a_copy_source() {
        assert_eq!(
            frame_readback_route(wgpu::TextureUsages::RENDER_ATTACHMENT),
            FrameReadbackRoute::OffscreenRerender
        );
        assert_eq!(
            frame_readback_route(
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
            ),
            FrameReadbackRoute::Surface
        );
        assert_eq!(
            frame_readback_route(wgpu::TextureUsages::COPY_SRC),
            FrameReadbackRoute::Surface
        );
        assert_eq!(
            frame_readback_route(wgpu::TextureUsages::empty()),
            FrameReadbackRoute::OffscreenRerender
        );
        assert_eq!(
            FrameReadbackRoute::Surface.as_str(),
            "copied from the surface"
        );
        assert_eq!(
            FrameReadbackRoute::OffscreenRerender.as_str(),
            "copied from an offscreen re-render of the shade pass"
        );
    }
}
