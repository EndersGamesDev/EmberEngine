/// Precision policy: exact and cross-machine reproducible, or accurate to the picture's budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PrecisionMode {
    Deterministic = 0,
    PictureFast = 1,
}

/// Consumer stage that decides whether PictureFast verifies a reference orbit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReferencePass {
    Preview = 0,
    Final = 1,
    Measure = 2,
}

/// Whether the published reference was checked against its higher-precision orbit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReferenceVerification {
    Deferred = 0,
    Stable = 1,
}

/// Axis in the fixed order `(z.re, z.im, c.re, c.im)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Axis4 {
    E1 = 0,
    E2 = 1,
    E3 = 2,
    E4 = 3,
}

/// Precision policy: exact and cross-machine reproducible, or accurate to the picture's budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PrecisionMode {
    /// Preserve the exact operation sequence used by conformance and replay.
    Deterministic = 0,
    /// Permit implementations whose Final pixels remain inside the picture contract.
    PictureFast = 1,
}

impl PrecisionMode {
    /// Both supported policies, for cfg-free conformance tests.
    pub const ALL: [Self; 2] = [Self::Deterministic, Self::PictureFast];

    /// Decodes the stable worker and page discriminant.
    #[must_use]
    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Deterministic),
            1 => Some(Self::PictureFast),
            _ => None,
        }
    }

    /// Returns the stable facts and provenance spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deterministic => "Deterministic",
            Self::PictureFast => "PictureFast",
        }
    }

    /// Whether a conformance assertion requires exact operation or word identity.
    #[must_use]
    pub const fn requires_bit_identity(self) -> bool {
        matches!(self, Self::Deterministic)
    }
}

#[allow(clippy::derivable_impls)]
impl Default for PrecisionMode {
    fn default() -> Self {
        Self::Deterministic
    }
}
impl Axis4 {
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneAngles {
    pub theta_1: f64,
    pub theta_2: f64,
}

/// One combined drag and pointer-anchored zoom edit in canvas-centred pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NavigationDelta {
    /// Drag displacement with positive y upward.
    pub pan_canvas_px: [f64; 2],
    /// Change in the base-two zoom exponent.
    pub zoom_delta_log2: f64,
    /// Zoom anchor relative to the canvas centre, with positive y upward.
    pub anchor_canvas_px: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct CentreF64 {
    pub coords: [f64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C, align(16))]
pub struct CentreSplit {
    pub hi: [f32; 4],
    pub lo: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C, align(16))]
pub struct Plane {
    pub basis_u: [f32; 4],
    pub basis_v: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct EscapeParams {
    pub max_iter: u32,
    pub bailout: f32,
}

impl EscapeParams {
    pub const BAILOUT: f32 = 256.0;

    #[must_use]
    pub const fn new(max_iter: u32) -> Self {
        Self {
            max_iter,
            bailout: Self::BAILOUT,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaledPixelScale {
    pub mantissa: f32,
    pub exponent: i32,
}

pub type ScaleSplit = ScaledPixelScale;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrecisionPlan {
    pub floor_digits: u32,
    pub working_digits: u32,
    pub requested_bits: u32,
    pub policy_digits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EscapeSample {
    pub smooth_iter: f32,
    pub escaped: bool,
    pub escape_index: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerturbSample {
    pub smooth_iter: f32,
    pub escaped: bool,
    pub escape_index: Option<u32>,
    pub rebase_count: u32,
    pub glitch: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerturbationEnvelope {
    pub delta_abs_error: f64,
    pub escape_norm2_error: f64,
    pub smooth_error: f64,
    pub minimum_escape_margin: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct ReferenceOrbitRecord {
    pub re_hi: f32,
    pub im_hi: f32,
    pub re_lo: f32,
    pub im_lo: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct EscapeGridRecord {
    pub smooth_iter: f32,
    pub escaped: f32,
    pub rebase_count: f32,
    pub glitch: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComputedOrbit {
    pub records: Vec<ReferenceOrbitRecord>,
    pub length: u32,
    pub precision_bits: u32,
    pub escape_index: Option<u32>,
    pub verification: ReferenceVerification,
    pub max_consumed_word_error_ulps: Option<u32>,
    pub precision_escalations: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrbitStep {
    Pending { stored: u32 },
    Complete(ComputedOrbit),
}

/// Every degree of freedom of the VIEW, as continuous controls.
///
/// There is no view mode and no clock: `theta_1` and `theta_2` are the two independent angles of
/// `R₁₂(θᵥ₁)·R₃₅(θᵥ₂)`, `camera_yaw` and `camera_pitch` orient the three-space observer,
/// `height_scale` multiplies the escape height so zero is exactly the flat chart, and the two
/// distances are the poles of the double perspective, the second of which is also the observer
/// distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewControls {
    /// First VIEW angle, acting in the display `(1,2)` plane.
    pub theta_1: f64,
    /// Second VIEW angle, acting in the display `(3,5)` plane.
    pub theta_2: f64,
    /// Observer yaw in radians.
    pub camera_yaw: f64,
    /// Observer pitch in radians.
    pub camera_pitch: f64,
    /// Escape-height amplitude; zero is the flat chart and one the shipped relief.
    pub height_scale: f64,
    /// Pole of the five-to-four perspective.
    pub distance_five: f64,
    /// Pole of the four-to-three perspective and the observer distance.
    pub distance_four: f64,
}

impl ViewControls {
    /// The row every preset starts from: no rotation, no relief, both distances at eight.
    pub const NEUTRAL: Self = Self {
        theta_1: 0.0,
        theta_2: 0.0,
        camera_yaw: 0.0,
        camera_pitch: 0.0,
        height_scale: 0.0,
        distance_five: 8.0,
        distance_four: 8.0,
    };

    /// Returns every control as one array, in the order the records and facts publish them.
    #[must_use]
    pub const fn as_array(self) -> [f64; 7] {
        [
            self.theta_1,
            self.theta_2,
            self.camera_yaw,
            self.camera_pitch,
            self.height_scale,
            self.distance_five,
            self.distance_four,
        ]
    }

    /// Reports whether every control is finite, the height is non-negative, and both distances are
    /// strictly positive.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.as_array().iter().all(|value| value.is_finite())
            && self.height_scale >= 0.0
            && self.distance_five > 0.0
            && self.distance_four > 0.0
    }
}

impl Default for ViewControls {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub epoch: u64,
    pub orbit_generation: u32,
    pub plane: Plane,
    pub plane_theta_1: f64,
    pub plane_theta_2: f64,
    pub zoom_log2: f64,
    pub view: ViewControls,
    pub grid_width: u32,
    pub grid_height: u32,
    pub centre_from_reference_px: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarpMatrix {
    pub forward: [f64; 9],
    pub inverse: [f64; 9],
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum MathError {
    #[error("a numeric input or result was not finite")]
    NonFinite,
    #[error("grid extent must be nonzero")]
    InvalidExtent,
    #[error("max_iter must be nonzero")]
    InvalidMaxIter,
    #[error("a VIEW control was not finite, or a height or distance left its range")]
    InvalidViewControls,
    #[error("f32 plane rounding exceeded the proved postcondition")]
    PlaneRoundingBound,
    #[error("the centre encoding is not canonical")]
    InvalidCentreEncoding,
    #[error("the escape bailout is not the fixed squared radius 256.0")]
    InvalidBailout,
    #[error("bignum centres use different delivered precisions")]
    PrecisionMismatch,
    #[error("the scale exponent is outside i32 range")]
    ScaleExponentOverflow,
    #[error("the warp matrix is degenerate")]
    DegenerateWarp,
    #[error("the orbit cannot be represented by the requested record count")]
    OrbitTooLong,
    #[error("the reference-orbit builder reached an inconsistent state")]
    InvalidOrbitState,
    #[error("the reference orbit is empty")]
    EmptyReferenceOrbit,
    #[error("the precision plan is internally inconsistent")]
    InvalidPrecisionPlan,
    #[error("an exact counter overflowed")]
    CounterOverflow,
    #[error("a measured duration exceeded its u32 representation")]
    DurationOverflow,
    #[error("working precision {requested_digits} exceeded policy {policy_digits}")]
    PrecisionExhausted {
        requested_digits: u32,
        policy_digits: u32,
    },
    #[error("Astro-float rejected an operation")]
    BigFloat,
}

#[cfg(test)]
mod tests {
    use super::{
        CentreF64, CentreSplit, EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord,
    };
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn shared_record_layouts_are_exact() {
        assert_eq!(size_of::<Plane>(), 32);
        assert_eq!(align_of::<Plane>(), 16);
        assert_eq!(offset_of!(Plane, basis_u), 0);
        assert_eq!(offset_of!(Plane, basis_v), 16);
        assert_eq!(size_of::<CentreSplit>(), 32);
        assert_eq!(align_of::<CentreSplit>(), 16);
        assert_eq!(offset_of!(CentreSplit, hi), 0);
        assert_eq!(offset_of!(CentreSplit, lo), 16);
        assert_eq!(size_of::<EscapeParams>(), 8);
        assert_eq!(size_of::<ReferenceOrbitRecord>(), 16);
        assert_eq!(size_of::<EscapeGridRecord>(), 16);
        assert_eq!(size_of::<CentreF64>(), 32);
    }
}
