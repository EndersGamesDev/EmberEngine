//! One event shape for every phase.
//!
//! One shape rather than one per phase: a page renders the load in a single
//! handler, and a handler that has to know which fields exist for which phase
//! is a handler that reads a stale `percent` during compilation. Fields that
//! do not apply are absent, and absent is a value the page can test.

use ember_boundary::Boundary;

use crate::phase::Phase;

/// Property names on the plain object delivered to a page.
pub mod js_key {
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

    pub const ALL: &[&str] = &[
        PHASE, STATUS, ELAPSED_MS, PHASE_MS, LOADED, TOTAL, PERCENT, RATE_BPS, ETA_MS, STALLED,
        STALLED_MS, REASON, DETAIL, TEXT,
    ];

    /// The documented browser event shape, in Rust field order.
    #[cfg(test)]
    pub const FIELD_COUNT: usize = 14;

    #[cfg(test)]
    pub const NAMED: [&str; super::EVENT_FIELD_COUNT] = [
        PHASE, STATUS, ELAPSED_MS, PHASE_MS, LOADED, TOTAL, PERCENT, RATE_BPS, ETA_MS, STALLED,
        STALLED_MS, REASON, DETAIL, TEXT,
    ];
}

/// Where in a phase an event sits.
#[derive(Boundary, serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[boundary(direction = "output")]
#[serde(rename_all = "snake_case")]
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
#[derive(Boundary, serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
#[boundary(direction = "output")]
#[serde(rename_all = "camelCase")]
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
    ///
    /// Precise because browser resource counts stay within the safe-integer range.
    #[boundary(
        omit_none,
        wide = "precise",
        bound = "0..=9,007,199,254,740,991 browser resource bytes"
    )]
    pub loaded: Option<u64>,
    /// The known decoded length, when there is one.
    ///
    /// Precise because received and advertised lengths are recorded only within
    /// the safe-integer range; an advertised length beyond it is treated as unknown.
    #[boundary(
        omit_none,
        wide = "precise",
        bound = "0..=9,007,199,254,740,991 browser resource bytes"
    )]
    pub total: Option<u64>,
    /// Whole percent, when the decoded length is known.
    #[boundary(omit_none)]
    pub percent: Option<u32>,
    /// Mean bytes per second, once two chunks have arrived.
    #[boundary(omit_none)]
    pub rate_bps: Option<f64>,
    /// Milliseconds left at that rate, when the length is known.
    #[boundary(omit_none)]
    pub eta_ms: Option<f64>,
    /// Whether the download has gone quiet.
    pub stalled: bool,
    /// How long it has been quiet, while it is.
    pub stalled_ms: f64,
    /// A short machine-readable code on a failure.
    #[boundary(omit_none)]
    pub reason: Option<String>,
    /// The underlying message on a failure.
    #[boundary(omit_none)]
    pub detail: Option<String>,
    /// One line a page can print as it stands.
    pub text: String,
}

/// One value in the plain script object lowered from an event.
pub enum EventValue<'a> {
    /// A JavaScript string.
    Text(&'a str),
    /// A JavaScript number.
    Number(f64),
    /// A JavaScript boolean.
    Boolean(bool),
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

    /// Visits exactly the keys and values emitted into the browser object.
    pub fn visit_fields(&self, mut visit: impl FnMut(&str, EventValue<'_>)) {
        let values = [
            Some(EventValue::Text(self.phase.as_str())),
            Some(EventValue::Text(self.status.as_str())),
            Some(EventValue::Number(self.elapsed_ms)),
            Some(EventValue::Number(self.phase_ms)),
            self.loaded
                .map(|value| EventValue::Number(crate::progress::as_f64(value))),
            self.total
                .map(|value| EventValue::Number(crate::progress::as_f64(value))),
            self.percent.map(f64::from).map(EventValue::Number),
            self.rate_bps.map(EventValue::Number),
            self.eta_ms.map(EventValue::Number),
            Some(EventValue::Boolean(self.stalled)),
            Some(EventValue::Number(self.stalled_ms)),
            self.reason.as_deref().map(EventValue::Text),
            self.detail.as_deref().map(EventValue::Text),
            Some(EventValue::Text(&self.text)),
        ];
        for (key, value) in js_key::ALL.iter().copied().zip(values) {
            if let Some(value) = value {
                visit(key, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ember_boundary::Boundary;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use serde_json::Value;

    use super::{EVENT_FIELD_COUNT, Event, EventValue, Status, js_key};
    use crate::phase::Phase;

    #[test]
    fn the_browser_event_shape_has_one_camel_case_key_per_rust_field() {
        let unique = js_key::ALL.iter().collect::<BTreeSet<_>>();
        assert_eq!(js_key::FIELD_COUNT, EVENT_FIELD_COUNT);
        assert_eq!(unique.len(), EVENT_FIELD_COUNT);
        assert!(unique.iter().all(|key| !key.contains('_')));
        assert_eq!(js_key::NAMED, js_key::ALL);
        assert_eq!(js_key::ALL, <Event as Boundary>::OBJECT_KEYS);
    }

    #[test]
    fn native_event_lowering_matches_the_output_descriptor() {
        let mut event = Event::new(Phase::Download, Status::Progress, 12.25, 3.5);
        event.loaded = Some(9);
        event.total = Some(19);
        event.percent = Some(47);
        event.rate_bps = Some(101.5);
        event.eta_ms = Some(202.75);
        event.stalled = true;
        event.stalled_ms = 303.25;
        event.reason = Some("reason-code".into());
        event.detail = Some("detail-message".into());
        event.text = "loading-text".into();
        let mut object = serde_json::Map::new();
        event.visit_fields(|key, value| {
            let value = match value {
                EventValue::Text(value) => Value::String(value.into()),
                EventValue::Number(value) => serde_json::json!(value),
                EventValue::Boolean(value) => Value::Bool(value),
            };
            object.insert(key.into(), value);
        });
        let value = Value::Object(object);
        assert_eq!(
            value,
            serde_json::json!({
                "phase": "download",
                "status": "progress",
                "elapsedMs": 12.25,
                "phaseMs": 3.5,
                "loaded": 9.0,
                "total": 19.0,
                "percent": 47.0,
                "rateBps": 101.5,
                "etaMs": 202.75,
                "stalled": true,
                "stalledMs": 303.25,
                "reason": "reason-code",
                "detail": "detail-message",
                "text": "loading-text",
            })
        );
        let schema = ember_boundary::json_schema::<Event>(ember_boundary::View::Output);
        jsonschema::validator_for(&schema)
            .expect("schema compiles")
            .validate(&value)
            .expect("lowered event matches descriptor");
    }

    #[test]
    fn every_loader_boundary_type_is_enumerated() {
        assert_eq!(crate::boundary_descriptions().len(), 3);
    }

    #[test]
    fn loader_enum_wire_samples_cover_every_variant() {
        fn prove<T>(samples: &[&str])
        where
            T: Boundary + DeserializeOwned + Serialize,
        {
            assert_eq!(samples.len(), T::DESCRIPTION.variant_count());
            let schema = ember_boundary::json_schema::<T>(ember_boundary::View::Output);
            let schema = jsonschema::validator_for(&schema).expect("schema compiles");
            for sample in samples {
                let value: Value = serde_json::from_str(sample).expect("sample is JSON");
                schema.validate(&value).expect("sample matches descriptor");
                let decoded: T =
                    serde_json::from_value(value.clone()).expect("sample deserializes");
                assert_eq!(
                    serde_json::to_value(decoded).expect("sample serializes"),
                    value
                );
            }
        }
        prove::<Phase>(&[
            r#""boot""#,
            r#""host""#,
            r#""download""#,
            r#""compile""#,
            r#""init""#,
            r#""start""#,
            r#""ready""#,
        ]);
        prove::<Status>(&[r#""begin""#, r#""progress""#, r#""done""#, r#""fail""#]);
    }
}
