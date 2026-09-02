// CPU mirrors intentionally reproduce WGSL's fixed-width conversions and written operation order.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::imprecise_flops,
    clippy::suboptimal_flops
)]

use ember_julibrot_math::{
    EscapeGridRecord, EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit,
};

use crate::{
    GridExtent, KernelError, KernelSample, PerturbUniform, RefinementLevel,
    records::pixel_offset,
    shallow::{validate_extent, validate_params},
};

const REBASE_EXACT_LIMIT: u32 = 1 << 24;
const LDEXP_EXPONENT_LIMIT: i32 = 512;
const MAX_RESCALE_STEPS: u32 = u32::MAX / 64;

#[derive(Clone, Copy, Debug, PartialEq)]
struct ScaledState {
    delta: [f32; 2],
    delta_c: [f32; 2],
    exponent: i32,
    glitch: bool,
}

impl PerturbUniform {
    /// Packs a checked scaled-perturbation payload.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for an invalid extent, scale split, escape parameters, or orbit
    /// length outside `1..=max_iter`.
    pub fn pack(
        plane: Plane,
        scale: ScaleSplit,
        extent: GridExtent,
        params: EscapeParams,
        orbit_length: u32,
        level: RefinementLevel,
    ) -> Result<Self, KernelError> {
        validate_extent(extent)?;
        validate_params(params)?;
        if !scale.mantissa.is_finite() || !(0.5..1.0).contains(&scale.mantissa) {
            return Err(KernelError::InvalidEscapeParams);
        }
        if orbit_length == 0 || orbit_length > params.max_iter {
            return Err(KernelError::ReferenceLengthMismatch);
        }
        Ok(Self::from_parts(
            plane,
            scale,
            extent,
            params.max_iter,
            orbit_length,
            level,
        ))
    }
}

fn complex_mul(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    let real = left[0] * right[0] - left[1] * right[1];
    let imaginary = left[0] * right[1] + left[1] * right[0];
    [real, imaginary]
}

fn add(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    [left[0] + right[0], left[1] + right[1]]
}

fn subtract(left: [f32; 2], right: [f32; 2]) -> [f32; 2] {
    [left[0] - right[0], left[1] - right[1]]
}

fn multiply(value: [f32; 2], factor: f32) -> [f32; 2] {
    [value[0] * factor, value[1] * factor]
}

fn finite(value: [f32; 2]) -> bool {
    value.into_iter().all(f32::is_finite)
}

fn ldexp(value: f32, exponent: i32) -> f32 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    if exponent > LDEXP_EXPONENT_LIMIT {
        return f32::INFINITY.copysign(value);
    }
    if exponent < -LDEXP_EXPONENT_LIMIT {
        return 0.0_f32.copysign(value);
    }
    (f64::from(value) * 2.0_f64.powi(exponent)) as f32
}

fn scale(value: [f32; 2], exponent: i32) -> [f32; 2] {
    [ldexp(value[0], exponent), ldexp(value[1], exponent)]
}

fn robust_norm(value: [f32; 2]) -> f32 {
    let magnitude = value[0].abs().max(value[1].abs());
    if magnitude == 0.0 {
        return 0.0;
    }
    let normalized = [value[0] / magnitude, value[1] / magnitude];
    magnitude * (normalized[0] * normalized[0] + normalized[1] * normalized[1]).sqrt()
}

fn log2_norm(value: [f32; 2]) -> f32 {
    let magnitude = value[0].abs().max(value[1].abs());
    let normalized = [value[0] / magnitude, value[1] / magnitude];
    magnitude.log2() + 0.5 * (normalized[0] * normalized[0] + normalized[1] * normalized[1]).log2()
}

fn normalize_scaled(mut state: ScaledState) -> ScaledState {
    if !finite(state.delta) || !finite(state.delta_c) {
        state.glitch = true;
        return state;
    }
    let low = ldexp(1.0, -64);
    let high = ldexp(1.0, 64);
    let mut steps = 0_u32;
    loop {
        let magnitude = robust_norm(state.delta);
        if magnitude == 0.0 || (magnitude >= low && magnitude <= high) {
            return state;
        }
        let (factor, exponent) = if magnitude > high {
            let Some(exponent) = state.exponent.checked_add(64) else {
                state.glitch = true;
                return state;
            };
            (low, exponent)
        } else {
            let Some(exponent) = state.exponent.checked_sub(64) else {
                state.glitch = true;
                return state;
            };
            (high, exponent)
        };
        state.delta = multiply(state.delta, factor);
        state.delta_c = multiply(state.delta_c, factor);
        state.exponent = exponent;
        steps += 1;
        // Each successful step moves an i32 exponent 64 toward one bound.  At most
        // floor((i32::MAX - i32::MIN) / 64) steps fit; checked arithmetic refuses the next
        // step, so this defensive branch is unreachable.
        if steps > MAX_RESCALE_STEPS {
            state.glitch = true;
            return state;
        }
        if !finite(state.delta) || !finite(state.delta_c) {
            state.glitch = true;
            return state;
        }
    }
}

fn reconstruct(record: ReferenceOrbitRecord) -> [f32; 2] {
    [record.re_hi + record.re_lo, record.im_hi + record.im_lo]
}

fn radius_squared(value: [f32; 2]) -> f32 {
    value[0] * value[0] + value[1] * value[1]
}

fn smooth_iteration(iteration: u32, value: [f32; 2]) -> f32 {
    iteration as f32 + 1.0 - log2_norm(value).log2()
}

fn record(rebases: u32, glitch: bool) -> KernelSample {
    KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: rebases as f32,
            glitch: u8::from(glitch).into(),
        },
        escape_index: None,
    }
}

fn advance(reference: [f32; 2], delta: [f32; 2], delta_c: [f32; 2], exponent: i32) -> [f32; 2] {
    let linear = multiply(complex_mul(reference, delta), 2.0);
    let quadratic = scale(complex_mul(delta, delta), exponent);
    add(add(linear, quadratic), delta_c)
}

/// Mirrors the scaled perturbation WGSL from one already normalized pixel offset.
///
/// # Errors
///
/// Returns a typed refusal when uniform parameters or the reference slice violate their exact
/// lengths; arithmetic failures inside a pixel produce the honest glitch record.
pub fn perturb_scaled_offset(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f32; 4],
) -> Result<KernelSample, KernelError> {
    validate_params(EscapeParams {
        max_iter: uniforms.max_iter,
        bailout: uniforms.bailout,
    })?;
    let Ok(orbit_length) = usize::try_from(uniforms.orbit_length) else {
        return Err(KernelError::ReferenceLengthMismatch);
    };
    if uniforms.orbit_length == 0
        || uniforms.orbit_length > uniforms.max_iter
        || orbit.len() < orbit_length
    {
        return Err(KernelError::ReferenceLengthMismatch);
    }
    let z_zero = reconstruct(orbit[0]);
    let mut state = normalize_scaled(ScaledState {
        delta: [offset_prime[0], offset_prime[1]],
        delta_c: [offset_prime[2], offset_prime[3]],
        exponent: uniforms.scale_exponent,
        glitch: false,
    });
    if state.glitch {
        return Ok(record(0, true));
    }
    let mut reference_index = 0_u32;
    let mut rebases = 0_u32;
    for iteration in 0..uniforms.max_iter {
        if reference_index >= uniforms.orbit_length {
            return Ok(record(rebases, true));
        }
        let reference = reconstruct(orbit[reference_index as usize]);
        let represented_delta = scale(state.delta, state.exponent);
        let z = add(reference, represented_delta);
        if !finite(z) {
            return Ok(record(rebases, true));
        }
        if radius_squared(z) > uniforms.bailout {
            return Ok(KernelSample {
                record: EscapeGridRecord {
                    smooth_iter: smooth_iteration(iteration, z),
                    escaped: 1.0,
                    rebase_count: rebases as f32,
                    glitch: 0.0,
                },
                escape_index: Some(iteration),
            });
        }
        if iteration + 1 >= uniforms.max_iter {
            break;
        }
        let advance_reference = if robust_norm(z) < robust_norm(represented_delta) {
            if rebases >= REBASE_EXACT_LIMIT {
                return Ok(record(rebases, true));
            }
            let Some(reverse_exponent) = state.exponent.checked_neg() else {
                return Ok(record(rebases, true));
            };
            state.delta = scale(subtract(z, z_zero), reverse_exponent);
            reference_index = 0;
            rebases += 1;
            state = normalize_scaled(state);
            if state.glitch {
                return Ok(record(rebases, true));
            }
            z_zero
        } else {
            reference
        };
        state.delta = advance(
            advance_reference,
            state.delta,
            state.delta_c,
            state.exponent,
        );
        reference_index += 1;
        state = normalize_scaled(state);
        if state.glitch {
            return Ok(record(rebases, true));
        }
    }
    Ok(record(rebases, false))
}

/// Forms one normalized bottom-up pixel offset and mirrors the scaled perturbation kernel.
///
/// # Errors
///
/// Returns a typed refusal for an invalid pixel index or reference length.
pub fn perturb_scaled_pixel(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    index: u32,
) -> Result<KernelSample, KernelError> {
    let extent = GridExtent {
        width: uniforms.width,
        height: uniforms.height,
    };
    let active_len = validate_extent(extent)?;
    if index >= active_len {
        return Err(KernelError::InvalidExtent);
    }
    let offset = pixel_offset(
        index,
        extent,
        Plane {
            basis_u: uniforms.basis_u,
            basis_v: uniforms.basis_v,
        },
        uniforms.pixel_scale,
    );
    perturb_scaled_offset(uniforms, orbit, offset)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RESCALE_STEPS, ScaledState, ldexp, normalize_scaled, perturb_scaled_offset,
        perturb_scaled_pixel, scale,
    };
    use crate::{GridExtent, KernelError, PerturbUniform, RefinementLevel};
    use ember_julibrot_math::{EscapeParams, Plane, ReferenceOrbitRecord, ScaleSplit};

    const ZERO: ReferenceOrbitRecord = ReferenceOrbitRecord {
        re_hi: 0.0,
        im_hi: 0.0,
        re_lo: 0.0,
        im_lo: 0.0,
    };

    fn uniform(max_iter: u32, orbit_length: u32) -> PerturbUniform {
        PerturbUniform::pack(
            Plane {
                basis_u: [0.0, 0.0, 1.0, 0.0],
                basis_v: [0.0, 0.0, 0.0, 1.0],
            },
            ScaleSplit {
                mantissa: 0.5,
                exponent: 0,
            },
            GridExtent {
                width: 1,
                height: 1,
            },
            EscapeParams::new(max_iter),
            orbit_length,
            RefinementLevel::Final,
        )
        .expect("fixture uniform is valid")
    }

    #[test]
    fn zero_delta_caps_and_short_reference_glitches() {
        let capped = perturb_scaled_pixel(&uniform(4, 4), &[ZERO; 4], 0)
            .expect("complete reference is valid");
        assert_eq!(capped.escape_index, None);
        assert_eq!(capped.record.glitch, 0.0);
        let glitch = perturb_scaled_pixel(&uniform(4, 1), &[ZERO], 0)
            .expect("short reference is represented honestly");
        assert_eq!(glitch.record.glitch, 1.0);
        assert_eq!(glitch.record.escaped, 0.0);
    }

    #[test]
    fn nonzero_z_zero_rebase_uses_the_correct_delta() {
        let one = ReferenceOrbitRecord { re_hi: 1.0, ..ZERO };
        let sample = perturb_scaled_offset(&uniform(2, 2), &[one, one], [-0.75, 0.0, 0.0, 0.0])
            .expect("reference length matches");
        assert_eq!(sample.record.rebase_count, 1.0);
        assert_eq!(sample.record.glitch, 0.0);
        assert_eq!(sample.escape_index, None);
    }

    #[test]
    fn renormalization_preserves_delta_and_delta_c() {
        for initial in [
            ScaledState {
                delta: [2.0_f32.powi(65), 0.0],
                delta_c: [1.0, -0.5],
                exponent: -100,
                glitch: false,
            },
            ScaledState {
                delta: [2.0_f32.powi(-65), 0.0],
                delta_c: [1.0, -0.5],
                exponent: -100,
                glitch: false,
            },
        ] {
            let delta_before = scale(initial.delta, initial.exponent);
            let delta_c_before = scale(initial.delta_c, initial.exponent);
            let normalized = normalize_scaled(initial);
            assert!(!normalized.glitch);
            assert_eq!(scale(normalized.delta, normalized.exponent), delta_before);
            assert_eq!(
                scale(normalized.delta_c, normalized.exponent),
                delta_c_before
            );
        }
    }

    #[test]
    fn renormalization_repeats_until_the_smallest_subnormal_is_restored() {
        let normalized = normalize_scaled(ScaledState {
            delta: [f32::from_bits(1), 0.0],
            delta_c: [0.0; 2],
            exponent: 128,
            glitch: false,
        });
        assert!(!normalized.glitch);
        assert_eq!(normalized.exponent, 0);
        assert_eq!(normalized.delta, [2.0_f32.powi(-21), 0.0]);
        assert_eq!(MAX_RESCALE_STEPS, 67_108_863);
    }

    #[test]
    fn ldexp_clamp_boundary_preserves_signed_saturation() {
        for exponent in [512, 513] {
            assert_eq!(ldexp(1.0, exponent), f32::INFINITY);
            assert_eq!(ldexp(-1.0, exponent), f32::NEG_INFINITY);
        }
        for exponent in [-512, -513] {
            assert_eq!(ldexp(1.0, exponent).to_bits(), 0.0_f32.to_bits());
            assert_eq!(ldexp(-1.0, exponent).to_bits(), (-0.0_f32).to_bits());
        }
    }

    #[test]
    fn exponent_overflow_becomes_a_glitch() {
        let state = normalize_scaled(ScaledState {
            delta: [2.0_f32.powi(65), 0.0],
            delta_c: [0.0; 2],
            exponent: i32::MAX - 63,
            glitch: false,
        });
        assert!(state.glitch);
        assert_eq!(ldexp(1.0, -600), 0.0);
    }

    #[test]
    fn reference_length_mismatch_is_a_typed_refusal() {
        assert_eq!(
            perturb_scaled_offset(&uniform(4, 4), &[ZERO; 3], [0.0; 4]),
            Err(KernelError::ReferenceLengthMismatch)
        );
    }

    #[test]
    fn scaled_control_flow_matches_the_math_oracle() {
        let orbit = [ZERO; 8];
        let uniforms = uniform(8, 8);
        let offset = [0.0_f32; 4];
        let actual = perturb_scaled_offset(&uniforms, &orbit, offset)
            .expect("kernel mirror accepts fixture");
        let (expected, envelope) = ember_julibrot_math::perturb_scaled_f64_with_envelope(
            &orbit,
            offset.map(f64::from),
            uniforms.scale_exponent,
            EscapeParams::new(8),
        )
        .expect("math oracle accepts fixture");
        assert_eq!(
            crate::evaluate_perturbation_conformance(actual, expected, envelope).verdict,
            crate::ConformanceVerdict::Pass
        );
    }

    #[test]
    fn deep_corpus_matches_math_across_rescales_rebase_and_mixed_offsets() {
        let escaped_orbit = [
            ZERO,
            ReferenceOrbitRecord { re_hi: 2.0, ..ZERO },
            ReferenceOrbitRecord { re_hi: 6.0, ..ZERO },
            ReferenceOrbitRecord {
                re_hi: 38.0,
                ..ZERO
            },
        ];
        let zero_orbit = [ZERO, ZERO];
        let cases: &[(&[ReferenceOrbitRecord], [f32; 4], i32, u32)] = &[
            (&escaped_orbit, [0.0; 4], -900, 4),
            (&escaped_orbit, [0.25, -0.125, 0.5, 0.0], -8, 4),
            (&zero_orbit, [2.0_f32.powi(80), 0.0, 0.0, 0.0], -80, 2),
            (&zero_orbit, [2.0_f32.powi(-80), 0.0, 0.0, 0.0], 80, 2),
        ];
        for &(orbit, offset, exponent, max_iter) in cases {
            let uniforms = PerturbUniform::pack(
                Plane {
                    basis_u: [1.0, 0.0, 0.0, 0.0],
                    basis_v: [0.0, 1.0, 0.0, 0.0],
                },
                ScaleSplit {
                    mantissa: 0.5,
                    exponent,
                },
                GridExtent {
                    width: 1,
                    height: 1,
                },
                EscapeParams::new(max_iter),
                u32::try_from(orbit.len()).expect("fixture orbit length fits"),
                RefinementLevel::Final,
            )
            .expect("deep fixture uniform");
            let actual = perturb_scaled_offset(&uniforms, orbit, offset).expect("kernel mirror");
            let (expected, envelope) = ember_julibrot_math::perturb_scaled_f64_with_envelope(
                orbit,
                offset.map(f64::from),
                exponent,
                EscapeParams::new(max_iter),
            )
            .expect("math mirror");
            let result = crate::evaluate_perturbation_conformance(actual, expected, envelope);
            assert_ne!(result.verdict, crate::ConformanceVerdict::Fail);
        }
    }
}
