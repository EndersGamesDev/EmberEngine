//! One requested copy of a presented frame, taken where the pass already owns the frame texture.
//!
//! A picture that can only be read back through a context flag set in a private copy of the page is
//! a picture measured on an edited page, and the edit is exactly the sort of thing a proof is meant
//! to exclude. The copy below needs no such edit: the frame texture the warp pass drew into is
//! copied into a mapped buffer inside the renderer, on request only, and never per frame. The cost
//! is stated rather than hidden — one full copy of the surface, four bytes a pixel, so 2,073,600
//! bytes at 960 by 540, plus the buffer's own allocation for as long as the copy is in flight.
//!
//! Rows come back top-down, in the order the texture holds them, packed and in RGBA order whatever
//! the surface's own channel order is: a caller counting colours must not have to know which of the
//! two eight-bit surface formats the browser handed the device.

use std::sync::{Arc, Mutex};

use crate::PresentError;

use super::{MapSignal, Presenter, RGBA8_BYTES_PER_TEXEL};

/// One copy of a presented frame: packed RGBA, four bytes a pixel, rows top-down.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameReadback {
    /// Copied width in pixels.
    pub width: u32,
    /// Copied height in pixels.
    pub height: u32,
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

pub(super) struct PendingFrameReadback {
    buffer: wgpu::Buffer,
    extent: [u32; 2],
    bytes_per_row: u32,
    order: ChannelOrder,
    signal: MapSignal,
}

impl Presenter {
    /// Copies one presented frame texture into a mapped buffer, on request only.
    ///
    /// The texture is the image the pass just drew and the surface is about to present, so what is
    /// read back is the frame itself rather than a second render of the same inputs. The copy is
    /// submitted here and completes on a later turn; [`Self::take_frame_readback`] returns it.
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
        let order = channel_order(texture.format()).ok_or(PresentError::Device {
            operation: "copy a frame whose format is not eight-bit four-channel",
        })?;
        let extent = [texture.width(), texture.height()];
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
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Julibrot presented frame readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Julibrot presented frame copy"),
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
        self.queue.submit([encoder.finish()]);
        let signal: MapSignal = Arc::new(Mutex::new(None));
        let callback = Arc::clone(&signal);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if let Ok(mut slot) = callback.lock() {
                    *slot = Some(result.map_err(|_| ()));
                }
            });
        self.frame_readback = Some(PendingFrameReadback {
            buffer,
            extent,
            bytes_per_row,
            order,
            signal,
        });
        Ok(())
    }

    /// Reports whether a requested frame copy is still in flight.
    #[must_use]
    pub const fn frame_readback_pending(&self) -> bool {
        self.frame_readback.is_some()
    }

    /// Returns the requested frame copy once its map has completed.
    ///
    /// Answers `Ok(None)` while the copy is still in flight, which is the ordinary case for the
    /// turns between the request and its completion.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal when the map itself failed; the request is retired either way, so a
    /// failed copy does not wedge the next one.
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
                self.frame_readback = None;
                Ok(Some(FrameReadback {
                    width: extent[0],
                    height: extent[1],
                    rgba,
                }))
            }
        }
    }
}

/// The copy's row stride: the packed row rounded up to the copy alignment the API requires.
///
/// At 960 pixels the packed row is already 3,840 bytes and a whole number of 256-byte units, so the
/// lab's own frame is copied with no padding at all; the arithmetic is here for every other extent.
pub(super) const fn padded_bytes_per_row(width: u32) -> Option<u32> {
    let Some(packed) = width.checked_mul(RGBA8_BYTES_PER_TEXEL) else {
        return None;
    };
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let units = packed.div_ceil(alignment);
    units.checked_mul(alignment)
}

/// Whether an eight-bit four-channel surface format needs its red and blue exchanged.
///
/// Both sRGB and linear spellings appear, because which one a browser hands the device is the
/// browser's choice; the bytes are the same eight-bit values either way, which is exactly what a
/// screenshot of the canvas shows.
pub(super) const fn channel_order(format: wgpu::TextureFormat) -> Option<ChannelOrder> {
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
/// refusal to answer, not a picture with a ragged edge.
pub(super) fn pack_rows(
    bytes: &[u8],
    extent: [u32; 2],
    bytes_per_row: u32,
    order: ChannelOrder,
) -> Vec<u8> {
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
    use super::{ChannelOrder, channel_order, pack_rows, padded_bytes_per_row};

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
        assert_eq!(packed.len(), 2 * 3 * 4);
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

    #[test]
    fn a_short_buffer_ends_the_copy_rather_than_inventing_a_row() {
        let bytes = vec![1_u8; 4];
        let packed = pack_rows(&bytes, [1, 3], 4, ChannelOrder::Rgba);
        assert_eq!(packed.len(), 4);
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
}
