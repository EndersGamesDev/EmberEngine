/// Consumer stage that decides whether `PictureFast` verifies a reference orbit.
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

/// Six ordered object-plane rotations in four-dimensional ambient space.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ObjectAngles {
    pub rho_12: f64,
    pub rho_13: f64,
    pub rho_14: f64,
    pub rho_23: f64,
    pub rho_24: f64,
    pub rho_34: f64,
}

impl ObjectAngles {
    pub const IDENTITY: Self = Self {
        rho_12: 0.0,
        rho_13: 0.0,
        rho_14: 0.0,
        rho_23: 0.0,
        rho_24: 0.0,
        rho_34: 0.0,
    };

    pub const JULIA: Self = Self {
        rho_13: -core::f64::consts::FRAC_PI_2,
        rho_24: -core::f64::consts::FRAC_PI_2,
        ..Self::IDENTITY
    };

    #[must_use]
    pub const fn as_array(self) -> [f64; 6] {
        [
            self.rho_12,
            self.rho_13,
            self.rho_14,
            self.rho_23,
            self.rho_24,
            self.rho_34,
        ]
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.as_array()
            .into_iter()
            .all(|angle| angle.is_finite() && angle.abs() <= core::f64::consts::PI)
    }
}

impl From<PlaneAngles> for ObjectAngles {
    fn from(value: PlaneAngles) -> Self {
        Self {
            rho_13: value.theta_1,
            rho_24: value.theta_2,
            ..Self::IDENTITY
        }
    }
}

/// One combined target, selection, or scale edit in mapped plane-offset pixels.
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
    pub re: f32,
    pub im: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct EscapeGridRecord {
    pub smooth_iter: f32,
    pub escaped: f32,
    pub rebase_count: f32,
    pub status: f32,
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

/// Every degree of freedom of the ambient camera and three-dimensional observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewControls {
    /// Ten `SO(5)` camera angles in product order: 12, 13, 14, 23, 24, 34, 15, 25, 35, 45.
    pub camera: [f64; 10],
    /// Five-dimensional camera translation applied after rotation and before perspective.
    pub camera_translation: [f64; 5],
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
        camera: [0.0; 10],
        camera_translation: [0.0; 5],
        camera_yaw: 0.0,
        camera_pitch: 0.0,
        height_scale: 0.0,
        distance_five: 8.0,
        distance_four: 8.0,
    };

    /// The camera row that faces the unrotated Mandelbrot seed toward screen axes one and two.
    pub const MANDELBROT_FLAT: Self = Self {
        camera: [
            0.0,
            -core::f64::consts::FRAC_PI_2,
            0.0,
            0.0,
            -core::f64::consts::FRAC_PI_2,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
        ],
        ..Self::NEUTRAL
    };

    pub const CAMERA_PLANES: [(usize, usize); 10] = [
        (0, 1),
        (0, 2),
        (0, 3),
        (1, 2),
        (1, 3),
        (2, 3),
        (0, 4),
        (1, 4),
        (2, 4),
        (3, 4),
    ];

    /// Returns every control as one array, in the order the records and facts publish them.
    #[must_use]
    pub const fn as_array(self) -> [f64; 20] {
        [
            self.camera[0],
            self.camera[1],
            self.camera[2],
            self.camera[3],
            self.camera[4],
            self.camera[5],
            self.camera[6],
            self.camera[7],
            self.camera[8],
            self.camera[9],
            self.camera_translation[0],
            self.camera_translation[1],
            self.camera_translation[2],
            self.camera_translation[3],
            self.camera_translation[4],
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
            && self
                .camera
                .into_iter()
                .all(|angle| angle.abs() <= core::f64::consts::PI)
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
    pub object: ObjectAngles,
    /// Absolute affine-plane origin in the four-dimensional object coordinates.
    pub plane_origin: [f64; 4],
    pub zoom_log2: f64,
    pub view: ViewControls,
    pub grid_width: u32,
    pub grid_height: u32,
    pub map: PoseMap,
    pub centre_from_reference_px: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PoseMap {
    Mapped(Homography),
    EdgeOn,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography {
    pub rows: [f64; 9],
    pub inverse: [f64; 9],
    pub condition_number: f64,
    /// Applied fixed-record sampling apron; the homography rows remain the presented camera.
    pub apron_scale: f64,
}

impl Homography {
    pub const IDENTITY: Self = Self {
        rows: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        inverse: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        condition_number: 1.0,
        apron_scale: 1.0,
    };
}

impl Default for Homography {
    fn default() -> Self {
        Self::IDENTITY
    }
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
    #[error("the neutral-height screen map is degenerate or uncertified")]
    DegenerateViewMap,
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
        assert_eq!(size_of::<ReferenceOrbitRecord>(), 8);
        assert_eq!(size_of::<EscapeGridRecord>(), 16);
        assert_eq!(offset_of!(EscapeGridRecord, status), 12);
        assert_eq!(size_of::<CentreF64>(), 32);
    }
}
