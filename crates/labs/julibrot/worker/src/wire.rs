//! Fixed-layout little-endian wire records and owned pool buffers.

use crate::{ChannelError, ComputedOrbit, ErrorCode, ReferenceOrbitRecord};
use ember_julibrot_math::ReferenceVerification;

/// Exported module and wire ABI version.
pub const JULIBROT_ABI_VERSION: u32 = 3;
/// Little-endian byte string `JBL1`.
pub const MAGIC: u32 = 0x314c_424a;
/// Little-endian pool-trailer byte string `JBLT`.
pub const TRAILER_MAGIC: u32 = 0x544c_424a;
/// Header size in bytes.
pub const HEADER_BYTES: usize = 32;
/// Pool-trailer size in bytes.
pub const POOL_TRAILER_BYTES: usize = 16;
/// Verification-fact tail size in bytes.
pub const ORBIT_FACT_BYTES: usize = 16;
/// Per-buffer bytes outside orbit-record capacity.
pub const BUFFER_OVERHEAD_BYTES: usize = HEADER_BYTES + ORBIT_FACT_BYTES + POOL_TRAILER_BYTES;
/// One reference-orbit record size in bytes.
pub const ORBIT_RECORD_BYTES: usize = 8;
/// One error record size in bytes.
pub const ERROR_RECORD_BYTES: usize = 16;
const MIN_BUFFER_CAPACITY_BYTES: usize = 644;

/// Returns `max(644, 64 + 8 * max_iter)` so a cap-64 request still fits the 300-digit policy.
///
/// # Errors
///
/// Returns `BadLength` if the capacity cannot be represented by the target.
pub fn buffer_capacity(max_iter: u32) -> Result<usize, ChannelError> {
    let records = usize::try_from(max_iter)
        .ok()
        .and_then(|count| count.checked_mul(ORBIT_RECORD_BYTES));
    records
        .and_then(|bytes| bytes.checked_add(BUFFER_OVERHEAD_BYTES))
        .map(|bytes| bytes.max(MIN_BUFFER_CAPACITY_BYTES))
        .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, max_iter, u32::MAX, 0))
}

/// The nine version-three wire messages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MessageKind {
    /// Main transfers a reference request to the producer.
    OrbitRequest = 1,
    /// Producer returns an emptied request buffer.
    RequestReturn = 2,
    /// Producer transfers a completed reference orbit to main.
    OrbitResponse = 3,
    /// Main returns a buffer after installing its generation.
    CreditApplied = 4,
    /// Main returns a buffer after rejecting its stale generation.
    CreditStale = 5,
    /// Producer returns measured but cancelled work.
    OrbitCancelled = 6,
    /// Either endpoint reports one [`ErrorRecord`].
    ChannelError = 7,
    /// Main asks the producer to reconcile and stop.
    Shutdown = 8,
    /// Producer confirms reconciliation and stop.
    ShutdownAck = 9,
}

impl TryFrom<u32> for MessageKind {
    type Error = ChannelError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::OrbitRequest),
            2 => Ok(Self::RequestReturn),
            3 => Ok(Self::OrbitResponse),
            4 => Ok(Self::CreditApplied),
            5 => Ok(Self::CreditStale),
            6 => Ok(Self::OrbitCancelled),
            7 => Ok(Self::ChannelError),
            8 => Ok(Self::Shutdown),
            9 => Ok(Self::ShutdownAck),
            detail => Err(ChannelError::new(ErrorCode::BadKind, detail, 0, 0)),
        }
    }
}

/// One eight-word message header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MessageHeader {
    /// [`MAGIC`].
    pub magic: u32,
    /// [`JULIBROT_ABI_VERSION`].
    pub version: u32,
    /// Checked request generation.
    pub generation: u32,
    /// [`MessageKind`] discriminant.
    pub kind: u32,
    /// Kind-specific record count.
    pub length: u32,
    /// Requested or delivered bignum precision.
    pub precision_bits: u32,
    /// Measured producer wall in microseconds.
    pub compute_us: u32,
    /// Producer admission or returned owner credit in microseconds.
    pub credit_us: u32,
}

impl MessageHeader {
    /// Builds a canonical version-three header.
    #[must_use]
    pub const fn new(kind: MessageKind, generation: u32) -> Self {
        Self {
            magic: MAGIC,
            version: JULIBROT_ABI_VERSION,
            generation,
            kind: kind as u32,
            length: 0,
            precision_bits: 0,
            compute_us: 0,
            credit_us: 0,
        }
    }

    /// Validates fixed words and returns the decoded kind.
    ///
    /// # Errors
    ///
    /// Returns the stable refusal for bad magic, version, or kind.
    pub fn validate(self) -> Result<MessageKind, ChannelError> {
        if self.magic != MAGIC {
            return Err(ChannelError::new(ErrorCode::BadMagic, self.magic, 0, 0));
        }
        if self.version != JULIBROT_ABI_VERSION {
            return Err(ChannelError::new(ErrorCode::BadVersion, self.version, 0, 0));
        }
        MessageKind::try_from(self.kind)
    }

    /// Writes all eight words in little-endian order.
    pub(crate) fn write_to(self, destination: &mut [u8]) -> Result<(), ChannelError> {
        if destination.len() < HEADER_BYTES {
            return Err(short_buffer(HEADER_BYTES, destination.len()));
        }
        let words = [
            self.magic,
            self.version,
            self.generation,
            self.kind,
            self.length,
            self.precision_bits,
            self.compute_us,
            self.credit_us,
        ];
        write_words(destination, &words);
        Ok(())
    }

    /// Decodes and validates all eight words.
    pub(crate) fn read_from(source: &[u8]) -> Result<Self, ChannelError> {
        if source.len() < HEADER_BYTES {
            return Err(short_buffer(HEADER_BYTES, source.len()));
        }
        let header = Self {
            magic: read_u32(source, 0),
            version: read_u32(source, 4),
            generation: read_u32(source, 8),
            kind: read_u32(source, 12),
            length: read_u32(source, 16),
            precision_bits: read_u32(source, 20),
            compute_us: read_u32(source, 24),
            credit_us: read_u32(source, 28),
        };
        header.validate()?;
        Ok(header)
    }
}

/// Stable four-word error payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ErrorRecord {
    /// [`ErrorCode`] discriminant.
    pub code: u32,
    /// Category-specific offending value.
    pub detail: u32,
    /// Required bytes for a capacity refusal.
    pub requested_bytes: u32,
    /// Available bytes for a capacity refusal.
    pub available_bytes: u32,
}

/// Verification facts carried outside the initialized orbit-record range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct OrbitVerificationFacts {
    /// [`ReferenceVerification`] discriminant.
    pub verification: u32,
    /// Maximum ULP distance over the four consumed reference words, or `u32::MAX` when deferred.
    pub max_consumed_word_error_ulps: u32,
    /// Number of sixteen-digit precision escalations before publication.
    pub precision_escalations: u32,
    /// Reserved zero word.
    pub reserved: u32,
}

impl OrbitVerificationFacts {
    /// Creates deferred-verification facts for a `PictureFast` Preview orbit.
    #[must_use]
    pub const fn deferred() -> Self {
        Self {
            verification: ReferenceVerification::Deferred as u32,
            max_consumed_word_error_ulps: u32::MAX,
            precision_escalations: 0,
            reserved: 0,
        }
    }

    /// Creates completed-verification facts.
    #[must_use]
    pub const fn stable(max_consumed_word_error_ulps: u32, precision_escalations: u32) -> Self {
        Self {
            verification: ReferenceVerification::Stable as u32,
            max_consumed_word_error_ulps,
            precision_escalations,
            reserved: 0,
        }
    }

    /// Converts math's completed-orbit facts into the fixed wire tail.
    #[must_use]
    pub fn from_orbit(orbit: &ComputedOrbit) -> Self {
        Self {
            verification: orbit.verification as u32,
            max_consumed_word_error_ulps: orbit.max_consumed_word_error_ulps.unwrap_or(u32::MAX),
            precision_escalations: orbit.precision_escalations,
            reserved: 0,
        }
    }

    /// Returns the validated verification state.
    ///
    /// # Errors
    ///
    /// Returns `BadLength` for an unknown state, a contradictory maximum, or nonzero reserve.
    pub const fn status(self) -> Result<ReferenceVerification, ChannelError> {
        if self.reserved != 0 {
            return Err(ChannelError::new(ErrorCode::BadLength, self.reserved, 0, 0));
        }
        match self.verification {
            value if value == ReferenceVerification::Deferred as u32 => {
                if self.max_consumed_word_error_ulps != u32::MAX || self.precision_escalations != 0
                {
                    return Err(ChannelError::new(
                        ErrorCode::BadLength,
                        self.max_consumed_word_error_ulps,
                        0,
                        0,
                    ));
                }
                Ok(ReferenceVerification::Deferred)
            }
            value if value == ReferenceVerification::Stable as u32 => {
                if self.max_consumed_word_error_ulps > 2 {
                    return Err(ChannelError::new(ErrorCode::BadLength, value, 0, 0));
                }
                Ok(ReferenceVerification::Stable)
            }
            value => Err(ChannelError::new(ErrorCode::BadLength, value, 0, 0)),
        }
    }

    /// Returns the maximum consumed-word error when verification ran.
    #[must_use]
    pub const fn max_consumed_word_error_ulps(self) -> Option<u32> {
        if self.max_consumed_word_error_ulps == u32::MAX {
            None
        } else {
            Some(self.max_consumed_word_error_ulps)
        }
    }

    fn write_to(self, destination: &mut [u8]) {
        write_words(
            destination,
            &[
                self.verification,
                self.max_consumed_word_error_ulps,
                self.precision_escalations,
                self.reserved,
            ],
        );
    }

    const fn read_from(source: &[u8]) -> Self {
        Self {
            verification: read_u32(source, 0),
            max_consumed_word_error_ulps: read_u32(source, 4),
            precision_escalations: read_u32(source, 8),
            reserved: read_u32(source, 12),
        }
    }
}

impl ErrorRecord {
    /// Writes the four words at the start of a message body.
    #[cfg(test)]
    fn write_to(self, destination: &mut [u8]) -> Result<(), ChannelError> {
        if destination.len() < ERROR_RECORD_BYTES {
            return Err(short_buffer(ERROR_RECORD_BYTES, destination.len()));
        }
        write_words(
            destination,
            &[
                self.code,
                self.detail,
                self.requested_bytes,
                self.available_bytes,
            ],
        );
        Ok(())
    }

    /// Reads and validates the stable error-code word.
    #[cfg(test)]
    fn read_from(source: &[u8]) -> Result<Self, ChannelError> {
        if source.len() < ERROR_RECORD_BYTES {
            return Err(short_buffer(ERROR_RECORD_BYTES, source.len()));
        }
        let record = Self {
            code: read_u32(source, 0),
            detail: read_u32(source, 4),
            requested_bytes: read_u32(source, 8),
            available_bytes: read_u32(source, 12),
        };
        ErrorCode::try_from(record.code)?;
        Ok(record)
    }
}

impl From<ChannelError> for ErrorRecord {
    fn from(value: ChannelError) -> Self {
        Self {
            code: value.code as u32,
            detail: value.detail,
            requested_bytes: value.requested_bytes,
            available_bytes: value.available_bytes,
        }
    }
}

impl TryFrom<ErrorRecord> for ChannelError {
    type Error = Self;

    fn try_from(value: ErrorRecord) -> Result<Self, Self::Error> {
        Ok(Self::new(
            ErrorCode::try_from(value.code)?,
            value.detail,
            value.requested_bytes,
            value.available_bytes,
        ))
    }
}

/// Pool identity in an immutable trailer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Pool {
    /// Main-to-producer request buffers.
    Request = 1,
    /// Producer-to-main orbit buffers.
    Orbit = 2,
}

impl TryFrom<u32> for Pool {
    type Error = ChannelError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Request),
            2 => Ok(Self::Orbit),
            detail => Err(ChannelError::new(ErrorCode::BadTrailer, detail, 0, 0)),
        }
    }
}

/// Immutable four-word pool trailer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct PoolTrailer {
    /// [`Pool`] discriminant.
    pub pool: u32,
    /// Pool-local slot, zero or one.
    pub slot: u32,
    /// Total allocated bytes, including header and trailer.
    pub capacity_bytes: u32,
    /// [`TRAILER_MAGIC`].
    pub trailer_magic: u32,
}

impl PoolTrailer {
    /// Builds one immutable allocation identity.
    pub(crate) fn new(pool: Pool, slot: u32, capacity_bytes: usize) -> Result<Self, ChannelError> {
        if slot > 1 {
            return Err(ChannelError::new(ErrorCode::BadTrailer, slot, 0, 0));
        }
        let capacity_bytes = u32::try_from(capacity_bytes)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        Ok(Self {
            pool: pool as u32,
            slot,
            capacity_bytes,
            trailer_magic: TRAILER_MAGIC,
        })
    }

    /// Validates the pool, slot, capacity, and magic.
    pub(crate) fn validate(self, actual_capacity: usize) -> Result<Pool, ChannelError> {
        if self.trailer_magic != TRAILER_MAGIC
            || self.slot > 1
            || usize::try_from(self.capacity_bytes).ok() != Some(actual_capacity)
        {
            return Err(ChannelError::new(
                ErrorCode::BadTrailer,
                self.slot,
                self.capacity_bytes,
                u32::try_from(actual_capacity).unwrap_or(u32::MAX),
            ));
        }
        Pool::try_from(self.pool)
    }
}

/// One preallocated pool buffer with immutable trailer identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireBuffer {
    bytes: Box<[u8]>,
}

impl WireBuffer {
    /// Allocates and initializes one zero-filled pool slot.
    pub(crate) fn new(pool: Pool, slot: u32, max_iter: u32) -> Result<Self, ChannelError> {
        let capacity = buffer_capacity(max_iter)?;
        let mut bytes = vec![0; capacity].into_boxed_slice();
        let trailer = PoolTrailer::new(pool, slot, capacity)?;
        write_words(
            &mut bytes[capacity - POOL_TRAILER_BYTES..],
            &[
                trailer.pool,
                trailer.slot,
                trailer.capacity_bytes,
                trailer.trailer_magic,
            ],
        );
        Ok(Self { bytes })
    }

    /// Adopts one copied standalone buffer after validating its immutable trailer.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_transferred(bytes: Box<[u8]>) -> Result<Self, ChannelError> {
        if bytes.len() < BUFFER_OVERHEAD_BYTES {
            return Err(short_buffer(BUFFER_OVERHEAD_BYTES, bytes.len()));
        }
        let buffer = Self { bytes };
        buffer.trailer()?;
        Ok(buffer)
    }

    /// Returns the full allocated byte count.
    pub(crate) const fn capacity(&self) -> usize {
        self.bytes.len()
    }

    /// Returns immutable bytes including the trailer.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns mutable message bytes while withholding the immutable trailer.
    pub(crate) fn message_bytes_mut(&mut self) -> &mut [u8] {
        let end = self.bytes.len() - POOL_TRAILER_BYTES;
        &mut self.bytes[..end]
    }

    /// Reads and validates this allocation's trailer.
    pub(crate) fn trailer(&self) -> Result<PoolTrailer, ChannelError> {
        let offset = self.bytes.len() - POOL_TRAILER_BYTES;
        let trailer = PoolTrailer {
            pool: read_u32(&self.bytes, offset),
            slot: read_u32(&self.bytes, offset + 4),
            capacity_bytes: read_u32(&self.bytes, offset + 8),
            trailer_magic: read_u32(&self.bytes, offset + 12),
        };
        trailer.validate(self.bytes.len())?;
        Ok(trailer)
    }

    /// Returns this buffer's pool and slot after trailer validation.
    pub(crate) fn identity(&self) -> Result<(Pool, u32), ChannelError> {
        let trailer = self.trailer()?;
        Ok((Pool::try_from(trailer.pool)?, trailer.slot))
    }

    /// Clears all mutable bytes while preserving the trailer exactly.
    pub(crate) fn clear_message(&mut self) {
        self.message_bytes_mut().fill(0);
    }

    /// Writes one canonical header, retaining producer-owned orbit payload bytes when the kind
    /// never reads them.
    pub(crate) fn write_header(&mut self, header: MessageHeader) -> Result<(), ChannelError> {
        let kind = header.validate()?;
        if !retains_orbit_payload(kind) {
            self.clear_message();
        }
        header.write_to(self.message_bytes_mut())
    }

    /// Reads and validates this buffer's header and trailer.
    pub(crate) fn header(&self) -> Result<MessageHeader, ChannelError> {
        self.trailer()?;
        MessageHeader::read_from(&self.bytes)
    }

    /// Validates pool, kind, count, and any unused capacity owned by the message kind.
    pub(crate) fn validate_message(&self) -> Result<MessageKind, ChannelError> {
        let (pool, _) = self.identity()?;
        let header = self.header()?;
        let kind = validate_message_layout(pool, self.capacity(), header, |used, message_end| {
            self.bytes[used..message_end].iter().all(|byte| *byte == 0)
        })?;
        if kind == MessageKind::OrbitResponse {
            self.orbit_facts()?.status()?;
        }
        Ok(kind)
    }

    /// Writes a typed channel-error message and its four-word body.
    #[cfg(test)]
    pub(crate) fn write_error(
        &mut self,
        generation: u32,
        error: ChannelError,
    ) -> Result<(), ChannelError> {
        let mut header = MessageHeader::new(MessageKind::ChannelError, generation);
        header.length = 4;
        self.write_header(header)?;
        ErrorRecord::from(error).write_to(&mut self.message_bytes_mut()[HEADER_BYTES..])
    }

    /// Reads the typed body of a validated channel-error message.
    #[cfg(test)]
    pub(crate) fn error(&self) -> Result<ChannelError, ChannelError> {
        if self.validate_message()? != MessageKind::ChannelError {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                self.header()?.kind,
                0,
                0,
            ));
        }
        ErrorRecord::read_from(&self.bytes[HEADER_BYTES..])?.try_into()
    }

    /// Copies reusable orbit scratch into this standalone orbit buffer once.
    pub(crate) fn write_orbit(
        &mut self,
        generation: u32,
        precision_bits: u32,
        compute_us: u32,
        admission_credit_us: u32,
        records: &[ReferenceOrbitRecord],
        facts: OrbitVerificationFacts,
    ) -> Result<(), ChannelError> {
        facts.status()?;
        let (pool, _) = self.identity()?;
        let requested = records
            .len()
            .checked_mul(ORBIT_RECORD_BYTES)
            .and_then(|bytes| HEADER_BYTES.checked_add(bytes))
            .ok_or_else(|| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        let available = self.capacity() - POOL_TRAILER_BYTES - ORBIT_FACT_BYTES;
        if pool != Pool::Orbit || records.is_empty() || requested > available {
            return Err(ChannelError::new(
                ErrorCode::BadLength,
                u32::try_from(records.len()).unwrap_or(u32::MAX),
                u32::try_from(requested).unwrap_or(u32::MAX),
                u32::try_from(available).unwrap_or(u32::MAX),
            ));
        }
        let mut header = MessageHeader::new(MessageKind::OrbitResponse, generation);
        header.length = u32::try_from(records.len())
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, u32::MAX, 0))?;
        header.precision_bits = precision_bits;
        header.compute_us = compute_us;
        header.credit_us = admission_credit_us;
        self.write_header(header)?;
        let message = self.message_bytes_mut();
        for (index, record) in records.iter().enumerate() {
            let offset = HEADER_BYTES + index * ORBIT_RECORD_BYTES;
            write_words(
                &mut message[offset..offset + ORBIT_RECORD_BYTES],
                &[record.re.to_bits(), record.im.to_bits()],
            );
        }
        let facts_offset = message.len() - ORBIT_FACT_BYTES;
        facts.write_to(&mut message[facts_offset..]);
        Ok(())
    }

    /// Reads and validates the fixed verification-fact tail.
    pub(crate) fn orbit_facts(&self) -> Result<OrbitVerificationFacts, ChannelError> {
        let offset = self.capacity() - POOL_TRAILER_BYTES - ORBIT_FACT_BYTES;
        let facts =
            OrbitVerificationFacts::read_from(&self.bytes[offset..offset + ORBIT_FACT_BYTES]);
        facts.status()?;
        Ok(facts)
    }

    /// Decodes a validated orbit payload into CPU records.
    #[cfg(test)]
    pub(crate) fn orbit_records(&self) -> Result<Vec<ReferenceOrbitRecord>, ChannelError> {
        if self.validate_message()? != MessageKind::OrbitResponse {
            return Err(ChannelError::new(
                ErrorCode::BadKind,
                self.header()?.kind,
                0,
                0,
            ));
        }
        let count = usize::try_from(self.header()?.length)
            .map_err(|_| ChannelError::new(ErrorCode::BadLength, 0, 0, 0))?;
        Ok((0..count)
            .map(|index| {
                let offset = HEADER_BYTES + index * ORBIT_RECORD_BYTES;
                ReferenceOrbitRecord {
                    re: f32::from_bits(read_u32(&self.bytes, offset)),
                    im: f32::from_bits(read_u32(&self.bytes, offset + 4)),
                }
            })
            .collect())
    }
}

pub const fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

pub fn write_words(destination: &mut [u8], words: &[u32]) {
    for (index, word) in words.iter().enumerate() {
        let offset = index * 4;
        destination[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
    }
}

fn short_buffer(requested: usize, available: usize) -> ChannelError {
    ChannelError::new(
        ErrorCode::BadLength,
        0,
        u32::try_from(requested).unwrap_or(u32::MAX),
        u32::try_from(available).unwrap_or(u32::MAX),
    )
}

fn bad_kind_or_length(header: MessageHeader) -> ChannelError {
    let code = if MessageKind::try_from(header.kind).is_ok() {
        ErrorCode::BadLength
    } else {
        ErrorCode::BadKind
    };
    ChannelError::new(code, header.kind, header.length, 0)
}

/// Validates one message using the shared byte layout and caller-supplied zero-tail check.
pub fn validate_message_layout(
    pool: Pool,
    capacity: usize,
    header: MessageHeader,
    unused_is_zero: impl FnOnce(usize, usize) -> bool,
) -> Result<MessageKind, ChannelError> {
    let kind = header.validate()?;
    let max_records = (capacity - BUFFER_OVERHEAD_BYTES) / ORBIT_RECORD_BYTES;
    let used = match kind {
        MessageKind::OrbitRequest => {
            if pool != Pool::Request || header.length == 0 {
                return Err(bad_kind_or_length(header));
            }
            return Ok(kind);
        }
        MessageKind::OrbitResponse => {
            let length = usize::try_from(header.length).map_err(|_| bad_kind_or_length(header))?;
            if pool != Pool::Orbit || length == 0 || length > max_records {
                return Err(bad_kind_or_length(header));
            }
            HEADER_BYTES + length * ORBIT_RECORD_BYTES
        }
        MessageKind::ChannelError => {
            if header.length != 4 {
                return Err(bad_kind_or_length(header));
            }
            HEADER_BYTES + ERROR_RECORD_BYTES
        }
        MessageKind::RequestReturn | MessageKind::Shutdown | MessageKind::ShutdownAck => {
            if pool != Pool::Request || header.length != 0 {
                return Err(bad_kind_or_length(header));
            }
            HEADER_BYTES
        }
        MessageKind::CreditApplied | MessageKind::CreditStale | MessageKind::OrbitCancelled => {
            if pool != Pool::Orbit || header.length != 0 {
                return Err(bad_kind_or_length(header));
            }
            HEADER_BYTES
        }
    };
    let message_end = capacity
        - POOL_TRAILER_BYTES
        - usize::from(kind == MessageKind::OrbitResponse) * ORBIT_FACT_BYTES;
    if !retains_orbit_payload(kind) && !unused_is_zero(used, message_end) {
        return Err(ChannelError::new(
            ErrorCode::BadLength,
            header.length,
            u32::try_from(used).unwrap_or(u32::MAX),
            u32::try_from(message_end).unwrap_or(u32::MAX),
        ));
    }
    Ok(kind)
}

pub(crate) const fn retains_orbit_payload(kind: MessageKind) -> bool {
    matches!(
        kind,
        MessageKind::OrbitResponse
            | MessageKind::CreditApplied
            | MessageKind::CreditStale
            | MessageKind::OrbitCancelled
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ERROR_RECORD_BYTES, HEADER_BYTES, MIN_BUFFER_CAPACITY_BYTES, MessageHeader, MessageKind,
        ORBIT_FACT_BYTES, ORBIT_RECORD_BYTES, OrbitVerificationFacts, POOL_TRAILER_BYTES, Pool,
        PoolTrailer, WireBuffer, buffer_capacity,
    };
    use crate::{ErrorCode, ReferenceOrbitRecord};

    #[test]
    fn native_layouts_match_the_wire_words() {
        assert_eq!(size_of::<MessageHeader>(), HEADER_BYTES);
        assert_eq!(align_of::<MessageHeader>(), 4);
        assert_eq!(size_of::<PoolTrailer>(), POOL_TRAILER_BYTES);
        assert_eq!(align_of::<PoolTrailer>(), 4);
        assert_eq!(size_of::<super::ErrorRecord>(), ERROR_RECORD_BYTES);
        assert_eq!(size_of::<super::OrbitVerificationFacts>(), ORBIT_FACT_BYTES);
        assert_eq!(size_of::<ReferenceOrbitRecord>(), ORBIT_RECORD_BYTES);
        assert_eq!(buffer_capacity(64).unwrap(), MIN_BUFFER_CAPACITY_BYTES);
    }

    #[test]
    fn every_kind_has_a_pinned_little_endian_header() {
        let kinds = [
            MessageKind::OrbitRequest,
            MessageKind::RequestReturn,
            MessageKind::OrbitResponse,
            MessageKind::CreditApplied,
            MessageKind::CreditStale,
            MessageKind::OrbitCancelled,
            MessageKind::ChannelError,
            MessageKind::Shutdown,
            MessageKind::ShutdownAck,
        ];
        for (index, kind) in kinds.into_iter().enumerate() {
            let mut bytes = [0_u8; HEADER_BYTES];
            let header = MessageHeader {
                length: u32::try_from(index).unwrap(),
                precision_bits: 320,
                compute_us: 41,
                credit_us: 250_000,
                ..MessageHeader::new(kind, 9)
            };
            header.write_to(&mut bytes).unwrap();
            assert_eq!(MessageHeader::read_from(&bytes).unwrap(), header);
            assert_eq!(&bytes[12..16], &(kind as u32).to_le_bytes());
        }
    }

    #[test]
    fn bad_fixed_words_and_trailer_are_typed() {
        let mut buffer = WireBuffer::new(Pool::Orbit, 1, 64).unwrap();
        buffer
            .write_header(MessageHeader::new(MessageKind::OrbitResponse, 3))
            .unwrap();
        let trailer = buffer.trailer().unwrap();
        assert_eq!(trailer.pool, Pool::Orbit as u32);
        assert_eq!(trailer.slot, 1);

        let mut header = buffer.header().unwrap();
        header.magic = 0;
        assert_eq!(header.validate().unwrap_err().code, ErrorCode::BadMagic);
        header.magic = super::MAGIC;
        header.version = super::JULIBROT_ABI_VERSION + 1;
        assert_eq!(header.validate().unwrap_err().code, ErrorCode::BadVersion);
        header.version = super::JULIBROT_ABI_VERSION;
        header.kind = 10;
        assert_eq!(header.validate().unwrap_err().code, ErrorCode::BadKind);
    }

    #[test]
    fn clearing_a_message_preserves_trailer_bit_exactly() {
        let mut buffer = WireBuffer::new(Pool::Request, 0, 64).unwrap();
        let trailer_offset = buffer.capacity() - POOL_TRAILER_BYTES;
        let trailer = buffer.as_bytes()[trailer_offset..].to_vec();
        buffer
            .write_header(MessageHeader::new(MessageKind::OrbitRequest, 1))
            .unwrap();
        buffer.clear_message();
        assert!(
            buffer.as_bytes()[..trailer_offset]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(&buffer.as_bytes()[trailer_offset..], trailer);
    }

    #[test]
    fn orbit_decode_ignores_poisoned_tail_and_credit_retains_payload() {
        let records = [
            ReferenceOrbitRecord { re: 1.0, im: -2.0 },
            ReferenceOrbitRecord { re: 0.0, im: 0.0 },
        ];
        let mut orbit = WireBuffer::new(Pool::Orbit, 0, 64).unwrap();
        orbit.message_bytes_mut()[HEADER_BYTES..].fill(0xa5);
        let facts = OrbitVerificationFacts::stable(2, 1);
        orbit
            .write_orbit(7, 320, 901, 249_099, &records, facts)
            .unwrap();
        assert_eq!(orbit.validate_message(), Ok(MessageKind::OrbitResponse));
        assert_eq!(orbit.orbit_records().unwrap(), records);
        assert_eq!(orbit.orbit_facts().unwrap(), facts);
        assert_eq!(&orbit.as_bytes()[32..36], &1.0_f32.to_le_bytes());
        assert!(
            orbit.as_bytes()[48..orbit.capacity() - 32]
                .iter()
                .all(|byte| *byte == 0xa5)
        );

        let payload = orbit.as_bytes()[HEADER_BYTES..48].to_vec();
        let mut credit = MessageHeader::new(MessageKind::CreditApplied, 7);
        credit.precision_bits = 320;
        credit.compute_us = 901;
        credit.credit_us = 249_099;
        orbit.write_header(credit).unwrap();
        assert_eq!(orbit.validate_message(), Ok(MessageKind::CreditApplied));
        assert_eq!(&orbit.as_bytes()[HEADER_BYTES..48], payload.as_slice());

        let expected = crate::ChannelError::new(ErrorCode::CentreEncodingWall, 128, 624, 512);
        let mut error = WireBuffer::new(Pool::Request, 1, 64).unwrap();
        error.write_error(8, expected).unwrap();
        assert_eq!(error.validate_message(), Ok(MessageKind::ChannelError));
        assert_eq!(error.error(), Ok(expected));
    }
}
