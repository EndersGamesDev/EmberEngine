use thiserror::Error;

/// Typed refusal returned before any incomplete kernel result is published.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum KernelError {
    #[error("grid extent or pixel index is invalid")]
    InvalidExtent,
    #[error("fixed-width kernel arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("the scale exponent cannot be represented")]
    ScaleExponentOverflow,
    #[error("escape parameters or pixel scale are invalid")]
    InvalidEscapeParams,
    #[error("the refinement-level discriminant is unknown")]
    UnknownLevel,
    #[error("a perturbation dispatch has no reference")]
    MissingReference,
    #[error("the supplied reference generation is stale")]
    StaleReference,
    #[error("reference length disagrees with its span or records")]
    ReferenceLengthMismatch,
    #[error("reference precision is unavailable or inconsistent")]
    ReferencePrecisionMismatch,
    #[error("the heap refused an allocation or lookup")]
    Heap,
    #[error("dialect registration failed")]
    Register,
    #[error("kernel dispatch planning failed")]
    Dispatch,
    #[error("the paid scratch-copy output path is unavailable")]
    OutputTransferUnsupported,
    #[error("the GPU device was lost")]
    DeviceLost,
}
