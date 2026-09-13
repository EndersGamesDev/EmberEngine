//! One short line per event, so every page says the same thing.
//!
//! The wording lives here rather than in each page for the reason the host
//! chip's wording lives in `web/hosts.js`: six pages that each phrase a stall
//! their own way are six places to fix when the phrasing is wrong, and five
//! of them are found by a player rather than by a reader.

use crate::event::{Event, Status};
use crate::phase::Phase;
use crate::progress::{as_f64, whole_bytes};

/// A byte count a player can read.
///
/// Decimal units, because that is what a browser's own network panel shows
/// next to it.
#[must_use]
pub fn bytes_label(bytes: u64) -> String {
    let value = as_f64(bytes);
    if bytes < 1_000 {
        format!("{bytes} B")
    } else if bytes < 1_000_000 {
        format!("{:.1} kB", value / 1_000.0)
    } else if bytes < 1_000_000_000 {
        format!("{:.1} MB", value / 1_000_000.0)
    } else {
        format!("{:.1} GB", value / 1_000_000_000.0)
    }
}

/// A duration a player can read.
///
/// Sub-ten-second spans keep a decimal because that is the range where the
/// difference between 1 s and 9 s is the whole news; past a minute the
/// seconds are padded so the line stops jumping.
#[must_use]
pub fn secs_label(ms: f64) -> String {
    let ms = if ms.is_finite() && ms > 0.0 { ms } else { 0.0 };
    let secs = ms / 1000.0;
    if secs < 10.0 {
        format!("{secs:.1} s")
    } else {
        let whole = secs.round();
        if whole < 60.0 {
            return format!("{whole:.0} s");
        }
        let minutes = (whole / 60.0).trunc();
        let rest = f64::mul_add(minutes, -60.0, whole);
        format!("{minutes:.0} m {rest:02.0} s")
    }
}

fn download_line(ev: &Event) -> String {
    let mut parts = vec![bytes_label(ev.loaded.unwrap_or(0))];
    if let Some(total) = ev.total {
        parts[0] = format!("{} of {}", parts[0], bytes_label(total));
    }
    if let Some(percent) = ev.percent {
        parts.push(format!("{percent}%"));
    }
    if ev.stalled {
        parts.push(format!("no data for {}", secs_label(ev.stalled_ms)));
    } else {
        if let Some(rate) = ev.rate_bps {
            parts.push(format!("{}/s", bytes_label(whole_bytes(rate))));
        }
        if let Some(eta) = ev.eta_ms {
            parts.push(format!("{} left", secs_label(eta)));
        }
    }
    format!("downloading {}", parts.join(" · "))
}

/// The line for one event.
#[must_use]
pub fn render(ev: &Event) -> String {
    if ev.status == Status::Fail {
        let why = ev
            .detail
            .as_deref()
            .filter(|d| !d.is_empty())
            .or(ev.reason.as_deref())
            .unwrap_or("no reason given");
        return match ev.phase {
            Phase::Boot => format!("the loader could not start: {why}"),
            Phase::Host => format!("no server: {why}"),
            Phase::Download => format!("the download failed: {why}"),
            Phase::Compile => format!("the game could not be compiled: {why}"),
            Phase::Init => format!("the engine could not start: {why}"),
            Phase::Start => format!("the game could not start: {why}"),
            Phase::Ready => format!("the load failed: {why}"),
        };
    }
    match (ev.phase, ev.status) {
        (Phase::Boot, _) => "starting".to_owned(),
        (Phase::Host, Status::Begin) => "looking for a server".to_owned(),
        (Phase::Host, _) => "server ready".to_owned(),
        (Phase::Download, Status::Begin) => "downloading the game".to_owned(),
        (Phase::Download, Status::Progress) => download_line(ev),
        (Phase::Download, _) => format!(
            "downloaded {} in {}",
            bytes_label(ev.loaded.unwrap_or(0)),
            secs_label(ev.phase_ms)
        ),
        (Phase::Compile, Status::Begin) => "compiling the game".to_owned(),
        (Phase::Compile, _) => format!("compiled in {}", secs_label(ev.phase_ms)),
        (Phase::Init, Status::Begin) => "starting the engine".to_owned(),
        (Phase::Init, _) => format!("engine ready in {}", secs_label(ev.phase_ms)),
        (Phase::Start, Status::Begin) => "starting the game".to_owned(),
        (Phase::Start, _) => "game started".to_owned(),
        (Phase::Ready, _) => format!("ready in {}", secs_label(ev.elapsed_ms)),
    }
}

#[cfg(test)]
mod tests {
    use super::{bytes_label, secs_label};

    #[test]
    fn byte_labels_change_unit_at_the_decimal_boundary() {
        assert_eq!(bytes_label(0), "0 B");
        assert_eq!(bytes_label(999), "999 B");
        assert_eq!(bytes_label(1_000), "1.0 kB");
        assert_eq!(bytes_label(999_999), "1000.0 kB");
        assert_eq!(bytes_label(1_000_000), "1.0 MB");
        assert_eq!(bytes_label(38_100_000), "38.1 MB");
        assert_eq!(bytes_label(1_000_000_000), "1.0 GB");
    }

    #[test]
    fn duration_labels_keep_a_decimal_only_where_it_carries_news() {
        assert_eq!(secs_label(0.0), "0.0 s");
        assert_eq!(secs_label(-5.0), "0.0 s");
        assert_eq!(secs_label(940.0), "0.9 s");
        assert_eq!(secs_label(9_949.0), "9.9 s");
        assert_eq!(secs_label(12_400.0), "12 s");
        assert_eq!(secs_label(59_400.0), "59 s");
        assert_eq!(secs_label(59_950.0), "1 m 00 s");
        assert_eq!(secs_label(65_000.0), "1 m 05 s");
        assert_eq!(secs_label(3_725_000.0), "62 m 05 s");
    }
}
