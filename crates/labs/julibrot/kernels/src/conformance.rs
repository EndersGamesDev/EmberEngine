use ember_julibrot_math::{EscapeSample, PerturbSample, PerturbationEnvelope, PrecisionMode};

use crate::{KernelMode, KernelSample, SampleStatus};

/// Accepted absolute smooth-iteration error for shallow conformance.
pub const SHALLOW_SMOOTH_TOLERANCE: f32 = 1.0e-4;

/// Accepted absolute smooth-iteration error for scaled perturbation conformance.
pub const PERTURB_SMOOTH_TOLERANCE: f32 = 2.0e-3;

const MAX_EXACT_REBASE_COUNT: f32 = 16_777_216.0;

/// Honest result class for one CPU-oracle versus GPU-readback comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ConformanceVerdict {
    /// Every required comparison passed outside the propagated boundary envelope.
    Pass = 0,
    /// The sample was inside the precomputed perturbation boundary envelope.
    Boundary = 1,
    /// A required exact comparison, record law, or smooth tolerance failed.
    Fail = 2,
}

/// Complete comparison facts for one deterministic conformance sample.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConformanceResult {
    pub precision_mode: &'static str,
    pub verdict: ConformanceVerdict,
    pub boundary: bool,
    pub record_well_formed: bool,
    pub classification_exact: bool,
    pub escape_index_exact: bool,
    pub rebase_count_exact: bool,
    pub glitch_exact: bool,
    pub smooth_abs_error: f32,
    pub smooth_tolerance: f32,
}

/// One browser-only observation that must never be inferred from native or wasm compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VisibleReplayCard {
    pub id: &'static str,
    pub requirement: &'static str,
}

/// Browser observations left explicitly unqualified by native conformance.
pub const VISIBLE_REPLAY_CARDS: &[VisibleReplayCard] = &[
    VisibleReplayCard {
        id: "shallow-readback",
        requirement: "requires visible replay: shallow RGBA32F and auxiliary index readback",
    },
    VisibleReplayCard {
        id: "perturbation-readback",
        requirement: "requires visible replay: scaled perturbation RGBA32F and index readback",
    },
    VisibleReplayCard {
        id: "scratch-copy",
        requirement: "requires visible replay: exact rows and tail reach nonzero DATA origins",
    },
    VisibleReplayCard {
        id: "present-consumption",
        requirement: "requires visible replay: bottom-up active prefix is sampled without feedback",
    },
    VisibleReplayCard {
        id: "binding-identity",
        requirement: "requires visible replay: all levels and orbit changes retain heap bindings",
    },
    VisibleReplayCard {
        id: "pipeline-switch",
        requirement: "requires visible replay: zoom-14 shallow/deep continuity after warm-up",
    },
    VisibleReplayCard {
        id: "fence-handoff",
        requirement: "requires visible replay: four-byte scene fence completes before presentation",
    },
];

/// Checks the production-grid record invariants without interpreting browser measurements.
#[must_use]
pub fn record_is_well_formed(sample: KernelSample, mode: KernelMode) -> bool {
    let record = sample.record;
    let Some(escaped) = binary_flag(record.escaped) else {
        return false;
    };
    let Some(status) = SampleStatus::from_f32(record.status) else {
        return false;
    };
    let glitch = status == SampleStatus::Glitch;
    let rebase_count = exact_rebase_count(record.rebase_count);
    let index_matches = escaped == sample.escape_index.is_some();
    let terminal_matches = match status {
        SampleStatus::Sampled | SampleStatus::MapUncertain if escaped => {
            record.smooth_iter.is_finite()
        }
        SampleStatus::Glitch if !escaped => {
            record.smooth_iter.to_bits() == crate::GLITCH_REFERENCE_EXHAUSTED.to_bits()
                || record.smooth_iter.to_bits() == crate::GLITCH_NUMERIC_FAILURE.to_bits()
        }
        SampleStatus::Sampled | SampleStatus::MapUncertain if !escaped => {
            record.smooth_iter.to_bits() == (-1.0_f32).to_bits()
        }
        SampleStatus::Horizon => {
            !escaped
                && sample.escape_index.is_none()
                && record.smooth_iter.to_bits() == (-1.0_f32).to_bits()
                && record.rebase_count.to_bits() == 0
        }
        _ => false,
    };
    let mode_matches =
        mode != KernelMode::Shallow || (record.rebase_count.to_bits() == 0 && !glitch);
    rebase_count.is_some() && index_matches && terminal_matches && mode_matches
}

/// Compares one shallow readback sample with math's binary32 oracle.
#[must_use]
pub fn evaluate_shallow_conformance(
    precision_mode: PrecisionMode,
    observed: KernelSample,
    expected: EscapeSample,
) -> ConformanceResult {
    let classification_exact = binary_flag(observed.record.escaped) == Some(expected.escaped);
    let escape_index_exact = observed.escape_index == expected.escape_index;
    let smooth_abs_error = smooth_error(observed.record.smooth_iter, expected.smooth_iter);
    let record_well_formed = record_is_well_formed(observed, KernelMode::Shallow);
    let passes = record_well_formed
        && classification_exact
        && escape_index_exact
        && smooth_abs_error <= SHALLOW_SMOOTH_TOLERANCE;
    ConformanceResult {
        precision_mode: precision_mode.as_str(),
        verdict: if passes {
            ConformanceVerdict::Pass
        } else {
            ConformanceVerdict::Fail
        },
        boundary: false,
        record_well_formed,
        classification_exact,
        escape_index_exact,
        rebase_count_exact: observed.record.rebase_count == 0.0,
        glitch_exact: SampleStatus::from_f32(observed.record.status) == Some(SampleStatus::Sampled),
        smooth_abs_error,
        smooth_tolerance: SHALLOW_SMOOTH_TOLERANCE,
    }
}

/// Compares one perturbation readback sample with math's binary64 oracle and error envelope.
#[must_use]
pub fn evaluate_perturbation_conformance(
    precision_mode: PrecisionMode,
    observed: KernelSample,
    expected: PerturbSample,
    envelope: PerturbationEnvelope,
) -> ConformanceResult {
    let boundary = envelope.minimum_escape_margin <= envelope.escape_norm2_error;
    let classification_exact = binary_flag(observed.record.escaped) == Some(expected.escaped);
    let escape_index_exact = observed.escape_index == expected.escape_index;
    let rebase_count_exact =
        exact_rebase_count(observed.record.rebase_count) == Some(expected.rebase_count);
    let glitch_exact = SampleStatus::from_f32(observed.record.status)
        .map(|status| status == SampleStatus::Glitch)
        == Some(expected.glitch);
    let smooth_abs_error = smooth_error(observed.record.smooth_iter, expected.smooth_iter);
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the finite binary64 envelope is compared with a binary32 kernel readback"
    )]
    let smooth_tolerance = (envelope.smooth_error as f32).max(PERTURB_SMOOTH_TOLERANCE);
    let record_well_formed = record_is_well_formed(observed, KernelMode::Perturbation);
    let common = record_well_formed
        && (!precision_mode.requires_bit_identity() || rebase_count_exact)
        && glitch_exact
        && smooth_abs_error <= smooth_tolerance;
    let verdict = if !common {
        ConformanceVerdict::Fail
    } else if boundary {
        ConformanceVerdict::Boundary
    } else if classification_exact && escape_index_exact {
        ConformanceVerdict::Pass
    } else {
        ConformanceVerdict::Fail
    };
    ConformanceResult {
        precision_mode: precision_mode.as_str(),
        verdict,
        boundary,
        record_well_formed,
        classification_exact,
        escape_index_exact,
        rebase_count_exact,
        glitch_exact,
        smooth_abs_error,
        smooth_tolerance,
    }
}

const fn binary_flag(value: f32) -> Option<bool> {
    match value.to_bits() {
        0 => Some(false),
        bits if bits == 1.0_f32.to_bits() => Some(true),
        _ => None,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn exact_rebase_count(value: f32) -> Option<u32> {
    let count = value as u32;
    (value.is_finite()
        && (0.0..=MAX_EXACT_REBASE_COUNT).contains(&value)
        && value.to_bits() == (count as f32).to_bits())
    .then_some(count)
}

fn smooth_error(observed: f32, expected: f32) -> f32 {
    if observed.is_finite() && expected.is_finite() {
        (observed - expected).abs()
    } else {
        f32::INFINITY
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;
    use std::{collections::BTreeSet, time::Instant};

    use ember_julibrot_math::{
        BigCentre, EscapeGridRecord, EscapeParams, Homography, ObjectAngles, OrbitStep,
        PerturbationEnvelope, Plane, PrecisionMode, ReferenceOrbitBuilder, ReferenceOrbitRecord,
        ScaleSplit, construct_plane, escape_f32, pixel_scale, precision_for, scale_split,
    };

    use super::{
        ConformanceVerdict, VISIBLE_REPLAY_CARDS, evaluate_perturbation_conformance,
        evaluate_shallow_conformance,
    };
    use crate::perturb::{
        ACCUMULATED_ERROR_LIMIT, PAULDELBROT_GLITCH_EPSILON,
        perturb_scaled_offset_for_accumulated_error, perturb_scaled_pixel_for_accumulated_error,
        perturb_scaled_pixel_for_epsilon,
    };
    use crate::{
        GridExtent, KernelSample, PerturbUniform, RefinementLevel, SampleStatus,
        escape_shallow_point, perturb_scaled_pixel,
    };

    #[allow(clippy::suboptimal_flops)]
    fn exact_mandelbrot_sample(c: [f64; 2], cap: u32) -> (Option<u32>, f64) {
        let mut z = [0.0_f64; 2];
        for iteration in 0..cap {
            let norm_squared = z[0] * z[0] + z[1] * z[1];
            if norm_squared > f64::from(EscapeParams::BAILOUT) {
                let smooth = f64::from(iteration) + 1.0 - norm_squared.sqrt().log2().log2();
                return (Some(iteration), smooth);
            }
            if iteration + 1 == cap {
                break;
            }
            z = [z[0] * z[0] - z[1] * z[1] + c[0], 2.0 * z[0] * z[1] + c[1]];
        }
        (None, -1.0)
    }

    fn exact_mandelbrot_escape_index(c: [f64; 2], cap: u32) -> Option<u32> {
        exact_mandelbrot_sample(c, cap).0
    }

    fn old_sample_is_wrong(old: KernelSample, exact_index: Option<u32>, exact_smooth: f64) -> bool {
        match (old.escape_index, exact_index) {
            (Some(old_index), Some(exact_index)) => {
                old_index.abs_diff(exact_index) > 1
                    || (f64::from(old.record.smooth_iter) - exact_smooth).abs() > 1.0
            }
            (None, None) => false,
            _ => true,
        }
    }

    #[test]
    fn pauldelbrot_cancellation_is_a_numeric_glitch() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const REFERENCE_LENGTH: u32 = 41;
        const PIXEL_INDEX: u32 = 696;
        const EXACT_ESCAPE_INDEX: u32 = 250;
        let target = [-0.743_643_887_037_151, 0.131_825_904_205_33];
        let plan = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64([0.0, 0.0, target[0], target[1]], plan.requested_bits)
            .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            REFERENCE_LENGTH,
            RefinementLevel::Final,
        )
        .expect("short-reference uniform");
        let before = perturb_scaled_pixel_for_epsilon(&uniforms, &orbit.records, PIXEL_INDEX, 0.0)
            .expect("old kernel mirror");
        let sample = perturb_scaled_pixel(&uniforms, &orbit.records, PIXEL_INDEX)
            .expect("cancellation pixel is valid");
        let scale = pixel_scale(14.0, WIDTH).expect("Final pixel scale");
        let x = 0.5_f64.mul_add(-f64::from(WIDTH), f64::from(PIXEL_INDEX % WIDTH) + 0.5);
        let y = 0.5_f64.mul_add(-f64::from(HEIGHT), f64::from(PIXEL_INDEX / WIDTH) + 0.5);
        let c = [x.mul_add(scale, target[0]), y.mul_add(scale, target[1])];
        let gpu_source = crate::perturbation_kernel().body;
        let gpu_escape = gpu_source
            .find("if (z_squared > uniforms.bailout)")
            .expect("GPU escape test");
        let gpu_glitch = gpu_source
            .find("if (z_squared < 0.000001 * reference_squared)")
            .expect("GPU Pauldelbrot test");
        let gpu_accumulated = gpu_source
            .find("accumulated_relative_error > 0.001")
            .expect("GPU accumulated-error test");
        let gpu_rebase = gpu_source
            .find("if (perturb_norm(z) < perturb_norm(represented_delta))")
            .expect("GPU rebase test");

        assert_eq!(
            exact_mandelbrot_escape_index(c, CAP),
            Some(EXACT_ESCAPE_INDEX)
        );
        assert_eq!(before.escape_index, None);
        assert_eq!(before.record.smooth_iter, -1.0);
        assert_eq!(before.record.rebase_count, 27.0);
        assert_eq!(
            SampleStatus::from_f32(before.record.status),
            Some(SampleStatus::Sampled)
        );
        assert!(gpu_escape < gpu_glitch && gpu_glitch < gpu_rebase);
        assert!(gpu_rebase < gpu_accumulated);
        assert_eq!(sample.escape_index, None);
        assert_eq!(
            SampleStatus::from_f32(sample.record.status),
            Some(SampleStatus::Glitch)
        );
        assert_eq!(sample.record.smooth_iter, crate::GLITCH_NUMERIC_FAILURE);
    }

    #[test]
    fn a_true_interior_pixel_is_not_a_relative_precision_glitch() {
        const CAP: u32 = 512;
        let plan = precision_for(14.0, 960, CAP).expect("zoom fourteen precision");
        let centre =
            BigCentre::from_f64([0.0; 4], plan.requested_bits).expect("finite interior reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, 960).expect("zoom fourteen scale"),
            GridExtent {
                width: 1,
                height: 1,
            },
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("interior uniform");
        let sample =
            perturb_scaled_pixel(&uniforms, &orbit.records, 0).expect("interior pixel is valid");

        assert_eq!(orbit.length, CAP);
        assert_eq!(exact_mandelbrot_escape_index([0.0; 2], CAP), None);
        assert_eq!(sample.escape_index, None);
        assert_eq!(sample.record.smooth_iter, -1.0);
        assert_eq!(
            SampleStatus::from_f32(sample.record.status),
            Some(SampleStatus::Sampled)
        );
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected epsilon sweep reports the measured fractions"
    )]
    fn measures_pauldelbrot_epsilon_on_standard_seahorse_corpus() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const REFERENCE_LENGTH: u32 = 41;
        let plan = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64(
            [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
            plan.requested_bits,
        )
        .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            REFERENCE_LENGTH,
            RefinementLevel::Final,
        )
        .expect("standard-corpus uniform");
        let corpus = WIDTH * HEIGHT;

        for epsilon in [1.0e-4_f32, PAULDELBROT_GLITCH_EPSILON, 1.0e-8_f32] {
            let flagged = (0..corpus)
                .map(|index| {
                    perturb_scaled_pixel_for_epsilon(&uniforms, &orbit.records, index, epsilon)
                        .expect("standard-corpus pixel")
                })
                .filter(|sample| {
                    sample.record.smooth_iter.to_bits() == crate::GLITCH_NUMERIC_FAILURE.to_bits()
                })
                .count();
            #[allow(clippy::cast_precision_loss)]
            let fraction = flagged as f64 / f64::from(corpus);
            eprintln!(
                "pauldelbrot_epsilon epsilon={epsilon:e} corpus={corpus} flagged={flagged} fraction={fraction:.9}"
            );
        }
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        clippy::too_many_lines,
        reason = "the explicitly selected error-bound sweep reports measured fractions"
    )]
    fn measures_accumulated_error_bounds_on_seahorse_corpus() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const REFERENCE_LENGTH: u32 = 41;
        let target = [-0.743_643_887_037_151, 0.131_825_904_205_33];
        let plan = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64([0.0, 0.0, target[0], target[1]], plan.requested_bits)
            .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            REFERENCE_LENGTH,
            RefinementLevel::Final,
        )
        .expect("standard-corpus uniform");
        let scale = pixel_scale(14.0, WIDTH).expect("Final pixel scale");
        let corpus = WIDTH * HEIGHT;

        for limit in [1.0e-4_f32, ACCUMULATED_ERROR_LIMIT, 1.0e-2_f32] {
            let pinned = perturb_scaled_pixel_for_accumulated_error(
                &uniforms,
                &orbit.records,
                696,
                Some(limit),
            )
            .expect("pinned corpus pixel");
            let mut flagged = 0_u32;
            let mut no_escape_within_cap = 0_u32;
            let mut verified_correct_escape = 0_u32;
            let mut detected_wrong = 0_u32;
            for index in 0..corpus {
                let sample = perturb_scaled_pixel_for_accumulated_error(
                    &uniforms,
                    &orbit.records,
                    index,
                    Some(limit),
                )
                .expect("standard-corpus pixel");
                if sample.record.smooth_iter.to_bits() != crate::GLITCH_NUMERIC_FAILURE.to_bits() {
                    continue;
                }
                flagged += 1;
                let x = 0.5_f64.mul_add(-f64::from(WIDTH), f64::from(index % WIDTH) + 0.5);
                let y = 0.5_f64.mul_add(-f64::from(HEIGHT), f64::from(index / WIDTH) + 0.5);
                let c = [x.mul_add(scale, target[0]), y.mul_add(scale, target[1])];
                let exact = exact_mandelbrot_escape_index(c, CAP);
                let baseline = perturb_scaled_pixel_for_accumulated_error(
                    &uniforms,
                    &orbit.records,
                    index,
                    None,
                )
                .expect("baseline corpus pixel");
                match exact {
                    None => no_escape_within_cap += 1,
                    Some(exact_index) if baseline.escape_index == Some(exact_index) => {
                        verified_correct_escape += 1;
                    }
                    Some(_) => detected_wrong += 1,
                }
            }
            #[allow(clippy::cast_precision_loss)]
            let fraction = f64::from(flagged) / f64::from(corpus);
            eprintln!(
                "accumulated_error limit={limit:e} corpus={corpus} flagged={flagged} fraction={fraction:.9} detected_wrong={detected_wrong} verified_correct_escape={verified_correct_escape} no_escape_within_cap={no_escape_within_cap} pinned_status={} pinned_rebases={}",
                pinned.record.status, pinned.record.rebase_count
            );
        }

        let escaped_orbit = [
            ReferenceOrbitRecord { re: 0.0, im: 0.0 },
            ReferenceOrbitRecord { re: 2.0, im: 0.0 },
            ReferenceOrbitRecord { re: 6.0, im: 0.0 },
            ReferenceOrbitRecord { re: 38.0, im: 0.0 },
        ];
        let zero_orbit = [ReferenceOrbitRecord { re: 0.0, im: 0.0 }; 2];
        let standard_cases: &[(&[ReferenceOrbitRecord], [f32; 4], i32, u32)] = &[
            (&escaped_orbit, [0.0; 4], -900, 4),
            (&escaped_orbit, [0.25, -0.125, 0.5, 0.0], -8, 4),
            (&zero_orbit, [2.0_f32.powi(80), 0.0, 0.0, 0.0], -80, 2),
            (&zero_orbit, [2.0_f32.powi(-80), 0.0, 0.0, 0.0], 80, 2),
        ];
        for limit in [1.0e-4_f32, ACCUMULATED_ERROR_LIMIT, 1.0e-2_f32] {
            let mut flagged = 0_usize;
            let mut false_positive = 0_usize;
            for &(case_orbit, offset, exponent, max_iter) in standard_cases {
                let case_uniforms = PerturbUniform::pack(
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
                    u32::try_from(case_orbit.len()).expect("fixture orbit length fits"),
                    RefinementLevel::Final,
                )
                .expect("standard conformance uniform");
                let sample = perturb_scaled_offset_for_accumulated_error(
                    &case_uniforms,
                    case_orbit,
                    offset,
                    Some(limit),
                )
                .expect("standard conformance sample");
                let numeric =
                    sample.record.smooth_iter.to_bits() == crate::GLITCH_NUMERIC_FAILURE.to_bits();
                flagged += usize::from(numeric);
                let (exact, _) = ember_julibrot_math::perturb_scaled_f64_with_envelope(
                    case_orbit,
                    offset.map(f64::from),
                    exponent,
                    EscapeParams::new(max_iter),
                )
                .expect("standard conformance oracle");
                false_positive += usize::from(numeric && !exact.glitch);
            }
            #[allow(clippy::cast_precision_loss)]
            let fraction = flagged as f64 / standard_cases.len() as f64;
            eprintln!(
                "accumulated_error_standard limit={limit:e} corpus={} flagged={flagged} fraction={fraction:.9} false_positive={false_positive}",
                standard_cases.len()
            );
        }
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        clippy::too_many_lines,
        reason = "the explicit corrected-Final audit compares every detector flag with binary64"
    )]
    fn measures_corrected_final_detector_precision() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const UNFLAGGED_SAMPLE: usize = 1_024;
        let view_centre = [-0.743_643_887_037_151, 0.131_825_904_205_33];
        let reference_centre = [-0.743_753_114_541_220_1, 0.131_757_366_807_549_76];
        let plan = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64(
            [0.0, 0.0, reference_centre[0], reference_centre[1]],
            plan.requested_bits,
        )
        .expect("finite corrected reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack_referenced(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            [429.5, 269.5],
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("corrected-Final uniform");
        let scale = pixel_scale(14.0, WIDTH).expect("Final pixel scale");
        let corpus = WIDTH * HEIGHT;
        let mut flagged = Vec::new();
        let mut production = Vec::with_capacity(usize::try_from(corpus).expect("corpus fits"));
        for index in 0..corpus {
            let sample = perturb_scaled_pixel(&uniforms, &orbit.records, index)
                .expect("corrected-Final pixel");
            if sample.record.smooth_iter.to_bits() == crate::GLITCH_NUMERIC_FAILURE.to_bits() {
                flagged.push(index);
            }
            production.push(sample);
        }

        let mut old_wrong = 0_usize;
        let mut old_right = 0_usize;
        let mut old_glitch = 0_usize;
        for &index in &flagged {
            let old =
                perturb_scaled_pixel_for_accumulated_error(&uniforms, &orbit.records, index, None)
                    .expect("detector-disabled pixel");
            if SampleStatus::from_f32(old.record.status) == Some(SampleStatus::Glitch) {
                old_glitch += 1;
                continue;
            }
            let x = 0.5_f64.mul_add(-f64::from(WIDTH), f64::from(index % WIDTH) + 0.5);
            let y = 0.5_f64.mul_add(-f64::from(HEIGHT), f64::from(index / WIDTH) + 0.5);
            let c = [
                x.mul_add(scale, view_centre[0]),
                y.mul_add(scale, view_centre[1]),
            ];
            let (exact_index, exact_smooth) = exact_mandelbrot_sample(c, CAP);
            if old_sample_is_wrong(old, exact_index, exact_smooth) {
                old_wrong += 1;
            } else {
                old_right += 1;
            }
        }
        let comparable = old_wrong + old_right;
        #[allow(clippy::cast_precision_loss)]
        let true_positive_fraction = old_wrong as f64 / comparable as f64;

        let mut unflagged_checked = 0_usize;
        let mut unflagged_wrong = 0_usize;
        for step in 0..corpus {
            let index = step * 509 % corpus;
            if production[usize::try_from(index).expect("sample index fits")]
                .record
                .smooth_iter
                .to_bits()
                == crate::GLITCH_NUMERIC_FAILURE.to_bits()
            {
                continue;
            }
            let old =
                perturb_scaled_pixel_for_accumulated_error(&uniforms, &orbit.records, index, None)
                    .expect("unflagged detector-disabled pixel");
            if SampleStatus::from_f32(old.record.status) == Some(SampleStatus::Glitch) {
                continue;
            }
            let x = 0.5_f64.mul_add(-f64::from(WIDTH), f64::from(index % WIDTH) + 0.5);
            let y = 0.5_f64.mul_add(-f64::from(HEIGHT), f64::from(index / WIDTH) + 0.5);
            let c = [
                x.mul_add(scale, view_centre[0]),
                y.mul_add(scale, view_centre[1]),
            ];
            let (exact_index, exact_smooth) = exact_mandelbrot_sample(c, CAP);
            unflagged_wrong += usize::from(old_sample_is_wrong(old, exact_index, exact_smooth));
            unflagged_checked += 1;
            if unflagged_checked == UNFLAGGED_SAMPLE {
                break;
            }
        }
        #[allow(clippy::cast_precision_loss)]
        let false_negative_fraction = unflagged_wrong as f64 / unflagged_checked as f64;
        eprintln!(
            "corrected_final_precision limit={ACCUMULATED_ERROR_LIMIT:e} corpus={corpus} flagged={} old_wrong={old_wrong} old_right={old_right} old_glitch={old_glitch} true_positive_fraction={true_positive_fraction:.9} unflagged_checked={unflagged_checked} unflagged_wrong={unflagged_wrong} false_negative_fraction={false_negative_fraction:.9}",
            flagged.len()
        );

        let pin_centre = BigCentre::from_f64(
            [0.0, 0.0, view_centre[0], view_centre[1]],
            plan.requested_bits,
        )
        .expect("finite pin reference");
        let mut pin_builder = ReferenceOrbitBuilder::new(&pin_centre, plan, EscapeParams::new(CAP))
            .expect("pin reference builder");
        let pin_orbit = loop {
            match pin_builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("pin reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let pin_uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            41,
            RefinementLevel::Final,
        )
        .expect("pin uniform");
        for limit in [3.0e-3_f32, 1.0e-2_f32, 3.0e-2_f32] {
            let pin = perturb_scaled_pixel_for_accumulated_error(
                &pin_uniforms,
                &pin_orbit.records,
                696,
                Some(limit),
            )
            .expect("pin pixel");
            let mut candidate_flagged = 0_usize;
            let mut candidate_wrong = 0_usize;
            let mut candidate_right = 0_usize;
            let mut candidate_old_glitch = 0_usize;
            for index in 0..corpus {
                let sample = perturb_scaled_pixel_for_accumulated_error(
                    &uniforms,
                    &orbit.records,
                    index,
                    Some(limit),
                )
                .expect("candidate-bound pixel");
                if sample.record.smooth_iter.to_bits() != crate::GLITCH_NUMERIC_FAILURE.to_bits() {
                    continue;
                }
                candidate_flagged += 1;
                let old = perturb_scaled_pixel_for_accumulated_error(
                    &uniforms,
                    &orbit.records,
                    index,
                    None,
                )
                .expect("candidate detector-disabled pixel");
                if SampleStatus::from_f32(old.record.status) == Some(SampleStatus::Glitch) {
                    candidate_old_glitch += 1;
                    continue;
                }
                let x = 0.5_f64.mul_add(-f64::from(WIDTH), f64::from(index % WIDTH) + 0.5);
                let y = 0.5_f64.mul_add(-f64::from(HEIGHT), f64::from(index / WIDTH) + 0.5);
                let c = [
                    x.mul_add(scale, view_centre[0]),
                    y.mul_add(scale, view_centre[1]),
                ];
                let (exact_index, exact_smooth) = exact_mandelbrot_sample(c, CAP);
                if old_sample_is_wrong(old, exact_index, exact_smooth) {
                    candidate_wrong += 1;
                } else {
                    candidate_right += 1;
                }
            }
            let candidate_comparable = candidate_wrong + candidate_right;
            #[allow(clippy::cast_precision_loss)]
            let false_positive_fraction = candidate_right as f64 / candidate_comparable as f64;
            eprintln!(
                "corrected_final_candidate limit={limit:e} flagged={candidate_flagged} old_wrong={candidate_wrong} old_right={candidate_right} old_glitch={candidate_old_glitch} false_positive_fraction={false_positive_fraction:.9} pin_status={} pin_rebases={}",
                pin.record.status, pin.record.rebase_count
            );
        }
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports its wall"
    )]
    fn measures_pauldelbrot_comparison_cost() {
        const CAP: u32 = 512;
        const ROUNDS: u32 = 200_000;
        let plan = precision_for(14.0, 960, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64(
            [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
            plan.requested_bits,
        )
        .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, 960).expect("zoom fourteen scale"),
            GridExtent {
                width: 1,
                height: 1,
            },
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("measurement uniform");
        let start = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(
                perturb_scaled_pixel(&uniforms, &orbit.records, 0).expect("interior sample"),
            );
        }
        let wall = start.elapsed();
        let iterations = u128::from(ROUNDS) * u128::from(CAP);
        eprintln!(
            "pauldelbrot_cost rounds={ROUNDS} iterations={iterations} wall_us={} ps_per_iteration={}",
            wall.as_micros(),
            wall.as_nanos() * 1_000 / iterations
        );
    }

    #[test]
    #[ignore = "native kernels measurement harness"]
    #[allow(
        clippy::print_stderr,
        reason = "the explicitly selected performance harness reports per-rebase cost"
    )]
    fn measures_accumulated_error_cost_per_rebase() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const ROUNDS: u32 = 100_000;
        let plan = precision_for(14.0, WIDTH, CAP).expect("zoom fourteen precision");
        let centre = BigCentre::from_f64(
            [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
            plan.requested_bits,
        )
        .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let uniforms = PerturbUniform::pack(
            construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane"),
            &Homography::IDENTITY,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            41,
            RefinementLevel::Final,
        )
        .expect("cost uniform");
        let baseline =
            perturb_scaled_pixel_for_accumulated_error(&uniforms, &orbit.records, 696, None)
                .expect("baseline pixel");
        assert_eq!(baseline.record.rebase_count, 27.0);
        std::hint::black_box(
            perturb_scaled_pixel_for_accumulated_error(
                &uniforms,
                &orbit.records,
                696,
                Some(f32::INFINITY),
            )
            .expect("warm accumulated pixel"),
        );

        let baseline_start = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(
                perturb_scaled_pixel_for_accumulated_error(&uniforms, &orbit.records, 696, None)
                    .expect("baseline pixel"),
            );
        }
        let baseline_wall = baseline_start.elapsed();
        let accumulated_start = Instant::now();
        for _ in 0..ROUNDS {
            std::hint::black_box(
                perturb_scaled_pixel_for_accumulated_error(
                    &uniforms,
                    &orbit.records,
                    696,
                    Some(f32::INFINITY),
                )
                .expect("accumulated pixel"),
            );
        }
        let accumulated_wall = accumulated_start.elapsed();
        let rebase_events = f64::from(ROUNDS) * f64::from(baseline.record.rebase_count);
        let delta_ns_per_rebase =
            (accumulated_wall.as_secs_f64() - baseline_wall.as_secs_f64()) * 1.0e9 / rebase_events;
        eprintln!(
            "accumulated_error_cost rounds={ROUNDS} rebases_per_round={} baseline_us={} accumulated_us={} delta_ns_per_rebase={delta_ns_per_rebase:.3}",
            baseline.record.rebase_count,
            baseline_wall.as_micros(),
            accumulated_wall.as_micros()
        );
    }

    #[test]
    fn reused_zoom_twelve_reference_is_classified_as_a_zoom_fourteen_glitch() {
        const WIDTH: u32 = 960;
        const HEIGHT: u32 = 540;
        const CAP: u32 = 512;
        const GLITCH_INDEX: u32 = 387 * WIDTH + 478;
        let target = [-0.743_643_887_037_151, 0.131_825_904_205_33];
        let zoom_twelve_scale = pixel_scale(12.0, WIDTH).expect("zoom twelve scale");
        let plan = precision_for(12.0, WIDTH, CAP).expect("zoom twelve precision");
        let centre = BigCentre::from_f64(
            [
                0.0,
                0.0,
                target[0],
                30.0_f64.mul_add(zoom_twelve_scale, target[1]),
            ],
            plan.requested_bits,
        )
        .expect("finite seahorse reference");
        let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let map = Homography {
            rows: [1.0, 0.0, 0.0, 0.0, 1.0, -120.0, 0.0, 0.0, 1.0],
            inverse: [1.0, 0.0, 0.0, 0.0, 1.0, 120.0, 0.0, 0.0, 1.0],
            condition_number: 1.0,
            apron_scale: 1.0,
        };
        let plane: Plane = construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane");
        let uniforms = PerturbUniform::pack(
            plane,
            &map,
            scale_split(14.0, WIDTH).expect("zoom fourteen scale"),
            GridExtent {
                width: WIDTH,
                height: HEIGHT,
            },
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("reused-reference uniform");
        let sample = perturb_scaled_pixel(&uniforms, &orbit.records, GLITCH_INDEX)
            .expect("pinned pixel is in bounds");

        assert_eq!(plan.requested_bits, 60);
        assert_eq!(orbit.length, 78);
        assert_eq!(
            SampleStatus::from_f32(sample.record.status),
            Some(SampleStatus::Glitch)
        );
        assert_eq!(sample.escape_index, None);
    }

    #[test]
    fn forty_one_record_reference_exhaustion_is_corrected_by_a_cap_length_reference() {
        const CAP: u32 = 512;
        let target = BigCentre::from_f64(
            [0.0, 0.0, -0.743_643_887_037_151, 0.131_825_904_205_33],
            precision_for(14.0, 960, CAP)
                .expect("zoom fourteen precision")
                .requested_bits,
        )
        .expect("finite seahorse reference");
        let plan = precision_for(14.0, 960, CAP).expect("zoom fourteen precision");
        let mut builder = ReferenceOrbitBuilder::new(&target, plan, EscapeParams::new(CAP))
            .expect("reference builder");
        let orbit = loop {
            match builder
                .step(NonZeroU32::new(CAP).expect("nonzero cap"))
                .expect("reference step")
            {
                OrbitStep::Complete(orbit) => break orbit,
                OrbitStep::Pending { .. } => {}
            }
        };
        let plane = construct_plane(ObjectAngles::IDENTITY).expect("Mandelbrot plane");
        let extent = GridExtent {
            width: 1,
            height: 1,
        };
        let early = PerturbUniform::pack(
            plane,
            &Homography::IDENTITY,
            scale_split(14.0, 960).expect("zoom fourteen scale"),
            extent,
            EscapeParams::new(CAP),
            41,
            RefinementLevel::Final,
        )
        .expect("short-reference uniform");
        let corrected = PerturbUniform::pack(
            plane,
            &Homography::IDENTITY,
            scale_split(14.0, 960).expect("zoom fourteen scale"),
            extent,
            EscapeParams::new(CAP),
            orbit.length,
            RefinementLevel::Final,
        )
        .expect("long-reference uniform");

        let before = perturb_scaled_pixel(&early, &orbit.records[..41], 0)
            .expect("short reference is represented honestly");
        let after = perturb_scaled_pixel(&corrected, &orbit.records, 0)
            .expect("long reference corrects the sample");

        assert_eq!(orbit.length, CAP);
        assert_eq!(
            SampleStatus::from_f32(before.record.status),
            Some(SampleStatus::Glitch)
        );
        assert_eq!(
            SampleStatus::from_f32(after.record.status),
            Some(SampleStatus::Sampled)
        );
        assert_eq!(after.escape_index, None);
    }

    #[test]
    fn shallow_oracle_pass_and_exact_failure_are_distinct() {
        let point = [0.0, 0.0, 2.0, 0.0];
        let params = EscapeParams::new(16);
        let observed = escape_shallow_point(point, params).expect("kernel mirror");
        let expected = escape_f32(point, params).expect("math oracle");
        for mode in PrecisionMode::ALL {
            let result = evaluate_shallow_conformance(mode, observed, expected);
            assert_eq!(result.verdict, ConformanceVerdict::Pass);
            assert_eq!(result.precision_mode, mode.as_str());
        }
        let wrong = KernelSample {
            record: EscapeGridRecord {
                escaped: 0.0,
                smooth_iter: -1.0,
                ..observed.record
            },
            escape_index: None,
        };
        for mode in PrecisionMode::ALL {
            assert_eq!(
                evaluate_shallow_conformance(mode, wrong, expected).verdict,
                ConformanceVerdict::Fail
            );
        }
    }

    #[test]
    fn boundary_never_masquerades_as_an_exact_perturbation_pass() {
        let observed = KernelSample {
            record: EscapeGridRecord {
                smooth_iter: -1.0,
                escaped: 0.0,
                rebase_count: 0.0,
                status: 0.0,
            },
            escape_index: None,
        };
        let expected = ember_julibrot_math::PerturbSample {
            smooth_iter: -1.0,
            escaped: true,
            escape_index: Some(4),
            rebase_count: 0,
            glitch: false,
        };
        let result = evaluate_perturbation_conformance(
            PrecisionMode::PictureFast,
            observed,
            expected,
            PerturbationEnvelope {
                delta_abs_error: 1.0,
                escape_norm2_error: 1.0,
                smooth_error: 0.0,
                minimum_escape_margin: 0.5,
            },
        );
        assert_eq!(result.verdict, ConformanceVerdict::Boundary);
        assert!(!result.classification_exact);
    }

    #[test]
    fn rebase_count_identity_is_only_a_deterministic_requirement() {
        let observed = KernelSample {
            record: EscapeGridRecord {
                smooth_iter: -1.0,
                escaped: 0.0,
                rebase_count: 2.0,
                status: crate::SampleStatus::Sampled.as_f32(),
            },
            escape_index: None,
        };
        let expected = ember_julibrot_math::PerturbSample {
            smooth_iter: -1.0,
            escaped: false,
            escape_index: None,
            rebase_count: 1,
            glitch: false,
        };
        let envelope = PerturbationEnvelope {
            delta_abs_error: 0.0,
            escape_norm2_error: 0.0,
            smooth_error: 0.0,
            minimum_escape_margin: 1.0,
        };
        for (mode, verdict) in [
            (PrecisionMode::Deterministic, ConformanceVerdict::Fail),
            (PrecisionMode::PictureFast, ConformanceVerdict::Pass),
        ] {
            let result = evaluate_perturbation_conformance(mode, observed, expected, envelope);
            assert!(!result.rebase_count_exact);
            assert_eq!(result.verdict, verdict);
        }
    }

    #[test]
    fn uncertain_records_obey_the_same_sample_invariants_as_certified_records() {
        let uncertain = KernelSample {
            record: EscapeGridRecord {
                smooth_iter: 0.0,
                escaped: 1.0,
                rebase_count: 0.0,
                status: crate::SampleStatus::MapUncertain.as_f32(),
            },
            escape_index: Some(0),
        };
        assert!(super::record_is_well_formed(
            uncertain,
            crate::KernelMode::Shallow
        ));
    }

    #[test]
    fn visible_cards_are_unique_and_cannot_claim_native_evidence() {
        let ids = VISIBLE_REPLAY_CARDS
            .iter()
            .map(|card| card.id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), VISIBLE_REPLAY_CARDS.len());
        assert!(
            VISIBLE_REPLAY_CARDS
                .iter()
                .all(|card| card.requirement.starts_with("requires visible replay:"))
        );
    }
}
