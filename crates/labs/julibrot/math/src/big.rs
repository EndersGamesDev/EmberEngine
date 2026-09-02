use astro_float::{BigFloat, Consts, Radix, RoundingMode, Sign};

use crate::MathError;

#[derive(Clone, Debug, PartialEq)]
pub struct BigScalar {
    pub(crate) value: BigFloat,
}

impl BigScalar {
    pub fn from_f64(value: f64, precision_bits: u32) -> Result<Self, MathError> {
        if !value.is_finite() || precision_bits == 0 {
            return Err(MathError::NonFinite);
        }
        Self::checked(BigFloat::from_f64(value, precision_bits as usize))
    }

    pub fn from_f32(value: f32, precision_bits: u32) -> Result<Self, MathError> {
        if !value.is_finite() || precision_bits == 0 {
            return Err(MathError::NonFinite);
        }
        Self::checked(BigFloat::from_f32(value, precision_bits as usize))
    }

    pub fn zero(precision_bits: u32) -> Result<Self, MathError> {
        if precision_bits == 0 {
            return Err(MathError::InvalidCentreEncoding);
        }
        Self::checked(BigFloat::new(precision_bits as usize))
    }

    pub(crate) fn checked(value: BigFloat) -> Result<Self, MathError> {
        if value.is_nan() || value.is_inf() {
            Err(MathError::BigFloat)
        } else {
            Ok(Self { value })
        }
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.value.is_zero()
    }

    pub fn precision_bits(&self) -> Result<u32, MathError> {
        let precision = self.value.precision().ok_or(MathError::BigFloat)?;
        u32::try_from(precision).map_err(|_| MathError::CounterOverflow)
    }

    pub fn to_f64(&self) -> Result<f64, MathError> {
        let encoded = encode_big_scalar(self)?;
        Ok(f64::from_bits(round_dyadic(
            &encoded,
            FloatFormat {
                precision: 53,
                fraction_bits: 52,
                minimum_normal_exponent: -1022,
                minimum_subnormal_exponent: -1074,
                maximum_exponent: 1023,
                exponent_bias: 1023,
                sign_shift: 63,
            },
        )?))
    }

    pub fn to_f32(&self) -> Result<f32, MathError> {
        let encoded = encode_big_scalar(self)?;
        let bits = u32::try_from(round_dyadic(
            &encoded,
            FloatFormat {
                precision: 24,
                fraction_bits: 23,
                minimum_normal_exponent: -126,
                minimum_subnormal_exponent: -149,
                maximum_exponent: 127,
                exponent_bias: 127,
                sign_shift: 31,
            },
        )?)
        .map_err(|_| MathError::CounterOverflow)?;
        Ok(f32::from_bits(bits))
    }

    pub(crate) fn with_precision(&self, precision_bits: u32) -> Result<Self, MathError> {
        if precision_bits == 0 {
            return Err(MathError::InvalidCentreEncoding);
        }
        let mut value = self.value.clone();
        value
            .set_precision(precision_bits as usize, RoundingMode::ToEven)
            .map_err(|_| MathError::BigFloat)?;
        Self::checked(value)
    }

    pub(crate) fn add(&self, other: &Self, precision_bits: u32) -> Result<Self, MathError> {
        Self::checked(self.value.add(
            &other.value,
            precision_bits as usize,
            RoundingMode::ToEven,
        ))
    }

    pub(crate) fn sub(&self, other: &Self, precision_bits: u32) -> Result<Self, MathError> {
        Self::checked(self.value.sub(
            &other.value,
            precision_bits as usize,
            RoundingMode::ToEven,
        ))
    }

    pub(crate) fn mul(&self, other: &Self, precision_bits: u32) -> Result<Self, MathError> {
        Self::checked(self.value.mul(
            &other.value,
            precision_bits as usize,
            RoundingMode::ToEven,
        ))
    }

    pub(crate) fn div(&self, other: &Self, precision_bits: u32) -> Result<Self, MathError> {
        Self::checked(self.value.div(
            &other.value,
            precision_bits as usize,
            RoundingMode::ToEven,
        ))
    }

    pub(crate) fn scale_pow2(&self, shift: i32) -> Result<Self, MathError> {
        if self.is_zero() {
            return Ok(self.clone());
        }
        let mut value = self.value.clone();
        let exponent = value.exponent().ok_or(MathError::BigFloat)?;
        value.set_exponent(
            exponent
                .checked_add(shift)
                .ok_or(MathError::ScaleExponentOverflow)?,
        );
        Self::checked(value)
    }

    pub(crate) fn compare(&self, other: &Self) -> Result<i8, MathError> {
        let ordering = self.value.cmp(&other.value).ok_or(MathError::BigFloat)?;
        Ok(ordering.signum() as i8)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BigCentre {
    pub coords: [BigScalar; 4],
    pub precision_bits: u32,
}

impl BigCentre {
    pub fn from_f64(coords: [f64; 4], precision_bits: u32) -> Result<Self, MathError> {
        let [a, b, c, d] = coords;
        Ok(Self {
            coords: [
                BigScalar::from_f64(a, precision_bits)?,
                BigScalar::from_f64(b, precision_bits)?,
                BigScalar::from_f64(c, precision_bits)?,
                BigScalar::from_f64(d, precision_bits)?,
            ],
            precision_bits,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedBigScalar {
    pub sign: u32,
    pub exponent: i32,
    pub limbs: Vec<u32>,
}

#[derive(Clone, Copy)]
struct FloatFormat {
    precision: u32,
    fraction_bits: u32,
    minimum_normal_exponent: i64,
    minimum_subnormal_exponent: i64,
    maximum_exponent: i64,
    exponent_bias: i64,
    sign_shift: u32,
}

fn round_dyadic(encoded: &EncodedBigScalar, format: FloatFormat) -> Result<u64, MathError> {
    if encoded.limbs.is_empty() {
        return Ok(0);
    }
    let high = *encoded
        .limbs
        .last()
        .ok_or(MathError::InvalidCentreEncoding)?;
    let word_count = i64::try_from(encoded.limbs.len() - 1)
        .map_err(|_| MathError::CounterOverflow)?;
    let bit_length = word_count
        .checked_mul(32)
        .and_then(|bits| bits.checked_add(i64::from(32 - high.leading_zeros())))
        .ok_or(MathError::CounterOverflow)?;
    let mut high_exponent = i64::from(encoded.exponent)
        .checked_add(bit_length - 1)
        .ok_or(MathError::ScaleExponentOverflow)?;
    if high_exponent > format.maximum_exponent {
        return Err(MathError::NonFinite);
    }
    let normal = high_exponent >= format.minimum_normal_exponent;
    let unit_exponent = if normal {
        high_exponent - i64::from(format.precision - 1)
    } else {
        format.minimum_subnormal_exponent
    };
    let right_shift = unit_exponent
        .checked_sub(i64::from(encoded.exponent))
        .ok_or(MathError::ScaleExponentOverflow)?;
    let mut significand = round_shift(&encoded.limbs, bit_length, right_shift)?;
    if normal && significand == 1_u64 << format.precision {
        significand >>= 1;
        high_exponent += 1;
        if high_exponent > format.maximum_exponent {
            return Err(MathError::NonFinite);
        }
    }
    let magnitude_bits = if normal {
        let exponent_field = u64::try_from(high_exponent + format.exponent_bias)
            .map_err(|_| MathError::CounterOverflow)?;
        let implicit = 1_u64 << (format.precision - 1);
        (exponent_field << format.fraction_bits) | (significand - implicit)
    } else {
        significand
    };
    Ok(magnitude_bits | (u64::from(encoded.sign) << format.sign_shift))
}

fn round_shift(limbs: &[u32], bit_length: i64, right_shift: i64) -> Result<u64, MathError> {
    if right_shift <= 0 {
        let left_shift = u32::try_from(-right_shift).map_err(|_| MathError::CounterOverflow)?;
        let mut value = 0_u64;
        for bit in (0..bit_length).rev() {
            let next = u64::from(bit_at(limbs, bit)?);
            value = value
                .checked_mul(2)
                .and_then(|v| v.checked_add(next))
                .ok_or(MathError::CounterOverflow)?;
        }
        return value
            .checked_shl(left_shift)
            .ok_or(MathError::CounterOverflow);
    }
    let mut value = 0_u64;
    for bit in (right_shift..bit_length).rev() {
        let next = u64::from(bit_at(limbs, bit)?);
        value = value
            .checked_mul(2)
            .and_then(|v| v.checked_add(next))
            .ok_or(MathError::CounterOverflow)?;
    }
    let round_bit = bit_at(limbs, right_shift - 1)?;
    let sticky = any_bits_below(limbs, right_shift - 1)?;
    if round_bit != 0 && (sticky || value & 1 != 0) {
        value = value.checked_add(1).ok_or(MathError::CounterOverflow)?;
    }
    Ok(value)
}

fn any_bits_below(limbs: &[u32], exclusive_bit: i64) -> Result<bool, MathError> {
    if exclusive_bit <= 0 {
        return Ok(false);
    }
    let exclusive_bit = usize::try_from(exclusive_bit).map_err(|_| MathError::CounterOverflow)?;
    let full_limbs = (exclusive_bit / 32).min(limbs.len());
    if limbs[..full_limbs].iter().any(|limb| *limb != 0) {
        return Ok(true);
    }
    let partial_bits = exclusive_bit % 32;
    Ok(partial_bits != 0
        && limbs
            .get(full_limbs)
            .is_some_and(|limb| limb & ((1_u32 << partial_bits) - 1) != 0))
}

fn bit_at(limbs: &[u32], bit: i64) -> Result<u32, MathError> {
    if bit < 0 {
        return Ok(0);
    }
    let bit = usize::try_from(bit).map_err(|_| MathError::CounterOverflow)?;
    let Some(limb) = limbs.get(bit / 32) else {
        return Ok(0);
    };
    Ok((limb >> (bit % 32)) & 1)
}

pub fn encode_big_scalar(value: &BigScalar) -> Result<EncodedBigScalar, MathError> {
    if value.is_zero() {
        return Ok(EncodedBigScalar {
            sign: 0,
            exponent: 0,
            limbs: Vec::new(),
        });
    }
    let mut constants = Consts::new().map_err(|_| MathError::BigFloat)?;
    let (sign, digits, mut radix_exponent) = value
        .value
        .convert_to_radix(Radix::Bin, RoundingMode::None, &mut constants)
        .map_err(|_| MathError::BigFloat)?;
    if digits.iter().any(|digit| *digit > 1) {
        return Err(MathError::InvalidCentreEncoding);
    }
    let leading_zeros = digits
        .iter()
        .position(|digit| *digit == 1)
        .ok_or(MathError::InvalidCentreEncoding)?;
    radix_exponent = radix_exponent
        .checked_sub(i32::try_from(leading_zeros).map_err(|_| MathError::CounterOverflow)?)
        .ok_or(MathError::InvalidCentreEncoding)?;
    let digits = &digits[leading_zeros..];
    let digit_count = i32::try_from(digits.len()).map_err(|_| MathError::CounterOverflow)?;
    let exponent = radix_exponent
        .checked_sub(digit_count)
        .ok_or(MathError::InvalidCentreEncoding)?;
    let mut limbs = vec![0_u32; digits.len().div_ceil(32)];
    for (bit_from_low, digit) in digits.iter().rev().enumerate() {
        if *digit == 1 {
            limbs[bit_from_low / 32] |= 1_u32 << (bit_from_low % 32);
        }
    }
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
    Ok(EncodedBigScalar {
        sign: u32::from(sign == Sign::Neg),
        exponent,
        limbs,
    })
}

pub fn decode_big_scalar(
    sign: u32,
    exponent: i32,
    limbs: &[u32],
    precision_bits: u32,
) -> Result<BigScalar, MathError> {
    if precision_bits == 0 || sign > 1 {
        return Err(MathError::InvalidCentreEncoding);
    }
    if limbs.is_empty() {
        return (sign == 0 && exponent == 0)
            .then(|| BigScalar::zero(precision_bits))
            .ok_or(MathError::InvalidCentreEncoding)?;
    }
    let high = *limbs.last().ok_or(MathError::InvalidCentreEncoding)?;
    if high == 0 {
        return Err(MathError::InvalidCentreEncoding);
    }
    let high_bits = 32_u32 - high.leading_zeros();
    let lower_bits = u32::try_from(limbs.len() - 1)
        .map_err(|_| MathError::CounterOverflow)?
        .checked_mul(32)
        .ok_or(MathError::CounterOverflow)?;
    let bit_count = lower_bits
        .checked_add(high_bits)
        .ok_or(MathError::CounterOverflow)?;
    let mut digits = Vec::with_capacity(bit_count as usize);
    for bit_from_high in (0..bit_count).rev() {
        digits.push(((limbs[(bit_from_high / 32) as usize] >> (bit_from_high % 32)) & 1) as u8);
    }
    let radix_exponent = exponent
        .checked_add(i32::try_from(bit_count).map_err(|_| MathError::CounterOverflow)?)
        .ok_or(MathError::InvalidCentreEncoding)?;
    let requested_precision = precision_bits.max(bit_count) as usize;
    let mut constants = Consts::new().map_err(|_| MathError::BigFloat)?;
    let value = BigFloat::convert_from_radix(
        if sign == 0 { Sign::Pos } else { Sign::Neg },
        &digits,
        radix_exponent,
        Radix::Bin,
        requested_precision,
        RoundingMode::None,
        &mut constants,
    );
    BigScalar::checked(value)
}

#[cfg(test)]
mod tests {
    use super::{BigScalar, decode_big_scalar, encode_big_scalar};
    use crate::MathError;

    #[test]
    fn dyadic_codec_round_trips_exact_f64_values() -> Result<(), MathError> {
        for value in [0.0, -0.0, 1.0, -2.5, f64::from_bits(1), 1.0 / 3.0] {
            let value = BigScalar::from_f64(value, 256)?;
            let encoded = encode_big_scalar(&value)?;
            let decoded = decode_big_scalar(
                encoded.sign,
                encoded.exponent,
                &encoded.limbs,
                value.precision_bits()?,
            )?;
            assert_eq!(decoded, value);
        }
        Ok(())
    }

    #[test]
    fn dyadic_codec_rejects_noncanonical_inputs() {
        assert_eq!(
            decode_big_scalar(1, 0, &[], 64),
            Err(MathError::InvalidCentreEncoding)
        );
        assert_eq!(
            decode_big_scalar(0, 0, &[0], 64),
            Err(MathError::InvalidCentreEncoding)
        );
        assert_eq!(
            decode_big_scalar(2, 0, &[1], 64),
            Err(MathError::InvalidCentreEncoding)
        );
    }
}
