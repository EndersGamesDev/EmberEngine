//! Thin wasm exports for browser orchestration and canonical submission.

use std::cell::RefCell;
use std::collections::VecDeque;

use ember_client_net::{
    CanonicalHandshake, CanonicalSelection, ClientConnection, ConnectionProgress, TransportConfig,
    WireFrame,
};
use ember_game_what_is_this_v1::{
    ClientMessage, DiagnosticReport, FaerWasmVerdict, GAME_ID, GAME_VERSION, MAX_INNER_FRAME_BYTES,
    ServerMessage,
};
use ember_net::outer::{CreateLobby, Hello, OUTER_VERSION};
use serde::Serialize;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

use crate::{
    FloatProbeResult, KernelSuite, bar_progress, derive_verdict, jank_chunk, julibrot_scenarios,
    kernel_specs,
};

thread_local! {
    static KERNELS: RefCell<KernelSuite> = RefCell::new(KernelSuite::new());
    static SUBMISSION: RefCell<Option<Submission>> = const { RefCell::new(None) };
}

#[derive(Serialize)]
struct SubmissionView<'a> {
    state: &'a str,
    detail: &'a str,
    receipt_id: Option<&'a str>,
}

struct Submission {
    connection: ClientConnection<CanonicalHandshake>,
    report_frame: WireFrame,
    sent: bool,
    state: String,
    detail: String,
    receipt_id: Option<String>,
}

impl Submission {
    fn view_json(&self) -> String {
        serde_json::to_string(&SubmissionView {
            state: &self.state,
            detail: &self.detail,
            receipt_id: self.receipt_id.as_deref(),
        })
        .unwrap_or_else(|_| {
            r#"{"state":"failed","detail":"could not encode submission status","receipt_id":null}"#
                .to_string()
        })
    }

    fn poll(&mut self) {
        let mut incoming = VecDeque::new();
        self.connection.drain(&mut incoming);
        while let Some(frame) = incoming.pop_front() {
            let WireFrame::Text(text) = frame else {
                continue;
            };
            let Ok(message) = serde_json::from_str::<ServerMessage>(&text) else {
                continue;
            };
            match message {
                ServerMessage::Accepted { receipt_id } => {
                    self.state = "accepted".to_string();
                    self.detail = "server accepted the report and returned a receipt".to_string();
                    self.receipt_id = Some(receipt_id);
                }
                ServerMessage::Rejected { code, message } => {
                    self.state = "failed".to_string();
                    self.detail = format!("server rejected the report ({code}): {message}");
                }
            }
        }

        if self.state == "accepted" || self.state == "failed" {
            return;
        }
        match self.connection.progress() {
            ConnectionProgress::Connecting | ConnectionProgress::AwaitingWelcome => {
                self.state = "connecting".to_string();
                self.detail = "waiting for the canonical Ember server handshake".to_string();
            }
            ConnectionProgress::Selecting => {
                self.state = "creating_lobby".to_string();
                self.detail = "creating the generated solo what-is-this/1 lobby".to_string();
            }
            ConnectionProgress::Joined if !self.sent => {
                match self.connection.send(self.report_frame.clone()) {
                    Ok(()) => {
                        self.sent = true;
                        self.state = "submitting".to_string();
                        self.detail =
                            "report sent once; waiting for the server receipt".to_string();
                    }
                    Err(error) => {
                        self.state = "failed".to_string();
                        self.detail = format!("could not send the report: {error:?}");
                    }
                }
            }
            ConnectionProgress::Joined => {}
            ConnectionProgress::Browsing => {
                let diagnostics = self.connection.diagnostics();
                self.state = "failed".to_string();
                self.detail =
                    diagnostics.handshake.last().cloned().unwrap_or_else(|| {
                        "server left the submission in browsing state".to_string()
                    });
            }
            ConnectionProgress::Closed(reason) => {
                self.state = "failed".to_string();
                self.detail = format!("submission connection closed: {}", reason.detail);
            }
        }
    }
}

/// Initializes the wasm module without starting a renderer or network connection.
#[wasm_bindgen(start)]
pub fn wasm_init() {}

/// Returns the stable timed-kernel inventory as JSON.
#[wasm_bindgen]
pub fn kernel_inventory_json() -> String {
    serde_json::to_string(kernel_specs()).unwrap_or_else(|_| "[]".to_string())
}

/// Returns the stable Julibrot slide scenario inventory as JSON.
#[wasm_bindgen]
pub fn julibrot_scenario_inventory_json() -> String {
    serde_json::to_string(julibrot_scenarios()).unwrap_or_else(|_| "[]".to_string())
}

/// Validates and encodes one bounded Julibrot scenario as a schema-1 kernel measurement.
///
/// # Errors
///
/// Returns a JavaScript error when the scenario identifier, fixed step table, numeric samples, or
/// stage-specific byte and sample cap is invalid.
#[wasm_bindgen]
pub fn julibrot_measurement_json(observation_json: &str) -> Result<String, JsValue> {
    let measurement = crate::julibrot_measurement(observation_json)
        .map_err(|error| JsValue::from_str(&error))?;
    serde_json::to_string(&measurement).map_err(|error| {
        JsValue::from_str(&format!("could not encode Julibrot measurement: {error}"))
    })
}

/// Encodes an unavailable Julibrot stage result as a schema-1 kernel measurement.
#[wasm_bindgen]
pub fn julibrot_unavailable_measurement_json(reason: &str) -> String {
    serde_json::to_string(&crate::julibrot_unavailable_measurement(reason))
        .unwrap_or_else(|_| "{}".to_string())
}

/// Returns the maximum compact bytes reserved for all Julibrot scenario measurements.
#[wasm_bindgen]
pub fn julibrot_report_byte_budget() -> u32 {
    u32::try_from(crate::JULIBROT_REPORT_BYTE_BUDGET).unwrap_or(u32::MAX)
}

/// Returns the stable compute-only WebGPU kernel inventory as JSON.
#[wasm_bindgen]
pub fn gpu_compute_inventory_json() -> String {
    serde_json::to_string(crate::gpu::kernel_specs()).unwrap_or_else(|_| "[]".to_string())
}

/// Requests a compute-only WebGPU adapter and device, compiles the fixed pipelines, and returns
/// adapter facts as JSON.
///
/// # Errors
///
/// Returns a JavaScript error when WebGPU is absent, the browser refuses an adapter or device, or
/// the fixed shader and pipeline warmup fails validation.
#[wasm_bindgen]
pub async fn initialize_gpu_compute_json() -> Result<String, JsValue> {
    crate::gpu::initialize()
        .await
        .map_err(|error| JsValue::from_str(&error))
}

/// Runs one adaptively repeated compute-only WebGPU workload and returns timing and validation
/// facts as JSON.
///
/// # Errors
///
/// Returns a JavaScript error for an unknown kernel, invalid repeat count, device loss, mapping
/// failure, non-finite output, or output that differs from the fixed expected values.
#[wasm_bindgen]
pub async fn run_gpu_compute_json(kernel_id: &str, repeat_count: u32) -> Result<String, JsValue> {
    crate::gpu::run(kernel_id, repeat_count)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

/// Returns whether the initialized WebGPU compute device is still usable, including its device
/// loss reason when it is not.
#[wasm_bindgen]
pub fn gpu_compute_status_json() -> String {
    crate::gpu::status_json()
}

/// Cancels restoration of an in-flight WebGPU compute suite after a page watchdog expires.
#[wasm_bindgen]
pub fn cancel_gpu_compute() {
    crate::gpu::cancel();
}

/// Initializes the surface-backed WebGPU progress bar and returns adapter and surface facts as
/// JSON. The progress renderer owns a device separate from the compute suite.
///
/// # Errors
///
/// Returns a JavaScript error when WebGPU is absent, no canvas-compatible adapter is available, or
/// surface or pipeline initialization fails.
#[wasm_bindgen]
pub async fn initialize_render_bar_json(
    canvas: web_sys::HtmlCanvasElement,
) -> Result<String, JsValue> {
    crate::render_bar::initialize(canvas)
        .await
        .map_err(|error| JsValue::from_str(&error))
}

/// Presents one progress frame and returns the renderer's cumulative presented-frame count.
///
/// # Errors
///
/// Returns a JavaScript error when the renderer is unavailable, its device was lost, or surface
/// acquisition or presentation fails.
#[wasm_bindgen]
pub fn render_bar_frame(progress: f64) -> Result<u32, JsValue> {
    crate::render_bar::frame(progress).map_err(|error| JsValue::from_str(&error))
}

/// Drops the current progress surface and invalidates any pending renderer initialization.
#[wasm_bindgen]
pub fn reset_render_bar() {
    crate::render_bar::reset();
}

/// Returns the normalized 3D bar target derived from real suite, kernel, and sample progress.
#[wasm_bindgen]
pub fn bar_progress_fraction(
    stage_index: u32,
    stage_count: u32,
    kernel_index: u32,
    kernel_count: u32,
    sample_count: u32,
    sample_total: u32,
) -> f64 {
    bar_progress(
        stage_index,
        stage_count,
        kernel_index,
        kernel_count,
        sample_count,
        sample_total,
    )
}

/// Runs one preallocated fixed workload and returns its opaque checksum.
///
/// # Errors
///
/// Returns a JavaScript error when the supplied kernel identifier is unknown.
#[wasm_bindgen]
pub fn run_kernel(kernel_id: &str) -> Result<f64, JsValue> {
    KERNELS.with_borrow_mut(|kernels| {
        kernels
            .run(kernel_id)
            .map_err(|error| JsValue::from_str(&error))
    })
}

/// Runs repeated base invocations inside one wasm call for coarse-timer measurement.
///
/// # Errors
///
/// Returns a JavaScript error for zero repeats or an unknown kernel identifier.
#[wasm_bindgen]
pub fn run_kernel_repeated(kernel_id: &str, repeat_count: u32) -> Result<f64, JsValue> {
    KERNELS.with_borrow_mut(|kernels| {
        kernels
            .run_repeated(kernel_id, repeat_count)
            .map_err(|error| JsValue::from_str(&error))
    })
}

/// Runs one short arithmetic chunk for the page's controlled 50 ms main-thread burst.
#[wasm_bindgen]
pub fn run_jank_chunk() -> f64 {
    jank_chunk()
}

/// Returns the scalar contraction and baked-reference transcendental fingerprint as JSON.
#[wasm_bindgen]
pub fn float_probe_json() -> String {
    serde_json::to_string(&FloatProbeResult::measure()).unwrap_or_else(|_| {
        r#"{"fma":{"plain_result_bits":0,"separated_result_bits":0,"fused_result_bits":0,"contracts_to_fma":false},"transcendentals":[]}"#.to_string()
    })
}

/// Returns the pinned faer wasm compile verdict represented by this loaded module.
#[wasm_bindgen]
pub fn faer_wasm_verdict_json() -> String {
    serde_json::to_string(&FaerWasmVerdict {
        version: "0.24.4".to_string(),
        compiled: true,
        configuration: "default features disabled; std,linalg enabled; Par::Seq; no Rayon"
            .to_string(),
        consequence: "manual SoA plus matching faer rank-4, 6x6 LLT, and Spin(4) kernels included"
            .to_string(),
    })
    .unwrap_or_else(|_| "{}".to_string())
}

/// Resets per-run kernel buffers and any previous submission connection.
#[wasm_bindgen]
pub fn reset_run_state() {
    KERNELS.with_borrow_mut(|kernels| *kernels = KernelSuite::new());
    SUBMISSION.with_borrow_mut(|slot| *slot = None);
    crate::gpu::reset();
    crate::render_bar::reset();
}

/// Returns the deterministic, measurement-derived verdict presentation as JSON.
///
/// # Errors
///
/// Returns a JavaScript error when the supplied document is not a valid schema-1 report.
#[wasm_bindgen]
pub fn verdict_json(report_json: &str) -> Result<String, JsValue> {
    let report = serde_json::from_str::<DiagnosticReport>(report_json)
        .map_err(|error| JsValue::from_str(&format!("report does not match schema 1: {error}")))?;
    report
        .validate()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::to_string(&derive_verdict(&report))
        .map_err(|error| JsValue::from_str(&format!("could not encode verdict: {error}")))
}

/// Validates and starts the sole opt-in network submission through the canonical outer protocol.
///
/// # Errors
///
/// Returns a JavaScript error for invalid report JSON, an oversized report, an invalid URL, or an
/// immediate browser WebSocket failure.
#[wasm_bindgen]
pub fn start_submission(server_url: &str, report_json: &str) -> Result<(), JsValue> {
    let report = serde_json::from_str::<DiagnosticReport>(report_json)
        .map_err(|error| JsValue::from_str(&format!("report does not match schema 1: {error}")))?;
    report
        .validate()
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let canonical_report = serde_json::to_vec(&report)
        .map_err(|error| JsValue::from_str(&format!("could not encode report: {error}")))?;
    let digest = Sha256::digest(&canonical_report);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut lobby_name = String::from("wit-");
    for byte in digest.iter().take(6) {
        lobby_name.push(char::from(HEX[usize::from(byte >> 4)]));
        lobby_name.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    let message = ClientMessage::SubmitReport {
        report: Box::new(report),
    };
    let report_frame =
        WireFrame::Text(serde_json::to_string(&message).map_err(|error| {
            JsValue::from_str(&format!("could not encode submission: {error}"))
        })?);
    let handshake = CanonicalHandshake::new(
        Hello {
            outer_version: OUTER_VERSION,
            handle: "what-is-this".to_string(),
        },
        CanonicalSelection::Create(CreateLobby {
            game_id: GAME_ID.to_string(),
            game_version: GAME_VERSION,
            lobby_name,
            password: None,
        }),
    );
    let connection = ClientConnection::connect(
        server_url,
        TransportConfig {
            max_frame_bytes: MAX_INNER_FRAME_BYTES,
            inbox_capacity: 16,
            outbox_capacity: 8,
            keepalive: None,
        },
        handshake,
    )
    .map_err(|error| JsValue::from_str(&error))?;
    SUBMISSION.with_borrow_mut(|slot| {
        *slot = Some(Submission {
            connection,
            report_frame,
            sent: false,
            state: "connecting".to_string(),
            detail: "waiting for the canonical Ember server handshake".to_string(),
            receipt_id: None,
        });
    });
    Ok(())
}

/// Pumps the opt-in submission and returns its current status as JSON.
#[wasm_bindgen]
pub fn poll_submission_json() -> String {
    SUBMISSION.with_borrow_mut(|slot| match slot.as_mut() {
        Some(submission) => {
            submission.poll();
            submission.view_json()
        }
        None => r#"{"state":"idle","detail":"no report has been submitted","receipt_id":null}"#
            .to_string(),
    })
}
