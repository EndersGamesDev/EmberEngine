//! Hosted report receiver and report schema for “what is this?” protocol 1.
//!
//! The browser submits one bounded [`DiagnosticReport`] as one JSON text frame.
//! The session acknowledges it with a receipt derived from the canonical report
//! bytes, emits the accepted report through structured logging, and closes.

#![deny(missing_docs)]
// Protocol-qualified names remain useful at call sites.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeSet;

use ember_legacy::{
    AdmissionMetadata, AdmissionRefusal, CloseReason, CloseRequest, DecodedInput, EncodedEvent,
    FactoryError, GameFactory, GameKey, GameSession, InnerCodec, InnerCodecError, InnerFrame,
    LeaveReason, LegacyCapabilities, LobbyStatus, MonotonicTimestamp, OutboundEvent,
    OutboundTarget, PeerId, SessionCreationData, SessionInput, SessionUpdate,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Permanent hosted-game identifier.
pub const GAME_ID: &str = "what-is-this";
/// Frozen inner-wire and behavior version.
pub const GAME_VERSION: u32 = 1;
/// Immutable behavior-gate suite named by the hosted manifest.
pub const FIXTURE_SUITE_ID: &str = "what-is-this-v1-hosted-contract";
/// Version of the downloadable and submitted diagnostic report document.
pub const REPORT_SCHEMA_VERSION: u16 = 1;
/// Largest number of raw observations retained for one kernel.
pub const MAX_RAW_SAMPLES_PER_KERNEL: usize = 120;
/// Maximum compact JSON bytes accepted for the report object alone.
pub const MAX_REPORT_BYTES: usize = 52 * 1_024;
/// Maximum protocol-1 JSON text frame, including its message envelope.
pub const MAX_INNER_FRAME_BYTES: usize = 56 * 1_024;

/// Returns the exact registry key implemented by this crate.
#[must_use]
pub fn game_key() -> GameKey {
    GameKey {
        game_id: GAME_ID.to_string(),
        game_version: GAME_VERSION,
    }
}

/// Summary statistics calculated over the complete sample set before raw samples are capped.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SummaryStats {
    /// Number of observations represented by the summary.
    pub sample_count: u32,
    /// Median observation.
    pub median: f64,
    /// Nearest-rank 95th percentile observation.
    pub p95: f64,
    /// Smallest observation.
    pub min: f64,
    /// Largest observation.
    pub max: f64,
}

/// Screen facts exposed by the browser without additional permission.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenFacts {
    /// CSS-pixel screen width.
    pub width: u32,
    /// CSS-pixel screen height.
    pub height: u32,
    /// Browser device-pixel ratio.
    pub device_pixel_ratio: f64,
}

/// One page-visibility observation captured during the suite.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisibilityObservation {
    /// Milliseconds since the suite began.
    pub elapsed_ms: f64,
    /// Browser visibility state at that instant.
    pub state: String,
}

/// Resolution and monotonicity observations for `performance.now()`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimerFacts {
    /// Smallest positive observed timer delta in milliseconds.
    pub resolution_ms: Option<f64>,
    /// Number of equal consecutive readings.
    pub zero_delta_count: u32,
    /// Number of readings that moved backwards.
    pub monotonicity_violations: u32,
    /// Capped positive consecutive deltas in milliseconds.
    pub positive_delta_samples_ms: Vec<f64>,
    /// Summary of every positive consecutive delta.
    pub positive_delta_summary: Option<SummaryStats>,
    /// Caveat explaining that browser timing precision and scheduling affect every benchmark.
    pub caveat: String,
}

/// Browser and device facts collected without cookies, storage, or permission prompts.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentFacts {
    /// Stable versioned environment-facts kernel identifier.
    pub kernel_id: String,
    /// Browser-provided user-agent string.
    pub user_agent: String,
    /// Logical processor count when the browser exposes it.
    pub hardware_concurrency: Option<u32>,
    /// Approximate device memory in GiB when the browser exposes it.
    pub device_memory_gib: Option<f64>,
    /// Screen size and pixel density.
    pub screen: ScreenFacts,
    /// Timer-resolution and monotonicity probe.
    pub timer: TimerFacts,
    /// Visibility state when the run began.
    pub initial_visibility: String,
    /// Visibility state when the report was finalized.
    pub final_visibility: String,
    /// Whether any observation found the document hidden.
    pub hidden_during_run: bool,
    /// Capped visibility transitions and stage-boundary observations.
    pub visibility_observations: Vec<VisibilityObservation>,
}

/// One capability detected by executing or validating a feature probe.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityProbe {
    /// Stable versioned capability-kernel identifier.
    pub kernel_id: String,
    /// Whether the feature test succeeded.
    pub available: bool,
    /// Concrete failure or policy reason when unavailable.
    pub unavailable_reason: Option<String>,
}

/// WebAssembly capabilities detected without user-agent parsing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityFlags {
    /// SIMD128 validation result.
    pub simd128: CapabilityProbe,
    /// Shared-memory and thread-prerequisite result.
    pub threads: CapabilityProbe,
    /// Bulk-memory instruction validation result.
    pub bulk_memory: CapabilityProbe,
}

/// Result of the scalar multiply-add contraction probe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FmaProbe {
    /// Bits produced by a plain `a * b + c` expression.
    pub plain_result_bits: u32,
    /// Bits produced when multiplication and addition are forced apart.
    pub separated_result_bits: u32,
    /// Bits produced by explicit fused multiply-add.
    pub fused_result_bits: u32,
    /// Whether the plain expression matched the fused result instead of the separated result.
    pub contracts_to_fma: bool,
}

/// One fixed-input transcendental observation against a baked double-precision reference.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranscendentalObservation {
    /// Exact input as f32 bits.
    pub input_bits: u32,
    /// Baked double-precision sine reference.
    pub sin_reference_f64: f64,
    /// Observed wasm f32 sine bits.
    pub sin_observed_bits: u32,
    /// ULP distance from the correctly rounded f32 form of the reference.
    pub sin_ulp: u32,
    /// Baked double-precision cosine reference.
    pub cos_reference_f64: f64,
    /// Observed wasm f32 cosine bits.
    pub cos_observed_bits: u32,
    /// ULP distance from the correctly rounded f32 form of the reference.
    pub cos_ulp: u32,
}

/// Cross-device floating-point behavior recorded by stable probe kernels.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloatBehaviorFingerprint {
    /// Whether the wasm probe code ran.
    pub available: bool,
    /// Concrete module or execution failure when unavailable.
    pub unavailable_reason: Option<String>,
    /// Stable versioned contraction-kernel identifier.
    pub fma_kernel_id: String,
    /// Contraction observation.
    pub fma: Option<FmaProbe>,
    /// Stable versioned transcendental-kernel identifier.
    pub transcendental_kernel_id: String,
    /// Observations for the frozen input and reference table.
    pub transcendentals: Vec<TranscendentalObservation>,
}

/// Build-time outcome of the pinned faer wasm evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaerWasmVerdict {
    /// Evaluated faer version.
    pub version: String,
    /// Whether this exact client compiled with the dependency for wasm.
    pub compiled: bool,
    /// Explicit feature set and execution mode.
    pub configuration: String,
    /// Which kernels the verdict caused the client to ship.
    pub consequence: String,
}

/// Completion state for one diagnostic stage.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// The stage completed and contributed results.
    Complete,
    /// The stage could not safely run on this browser.
    Unavailable,
}

/// One named stage in the ordered browser suite.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageReport {
    /// Stable versioned stage identifier.
    pub stage_id: String,
    /// User-facing stage name.
    pub name: String,
    /// Completion state.
    pub status: StageStatus,
    /// Concrete reason when the stage is unavailable.
    pub unavailable_reason: Option<String>,
    /// Stage duration in milliseconds.
    pub duration_ms: f64,
}

/// Availability state for one kernel result.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelStatus {
    /// The base workload ran in resolved batches and produced observations.
    Complete,
    /// The workload could not safely run.
    Unavailable,
}

/// Raw and summarized observations for one stable, versioned kernel.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelMeasurement {
    /// Stable versioned kernel identifier.
    pub kernel_id: String,
    /// Base workload and timing strategy, including adaptive batching when used.
    pub workload: String,
    /// Unit shared by raw samples and summary values.
    pub unit: String,
    /// Number of untimed warmup invocations.
    pub warmup_runs: u16,
    /// Completion state.
    pub status: KernelStatus,
    /// Concrete reason when unavailable.
    pub unavailable_reason: Option<String>,
    /// Capped raw observations.
    pub raw_samples: Vec<f64>,
    /// Summary over the complete uncapped observation set.
    pub summary: Option<SummaryStats>,
    /// Kernel-specific caveats, adaptive repeat counts, and non-numeric facts; open-list GPU
    /// adapter facts record identity, browser-exposed limits, optional features, and timing mode;
    /// surface render records add adapter identity, surface format, device-isolation policy,
    /// visibility, and successful present count here without changing report schema 1.
    pub notes: Vec<String>,
}

/// Complete downloadable and optionally submitted diagnostic document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticReport {
    /// Schema discriminator; protocol 1 accepts only [`REPORT_SCHEMA_VERSION`].
    pub report_schema_version: u16,
    /// Client wall-clock timestamp in ISO-8601 form.
    pub generated_at: String,
    /// Total suite wall time measured with the browser monotonic timer.
    pub total_run_wall_ms: f64,
    /// Browser and timing environment.
    pub environment: EnvironmentFacts,
    /// Feature-tested wasm capabilities.
    pub capabilities: CapabilityFlags,
    /// Floating-point contraction and transcendental fingerprint.
    pub float_behavior: FloatBehaviorFingerprint,
    /// Pinned faer wasm build verdict.
    pub faer_wasm: FaerWasmVerdict,
    /// Ordered stage outcomes, including unavailable reasons.
    pub stages: Vec<StageReport>,
    /// Per-kernel raw samples and summary statistics.
    pub kernels: Vec<KernelMeasurement>,
}

/// Why a report violates protocol-1 structural or size limits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReportValidationError {
    /// The report requested an unknown schema version.
    UnsupportedSchema(u16),
    /// The client omitted its generation timestamp.
    MissingGeneratedAt,
    /// One kernel retained too many raw samples.
    TooManyRawSamples {
        /// Kernel whose raw array exceeded the cap.
        kernel_id: String,
        /// Number of supplied raw observations.
        supplied: usize,
    },
    /// Two kernel records claimed the same stable identifier.
    DuplicateKernelId(String),
    /// An unavailable stage or kernel omitted its reason.
    MissingUnavailableReason(String),
    /// Compact report JSON exceeded the protocol limit.
    ReportTooLarge(usize),
    /// The report could not be represented as finite JSON.
    NotSerializable(String),
}

impl std::fmt::Display for ReportValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported report schema version {version}")
            }
            Self::MissingGeneratedAt => formatter.write_str("generated_at is empty"),
            Self::TooManyRawSamples {
                kernel_id,
                supplied,
            } => write!(
                formatter,
                "kernel {kernel_id} supplied {supplied} raw samples; maximum is {MAX_RAW_SAMPLES_PER_KERNEL}"
            ),
            Self::DuplicateKernelId(kernel_id) => {
                write!(formatter, "duplicate kernel id {kernel_id}")
            }
            Self::MissingUnavailableReason(id) => {
                write!(formatter, "unavailable result {id} omitted its reason")
            }
            Self::ReportTooLarge(bytes) => write!(
                formatter,
                "report is {bytes} bytes; maximum is {MAX_REPORT_BYTES}"
            ),
            Self::NotSerializable(message) => {
                write!(formatter, "report is not serializable: {message}")
            }
        }
    }
}

impl std::error::Error for ReportValidationError {}

impl DiagnosticReport {
    /// Validates schema, bounded arrays, unavailable reasons, unique kernel IDs, and compact size.
    ///
    /// # Errors
    ///
    /// Returns the first protocol-1 report validation failure.
    pub fn validate(&self) -> Result<(), ReportValidationError> {
        if self.report_schema_version != REPORT_SCHEMA_VERSION {
            return Err(ReportValidationError::UnsupportedSchema(
                self.report_schema_version,
            ));
        }
        if self.generated_at.is_empty() {
            return Err(ReportValidationError::MissingGeneratedAt);
        }
        for stage in &self.stages {
            if stage.status == StageStatus::Unavailable
                && stage
                    .unavailable_reason
                    .as_deref()
                    .is_none_or(str::is_empty)
            {
                return Err(ReportValidationError::MissingUnavailableReason(
                    stage.stage_id.clone(),
                ));
            }
        }
        let mut kernel_ids = BTreeSet::new();
        for kernel in &self.kernels {
            if kernel.raw_samples.len() > MAX_RAW_SAMPLES_PER_KERNEL {
                return Err(ReportValidationError::TooManyRawSamples {
                    kernel_id: kernel.kernel_id.clone(),
                    supplied: kernel.raw_samples.len(),
                });
            }
            if !kernel_ids.insert(kernel.kernel_id.clone()) {
                return Err(ReportValidationError::DuplicateKernelId(
                    kernel.kernel_id.clone(),
                ));
            }
            if kernel.status == KernelStatus::Unavailable
                && kernel
                    .unavailable_reason
                    .as_deref()
                    .is_none_or(str::is_empty)
            {
                return Err(ReportValidationError::MissingUnavailableReason(
                    kernel.kernel_id.clone(),
                ));
            }
        }
        let encoded = serde_json::to_vec(self)
            .map_err(|error| ReportValidationError::NotSerializable(error.to_string()))?;
        if encoded.len() > MAX_REPORT_BYTES {
            return Err(ReportValidationError::ReportTooLarge(encoded.len()));
        }
        Ok(())
    }
}

/// Exact client-to-server JSON messages for protocol 1.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// Submit the sole report accepted by this solo session.
    SubmitReport {
        /// Complete schema-v1 report.
        report: Box<DiagnosticReport>,
    },
}

/// Exact server-to-client JSON messages for protocol 1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    /// The host accepted and logged the report.
    Accepted {
        /// Stable content-derived receipt identifier.
        receipt_id: String,
    },
    /// The host rejected the message while preserving a stable error code.
    Rejected {
        /// Stable machine-readable refusal code.
        code: String,
        /// Human-readable refusal detail.
        message: String,
    },
}

/// Protocol-1 JSON text-frame codec.
#[derive(Clone, Copy, Debug, Default)]
pub struct WhatIsThisCodec;

impl InnerCodec for WhatIsThisCodec {
    fn decode(&self, frame: &InnerFrame) -> Result<DecodedInput, InnerCodecError> {
        let InnerFrame::Text(text) = frame else {
            return Err(InnerCodecError::WrongFrameKind);
        };
        if text.len() > MAX_INNER_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "what-is-this protocol 1 frame is {} bytes; maximum is {MAX_INNER_FRAME_BYTES}",
                text.len()
            )));
        }
        let message = serde_json::from_str::<ClientMessage>(text)
            .map_err(|error| InnerCodecError::DecodeFailed(error.to_string()))?;
        match &message {
            ClientMessage::SubmitReport { report } => report
                .validate()
                .map_err(|error| InnerCodecError::InvalidFrame(error.to_string()))?,
        }
        Ok(DecodedInput {
            payload: text.as_bytes().to_vec(),
        })
    }

    fn encode(&self, event: &EncodedEvent) -> Result<InnerFrame, InnerCodecError> {
        if event.payload.len() > MAX_INNER_FRAME_BYTES {
            return Err(InnerCodecError::InvalidFrame(format!(
                "what-is-this protocol 1 event is {} bytes; maximum is {MAX_INNER_FRAME_BYTES}",
                event.payload.len()
            )));
        }
        serde_json::from_slice::<ServerMessage>(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        let text = std::str::from_utf8(&event.payload)
            .map_err(|error| InnerCodecError::EncodeFailed(error.to_string()))?;
        Ok(InnerFrame::Text(text.to_owned()))
    }
}

/// Registry factory for solo report-receiver sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct WhatIsThisFactory;

impl GameFactory for WhatIsThisFactory {
    fn create(
        &self,
        _capabilities: &LegacyCapabilities,
        creation: &SessionCreationData,
    ) -> Result<Box<dyn GameSession>, FactoryError> {
        if creation.game_key != game_key() {
            return Err(FactoryError::InvalidConfiguration(format!(
                "expected {GAME_ID}/{GAME_VERSION}, got {}/{}",
                creation.game_key.game_id, creation.game_key.game_version
            )));
        }
        if !creation.configured_rules.is_empty() {
            return Err(FactoryError::InvalidConfiguration(
                "what-is-this protocol 1 accepts no configured rules".to_string(),
            ));
        }
        Ok(Box::new(ReportSession::default()))
    }
}

/// One solo session accepting at most one diagnostic report.
#[derive(Default)]
pub struct ReportSession {
    member: Option<PeerId>,
    accepted: bool,
}

impl ReportSession {
    fn response(peer_id: PeerId, message: &ServerMessage) -> SessionUpdate {
        let event = serde_json::to_vec(message).map_or_else(
            |_| EncodedEvent {
                payload: br#"{"type":"rejected","code":"encode_failed","message":"server could not encode its response"}"#.to_vec(),
            },
            |payload| EncodedEvent { payload },
        );
        SessionUpdate {
            outbound: vec![OutboundEvent {
                target: OutboundTarget::Peers(vec![peer_id]),
                event,
            }],
            scheduling: Vec::new(),
            closes: Vec::new(),
        }
    }

    fn accept_report(&mut self, peer_id: PeerId, report: &DiagnosticReport) -> SessionUpdate {
        if self.accepted {
            return Self::response(
                peer_id,
                &ServerMessage::Rejected {
                    code: "one_report_per_session".to_string(),
                    message: "this session already accepted its report".to_string(),
                },
            );
        }
        let Ok(report_json) = serde_json::to_string(report) else {
            return Self::response(
                peer_id,
                &ServerMessage::Rejected {
                    code: "invalid_report".to_string(),
                    message: "the report could not be serialized".to_string(),
                },
            );
        };
        let receipt_id = receipt_id(report_json.as_bytes());
        self.accepted = true;
        tracing::info!(
            target: "what_is_this_report",
            receipt_id = %receipt_id,
            report_schema_version = report.report_schema_version,
            report_bytes = report_json.len(),
            report_json = %report_json,
            "accepted what-is-this diagnostic report"
        );
        let mut update = Self::response(peer_id, &ServerMessage::Accepted { receipt_id });
        update.closes.push(CloseRequest {
            peer_id,
            reason: CloseReason::Requested,
        });
        update
    }
}

impl GameSession for ReportSession {
    fn step(&mut self, _timestamp: MonotonicTimestamp, inputs: Vec<SessionInput>) -> SessionUpdate {
        let mut combined = SessionUpdate::default();
        for input in inputs {
            if self.member != Some(input.peer_id) {
                continue;
            }
            let Ok(message) = serde_json::from_slice::<ClientMessage>(&input.input.payload) else {
                continue;
            };
            let mut update = match message {
                ClientMessage::SubmitReport { report } => {
                    self.accept_report(input.peer_id, report.as_ref())
                }
            };
            combined.outbound.append(&mut update.outbound);
            combined.scheduling.append(&mut update.scheduling);
            combined.closes.append(&mut update.closes);
        }
        combined
    }

    fn join(&mut self, admission: AdmissionMetadata) -> Result<SessionUpdate, AdmissionRefusal> {
        if self.member.is_some() {
            return Err(AdmissionRefusal {
                code: "solo_lobby_full".to_string(),
                message: "this diagnostic lobby already has its one submitter".to_string(),
            });
        }
        self.member = Some(admission.peer_id);
        Ok(SessionUpdate::default())
    }

    fn leave(&mut self, peer_id: PeerId, _reason: LeaveReason) -> SessionUpdate {
        if self.member == Some(peer_id) {
            self.member = None;
        }
        SessionUpdate::default()
    }

    fn lobby_status(&self) -> LobbyStatus {
        LobbyStatus {
            code: if self.accepted {
                "report_received"
            } else {
                "awaiting_report"
            }
            .to_string(),
            detail: None,
        }
    }
}

fn receipt_id(report_bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(report_bytes);
    let mut receipt = String::with_capacity(5 + digest.len() * 2);
    receipt.push_str("wit1-");
    for byte in digest {
        receipt.push(char::from(HEX[usize::from(byte >> 4)]));
        receipt.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    receipt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_is_stable_and_content_derived() {
        let first = receipt_id(br#"{"report_schema_version":1}"#);
        assert_eq!(first, receipt_id(br#"{"report_schema_version":1}"#));
        assert_ne!(first, receipt_id(br#"{"report_schema_version":2}"#));
        assert_eq!(first.len(), 69);
        assert!(first.starts_with("wit1-"));
    }

    #[test]
    fn codec_rejects_binary_frames() {
        assert_eq!(
            WhatIsThisCodec.decode(&InnerFrame::Binary(vec![1, 2, 3])),
            Err(InnerCodecError::WrongFrameKind)
        );
    }
}
