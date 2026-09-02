use core::num::NonZeroU32;

use crate::{
    BigCentre, BigScalar, ComputedOrbit, EscapeParams, EscapeSample, MathError, OrbitStep,
    PrecisionPlan, ReferenceOrbitRecord, split_scalar,
};

const LOG2_10: f64 = core::f64::consts::LOG2_10;

pub fn escape_f32(point: [f32; 4], params: EscapeParams) -> Result<EscapeSample, MathError> {
    validate_escape_inputs(point, params)?;
    let [mut z_re, mut z_im, c_re, c_im] = point;
    for iteration in 0..params.max_iter {
        let magnitude_squared = z_re.mul_add(z_re, z_im * z_im);
        if magnitude_squared > params.bailout {
            return Ok(EscapeSample {
                smooth_iter: smooth_iteration_f32(iteration, z_re, z_im),
                escaped: true,
                escape_index: Some(iteration),
            });
        }
        if iteration + 1 == params.max_iter {
            break;
        }
        let next_re = z_re.mul_add(z_re, -(z_im * z_im)) + c_re;
        let next_im = (2.0 * z_re).mul_add(z_im, c_im);
        z_re = next_re;
        z_im = next_im;
    }
    Ok(EscapeSample {
        smooth_iter: -1.0,
        escaped: false,
        escape_index: None,
    })
}

#[derive(Debug)]
pub struct ReferenceOrbitBuilder {
    centre: BigCentre,
    plan: PrecisionPlan,
    params: EscapeParams,
    attempt_digits: u32,
    primary: OrbitState,
    verification: OrbitState,
    mismatch: bool,
}

impl ReferenceOrbitBuilder {
    pub fn new(
        centre: &BigCentre,
        plan: PrecisionPlan,
        params: EscapeParams,
    ) -> Result<Self, MathError> {
        validate_params(params)?;
        if plan.working_digits < plan.floor_digits || plan.policy_digits == 0 {
            return Err(MathError::InvalidPrecisionPlan);
        }
        let (primary, verification) = make_attempt(centre, plan.working_digits, plan.policy_digits)?;
        Ok(Self {
            centre: centre.clone(),
            plan,
            params,
            attempt_digits: plan.working_digits,
            primary,
            verification,
            mismatch: false,
        })
    }

    pub fn step(&mut self, max_entries: NonZeroU32) -> Result<OrbitStep, MathError> {
        let mut work = 0_u32;
        while work < max_entries.get() {
            let primary = self.primary.advance(self.params)?;
            let verification = self.verification.advance(self.params)?;
            work += 1;
            match (primary, verification) {
                (Some(left), Some(right)) => {
                    self.mismatch |= !records_within_two_ulps(left.record, right.record);
                    self.mismatch |= left.escaped != right.escaped;
                    if left.done || right.done {
                        if left.done != right.done || self.mismatch {
                            self.restart_at_higher_precision()?;
                            continue;
                        }
                        return Ok(OrbitStep::Complete(self.primary.finish()?));
                    }
                }
                _ => return Err(MathError::InvalidOrbitState),
            }
        }
        Ok(OrbitStep::Pending {
            stored: u32::try_from(self.primary.records.len())
                .map_err(|_| MathError::OrbitTooLong)?,
        })
    }

    fn restart_at_higher_precision(&mut self) -> Result<(), MathError> {
        self.attempt_digits = self
            .attempt_digits
            .checked_add(16)
            .ok_or(MathError::CounterOverflow)?;
        let (primary, verification) = make_attempt(
            &self.centre,
            self.attempt_digits,
            self.plan.policy_digits,
        )?;
        self.primary = primary;
        self.verification = verification;
        self.mismatch = false;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ComplexBig {
    re: BigScalar,
    im: BigScalar,
}

#[derive(Debug)]
struct OrbitState {
    z: ComplexBig,
    c: ComplexBig,
    bailout: BigScalar,
    precision_bits: u32,
    records: Vec<ReferenceOrbitRecord>,
    escape_index: Option<u32>,
    done: bool,
}

#[derive(Clone, Copy)]
struct AdvanceResult {
    record: ReferenceOrbitRecord,
    escaped: bool,
    done: bool,
}

impl OrbitState {
    fn new(centre: &BigCentre, precision_bits: u32) -> Result<Self, MathError> {
        let [z_re, z_im, c_re, c_im] = &centre.coords;
        Ok(Self {
            z: ComplexBig {
                re: z_re.with_precision(precision_bits)?,
                im: z_im.with_precision(precision_bits)?,
            },
            c: ComplexBig {
                re: c_re.with_precision(precision_bits)?,
                im: c_im.with_precision(precision_bits)?,
            },
            bailout: BigScalar::from_f32(EscapeParams::BAILOUT, precision_bits)?,
            precision_bits,
            records: Vec::new(),
            escape_index: None,
            done: false,
        })
    }

    fn advance(&mut self, params: EscapeParams) -> Result<Option<AdvanceResult>, MathError> {
        if self.done {
            return Ok(None);
        }
        let iteration = u32::try_from(self.records.len()).map_err(|_| MathError::OrbitTooLong)?;
        let record = split_complex(&self.z)?;
        self.records.push(record);
        let escaped = self.escaped()?;
        if escaped {
            self.escape_index = Some(iteration);
        }
        self.done = escaped || iteration + 1 == params.max_iter;
        if !self.done {
            self.advance_value()?;
        }
        Ok(Some(AdvanceResult {
            record,
            escaped,
            done: self.done,
        }))
    }

    fn escaped(&self) -> Result<bool, MathError> {
        let re_squared = self.z.re.mul(&self.z.re, self.precision_bits)?;
        let im_squared = self.z.im.mul(&self.z.im, self.precision_bits)?;
        let magnitude_squared = re_squared.add(&im_squared, self.precision_bits)?;
        Ok(magnitude_squared.compare(&self.bailout)? > 0)
    }

    fn advance_value(&mut self) -> Result<(), MathError> {
        let re_squared = self.z.re.mul(&self.z.re, self.precision_bits)?;
        let im_squared = self.z.im.mul(&self.z.im, self.precision_bits)?;
        let re_im = self.z.re.mul(&self.z.im, self.precision_bits)?;
        let two_re_im = re_im.scale_pow2(1)?;
        let next_re = re_squared
            .sub(&im_squared, self.precision_bits)?
            .add(&self.c.re, self.precision_bits)?;
        let next_im = two_re_im.add(&self.c.im, self.precision_bits)?;
        self.z = ComplexBig {
            re: next_re,
            im: next_im,
        };
        Ok(())
    }

    fn finish(&mut self) -> Result<ComputedOrbit, MathError> {
        if !self.done || self.records.is_empty() {
            return Err(MathError::InvalidOrbitState);
        }
        let records = core::mem::take(&mut self.records);
        let length = u32::try_from(records.len()).map_err(|_| MathError::OrbitTooLong)?;
        Ok(ComputedOrbit {
            records,
            length,
            precision_bits: self.precision_bits,
            escape_index: self.escape_index,
        })
    }
}

fn make_attempt(
    centre: &BigCentre,
    primary_digits: u32,
    policy_digits: u32,
) -> Result<(OrbitState, OrbitState), MathError> {
    let verification_digits = primary_digits
        .checked_add(16)
        .ok_or(MathError::CounterOverflow)?;
    if verification_digits > policy_digits {
        return Err(MathError::PrecisionExhausted {
            requested_digits: verification_digits,
            policy_digits,
        });
    }
    let primary_bits = bits_for_digits(primary_digits)?;
    let verification_bits = bits_for_digits(verification_digits)?;
    Ok((
        OrbitState::new(centre, primary_bits)?,
        OrbitState::new(centre, verification_bits)?,
    ))
}

fn bits_for_digits(digits: u32) -> Result<u32, MathError> {
    let bits = (f64::from(digits) * LOG2_10).ceil();
    if !(1.0..=f64::from(u32::MAX)).contains(&bits) {
        return Err(MathError::CounterOverflow);
    }
    Ok(bits as u32)
}

fn split_complex(value: &ComplexBig) -> Result<ReferenceOrbitRecord, MathError> {
    let re = split_scalar(&value.re)?;
    let im = split_scalar(&value.im)?;
    Ok(ReferenceOrbitRecord {
        re_hi: re[0],
        im_hi: im[0],
        re_lo: re[1],
        im_lo: im[1],
    })
}

fn records_within_two_ulps(left: ReferenceOrbitRecord, right: ReferenceOrbitRecord) -> bool {
    [
        (left.re_hi, right.re_hi),
        (left.im_hi, right.im_hi),
        (left.re_lo, right.re_lo),
        (left.im_lo, right.im_lo),
    ]
    .into_iter()
    .all(|(a, b)| ulp_distance(a, b).is_some_and(|distance| distance <= 2))
}

fn ulp_distance(left: f32, right: f32) -> Option<u32> {
    if !left.is_finite() || !right.is_finite() {
        return None;
    }
    let ordered = |value: f32| {
        let bits = value.to_bits();
        if bits & 0x8000_0000 == 0 {
            bits | 0x8000_0000
        } else {
            !bits
        }
    };
    Some(ordered(left).abs_diff(ordered(right)))
}

fn validate_escape_inputs(point: [f32; 4], params: EscapeParams) -> Result<(), MathError> {
    validate_params(params)?;
    if point.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err(MathError::NonFinite)
    }
}

fn validate_params(params: EscapeParams) -> Result<(), MathError> {
    if params.max_iter == 0 {
        return Err(MathError::InvalidMaxIter);
    }
    if params.bailout != EscapeParams::BAILOUT {
        return Err(MathError::InvalidBailout);
    }
    Ok(())
}

pub fn smooth_iteration_f64(iteration: u32, z_re: f64, z_im: f64) -> f32 {
    let magnitude = z_re.hypot(z_im);
    (f64::from(iteration) + 1.0 - magnitude.log2().log2()) as f32
}

fn smooth_iteration_f32(iteration: u32, z_re: f32, z_im: f32) -> f32 {
    let magnitude = z_re.hypot(z_im);
    iteration as f32 + 1.0 - magnitude.log2().log2()
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use super::{ReferenceOrbitBuilder, escape_f32};
    use crate::{BigCentre, EscapeParams, MathError, OrbitStep, precision_for};

    #[test]
    fn shallow_escape_uses_the_current_index() -> Result<(), MathError> {
        let sample = escape_f32([0.0, 0.0, 2.0, 0.0], EscapeParams::new(16))?;
        assert_eq!(sample.escape_index, Some(3));
        assert!(sample.escaped);
        assert!(sample.smooth_iter.is_finite());
        assert_eq!(
            escape_f32([0.0, 0.0, 0.0, 0.0], EscapeParams::new(16))?.smooth_iter,
            -1.0
        );
        Ok(())
    }

    #[test]
    fn orbit_is_cooperative_and_starts_at_z_zero() -> Result<(), MathError> {
        let centre = BigCentre::from_f64([0.0, 0.0, 2.0, 0.0], 256)?;
        let plan = precision_for(0.0, 1920, 16)?;
        let builder_result = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(16));
        assert!(builder_result.is_ok(), "builder construction failed: {builder_result:?}");
        let mut builder = builder_result?;
        assert_eq!(
            builder.step(NonZeroU32::new(2).ok_or(MathError::InvalidMaxIter)?)?,
            OrbitStep::Pending { stored: 2 }
        );
        let OrbitStep::Complete(orbit) =
            builder.step(NonZeroU32::new(2).ok_or(MathError::InvalidMaxIter)?)?
        else {
            return Err(MathError::InvalidOrbitState);
        };
        assert_eq!(orbit.length, 4);
        assert_eq!(orbit.escape_index, Some(3));
        assert_eq!(orbit.records[0].re_hi, 0.0);
        assert_eq!(orbit.records[1].re_hi, 2.0);
        Ok(())
    }
}
