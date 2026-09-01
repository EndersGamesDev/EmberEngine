//! Thin wasm exports for browser orchestration and canonical submission.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;

use ember_client_net::{
    CanonicalHandshake, CanonicalSelection, ClientConnection, ConnectionProgress, TransportConfig,
    WireFrame,
};
use ember_game_what_is_this_v1::{
    ClientMessage, DiagnosticReport, FaerWasmVerdict, GAME_ID, GAME_VERSION,
    MAX_INNER_FRAME_BYTES, ServerMessage,
};
use ember_net::outer::{CreateLobby, Hello, OUTER_VERSION};
use serde::Serialize;
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

use crate::{FloatProbeResult, KernelSuite, jank_chunk, kernel_specs};

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
            r#"{"state":"failed","detail":"could not encode submission status","receipt_id":null}"#.to_string()
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
                        self.detail = "report sent once; waiting for the server receipt".to_string();
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
                self.detail = diagnostics.handshake.last().cloned().unwrap_or_else(|| {
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
    let mut lobby_name = String::from("wit-");
    for byte in digest.iter().take(6) {
        drop(write!(&mut lobby_name, "{byte:02x}"));
    }
    let message = ClientMessage::SubmitReport {
        report: Box::new(report),
    };
    let report_frame = WireFrame::Text(
        serde_json::to_string(&message)
            .map_err(|error| JsValue::from_str(&format!("could not encode submission: {error}")))?,
    );
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
