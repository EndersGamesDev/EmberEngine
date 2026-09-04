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
        SampleStatus::Sampled | SampleStatus::MapUncertain | SampleStatus::Glitch if !escaped => {
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
    use std::collections::BTreeSet;

    use ember_julibrot_math::{
        BigCentre, EscapeGridRecord, EscapeParams, Homography, ObjectAngles, OrbitStep,
        PerturbationEnvelope, Plane, PrecisionMode, ReferenceOrbitBuilder, construct_plane,
        escape_f32, pixel_scale, precision_for, scale_split,
    };

    use super::{
        ConformanceVerdict, VISIBLE_REPLAY_CARDS, evaluate_perturbation_conformance,
        evaluate_shallow_conformance,
    };
    use crate::{
        GridExtent, KernelSample, PerturbUniform, RefinementLevel, SampleStatus,
        escape_shallow_point, perturb_scaled_pixel,
    };

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
