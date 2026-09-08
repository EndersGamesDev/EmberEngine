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
/// One complete 64-bit limb gives camera centres a signed range of ±2⁶³ plane units. Every
/// additional limb is fractional precision, so the first consumer's eight limbs retain 448
/// fractional bits without making that width a ceiling for other consumers.
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
    use super::{CameraError, Fixed};
    use core::cmp::Ordering;

    type TestFixed = Fixed<2>;

    fn raw(low: u64, high: u64) -> TestFixed {
        TestFixed::from_le_bytes([low.to_le_bytes(), high.to_le_bytes()])
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
