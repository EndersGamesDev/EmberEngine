//! Browser diagnostic and benchmark client for “what is this?”.

#![deny(missing_docs)]

mod kernels;

#[cfg(target_arch = "wasm32")]
mod gpu;

use ember_game_what_is_this_v1::{DiagnosticReport, KernelMeasurement, KernelStatus};
use serde::Serialize;

pub use kernels::{FloatProbeResult, KernelSpec, KernelSuite, jank_chunk, kernel_specs};

/// One report-derived sentence shown on the final verdict card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerdictObservation {
    /// Sentence text; every number in it comes from the supplied report.
    pub text: String,
}

/// One deterministic badge shown on the final verdict card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerdictBadge {
    /// Stable presentation identifier used by page styling and tests.
    pub id: &'static str,
    /// Short badge name.
    pub name: &'static str,
    /// Text-only, emoji-free mark.
    pub glyph: &'static str,
    /// Exact report measurement that earned the badge.
    pub measurement: String,
}

/// Measurement-derived personality for the final verdict card.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerdictPersonality {
    /// Ordered report-derived observations.
    pub observations: Vec<VerdictObservation>,
    /// Ordered badges earned by deterministic rules.
    pub badges: Vec<VerdictBadge>,
}

fn best_memcpy(report: &DiagnosticReport) -> Option<&KernelMeasurement> {
    report
        .kernels
        .iter()
        .filter(|kernel| {
            kernel.kernel_id.starts_with("cpu.memcpy.")
                && kernel.status == KernelStatus::Complete
                && kernel.unit == "MiB/s"
                && kernel
                    .summary
                    .is_some_and(|summary| summary.median.is_finite() && summary.median > 0.0)
        })
        .max_by(|left, right| {
            left.summary
                .map(|summary| summary.median)
                .unwrap_or_default()
                .total_cmp(
                    &right
                        .summary
                        .map(|summary| summary.median)
                        .unwrap_or_default(),
                )
        })
}

/// Derives verdict sentences and badges from a completed schema-1 report.
///
/// Badge rules are frozen presentation logic: `simd-ready` requires the SIMD128 feature probe;
/// `float-honest` requires a nonempty sine/cosine table with every ULP distance zero;
/// `fma-contractor` and `strict-multiplier` mirror the contraction bit; `timer-truthful` requires
/// a measured timer quantum no larger than 0.1 ms while `timer-coy` covers coarser or unobserved
/// clocks; `thread-rich` requires at least eight exposed logical processors; and `memory-mover`
/// requires a complete memcpy kernel with a finite positive median. Observations use the same
/// report facts and never invent a score or substitute measurement.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn derive_verdict(report: &DiagnosticReport) -> VerdictPersonality {
    let mut observations = Vec::new();
    let timer = &report.environment.timer;
    match timer.resolution_ms {
        Some(quantum) if timer.zero_delta_count > 0 => observations.push(VerdictObservation {
            text: format!(
                "Your clock held perfectly still for {} reads and then moved in {:.4} ms steps — privacy armor has a measurable tick.",
                timer.zero_delta_count, quantum
            ),
        }),
        Some(quantum) => observations.push(VerdictObservation {
            text: format!(
                "Your clock exposed a {quantum:.4} ms smallest step with no equal consecutive reads in the bounded probe."
            ),
        }),
        None => observations.push(VerdictObservation {
            text: format!(
                "Your clock refused to admit time passed {} times in the bounded probe — the timer quantum stayed hidden.",
                timer.zero_delta_count
            ),
        }),
    }

    observations.push(VerdictObservation {
        text: report.environment.hardware_concurrency.map_or_else(
            || "Your browser kept its logical thread count to itself; mystery accepted.".to_string(),
            |threads| {
                format!(
                    "Your browser reports {threads} logical threads — exactly {threads} answers to the first hardware question."
                )
            },
        ),
    });

    let ulp_count = report.float_behavior.transcendentals.len() * 2;
    let max_ulp = report
        .float_behavior
        .transcendentals
        .iter()
        .flat_map(|value| [value.sin_ulp, value.cos_ulp])
        .max();
    if let Some(max_ulp) = max_ulp {
        observations.push(VerdictObservation {
            text: if max_ulp == 0 {
                format!(
                    "All {ulp_count} sine and cosine checks landed at 0 ULP — suspiciously tidy, numerically real."
                )
            } else {
                format!(
                    "Across {ulp_count} sine and cosine checks, the widest fingerprint was {max_ulp} ULP — a real edge left visible."
                )
            },
        });
    }

    if let Some(fma) = report.float_behavior.fma {
        observations.push(VerdictObservation {
            text: if fma.contracts_to_fma {
                "The multiply-add probe contracted to FMA; this machine takes the fused shortcut and signs its work."
                    .to_string()
            } else {
                "The multiply-add probe stayed separate; this machine rounds the multiply before the add."
                    .to_string()
            },
        });
    }

    observations.push(VerdictObservation {
        text: if report.capabilities.simd128.available {
            "The SIMD128 feature test passed — four f32 lanes may proceed abreast.".to_string()
        } else {
            format!(
                "SIMD128 stayed unavailable: {}.",
                report
                    .capabilities
                    .simd128
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("the feature probe returned no reason")
            )
        },
    });

    if let Some(kernel) = best_memcpy(report) {
        let median = kernel
            .summary
            .map(|summary| summary.median)
            .unwrap_or_default();
        observations.push(VerdictObservation {
            text: format!(
                "Your best memcpy median was {:.2} MiB/s in {} — the bytes did not merely pose for the camera.",
                median, kernel.kernel_id
            ),
        });
    }

    if report.environment.hidden_during_run {
        observations.push(VerdictObservation {
            text: "The page was hidden during this run; the report kept the visibility change instead of pretending the scheduler behaved normally."
                .to_string(),
        });
    }

    if let Some(gpu) = report
        .kernels
        .iter()
        .find(|kernel| kernel.kernel_id == "gpu.ember-fixed-scene.v1")
    {
        observations.push(VerdictObservation {
            text: match gpu.status {
                KernelStatus::Complete => gpu.summary.map_or_else(
                    || "The GPU stage completed but supplied no frame-time summary; noted without embroidery."
                        .to_string(),
                    |summary| {
                        format!(
                            "The GPU scene reported a {:.3} ms median frame time; the adapter showed its work.",
                            summary.median
                        )
                    },
                ),
                KernelStatus::Unavailable => format!(
                    "The GPU stage declined with a shrug: {}",
                    gpu.unavailable_reason
                        .as_deref()
                        .unwrap_or("no unavailable reason was recorded")
                ),
            },
        });
    }

    let complete = report
        .kernels
        .iter()
        .filter(|kernel| kernel.status == KernelStatus::Complete)
        .count();
    let unavailable = report.kernels.len().saturating_sub(complete);
    observations.push(VerdictObservation {
        text: format!(
            "The suite completed {complete} kernels and marked {unavailable} unavailable — absence counted, never disguised."
        ),
    });

    let mut badges = Vec::new();
    if report.capabilities.simd128.available {
        badges.push(VerdictBadge {
            id: "simd-ready",
            name: "SIMD Ready",
            glyph: "S4",
            measurement: "wasm SIMD128 feature test: available".to_string(),
        });
    }
    if max_ulp == Some(0) {
        badges.push(VerdictBadge {
            id: "float-honest",
            name: "Float Honest",
            glyph: "0u",
            measurement: format!("{ulp_count} sine/cosine comparisons: maximum 0 ULP"),
        });
    }
    if let Some(fma) = report.float_behavior.fma {
        badges.push(if fma.contracts_to_fma {
            VerdictBadge {
                id: "fma-contractor",
                name: "FMA Contractor",
                glyph: "F+",
                measurement: "plain a*b+c matched the explicit fused result".to_string(),
            }
        } else {
            VerdictBadge {
                id: "strict-multiplier",
                name: "Strict Multiplier",
                glyph: "M+",
                measurement: "plain a*b+c matched the separated result".to_string(),
            }
        });
    }
    badges.push(match timer.resolution_ms {
        Some(quantum) if quantum <= 0.1 => VerdictBadge {
            id: "timer-truthful",
            name: "Timer Truthful",
            glyph: "dt",
            measurement: format!("smallest observed performance.now() step: {quantum:.4} ms"),
        },
        Some(quantum) => VerdictBadge {
            id: "timer-coy",
            name: "Timer Coy",
            glyph: "dt?",
            measurement: format!("smallest observed performance.now() step: {quantum:.4} ms"),
        },
        None => VerdictBadge {
            id: "timer-coy",
            name: "Timer Coy",
            glyph: "dt?",
            measurement: format!(
                "no positive timer step observed; {} equal reads",
                timer.zero_delta_count
            ),
        },
    });
    if let Some(threads) = report
        .environment
        .hardware_concurrency
        .filter(|threads| *threads >= 8)
    {
        badges.push(VerdictBadge {
            id: "thread-rich",
            name: "Thread Rich",
            glyph: "||",
            measurement: format!("navigator.hardwareConcurrency: {threads}"),
        });
    }
    if let Some(kernel) = best_memcpy(report) {
        let median = kernel
            .summary
            .map(|summary| summary.median)
            .unwrap_or_default();
        badges.push(VerdictBadge {
            id: "memory-mover",
            name: "Memory Mover",
            glyph: ">>",
            measurement: format!("{} median: {median:.2} MiB/s", kernel.kernel_id),
        });
    }

    VerdictPersonality {
        observations,
        badges,
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_api;

#[cfg(test)]
mod tests {
    use ember_game_what_is_this_v1::{
        CapabilityFlags, CapabilityProbe, EnvironmentFacts, FaerWasmVerdict,
        FloatBehaviorFingerprint, FmaProbe, ScreenFacts, SummaryStats, TimerFacts,
        TranscendentalObservation,
    };

    use super::*;

    fn sample_report() -> DiagnosticReport {
        DiagnosticReport {
            report_schema_version: 1,
            generated_at: "2026-09-01T00:00:00.000Z".to_string(),
            total_run_wall_ms: 5_500.0,
            environment: EnvironmentFacts {
                kernel_id: "environment.browser-facts.v1".to_string(),
                user_agent: "test".to_string(),
                hardware_concurrency: Some(14),
                device_memory_gib: None,
                screen: ScreenFacts {
                    width: 1_920,
                    height: 1_080,
                    device_pixel_ratio: 1.0,
                },
                timer: TimerFacts {
                    resolution_ms: Some(1.0),
                    zero_delta_count: 4_096,
                    monotonicity_violations: 0,
                    positive_delta_samples_ms: vec![1.0],
                    positive_delta_summary: Some(SummaryStats {
                        sample_count: 1,
                        median: 1.0,
                        p95: 1.0,
                        min: 1.0,
                        max: 1.0,
                    }),
                    caveat: "coarse timer".to_string(),
                },
                initial_visibility: "visible".to_string(),
                final_visibility: "visible".to_string(),
                hidden_during_run: false,
                visibility_observations: Vec::new(),
            },
            capabilities: CapabilityFlags {
                simd128: CapabilityProbe {
                    kernel_id: "capability.wasm-simd128.v1".to_string(),
                    available: true,
                    unavailable_reason: None,
                },
                threads: CapabilityProbe {
                    kernel_id: "capability.wasm-threads.v1".to_string(),
                    available: false,
                    unavailable_reason: Some("not isolated".to_string()),
                },
                bulk_memory: CapabilityProbe {
                    kernel_id: "capability.wasm-bulk-memory.v1".to_string(),
                    available: true,
                    unavailable_reason: None,
                },
            },
            float_behavior: FloatBehaviorFingerprint {
                available: true,
                unavailable_reason: None,
                fma_kernel_id: "float.fma-contraction.v1".to_string(),
                fma: Some(FmaProbe {
                    plain_result_bits: 0,
                    separated_result_bits: 0,
                    fused_result_bits: 1,
                    contracts_to_fma: false,
                }),
                transcendental_kernel_id: "float.f32-sin-cos-ulp.v1".to_string(),
                transcendentals: vec![TranscendentalObservation {
                    input_bits: 0,
                    sin_reference_f64: 0.0,
                    sin_observed_bits: 0,
                    sin_ulp: 0,
                    cos_reference_f64: 1.0,
                    cos_observed_bits: 1.0_f32.to_bits(),
                    cos_ulp: 0,
                }],
            },
            faer_wasm: FaerWasmVerdict {
                version: "0.24.4".to_string(),
                compiled: true,
                configuration: "sequential".to_string(),
                consequence: "faer kernels included".to_string(),
            },
            stages: Vec::new(),
            kernels: vec![KernelMeasurement {
                kernel_id: "cpu.memcpy.4m.v2".to_string(),
                workload: "16 MiB copy".to_string(),
                unit: "MiB/s".to_string(),
                warmup_runs: 3,
                status: KernelStatus::Complete,
                unavailable_reason: None,
                raw_samples: vec![4_096.0],
                summary: Some(SummaryStats {
                    sample_count: 1,
                    median: 4_096.0,
                    p95: 4_096.0,
                    min: 4_096.0,
                    max: 4_096.0,
                }),
                notes: Vec::new(),
            }],
        }
    }

    #[test]
    fn badge_mapping_is_deterministic_and_measurement_backed() {
        let verdict = derive_verdict(&sample_report());
        let ids: Vec<_> = verdict.badges.iter().map(|badge| badge.id).collect();
        assert_eq!(
            ids,
            [
                "simd-ready",
                "float-honest",
                "strict-multiplier",
                "timer-coy",
                "thread-rich",
                "memory-mover",
            ]
        );
        assert!(
            verdict
                .badges
                .iter()
                .all(|badge| !badge.measurement.is_empty())
        );
    }

    #[test]
    fn observations_cite_report_values_and_unavailable_gpu_reason() {
        let mut report = sample_report();
        report.kernels.push(KernelMeasurement {
            kernel_id: "gpu.ember-fixed-scene.v1".to_string(),
            workload: "fixed scene".to_string(),
            unit: "ms".to_string(),
            warmup_runs: 0,
            status: KernelStatus::Unavailable,
            unavailable_reason: Some("no safe offscreen seam".to_string()),
            raw_samples: Vec::new(),
            summary: None,
            notes: Vec::new(),
        });
        let text = derive_verdict(&report)
            .observations
            .into_iter()
            .map(|observation| observation.text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("4096 reads"));
        assert!(text.contains("14 logical threads"));
        assert!(text.contains("2 sine and cosine checks"));
        assert!(text.contains("4096.00 MiB/s"));
        assert!(text.contains("no safe offscreen seam"));
    }
}
