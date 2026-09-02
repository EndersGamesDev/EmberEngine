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

pub fn encode_big_scalar(value: &BigScalar) -> Result<EncodedBigScalar, MathError> {
    if value.is_zero() {
        return Ok(EncodedBigScalar {
            sign: 0,
            exponent: 0,
            limbs: Vec::new(),
        });
    }
    let mut constants = Consts::new().map_err(|_| MathError::BigFloat)?;
    let (sign, digits, radix_exponent) = value
        .value
        .convert_to_radix(Radix::Bin, RoundingMode::None, &mut constants)
        .map_err(|_| MathError::BigFloat)?;
    if digits.first() != Some(&1) || digits.iter().any(|digit| *digit > 1) {
        return Err(MathError::InvalidCentreEncoding);
    }
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
