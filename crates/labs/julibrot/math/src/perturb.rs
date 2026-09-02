use crate::{
    EscapeParams, MathError, PerturbSample, PerturbationEnvelope, ReferenceOrbitRecord,
    smooth_iteration_f64,
};

const RESCALE_HIGH: f64 = 18_446_744_073_709_551_616.0;
const RESCALE_LOW: f64 = 1.0 / RESCALE_HIGH;
const MAX_EXACT_F32_INTEGER: u32 = 1 << 24;

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

    fn finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

pub fn perturb_scaled_f64(
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f64; 4],
    scale_exponent: i32,
    params: EscapeParams,
) -> Result<PerturbSample, MathError> {
    Ok(perturb_scaled_f64_with_envelope(
        orbit,
        offset_prime,
        scale_exponent,
        params,
    )?
    .0)
}

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
    let mut absolute_error = 0.0_f64;
    let mut minimum_escape_margin = f64::INFINITY;
    let mut last_value = z_zero;
    for iteration in 0..params.max_iter {
        let Some(record) = orbit.get(reference_index) else {
            return Ok(glitch_result(rebase_count, absolute_error, minimum_escape_margin));
        };
        let reference = reconstruct(*record);
        let actual_delta = ldexp_complex(delta_prime, exponent);
        let z = reference.add(actual_delta);
        last_value = z;
        let magnitude_squared = norm_squared(z);
        let escape_margin = (magnitude_squared - f64::from(params.bailout)).abs();
        minimum_escape_margin = minimum_escape_margin.min(escape_margin);
        let norm_error = norm_squared_error(z, absolute_error);
        if magnitude_squared > f64::from(params.bailout) {
            return Ok((
                PerturbSample {
                    smooth_iter: smooth_iteration_f64(iteration, z.re, z.im),
                    escaped: true,
                    escape_index: Some(iteration),
                    rebase_count,
                    glitch: false,
                },
                PerturbationEnvelope {
                    delta_abs_error: absolute_error,
                    escape_norm2_error: norm_error,
                    smooth_error: smooth_error_bound(z.hypot(), absolute_error),
                    minimum_escape_margin,
                },
            ));
        }
        if iteration + 1 == params.max_iter {
            break;
        }
        let should_rebase = actual_delta.hypot() != 0.0 && z.hypot() < actual_delta.hypot();
        if should_rebase {
            if rebase_count == MAX_EXACT_F32_INTEGER {
                return Ok(glitch_result(rebase_count, absolute_error, minimum_escape_margin));
            }
            let Some(inverse_exponent) = exponent.checked_neg() else {
                return Ok(glitch_result(rebase_count, absolute_error, minimum_escape_margin));
            };
            delta_prime = ldexp_complex(z.sub(z_zero), inverse_exponent);
            reference_index = 0;
            rebase_count += 1;
        }
        let reference = if should_rebase { z_zero } else { reference };
        let reference_norm = reference.hypot();
        let delta_norm = ldexp_complex(delta_prime, exponent).hypot();
        absolute_error = propagated_error(reference_norm, delta_norm, absolute_error);
        delta_prime = ordinary_advance(delta_prime, delta_c_prime, exponent, reference);
        reference_index += 1;
        if !renormalize(&mut delta_prime, &mut delta_c_prime, &mut exponent) {
            return Ok(glitch_result(rebase_count, absolute_error, minimum_escape_margin));
        }
    }
    Ok((
        PerturbSample {
            smooth_iter: -1.0,
            escaped: false,
            escape_index: None,
            rebase_count,
            glitch: false,
        },
        PerturbationEnvelope {
            delta_abs_error: absolute_error,
            escape_norm2_error: norm_squared_error(last_value, absolute_error),
            smooth_error: 0.0,
            minimum_escape_margin,
        },
    ))
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

fn propagated_error(reference_norm: f64, delta_norm: f64, previous: f64) -> f64 {
    let amplification = 2.0 * (reference_norm + delta_norm);
    let arithmetic = 32.0
        * f64::EPSILON
        * (1.0 + reference_norm + delta_norm).powi(2);
    amplification.mul_add(previous, arithmetic)
}

fn norm_squared_error(value: Complex64, absolute_error: f64) -> f64 {
    (2.0 * value.hypot() + absolute_error) * absolute_error + 8.0 * f64::EPSILON
}

fn smooth_error_bound(magnitude: f64, absolute_error: f64) -> f64 {
    if magnitude <= 1.0 || absolute_error == 0.0 {
        0.0
    } else {
        absolute_error / (magnitude * magnitude.ln() * core::f64::consts::LN_2)
            + 16.0 * f64::EPSILON
    }
}

fn glitch_result(
    rebase_count: u32,
    absolute_error: f64,
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
    use super::{perturb_scaled_f64, perturb_scaled_f64_with_envelope};
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
        Ok(())
    }

    #[test]
    fn orbit_exhaustion_is_an_honest_glitch() -> Result<(), MathError> {
        let sample = perturb_scaled_f64(
            &[record(0.0, 0.0)],
            [0.0; 4],
            -500,
            EscapeParams::new(2),
        )?;
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
        let (sample, envelope) = perturb_scaled_f64_with_envelope(
            &orbit,
            [0.0; 4],
            -900,
            EscapeParams::new(16),
        )?;
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
        Ok(())
    }
}
