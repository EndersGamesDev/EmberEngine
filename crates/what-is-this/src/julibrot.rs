//! Fixed Julibrot slide scenarios and bounded report records.

use ember_game_what_is_this_v1::KernelStatus;
use serde::Serialize;

const STAGE_ID: &str = "stage.julibrot-slide.v1";
/// Maximum compact JSON bytes reserved for all Julibrot scenario measurements. The adversarial page-contract fixture starts with 17-significant-digit browser floats, rounds them at the shipped precisions, and uses 17-digit scene, worker, credit, and fence values. Its eight compact records measure 18,879 bytes, leaving 1,601 bytes here and 32 KiB of the protocol cap for the existing report.
pub const JULIBROT_REPORT_BYTE_BUDGET: usize = 20 * 1_024;

/// Stable metadata for one scripted Julibrot control sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct JulibrotScenarioSpec {
    /// Stable identifier used as the scenario measurement suffix.
    pub scenario_id: &'static str,
    /// User-facing scenario name.
    pub name: &'static str,
    /// Named row established before inputs begin.
    pub baseline_row: &'static str,
    /// Number of scripted control updates.
    pub step_count: u16,
    /// Fixed delay between updates in milliseconds.
    pub interval_ms: u16,
}

const SCENARIOS: [JulibrotScenarioSpec; 8] = [
    JulibrotScenarioSpec {
        scenario_id: "object-o13-range",
        name: "o13 full-range drag",
        baseline_row: "Mandelbrot",
        step_count: 12,
        interval_ms: 50,
    },
    JulibrotScenarioSpec {
        scenario_id: "height-gentle-d5-8",
        name: "height drag at d5 8",
        baseline_row: "gentle-d5-8",
        step_count: 24,
        interval_ms: 125,
    },
    JulibrotScenarioSpec {
        scenario_id: "height-close-d5-2",
        name: "height drag at d5 2",
        baseline_row: "close-d5-2",
        step_count: 24,
        interval_ms: 125,
    },
    JulibrotScenarioSpec {
        scenario_id: "scale-deep-row",
        name: "scale drag on a deep row",
        baseline_row: "deep-scale-14",
        step_count: 10,
        interval_ms: 80,
    },
    JulibrotScenarioSpec {
        scenario_id: "rapid-a-to-b-morph",
        name: "rapid A-to-B morph",
        baseline_row: "Mandelbrot relief to Julia relief",
        step_count: 16,
        interval_ms: 25,
    },
    JulibrotScenarioSpec {
        scenario_id: "iteration-cap-mid-view",
        name: "iteration-cap change mid-view",
        baseline_row: "deep-scale-14",
        step_count: 2,
        interval_ms: 100,
    },
    JulibrotScenarioSpec {
        scenario_id: "hold-exact-2000ms",
        name: "exact 2,000 ms hold after one change",
        baseline_row: "Mandelbrot relief",
        step_count: 1,
        interval_ms: 2_000,
    },
    JulibrotScenarioSpec {
        scenario_id: "alternating-two-slider-burst",
        name: "30-step alternating-slider burst",
        baseline_row: "Mandelbrot relief",
        step_count: 30,
        interval_ms: 16,
    },
];

/// Returns the complete stable Julibrot scenario inventory in report order.
#[must_use]
pub const fn julibrot_scenarios() -> &'static [JulibrotScenarioSpec] {
    &SCENARIOS
}

/// Derives the report-backed Julibrot verdict sentence when the stage was present.
pub fn observation(report: &ember_game_what_is_this_v1::DiagnosticReport) -> Option<String> {
    let stage = report
        .stages
        .iter()
        .find(|stage| stage.stage_id == STAGE_ID)?;
    if stage.status == ember_game_what_is_this_v1::StageStatus::Unavailable {
        return Some(format!(
            "The Julibrot slide stage was unavailable: {}.",
            stage
                .unavailable_reason
                .as_deref()
                .unwrap_or("the lab supplied no refusal reason")
        ));
    }
    let mut scenarios = 0_u32;
    let mut observed = 0_u32;
    let mut samples = 0_u32;
    let mut clear_only = 0_u32;
    let mut held = 0_u32;
    let mut malformed = 0_u32;
    let mut partial = None;
    for kernel in report.kernels.iter().filter(|kernel| {
        kernel.kernel_id.starts_with("julibrot-slide.") && kernel.status == KernelStatus::Complete
    }) {
        let Some(note) = kernel.notes.first() else {
            malformed = malformed.saturating_add(1);
            continue;
        };
        let Ok(measured) = serde_json::from_str::<serde_json::Value>(note) else {
            malformed = malformed.saturating_add(1);
            continue;
        };
        let Some(frames) = measured.get("frames") else {
            malformed = malformed.saturating_add(1);
            continue;
        };
        scenarios += 1;
        observed = observed.saturating_add(
            frames
                .get("observed")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_default(),
        );
        samples = samples.saturating_add(
            frames
                .get("sample_count")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_default(),
        );
        clear_only = clear_only.saturating_add(
            frames
                .get("clear_only")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_default(),
        );
        held = held.saturating_add(
            frames
                .get("held")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_default(),
        );
        if let Some(progress) = measured.get("stage_progress")
            && let Some(reason) = progress
                .get("partial_reason")
                .and_then(serde_json::Value::as_str)
        {
            let total = progress
                .get("total_scenarios")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or_else(|| u32::try_from(SCENARIOS.len()).unwrap_or(u32::MAX));
            partial = Some((total, reason));
        }
    }
    if scenarios == 0 {
        return Some(if malformed == 0 {
            "The Julibrot slide stage completed but supplied no readable scenario measurements."
                .to_string()
        } else {
            format!(
                "The Julibrot slide stage completed, but all {malformed} scenario measurement notes were malformed."
            )
        });
    }
    let observation = if let Some((total, reason)) = partial {
        format!(
            "The Julibrot slide completed {scenarios} of {total} fixed scenarios across {observed} distinct lab turns from {samples} parent Facts samples before stopping early: {reason}."
        )
    } else {
        format!(
            "The Julibrot slide completed {scenarios} fixed scenarios across {observed} distinct lab turns from {samples} parent Facts samples: {clear_only} clear-only and {held} held."
        )
    };
    Some(if malformed == 0 {
        observation
    } else {
        format!("{observation} {malformed} additional scenario notes were malformed.")
    })
}

#[cfg(test)]
mod tests {
    use ember_game_what_is_this_v1::{KernelMeasurement, SummaryStats};
    use serde_json::json;

    use super::*;

    const FACT_SAMPLE_CAP: usize = 12;
    const ADVERSARIAL_GAME_WALL_INPUT: f64 = 239_999.989_999_999_99;
    const ADVERSARIAL_FRACTION_INPUT: f64 = 0.123_456_789_012_345_67;
    const ADVERSARIAL_MEDIAN_FRACTION_INPUT: f64 = 0.567_890_123_456_789_01;
    const ADVERSARIAL_HIGH_FRACTION_INPUT: f64 = 0.987_654_321_098_765_43;
    const ADVERSARIAL_HOLD_WALL_INPUT: f64 = 2_000.009_999_999_999_8;
    const ADVERSARIAL_INTEGER: u64 = 12_345_678_901_234_567;
    const ADVERSARIAL_RECORD_BYTES: usize = 18_879;

    fn round_fixture(value: f64, decimal_places: i32) -> f64 {
        let scale = 10_f64.powi(decimal_places);
        (value * scale).round() / scale
    }

    fn page() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../web/games/what-is-this/v1/index.html"
        ))
    }

    fn section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let (_, tail) = source.split_once(start).expect("section start");
        let (body, _) = tail.split_once(end).expect("section end");
        body
    }

    fn adversarial_record(
        spec: JulibrotScenarioSpec,
        final_record: bool,
    ) -> KernelMeasurement {
        let game_wall_ms = round_fixture(ADVERSARIAL_GAME_WALL_INPUT, 2);
        let low_fraction = round_fixture(ADVERSARIAL_FRACTION_INPUT, 4);
        let median_fraction = round_fixture(ADVERSARIAL_MEDIAN_FRACTION_INPUT, 4);
        let high_fraction = round_fixture(ADVERSARIAL_HIGH_FRACTION_INPUT, 4);
        let hold_wall_ms = round_fixture(ADVERSARIAL_HOLD_WALL_INPUT, 2);
        let wall_samples = vec![ADVERSARIAL_INTEGER; FACT_SAMPLE_CAP];
        let scene_ids = (0..FACT_SAMPLE_CAP)
            .map(|index| ADVERSARIAL_INTEGER + u64::try_from(index).expect("fixture index"))
            .collect::<Vec<_>>();
        let mut note = json!({
            "scenario_id": spec.scenario_id,
            "step_count": spec.step_count,
            "interval_ms": spec.interval_ms,
            "sample_window": "last 12 distinct lab turns",
            "live_iteration_cap": 4096,
            "frames": {
                "sample_count": 4_294_967_295_u64,
                "observed": 4_294_967_295_u64,
                "frames_from_raf": 4_294_967_295_u64,
                "frames_from_fallback": 4_294_967_295_u64,
                "frame_schedules": 4_294_967_295_u64,
                "clear_only": 4_294_967_295_u64,
                "held": 4_294_967_295_u64,
                "clear_only_fraction": low_fraction,
                "held_fraction": high_fraction,
                "warp_refused": null,
                "warp_refused_fraction": null,
                "uncovered_fraction": {
                    "min": low_fraction,
                    "median": median_fraction,
                    "max": high_fraction,
                },
            },
            "completed_scene_ids": scene_ids,
            "discarded_scene_ids": null,
            "presented_scene_ids": null,
            "settle_to_paint_ms": null,
            "reference_requests_issued": null,
            "sampled_reference_requests_issued": 4_294_967_295_u64,
            "timing_records_observed": 64,
            "walls_us": {
                "worker_reference_us": wall_samples,
                "credit_wait_us": wall_samples,
                "transfer_us": null,
                "packing_us": null,
                "upload_us": null,
                "dispatch_us": null,
                "fence_us": wall_samples,
            },
            "scripted_hold_wall_ms": (spec.scenario_id == "hold-exact-2000ms").then_some(hold_wall_ms),
        });
        if final_record {
            note["stage_progress"] = json!({
                "completed_scenarios": 8,
                "total_scenarios": 8,
                "partial_reason": null,
            });
        }
        let raw_samples = vec![game_wall_ms; usize::from(spec.step_count)];
        KernelMeasurement {
            kernel_id: format!("julibrot-slide.{}.v1", spec.scenario_id),
            workload: format!(
                "{}; baseline {}; steps={}; interval_ms={}; raw=game-side step wall ms",
                spec.name, spec.baseline_row, spec.step_count, spec.interval_ms
            ),
            unit: "ms".to_string(),
            warmup_runs: 0,
            status: KernelStatus::Complete,
            unavailable_reason: None,
            raw_samples,
            summary: Some(SummaryStats {
                sample_count: u32::from(spec.step_count),
                median: game_wall_ms,
                p95: game_wall_ms,
                min: game_wall_ms,
                max: game_wall_ms,
            }),
            notes: vec![serde_json::to_string(&note).expect("fixture note")],
        }
    }

    #[test]
    fn scenario_table_is_stable_and_covers_every_requested_sequence() {
        assert_eq!(SCENARIOS.len(), 8);
        assert_eq!(SCENARIOS[0].scenario_id, "object-o13-range");
        assert_eq!(SCENARIOS[1].baseline_row, "gentle-d5-8");
        assert_eq!(SCENARIOS[2].baseline_row, "close-d5-2");
        assert_eq!(SCENARIOS[6].interval_ms, 2_000);
        assert_eq!(SCENARIOS[7].step_count, 30);
        let inventory = serde_json::to_string(&SCENARIOS).expect("scenario inventory");
        assert!(!inventory.contains("\"action\""));
    }

    #[test]
    fn adversarial_shipped_records_fit_the_stage_byte_budget() {
        let records = SCENARIOS
            .into_iter()
            .enumerate()
            .map(|(index, spec)| adversarial_record(spec, index + 1 == SCENARIOS.len()))
            .collect::<Vec<_>>();
        let encoded = serde_json::to_vec(&records).expect("measurement JSON");
        let decoded =
            serde_json::from_slice::<Vec<KernelMeasurement>>(&encoded).expect("schema records");
        assert_eq!(decoded, records);
        let bytes = encoded.len();
        assert_eq!(bytes, ADVERSARIAL_RECORD_BYTES);
        assert!(
            bytes <= JULIBROT_REPORT_BYTE_BUDGET,
            "{bytes} > {JULIBROT_REPORT_BYTE_BUDGET}"
        );
    }

    #[test]
    fn shipped_page_uses_lab_turns_appended_timings_and_bounded_waits() {
        let timing = section(
            page(),
            "function appendedJulibrotTimings",
            "function julibrotCollector",
        );
        assert!(timing.contains("finalKeys.lastIndexOf(baselineTail)"));
        let collector = section(
            page(),
            "function julibrotCollector",
            "async function observeJulibrotInterval",
        );
        assert!(collector.contains("frames.sample_count += 1"));
        assert!(collector.contains("frames.frames_from_raf"));
        assert!(collector.contains("frames.frames_from_fallback"));
        assert!(collector.contains("warp_refused: null"));
        assert!(collector.contains("appendedJulibrotTimings"));
        assert!(!collector.contains("record.edit >"));
        assert!(collector.contains("roundJulibrot(wallMs, 2)"));
        assert!(collector.contains("julibrotNumberSummary(uncoveredFractions, 4)"));
        let baseline = section(
            page(),
            "async function establishJulibrotBaseline",
            "function cappedDistinct",
        );
        assert!(baseline.contains("setJulibrotControl(document, 'iteration-cap', 512, 'change')"));
        let reader = section(
            page(),
            "function julibrotFrameFacts",
            "function isSettledJulibrotFinal",
        );
        assert!(reader.contains("JULIBROT_FACT_FRAME_DEADLINE_MS"));
        assert!(reader.contains("onStageTokenCancel"));
        assert!(page().contains("the page was hidden during the Julibrot slide; the lab drops to its 250 ms fallback timer and the game's animation-frame sampling stops, so no fast-slide frame can be observed"));
    }

    #[test]
    fn shipped_page_keeps_unavailable_partial_and_teardown_paths_distinct() {
        let measurement = section(
            page(),
            "async function measureJulibrotSlide",
            "function julibrotReportSummary",
        );
        assert!(measurement.contains("if (!records.length && failure)"));
        assert!(measurement.contains("run.kernels.push(...records)"));
        assert!(!measurement.contains("run.kernels.push(record)"));
        assert!(measurement.contains("setJulibrotStageProgress"));
        assert!(measurement.contains("fitJulibrotRecordsToBudget"));
        let local_budget = section(
            page(),
            "function fitJulibrotRecordsToBudget",
            "function setJulibrotStageProgress",
        );
        assert!(!local_budget.contains("throw"));
        let report_budget = section(
            page(),
            "function fitReportToByteCap",
            "function fallbackPersonality",
        );
        let julibrot_raw = report_budget
            .find("truncateJulibrotRawSample(julibrotRecords)")
            .expect("Julibrot raw-sample truncation");
        let julibrot_note = report_budget
            .find("truncateJulibrotNoteDetail(julibrotRecords)")
            .expect("Julibrot note-detail truncation");
        let other_raw = report_budget
            .find("!kernel.kernel_id.startsWith('julibrot-slide.') && kernel.raw_samples.length")
            .expect("other-stage raw-sample truncation");
        assert!(julibrot_raw < julibrot_note && julibrot_note < other_raw);
        assert!(page().contains("report cap truncation:'))"));
        assert!(page().contains("summary still describes the full measured sample set"));
        let retirement = section(
            page(),
            "function retireJulibrotFrame",
            "function julibrotFrameFacts",
        );
        assert!(retirement.contains("retired.src = 'about:blank'"));
        assert!(retirement.contains("retired.remove()"));
        assert!(retirement.contains("retired.contentDocument === null"));
        let stage = section(page(), "async function runStage", "function completeKernel");
        assert!(stage.contains("hiddenDuringStage"));
        assert!(stage.contains("page visibility over the stage:"));
        assert!(stage.contains("cancelStageToken(token, outcome.unavailableReason)"));
        assert!(page().contains("wasm.julibrot_report_byte_budget()"));
    }
}
