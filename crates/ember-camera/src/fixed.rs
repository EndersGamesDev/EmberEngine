use core::cmp::Ordering;
use core::fmt;

const LIMB_BITS: usize = 64;
const SIGN_BIT: u64 = 1_u64 << (u64::BITS - 1);
const F64_FRACTION_BITS: usize = 52;
const F64_SIGNIFICAND_BITS: usize = 53;
const F64_EXPONENT_BIAS: i64 = 1_023;
const F64_MINIMUM_NORMAL_EXPONENT: i64 = -1_022;
const F64_SUBNORMAL_EXPONENT: i64 = -1_074;

/// Integer bits retained by every [`Fixed`], including its sign bit.
///
/// One complete 64-bit limb gives camera centres the standard signed range from −2⁶³ through the
/// fixed value immediately below 2⁶³. Every additional limb is fractional precision, so the first
/// consumer's eight limbs retain 448 fractional bits without making that width a ceiling.
pub const FIXED_INTEGER_BITS: usize = LIMB_BITS;

/// A typed refusal from exact camera arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraError {
    /// A zero-limb fixed-point type cannot represent a signed number.
    InvalidWidth,
    /// A finite input or arithmetic result is outside the fixed-width signed range.
    Overflow,
    /// A floating-point boundary input is NaN or infinite.
    NonFinite,
    /// A screen dimension is zero.
    InvalidScreen,
    /// An exponent lies outside the camera's named navigation range.
    ExponentOutOfRange,
    /// An orientation uses a diagonal or lower-triangle storage slot.
    InvalidOrientation,
    /// An image plane requires at least two ambient dimensions.
    DimensionTooSmall,
    /// Points cannot define the requested frame.
    DegenerateFrame,
    /// Perspective parameters cannot define a forward ray.
    InvalidPerspective,
    /// A pixel coordinate is outside the named screen-space working range.
    ScreenCoordinateOutOfRange,
    /// A box or point span has zero extent.
    EmptySelection,
}

impl fmt::Display for CameraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWidth => formatter.write_str("fixed-point width must contain a limb"),
            Self::Overflow => formatter.write_str("value is outside the fixed-point range"),
            Self::NonFinite => formatter.write_str("floating-point input is not finite"),
            Self::InvalidScreen => formatter.write_str("screen dimensions must be nonzero"),
            Self::ExponentOutOfRange => {
                formatter.write_str("exponent is outside the navigation range")
            }
            Self::InvalidOrientation => formatter.write_str("orientation storage is not canonical"),
            Self::DimensionTooSmall => {
                formatter.write_str("an image plane needs at least two dimensions")
            }
            Self::DegenerateFrame => formatter.write_str("points do not define a camera frame"),
            Self::InvalidPerspective => {
                formatter.write_str("observer perspective does not define a forward ray")
            }
            Self::ScreenCoordinateOutOfRange => {
                formatter.write_str("pixel coordinate is outside the screen working range")
            }
            Self::EmptySelection => formatter.write_str("selection has zero extent"),
        }
    }
}

impl core::error::Error for CameraError {}

/// A signed two's-complement fixed-point value stored least-significant limb first.
///
/// The high limb contains the sign and 63 magnitude bits. The remaining limbs contain the
/// fraction. Arithmetic is checked: an operation that would leave the represented signed range
/// returns [`CameraError::Overflow`] instead of changing a caller's value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Fixed<const LIMBS: usize> {
    limbs: [u64; LIMBS],
}

impl<const LIMBS: usize> Fixed<LIMBS> {
    /// Zero at this width.
    pub const ZERO: Self = Self { limbs: [0; LIMBS] };

    /// Number of fractional bits carried at this width.
    pub const FRACTION_BITS: usize = LIMBS.saturating_sub(1).saturating_mul(LIMB_BITS);

    /// Creates an exact fixed-point integer.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidWidth`] when `LIMBS` is zero.
    pub fn from_i64(value: i64) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let mut limbs = [0; LIMBS];
        limbs[LIMBS - 1] = u64::from_ne_bytes(value.to_ne_bytes());
        Ok(Self { limbs })
    }

    /// Converts a finite binary64 value, rounding to nearest with ties to an even lowest bit.
    ///
    /// The conversion is lossless whenever the binary64 significand lies inside this type's
    /// fractional width. Values nearer to zero than that width are quantised at the documented
    /// floating-point boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width, a non-finite input, or an out-of-range value.
    pub fn from_f64(value: f64) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let bits = value.to_bits();
        let exponent_field = (bits >> F64_FRACTION_BITS) & 0x7ff;
        if exponent_field == 0x7ff {
            return Err(CameraError::NonFinite);
        }
        let fraction = bits & ((1_u64 << F64_FRACTION_BITS) - 1);
        let (significand, exponent) = if exponent_field == 0 {
            (fraction, F64_SUBNORMAL_EXPONENT)
        } else {
            let unbiased = i64::try_from(exponent_field).map_err(|_| CameraError::Overflow)?
                - F64_EXPONENT_BIAS;
            (
                (1_u64 << F64_FRACTION_BITS) | fraction,
                unbiased
                    - i64::try_from(F64_FRACTION_BITS).map_err(|_| CameraError::InvalidWidth)?,
            )
        };
        if significand == 0 {
            return Ok(Self::ZERO);
        }
        let fraction_bits =
            i64::try_from(Self::FRACTION_BITS).map_err(|_| CameraError::InvalidWidth)?;
        let raw_shift = exponent
            .checked_add(fraction_bits)
            .ok_or(CameraError::Overflow)?;
        let mut magnitude = [0; LIMBS];
        if raw_shift >= 0 {
            let shift = usize::try_from(raw_shift).map_err(|_| CameraError::Overflow)?;
            place_significand(&mut magnitude, significand, shift)?;
        } else {
            let shift =
                usize::try_from(raw_shift.unsigned_abs()).map_err(|_| CameraError::Overflow)?;
            magnitude[0] = round_u64_right(significand, shift);
        }
        Self::from_magnitude(magnitude, bits & SIGN_BIT != 0)
    }

    /// Converts a binary64 bit pattern through the documented lossy ingress boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pattern, an invalid width, or a value outside the fixed
    /// range.
    pub fn from_binary64_bits(bits: u64) -> Result<Self, CameraError> {
        Self::from_f64(f64::from_bits(bits))
    }

    /// Converts this value to binary64, rounding to nearest with ties to even.
    ///
    /// This readout conversion is lossy whenever more than 53 significant bits are set.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidWidth`] when `LIMBS` is zero or its bit count cannot be
    /// represented by the conversion bookkeeping.
    pub fn to_f64(&self) -> Result<f64, CameraError> {
        Self::validate_width()?;
        let negative = self.is_negative();
        let magnitude = self.magnitude();
        let Some(high_bit) = highest_set_bit(&magnitude) else {
            return Ok(0.0);
        };
        let high_bit_i64 = i64::try_from(high_bit).map_err(|_| CameraError::InvalidWidth)?;
        let fraction_bits =
            i64::try_from(Self::FRACTION_BITS).map_err(|_| CameraError::InvalidWidth)?;
        let mut exponent = high_bit_i64 - fraction_bits;
        let sign = if negative { SIGN_BIT } else { 0 };
        if exponent >= F64_MINIMUM_NORMAL_EXPONENT {
            let mut significand = if high_bit < F64_SIGNIFICAND_BITS {
                magnitude[0] << (F64_FRACTION_BITS - high_bit)
            } else {
                round_magnitude_right(&magnitude, high_bit - F64_FRACTION_BITS)
            };
            if significand == 1_u64 << F64_SIGNIFICAND_BITS {
                significand >>= 1;
                exponent = exponent.checked_add(1).ok_or(CameraError::Overflow)?;
            }
            if exponent > 1_023 {
                return Err(CameraError::Overflow);
            }
            let biased =
                u64::try_from(exponent + F64_EXPONENT_BIAS).map_err(|_| CameraError::Overflow)?;
            let fraction_mask = (1_u64 << F64_FRACTION_BITS) - 1;
            return Ok(f64::from_bits(
                sign | (biased << F64_FRACTION_BITS) | (significand & fraction_mask),
            ));
        }
        let subnormal_shift = fraction_bits - -F64_SUBNORMAL_EXPONENT;
        let significand = if subnormal_shift <= 0 {
            let left = usize::try_from(subnormal_shift.unsigned_abs())
                .map_err(|_| CameraError::InvalidWidth)?;
            magnitude[0] << left
        } else {
            round_magnitude_right(
                &magnitude,
                usize::try_from(subnormal_shift).map_err(|_| CameraError::InvalidWidth)?,
            )
        };
        Ok(f64::from_bits(sign | significand))
    }

    /// Returns whether the represented value is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.limbs.iter().all(|limb| *limb == 0)
    }

    /// Returns whether the represented value is negative.
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        LIMBS != 0 && self.limbs[LIMBS - 1] & SIGN_BIT != 0
    }

    /// Returns the exponent of the highest set magnitude bit, where zero denotes one plane unit.
    ///
    /// Zero has no leading bit.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidWidth`] when `LIMBS` is zero or its bit count cannot be
    /// represented by the result.
    pub fn leading_bit(&self) -> Result<Option<i64>, CameraError> {
        Self::validate_width()?;
        let Some(bit) = highest_set_bit(&self.magnitude()) else {
            return Ok(None);
        };
        let bit = i64::try_from(bit).map_err(|_| CameraError::InvalidWidth)?;
        let fraction = i64::try_from(Self::FRACTION_BITS).map_err(|_| CameraError::InvalidWidth)?;
        Ok(Some(bit - fraction))
    }

    /// Adds two values without changing either operand on overflow.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or signed overflow.
    pub fn add(&self, other: &Self) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let left_negative = self.is_negative();
        let right_negative = other.is_negative();
        let mut limbs = [0; LIMBS];
        let mut carry = false;
        for ((output, left), right) in limbs.iter_mut().zip(self.limbs).zip(other.limbs) {
            let (partial, first_carry) = left.overflowing_add(right);
            let (sum, second_carry) = partial.overflowing_add(u64::from(carry));
            *output = sum;
            carry = first_carry || second_carry;
        }
        let result = Self { limbs };
        if left_negative == right_negative && result.is_negative() != left_negative {
            Err(CameraError::Overflow)
        } else {
            Ok(result)
        }
    }

    /// Subtracts two values without changing either operand on overflow.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or signed overflow.
    pub fn sub(&self, other: &Self) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let left_negative = self.is_negative();
        let right_negative = other.is_negative();
        let mut limbs = [0; LIMBS];
        let mut borrow = false;
        for ((output, left), right) in limbs.iter_mut().zip(self.limbs).zip(other.limbs) {
            let (partial, first_borrow) = left.overflowing_sub(right);
            let (difference, second_borrow) = partial.overflowing_sub(u64::from(borrow));
            *output = difference;
            borrow = first_borrow || second_borrow;
        }
        let result = Self { limbs };
        if left_negative != right_negative && result.is_negative() != left_negative {
            Err(CameraError::Overflow)
        } else {
            Ok(result)
        }
    }

    /// Negates this value without changing it when the positive result is unrepresentable.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or negation of the most-negative value.
    pub fn neg(&self) -> Result<Self, CameraError> {
        Self::validate_width()?;
        Self::from_magnitude(self.magnitude(), !self.is_negative())
    }

    /// Multiplies by a signed integer exactly.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or signed overflow.
    pub fn mul_small(&self, factor: i64) -> Result<Self, CameraError> {
        Self::validate_width()?;
        if factor == 0 || self.is_zero() {
            return Ok(Self::ZERO);
        }
        let mut magnitude = self.magnitude();
        let factor_magnitude = factor.unsigned_abs();
        let mut carry = 0_u128;
        for limb in &mut magnitude {
            let product = u128::from(*limb) * u128::from(factor_magnitude) + carry;
            *limb = low_u64(product)?;
            carry = product >> LIMB_BITS;
        }
        if carry != 0 {
            return Err(CameraError::Overflow);
        }
        Self::from_magnitude(magnitude, self.is_negative() != factor.is_negative())
    }

    /// Multiplies two fixed-point values and rounds once, to nearest with ties to even.
    ///
    /// This is the fixed arithmetic's sole rounding operation. It rounds the discarded low half
    /// of the double-width product directly into the lowest retained bit.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or signed overflow after rounding.
    pub fn mul(&self, other: &Self) -> Result<Self, CameraError> {
        Self::validate_width()?;
        if self.is_zero() || other.is_zero() {
            return Ok(Self::ZERO);
        }
        let left = self.magnitude();
        let right = other.magnitude();
        let (low, high) = multiply_wide(&left, &right)?;
        let shift_limbs = LIMBS - 1;
        let mut magnitude = [0; LIMBS];
        for (index, output) in magnitude.iter_mut().enumerate() {
            *output = wide_limb(&low, &high, index + shift_limbs);
        }
        let mut extra = high[LIMBS - 1];
        if shift_limbs != 0 {
            let rounding_limb = low[shift_limbs - 1];
            let round_bit = rounding_limb & SIGN_BIT != 0;
            let sticky = rounding_limb & (SIGN_BIT - 1) != 0
                || low[..shift_limbs - 1].iter().any(|limb| *limb != 0);
            if round_bit && (sticky || magnitude[0] & 1 != 0) {
                let carry = increment(&mut magnitude);
                if carry {
                    extra = extra.checked_add(1).ok_or(CameraError::Overflow)?;
                }
            }
        }
        if extra != 0 {
            return Err(CameraError::Overflow);
        }
        Self::from_magnitude(magnitude, self.is_negative() != other.is_negative())
    }

    /// Multiplies by one fixed value and divides by another with one nearest-even rounding.
    ///
    /// The complete double-width product is divided as an integer before the result is narrowed,
    /// so no intermediate fixed multiplication rounding enters the ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero divisor, an invalid width, or a rounded result outside the
    /// signed fixed range.
    pub fn mul_ratio_round_even(
        &self,
        multiplier: &Self,
        divisor: &Self,
    ) -> Result<Self, CameraError> {
        let divisor = RatioDivisor::new(divisor)?;
        self.mul_prepared_ratio_round_even(multiplier, &divisor)
    }

    /// Applies one exact ratio to an array while sharing its normalized divisor.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero divisor, an invalid width, or any rounded component outside
    /// the signed fixed range.
    pub(crate) fn mul_ratio_components_round_even<const N: usize>(
        values: &[Self; N],
        multiplier: &Self,
        divisor: &Self,
    ) -> Result<[Self; N], CameraError> {
        let divisor = RatioDivisor::new(divisor)?;
        let mut output = [Self::ZERO; N];
        for (result, value) in output.iter_mut().zip(values) {
            *result = value.mul_prepared_ratio_round_even(multiplier, &divisor)?;
        }
        Ok(output)
    }

    fn mul_prepared_ratio_round_even(
        &self,
        multiplier: &Self,
        divisor: &RatioDivisor<LIMBS>,
    ) -> Result<Self, CameraError> {
        let product = multiply_signed_wide::<LIMBS, 1, 1, 2>(
            &fixed_as_wide(self),
            &fixed_as_wide(multiplier),
        )?;
        divide_product_to_fixed(&product, divisor)
    }

    /// Shifts left by a bit count, refusing any signed overflow.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero-limb width or when a nonzero result leaves the signed range.
    pub fn shift_left(&self, bit_count: usize) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let bit_width = LIMBS
            .checked_mul(LIMB_BITS)
            .ok_or(CameraError::InvalidWidth)?;
        if bit_count >= bit_width {
            return self
                .is_zero()
                .then_some(Self::ZERO)
                .ok_or(CameraError::Overflow);
        }
        let mut result = *self;
        for _ in 0..bit_count {
            let was_negative = result.is_negative();
            let mut carry = 0;
            for limb in &mut result.limbs {
                let next_carry = *limb >> (u64::BITS - 1);
                *limb = (*limb << 1) | carry;
                carry = next_carry;
            }
            if result.is_negative() != was_negative {
                return Err(CameraError::Overflow);
            }
        }
        Ok(result)
    }

    /// Arithmetically shifts right, dropping low bits toward negative infinity.
    ///
    /// This named shift quantisation is separate from fixed multiplication's nearest-even rule.
    /// A count at least as wide as the storage produces zero for a nonnegative value and the
    /// smallest negative fractional value for a negative value.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::InvalidWidth`] when `LIMBS` is zero.
    pub fn shift_right(&self, bit_count: usize) -> Result<Self, CameraError> {
        Self::validate_width()?;
        let bit_width = LIMBS
            .checked_mul(LIMB_BITS)
            .ok_or(CameraError::InvalidWidth)?;
        if bit_count >= bit_width {
            return Ok(if self.is_negative() {
                Self {
                    limbs: [u64::MAX; LIMBS],
                }
            } else {
                Self::ZERO
            });
        }
        let mut result = *self;
        for _ in 0..bit_count {
            let mut carry = if result.is_negative() { SIGN_BIT } else { 0 };
            for limb in result.limbs.iter_mut().rev() {
                let next_carry = (*limb & 1) << (u64::BITS - 1);
                *limb = (*limb >> 1) | carry;
                carry = next_carry;
            }
        }
        Ok(result)
    }

    /// Divides by a small unsigned integer, rounding to nearest with ties to even.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero divisor, an invalid width, or a rounded quotient outside the
    /// fixed range.
    pub fn div_u32_round_even(&self, divisor: u32) -> Result<Self, CameraError> {
        Self::validate_width()?;
        if divisor == 0 {
            return Err(CameraError::InvalidScreen);
        }
        let negative = self.is_negative();
        let magnitude = self.magnitude();
        let mut quotient = [0; LIMBS];
        let divisor = u128::from(divisor);
        let mut remainder = 0_u128;
        for (output, limb) in quotient.iter_mut().zip(magnitude).rev() {
            let dividend = (remainder << LIMB_BITS) | u128::from(limb);
            *output = low_u64(dividend / divisor)?;
            remainder = dividend % divisor;
        }
        let twice_remainder = remainder.checked_mul(2).ok_or(CameraError::Overflow)?;
        if (twice_remainder > divisor || (twice_remainder == divisor && quotient[0] & 1 != 0))
            && increment(&mut quotient)
        {
            return Err(CameraError::Overflow);
        }
        Self::from_magnitude(quotient, negative)
    }

    /// Returns the nonnegative magnitude without wrapping the signed minimum.
    ///
    /// # Errors
    ///
    /// Returns [`CameraError::Overflow`] when the value is the signed minimum.
    pub fn abs_checked(&self) -> Result<Self, CameraError> {
        if self.is_negative() {
            self.neg()
        } else {
            Ok(*self)
        }
    }

    /// Returns the midpoint, dropping a half-lowest-bit result toward negative infinity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid width or checked intermediate arithmetic failure.
    pub fn midpoint_floor(&self, other: &Self) -> Result<Self, CameraError> {
        Self::validate_width()?;
        if let Ok(sum) = self.add(other) {
            return sum.shift_right(1);
        }
        let mut midpoint = self.shift_right(1)?.add(&other.shift_right(1)?)?;
        if self.limbs[0] & 1 != 0 && other.limbs[0] & 1 != 0 {
            let mut lowest_bit = [0; LIMBS];
            lowest_bit[0] = 1;
            midpoint = midpoint.add(&Self { limbs: lowest_bit })?;
        }
        Ok(midpoint)
    }

    /// Compares two values numerically.
    #[must_use]
    pub fn compare(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    /// Encodes the two's-complement limbs in least-significant-limb order.
    ///
    /// Each nested array is one little-endian limb, making the total fixed width `8 * LIMBS`
    /// bytes without unstable const-generic expressions.
    #[must_use]
    pub fn to_le_bytes(&self) -> [[u8; 8]; LIMBS] {
        let mut bytes = [[0; 8]; LIMBS];
        for (chunk, limb) in bytes.iter_mut().zip(self.limbs) {
            *chunk = limb.to_le_bytes();
        }
        bytes
    }

    /// Decodes nested little-endian limbs produced by [`Self::to_le_bytes`].
    #[must_use]
    pub fn from_le_bytes(bytes: [[u8; 8]; LIMBS]) -> Self {
        let mut limbs = [0; LIMBS];
        for (limb, chunk) in limbs.iter_mut().zip(bytes) {
            *limb = u64::from_le_bytes(chunk);
        }
        Self { limbs }
    }

    const fn validate_width() -> Result<(), CameraError> {
        if LIMBS == 0 {
            Err(CameraError::InvalidWidth)
        } else {
            Ok(())
        }
    }

    fn magnitude(&self) -> [u64; LIMBS] {
        let mut magnitude = self.limbs;
        if self.is_negative() {
            twos_complement(&mut magnitude);
        }
        magnitude
    }

    fn from_magnitude(mut magnitude: [u64; LIMBS], negative: bool) -> Result<Self, CameraError> {
        Self::validate_width()?;
        if magnitude.iter().all(|limb| *limb == 0) {
            return Ok(Self::ZERO);
        }
        let high = magnitude[LIMBS - 1];
        if negative {
            let lower_nonzero = magnitude[..LIMBS - 1].iter().any(|limb| *limb != 0);
            if high > SIGN_BIT || (high == SIGN_BIT && lower_nonzero) {
                return Err(CameraError::Overflow);
            }
            twos_complement(&mut magnitude);
        } else if high & SIGN_BIT != 0 {
            return Err(CameraError::Overflow);
        }
        Ok(Self { limbs: magnitude })
    }
}

#[derive(Clone, Copy)]
struct SignedWide<const LIMBS: usize, const PARTS: usize> {
    magnitude: [[u64; LIMBS]; PARTS],
    negative: bool,
}

#[derive(Clone, Copy)]
struct RatioDivisor<const LIMBS: usize> {
    magnitude: [u64; LIMBS],
    significant_limbs: usize,
    normalization_shift: u32,
    negative: bool,
}

impl<const LIMBS: usize> RatioDivisor<LIMBS> {
    fn new(divisor: &Fixed<LIMBS>) -> Result<Self, CameraError> {
        Fixed::<LIMBS>::validate_width()?;
        let magnitude = divisor.magnitude();
        let significant_limbs = magnitude
            .iter()
            .rposition(|limb| *limb != 0)
            .map_or(0, |index| index + 1);
        if significant_limbs == 0 {
            return Err(CameraError::Overflow);
        }
        let normalization_shift = magnitude[significant_limbs - 1].leading_zeros();
        Ok(Self {
            magnitude: normalize_magnitude(&magnitude, normalization_shift),
            significant_limbs,
            normalization_shift,
            negative: divisor.is_negative(),
        })
    }
}

impl<const LIMBS: usize, const PARTS: usize> SignedWide<LIMBS, PARTS> {
    const ZERO: Self = Self {
        magnitude: [[0; LIMBS]; PARTS],
        negative: false,
    };

    fn is_zero(&self) -> bool {
        self.magnitude.iter().flatten().all(|limb| *limb == 0)
    }
}

/// Solves a two-axis Gram projection with exact nested-limb intermediates.
///
/// Fixed inputs are treated as their signed raw integers. Five dot products, the determinant, both
/// numerators, and the scale denominator retain their complete products without rescaling. The
/// final rational values are each rounded directly to nearest-even binary64, so the result is
/// deterministic on native and wasm32.
///
/// # Errors
///
/// Returns an error for an invalid width, intermediate overflow, a nonpositive Gram determinant or
/// scale, or a quotient beyond binary64's finite range.
pub fn solve_two_axis_gram<const N: usize, const LIMBS: usize>(
    delta: &[Fixed<LIMBS>; N],
    horizontal_basis: &[Fixed<LIMBS>; N],
    vertical_basis: &[Fixed<LIMBS>; N],
    scale: &Fixed<LIMBS>,
) -> Result<[f64; 2], CameraError> {
    Fixed::<LIMBS>::validate_width()?;
    if scale.is_zero() || scale.is_negative() {
        return Err(CameraError::Overflow);
    }
    let horizontal_dot = exact_dot(delta, horizontal_basis)?;
    let vertical_dot = exact_dot(delta, vertical_basis)?;
    let horizontal_norm_squared = exact_dot(horizontal_basis, horizontal_basis)?;
    let cross_dot = exact_dot(horizontal_basis, vertical_basis)?;
    let vertical_norm_squared = exact_dot(vertical_basis, vertical_basis)?;

    let norm_product =
        multiply_signed_wide::<LIMBS, 2, 2, 4>(&horizontal_norm_squared, &vertical_norm_squared)?;
    let cross_squared = multiply_signed_wide::<LIMBS, 2, 2, 4>(&cross_dot, &cross_dot)?;
    let determinant = subtract_signed_wide(&norm_product, &cross_squared)?;
    if determinant.is_zero() || determinant.negative {
        return Err(CameraError::DegenerateFrame);
    }

    let horizontal_primary =
        multiply_signed_wide::<LIMBS, 2, 2, 4>(&horizontal_dot, &vertical_norm_squared)?;
    let horizontal_cross = multiply_signed_wide::<LIMBS, 2, 2, 4>(&vertical_dot, &cross_dot)?;
    let horizontal_numerator = subtract_signed_wide(&horizontal_primary, &horizontal_cross)?;
    let vertical_primary =
        multiply_signed_wide::<LIMBS, 2, 2, 4>(&vertical_dot, &horizontal_norm_squared)?;
    let vertical_cross = multiply_signed_wide::<LIMBS, 2, 2, 4>(&horizontal_dot, &cross_dot)?;
    let vertical_numerator = subtract_signed_wide(&vertical_primary, &vertical_cross)?;

    let scale_wide = fixed_as_wide(scale);
    let scaled_determinant = multiply_signed_wide::<LIMBS, 4, 1, 5>(&determinant, &scale_wide)?;
    if scaled_determinant.is_zero() || scaled_determinant.negative {
        return Err(CameraError::Overflow);
    }
    let fraction_words = LIMBS.checked_sub(1).ok_or(CameraError::InvalidWidth)?;
    let horizontal_scaled = shift_wide_words::<LIMBS, 4, 5>(&horizontal_numerator, fraction_words)?;
    let vertical_scaled = shift_wide_words::<LIMBS, 4, 5>(&vertical_numerator, fraction_words)?;
    Ok([
        divide_wide_to_f64(&horizontal_scaled, &scaled_determinant)?,
        divide_wide_to_f64(&vertical_scaled, &scaled_determinant)?,
    ])
}

impl<const LIMBS: usize> Default for Fixed<LIMBS> {
    fn default() -> Self {
        Self::ZERO
    }
}

impl<const LIMBS: usize> Ord for Fixed<LIMBS> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.is_negative(), other.is_negative()) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => self
                .limbs
                .iter()
                .zip(&other.limbs)
                .rev()
                .find_map(|(left, right)| (left != right).then_some(left.cmp(right)))
                .unwrap_or(Ordering::Equal),
        }
    }
}

impl<const LIMBS: usize> PartialOrd for Fixed<LIMBS> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn place_significand<const LIMBS: usize>(
    magnitude: &mut [u64; LIMBS],
    significand: u64,
    shift: usize,
) -> Result<(), CameraError> {
    let word = shift / LIMB_BITS;
    if word >= LIMBS {
        return Err(CameraError::Overflow);
    }
    let bits = shift % LIMB_BITS;
    magnitude[word] = significand << bits;
    if bits != 0 {
        let spill = significand >> (LIMB_BITS - bits);
        if spill != 0 {
            let next = word.checked_add(1).ok_or(CameraError::Overflow)?;
            if next >= LIMBS {
                return Err(CameraError::Overflow);
            }
            magnitude[next] = spill;
        }
    }
    Ok(())
}

fn round_u64_right(value: u64, shift: usize) -> u64 {
    if shift == 0 {
        return value;
    }
    if shift >= LIMB_BITS {
        return 0;
    }
    let retained = value >> shift;
    let round_bit = value & (1_u64 << (shift - 1)) != 0;
    let sticky_mask = (1_u64 << (shift - 1)) - 1;
    let sticky = value & sticky_mask != 0;
    retained + u64::from(round_bit && (sticky || retained & 1 != 0))
}

fn round_magnitude_right<const LIMBS: usize>(magnitude: &[u64; LIMBS], shift: usize) -> u64 {
    let Some(high_bit) = highest_set_bit(magnitude) else {
        return 0;
    };
    let mut retained = 0;
    if high_bit >= shift {
        for bit in (shift..=high_bit).rev() {
            retained = (retained << 1) | magnitude_bit(magnitude, bit);
        }
    }
    if shift == 0 {
        return retained;
    }
    let round_bit = magnitude_bit(magnitude, shift - 1) != 0;
    let sticky = any_magnitude_bits_below(magnitude, shift - 1);
    retained + u64::from(round_bit && (sticky || retained & 1 != 0))
}

fn magnitude_bit<const LIMBS: usize>(magnitude: &[u64; LIMBS], bit: usize) -> u64 {
    magnitude
        .get(bit / LIMB_BITS)
        .map_or(0, |limb| (limb >> (bit % LIMB_BITS)) & 1)
}

fn any_magnitude_bits_below<const LIMBS: usize>(
    magnitude: &[u64; LIMBS],
    exclusive_bit: usize,
) -> bool {
    let full_limbs = (exclusive_bit / LIMB_BITS).min(LIMBS);
    if magnitude[..full_limbs].iter().any(|limb| *limb != 0) {
        return true;
    }
    let partial_bits = exclusive_bit % LIMB_BITS;
    partial_bits != 0
        && magnitude.get(full_limbs).is_some_and(|limb| {
            let mask = (1_u64 << partial_bits) - 1;
            limb & mask != 0
        })
}

fn highest_set_bit<const LIMBS: usize>(magnitude: &[u64; LIMBS]) -> Option<usize> {
    let index = magnitude.iter().rposition(|limb| *limb != 0)?;
    let within = usize::try_from(u64::BITS - 1 - magnitude[index].leading_zeros()).ok()?;
    index.checked_mul(LIMB_BITS)?.checked_add(within)
}

fn fixed_as_wide<const LIMBS: usize>(value: &Fixed<LIMBS>) -> SignedWide<LIMBS, 1> {
    SignedWide {
        magnitude: [value.magnitude()],
        negative: value.is_negative(),
    }
}

fn exact_dot<const N: usize, const LIMBS: usize>(
    left: &[Fixed<LIMBS>; N],
    right: &[Fixed<LIMBS>; N],
) -> Result<SignedWide<LIMBS, 2>, CameraError> {
    let mut result = SignedWide::<LIMBS, 2>::ZERO;
    for (left_component, right_component) in left.iter().zip(right) {
        let left_wide = fixed_as_wide(left_component);
        let right_wide = fixed_as_wide(right_component);
        let product = multiply_signed_wide::<LIMBS, 1, 1, 2>(&left_wide, &right_wide)?;
        result = add_signed_wide(&result, &product)?;
    }
    Ok(result)
}

fn multiply_signed_wide<
    const LIMBS: usize,
    const LEFT_PARTS: usize,
    const RIGHT_PARTS: usize,
    const OUTPUT_PARTS: usize,
>(
    left: &SignedWide<LIMBS, LEFT_PARTS>,
    right: &SignedWide<LIMBS, RIGHT_PARTS>,
) -> Result<SignedWide<LIMBS, OUTPUT_PARTS>, CameraError> {
    let left_count = LIMBS
        .checked_mul(LEFT_PARTS)
        .ok_or(CameraError::InvalidWidth)?;
    let right_count = LIMBS
        .checked_mul(RIGHT_PARTS)
        .ok_or(CameraError::InvalidWidth)?;
    let output_count = LIMBS
        .checked_mul(OUTPUT_PARTS)
        .ok_or(CameraError::InvalidWidth)?;
    let mut output = SignedWide::<LIMBS, OUTPUT_PARTS>::ZERO;
    let mut left_index = 0;
    while left_index < left_count {
        let left_limb = wide_limb_value(&left.magnitude, left_index);
        let mut carry = 0_u128;
        let mut right_index = 0;
        while right_index < right_count {
            let output_index = left_index
                .checked_add(right_index)
                .ok_or(CameraError::InvalidWidth)?;
            let right_limb = wide_limb_value(&right.magnitude, right_index);
            if output_index >= output_count {
                if left_limb != 0 && right_limb != 0 {
                    return Err(CameraError::Overflow);
                }
            } else {
                let product = u128::from(left_limb) * u128::from(right_limb)
                    + u128::from(wide_limb_value(&output.magnitude, output_index))
                    + carry;
                set_wide_limb_value(&mut output.magnitude, output_index, low_u64(product)?);
                carry = product >> LIMB_BITS;
            }
            right_index += 1;
        }
        let mut output_index = left_index
            .checked_add(right_count)
            .ok_or(CameraError::InvalidWidth)?;
        while carry != 0 {
            if output_index >= output_count {
                return Err(CameraError::Overflow);
            }
            let sum = u128::from(wide_limb_value(&output.magnitude, output_index)) + carry;
            set_wide_limb_value(&mut output.magnitude, output_index, low_u64(sum)?);
            carry = sum >> LIMB_BITS;
            output_index = output_index
                .checked_add(1)
                .ok_or(CameraError::InvalidWidth)?;
        }
        left_index += 1;
    }
    output.negative = left.negative != right.negative && !output.is_zero();
    Ok(output)
}

fn add_signed_wide<const LIMBS: usize, const PARTS: usize>(
    left: &SignedWide<LIMBS, PARTS>,
    right: &SignedWide<LIMBS, PARTS>,
) -> Result<SignedWide<LIMBS, PARTS>, CameraError> {
    if left.negative == right.negative {
        let mut result = SignedWide::<LIMBS, PARTS>::ZERO;
        let mut carry = false;
        for ((output_part, left_part), right_part) in result
            .magnitude
            .iter_mut()
            .zip(left.magnitude)
            .zip(right.magnitude)
        {
            for ((output, left_limb), right_limb) in
                output_part.iter_mut().zip(left_part).zip(right_part)
            {
                let (partial, first_carry) = left_limb.overflowing_add(right_limb);
                let (sum, second_carry) = partial.overflowing_add(u64::from(carry));
                *output = sum;
                carry = first_carry || second_carry;
            }
        }
        if carry {
            return Err(CameraError::Overflow);
        }
        result.negative = left.negative && !result.is_zero();
        return Ok(result);
    }

    match compare_wide_magnitudes(&left.magnitude, &right.magnitude) {
        Ordering::Equal => Ok(SignedWide::ZERO),
        Ordering::Greater => Ok(SignedWide {
            magnitude: subtract_wide_magnitudes(&left.magnitude, &right.magnitude),
            negative: left.negative,
        }),
        Ordering::Less => Ok(SignedWide {
            magnitude: subtract_wide_magnitudes(&right.magnitude, &left.magnitude),
            negative: right.negative,
        }),
    }
}

fn subtract_signed_wide<const LIMBS: usize, const PARTS: usize>(
    left: &SignedWide<LIMBS, PARTS>,
    right: &SignedWide<LIMBS, PARTS>,
) -> Result<SignedWide<LIMBS, PARTS>, CameraError> {
    let mut negated = *right;
    if !negated.is_zero() {
        negated.negative = !negated.negative;
    }
    add_signed_wide(left, &negated)
}

fn compare_wide_magnitudes<const LIMBS: usize, const PARTS: usize>(
    left: &[[u64; LIMBS]; PARTS],
    right: &[[u64; LIMBS]; PARTS],
) -> Ordering {
    let Some(mut index) = LIMBS.checked_mul(PARTS) else {
        return Ordering::Equal;
    };
    while index != 0 {
        index -= 1;
        let left_limb = wide_limb_value(left, index);
        let right_limb = wide_limb_value(right, index);
        if left_limb != right_limb {
            return left_limb.cmp(&right_limb);
        }
    }
    Ordering::Equal
}

fn subtract_wide_magnitudes<const LIMBS: usize, const PARTS: usize>(
    minuend: &[[u64; LIMBS]; PARTS],
    subtrahend: &[[u64; LIMBS]; PARTS],
) -> [[u64; LIMBS]; PARTS] {
    let mut result = *minuend;
    let mut borrow = false;
    for (output, right) in result.iter_mut().flatten().zip(subtrahend.iter().flatten()) {
        let (partial, first_borrow) = output.overflowing_sub(*right);
        let (difference, second_borrow) = partial.overflowing_sub(u64::from(borrow));
        *output = difference;
        borrow = first_borrow || second_borrow;
    }
    result
}

fn shift_wide_words<const LIMBS: usize, const INPUT_PARTS: usize, const OUTPUT_PARTS: usize>(
    value: &SignedWide<LIMBS, INPUT_PARTS>,
    word_count: usize,
) -> Result<SignedWide<LIMBS, OUTPUT_PARTS>, CameraError> {
    let input_count = LIMBS
        .checked_mul(INPUT_PARTS)
        .ok_or(CameraError::InvalidWidth)?;
    let output_count = LIMBS
        .checked_mul(OUTPUT_PARTS)
        .ok_or(CameraError::InvalidWidth)?;
    let mut result = SignedWide::<LIMBS, OUTPUT_PARTS>::ZERO;
    let mut input_index = 0;
    while input_index < input_count {
        let limb = wide_limb_value(&value.magnitude, input_index);
        let output_index = input_index
            .checked_add(word_count)
            .ok_or(CameraError::InvalidWidth)?;
        if output_index >= output_count {
            if limb != 0 {
                return Err(CameraError::Overflow);
            }
        } else {
            set_wide_limb_value(&mut result.magnitude, output_index, limb);
        }
        input_index += 1;
    }
    result.negative = value.negative && !result.is_zero();
    Ok(result)
}

fn divide_wide_to_f64<const LIMBS: usize, const PARTS: usize>(
    numerator: &SignedWide<LIMBS, PARTS>,
    denominator: &SignedWide<LIMBS, PARTS>,
) -> Result<f64, CameraError> {
    if denominator.is_zero() {
        return Err(CameraError::DegenerateFrame);
    }
    let sign = if numerator.negative == denominator.negative {
        0
    } else {
        SIGN_BIT
    };
    if numerator.is_zero() {
        return Ok(f64::from_bits(sign));
    }
    let numerator_high = highest_wide_bit(&numerator.magnitude).ok_or(CameraError::Overflow)?;
    let mut position = i64::try_from(numerator_high).map_err(|_| CameraError::InvalidWidth)?;
    let mut remainder = [[0; LIMBS]; PARTS];
    let mut binary_exponent = None;
    let mut retained_target = None;
    let mut retained_count = 0;
    let mut significand = 0_u64;

    loop {
        let input_bit = if position < 0 {
            0
        } else {
            let bit = usize::try_from(position).map_err(|_| CameraError::InvalidWidth)?;
            wide_magnitude_bit(&numerator.magnitude, bit)
        };
        let quotient_bit =
            wide_ratio_quotient_bit(&mut remainder, input_bit, &denominator.magnitude);
        if binary_exponent.is_none() && quotient_bit != 0 {
            if position > F64_EXPONENT_BIAS {
                return Err(CameraError::Overflow);
            }
            let half_subnormal = F64_SUBNORMAL_EXPONENT - 1;
            match position.cmp(&half_subnormal) {
                Ordering::Less => return Ok(f64::from_bits(sign)),
                Ordering::Equal => {
                    let rounds_up =
                        wide_ratio_tail_exists(&remainder, &numerator.magnitude, position)?;
                    return Ok(f64::from_bits(sign | u64::from(rounds_up)));
                }
                Ordering::Greater => {}
            }
            binary_exponent = Some(position);
            retained_target = Some(if position >= F64_MINIMUM_NORMAL_EXPONENT {
                F64_SIGNIFICAND_BITS
            } else {
                let subnormal_bits = position
                    .checked_sub(F64_SUBNORMAL_EXPONENT)
                    .and_then(|difference| difference.checked_add(1))
                    .ok_or(CameraError::InvalidWidth)?;
                usize::try_from(subnormal_bits).map_err(|_| CameraError::InvalidWidth)?
            });
        }
        if let Some(target) = retained_target {
            if retained_count < target {
                significand = (significand << 1) | quotient_bit;
                retained_count += 1;
            } else {
                let sticky = wide_ratio_tail_exists(&remainder, &numerator.magnitude, position)?;
                if quotient_bit != 0 && (sticky || significand & 1 != 0) {
                    significand += 1;
                }
                return encode_f64_ratio(sign, binary_exponent, significand);
            }
        }
        position = position.checked_sub(1).ok_or(CameraError::InvalidWidth)?;
    }
}

fn divide_product_to_fixed<const LIMBS: usize>(
    numerator: &SignedWide<LIMBS, 2>,
    denominator: &RatioDivisor<LIMBS>,
) -> Result<Fixed<LIMBS>, CameraError> {
    if numerator.is_zero() {
        return Ok(Fixed::ZERO);
    }
    let (mut dividend, numerator_limbs) =
        normalize_product(&numerator.magnitude, denominator.normalization_shift)?;
    let divisor_limbs = denominator.significant_limbs;
    let mut quotient = [0; LIMBS];
    // A normalized high divisor limb makes each trial a base-2^64 quotient digit. This replaces
    // the prior quotient-bit walk while retaining the complete product and remainder for the one
    // nearest-even rounding below.
    if numerator_limbs >= divisor_limbs {
        let mut quotient_index = numerator_limbs - divisor_limbs + 1;
        while quotient_index != 0 {
            quotient_index -= 1;
            let mut digit = trial_quotient(&dividend, quotient_index, denominator)?;
            if subtract_quotient_digit(&mut dividend, quotient_index, denominator, digit)? {
                digit = digit.checked_sub(1).ok_or(CameraError::Overflow)?;
                add_divisor_back(&mut dividend, quotient_index, denominator);
            }
            if quotient_index < LIMBS {
                quotient[quotient_index] = digit;
            } else if digit != 0 {
                return Err(CameraError::Overflow);
            }
        }
    }
    let remainder_order = compare_twice_remainder(&dividend, denominator);
    let rounds_up = remainder_order == Ordering::Greater
        || (remainder_order == Ordering::Equal && quotient[0] & 1 != 0);
    if rounds_up && increment(&mut quotient) {
        return Err(CameraError::Overflow);
    }
    Fixed::from_magnitude(quotient, numerator.negative != denominator.negative)
}

fn normalize_magnitude<const LIMBS: usize>(input: &[u64; LIMBS], shift: u32) -> [u64; LIMBS] {
    if shift == 0 {
        return *input;
    }
    let mut output = [0; LIMBS];
    let mut carry = 0;
    for (result, limb) in output.iter_mut().zip(input) {
        *result = (*limb << shift) | carry;
        carry = *limb >> (u64::BITS - shift);
    }
    debug_assert_eq!(carry, 0);
    output
}

fn normalize_product<const LIMBS: usize>(
    input: &[[u64; LIMBS]; 2],
    shift: u32,
) -> Result<([[u64; LIMBS]; 4], usize), CameraError> {
    let input_limbs = LIMBS.checked_mul(2).ok_or(CameraError::InvalidWidth)?;
    let mut output = [[0; LIMBS]; 4];
    let mut carry = 0;
    let mut index = 0;
    while index < input_limbs {
        let limb = wide_limb_value(input, index);
        let shifted = if shift == 0 {
            limb
        } else {
            (limb << shift) | carry
        };
        set_wide_limb_value(&mut output, index, shifted);
        carry = if shift == 0 {
            0
        } else {
            limb >> (u64::BITS - shift)
        };
        index += 1;
    }
    if carry != 0 {
        set_wide_limb_value(&mut output, input_limbs, carry);
    }
    let capacity = LIMBS.checked_mul(4).ok_or(CameraError::InvalidWidth)?;
    let mut significant_limbs = capacity;
    while significant_limbs != 0 && wide_limb_value(&output, significant_limbs - 1) == 0 {
        significant_limbs -= 1;
    }
    Ok((output, significant_limbs))
}

fn trial_quotient<const LIMBS: usize>(
    dividend: &[[u64; LIMBS]; 4],
    quotient_index: usize,
    denominator: &RatioDivisor<LIMBS>,
) -> Result<u64, CameraError> {
    let divisor_limbs = denominator.significant_limbs;
    let divisor_high = denominator.magnitude[divisor_limbs - 1];
    let dividend_high = wide_limb_value(dividend, quotient_index + divisor_limbs);
    let dividend_next = wide_limb_value(dividend, quotient_index + divisor_limbs - 1);
    if dividend_high > divisor_high {
        return Err(CameraError::Overflow);
    }
    let (mut trial, mut remainder, mut remainder_overflow) = if dividend_high == divisor_high {
        let (remainder, overflow) = dividend_next.overflowing_add(divisor_high);
        (u64::MAX, remainder, overflow)
    } else {
        let pair = (u128::from(dividend_high) << u64::BITS) | u128::from(dividend_next);
        (
            u64::try_from(pair / u128::from(divisor_high)).map_err(|_| CameraError::Overflow)?,
            u64::try_from(pair % u128::from(divisor_high)).map_err(|_| CameraError::Overflow)?,
            false,
        )
    };
    if divisor_limbs == 1 {
        return Ok(trial);
    }
    let dividend_low = wide_limb_value(dividend, quotient_index + divisor_limbs - 2);
    let divisor_next = denominator.magnitude[divisor_limbs - 2];
    while !remainder_overflow
        && u128::from(trial) * u128::from(divisor_next)
            > ((u128::from(remainder) << u64::BITS) | u128::from(dividend_low))
    {
        trial = trial.checked_sub(1).ok_or(CameraError::Overflow)?;
        (remainder, remainder_overflow) = remainder.overflowing_add(divisor_high);
    }
    Ok(trial)
}

fn subtract_quotient_digit<const LIMBS: usize>(
    dividend: &mut [[u64; LIMBS]; 4],
    quotient_index: usize,
    denominator: &RatioDivisor<LIMBS>,
    quotient_digit: u64,
) -> Result<bool, CameraError> {
    let mut borrow = 0_u64;
    for (index, divisor_limb) in denominator.magnitude[..denominator.significant_limbs]
        .iter()
        .copied()
        .enumerate()
    {
        let product = u128::from(quotient_digit) * u128::from(divisor_limb) + u128::from(borrow);
        let product_low = low_u64(product)?;
        let product_high =
            u64::try_from(product >> u64::BITS).map_err(|_| CameraError::Overflow)?;
        let position = quotient_index + index;
        let current = wide_limb_value(dividend, position);
        let (difference, underflow) = current.overflowing_sub(product_low);
        set_wide_limb_value(dividend, position, difference);
        borrow = product_high
            .checked_add(u64::from(underflow))
            .ok_or(CameraError::Overflow)?;
    }
    let high_position = quotient_index + denominator.significant_limbs;
    let high = wide_limb_value(dividend, high_position);
    let (difference, underflow) = high.overflowing_sub(borrow);
    set_wide_limb_value(dividend, high_position, difference);
    Ok(underflow)
}

fn add_divisor_back<const LIMBS: usize>(
    dividend: &mut [[u64; LIMBS]; 4],
    quotient_index: usize,
    denominator: &RatioDivisor<LIMBS>,
) {
    let mut carry = false;
    for (index, divisor_limb) in denominator.magnitude[..denominator.significant_limbs]
        .iter()
        .copied()
        .enumerate()
    {
        let position = quotient_index + index;
        let current = wide_limb_value(dividend, position);
        let (partial, first_carry) = current.overflowing_add(divisor_limb);
        let (sum, second_carry) = partial.overflowing_add(u64::from(carry));
        set_wide_limb_value(dividend, position, sum);
        carry = first_carry || second_carry;
    }
    let high_position = quotient_index + denominator.significant_limbs;
    let high = wide_limb_value(dividend, high_position);
    set_wide_limb_value(dividend, high_position, high.wrapping_add(u64::from(carry)));
}

fn compare_twice_remainder<const LIMBS: usize>(
    dividend: &[[u64; LIMBS]; 4],
    denominator: &RatioDivisor<LIMBS>,
) -> Ordering {
    let mut doubled = [0; LIMBS];
    let mut carry = 0;
    for (index, output) in doubled[..denominator.significant_limbs]
        .iter_mut()
        .enumerate()
    {
        let remainder = wide_limb_value(dividend, index);
        *output = (remainder << 1) | carry;
        carry = remainder >> (u64::BITS - 1);
    }
    if carry != 0 {
        return Ordering::Greater;
    }
    for (left, right) in doubled[..denominator.significant_limbs]
        .iter()
        .zip(&denominator.magnitude[..denominator.significant_limbs])
        .rev()
    {
        if left != right {
            return left.cmp(right);
        }
    }
    Ordering::Equal
}

#[cfg(test)]
fn divide_wide_to_fixed_bitwise<const LIMBS: usize, const PARTS: usize>(
    numerator: &SignedWide<LIMBS, PARTS>,
    denominator: &SignedWide<LIMBS, PARTS>,
) -> Result<Fixed<LIMBS>, CameraError> {
    if denominator.is_zero() {
        return Err(CameraError::DegenerateFrame);
    }
    if numerator.is_zero() {
        return Ok(Fixed::ZERO);
    }
    let numerator_high = highest_wide_bit(&numerator.magnitude).ok_or(CameraError::Overflow)?;
    let quotient_bits = LIMBS
        .checked_mul(LIMB_BITS)
        .ok_or(CameraError::InvalidWidth)?;
    let mut remainder = [[0; LIMBS]; PARTS];
    let mut quotient = [0; LIMBS];
    let mut position = numerator_high;
    loop {
        let input_bit = wide_magnitude_bit(&numerator.magnitude, position);
        let quotient_bit =
            wide_ratio_quotient_bit(&mut remainder, input_bit, &denominator.magnitude);
        if quotient_bit != 0 {
            if position >= quotient_bits {
                return Err(CameraError::Overflow);
            }
            quotient[position / LIMB_BITS] |= 1_u64 << (position % LIMB_BITS);
        }
        if position == 0 {
            break;
        }
        position -= 1;
    }
    let above_or_at_half = wide_ratio_quotient_bit(&mut remainder, 0, &denominator.magnitude) != 0;
    let above_half = above_or_at_half && remainder.iter().flatten().any(|limb| *limb != 0);
    let tie_rounds_up = above_or_at_half && !above_half && quotient[0] & 1 != 0;
    if (above_half || tie_rounds_up) && increment(&mut quotient) {
        return Err(CameraError::Overflow);
    }
    Fixed::from_magnitude(quotient, numerator.negative != denominator.negative)
}

fn encode_f64_ratio(
    sign: u64,
    binary_exponent: Option<i64>,
    mut significand: u64,
) -> Result<f64, CameraError> {
    let mut exponent = binary_exponent.ok_or(CameraError::Overflow)?;
    if exponent < F64_MINIMUM_NORMAL_EXPONENT {
        return Ok(f64::from_bits(sign | significand));
    }
    if significand == 1_u64 << F64_SIGNIFICAND_BITS {
        significand >>= 1;
        exponent = exponent.checked_add(1).ok_or(CameraError::Overflow)?;
    }
    if exponent > F64_EXPONENT_BIAS {
        return Err(CameraError::Overflow);
    }
    let biased = u64::try_from(exponent + F64_EXPONENT_BIAS).map_err(|_| CameraError::Overflow)?;
    let fraction_mask = (1_u64 << F64_FRACTION_BITS) - 1;
    Ok(f64::from_bits(
        sign | (biased << F64_FRACTION_BITS) | (significand & fraction_mask),
    ))
}

fn wide_ratio_quotient_bit<const LIMBS: usize, const PARTS: usize>(
    remainder: &mut [[u64; LIMBS]; PARTS],
    input_bit: u64,
    denominator: &[[u64; LIMBS]; PARTS],
) -> u64 {
    let mut carry = input_bit;
    for limb in remainder.iter_mut().flatten() {
        let next_carry = *limb >> (u64::BITS - 1);
        *limb = (*limb << 1) | carry;
        carry = next_carry;
    }
    if carry != 0 || compare_wide_magnitudes(remainder, denominator) != Ordering::Less {
        *remainder = subtract_wide_magnitudes(remainder, denominator);
        1
    } else {
        0
    }
}

fn wide_ratio_tail_exists<const LIMBS: usize, const PARTS: usize>(
    remainder: &[[u64; LIMBS]; PARTS],
    numerator: &[[u64; LIMBS]; PARTS],
    position: i64,
) -> Result<bool, CameraError> {
    if remainder.iter().flatten().any(|limb| *limb != 0) {
        return Ok(true);
    }
    if position <= 0 {
        return Ok(false);
    }
    let exclusive_bit = usize::try_from(position).map_err(|_| CameraError::InvalidWidth)?;
    Ok(any_wide_bits_below(numerator, exclusive_bit))
}

fn highest_wide_bit<const LIMBS: usize, const PARTS: usize>(
    magnitude: &[[u64; LIMBS]; PARTS],
) -> Option<usize> {
    let limb_count = LIMBS.checked_mul(PARTS)?;
    let limb_index = (0..limb_count)
        .rev()
        .find(|index| wide_limb_value(magnitude, *index) != 0)?;
    let limb = wide_limb_value(magnitude, limb_index);
    let within = usize::try_from(u64::BITS - 1 - limb.leading_zeros()).ok()?;
    limb_index.checked_mul(LIMB_BITS)?.checked_add(within)
}

const fn wide_magnitude_bit<const LIMBS: usize, const PARTS: usize>(
    magnitude: &[[u64; LIMBS]; PARTS],
    bit: usize,
) -> u64 {
    let limb = bit / LIMB_BITS;
    let Some(limb_count) = LIMBS.checked_mul(PARTS) else {
        return 0;
    };
    if limb >= limb_count {
        0
    } else {
        (wide_limb_value(magnitude, limb) >> (bit % LIMB_BITS)) & 1
    }
}

const fn any_wide_bits_below<const LIMBS: usize, const PARTS: usize>(
    magnitude: &[[u64; LIMBS]; PARTS],
    exclusive_bit: usize,
) -> bool {
    let Some(limb_count) = LIMBS.checked_mul(PARTS) else {
        return true;
    };
    let available_limbs = exclusive_bit / LIMB_BITS;
    let full_limbs = if available_limbs < limb_count {
        available_limbs
    } else {
        limb_count
    };
    let mut index = 0;
    while index < full_limbs {
        if wide_limb_value(magnitude, index) != 0 {
            return true;
        }
        index += 1;
    }
    let partial_bits = exclusive_bit % LIMB_BITS;
    if partial_bits == 0 || full_limbs >= limb_count {
        return false;
    }
    let mask = (1_u64 << partial_bits) - 1;
    wide_limb_value(magnitude, full_limbs) & mask != 0
}

const fn wide_limb_value<const LIMBS: usize, const PARTS: usize>(
    magnitude: &[[u64; LIMBS]; PARTS],
    index: usize,
) -> u64 {
    magnitude[index / LIMBS][index % LIMBS]
}

const fn set_wide_limb_value<const LIMBS: usize, const PARTS: usize>(
    magnitude: &mut [[u64; LIMBS]; PARTS],
    index: usize,
    value: u64,
) {
    magnitude[index / LIMBS][index % LIMBS] = value;
}

fn twos_complement<const LIMBS: usize>(limbs: &mut [u64; LIMBS]) {
    let mut carry = true;
    for limb in limbs {
        let (value, next_carry) = (!*limb).overflowing_add(u64::from(carry));
        *limb = value;
        carry = next_carry;
    }
}

fn multiply_wide<const LIMBS: usize>(
    left: &[u64; LIMBS],
    right: &[u64; LIMBS],
) -> Result<([u64; LIMBS], [u64; LIMBS]), CameraError> {
    let mut low = [0; LIMBS];
    let mut high = [0; LIMBS];
    for (left_index, left_limb) in left.iter().copied().enumerate() {
        let mut carry = 0_u128;
        for (right_index, right_limb) in right.iter().copied().enumerate() {
            let index = left_index + right_index;
            let product = u128::from(left_limb) * u128::from(right_limb)
                + u128::from(wide_limb(&low, &high, index))
                + carry;
            set_wide_limb(&mut low, &mut high, index, low_u64(product)?);
            carry = product >> LIMB_BITS;
        }
        if carry != 0 {
            set_wide_limb(&mut low, &mut high, left_index + LIMBS, low_u64(carry)?);
        }
    }
    Ok((low, high))
}

fn wide_limb<const LIMBS: usize>(low: &[u64; LIMBS], high: &[u64; LIMBS], index: usize) -> u64 {
    if index < LIMBS {
        low[index]
    } else {
        high[index - LIMBS]
    }
}

fn set_wide_limb<const LIMBS: usize>(
    low: &mut [u64; LIMBS],
    high: &mut [u64; LIMBS],
    index: usize,
    value: u64,
) {
    if index < LIMBS {
        low[index] = value;
    } else {
        high[index - LIMBS] = value;
    }
}

fn low_u64(value: u128) -> Result<u64, CameraError> {
    u64::try_from(value & u128::from(u64::MAX)).map_err(|_| CameraError::Overflow)
}

fn increment<const LIMBS: usize>(limbs: &mut [u64; LIMBS]) -> bool {
    for limb in limbs {
        let (value, carry) = limb.overflowing_add(1);
        *limb = value;
        if !carry {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{
        CameraError, Fixed, divide_wide_to_f64, divide_wide_to_fixed_bitwise, fixed_as_wide,
        multiply_signed_wide, shift_wide_words,
    };
    use core::cmp::Ordering;

    type TestFixed = Fixed<2>;

    fn raw(low: u64, high: u64) -> TestFixed {
        TestFixed::from_le_bytes([low.to_le_bytes(), high.to_le_bytes()])
    }

    fn ratio(left: &TestFixed, right: &TestFixed) -> Result<f64, CameraError> {
        divide_wide_to_f64(&fixed_as_wide(left), &fixed_as_wide(right))
    }

    fn bitwise_mul_ratio(
        value: &TestFixed,
        multiplier: &TestFixed,
        divisor: &TestFixed,
    ) -> Result<TestFixed, CameraError> {
        if divisor.is_zero() {
            return Err(CameraError::Overflow);
        }
        let product =
            multiply_signed_wide::<2, 1, 1, 2>(&fixed_as_wide(value), &fixed_as_wide(multiplier))?;
        let denominator = shift_wide_words::<2, 1, 2>(&fixed_as_wide(divisor), 0)?;
        divide_wide_to_fixed_bitwise(&product, &denominator)
    }

    #[test]
    fn addition_carries_and_refuses_signed_overflow() -> Result<(), CameraError> {
        let low_max = raw(u64::MAX, 0);
        let lowest_bit = raw(1, 0);
        assert_eq!(low_max.add(&lowest_bit)?, TestFixed::from_i64(1)?);
        let largest = raw(u64::MAX, i64::MAX.unsigned_abs());
        assert_eq!(largest.add(&lowest_bit), Err(CameraError::Overflow));
        Ok(())
    }

    #[test]
    fn subtraction_negation_and_comparison_preserve_sign() -> Result<(), CameraError> {
        let two = TestFixed::from_i64(2)?;
        let negative_two = two.neg()?;
        assert_eq!(negative_two.to_f64()?, -2.0);
        assert_eq!(negative_two.compare(&two), Ordering::Less);
        assert_eq!(two.sub(&negative_two)?, TestFixed::from_i64(4)?);
        let minimum = raw(0, i64::MIN.unsigned_abs());
        assert_eq!(minimum.neg(), Err(CameraError::Overflow));
        Ok(())
    }

    #[test]
    fn shifts_are_checked_and_arithmetic() -> Result<(), CameraError> {
        let one = TestFixed::from_i64(1)?;
        assert_eq!(one.shift_left(2)?, TestFixed::from_i64(4)?);
        assert_eq!(TestFixed::from_i64(-3)?.shift_right(1)?.to_f64()?, -1.5);
        assert_eq!(
            TestFixed::from_i64(i64::MAX)?.shift_left(1),
            Err(CameraError::Overflow)
        );
        assert_eq!(
            TestFixed::from_i64(-1)?.shift_right(128)?,
            raw(u64::MAX, u64::MAX)
        );
        Ok(())
    }

    #[test]
    fn small_multiplication_is_exact_and_checked() -> Result<(), CameraError> {
        let value = TestFixed::from_f64(-1.25)?;
        assert_eq!(value.mul_small(-4)?, TestFixed::from_i64(5)?);
        assert_eq!(
            TestFixed::from_i64(i64::MAX)?.mul_small(2),
            Err(CameraError::Overflow)
        );
        Ok(())
    }

    #[test]
    fn full_multiplication_rounds_half_to_even() -> Result<(), CameraError> {
        let half = raw(1_u64 << 63, 0);
        assert_eq!(raw(1, 0).mul(&half)?, TestFixed::ZERO);
        assert_eq!(raw(3, 0).mul(&half)?, raw(2, 0));
        assert_eq!(
            TestFixed::from_f64(-1.5)?.mul(&TestFixed::from_f64(2.0)?)?,
            TestFixed::from_i64(-3)?
        );
        assert_eq!(
            TestFixed::from_i64(i64::MAX)?.mul(&TestFixed::from_i64(2)?),
            Err(CameraError::Overflow)
        );
        assert_eq!(
            ratio(&raw((1_u64 << 53) + 1, 0), &raw(1_u64 << 53, 0))?.to_bits(),
            1.0_f64.to_bits()
        );
        assert_eq!(
            ratio(&raw((1_u64 << 53) + 3, 0), &raw(1_u64 << 53, 0))?.to_bits(),
            0x3ff0_0000_0000_0002
        );
        assert_eq!(
            ratio(&TestFixed::from_i64(1)?, &TestFixed::from_i64(3)?)?.to_bits(),
            0x3fd5_5555_5555_5555
        );
        assert_eq!(
            ratio(&TestFixed::from_i64(1)?, &TestFixed::ZERO),
            Err(CameraError::DegenerateFrame)
        );
        Ok(())
    }

    #[test]
    fn ratio_multiplication_rounds_once_with_even_ties() -> Result<(), CameraError> {
        let lowest_bit = raw(1, 0);
        let three_lowest_bits = raw(3, 0);
        let one = TestFixed::from_i64(1)?;
        let two = TestFixed::from_i64(2)?;
        assert_eq!(
            lowest_bit.mul_ratio_round_even(&one, &two)?,
            TestFixed::ZERO
        );
        assert_eq!(
            three_lowest_bits.mul_ratio_round_even(&one, &two)?,
            raw(2, 0)
        );
        assert_eq!(
            raw(5, 0).mul_ratio_round_even(&raw(3, 0), &raw(2, 0))?,
            raw(8, 0)
        );
        assert_eq!(
            TestFixed::from_f64(-1.5)?.mul_ratio_round_even(&two, &one)?,
            TestFixed::from_i64(-3)?
        );
        assert_eq!(
            one.mul_ratio_round_even(&one, &TestFixed::ZERO),
            Err(CameraError::Overflow)
        );
        Ok(())
    }

    #[test]
    fn word_division_matches_the_bitwise_ratio_oracle() -> Result<(), CameraError> {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for _ in 0..2_048 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut value = raw(state, state.rotate_left(17) & 0xf);
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut multiplier = raw(state, (state.rotate_left(29) & 0x3) + 1);
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let mut divisor = raw(state, (state.rotate_left(41) & 0x7) + 1);
            if state & 1 != 0 {
                value = value.neg()?;
            }
            if state & 2 != 0 {
                multiplier = multiplier.neg()?;
            }
            if state & 4 != 0 {
                divisor = divisor.neg()?;
            }
            assert_eq!(
                value.mul_ratio_round_even(&multiplier, &divisor),
                bitwise_mul_ratio(&value, &multiplier, &divisor)
            );
        }
        Ok(())
    }

    #[test]
    fn float_boundaries_are_lossy_only_beyond_the_carried_bits() -> Result<(), CameraError> {
        for value in [
            0.0,
            -0.0,
            1.0,
            -2.5,
            core::f64::consts::PI,
            f64::from_bits(1),
        ] {
            let fixed = TestFixed::from_f64(value)?;
            let expected = if value.abs() < 2.0_f64.powi(-64) {
                0.0
            } else {
                value
            };
            assert_eq!(fixed.to_f64()?, expected);
        }
        assert_eq!(TestFixed::from_f64(f64::NAN), Err(CameraError::NonFinite));
        assert_eq!(
            TestFixed::from_f64(f64::INFINITY),
            Err(CameraError::NonFinite)
        );
        assert_eq!(
            TestFixed::from_f64(2.0_f64.powi(63)),
            Err(CameraError::Overflow)
        );
        Ok(())
    }

    #[test]
    fn leading_bit_and_byte_encoding_are_canonical() -> Result<(), CameraError> {
        let value = TestFixed::from_f64(-2.5)?;
        assert_eq!(value.leading_bit()?, Some(1));
        assert_eq!(TestFixed::ZERO.leading_bit()?, None);
        assert_eq!(TestFixed::from_le_bytes(value.to_le_bytes()), value);
        Ok(())
    }

    #[test]
    fn midpoint_drops_low_bit_toward_negative_infinity() -> Result<(), CameraError> {
        let lowest_bit = raw(1, 0);
        let negative_lowest_bit = raw(u64::MAX, u64::MAX);
        assert_eq!(
            TestFixed::ZERO.midpoint_floor(&lowest_bit)?,
            TestFixed::ZERO
        );
        assert_eq!(
            TestFixed::ZERO.midpoint_floor(&negative_lowest_bit)?,
            negative_lowest_bit
        );
        Ok(())
    }

    #[test]
    fn zero_limb_operations_are_typed_refusals() {
        assert_eq!(Fixed::<0>::from_i64(0), Err(CameraError::InvalidWidth));
        assert_eq!(Fixed::<0>::ZERO.to_f64(), Err(CameraError::InvalidWidth));
    }
}
