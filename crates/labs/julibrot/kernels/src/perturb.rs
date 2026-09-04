// CPU mirrors intentionally reproduce WGSL's fixed-width conversions and written operation order.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::imprecise_flops,
    clippy::suboptimal_flops
)]

use ember_julibrot_math::{
    EscapeGridRecord, EscapeParams, Homography, Plane, ReferenceOrbitRecord, ScaleSplit,
};

use crate::{
    GridExtent, KernelError, KernelSample, PerturbUniform, RefinementLevel, SampleStatus,
    records::{pack_map_rows, pixel_offset},
    shallow::{terminal_sample, validate_extent, validate_params},
};

const REBASE_EXACT_LIMIT: u32 = 1 << 24;
const LDEXP_EXPONENT_LIMIT: i32 = 512;
const F32_EXPONENT_MASK: u32 = 0x7f80_0000;
const F32_SIGN_MASK: u32 = 0x8000_0000;
const MAX_RESCALE_STEPS: u32 = u32::MAX / 64;
pub(crate) const PAULDELBROT_GLITCH_EPSILON: f32 = 1.0e-6;
pub(crate) const ACCUMULATED_ERROR_LIMIT: f32 = 1.0e-3;

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
        screen_to_plane: &Homography,
        scale: ScaleSplit,
        extent: GridExtent,
        params: EscapeParams,
        orbit_length: u32,
        level: RefinementLevel,
    ) -> Result<Self, KernelError> {
        Self::pack_referenced(
            plane,
            screen_to_plane,
            [0.0; 2],
            scale,
            extent,
            params,
            orbit_length,
            level,
        )
    }

    /// Packs a payload whose reference may differ from the centre of the sampled view.
    ///
    /// `centre_from_reference_px` is expressed in pixels of this level. Adding it to the
    /// homogeneous quotient makes every perturbation relative to the sampled reference while the
    /// screen map remains relative to the view centre.
    ///
    /// # Errors
    ///
    /// Returns the same typed refusals as [`Self::pack`], plus an invalid-map refusal for a
    /// non-finite displacement or translated row.
    #[allow(clippy::too_many_arguments)]
    pub fn pack_referenced(
        plane: Plane,
        screen_to_plane: &Homography,
        centre_from_reference_px: [f64; 2],
        scale: ScaleSplit,
        extent: GridExtent,
        params: EscapeParams,
        orbit_length: u32,
        level: RefinementLevel,
    ) -> Result<Self, KernelError> {
        validate_extent(extent)?;
        validate_params(params)?;
        if !finite_scalar(scale.mantissa) || !(0.5..1.0).contains(&scale.mantissa) {
            return Err(KernelError::InvalidEscapeParams);
        }
        if orbit_length == 0 || orbit_length > params.max_iter {
            return Err(KernelError::ReferenceLengthMismatch);
        }
        if !centre_from_reference_px.into_iter().all(f64::is_finite) {
            return Err(KernelError::InvalidMap);
        }
        let mut referenced_map = *screen_to_plane;
        for column in 0..3 {
            let denominator = screen_to_plane.rows[6 + column];
            referenced_map.rows[column] =
                centre_from_reference_px[0].mul_add(denominator, screen_to_plane.rows[column]);
            referenced_map.rows[3 + column] =
                centre_from_reference_px[1].mul_add(denominator, screen_to_plane.rows[3 + column]);
        }
        Ok(Self::from_parts(
            plane,
            pack_map_rows(&referenced_map)?,
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
    value.into_iter().all(finite_scalar)
}

const fn finite_scalar(value: f32) -> bool {
    value.to_bits() & F32_EXPONENT_MASK != F32_EXPONENT_MASK
}

fn ldexp(value: f32, exponent: i32) -> f32 {
    let value_bits = value.to_bits();
    if value == 0.0 || value_bits & F32_EXPONENT_MASK == F32_EXPONENT_MASK {
        return value;
    }
    let sign_bit = value_bits & F32_SIGN_MASK;
    if exponent > LDEXP_EXPONENT_LIMIT {
        return f32::from_bits(sign_bit | F32_EXPONENT_MASK);
    }
    if exponent < -LDEXP_EXPONENT_LIMIT {
        return f32::from_bits(sign_bit);
    }
    let mut result = value;
    let mut remaining = exponent;
    while remaining != 0 {
        let step = remaining.clamp(-126, 127);
        let biased_exponent = (step + 127).unsigned_abs();
        let factor = f32::from_bits(biased_exponent << 23);
        result *= factor;
        remaining -= step;
    }
    result
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
            break;
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
    state
}

const fn reconstruct(record: ReferenceOrbitRecord) -> [f32; 2] {
    [record.re, record.im]
}

fn radius_squared(value: [f32; 2]) -> f32 {
    value[0] * value[0] + value[1] * value[1]
}

fn smooth_iteration(iteration: u32, value: [f32; 2]) -> f32 {
    iteration as f32 + 1.0 - log2_norm(value).log2()
}

const fn capped(rebases: u32) -> KernelSample {
    KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: rebases as f32,
            status: 0.0,
        },
        escape_index: None,
    }
}

/// Builds the honest glitch record, carrying which of the two glitch kinds produced it.
const fn glitch(rebases: u32, exhausted: bool) -> KernelSample {
    KernelSample {
        record: EscapeGridRecord {
            smooth_iter: if exhausted {
                crate::GLITCH_REFERENCE_EXHAUSTED
            } else {
                crate::GLITCH_NUMERIC_FAILURE
            },
            escaped: 0.0,
            rebase_count: rebases as f32,
            status: 1.0,
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
    perturb_scaled_offset_with_detectors(
        uniforms,
        orbit,
        offset_prime,
        PAULDELBROT_GLITCH_EPSILON,
        Some(ACCUMULATED_ERROR_LIMIT),
    )
}

fn perturb_scaled_offset_with_detectors(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f32; 4],
    glitch_epsilon: f32,
    accumulated_error_limit: Option<f32>,
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
        return Ok(glitch(0, false));
    }
    let mut reference_index = 0_u32;
    let mut rebases = 0_u32;
    let mut accumulated_relative_error = f32::EPSILON;
    for iteration in 0..uniforms.max_iter {
        if reference_index >= uniforms.orbit_length {
            return Ok(glitch(rebases, true));
        }
        let reference = reconstruct(orbit[reference_index as usize]);
        let represented_delta = scale(state.delta, state.exponent);
        let z = add(reference, represented_delta);
        if !finite(z) {
            return Ok(glitch(rebases, false));
        }
        let z_squared = radius_squared(z);
        let reference_squared = radius_squared(reference);
        if z_squared > uniforms.bailout {
            return Ok(KernelSample {
                record: EscapeGridRecord {
                    smooth_iter: smooth_iteration(iteration, z),
                    escaped: 1.0,
                    rebase_count: rebases as f32,
                    status: 0.0,
                },
                escape_index: Some(iteration),
            });
        }
        if z_squared < glitch_epsilon * reference_squared {
            return Ok(glitch(rebases, false));
        }
        if iteration + 1 >= uniforms.max_iter {
            break;
        }
        let advance_reference = if robust_norm(z) < robust_norm(represented_delta) {
            if rebases >= REBASE_EXACT_LIMIT {
                return Ok(glitch(rebases, false));
            }
            let Some(reverse_exponent) = state.exponent.checked_neg() else {
                return Ok(glitch(rebases, false));
            };
            if let Some(limit) = accumulated_error_limit {
                let cancellation_gain = (reference_squared / z_squared).sqrt();
                accumulated_relative_error *= cancellation_gain;
                accumulated_relative_error += f32::EPSILON;
                if !finite_scalar(accumulated_relative_error) || accumulated_relative_error > limit
                {
                    return Ok(glitch(rebases, false));
                }
            }
            state.delta = scale(subtract(z, z_zero), reverse_exponent);
            reference_index = 0;
            rebases += 1;
            state = normalize_scaled(state);
            if state.glitch {
                return Ok(glitch(rebases, false));
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
            return Ok(glitch(rebases, false));
        }
    }
    Ok(capped(rebases))
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
    perturb_scaled_pixel_with_detectors(
        uniforms,
        orbit,
        index,
        PAULDELBROT_GLITCH_EPSILON,
        Some(ACCUMULATED_ERROR_LIMIT),
    )
}

fn perturb_scaled_pixel_with_detectors(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    index: u32,
    glitch_epsilon: f32,
    accumulated_error_limit: Option<f32>,
) -> Result<KernelSample, KernelError> {
    let extent = GridExtent {
        width: uniforms.width,
        height: uniforms.height,
    };
    let active_len = validate_extent(extent)?;
    if index >= active_len {
        return Err(KernelError::InvalidExtent);
    }
    let mapped = match pixel_offset(
        index,
        extent,
        Plane {
            basis_u: uniforms.basis_u,
            basis_v: uniforms.basis_v,
        },
        [
            uniforms.screen_to_plane_row_0,
            uniforms.screen_to_plane_row_1,
            uniforms.screen_to_plane_row_2,
        ],
        uniforms.pixel_scale,
    ) {
        Ok(mapped) => mapped,
        Err(status) => return Ok(terminal_sample(status)),
    };
    let mut sample = perturb_scaled_offset_with_detectors(
        uniforms,
        orbit,
        mapped.offset,
        glitch_epsilon,
        accumulated_error_limit,
    )?;
    if SampleStatus::from_f32(sample.record.status) != Some(SampleStatus::Glitch) {
        sample.record.status = mapped.status.as_f32();
    }
    Ok(sample)
}

#[cfg(test)]
pub(crate) fn perturb_scaled_pixel_for_epsilon(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    index: u32,
    glitch_epsilon: f32,
) -> Result<KernelSample, KernelError> {
    perturb_scaled_pixel_with_detectors(uniforms, orbit, index, glitch_epsilon, None)
}

#[cfg(test)]
pub(crate) fn perturb_scaled_pixel_for_accumulated_error(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    index: u32,
    accumulated_error_limit: Option<f32>,
) -> Result<KernelSample, KernelError> {
    perturb_scaled_pixel_with_detectors(
        uniforms,
        orbit,
        index,
        PAULDELBROT_GLITCH_EPSILON,
        accumulated_error_limit,
    )
}

#[cfg(test)]
pub(crate) fn perturb_scaled_offset_for_accumulated_error(
    uniforms: &PerturbUniform,
    orbit: &[ReferenceOrbitRecord],
    offset_prime: [f32; 4],
    accumulated_error_limit: Option<f32>,
) -> Result<KernelSample, KernelError> {
    perturb_scaled_offset_with_detectors(
        uniforms,
        orbit,
        offset_prime,
        PAULDELBROT_GLITCH_EPSILON,
        accumulated_error_limit,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_RESCALE_STEPS, ScaledState, finite_scalar, ldexp, normalize_scaled,
        perturb_scaled_offset, perturb_scaled_pixel, reconstruct, scale,
    };
    use crate::{GridExtent, KernelError, PerturbUniform, RefinementLevel, SampleStatus};
    use ember_julibrot_math::{EscapeParams, Homography, Plane, ReferenceOrbitRecord, ScaleSplit};

    const ZERO: ReferenceOrbitRecord = ReferenceOrbitRecord { re: 0.0, im: 0.0 };

    fn uniform(max_iter: u32, orbit_length: u32) -> PerturbUniform {
        PerturbUniform::pack(
            Plane {
                basis_u: [0.0, 0.0, 1.0, 0.0],
                basis_v: [0.0, 0.0, 0.0, 1.0],
            },
            &Homography::IDENTITY,
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
        assert_eq!(capped.record.status, 0.0);
        let glitch = perturb_scaled_pixel(&uniform(4, 1), &[ZERO], 0)
            .expect("short reference is represented honestly");
        assert_eq!(glitch.record.status, 1.0);
        assert_eq!(glitch.record.escaped, 0.0);
    }

    #[test]
    fn mapped_terminal_precedes_reference_iteration() {
        let mut uniforms = uniform(4, 4);
        uniforms.screen_to_plane_row_2 = [0.0, 0.0, -1.0, 0.0];
        let sample = perturb_scaled_pixel(&uniforms, &[ZERO; 4], 0)
            .expect("mapped terminal is a valid result");
        assert_eq!(
            sample,
            crate::shallow::terminal_sample(SampleStatus::Horizon)
        );
    }

    #[test]
    fn mapped_uncertainty_runs_the_scaled_recurrence_with_sticky_status() {
        let mut uniforms = uniform(4, 4);
        uniforms.width = 2;
        uniforms.screen_to_plane_row_0 = [0.0, 0.0, 1.0, 0.0];
        uniforms.screen_to_plane_row_2 = [1.0, 0.0, 0.500_000_06, 0.0];
        let sample = perturb_scaled_pixel(&uniforms, &[ZERO; 4], 0)
            .expect("uncertain mapped pixel remains sampleable");
        assert_eq!(sample.record.status, SampleStatus::MapUncertain.as_f32());
        assert_eq!(sample.record.escaped, 1.0);
        assert_eq!(sample.escape_index, Some(1));
    }

    #[test]
    fn nonzero_z_zero_rebase_uses_the_correct_delta() {
        for mode in ember_julibrot_math::PrecisionMode::ALL {
            let one = ReferenceOrbitRecord { re: 1.0, ..ZERO };
            let sample = perturb_scaled_offset(&uniform(2, 2), &[one, one], [-0.75, 0.0, 0.0, 0.0])
                .expect("reference length matches");
            assert!(sample.record.rebase_count >= 0.0);
            if mode.requires_bit_identity() {
                // Deterministic-only contract: the CPU mirror rebases on the identical iteration.
                assert_eq!(sample.record.rebase_count, 1.0);
            }
            assert_eq!(sample.record.status, 0.0);
            assert_eq!(sample.escape_index, None);
        }
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
    fn ldexp_bit_construction_preserves_guards_and_gradual_underflow() {
        assert_eq!(ldexp(1.0, 127).to_bits(), 254_u32 << 23);
        assert_eq!(ldexp(1.0, -126).to_bits(), 1_u32 << 23);
        assert_eq!(ldexp(1.5, -127).to_bits(), 0x0060_0000);
        assert_eq!(ldexp(f32::from_bits(1), 127), 2.0_f32.powi(-22));
        for value in [
            0.0,
            -0.0,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::from_bits(0x7fc0_1234),
        ] {
            assert_eq!(ldexp(value, 17).to_bits(), value.to_bits());
        }
        assert!(finite_scalar(f32::MAX));
        assert!(!finite_scalar(f32::INFINITY));
        assert!(!finite_scalar(f32::from_bits(0x7fc0_1234)));
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
        for mode in ember_julibrot_math::PrecisionMode::ALL {
            assert_eq!(
                crate::evaluate_perturbation_conformance(mode, actual, expected, envelope).verdict,
                crate::ConformanceVerdict::Pass
            );
        }
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "this is the explicit corpus measurement"
    )]
    fn deep_corpus_matches_math_across_rescales_rebase_and_mixed_offsets() {
        let escaped_orbit = [
            ZERO,
            ReferenceOrbitRecord { re: 2.0, ..ZERO },
            ReferenceOrbitRecord { re: 6.0, ..ZERO },
            ReferenceOrbitRecord { re: 38.0, ..ZERO },
        ];
        let zero_orbit = [ZERO, ZERO];
        let cases: &[(&[ReferenceOrbitRecord], [f32; 4], i32, u32)] = &[
            (&escaped_orbit, [0.0; 4], -900, 4),
            (&escaped_orbit, [0.25, -0.125, 0.5, 0.0], -8, 4),
            (&zero_orbit, [2.0_f32.powi(80), 0.0, 0.0, 0.0], -80, 2),
            (&zero_orbit, [2.0_f32.powi(-80), 0.0, 0.0, 0.0], 80, 2),
        ];
        let mut boundary_count = 0_usize;
        for &(orbit, offset, exponent, max_iter) in cases {
            let uniforms = PerturbUniform::pack(
                Plane {
                    basis_u: [1.0, 0.0, 0.0, 0.0],
                    basis_v: [0.0, 1.0, 0.0, 0.0],
                },
                &Homography::IDENTITY,
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
            for mode in ember_julibrot_math::PrecisionMode::ALL {
                let result =
                    crate::evaluate_perturbation_conformance(mode, actual, expected, envelope);
                assert_ne!(result.verdict, crate::ConformanceVerdict::Fail);
                boundary_count += usize::from(result.boundary);
            }
        }
        eprintln!(
            "perturbation_interior_envelope corpus={} boundaries={boundary_count} violations=0",
            cases.len()
        );
        assert_eq!(boundary_count, 0);
    }

    #[test]
    #[allow(
        clippy::print_stderr,
        reason = "this is the explicit boundary measurement"
    )]
    fn boundary_envelope_is_conservative_and_within_four_times_observed_error() {
        let boundary = ReferenceOrbitRecord {
            re: f32::from_bits(16.0_f32.to_bits() - 1),
            im: 0.0,
        };
        let offset = [2.0_f32.powi(-21), 0.0, 0.0, 0.0];
        let uniforms = uniform(1, 1);
        let actual =
            perturb_scaled_offset(&uniforms, &[boundary], offset).expect("boundary kernel mirror");
        let (expected, envelope) = ember_julibrot_math::perturb_scaled_f64_with_envelope(
            &[boundary],
            offset.map(f64::from),
            uniforms.scale_exponent,
            EscapeParams::new(1),
        )
        .expect("boundary math mirror");
        let result = crate::evaluate_perturbation_conformance(
            ember_julibrot_math::PrecisionMode::PictureFast,
            actual,
            expected,
            envelope,
        );
        assert_eq!(result.verdict, crate::ConformanceVerdict::Boundary);
        let mut gpu = reconstruct(boundary);
        gpu[0] += offset[0];
        let exact_re = f64::from(boundary.re) + f64::from(offset[0]);
        let observed_norm_error =
            (f64::from(gpu[0] * gpu[0] + gpu[1] * gpu[1]) - exact_re * exact_re).abs();
        eprintln!(
            "perturbation_boundary_envelope existing_corpus=4 existing_boundaries=0 repaired_corpus=5 repaired_boundaries=1 observed_norm_error={observed_norm_error:e} envelope_norm_error={:e} tightness={:e}",
            envelope.escape_norm2_error,
            envelope.escape_norm2_error / observed_norm_error,
        );
        assert!(envelope.escape_norm2_error >= observed_norm_error);
        assert!(envelope.escape_norm2_error <= 4.0 * observed_norm_error);
    }
}
