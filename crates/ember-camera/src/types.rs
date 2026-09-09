use crate::{CameraError, Fixed};

/// Number of integer exponent steps in one factor-of-two scale change.
///
/// 1024 is the finest step emitted by the first consumer's wheel and slider paths. Keeping that
/// quantum integral makes every input scale reproducible from the stored exponent alone.
pub const EXPONENT_QUANTA_PER_OCTAVE: i32 = 1_024;

/// Shallow navigation limit, two octaves wider than the exponent-zero view.
pub const MIN_EXPONENT_QUANTA: i32 = -2 * EXPONENT_QUANTA_PER_OCTAVE;

/// Deep navigation limit retained from the first consumer's reachable control range.
///
/// At 120 octaves, a 512-bit value still retains nearly 300 guard bits beyond a maximum-width render
/// pixel, which is what permits a trip back out to recover the original centre bits.
pub const MAX_EXPONENT_QUANTA: i32 = 120 * EXPONENT_QUANTA_PER_OCTAVE;

/// An integer count of 1/1024-octave camera scale changes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Exponent(i32);

impl Exponent {
    /// Exponent zero, where the image plane spans four units across the render-grid width.
    pub const ZERO: Self = Self(0);

    /// Creates an exponent from its exact quantum count.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::ExponentOutOfRange`] outside the named navigation range.
    pub const fn new(quanta: i32) -> Result<Self, CameraError> {
        match quanta {
            MIN_EXPONENT_QUANTA..=MAX_EXPONENT_QUANTA => Ok(Self(quanta)),
            _ => Err(CameraError::ExponentOutOfRange),
        }
    }

    /// Returns the stored count of exponent quanta.
    #[must_use]
    pub const fn quanta(self) -> i32 {
        self.0
    }

    /// Returns a checked exponent change.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::ExponentOutOfRange`] when the sum leaves the named range.
    pub const fn checked_add(self, delta_quanta: i32) -> Result<Self, CameraError> {
        match self.0.checked_add(delta_quanta) {
            Some(quanta) => Self::new(quanta),
            None => Err(CameraError::ExponentOutOfRange),
        }
    }

    /// Encodes the signed quantum count in little-endian order.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// Decodes and validates a little-endian quantum count.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::ExponentOutOfRange`] for an exponent outside the named range.
    pub const fn from_le_bytes(bytes: [u8; 4]) -> Result<Self, CameraError> {
        Self::new(i32::from_le_bytes(bytes))
    }
}

impl Default for Exponent {
    fn default() -> Self {
        Self::ZERO
    }
}

/// An unsigned fraction of one complete turn.
///
/// Arithmetic wraps because adding or removing a complete turn does not change an orientation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Turn(u32);

impl Turn {
    /// Zero rotation.
    pub const ZERO: Self = Self(0);

    /// Creates a turn from its canonical binary fraction.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the canonical binary fraction of a full turn.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Adds an angular delta modulo one complete turn.
    #[must_use]
    pub const fn wrapping_add(self, delta: Self) -> Self {
        Self(self.0.wrapping_add(delta.0))
    }

    /// Returns the exactly represented inverse rotation.
    #[must_use]
    pub const fn inverse(self) -> Self {
        Self(self.0.wrapping_neg())
    }

    /// Encodes the turn fraction in little-endian order.
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// Decodes a little-endian turn fraction. Every bit pattern is canonical.
    #[must_use]
    pub const fn from_le_bytes(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }

    /// Quantises a finite radian angle into the canonical fraction of one turn.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite angle or failed integer conversion.
    pub fn from_radians(radians: f64) -> Result<Self, CameraError> {
        if !radians.is_finite() {
            return Err(CameraError::NonFinite);
        }
        let fraction = unit_turn_fraction(radians / core::f64::consts::TAU);
        let scaled = fraction * 4_294_967_296.0;
        let rounded = round_nonnegative_binary64(scaled)?;
        if rounded == 1_u64 << u32::BITS {
            Ok(Self::ZERO)
        } else {
            Ok(Self(
                u32::try_from(rounded).map_err(|_| CameraError::Overflow)?,
            ))
        }
    }
}

/// Reduces a finite turn count modulo one using only core floating-point arithmetic.
///
/// `%` leaves a negative remainder for a negative dividend. One bounded addition maps that result
/// into the Euclidean interval, and the final comparison canonicalises signed zero and a value
/// rounded up to one.
const fn unit_turn_fraction(turns: f64) -> f64 {
    let remainder = turns % 1.0;
    let nonnegative = if remainder < 0.0 {
        remainder + 1.0
    } else {
        remainder
    };
    if nonnegative == 0.0 || nonnegative >= 1.0 {
        0.0
    } else {
        nonnegative
    }
}

fn round_nonnegative_binary64(value: f64) -> Result<u64, CameraError> {
    if !value.is_finite() || value < 0.0 {
        return Err(CameraError::NonFinite);
    }
    if value == 0.0 {
        return Ok(0);
    }
    let bits = value.to_bits();
    let exponent_field = (bits >> 52) & 0x7ff;
    let exponent = i64::try_from(exponent_field).map_err(|_| CameraError::Overflow)? - 1_023;
    let significand = (1_u64 << 52) | (bits & ((1_u64 << 52) - 1));
    if exponent >= 52 {
        let shift = u32::try_from(exponent - 52).map_err(|_| CameraError::Overflow)?;
        return significand.checked_shl(shift).ok_or(CameraError::Overflow);
    }
    let shift = usize::try_from(52 - exponent).map_err(|_| CameraError::Overflow)?;
    if shift >= 64 {
        return Ok(0);
    }
    let retained = significand >> shift;
    let round_bit = significand & (1_u64 << (shift - 1)) != 0;
    let sticky = significand & ((1_u64 << (shift - 1)) - 1) != 0;
    Ok(retained + u64::from(round_bit && (sticky || retained & 1 != 0)))
}

/// Integer plane rotations for an N-dimensional orthonormal frame.
///
/// The strict upper triangle stores the `N * (N - 1) / 2` active turns. Every other slot is zero.
/// Product order is row-major over the active triangle: for `N = 5`, 12, 13, 14, 15, 23, 24,
/// 25, 34, 35, 45.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Orientation<const N: usize> {
    angles: [[Turn; N]; N],
}

impl<const N: usize> Orientation<N> {
    /// The unrotated frame.
    pub const IDENTITY: Self = Self {
        angles: [[Turn::ZERO; N]; N],
    };

    /// Creates a canonical orientation from nested triangle storage.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidOrientation`] when the diagonal or lower triangle is nonzero.
    pub fn new(angles: [[Turn; N]; N]) -> Result<Self, CameraError> {
        for (row_index, row) in angles.iter().enumerate() {
            if row[..=row_index.min(N.saturating_sub(1))]
                .iter()
                .any(|angle| *angle != Turn::ZERO)
            {
                return Err(CameraError::InvalidOrientation);
            }
        }
        Ok(Self { angles })
    }

    /// Returns all nested slots, including the canonical zero triangle.
    #[must_use]
    pub const fn angles(&self) -> &[[Turn; N]; N] {
        &self.angles
    }

    /// Returns one active plane angle, or `None` for a non-upper-triangle index.
    #[must_use]
    pub fn angle(&self, first: usize, second: usize) -> Option<Turn> {
        (first < second && second < N).then_some(self.angles[first][second])
    }

    /// Returns this orientation with every active delta added modulo a turn.
    #[must_use]
    pub fn with_deltas(&self, deltas: &Self) -> Self {
        let mut angles = self.angles;
        for (row_index, row) in angles.iter_mut().enumerate() {
            for (angle, delta) in row
                .iter_mut()
                .zip(deltas.angles[row_index])
                .skip(row_index + 1)
            {
                *angle = angle.wrapping_add(delta);
            }
        }
        Self { angles }
    }

    /// Returns the component-wise inverse delta.
    #[must_use]
    pub fn inverse(&self) -> Self {
        let mut angles = self.angles;
        for (row_index, row) in angles.iter_mut().enumerate() {
            for angle in row.iter_mut().skip(row_index + 1) {
                *angle = angle.inverse();
            }
        }
        Self { angles }
    }

    /// Encodes every nested turn in little-endian order.
    #[must_use]
    pub fn to_le_bytes(&self) -> [[[u8; 4]; N]; N] {
        let mut bytes = [[[0; 4]; N]; N];
        for (byte_row, angle_row) in bytes.iter_mut().zip(self.angles) {
            for (output, angle) in byte_row.iter_mut().zip(angle_row) {
                *output = angle.to_le_bytes();
            }
        }
        bytes
    }

    /// Decodes and validates nested little-endian turn storage.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidOrientation`] for a nonzero unused slot.
    pub fn from_le_bytes(bytes: [[[u8; 4]; N]; N]) -> Result<Self, CameraError> {
        let mut angles = [[Turn::ZERO; N]; N];
        for (angle_row, byte_row) in angles.iter_mut().zip(bytes) {
            for (angle, input) in angle_row.iter_mut().zip(byte_row) {
                *angle = Turn::from_le_bytes(input);
            }
        }
        Self::new(angles)
    }
}

impl<const N: usize> Default for Orientation<N> {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Exact camera state for a two-dimensional image plane in N-dimensional space.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct View<const N: usize, const LIMBS: usize> {
    /// Absolute centre coordinates.
    pub centre: [Fixed<LIMBS>; N],
    /// Exact logarithmic pixel scale.
    pub exponent: Exponent,
    /// Exact integer orientation of the complete frame.
    pub orientation: Orientation<N>,
}

impl<const N: usize, const LIMBS: usize> View<N, LIMBS> {
    /// Creates a view from its complete exact record.
    #[must_use]
    pub const fn new(
        centre: [Fixed<LIMBS>; N],
        exponent: Exponent,
        orientation: Orientation<N>,
    ) -> Self {
        Self {
            centre,
            exponent,
            orientation,
        }
    }

    /// Encodes the complete view into nested fixed-width little-endian fields.
    #[must_use]
    pub fn to_le_bytes(&self) -> ViewBytes<N, LIMBS> {
        let mut centre = [[[0; 8]; LIMBS]; N];
        for (output, coordinate) in centre.iter_mut().zip(self.centre) {
            *output = coordinate.to_le_bytes();
        }
        ViewBytes {
            centre,
            exponent: self.exponent.to_le_bytes(),
            orientation: self.orientation.to_le_bytes(),
        }
    }

    /// Decodes and validates a complete nested little-endian view.
    ///
    /// # Errors
    ///
    /// Returns an error for an out-of-range exponent or noncanonical orientation.
    pub fn from_le_bytes(bytes: ViewBytes<N, LIMBS>) -> Result<Self, CameraError> {
        let mut centre = [Fixed::ZERO; N];
        for (coordinate, input) in centre.iter_mut().zip(bytes.centre) {
            *coordinate = Fixed::from_le_bytes(input);
        }
        Ok(Self {
            centre,
            exponent: Exponent::from_le_bytes(bytes.exponent)?,
            orientation: Orientation::from_le_bytes(bytes.orientation)?,
        })
    }
}

/// Nested byte representation of an exact [`View`].
///
/// Nested arrays preserve fixed field widths on stable Rust without generic const expressions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ViewBytes<const N: usize, const LIMBS: usize> {
    /// Little-endian fixed-point coordinates.
    pub centre: [[[u8; 8]; LIMBS]; N],
    /// Little-endian signed exponent quanta.
    pub exponent: [u8; 4],
    /// Little-endian nested orientation turns.
    pub orientation: [[[u8; 4]; N]; N],
}

/// Render-grid geometry used by pixel-to-plane calculations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Screen {
    width: u32,
    height: u32,
}

impl Screen {
    /// Creates nonzero render-grid geometry.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidScreen`] when either dimension is zero.
    pub const fn new(width: u32, height: u32) -> Result<Self, CameraError> {
        if width == 0 || height == 0 {
            Err(CameraError::InvalidScreen)
        } else {
            Ok(Self { width, height })
        }
    }

    /// Returns the grid width in pixels.
    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    /// Returns the grid height in pixels.
    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }
}

/// Presentation-only observer controls, never read by exact camera edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observer<const N: usize> {
    /// Yaw in radians about the rebuilt image-plane frame.
    pub yaw: f64,
    /// Pitch in radians about the rebuilt image-plane frame.
    pub pitch: f64,
    /// Translation after frame rotation and before perspective division, in view units.
    pub translation: [f64; N],
    /// Positive perspective pole distance in view units.
    pub perspective: f64,
}

impl<const N: usize> Observer<N> {
    /// Creates validated presentation controls.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite values or a nonpositive perspective distance.
    pub fn new(
        yaw: f64,
        pitch: f64,
        translation: [f64; N],
        perspective: f64,
    ) -> Result<Self, CameraError> {
        if !yaw.is_finite()
            || !pitch.is_finite()
            || !perspective.is_finite()
            || perspective <= 0.0
            || !translation.iter().all(|component| component.is_finite())
        {
            return Err(CameraError::InvalidPerspective);
        }
        Ok(Self {
            yaw,
            pitch,
            translation,
            perspective,
        })
    }
}

/// Deterministically rebuilt floating-point orthonormal frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis<const N: usize> {
    /// First image-plane unit vector.
    pub u: [f64; N],
    /// Second image-plane unit vector.
    pub v: [f64; N],
    /// Remaining frame vectors in slots `2..N`; slots zero and one are canonical zeroes.
    pub remaining: [[f64; N]; N],
}

impl<const N: usize> Basis<N> {
    /// Returns a frame vector by index.
    #[must_use]
    pub fn vector(&self, index: usize) -> Option<&[f64; N]> {
        match index {
            0 => Some(&self.u),
            1 => Some(&self.v),
            _ => self.remaining.get(index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CameraError, Exponent, MAX_EXPONENT_QUANTA, MIN_EXPONENT_QUANTA, Orientation, Screen, Turn,
        View, unit_turn_fraction,
    };
    use crate::Fixed;

    #[test]
    fn unit_turn_fraction_is_euclidean_with_core_arithmetic() {
        for (turns, expected) in [
            (2.25_f64, 0.25_f64),
            (-2.25_f64, 0.75_f64),
            (3.0_f64, 0.0_f64),
            (-3.0_f64, 0.0_f64),
        ] {
            assert_eq!(unit_turn_fraction(turns).to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn exponent_range_and_encoding_are_exact() -> Result<(), CameraError> {
        for quanta in [MIN_EXPONENT_QUANTA, 0, MAX_EXPONENT_QUANTA] {
            let exponent = Exponent::new(quanta)?;
            assert_eq!(Exponent::from_le_bytes(exponent.to_le_bytes())?, exponent);
        }
        assert_eq!(
            Exponent::new(MIN_EXPONENT_QUANTA - 1),
            Err(CameraError::ExponentOutOfRange)
        );
        assert_eq!(
            Exponent::new(MAX_EXPONENT_QUANTA + 1),
            Err(CameraError::ExponentOutOfRange)
        );
        Ok(())
    }

    #[test]
    fn orientation_uses_only_the_strict_upper_triangle() -> Result<(), CameraError> {
        let mut angles = [[Turn::ZERO; 5]; 5];
        angles[0][4] = Turn::from_bits(7);
        angles[3][4] = Turn::from_bits(u32::MAX);
        let orientation = Orientation::new(angles)?;
        assert_eq!(orientation.angle(0, 4), Some(Turn::from_bits(7)));
        assert_eq!(orientation.angle(4, 0), None);
        assert_eq!(
            Orientation::from_le_bytes(orientation.to_le_bytes())?,
            orientation
        );
        angles[4][0] = Turn::from_bits(1);
        assert_eq!(
            Orientation::new(angles),
            Err(CameraError::InvalidOrientation)
        );
        Ok(())
    }

    #[test]
    fn angular_delta_and_inverse_restore_the_same_bits() -> Result<(), CameraError> {
        let mut angles = [[Turn::ZERO; 5]; 5];
        angles[0][1] = Turn::from_bits(123_456_789);
        angles[2][4] = Turn::from_bits(u32::MAX - 17);
        let delta = Orientation::new(angles)?;
        let original = Orientation::<5>::IDENTITY;
        assert_eq!(
            original.with_deltas(&delta).with_deltas(&delta.inverse()),
            original
        );
        Ok(())
    }

    #[test]
    fn complete_view_bytes_round_trip() -> Result<(), CameraError> {
        let screen = Screen::new(960, 540)?;
        assert_eq!([screen.width(), screen.height()], [960, 540]);
        let view = View::new(
            [Fixed::<2>::from_f64(1.25)?, Fixed::<2>::from_f64(-2.5)?],
            Exponent::new(17)?,
            Orientation::IDENTITY,
        );
        let bytes = view.to_le_bytes();
        assert_eq!(
            bytes.centre,
            [
                [[0, 0, 0, 0, 0, 0, 0, 64], [1, 0, 0, 0, 0, 0, 0, 0],],
                [
                    [0, 0, 0, 0, 0, 0, 0, 128],
                    [253, 255, 255, 255, 255, 255, 255, 255],
                ],
            ]
        );
        assert_eq!(bytes.exponent, [17, 0, 0, 0]);
        assert_eq!(bytes.orientation, [[[0; 4]; 2]; 2]);
        assert_eq!(View::from_le_bytes(bytes)?, view);
        Ok(())
    }
}
