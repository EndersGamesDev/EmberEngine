//! Browser diagnostic and benchmark client for “what is this?”.

#![deny(missing_docs)]

mod julibrot;
mod kernels;

#[cfg(target_arch = "wasm32")]
mod gpu;

#[cfg(target_arch = "wasm32")]
mod render_bar;

use ember_game_what_is_this_v1::{DiagnosticReport, KernelMeasurement, KernelStatus};
use serde::Serialize;

pub use julibrot::{JULIBROT_REPORT_BYTE_BUDGET, JulibrotScenarioSpec, julibrot_scenarios};
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

fn complete_gpu_compute(report: &DiagnosticReport) -> Vec<&KernelMeasurement> {
    report
        .kernels
        .iter()
        .filter(|kernel| {
            matches!(
                kernel.kernel_id.as_str(),
                "gpu.compute-rank4-soa.n256.v1"
                    | "gpu.compute-rank4-soa.n1024.v1"
                    | "gpu.storage-copy.4m.v1"
                    | "gpu.dispatch-roundtrip.tiny.v1"
            ) && kernel.status == KernelStatus::Complete
        })
        .collect()
}

fn gpu_adapter_identity(report: &DiagnosticReport) -> Option<&str> {
    report
        .kernels
        .iter()
        .find(|kernel| kernel.kernel_id == "gpu.adapter-facts.v1")
        .and_then(|kernel| {
            kernel
                .notes
                .iter()
                .find_map(|note| note.strip_prefix("adapter identity: "))
        })
}

fn render_present_frames(kernel: &KernelMeasurement) -> Option<u32> {
    kernel.notes.iter().find_map(|note| {
        note.strip_prefix("frames presented during kernel: ")
            .and_then(|value| value.parse().ok())
    })
}

fn complete_render_present(report: &DiagnosticReport) -> Option<(&KernelMeasurement, u32)> {
    report
        .kernels
        .iter()
        .find(|kernel| {
            kernel.kernel_id == "gpu.render-present.v1"
                && kernel.status == KernelStatus::Complete
                && kernel
                    .summary
                    .is_some_and(|summary| summary.median.is_finite() && summary.median > 0.0)
        })
        .and_then(|kernel| render_present_frames(kernel).map(|frames| (kernel, frames)))
        .filter(|(_, frames)| *frames > 0)
}

/// Maps real suite work to the render bar's normalized target position.
///
/// `stage_index` and `kernel_index` are zero-based completed-work offsets; the current sample
/// fraction advances only the current kernel. Empty stage or kernel totals produce conservative
/// progress instead of invented work. Inputs beyond their totals are clamped.
#[must_use]
pub fn bar_progress(
    stage_index: u32,
    stage_count: u32,
    kernel_index: u32,
    kernel_count: u32,
    sample_count: u32,
    sample_total: u32,
) -> f64 {
    if stage_count == 0 {
        return 0.0;
    }
    if stage_index >= stage_count {
        return 1.0;
    }
    let kernel_fraction = if kernel_count == 0 {
        0.0
    } else {
        let completed = kernel_index.min(kernel_count);
        let current = if completed < kernel_count && sample_total > 0 {
            f64::from(sample_count.min(sample_total)) / f64::from(sample_total)
        } else {
            0.0
        };
        (f64::from(completed) + current) / f64::from(kernel_count)
    };
    (f64::from(stage_index) + kernel_fraction) / f64::from(stage_count)
}

/// Derives verdict sentences and badges from a completed schema-1 report.
///
/// Badge rules are frozen presentation logic: `simd-ready` requires the SIMD128 feature probe;
/// `float-honest` requires a nonempty sine/cosine table with every ULP distance zero;
/// `fma-contractor` and `strict-multiplier` mirror the contraction bit; `timer-truthful` requires
/// a measured timer quantum no larger than 0.1 ms while `timer-coy` covers coarser or unobserved
/// clocks; `thread-rich` requires at least eight exposed logical processors; and `memory-mover`
/// requires a complete memcpy kernel with a finite positive median; `gpu-compute` requires at least
/// one complete timed WebGPU compute kernel and cites the adapter identity recorded by
/// `gpu.adapter-facts.v1`; and `gpu-render` requires a complete `gpu.render-present.v1` record with
/// a positive measured cadence and a positive frame count in its notes. Observations use the same
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

    let complete_gpu = complete_gpu_compute(report);
    if !complete_gpu.is_empty() {
        let identity = gpu_adapter_identity(report).unwrap_or("adapter identity not exposed");
        observations.push(VerdictObservation {
            text: format!(
                "WebGPU compute woke up on {identity} and completed {} timed kernel{} — no surface or window required.",
                complete_gpu.len(),
                if complete_gpu.len() == 1 { "" } else { "s" }
            ),
        });
        if let Some(storage) = complete_gpu.iter().find(|kernel| {
            kernel.kernel_id == "gpu.storage-copy.4m.v1"
                && kernel
                    .summary
                    .is_some_and(|summary| summary.median.is_finite() && summary.median > 0.0)
        }) {
            let median_mib = storage
                .summary
                .map(|summary| summary.median)
                .unwrap_or_default();
            observations.push(VerdictObservation {
                text: format!(
                    "Your GPU moved {:.3} GiB/s through its storage buffers in {} — queue overhead is counted when timestamp queries are absent.",
                    median_mib / 1_024.0,
                    storage.kernel_id
                ),
            });
        }
    } else if let Some(stage) = report
        .stages
        .iter()
        .find(|stage| stage.stage_id == "stage.gpu-compute.v1")
    {
        observations.push(VerdictObservation {
            text: format!(
                "WebGPU compute declined with a shrug: {}",
                stage
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("no timed compute kernel completed")
            ),
        });
    }

    if let Some((kernel, frames)) = complete_render_present(report) {
        let median = kernel
            .summary
            .map(|summary| summary.median)
            .unwrap_or_default();
        observations.push(VerdictObservation {
            text: format!(
                "The 3D progress bar presented {frames} measured frames at a {median:.3} ms median cadence — compositor pacing included, shader speed not claimed."
            ),
        });
    } else if let Some(kernel) = report
        .kernels
        .iter()
        .find(|kernel| kernel.kernel_id == "gpu.render-present.v1")
    {
        observations.push(VerdictObservation {
            text: format!(
                "The 3D progress bar never entered the measurement book: {}.",
                kernel
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or("no presented frame produced a valid cadence sample")
            ),
        });
    }

    if report.environment.hidden_during_run {
        observations.push(VerdictObservation {
            text: "The page was hidden during this run; the report kept the visibility change instead of pretending the scheduler behaved normally."
                .to_string(),
        });
    }

    if let Some(text) = julibrot::observation(report) {
        observations.push(VerdictObservation { text });
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
    if !complete_gpu.is_empty() {
        let identity = gpu_adapter_identity(report).unwrap_or("adapter identity not exposed");
        badges.push(VerdictBadge {
            id: "gpu-compute",
            name: "Compute Awake",
            glyph: "GC",
            measurement: format!(
                "{} timed WebGPU compute kernel{} completed on {identity}",
                complete_gpu.len(),
                if complete_gpu.len() == 1 { "" } else { "s" }
            ),
        });
    }
    if let Some((kernel, frames)) = complete_render_present(report) {
        let median = kernel
            .summary
            .map(|summary| summary.median)
            .unwrap_or_default();
        badges.push(VerdictBadge {
            id: "gpu-render",
            name: "Window Dressed",
            glyph: "3D",
            measurement: format!("{frames} frames presented; median canvas cadence {median:.3} ms"),
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
        FloatBehaviorFingerprint, FmaProbe, ScreenFacts, StageReport, StageStatus, SummaryStats,
        TimerFacts, TranscendentalObservation,
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

    fn add_julibrot_stage(
        report: &mut DiagnosticReport,
        status: StageStatus,
        reason: Option<&str>,
    ) {
        report.stages.push(StageReport {
            stage_id: "stage.julibrot-slide.v1".to_string(),
            name: "Julibrot fast-slide".to_string(),
            status,
            unavailable_reason: reason.map(str::to_string),
            duration_ms: 12_345.67,
        });
    }

    fn add_julibrot_measurement(report: &mut DiagnosticReport, suffix: &str, note: &str) {
        report.kernels.push(KernelMeasurement {
            kernel_id: format!("julibrot-slide.{suffix}.v1"),
            workload: "fixed scripted slide".to_string(),
            unit: "ms".to_string(),
            warmup_runs: 0,
            status: KernelStatus::Complete,
            unavailable_reason: None,
            raw_samples: vec![12.34],
            summary: Some(SummaryStats {
                sample_count: 1,
                median: 12.34,
                p95: 12.34,
                min: 12.34,
                max: 12.34,
            }),
            notes: vec![note.to_string()],
        });
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

    #[test]
    fn gpu_compute_personality_cites_adapter_and_measured_bandwidth() {
        let mut report = sample_report();
        report.kernels.push(KernelMeasurement {
            kernel_id: "gpu.adapter-facts.v1".to_string(),
            workload: "compute-only adapter facts".to_string(),
            unit: "facts".to_string(),
            warmup_runs: 0,
            status: KernelStatus::Complete,
            unavailable_reason: None,
            raw_samples: Vec::new(),
            summary: None,
            notes: vec!["adapter identity: Test Adapter (DiscreteGpu)".to_string()],
        });
        report.kernels.push(KernelMeasurement {
            kernel_id: "gpu.storage-copy.4m.v1".to_string(),
            workload: "16 MiB storage-buffer copy".to_string(),
            unit: "MiB/s".to_string(),
            warmup_runs: 3,
            status: KernelStatus::Complete,
            unavailable_reason: None,
            raw_samples: vec![2_048.0],
            summary: Some(SummaryStats {
                sample_count: 1,
                median: 2_048.0,
                p95: 2_048.0,
                min: 2_048.0,
                max: 2_048.0,
            }),
            notes: Vec::new(),
        });
        let verdict = derive_verdict(&report);
        let badge = verdict
            .badges
            .iter()
            .find(|badge| badge.id == "gpu-compute")
            .expect("the complete GPU kernel earns the compute badge");
        assert!(badge.measurement.contains("Test Adapter"));
        let text = verdict
            .observations
            .iter()
            .map(|observation| observation.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(text.contains("Test Adapter"));
        assert!(text.contains("2.000 GiB/s"));
    }

    #[test]
    fn bar_progress_uses_only_completed_real_work() {
        assert_eq!(bar_progress(0, 0, 0, 0, 0, 0), 0.0);
        assert_eq!(bar_progress(8, 8, 0, 0, 0, 0), 1.0);
        assert_eq!(bar_progress(2, 8, 1, 4, 7, 14), 0.296_875);
        assert_eq!(bar_progress(2, 8, 9, 4, 20, 14), 0.375);
    }

    #[test]
    fn render_badge_and_observation_cite_presented_frames() {
        let mut report = sample_report();
        report.kernels.push(KernelMeasurement {
            kernel_id: "gpu.render-present.v1".to_string(),
            workload: "15 visible canvas presents".to_string(),
            unit: "ms".to_string(),
            warmup_runs: 3,
            status: KernelStatus::Complete,
            unavailable_reason: None,
            raw_samples: vec![16.667],
            summary: Some(SummaryStats {
                sample_count: 1,
                median: 16.667,
                p95: 16.667,
                min: 16.667,
                max: 16.667,
            }),
            notes: vec!["frames presented during kernel: 19".to_string()],
        });
        let verdict = derive_verdict(&report);
        let badge = verdict
            .badges
            .iter()
            .find(|badge| badge.id == "gpu-render")
            .expect("a complete measured surface kernel earns the render badge");
        assert_eq!(badge.glyph, "3D");
        assert!(badge.measurement.contains("19 frames"));
        assert!(verdict.observations.iter().any(|observation| {
            observation.text.contains("19 measured frames")
                && observation.text.contains("16.667 ms")
        }));
    }

    #[test]
    fn julibrot_observation_reports_complete_measurements() {
        let mut report = sample_report();
        add_julibrot_stage(&mut report, StageStatus::Complete, None);
        add_julibrot_measurement(
            &mut report,
            "object-o13-range",
            r#"{"frames":{"sample_count":19,"observed":17,"clear_only":3,"held":2}}"#,
        );
        add_julibrot_measurement(
            &mut report,
            "height-gentle-d5-8",
            r#"{"frames":{"sample_count":23,"observed":21,"clear_only":4,"held":1}}"#,
        );
        assert_eq!(
            julibrot::observation(&report).as_deref(),
            Some(
                "The Julibrot slide completed 2 fixed scenarios across 38 distinct lab turns from 42 parent Facts samples: 7 clear-only and 3 held."
            )
        );
    }

    #[test]
    fn julibrot_observation_reports_partial_progress() {
        let mut report = sample_report();
        add_julibrot_stage(&mut report, StageStatus::Complete, None);
        add_julibrot_measurement(
            &mut report,
            "object-o13-range",
            r#"{"frames":{"sample_count":19,"observed":17,"clear_only":3,"held":2}}"#,
        );
        add_julibrot_measurement(
            &mut report,
            "height-gentle-d5-8",
            r#"{"frames":{"sample_count":23,"observed":21,"clear_only":4,"held":1},"stage_progress":{"completed_scenarios":2,"total_scenarios":8,"partial_reason":"the page was hidden"}}"#,
        );
        assert_eq!(
            julibrot::observation(&report).as_deref(),
            Some(
                "The Julibrot slide completed 2 of 8 fixed scenarios across 38 distinct lab turns from 42 parent Facts samples before stopping early: the page was hidden."
            )
        );
    }

    #[test]
    fn julibrot_observation_reports_stage_unavailability() {
        let mut report = sample_report();
        add_julibrot_stage(
            &mut report,
            StageStatus::Unavailable,
            Some("WebGL2 unavailable"),
        );
        assert_eq!(
            julibrot::observation(&report).as_deref(),
            Some("The Julibrot slide stage was unavailable: WebGL2 unavailable.")
        );
    }

    #[test]
    fn julibrot_observation_reports_malformed_notes() {
        let mut report = sample_report();
        add_julibrot_stage(&mut report, StageStatus::Complete, None);
        add_julibrot_measurement(&mut report, "object-o13-range", "not JSON");
        assert_eq!(
            julibrot::observation(&report).as_deref(),
            Some(
                "The Julibrot slide stage completed, but all 1 scenario measurement notes were malformed."
            )
        );
    }

    #[test]
    fn page_contract_carries_the_julibrot_card_disclosure_and_stage_only_query() {
        let page = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/games/what-is-this/v1/index.html"
        ));
        assert!(page.contains("Nine stages reveal"));
        assert!(page.contains("data-stage=\"stage.julibrot-slide.v1\""));
        assert!(page.contains("id=\"julibrot-result\""));
        assert!(page.contains("The game drove the same-origin Julibrot lab at labs/julibrot/, read the lab's own Facts display, and recorded none of your input."));
        assert!(page.contains("get('stage') === 'julibrot-slide'"));
        assert!(page.contains("if (JULIBROT_STAGE_ONLY) beginRun(null)"));
        assert!(page.contains("new URL('labs/julibrot/', ROOT)"));
        assert!(page.contains("const JULIBROT_FACT_SAMPLE_CAP = 12"));
        assert!(page.contains("function julibrotReportByteBudget()"));
        assert!(page.contains("wasm?.julibrot_report_byte_budget?.()"));
        assert!(page.contains("raw=game-side step wall ms"));
        assert!(page.contains("presented_scene_ids: null"));
        assert!(page.contains("warp_refused: null"));
        assert!(page.contains("live_iteration_cap"));
        assert!(!page.contains("facts.presented_scene_id"));
    }
}
