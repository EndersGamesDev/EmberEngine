//! One event shape for every phase.
//!
//! One shape rather than one per phase: a page renders the load in a single
//! handler, and a handler that has to know which fields exist for which phase
//! is a handler that reads a stale `percent` during compilation. Fields that
//! do not apply are absent, and absent is a value the page can test.

use crate::phase::Phase;

/// Property names on the plain object delivered to a page.
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) mod js_key {
    pub const PHASE: &str = "phase";
    pub const STATUS: &str = "status";
    pub const ELAPSED_MS: &str = "elapsedMs";
    pub const PHASE_MS: &str = "phaseMs";
    pub const LOADED: &str = "loaded";
    pub const TOTAL: &str = "total";
    pub const PERCENT: &str = "percent";
    pub const RATE_BPS: &str = "rateBps";
    pub const ETA_MS: &str = "etaMs";
    pub const STALLED: &str = "stalled";
    pub const STALLED_MS: &str = "stalledMs";
    pub const REASON: &str = "reason";
    pub const DETAIL: &str = "detail";
    pub const TEXT: &str = "text";

    /// The documented browser event shape, in Rust field order.
    #[cfg(test)]
    pub const ALL: [&str; super::EVENT_FIELD_COUNT] = [
        PHASE, STATUS, ELAPSED_MS, PHASE_MS, LOADED, TOTAL, PERCENT, RATE_BPS, ETA_MS, STALLED,
        STALLED_MS, REASON, DETAIL, TEXT,
    ];
}

/// Where in a phase an event sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The phase has just begun.
    Begin,
    /// The phase is running and something measurable changed.
    Progress,
    /// The phase finished.
    Done,
    /// The phase failed, with a reason.
    Fail,
}

impl Status {
    /// The name a page sees.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Begin => "begin",
            Self::Progress => "progress",
            Self::Done => "done",
            Self::Fail => "fail",
        }
    }
}

#[cfg(test)]
const EVENT_FIELD_COUNT: usize = 14;

/// What the page is told.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Which phase this is about.
    pub phase: Phase,
    /// Where in that phase.
    pub status: Status,
    /// Milliseconds since the load began.
    pub elapsed_ms: f64,
    /// Milliseconds since this phase began.
    pub phase_ms: f64,
    /// Bytes received so far, during and after the download.
    pub loaded: Option<u64>,
    /// The known decoded length, when there is one.
    pub total: Option<u64>,
    /// Whole percent, when the decoded length is known.
    pub percent: Option<u32>,
    /// Mean bytes per second, once two chunks have arrived.
    pub rate_bps: Option<f64>,
    /// Milliseconds left at that rate, when the length is known.
    pub eta_ms: Option<f64>,
    /// Whether the download has gone quiet.
    pub stalled: bool,
    /// How long it has been quiet, while it is.
    pub stalled_ms: f64,
    /// A short machine-readable code on a failure.
    pub reason: Option<String>,
    /// The underlying message on a failure.
    pub detail: Option<String>,
    /// One line a page can print as it stands.
    pub text: String,
}

impl Event {
    /// A bare event for `phase` and `status` at `elapsed_ms`.
    #[must_use]
    pub const fn new(phase: Phase, status: Status, elapsed_ms: f64, phase_ms: f64) -> Self {
        Self {
            phase,
            status,
            elapsed_ms,
            phase_ms,
            loaded: None,
            total: None,
            percent: None,
            rate_bps: None,
            eta_ms: None,
            stalled: false,
            stalled_ms: 0.0,
            reason: None,
            detail: None,
            text: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{EVENT_FIELD_COUNT, js_key};

    #[test]
    fn the_browser_event_shape_has_one_camel_case_key_per_rust_field() {
        let unique = js_key::ALL.into_iter().collect::<BTreeSet<_>>();
        assert_eq!(js_key::ALL.len(), EVENT_FIELD_COUNT);
        assert_eq!(unique.len(), EVENT_FIELD_COUNT);
        assert!(unique.iter().all(|key| !key.contains('_')));
    }
}
