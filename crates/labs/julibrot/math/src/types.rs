/// Axis in the fixed order `(z.re, z.im, c.re, c.im)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Axis4 {
    E1 = 0,
    E2 = 1,
    E3 = 2,
    E4 = 3,
}

impl Axis4 {
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlanePreset {
    Mandelbrot,
    Julia { c0: [f64; 2] },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneSpec {
    pub axis_a: Axis4,
    pub axis_b: Axis4,
    pub plane_origin: [f64; 4],
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
}

#[derive(Clone, Debug, PartialEq)]
pub enum OrbitStep {
    Pending { stored: u32 },
    Complete(ComputedOrbit),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ViewMode {
    Flat = 0,
    Tumbled = 1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub epoch: u64,
    pub orbit_generation: u32,
    pub plane: Plane,
    pub plane_theta_1: f64,
    pub plane_theta_2: f64,
    pub zoom_log2: f64,
    pub view_theta_1: f64,
    pub grid_width: u32,
    pub grid_height: u32,
    pub view: ViewMode,
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
    #[error("the plane seed axes must be distinct")]
    InvalidPlaneSeed,
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
