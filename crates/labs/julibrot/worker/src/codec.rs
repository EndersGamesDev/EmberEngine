//! Canonical dyadic centre validation and request encoding.

#![allow(
    clippy::redundant_pub_crate,
    reason = "request-layout seams are shared with a private wasm sibling but are not public ABI"
)]

use crate::compute::math_error;
use crate::wire::{
    HEADER_BYTES, MessageHeader, MessageKind, POOL_TRAILER_BYTES, Pool, WireBuffer,
    buffer_capacity, read_u32, write_words,
};
use crate::{ChannelError, ErrorCode};
use ember_julibrot_math::{PrecisionMode, ReferencePass};

const REQUEST_DEPTH_OFFSET: usize = HEADER_BYTES;
const REQUEST_REASON_OFFSET: usize = 36;
const REQUEST_CENTRE_REVISION_OFFSET: usize = 40;
const REQUEST_LIMB_COUNT_OFFSET: usize = 44;
const REQUEST_DESCRIPTORS_OFFSET: usize = 48;
const REQUEST_DESCRIPTOR_BYTES: usize = 16;
pub(crate) const REQUEST_MODE_OFFSET: usize = 112;
pub(crate) const REQUEST_FIXED_END: usize = 116;
const COORDINATE_COUNT: usize = 4;
const KNOWN_REASON_BITS: u32 = 0b1_1111;
const REFERENCE_PASS_SHIFT: u32 = 5;
const REFERENCE_PASS_MASK: u32 = 0b11 << REFERENCE_PASS_SHIFT;
const KNOWN_REQUEST_BITS: u32 = KNOWN_REASON_BITS | REFERENCE_PASS_MASK;

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
    /// Encodes math's authoritative bignum centre into canonical transport limbs.
    ///
    /// # Errors
    ///
    /// Returns `MathFailure` if a coordinate cannot be encoded, or `BadLength` if the shared limb
    /// partition cannot be represented.
    pub fn encode_math(
        centre: &ember_julibrot_math::BigCentre,
        revision: u32,
    ) -> Result<Self, ChannelError> {
        let mut limbs = Vec::new();
        let mut coordinates = [CoordinateDescriptor::default(); COORDINATE_COUNT];
        for (coordinate, value) in coordinates.iter_mut().zip(&centre.coords) {
            let encoded = ember_julibrot_math::encode_big_scalar(value)
                .map_err(|error| math_error(&error))?;
            let limb_start = u32::try_from(limbs.len())
                .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
            let limb_count = u32::try_from(encoded.limbs.len())
                .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
            *coordinate = CoordinateDescriptor {
                sign: encoded.sign,
                exponent_twos_complement: encoded.exponent.cast_unsigned(),
                limb_start,
                limb_count,
            };
            limbs.extend_from_slice(&encoded.limbs);
        }
        let encoded = Self {
            revision,
            coordinates,
            limbs,
        };
        encoded.validate()?;
        Ok(encoded)
    }

    /// Decodes canonical transport limbs into math's authoritative bignum centre.
    ///
    /// # Errors
    ///
    /// Returns a canonical-validation refusal or `MathFailure` for invalid bignum input.
    pub fn decode_math(
        &self,
        precision_bits: u32,
    ) -> Result<ember_julibrot_math::BigCentre, ChannelError> {
        self.validate()?;
        let mut values = Vec::with_capacity(COORDINATE_COUNT);
        for descriptor in self.coordinates {
            let start = usize::try_from(descriptor.limb_start)
                .map_err(|_| bad_descriptor(descriptor.limb_start))?;
            let count = usize::try_from(descriptor.limb_count)
                .map_err(|_| bad_descriptor(descriptor.limb_count))?;
            let end = start
                .checked_add(count)
                .ok_or_else(|| bad_descriptor(descriptor.limb_count))?;
            values.push(
                ember_julibrot_math::decode_big_scalar(
                    descriptor.sign,
                    descriptor.exponent_twos_complement.cast_signed(),
                    &self.limbs[start..end],
                    precision_bits,
                )
                .map_err(|error| math_error(&error))?,
            );
        }
        let delivered_precision = values[0].precision_bits();
        if values
            .iter()
            .any(|value| value.precision_bits() != delivered_precision)
        {
            return Err(math_error(
                &ember_julibrot_math::MathError::PrecisionMismatch,
            ));
        }
        let [a, b, c, d] = values
            .try_into()
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, 4, 0))?;
        Ok(ember_julibrot_math::BigCentre {
            coords: [a, b, c, d],
            precision_bits: delivered_precision,
        })
    }

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
            let high_index =
                usize::try_from(end - 1).map_err(|_| bad_descriptor(descriptor.limb_count))?;
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
    /// Requested precision policy changed.
    pub const PRECISION_MODE_CHANGE: Self = Self(1 << 4);

    /// Validates and constructs version-two reason bits.
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
    generation: u32,
    /// Canonical authoritative centre.
    centre: EncodedCentre,
    /// Integral decimal depth label.
    depth_digits: u32,
    /// Requested bignum precision.
    precision_bits: u32,
    /// Requested orbit-entry cap.
    max_iter: u32,
    /// Requested precision policy.
    precision_mode: PrecisionMode,
    /// Triggering policy reason or reasons.
    reason: OrbitReason,
    /// Reference pass selected by Preview, Final, or Measure.
    reference_pass: ReferencePass,
}

impl OrbitRequest {
    /// Builds a canonical request that fits its orbit-sized transport buffer.
    ///
    /// # Errors
    ///
    /// Returns `BadLength` for zero scalar fields or `CentreEncodingWall` when the canonical
    /// centre does not fit the buffer capacity implied by `max_iter`.
    pub fn new(
        generation: u32,
        centre: EncodedCentre,
        depth_digits: u32,
        precision_bits: u32,
        max_iter: u32,
        precision_mode: PrecisionMode,
        reason: OrbitReason,
    ) -> Result<Self, ChannelError> {
        if generation == 0 || precision_bits == 0 || max_iter == 0 {
            return Err(ChannelError::new(ErrorCode::BadLength, 0, 0, 0));
        }
        let requested = centre.request_bytes()?;
        let available = buffer_capacity(max_iter)? - POOL_TRAILER_BYTES;
        if requested > available {
            return Err(ChannelError::new(
                ErrorCode::CentreEncodingWall,
                u32::try_from(centre.limbs.len()).unwrap_or(u32::MAX),
                u32::try_from(requested).unwrap_or(u32::MAX),
                u32::try_from(available).unwrap_or(u32::MAX),
            ));
        }
        Ok(Self {
            generation,
            centre,
            depth_digits,
            precision_bits,
            max_iter,
            precision_mode,
            reason,
            reference_pass: match precision_mode {
                PrecisionMode::Deterministic => ReferencePass::Final,
                PrecisionMode::PictureFast => ReferencePass::Preview,
            },
        })
    }

    /// Selects the PictureFast stage policy carried in the request body.
    #[must_use]
    pub const fn with_precision_policy(
        mut self,
        precision_mode: PrecisionMode,
        reference_pass: ReferencePass,
    ) -> Self {
        self.precision_mode = precision_mode;
        self.reference_pass = match precision_mode {
            PrecisionMode::Deterministic => ReferencePass::Final,
            PrecisionMode::PictureFast => reference_pass,
        };
        self
    }

    /// Returns the checked request generation.
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// Borrows the canonical encoded centre.
    #[must_use]
    pub const fn centre(&self) -> &EncodedCentre {
        &self.centre
    }

    /// Returns the integral decimal-depth label.
    #[must_use]
    pub const fn depth_digits(&self) -> u32 {
        self.depth_digits
    }

    /// Returns requested bignum precision in bits.
    #[must_use]
    pub const fn precision_bits(&self) -> u32 {
        self.precision_bits
    }

    /// Returns requested orbit length.
    #[must_use]
    pub const fn max_iter(&self) -> u32 {
        self.max_iter
    }

    /// Returns the policy reasons that triggered this request.
    #[must_use]
    pub const fn reason(&self) -> OrbitReason {
        self.reason
    }

    /// Returns the transported precision policy.
    #[must_use]
    pub const fn precision_mode(&self) -> PrecisionMode {
        self.precision_mode
    }

    /// Returns the transported Preview, Final, or Measure pass.
    #[must_use]
    pub const fn reference_pass(&self) -> ReferencePass {
        self.reference_pass
    }

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
        visit_transfer_request_body_words(self, |offset, words| {
            let end = offset + words.len() * 4;
            write_words(&mut message[offset..end], words);
            Ok(())
        })
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
            precision_mode: view.precision_mode,
            reason: view.reason,
            reference_pass: view.reference_pass,
        })
    }
}

/// Visits the request-body word runs emitted by the browser transfer writer.
///
/// # Errors
///
/// Returns `BadLength` if the canonical limb count cannot be represented on the wire, or forwards
/// a refusal from the supplied writer.
pub(crate) fn visit_transfer_request_body_words(
    request: &OrbitRequest,
    mut write: impl FnMut(usize, &[u32]) -> Result<(), ChannelError>,
) -> Result<(), ChannelError> {
    let limb_count = u32::try_from(request.centre.limbs.len())
        .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
    write(
        REQUEST_DEPTH_OFFSET,
        &[
            request.depth_digits,
            encode_request_bits(
                request.reason,
                request.precision_mode,
                request.reference_pass,
            ),
            request.centre.revision,
            limb_count,
        ],
    )?;
    for (index, descriptor) in request.centre.coordinates.iter().enumerate() {
        write(
            REQUEST_DESCRIPTORS_OFFSET + index * REQUEST_DESCRIPTOR_BYTES,
            &[
                descriptor.sign,
                descriptor.exponent_twos_complement,
                descriptor.limb_start,
                descriptor.limb_count,
            ],
        )?;
    }
    write(REQUEST_MODE_OFFSET, &[request.precision_mode as u32])?;
    write(REQUEST_FIXED_END, &request.centre.limbs)
}

/// Allocation-free borrowed view of a validated request body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestBodyView {
    /// Request generation.
    pub(crate) generation: u32,
    /// Decimal-depth label.
    pub(crate) depth_digits: u32,
    /// Requested precision.
    pub(crate) precision_bits: u32,
    /// Requested orbit cap.
    pub(crate) max_iter: u32,
    /// Requested precision policy.
    pub(crate) precision_mode: PrecisionMode,
    /// Triggering reasons.
    pub(crate) reason: OrbitReason,
    /// Transported Preview, Final, or Measure pass.
    pub(crate) reference_pass: ReferencePass,
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
                u32::try_from(REQUEST_FIXED_END).unwrap_or(u32::MAX),
                u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            ));
        }
        let limb_count = usize::try_from(read_u32(bytes, REQUEST_LIMB_COUNT_OFFSET))
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
            let offset = REQUEST_DESCRIPTORS_OFFSET + index * REQUEST_DESCRIPTOR_BYTES;
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
            revision: read_u32(bytes, REQUEST_CENTRE_REVISION_OFFSET),
            coordinates,
            limbs,
        };
        centre.validate()?;
        let precision_mode = PrecisionMode::from_u32(read_u32(bytes, REQUEST_MODE_OFFSET))
            .ok_or_else(|| {
                ChannelError::new(
                    ErrorCode::BadLength,
                    read_u32(bytes, REQUEST_MODE_OFFSET),
                    0,
                    0,
                )
            })?;
        let (reason, reference_pass) =
            decode_request_bits(read_u32(bytes, REQUEST_REASON_OFFSET), precision_mode)?;
        Ok(Self {
            generation: header.generation,
            depth_digits: read_u32(bytes, REQUEST_DEPTH_OFFSET),
            precision_bits: header.precision_bits,
            max_iter: header.length,
            precision_mode,
            reason,
            reference_pass,
            centre_revision: centre.revision,
            coordinates,
            limbs: centre.limbs,
        })
    }
}

const fn encode_request_bits(
    reason: OrbitReason,
    precision_mode: PrecisionMode,
    reference_pass: ReferencePass,
) -> u32 {
    match precision_mode {
        PrecisionMode::Deterministic => reason.bits(),
        PrecisionMode::PictureFast => reason.bits() | ((reference_pass as u32) << REFERENCE_PASS_SHIFT),
    }
}

const fn decode_request_bits(
    bits: u32,
    precision_mode: PrecisionMode,
) -> Result<(OrbitReason, ReferencePass), ChannelError> {
    if bits & !KNOWN_REQUEST_BITS != 0 {
        return Err(ChannelError::new(ErrorCode::BadLength, bits, 0, 0));
    }
    let reason = OrbitReason::from_bits(bits & KNOWN_REASON_BITS)?;
    if precision_mode == PrecisionMode::Deterministic {
        if bits & REFERENCE_PASS_MASK != 0 {
            return Err(ChannelError::new(ErrorCode::BadLength, bits, 0, 0));
        }
        return Ok((reason, ReferencePass::Final));
    }
    let pass = match (bits & REFERENCE_PASS_MASK) >> REFERENCE_PASS_SHIFT {
        0 => ReferencePass::Preview,
        1 => ReferencePass::Final,
        2 => ReferencePass::Measure,
        _ => return Err(ChannelError::new(ErrorCode::BadLength, bits, 0, 0)),
    };
    Ok((reason, pass))
}

const fn bad_descriptor(detail: u32) -> ChannelError {
    ChannelError::new(ErrorCode::BadLength, detail, 0, 0)
}

#[cfg(test)]
mod tests {
    use ember_julibrot_math::{PrecisionMode, ReferencePass};

    use super::{
        CoordinateDescriptor, EncodedCentre, OrbitReason, OrbitRequest, REQUEST_FIXED_END,
        REQUEST_MODE_OFFSET, visit_transfer_request_body_words,
    };
    use crate::wire::{WireBuffer, write_words};
    use crate::{ErrorCode, Pool};

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

    fn deep_nonzero_request(mode: PrecisionMode) -> OrbitRequest {
        let centre =
            ember_julibrot_math::BigCentre::from_f64([0.25, -0.125, -0.75, 0.125], 1_024).unwrap();
        OrbitRequest::new(
            31,
            EncodedCentre::encode_math(&centre, 29).unwrap(),
            300,
            1_024,
            64,
            mode,
            OrbitReason::CENTRE_THRESHOLD,
        )
        .unwrap()
    }

    #[test]
    fn canonical_request_round_trips_little_endian() {
        let request = OrbitRequest::new(
            23,
            centre(),
            101,
            384,
            64,
            PrecisionMode::PictureFast,
            OrbitReason::INITIAL.union(OrbitReason::ZOOM_THRESHOLD),
        )
        .unwrap();
        let mut buffer = WireBuffer::new(Pool::Request, 1, 64).unwrap();
        let trailer_before = buffer.trailer().unwrap();
        request.encode_into(&mut buffer).unwrap();
        assert_eq!(OrbitRequest::decode(&buffer).unwrap(), request);
        assert_eq!(buffer.trailer().unwrap(), trailer_before);
        assert_eq!(
            &buffer.as_bytes()[32..48],
            &[101, 0, 0, 0, 5, 0, 0, 0, 17, 0, 0, 0, 4, 0, 0, 0]
        );
        assert_eq!(&buffer.as_bytes()[112..116], &1_u32.to_le_bytes());
        assert_eq!(&buffer.as_bytes()[116..120], &0x0123_4567_u32.to_le_bytes());

        let picture = request
            .with_precision_policy(PrecisionMode::PictureFast, ReferencePass::Measure);
        picture.encode_into(&mut buffer).unwrap();
        let decoded = OrbitRequest::decode(&buffer).unwrap();
        assert_eq!(decoded, picture);
        assert_eq!(decoded.precision_mode(), PrecisionMode::PictureFast);
        assert_eq!(decoded.reference_pass(), ReferencePass::Measure);
        assert_eq!(u32::from_le_bytes(buffer.as_bytes()[36..40].try_into().unwrap()), 69);
    }

    #[test]
    fn browser_transfer_body_matches_codec_for_a_deep_nonzero_centre() {
        let request = deep_nonzero_request(PrecisionMode::PictureFast);
        let mut codec_buffer = WireBuffer::new(Pool::Request, 1, request.max_iter()).unwrap();
        request.encode_into(&mut codec_buffer).unwrap();

        let message_end = codec_buffer.capacity() - crate::POOL_TRAILER_BYTES;
        let mut browser_body = vec![0_u8; message_end];
        visit_transfer_request_body_words(&request, |offset, words| {
            let end = offset + words.len() * 4;
            write_words(&mut browser_body[offset..end], words);
            Ok(())
        })
        .unwrap();

        assert_eq!(
            &browser_body[crate::HEADER_BYTES..],
            &codec_buffer.as_bytes()[crate::HEADER_BYTES..message_end]
        );
        assert_eq!(
            &browser_body[REQUEST_MODE_OFFSET..REQUEST_FIXED_END],
            &(PrecisionMode::PictureFast as u32).to_le_bytes()
        );
        assert_eq!(
            &browser_body[REQUEST_FIXED_END..REQUEST_FIXED_END + 4],
            &request.centre().limbs[0].to_le_bytes()
        );
    }

    #[test]
    fn nonzero_origin_request_round_trips_with_its_precision_mode() {
        for mode in PrecisionMode::ALL {
            let request = deep_nonzero_request(mode);
            let mut buffer = WireBuffer::new(Pool::Request, 0, request.max_iter()).unwrap();
            request.encode_into(&mut buffer).unwrap();
            let decoded = OrbitRequest::decode(&buffer).unwrap();

            assert_eq!(decoded.precision_mode(), mode);
            assert_eq!(decoded, request);
            assert_eq!(
                decoded.centre().decode_math(1_024).unwrap().to_f64_mirror(),
                [0.25, -0.125, -0.75, 0.125]
            );
        }
    }

    #[test]
    fn canonical_validation_rejects_every_partition_defect() {
        let valid = centre();
        assert_eq!(valid.validate(), Ok(()));

        let mut negative_zero = valid.clone();
        negative_zero.coordinates[2].sign = 1;
        assert_eq!(
            negative_zero.validate().unwrap_err().code,
            ErrorCode::BadLength
        );

        let mut gap = valid.clone();
        gap.coordinates[1].limb_start = 3;
        assert_eq!(gap.validate().unwrap_err().code, ErrorCode::BadLength);

        let mut leading_zero = valid.clone();
        leading_zero.limbs[1] = 0;
        assert_eq!(
            leading_zero.validate().unwrap_err().code,
            ErrorCode::BadLength
        );

        let mut unused = valid;
        unused.limbs.push(1);
        assert_eq!(unused.validate().unwrap_err().code, ErrorCode::BadLength);
    }

    #[test]
    fn full_mantissa_anchor_navigation_decodes_at_one_precision() {
        let plane = ember_julibrot_math::construct_plane(ember_julibrot_math::PlaneAngles {
            theta_1: 0.0,
            theta_2: 0.0,
        })
        .unwrap();
        // The navigator holds 1,024 bits while a shallow request declares the plan's 47.
        let mut centre = ember_julibrot_math::BigCentre::from_f64([0.0; 4], 1_024).unwrap();
        // Canvas-relative CSS pixels scaled by 960/1022.794: full 53-bit mantissas, not the short
        // dyadics an integer-pixel anchor produced.
        let pixel_ratio = 960.0_f64 / 1_022.794_f64;
        let anchor = [173.0_f64 * pixel_ratio, -91.0_f64 * pixel_ratio];
        let mut zoom_log2 = 0.0_f64;
        for tick in 1..=4_u32 {
            let after = zoom_log2 + 0.2;
            centre
                .apply_navigation(
                    &ember_julibrot_math::NavigationDelta {
                        pan_canvas_px: [0.0; 2],
                        zoom_delta_log2: 0.2,
                        anchor_canvas_px: anchor,
                    },
                    &plane,
                    zoom_log2,
                    after,
                    960,
                )
                .unwrap();
            zoom_log2 = after;
            let encoded = EncodedCentre::encode_math(&centre, tick).unwrap();
            for mode in PrecisionMode::ALL {
                let decoded = encoded.decode_math(47).unwrap();
                assert!(decoded.precision_bits >= 47, "tick {tick}");
                assert!(
                    decoded
                        .coords
                        .iter()
                        .all(|coordinate| coordinate.precision_bits() == decoded.precision_bits),
                    "tick {tick} delivered mixed coordinate precisions"
                );
                if mode.requires_bit_identity() {
                    // Deterministic-only contract: astro-float's word rounding is identical.
                    assert_eq!(decoded.precision_bits, 64, "tick {tick}");
                }
            }
        }
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
        let request = OrbitRequest::new(
            1,
            EncodedCentre {
                revision: 1,
                coordinates,
                limbs: vec![u32::MAX; 128],
            },
            300,
            1_024,
            64,
            PrecisionMode::Deterministic,
            OrbitReason::INITIAL,
        )
        .unwrap();
        let mut buffer = WireBuffer::new(Pool::Request, 0, 64).unwrap();
        request.encode_into(&mut buffer).unwrap();
        assert_eq!(request.centre.request_bytes().unwrap(), 628);
        assert_eq!(buffer.capacity(), 1_088);
    }
}
