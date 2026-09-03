use crate::orbit::smooth_iteration_f64;
use crate::{EscapeParams, MathError, PerturbSample, PerturbationEnvelope, ReferenceOrbitRecord};

const RESCALE_HIGH: f64 = 18_446_744_073_709_551_616.0;
const RESCALE_LOW: f64 = 1.0 / RESCALE_HIGH;
const MAX_EXACT_F32_INTEGER: u32 = 1 << 24;
const F32_UNIT_ROUNDOFF: f64 = 1.0 / 16_777_216.0;
const F32_MIN_SUBNORMAL: f64 = 1.401_298_464_324_817e-45;

#[derive(Clone, Copy)]
struct Complex64 {
    re: f64,
    im: f64,
}

impl Complex64 {
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    fn sub(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
        }
    }

    fn scale(self, factor: f64) -> Self {
        Self {
            re: self.re * factor,
            im: self.im * factor,
        }
    }

    fn square(self) -> Self {
        Self {
            re: self.re.mul_add(self.re, -(self.im * self.im)),
            im: (2.0 * self.re) * self.im,
        }
    }

    fn mul(self, other: Self) -> Self {
        Self {
            re: self.re.mul_add(other.re, -(self.im * other.im)),
            im: self.re.mul_add(other.im, self.im * other.re),
        }
    }

    fn hypot(self) -> f64 {
        self.re.hypot(self.im)
    }

    const fn finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

/// Runs the scaled binary64 perturbation mirror used by kernel conformance tests.
///
/// # Errors
///
/// Returns an error for an empty orbit, non-finite data, or invalid parameters.
pub fn perturb_scaled_f64(
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f64; 4],
    scale_exponent: i32,
    params: EscapeParams,
) -> Result<PerturbSample, MathError> {
    Ok(perturb_scaled_f64_with_envelope(orbit, offset_prime, scale_exponent, params)?.0)
}

/// Runs scaled perturbation and returns its propagated arithmetic envelope.
///
/// # Errors
///
/// Returns an error for an empty orbit, non-finite data, or invalid parameters.
pub fn perturb_scaled_f64_with_envelope(
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f64; 4],
    scale_exponent: i32,
    params: EscapeParams,
) -> Result<(PerturbSample, PerturbationEnvelope), MathError> {
    validate_inputs(orbit, offset_prime, params)?;
    let z_zero = reconstruct(orbit[0]);
    let mut delta_prime = Complex64 {
        re: offset_prime[0],
        im: offset_prime[1],
    };
    let mut delta_c_prime = Complex64 {
        re: offset_prime[2],
        im: offset_prime[3],
    };
    let mut exponent = scale_exponent;
    let mut reference_index = 0_usize;
    let mut rebase_count = 0_u32;
    let mut absolute_error = centre_offset_error(delta_prime, exponent);
    let mut centre_error = centre_offset_error(delta_c_prime, exponent);
    let mut minimum_escape_margin = f64::INFINITY;
    let mut maximum_norm_error = 0.0_f64;
    let mut last_value = z_zero;
    if !renormalize(
        &mut delta_prime,
        &mut delta_c_prime,
        &mut exponent,
        &mut absolute_error,
        &mut centre_error,
    ) {
        return Ok(glitch_result(
            rebase_count,
            absolute_error,
            maximum_norm_error,
            minimum_escape_margin,
        ));
    }
    for iteration in 0..params.max_iter {
        let Some(record) = orbit.get(reference_index) else {
            return Ok(glitch_result(
                rebase_count,
                absolute_error,
                maximum_norm_error,
                minimum_escape_margin,
            ));
        };
        let reference = reconstruct(*record);
        let reference_error = reference_reconstruction_error(*record);
        let actual_delta = ldexp_complex(delta_prime, exponent);
        let z = reference.add(actual_delta);
        last_value = z;
        let magnitude_squared = norm_squared(z);
        let escape_margin = (magnitude_squared - f64::from(params.bailout)).abs();
        minimum_escape_margin = minimum_escape_margin.min(escape_margin);
        let display_error = displayed_error(
            reference.hypot(),
            reference_error,
            actual_delta.hypot(),
            absolute_error,
            exponent,
        );
        let norm_error = norm_squared_error(z, display_error);
        maximum_norm_error = maximum_norm_error.max(norm_error);
        if magnitude_squared > f64::from(params.bailout) {
            return Ok(escaped_result(
                iteration,
                z,
                rebase_count,
                absolute_error,
                display_error,
                maximum_norm_error,
                minimum_escape_margin,
            ));
        }
        if iteration + 1 == params.max_iter {
            break;
        }
        let should_rebase = actual_delta.hypot() != 0.0 && z.hypot() < actual_delta.hypot();
        if should_rebase {
            if rebase_count == MAX_EXACT_F32_INTEGER {
                return Ok(glitch_result(
                    rebase_count,
                    absolute_error,
                    maximum_norm_error,
                    minimum_escape_margin,
                ));
            }
            let Some(inverse_exponent) = exponent.checked_neg() else {
                return Ok(glitch_result(
                    rebase_count,
                    absolute_error,
                    maximum_norm_error,
                    minimum_escape_margin,
                ));
            };
            delta_prime = ldexp_complex(z.sub(z_zero), inverse_exponent);
            reference_index = 0;
            rebase_count += 1;
            absolute_error = display_error
                + reference_reconstruction_error(orbit[0])
                + rebase_rounding_error(z, z_zero, exponent);
            if !renormalize(
                &mut delta_prime,
                &mut delta_c_prime,
                &mut exponent,
                &mut absolute_error,
                &mut centre_error,
            ) {
                return Ok(glitch_result(
                    rebase_count,
                    absolute_error,
                    maximum_norm_error,
                    minimum_escape_margin,
                ));
            }
        }
        let (reference, reference_error) = if should_rebase {
            (z_zero, reference_reconstruction_error(orbit[0]))
        } else {
            (reference, reference_error)
        };
        let reference_norm = reference.hypot();
        let delta_norm = ldexp_complex(delta_prime, exponent).hypot();
        let delta_c_norm = ldexp_complex(delta_c_prime, exponent).hypot();
        absolute_error = propagated_error(
            reference_norm,
            reference_error,
            delta_norm,
            absolute_error,
            delta_c_norm,
            centre_error,
            exponent,
        );
        delta_prime = ordinary_advance(delta_prime, delta_c_prime, exponent, reference);
        reference_index += 1;
        if !renormalize(
            &mut delta_prime,
            &mut delta_c_prime,
            &mut exponent,
            &mut absolute_error,
            &mut centre_error,
        ) {
            return Ok(glitch_result(
                rebase_count,
                absolute_error,
                maximum_norm_error,
                minimum_escape_margin,
            ));
        }
    }
    Ok(bounded_result(
        last_value,
        rebase_count,
        absolute_error,
        maximum_norm_error,
        minimum_escape_margin,
    ))
}

fn escaped_result(
    iteration: u32,
    z: Complex64,
    rebase_count: u32,
    absolute_error: f64,
    display_error: f64,
    maximum_norm_error: f64,
    minimum_escape_margin: f64,
) -> (PerturbSample, PerturbationEnvelope) {
    (
        PerturbSample {
            smooth_iter: smooth_iteration_f64(iteration, z.re, z.im),
            escaped: true,
            escape_index: Some(iteration),
            rebase_count,
            glitch: false,
        },
        PerturbationEnvelope {
            delta_abs_error: absolute_error,
            escape_norm2_error: maximum_norm_error,
            smooth_error: smooth_error_bound(z.hypot(), display_error),
            minimum_escape_margin,
        },
    )
}

fn bounded_result(
    last_value: Complex64,
    rebase_count: u32,
    absolute_error: f64,
    maximum_norm_error: f64,
    minimum_escape_margin: f64,
) -> (PerturbSample, PerturbationEnvelope) {
    (
        PerturbSample {
            smooth_iter: -1.0,
            escaped: false,
            escape_index: None,
            rebase_count,
            glitch: false,
        },
        PerturbationEnvelope {
            delta_abs_error: absolute_error,
            escape_norm2_error: maximum_norm_error
                .max(norm_squared_error(last_value, absolute_error)),
            smooth_error: 0.0,
            minimum_escape_margin,
        },
    )
}

fn ordinary_advance(
    delta_prime: Complex64,
    delta_c_prime: Complex64,
    exponent: i32,
    reference: Complex64,
) -> Complex64 {
    let linear = reference.mul(delta_prime).scale(2.0);
    let quadratic = ldexp_complex(delta_prime.square(), exponent);
    linear.add(quadratic).add(delta_c_prime)
}

fn renormalize(
    delta_prime: &mut Complex64,
    delta_c_prime: &mut Complex64,
    exponent: &mut i32,
    delta_error: &mut f64,
    delta_c_error: &mut f64,
) -> bool {
    loop {
        let norm = delta_prime.hypot();
        let (factor, exponent_delta) = if norm > RESCALE_HIGH {
            (RESCALE_LOW, 64)
        } else if norm != 0.0 && norm < RESCALE_LOW {
            (RESCALE_HIGH, -64)
        } else {
            return delta_prime.finite() && delta_c_prime.finite();
        };
        let Some(next_exponent) = exponent.checked_add(exponent_delta) else {
            return false;
        };
        *delta_prime = delta_prime.scale(factor);
        *delta_c_prime = delta_c_prime.scale(factor);
        *exponent = next_exponent;
        *delta_error += subnormal_renormalization_error(*delta_prime, next_exponent);
        *delta_c_error += subnormal_renormalization_error(*delta_c_prime, next_exponent);
    }
}

fn reconstruct(record: ReferenceOrbitRecord) -> Complex64 {
    Complex64 {
        re: f64::from(record.re_hi) + f64::from(record.re_lo),
        im: f64::from(record.im_hi) + f64::from(record.im_lo),
    }
}

fn ldexp_complex(value: Complex64, exponent: i32) -> Complex64 {
    if value.re == 0.0 && value.im == 0.0 {
        return value;
    }
    let factor = 2.0_f64.powi(exponent);
    Complex64 {
        re: if value.re == 0.0 {
            value.re
        } else {
            value.re * factor
        },
        im: if value.im == 0.0 {
            value.im
        } else {
            value.im * factor
        },
    }
}

fn norm_squared(value: Complex64) -> f64 {
    let largest = value.re.abs().max(value.im.abs());
    if largest > f64::from(EscapeParams::BAILOUT).sqrt() {
        f64::INFINITY
    } else {
        value.re.mul_add(value.re, value.im * value.im)
    }
}

fn reference_reconstruction_error(record: ReferenceOrbitRecord) -> f64 {
    half_ulp(record.re_lo).hypot(half_ulp(record.im_lo))
}

fn half_ulp(value: f32) -> f64 {
    let exponent = (value.to_bits() >> 23) & 0xff;
    if exponent == 0 {
        0.5 * F32_MIN_SUBNORMAL
    } else {
        let unbiased = i32::try_from(exponent).unwrap_or(255) - 127;
        0.5 * 2.0_f64.powi(unbiased - 23)
    }
}

fn centre_offset_error(value: Complex64, exponent: i32) -> f64 {
    let represented = ldexp_complex(value, exponent).hypot();
    F32_UNIT_ROUNDOFF * represented + scaled_subnormal_floor(exponent)
}

fn displayed_error(
    reference_norm: f64,
    reference_error: f64,
    delta_norm: f64,
    delta_error: f64,
    exponent: i32,
) -> f64 {
    reference_error
        + delta_error
        + gamma(2) * (reference_norm + delta_norm)
        + 2.0 * scaled_subnormal_floor(exponent)
}

fn rebase_rounding_error(value: Complex64, z_zero: Complex64, exponent: i32) -> f64 {
    gamma(2) * (value.hypot() + z_zero.hypot()) + scaled_subnormal_floor(exponent)
}

fn operation_rounding_error(
    reference_norm: f64,
    delta_norm: f64,
    delta_c_norm: f64,
    exponent: i32,
) -> f64 {
    let local_magnitude = 2.0 * reference_norm * delta_norm
        + delta_norm * delta_norm
        + delta_c_norm;
    gamma(20) * local_magnitude + 20.0 * scaled_subnormal_floor(exponent)
}

fn subnormal_renormalization_error(_value: Complex64, exponent: i32) -> f64 {
    scaled_subnormal_floor(exponent)
}

fn scaled_subnormal_floor(exponent: i32) -> f64 {
    core::f64::consts::SQRT_2 * F32_MIN_SUBNORMAL * 2.0_f64.powi(exponent)
}

fn gamma(operation_count: u32) -> f64 {
    let total = f64::from(operation_count) * F32_UNIT_ROUNDOFF;
    total / (1.0 - total)
}

fn propagated_error(
    reference_norm: f64,
    reference_error: f64,
    delta_norm: f64,
    previous: f64,
    delta_c_norm: f64,
    centre_error: f64,
    exponent: i32,
) -> f64 {
    let amplification = 2.0 * (reference_norm + delta_norm);
    let propagated = amplification.mul_add(previous, previous * previous);
    let reference = 2.0 * reference_error * (delta_norm + previous);
    let arithmetic = operation_rounding_error(
        reference_norm,
        delta_norm,
        delta_c_norm,
        exponent,
    );
    propagated + reference + centre_error + arithmetic
}

fn norm_squared_error(value: Complex64, absolute_error: f64) -> f64 {
    let magnitude = value.hypot();
    (2.0 * magnitude).mul_add(absolute_error, absolute_error * absolute_error)
        + radius_rounding_error(value)
        + 3.0 * F32_MIN_SUBNORMAL
}

#[allow(clippy::cast_possible_truncation)]
fn radius_rounding_error(value: Complex64) -> f64 {
    let re = value.re as f32;
    let im = value.im as f32;
    let computed = re * re + im * im;
    let exact = f64::from(re).mul_add(f64::from(re), f64::from(im) * f64::from(im));
    (f64::from(computed) - exact).abs()
}

fn smooth_error_bound(magnitude: f64, absolute_error: f64) -> f64 {
    if magnitude <= 1.0 || absolute_error == 0.0 {
        0.0
    } else {
        gamma(6)
            + absolute_error / (magnitude * magnitude.ln() * core::f64::consts::LN_2)
    }
}

const fn glitch_result(
    rebase_count: u32,
    absolute_error: f64,
    _maximum_norm_error: f64,
    minimum_escape_margin: f64,
) -> (PerturbSample, PerturbationEnvelope) {
    (
        PerturbSample {
            smooth_iter: -1.0,
            escaped: false,
            escape_index: None,
            rebase_count,
            glitch: true,
        },
        PerturbationEnvelope {
            delta_abs_error: absolute_error,
            escape_norm2_error: f64::INFINITY,
            smooth_error: f64::INFINITY,
            minimum_escape_margin,
        },
    )
}

fn validate_inputs(
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f64; 4],
    params: EscapeParams,
) -> Result<(), MathError> {
    if orbit.is_empty() {
        return Err(MathError::EmptyReferenceOrbit);
    }
    if params.max_iter == 0 {
        return Err(MathError::InvalidMaxIter);
    }
    if params.bailout != EscapeParams::BAILOUT {
        return Err(MathError::InvalidBailout);
    }
    if !offset_prime.iter().all(|component| component.is_finite())
        || !orbit.iter().all(|record| {
            [record.re_hi, record.im_hi, record.re_lo, record.im_lo]
                .into_iter()
                .all(f32::is_finite)
        })
    {
        return Err(MathError::NonFinite);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Complex64, ldexp_complex, ordinary_advance, perturb_scaled_f64,
        perturb_scaled_f64_with_envelope,
    };
    use crate::{EscapeParams, MathError, ReferenceOrbitRecord};

    fn record(re: f32, im: f32) -> ReferenceOrbitRecord {
        ReferenceOrbitRecord {
            re_hi: re,
            im_hi: im,
            re_lo: 0.0,
            im_lo: 0.0,
        }
    }

    #[test]
    fn nonzero_reference_rebase_preserves_the_full_value() -> Result<(), MathError> {
        let sample = perturb_scaled_f64(
            &[record(1.0, 0.0), record(1.0, 0.0)],
            [-0.75, 0.0, 0.0, 0.0],
            0,
            EscapeParams::new(2),
        )?;
        assert!(!sample.glitch);
        assert!(!sample.escaped);
        assert_eq!(sample.rebase_count, 1);
        let z_zero = Complex64 { re: 1.0, im: 0.0 };
        let z = Complex64 { re: 0.25, im: 0.0 };
        let rebased = ldexp_complex(z.sub(z_zero), 0);
        let advanced = ordinary_advance(rebased, Complex64 { re: 0.0, im: 0.0 }, 0, z_zero);
        assert_eq!(z_zero.add(advanced).re, 0.0625);
        Ok(())
    }

    #[test]
    fn orbit_exhaustion_is_an_honest_glitch() -> Result<(), MathError> {
        let sample = perturb_scaled_f64(&[record(0.0, 0.0)], [0.0; 4], -500, EscapeParams::new(2))?;
        assert!(sample.glitch);
        assert!(!sample.escaped);
        assert_eq!(sample.smooth_iter, -1.0);
        Ok(())
    }

    #[test]
    fn deep_scaled_zero_offset_matches_the_reference_classification() -> Result<(), MathError> {
        let orbit = [
            record(0.0, 0.0),
            record(2.0, 0.0),
            record(6.0, 0.0),
            record(38.0, 0.0),
        ];
        let (sample, envelope) =
            perturb_scaled_f64_with_envelope(&orbit, [0.0; 4], -900, EscapeParams::new(16))?;
        assert_eq!(sample.escape_index, Some(3));
        assert!(sample.escaped);
        assert!(envelope.smooth_error <= 2.0e-3);
        assert!(envelope.minimum_escape_margin > envelope.escape_norm2_error);
        Ok(())
    }

    #[test]
    fn renormalization_preserves_classification() -> Result<(), MathError> {
        let orbit = [record(0.0, 0.0), record(0.0, 0.0)];
        let sample = perturb_scaled_f64(
            &orbit,
            [2.0_f64.powi(80), 0.0, 0.0, 0.0],
            -80,
            EscapeParams::new(2),
        )?;
        assert!(!sample.escaped);
        assert!(!sample.glitch);
        let downward = perturb_scaled_f64(
            &orbit,
            [2.0_f64.powi(-80), 0.0, 0.0, 0.0],
            80,
            EscapeParams::new(2),
        )?;
        assert_eq!(downward.escaped, sample.escaped);
        assert_eq!(downward.glitch, sample.glitch);
        Ok(())
    }

    #[test]
    fn mixed_offsets_match_direct_f64_classification() -> Result<(), MathError> {
        let orbit = [
            record(0.0, 0.0),
            record(2.0, 0.0),
            record(6.0, 0.0),
            record(38.0, 0.0),
        ];
        for offset_prime in [
            [0.25, -0.125, 0.5, 0.0],
            [-0.5, 0.25, -0.5, 0.125],
            [0.0; 4],
        ] {
            let sample = perturb_scaled_f64(&orbit, offset_prime, -8, EscapeParams::new(16))?;
            let direct = direct_escape(offset_prime, -8, 16);
            assert_eq!(sample.escaped, direct.0);
            assert_eq!(sample.escape_index, direct.1);
            assert!(!sample.glitch);
        }
        Ok(())
    }

    fn direct_escape(offset_prime: [f64; 4], exponent: i32, max_iter: u32) -> (bool, Option<u32>) {
        let factor = 2.0_f64.powi(exponent);
        let mut z = Complex64 {
            re: offset_prime[0] * factor,
            im: offset_prime[1] * factor,
        };
        let c = Complex64 {
            re: offset_prime[2].mul_add(factor, 2.0),
            im: offset_prime[3] * factor,
        };
        for iteration in 0..max_iter {
            if z.re.mul_add(z.re, z.im * z.im) > f64::from(EscapeParams::BAILOUT) {
                return (true, Some(iteration));
            }
            z = z.square().add(c);
        }
        (false, None)
    }
}
