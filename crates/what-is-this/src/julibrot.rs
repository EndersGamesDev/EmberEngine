//! Fixed Julibrot slide scenarios and bounded report records.

use ember_game_what_is_this_v1::{KernelMeasurement, KernelStatus, SummaryStats};
use serde::{Deserialize, Serialize};

pub(crate) const STAGE_ID: &str = "stage.julibrot-slide.v1";
/// Maximum compact JSON bytes reserved for all Julibrot scenario measurements.
pub const JULIBROT_REPORT_BYTE_BUDGET: usize = 10 * 1_024;
const FACT_SAMPLE_CAP: usize = 12;
const NOTE_BYTE_CAP: usize = 1_280;

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
    /// Driver action interpreted by the page.
    pub action: &'static str,
}

const SCENARIOS: [JulibrotScenarioSpec; 8] = [
    JulibrotScenarioSpec {
        scenario_id: "object-o13-range",
        name: "o13 full-range drag",
        baseline_row: "Mandelbrot",
        step_count: 12,
        interval_ms: 50,
        action: "range:o13",
    },
    JulibrotScenarioSpec {
        scenario_id: "height-gentle-d5-8",
        name: "height drag at d5 8",
        baseline_row: "gentle-d5-8",
        step_count: 24,
        interval_ms: 125,
        action: "height:d5=8",
    },
    JulibrotScenarioSpec {
        scenario_id: "height-close-d5-2",
        name: "height drag at d5 2",
        baseline_row: "close-d5-2",
        step_count: 24,
        interval_ms: 125,
        action: "height:d5=2",
    },
    JulibrotScenarioSpec {
        scenario_id: "scale-deep-row",
        name: "scale drag on a deep row",
        baseline_row: "deep-scale-14",
        step_count: 10,
        interval_ms: 80,
        action: "range:scale=14..18",
    },
    JulibrotScenarioSpec {
        scenario_id: "rapid-a-to-b-morph",
        name: "rapid A-to-B morph",
        baseline_row: "Mandelbrot relief to Julia relief",
        step_count: 16,
        interval_ms: 25,
        action: "range:morph",
    },
    JulibrotScenarioSpec {
        scenario_id: "iteration-cap-mid-view",
        name: "iteration-cap change mid-view",
        baseline_row: "deep-scale-14",
        step_count: 2,
        interval_ms: 100,
        action: "select:iteration-cap=512,1024",
    },
    JulibrotScenarioSpec {
        scenario_id: "hold-exact-2000ms",
        name: "exact 2,000 ms hold after one change",
        baseline_row: "Mandelbrot relief",
        step_count: 1,
        interval_ms: 2_000,
        action: "hold:o13",
    },
    JulibrotScenarioSpec {
        scenario_id: "alternating-two-slider-burst",
        name: "30-step alternating-slider burst",
        baseline_row: "Mandelbrot relief",
        step_count: 30,
        interval_ms: 16,
        action: "alternate:o13,q13",
    },
];

/// Returns the complete stable Julibrot scenario inventory in report order.
#[must_use]
pub const fn julibrot_scenarios() -> &'static [JulibrotScenarioSpec] {
    &SCENARIOS
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrameObservation {
    observed: u32,
    clear_only: u32,
    held: u32,
    warp_refused: u32,
    clear_only_fraction: f64,
    held_fraction: f64,
    warp_refused_fraction: f64,
    uncovered_fraction_samples: Vec<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WallObservation {
    worker_reference_us: Option<Vec<u64>>,
    credit_wait_us: Option<Vec<u64>>,
    transfer_us: Option<Vec<u64>>,
    packing_us: Option<Vec<u64>>,
    upload_us: Option<Vec<u64>>,
    dispatch_us: Option<Vec<u64>>,
    fence_us: Option<Vec<u64>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioObservation {
    scenario_id: String,
    step_count: u16,
    interval_ms: u16,
    frames: FrameObservation,
    completed_scene_ids: Vec<u64>,
    discarded_scene_ids: Option<Vec<u64>>,
    presented_scene_ids: Option<Vec<u64>>,
    settle_to_paint_ms: Option<Vec<f64>>,
    reference_requests_issued: Option<u32>,
    sampled_reference_requests_issued: Option<u32>,
    walls_us: WallObservation,
    scripted_hold_wall_ms: Option<f64>,
    game_side_wall_ms: Vec<f64>,
}

fn scenario(scenario_id: &str) -> Option<&'static JulibrotScenarioSpec> {
    SCENARIOS
        .iter()
        .find(|candidate| candidate.scenario_id == scenario_id)
}

fn finite_nonnegative(values: &[f64]) -> bool {
    values
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0)
}

fn validate_optional_samples(values: &Option<Vec<u64>>) -> bool {
    values
        .as_ref()
        .is_none_or(|samples| samples.len() <= FACT_SAMPLE_CAP)
}

fn validate_observation(
    observation: &ScenarioObservation,
) -> Result<&'static JulibrotScenarioSpec, String> {
    let spec = scenario(&observation.scenario_id)
        .ok_or_else(|| format!("unknown Julibrot scenario {}", observation.scenario_id))?;
    if observation.step_count != spec.step_count || observation.interval_ms != spec.interval_ms {
        return Err(format!(
            "Julibrot scenario {} did not use its fixed step table",
            observation.scenario_id
        ));
    }
    if observation.game_side_wall_ms.len() != usize::from(spec.step_count)
        || !finite_nonnegative(&observation.game_side_wall_ms)
    {
        return Err(format!(
            "Julibrot scenario {} omitted a finite game-side wall for one or more steps",
            observation.scenario_id
        ));
    }
    let frames = &observation.frames;
    if frames.clear_only > frames.observed
        || frames.held > frames.observed
        || frames.warp_refused > frames.observed
        || frames.uncovered_fraction_samples.len() > FACT_SAMPLE_CAP
        || !finite_nonnegative(&frames.uncovered_fraction_samples)
        || ![
            frames.clear_only_fraction,
            frames.held_fraction,
            frames.warp_refused_fraction,
        ]
        .into_iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(&value))
    {
        return Err(format!(
            "Julibrot scenario {} supplied invalid frame observations",
            observation.scenario_id
        ));
    }
    if observation.completed_scene_ids.len() > FACT_SAMPLE_CAP
        || observation
            .discarded_scene_ids
            .as_ref()
            .is_some_and(|samples| samples.len() > FACT_SAMPLE_CAP)
        || observation
            .presented_scene_ids
            .as_ref()
            .is_some_and(|samples| samples.len() > FACT_SAMPLE_CAP)
        || observation
            .settle_to_paint_ms
            .as_ref()
            .is_some_and(|samples| {
                samples.len() > usize::from(spec.step_count) || !finite_nonnegative(samples)
            })
        || !validate_optional_samples(&observation.walls_us.worker_reference_us)
        || !validate_optional_samples(&observation.walls_us.credit_wait_us)
        || !validate_optional_samples(&observation.walls_us.transfer_us)
        || !validate_optional_samples(&observation.walls_us.packing_us)
        || !validate_optional_samples(&observation.walls_us.upload_us)
        || !validate_optional_samples(&observation.walls_us.dispatch_us)
        || !validate_optional_samples(&observation.walls_us.fence_us)
        || match (spec.scenario_id, observation.scripted_hold_wall_ms) {
            ("hold-exact-2000ms", Some(wall)) => !wall.is_finite() || wall < 2_000.0,
            ("hold-exact-2000ms", None) => true,
            (_, Some(_)) => true,
            (_, None) => false,
        }
    {
        return Err(format!(
            "Julibrot scenario {} exceeded a stage sample cap",
            observation.scenario_id
        ));
    }
    Ok(spec)
}

fn summarize(samples: &[f64]) -> Option<SummaryStats> {
    if samples.is_empty() {
        return None;
    }
    let mut ordered = samples.to_vec();
    ordered.sort_by(f64::total_cmp);
    let middle = ordered.len() / 2;
    let median = if ordered.len() % 2 == 0 {
        (ordered[middle - 1] + ordered[middle]) / 2.0
    } else {
        ordered[middle]
    };
    let p95_index = (ordered.len() * 95).div_ceil(100).saturating_sub(1);
    Some(SummaryStats {
        sample_count: u32::try_from(samples.len()).unwrap_or(u32::MAX),
        median,
        p95: ordered[p95_index],
        min: ordered[0],
        max: ordered[ordered.len() - 1],
    })
}

/// Validates one browser observation and builds its bounded schema-1 measurement.
///
/// # Errors
///
/// Returns a typed explanation when the observation does not match the fixed scenario table or
/// exceeds a stage-specific sample or byte cap.
pub fn julibrot_measurement(observation_json: &str) -> Result<KernelMeasurement, String> {
    let observation = serde_json::from_str::<ScenarioObservation>(observation_json)
        .map_err(|error| format!("Julibrot scenario record is invalid: {error}"))?;
    let spec = validate_observation(&observation)?;
    let mut note_value = serde_json::to_value(&observation)
        .map_err(|error| format!("Julibrot scenario record could not be encoded: {error}"))?;
    note_value
        .as_object_mut()
        .ok_or_else(|| "Julibrot scenario record did not encode as an object".to_string())?
        .remove("game_side_wall_ms");
    let note = serde_json::to_string(&note_value)
        .map_err(|error| format!("Julibrot scenario note could not be encoded: {error}"))?;
    if note.len() > NOTE_BYTE_CAP {
        return Err(format!(
            "Julibrot scenario {} note is {} bytes; cap is {NOTE_BYTE_CAP}",
            spec.scenario_id,
            note.len()
        ));
    }
    let summary = summarize(&observation.game_side_wall_ms);
    Ok(KernelMeasurement {
        kernel_id: format!("julibrot-slide.{}.v1", spec.scenario_id),
        workload: format!(
            "{}; baseline {}; steps={}; interval_ms={}; raw=game-side input-dispatch ms",
            spec.name, spec.baseline_row, spec.step_count, spec.interval_ms
        ),
        unit: "ms".to_string(),
        warmup_runs: 0,
        status: KernelStatus::Complete,
        unavailable_reason: None,
        raw_samples: observation.game_side_wall_ms,
        summary,
        notes: vec![note],
    })
}

/// Builds the schema-1 kernel record paired with an unavailable Julibrot stage.
#[must_use]
pub fn julibrot_unavailable_measurement(reason: &str) -> KernelMeasurement {
    KernelMeasurement {
        kernel_id: "julibrot-slide.unavailable.v1".to_string(),
        workload: "load the same-origin Julibrot lab and reach a settled Final scene".to_string(),
        unit: "facts".to_string(),
        warmup_runs: 0,
        status: KernelStatus::Unavailable,
        unavailable_reason: Some(reason.to_string()),
        raw_samples: Vec::new(),
        summary: None,
        notes: vec![
            "the Julibrot refusal is stage unavailability, not a failure of the diagnostic game"
                .to_string(),
        ],
    }
}

#[cfg(test)]
fn unavailable_stage(reason: &str, duration_ms: f64) -> ember_game_what_is_this_v1::StageReport {
    ember_game_what_is_this_v1::StageReport {
        stage_id: STAGE_ID.to_string(),
        name: "Julibrot fast-slide".to_string(),
        status: ember_game_what_is_this_v1::StageStatus::Unavailable,
        unavailable_reason: Some(reason.to_string()),
        duration_ms,
    }
}

pub(crate) fn observation(report: &ember_game_what_is_this_v1::DiagnosticReport) -> Option<String> {
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
    let mut clear_only = 0_u32;
    let mut held = 0_u32;
    for kernel in report.kernels.iter().filter(|kernel| {
        kernel.kernel_id.starts_with("julibrot-slide.") && kernel.status == KernelStatus::Complete
    }) {
        let Some(note) = kernel.notes.first() else {
            continue;
        };
        let Ok(measured) = serde_json::from_str::<ScenarioObservation>(note) else {
            continue;
        };
        scenarios += 1;
        observed = observed.saturating_add(measured.frames.observed);
        clear_only = clear_only.saturating_add(measured.frames.clear_only);
        held = held.saturating_add(measured.frames.held);
    }
    Some(format!(
        "The Julibrot slide drove {scenarios} fixed scenarios across {observed} observed frames: {clear_only} clear-only and {held} held."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation_for(spec: JulibrotScenarioSpec) -> ScenarioObservation {
        ScenarioObservation {
            scenario_id: spec.scenario_id.to_string(),
            step_count: spec.step_count,
            interval_ms: spec.interval_ms,
            frames: FrameObservation {
                observed: 120,
                clear_only: 12,
                held: 20,
                warp_refused: 20,
                clear_only_fraction: 0.1,
                held_fraction: 1.0 / 6.0,
                warp_refused_fraction: 1.0 / 6.0,
                uncovered_fraction_samples: vec![0.25; FACT_SAMPLE_CAP],
            },
            completed_scene_ids: (1..=u64::try_from(FACT_SAMPLE_CAP).unwrap_or(12)).collect(),
            discarded_scene_ids: None,
            presented_scene_ids: None,
            settle_to_paint_ms: None,
            reference_requests_issued: None,
            sampled_reference_requests_issued: Some(2),
            walls_us: WallObservation {
                worker_reference_us: Some(vec![1; FACT_SAMPLE_CAP]),
                credit_wait_us: None,
                transfer_us: None,
                packing_us: None,
                upload_us: None,
                dispatch_us: None,
                fence_us: Some(vec![2; FACT_SAMPLE_CAP]),
            },
            scripted_hold_wall_ms: (spec.scenario_id == "hold-exact-2000ms").then_some(2_000.25),
            game_side_wall_ms: vec![0.125; usize::from(spec.step_count)],
        }
    }

    #[test]
    fn scenario_table_is_stable_and_covers_every_requested_sequence() {
        assert_eq!(SCENARIOS.len(), 8);
        assert_eq!(SCENARIOS[0].action, "range:o13");
        assert_eq!(SCENARIOS[1].baseline_row, "gentle-d5-8");
        assert_eq!(SCENARIOS[2].baseline_row, "close-d5-2");
        assert_eq!(SCENARIOS[6].interval_ms, 2_000);
        assert_eq!(SCENARIOS[7].step_count, 30);
    }

    #[test]
    fn complete_scenario_records_fit_the_stage_byte_budget() {
        let records = SCENARIOS
            .into_iter()
            .map(|spec| {
                julibrot_measurement(
                    &serde_json::to_string(&observation_for(spec)).expect("fixture JSON"),
                )
                .expect("bounded scenario")
            })
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&records)
            .expect("measurement JSON")
            .len();
        assert!(
            bytes <= JULIBROT_REPORT_BYTE_BUDGET,
            "{bytes} > {JULIBROT_REPORT_BYTE_BUDGET}"
        );
    }

    #[test]
    fn unavailable_path_is_an_honest_schema_measurement() {
        let record = julibrot_unavailable_measurement("WebGL2 unavailable");
        let stage = unavailable_stage("WebGL2 unavailable", 12.5);
        assert_eq!(record.status, KernelStatus::Unavailable);
        assert_eq!(
            record.unavailable_reason.as_deref(),
            Some("WebGL2 unavailable")
        );
        assert_eq!(
            stage.status,
            ember_game_what_is_this_v1::StageStatus::Unavailable
        );
        assert_eq!(stage.unavailable_reason, record.unavailable_reason);
        assert!(
            serde_json::to_vec(&record).expect("measurement JSON").len()
                < JULIBROT_REPORT_BYTE_BUDGET
        );
    }

    #[test]
    fn scenario_record_round_trips_through_the_schema_measurement() {
        let record = julibrot_measurement(
            &serde_json::to_string(&observation_for(SCENARIOS[0])).expect("fixture JSON"),
        )
        .expect("bounded scenario");
        let encoded = serde_json::to_vec(&record).expect("measurement JSON");
        let decoded = serde_json::from_slice::<KernelMeasurement>(&encoded).expect("schema record");
        assert_eq!(decoded, record);
    }

    #[test]
    fn measurement_rejects_a_changed_scenario_table() {
        let mut changed = observation_for(SCENARIOS[0]);
        changed.step_count += 1;
        let error = julibrot_measurement(&serde_json::to_string(&changed).expect("fixture JSON"))
            .expect_err("changed step table must be rejected");
        assert!(error.contains("fixed step table"));
    }
}
