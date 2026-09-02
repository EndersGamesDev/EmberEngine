use crate::{
    BigCentre, BigScalar, CentreF64, CentreSplit, MathError, Plane, PrecisionPlan,
    ScaledPixelScale,
};

const LOG10_2: f64 = core::f64::consts::LOG10_2;
const LOG2_10: f64 = core::f64::consts::LOG2_10;
const PRECISION_POLICY_DIGITS: u32 = 300;

pub fn mirror_centre(centre: &BigCentre) -> Result<CentreF64, MathError> {
    let [a, b, c, d] = &centre.coords;
    let coords = [a.to_f64()?, b.to_f64()?, c.to_f64()?, d.to_f64()?];
    if coords.iter().all(|component| component.is_finite()) {
        Ok(CentreF64 { coords })
    } else {
        Err(MathError::NonFinite)
    }
}

pub fn split_scalar(value: &BigScalar) -> Result<[f32; 2], MathError> {
    let precision_bits = value.precision_bits()?;
    let high = value.to_f32()?;
    let exact_high = BigScalar::from_f32(high, precision_bits)?;
    let residual = value.sub(&exact_high, precision_bits)?;
    Ok([high, residual.to_f32()?])
}

pub fn split_centre(centre: &BigCentre) -> Result<CentreSplit, MathError> {
    let [a, b, c, d] = &centre.coords;
    let a = split_scalar(a)?;
    let b = split_scalar(b)?;
    let c = split_scalar(c)?;
    let d = split_scalar(d)?;
    Ok(CentreSplit {
        hi: [a[0], b[0], c[0], d[0]],
        lo: [a[1], b[1], c[1], d[1]],
    })
}

pub fn scaled_pixel_scale(
    zoom_log2: f64,
    grid_width: u32,
) -> Result<ScaledPixelScale, MathError> {
    if !zoom_log2.is_finite() {
        return Err(MathError::NonFinite);
    }
    if grid_width == 0 {
        return Err(MathError::InvalidExtent);
    }
    let logarithm = 2.0 - zoom_log2 - f64::from(grid_width).log2();
    let exponent_f64 = logarithm.floor() + 1.0;
    if exponent_f64 < f64::from(i32::MIN) || exponent_f64 > f64::from(i32::MAX) {
        return Err(MathError::ScaleExponentOverflow);
    }
    let mut exponent = exponent_f64 as i32;
    let mut mantissa = 2.0_f64.powf(logarithm - exponent_f64) as f32;
    if mantissa == 1.0 {
        exponent = exponent
            .checked_add(1)
            .ok_or(MathError::ScaleExponentOverflow)?;
        mantissa = 0.5;
    }
    if !(0.5..1.0).contains(&mantissa) {
        return Err(MathError::NonFinite);
    }
    Ok(ScaledPixelScale {
        mantissa,
        exponent,
    })
}

pub fn scale_split(zoom_log2: f64, grid_width: u32) -> Result<ScaledPixelScale, MathError> {
    scaled_pixel_scale(zoom_log2, grid_width)
}

pub fn shallow_pixel_scale(zoom_log2: f64, grid_width: u32) -> Result<f32, MathError> {
    let scale = scaled_pixel_scale(zoom_log2, grid_width)?;
    let value = f64::from(scale.mantissa) * 2.0_f64.powi(scale.exponent);
    let value = value as f32;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(MathError::ScaleExponentOverflow)
    }
}

pub fn precision_for(
    zoom_log2: f64,
    grid_width: u32,
    max_iter: u32,
) -> Result<PrecisionPlan, MathError> {
    if !zoom_log2.is_finite() {
        return Err(MathError::NonFinite);
    }
    if grid_width == 0 {
        return Err(MathError::InvalidExtent);
    }
    if max_iter == 0 {
        return Err(MathError::InvalidMaxIter);
    }
    let coordinate_digits = (zoom_log2.mul_add(LOG10_2, f64::from(grid_width).log10())).ceil();
    let floor_digits_f64 = (coordinate_digits + 8.0).max(1.0);
    let iteration_digits = f64::from(max_iter).log10().ceil();
    let working_digits_f64 = floor_digits_f64 + iteration_digits;
    if working_digits_f64 > f64::from(PRECISION_POLICY_DIGITS) {
        return Err(MathError::PrecisionExhausted {
            requested_digits: saturating_f64_to_u32(working_digits_f64),
            policy_digits: PRECISION_POLICY_DIGITS,
        });
    }
    let floor_digits = floor_digits_f64 as u32;
    let working_digits = working_digits_f64 as u32;
    let requested_bits = (working_digits_f64 * LOG2_10).ceil() as u32;
    Ok(PrecisionPlan {
        floor_digits,
        working_digits,
        requested_bits,
        policy_digits: PRECISION_POLICY_DIGITS,
    })
}

pub fn scaled_pixel_offset(
    plane: Plane,
    scale: ScaledPixelScale,
    extent: [u32; 2],
    pixel: [u32; 2],
) -> Result<[f32; 4], MathError> {
    let [width, height] = extent;
    let [column, row] = pixel;
    if width == 0 || height == 0 || column >= width || row >= height {
        return Err(MathError::InvalidExtent);
    }
    let x = (f64::from(column) + 0.5) - f64::from(width) * 0.5;
    let y = (f64::from(row) + 0.5) - f64::from(height) * 0.5;
    let mantissa = f64::from(scale.mantissa);
    Ok(core::array::from_fn(|axis| {
        (mantissa
            * x.mul_add(
                f64::from(plane.basis_u[axis]),
                y * f64::from(plane.basis_v[axis]),
            )) as f32
    }))
}

pub fn centre_displacement_px(
    centre: &BigCentre,
    reference: &BigCentre,
    plane: Plane,
    zoom_log2: f64,
    grid_width: u32,
) -> Result<[f64; 2], MathError> {
    if centre.precision_bits != reference.precision_bits {
        return Err(MathError::PrecisionMismatch);
    }
    let precision_bits = centre.precision_bits;
    let scale = scaled_pixel_scale(zoom_log2, grid_width)?;
    let scale_mantissa = BigScalar::from_f32(scale.mantissa, precision_bits)?;
    let inverse_exponent = scale
        .exponent
        .checked_neg()
        .ok_or(MathError::ScaleExponentOverflow)?;
    let mut delta = Vec::with_capacity(4);
    for (value, reference_value) in centre.coords.iter().zip(&reference.coords) {
        delta.push(value.sub(reference_value, precision_bits)?);
    }
    let project = |basis: [f32; 4]| -> Result<f64, MathError> {
        let mut sum = BigScalar::zero(precision_bits)?;
        for (component, weight) in delta.iter().zip(basis) {
            let weight = BigScalar::from_f32(weight, precision_bits)?;
            let term = component.mul(&weight, precision_bits)?;
            sum = sum.add(&term, precision_bits)?;
        }
        sum.div(&scale_mantissa, precision_bits)?
            .scale_pow2(inverse_exponent)?
            .to_f64()
    };
    let displacement = [project(plane.basis_u)?, project(plane.basis_v)?];
    if displacement.iter().all(|component| component.is_finite()) {
        Ok(displacement)
    } else {
        Err(MathError::NonFinite)
    }
}

fn saturating_f64_to_u32(value: f64) -> u32 {
    if value >= f64::from(u32::MAX) {
        u32::MAX
    } else if value <= 0.0 {
        0
    } else {
        value as u32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        centre_displacement_px, mirror_centre, precision_for, scaled_pixel_offset,
        scaled_pixel_scale, split_centre,
    };
    use crate::{BigCentre, MathError, Plane};

    #[test]
    fn scale_never_materializes_the_deep_power_of_two() -> Result<(), MathError> {
        let scale = scaled_pixel_scale(1000.25, 1920)?;
        assert!((0.5..1.0).contains(&scale.mantissa));
        assert_eq!(scale.exponent, -1009);
        Ok(())
    }

    #[test]
    fn split_is_direct_and_reconstructs_the_mirror() -> Result<(), MathError> {
        let centre = BigCentre::from_f64([1.0 / 3.0, -0.1, 1.25, -2.5], 256)?;
        let mirror = mirror_centre(&centre)?;
        let split = split_centre(&centre)?;
        for ((high, low), expected) in split.hi.into_iter().zip(split.lo).zip(mirror.coords) {
            let reconstructed = f64::from(high) + f64::from(low);
            assert!((reconstructed - expected).abs() <= f64::EPSILON);
        }
        Ok(())
    }

    #[test]
    fn precision_floor_accounts_for_iterations() -> Result<(), MathError> {
        let plan = precision_for(100.0, 1920, 4096)?;
        assert_eq!(plan.floor_digits, 42);
        assert_eq!(plan.working_digits, 46);
        assert_eq!(plan.policy_digits, 300);
        assert_eq!(plan.requested_bits, 153);
        Ok(())
    }

    #[test]
    fn row_zero_is_below_the_centre() -> Result<(), MathError> {
        let plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let offset = scaled_pixel_offset(
            plane,
            scaled_pixel_scale(0.0, 2)?,
            [2, 2],
            [0, 0],
        )?;
        assert_eq!(offset, [-0.25, -0.25, 0.0, 0.0]);
        Ok(())
    }

    #[test]
    fn displacement_divides_before_narrowing() -> Result<(), MathError> {
        let reference = BigCentre::from_f64([0.0; 4], 512)?;
        let centre = BigCentre::from_f64([2.0_f64.powi(-200), 0.0, 0.0, 0.0], 512)?;
        let plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let displacement = centre_displacement_px(&centre, &reference, plane, 190.0, 1024)?;
        assert_eq!(displacement, [0.25, 0.0]);
        Ok(())
    }
}
