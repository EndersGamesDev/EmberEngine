//! Canonical dyadic centre validation and request encoding.

use crate::wire::{HEADER_BYTES, POOL_TRAILER_BYTES, read_u32, write_words};
use crate::wire::{MessageHeader, MessageKind, Pool, WireBuffer};
use crate::{ChannelError, ErrorCode};

const REQUEST_FIXED_END: usize = 112;
const COORDINATE_COUNT: usize = 4;
const KNOWN_REASON_BITS: u32 = 0b1111;

/// One canonical dyadic coordinate descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct CoordinateDescriptor {
    /// Zero for positive and one for negative.
    pub sign: u32,
    /// Bit pattern of the signed base-two exponent.
    pub exponent_twos_complement: u32,
    /// First little-endian limb in the shared limb array.
    pub limb_start: u32,
    /// Number of limbs, or zero for canonical zero.
    pub limb_count: u32,
}

/// Library-independent canonical encoding of four coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedCentre {
    /// Monotonic authoritative-centre revision.
    pub revision: u32,
    /// Coordinate descriptors in `(z.re,z.im,c.re,c.im)` order.
    pub coordinates: [CoordinateDescriptor; COORDINATE_COUNT],
    /// Shared little-endian base-2^32 limbs.
    pub limbs: Vec<u32>,
}

impl EncodedCentre {
    /// Validates canonical zeroes and a contiguous exhaustive limb partition.
    ///
    /// # Errors
    ///
    /// Returns `BadLength` for any non-canonical descriptor or limb partition.
    pub fn validate(&self) -> Result<(), ChannelError> {
        let mut previous_end = 0_u32;
        for descriptor in self.coordinates {
            if descriptor.sign > 1 || descriptor.limb_start != previous_end {
                return Err(bad_descriptor(descriptor.limb_start));
            }
            if descriptor.limb_count == 0 {
                if descriptor.sign != 0 || descriptor.exponent_twos_complement != 0 {
                    return Err(bad_descriptor(descriptor.sign));
                }
                continue;
            }
            let end = descriptor
                .limb_start
                .checked_add(descriptor.limb_count)
                .ok_or_else(|| bad_descriptor(descriptor.limb_count))?;
            let high_index = usize::try_from(end - 1)
                .map_err(|_| bad_descriptor(descriptor.limb_count))?;
            if self.limbs.get(high_index).copied().unwrap_or(0) == 0 {
                return Err(bad_descriptor(end));
            }
            previous_end = end;
        }
        if usize::try_from(previous_end).ok() != Some(self.limbs.len()) {
            return Err(bad_descriptor(previous_end));
        }
        Ok(())
    }

    /// Returns exact bytes used through the final limb.
    ///
    /// # Errors
    ///
    /// Returns the canonical-validation refusal or `BadLength` on overflow.
    pub fn request_bytes(&self) -> Result<usize, ChannelError> {
        self.validate()?;
        self.limbs
            .len()
            .checked_mul(4)
            .and_then(|bytes| REQUEST_FIXED_END.checked_add(bytes))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))
    }
}

/// Why the owner requested a new reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrbitReason(u32);

impl OrbitReason {
    /// Initial reference for a new session or preset.
    pub const INITIAL: Self = Self(1 << 0);
    /// Desired centre crossed the displacement threshold.
    pub const CENTRE_THRESHOLD: Self = Self(1 << 1);
    /// Zoom crossed the reference threshold.
    pub const ZOOM_THRESHOLD: Self = Self(1 << 2);
    /// Requested maximum iteration changed.
    pub const MAX_ITER_CHANGE: Self = Self(1 << 3);

    /// Validates and constructs version-one reason bits.
    ///
    /// # Errors
    ///
    /// Returns `BadLength` for zero or unknown bits.
    pub const fn from_bits(bits: u32) -> Result<Self, ChannelError> {
        if bits == 0 || bits & !KNOWN_REASON_BITS != 0 {
            return Err(ChannelError::new(ErrorCode::BadLength, bits, 0, 0));
        }
        Ok(Self(bits))
    }

    /// Returns the canonical bit set.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Combines independently applicable reasons.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// One semantic reference-orbit request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrbitRequest {
    /// Checked request generation.
    pub generation: u32,
    /// Canonical authoritative centre.
    pub centre: EncodedCentre,
    /// Integral decimal depth label.
    pub depth_digits: u32,
    /// Requested bignum precision.
    pub precision_bits: u32,
    /// Requested orbit-entry cap.
    pub max_iter: u32,
    /// Triggering policy reason or reasons.
    pub reason: OrbitReason,
}

impl OrbitRequest {
    /// Encodes this request into one request-pool buffer without resizing it.
    pub(crate) fn encode_into(&self, buffer: &mut WireBuffer) -> Result<(), ChannelError> {
        let (pool, _) = buffer.identity()?;
        if pool != Pool::Request {
            return Err(ChannelError::new(ErrorCode::BadKind, pool as u32, 0, 0));
        }
        let requested = self.centre.request_bytes()?;
        let available = buffer.capacity() - POOL_TRAILER_BYTES;
        if requested > available {
            return Err(ChannelError::new(
                ErrorCode::CentreEncodingWall,
                u32::try_from(self.centre.limbs.len()).unwrap_or(u32::MAX),
                u32::try_from(requested).unwrap_or(u32::MAX),
                u32::try_from(available).unwrap_or(u32::MAX),
            ));
        }
        let mut header = MessageHeader::new(MessageKind::OrbitRequest, self.generation);
        header.length = self.max_iter;
        header.precision_bits = self.precision_bits;
        buffer.write_header(header)?;
        let message = buffer.message_bytes_mut();
        write_words(
            &mut message[HEADER_BYTES..48],
            &[
                self.depth_digits,
                self.reason.bits(),
                self.centre.revision,
                u32::try_from(self.centre.limbs.len()).map_err(|_| {
                    ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0)
                })?,
            ],
        );
        for (index, descriptor) in self.centre.coordinates.iter().enumerate() {
            let offset = 48 + index * 16;
            write_words(
                &mut message[offset..offset + 16],
                &[
                    descriptor.sign,
                    descriptor.exponent_twos_complement,
                    descriptor.limb_start,
                    descriptor.limb_count,
                ],
            );
        }
        write_words(&mut message[REQUEST_FIXED_END..requested], &self.centre.limbs);
        Ok(())
    }

    /// Decodes and validates an owned semantic request.
    pub(crate) fn decode(buffer: &WireBuffer) -> Result<Self, ChannelError> {
        let view = RequestBodyView::decode(buffer)?;
        Ok(Self {
            generation: view.generation,
            centre: EncodedCentre {
                revision: view.centre_revision,
                coordinates: view.coordinates,
                limbs: view.limbs,
            },
            depth_digits: view.depth_digits,
            precision_bits: view.precision_bits,
            max_iter: view.max_iter,
            reason: view.reason,
        })
    }
}

/// Allocation-free borrowed view of a validated request body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RequestBodyView {
    /// Request generation.
    pub(crate) generation: u32,
    /// Decimal-depth label.
    pub(crate) depth_digits: u32,
    /// Requested precision.
    pub(crate) precision_bits: u32,
    /// Requested orbit cap.
    pub(crate) max_iter: u32,
    /// Triggering reasons.
    pub(crate) reason: OrbitReason,
    /// Authoritative-centre revision.
    pub(crate) centre_revision: u32,
    /// Four canonical coordinate descriptors.
    pub(crate) coordinates: [CoordinateDescriptor; COORDINATE_COUNT],
    /// Shared canonical limbs.
    pub(crate) limbs: Vec<u32>,
}

impl RequestBodyView {
    /// Decodes and validates a request body.
    pub(crate) fn decode(buffer: &WireBuffer) -> Result<Self, ChannelError> {
        let (pool, _) = buffer.identity()?;
        let header = buffer.header()?;
        let kind = header.validate()?;
        if pool != Pool::Request || kind != MessageKind::OrbitRequest || header.length == 0 {
            return Err(ChannelError::new(ErrorCode::BadKind, header.kind, 0, 0));
        }
        let bytes = buffer.as_bytes();
        if bytes.len() < REQUEST_FIXED_END + POOL_TRAILER_BYTES {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                0,
                112,
                u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            ));
        }
        let limb_count = usize::try_from(read_u32(bytes, 44))
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, 0, 0))?;
        let used = limb_count
            .checked_mul(4)
            .and_then(|limb_bytes| REQUEST_FIXED_END.checked_add(limb_bytes))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        let available = bytes.len() - POOL_TRAILER_BYTES;
        if used > available || bytes[used..available].iter().any(|byte| *byte != 0) {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                0,
                u32::try_from(used).unwrap_or(u32::MAX),
                u32::try_from(available).unwrap_or(u32::MAX),
            ));
        }
        let mut coordinates = [CoordinateDescriptor::default(); COORDINATE_COUNT];
        for (index, descriptor) in coordinates.iter_mut().enumerate() {
            let offset = 48 + index * 16;
            *descriptor = CoordinateDescriptor {
                sign: read_u32(bytes, offset),
                exponent_twos_complement: read_u32(bytes, offset + 4),
                limb_start: read_u32(bytes, offset + 8),
                limb_count: read_u32(bytes, offset + 12),
            };
        }
        let limbs = (REQUEST_FIXED_END..used)
            .step_by(4)
            .map(|offset| read_u32(bytes, offset))
            .collect::<Vec<_>>();
        let centre = EncodedCentre {
            revision: read_u32(bytes, 40),
            coordinates,
            limbs,
        };
        centre.validate()?;
        Ok(Self {
            generation: header.generation,
            depth_digits: read_u32(bytes, 32),
            precision_bits: header.precision_bits,
            max_iter: header.length,
            reason: OrbitReason::from_bits(read_u32(bytes, 36))?,
            centre_revision: centre.revision,
            coordinates,
            limbs: centre.limbs,
        })
    }
}

const fn bad_descriptor(detail: u32) -> ChannelError {
    ChannelError::new(ErrorCode::BadLength, detail, 0, 0)
}


#[cfg(test)]
mod tests {
    use super::{CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest};
    use crate::{ErrorCode, Pool, WireBuffer};

    fn centre() -> EncodedCentre {
        EncodedCentre {
            revision: 17,
            coordinates: [
                CoordinateDescriptor {
                    sign: 0,
                    exponent_twos_complement: (-7_i32).cast_unsigned(),
                    limb_start: 0,
                    limb_count: 2,
                },
                CoordinateDescriptor {
                    sign: 1,
                    exponent_twos_complement: 3,
                    limb_start: 2,
                    limb_count: 1,
                },
                CoordinateDescriptor {
                    sign: 0,
                    exponent_twos_complement: 0,
                    limb_start: 3,
                    limb_count: 0,
                },
                CoordinateDescriptor {
                    sign: 0,
                    exponent_twos_complement: (-2_i32).cast_unsigned(),
                    limb_start: 3,
                    limb_count: 1,
                },
            ],
            limbs: vec![0x0123_4567, 0x89ab_cdef, 5, 9],
        }
    }

    #[test]
    fn canonical_request_round_trips_little_endian() {
        let request = OrbitRequest {
            generation: 23,
            centre: centre(),
            depth_digits: 101,
            precision_bits: 384,
            max_iter: 64,
            reason: OrbitReason::INITIAL.union(OrbitReason::ZOOM_THRESHOLD),
        };
        let mut buffer = WireBuffer::new(Pool::Request, 1, 64).unwrap();
        let trailer_before = buffer.trailer().unwrap();
        request.encode_into(&mut buffer).unwrap();
        assert_eq!(OrbitRequest::decode(&buffer).unwrap(), request);
        assert_eq!(buffer.trailer().unwrap(), trailer_before);
        assert_eq!(&buffer.as_bytes()[32..48], &[101, 0, 0, 0, 5, 0, 0, 0, 17, 0, 0, 0, 4, 0, 0, 0]);
        assert_eq!(&buffer.as_bytes()[112..116], &0x0123_4567_u32.to_le_bytes());
    }

    #[test]
    fn canonical_validation_rejects_every_partition_defect() {
        let valid = centre();
        assert_eq!(valid.validate(), Ok(()));

        let mut negative_zero = valid.clone();
        negative_zero.coordinates[2].sign = 1;
        assert_eq!(negative_zero.validate().unwrap_err().code, ErrorCode::BadLength);

        let mut gap = valid.clone();
        gap.coordinates[1].limb_start = 3;
        assert_eq!(gap.validate().unwrap_err().code, ErrorCode::BadLength);

        let mut leading_zero = valid.clone();
        leading_zero.limbs[1] = 0;
        assert_eq!(leading_zero.validate().unwrap_err().code, ErrorCode::BadLength);

        let mut unused = valid;
        unused.limbs.push(1);
        assert_eq!(unused.validate().unwrap_err().code, ErrorCode::BadLength);
    }

    #[test]
    fn max_iter_sixty_four_holds_the_policy_limit_fixture() {
        let mut start = 0_u32;
        let coordinates = std::array::from_fn(|_| {
            let descriptor = CoordinateDescriptor {
                sign: 0,
                exponent_twos_complement: (-997_i32).cast_unsigned(),
                limb_start: start,
                limb_count: 32,
            };
            start += 32;
            descriptor
        });
        let request = OrbitRequest {
            generation: 1,
            centre: EncodedCentre {
                revision: 1,
                coordinates,
                limbs: vec![u32::MAX; 128],
            },
            depth_digits: 300,
            precision_bits: 1_024,
            max_iter: 64,
            reason: OrbitReason::INITIAL,
        };
        let mut buffer = WireBuffer::new(Pool::Request, 0, 64).unwrap();
        request.encode_into(&mut buffer).unwrap();
        assert_eq!(request.centre.request_bytes().unwrap(), 624);
        assert_eq!(buffer.capacity(), 1_072);
    }
}
