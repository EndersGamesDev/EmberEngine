//! Typed channel and wire refusals.

use core::fmt;

/// Stable wire error codes carried by [`crate::ErrorRecord`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ErrorCode {
    /// Header magic was not `JBL1`.
    BadMagic = 1,
    /// Wire or module ABI version differed from version one.
    BadVersion = 2,
    /// Message-kind discriminant was unknown or illegal for a pool.
    BadKind = 3,
    /// Length, reserved bytes, reason bits, or descriptor shape was invalid.
    BadLength = 4,
    /// Pool trailer did not match its immutable allocation identity.
    BadTrailer = 5,
    /// The canonical centre did not fit the orbit-sized request buffer.
    CentreEncodingWall = 6,
    /// A generation counter could not advance without wrapping.
    GenerationExhausted = 7,
    /// The owner epoch could not advance without wrapping.
    EpochExhausted = 8,
    /// A measured duration could not be represented in microseconds.
    TimingOverflow = 9,
    /// A pool slot was missing or returned more than once.
    BufferStarved = 10,
    /// The math package refused reference-orbit work.
    MathFailure = 11,
    /// An internal browser state reached work already handled by the caller.
    UnexpectedWork = 12,
}

impl TryFrom<u32> for ErrorCode {
    type Error = ChannelError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::BadMagic),
            2 => Ok(Self::BadVersion),
            3 => Ok(Self::BadKind),
            4 => Ok(Self::BadLength),
            5 => Ok(Self::BadTrailer),
            6 => Ok(Self::CentreEncodingWall),
            7 => Ok(Self::GenerationExhausted),
            8 => Ok(Self::EpochExhausted),
            9 => Ok(Self::TimingOverflow),
            10 => Ok(Self::BufferStarved),
            11 => Ok(Self::MathFailure),
            12 => Ok(Self::UnexpectedWork),
            detail => Err(ChannelError::new(Self::BadKind, detail, 0, 0)),
        }
    }
}

/// One stable, allocation-free channel refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChannelError {
    /// Stable refusal category.
    pub code: ErrorCode,
    /// Category-specific offending value.
    pub detail: u32,
    /// Required byte count when capacity caused the refusal.
    pub requested_bytes: u32,
    /// Available byte count when capacity caused the refusal.
    pub available_bytes: u32,
}

impl ChannelError {
    /// Builds a fully specified refusal.
    #[must_use]
    pub const fn new(
        code: ErrorCode,
        detail: u32,
        requested_bytes: u32,
        available_bytes: u32,
    ) -> Self {
        Self {
            code,
            detail,
            requested_bytes,
            available_bytes,
        }
    }
}

impl fmt::Display for ChannelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "worker channel {:?}: detail {}, requested {} bytes, available {} bytes",
            self.code, self.detail, self.requested_bytes, self.available_bytes
        )
    }
}

impl std::error::Error for ChannelError {}
