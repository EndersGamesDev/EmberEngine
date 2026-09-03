use ember_julibrot_math::{EscapeSample, PerturbSample, PerturbationEnvelope, PrecisionMode};

use crate::{KernelMode, KernelSample};

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
    let Some(glitch) = binary_flag(record.glitch) else {
        return false;
    };
    let rebase_count = exact_rebase_count(record.rebase_count);
    let index_matches = escaped == sample.escape_index.is_some();
    let terminal_matches = if escaped {
        !glitch && record.smooth_iter.is_finite()
    } else {
        record.smooth_iter.to_bits() == (-1.0_f32).to_bits()
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
        glitch_exact: observed.record.glitch == 0.0,
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
    let glitch_exact = binary_flag(observed.record.glitch) == Some(expected.glitch);
    let smooth_abs_error = smooth_error(observed.record.smooth_iter, expected.smooth_iter);
    let record_well_formed = record_is_well_formed(observed, KernelMode::Perturbation);
    let common = record_well_formed
        && (!precision_mode.requires_bit_identity() || rebase_count_exact)
        && glitch_exact
        && smooth_abs_error <= PERTURB_SMOOTH_TOLERANCE;
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
        smooth_tolerance: PERTURB_SMOOTH_TOLERANCE,
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
    use std::collections::BTreeSet;

    use ember_julibrot_math::{
        EscapeGridRecord, EscapeParams, PerturbationEnvelope, PrecisionMode, escape_f32,
    };

    use super::{
        ConformanceVerdict, VISIBLE_REPLAY_CARDS, evaluate_perturbation_conformance,
        evaluate_shallow_conformance,
    };
    use crate::{KernelSample, escape_shallow_point};

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
                glitch: 0.0,
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
                glitch: 0.0,
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
