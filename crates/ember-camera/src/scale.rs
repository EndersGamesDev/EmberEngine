use crate::{CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, Screen};

/// Binary64 encodings of `2^(-1/2)`, `2^(-1/4)`, through `2^(-1/1024)`.
///
/// The dyadic table is copied bit-for-bit into fixed precision. Its 53-bit relative precision is
/// already the limiting precision of the rebuilt basis, while retaining it as integer bits avoids
/// platform `exp2` implementations and makes scale reconstruction reproducible on wasm32.
const FRACTIONAL_OCTAVE_FACTOR_BITS: [u64; 10] = [
    0x3fe6_a09e_667f_3bcd,
    0x3fea_e89f_995a_d3ad,
    0x3fed_5818_dcfb_a487,
    0x3fee_a4af_a2a4_90da,
    0x3fef_5076_5b6e_4540,
    0x3fef_a7c1_819e_90d8,
    0x3fef_d3c2_2b8f_71f1,
    0x3fef_e9d9_6b2a_23d9,
    0x3fef_f4ea_ca43_91b6,
    0x3fef_fa74_ea38_1efc,
];

/// Exact fixed-point pixel scale for a view exponent and render-grid width.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Scale<const LIMBS: usize> {
    units_per_pixel: Fixed<LIMBS>,
}

impl<const LIMBS: usize> Scale<LIMBS> {
    /// Returns the fixed plane units represented by one render-grid pixel.
    #[must_use]
    pub const fn units_per_pixel(self) -> Fixed<LIMBS> {
        self.units_per_pixel
    }
}

/// Derives `4 · 2^(−exponent) / width` without a floating-point operation.
///
/// Whole octaves are shifts. The ten fractional exponent bits select bit-pinned dyadic factors;
/// division by the `u32` grid width is rounded once to nearest even and is the scale's documented
/// grid quantisation. The same exponent and width always produce the same fixed bits.
///
/// # Errors
///
/// Returns a typed error for an invalid fixed width or an unrepresentable shallow scale.
pub fn scale_for<const LIMBS: usize>(
    exponent: Exponent,
    screen: Screen,
) -> Result<Scale<LIMBS>, CameraError> {
    let quanta = exponent.quanta();
    let octave = quanta.div_euclid(EXPONENT_QUANTA_PER_OCTAVE);
    let remainder = u32::try_from(quanta.rem_euclid(EXPONENT_QUANTA_PER_OCTAVE))
        .map_err(|_| CameraError::ExponentOutOfRange)?;
    let mut value = Fixed::from_i64(4)?;
    for (index, bits) in FRACTIONAL_OCTAVE_FACTOR_BITS.iter().copied().enumerate() {
        let bit = u32::try_from(FRACTIONAL_OCTAVE_FACTOR_BITS.len() - 1 - index)
            .map_err(|_| CameraError::InvalidWidth)?;
        if remainder & (1_u32 << bit) != 0 {
            value = value.mul(&Fixed::from_binary64_bits(bits)?)?;
        }
    }
    value = if octave >= 0 {
        value.shift_right(usize::try_from(octave).map_err(|_| CameraError::ExponentOutOfRange)?)?
    } else {
        value.shift_left(
            usize::try_from(octave.unsigned_abs()).map_err(|_| CameraError::ExponentOutOfRange)?,
        )?
    };
    Ok(Scale {
        units_per_pixel: value.div_u32_round_even(screen.width())?,
    })
}

#[cfg(test)]
mod tests {
    use super::scale_for;
    use crate::{CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, Screen};

    type TestFixed = Fixed<8>;

    #[test]
    fn integer_octaves_are_exact_shifts() -> Result<(), CameraError> {
        let screen = Screen::new(4, 4)?;
        let zero = scale_for::<8>(Exponent::ZERO, screen)?.units_per_pixel();
        let deep =
            scale_for::<8>(Exponent::new(EXPONENT_QUANTA_PER_OCTAVE)?, screen)?.units_per_pixel();
        let shallow =
            scale_for::<8>(Exponent::new(-EXPONENT_QUANTA_PER_OCTAVE)?, screen)?.units_per_pixel();
        assert_eq!(zero, TestFixed::from_i64(1)?);
        assert_eq!(
            scale_for::<2>(Exponent::ZERO, screen)?
                .units_per_pixel()
                .to_le_bytes(),
            [[0; 8], [1, 0, 0, 0, 0, 0, 0, 0]]
        );
        assert_eq!(deep, TestFixed::from_f64(0.5)?);
        assert_eq!(shallow, TestFixed::from_i64(2)?);
        Ok(())
    }

    #[test]
    fn fractional_scale_rebuilds_to_the_same_bits() -> Result<(), CameraError> {
        let screen = Screen::new(1_920, 1_080)?;
        let exponent = Exponent::new(37 * EXPONENT_QUANTA_PER_OCTAVE + 511)?;
        let first = scale_for::<8>(exponent, screen)?;
        let second = scale_for::<8>(exponent, screen)?;
        assert_eq!(first, second);
        let next = scale_for::<8>(exponent.checked_add(1)?, screen)?;
        assert!(next.units_per_pixel() < first.units_per_pixel());
        Ok(())
    }

    #[test]
    fn width_division_is_deterministic() -> Result<(), CameraError> {
        let exponent = Exponent::ZERO;
        assert_eq!(
            scale_for::<8>(exponent, Screen::new(4, 3)?)?.units_per_pixel(),
            TestFixed::from_i64(1)?
        );
        assert_eq!(
            scale_for::<8>(exponent, Screen::new(8, 3)?)?.units_per_pixel(),
            TestFixed::from_f64(0.5)?
        );
        Ok(())
    }
}
