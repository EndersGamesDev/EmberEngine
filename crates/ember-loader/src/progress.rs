//! Download arithmetic: what a page may honestly say about bytes in flight.
//!
//! Every value here is an `Option` for the same reason: a browser is not
//! obliged to tell the page how big the decoded answer is. The deploy records
//! that size for live bundles; otherwise an encoded or chunked response has a
//! byte count and no percentage. Inventing one from a transfer length in
//! different units is worse than showing none, because a bar that reaches 90%
//! and stays there reads as a hang rather than as a download nobody measured.

/// One download, as far as it has got.
#[derive(Debug, Clone, Copy, Default)]
pub struct Progress {
    /// Bytes handed to the page so far.
    pub loaded: u64,
    /// The known decoded length, from the catalog or an unencoded response.
    pub total: Option<u64>,
    /// How many chunks have arrived. A rate needs two.
    pub samples: u32,
    /// When the first chunk arrived.
    pub first_ms: Option<f64>,
    /// When the last chunk arrived, or when the download began.
    pub last_ms: f64,
}

/// `u64` bytes as `f64` for the rate arithmetic.
///
/// The cast is exact below 2^53 bytes, which is nine petabytes: every wasm
/// bundle this repository can build is off that scale by seven orders of
/// magnitude, and a rate is a display value in any case.
#[expect(
    clippy::cast_precision_loss,
    reason = "byte counts are far below the exactly representable range"
)]
#[must_use]
pub const fn as_f64(bytes: u64) -> f64 {
    bytes as f64
}

/// A rate, as whole bytes, for display only.
///
/// Saturating on both ends: a negative or non-finite rate cannot be produced
/// by the arithmetic above, and if one ever were, a zero is a wrong number a
/// reader can see through rather than a panic in a loading screen.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a display rate is rounded down and clamped at zero"
)]
#[must_use]
pub const fn whole_bytes(rate: f64) -> u64 {
    if rate.is_finite() && rate > 0.0 {
        rate.trunc() as u64
    } else {
        0
    }
}

impl Progress {
    /// Start a download whose clock runs from `now_ms`.
    #[must_use]
    pub const fn new(now_ms: f64) -> Self {
        Self {
            loaded: 0,
            total: None,
            samples: 0,
            first_ms: None,
            last_ms: now_ms,
        }
    }

    /// Record one chunk.
    pub const fn chunk(&mut self, now_ms: f64, bytes: u64) {
        self.loaded = self.loaded.saturating_add(bytes);
        if matches!(self.total, Some(total) if self.loaded > total) {
            self.total = None;
        }
        self.samples = self.samples.saturating_add(1);
        if self.first_ms.is_none() {
            self.first_ms = Some(now_ms);
        }
        self.last_ms = now_ms;
    }

    /// Whole percent, or `None` when no decoded length is known.
    ///
    /// The known total is discarded as soon as the decoded body outruns
    /// it, because keeping a smaller total would make every later percentage
    /// and estimate false.
    #[must_use]
    pub fn percent(&self) -> Option<u32> {
        let total = self.total.filter(|t| *t > 0)?;
        let done = self.loaded.min(total);
        let percent = u128::from(done) * 100 / u128::from(total);
        u32::try_from(percent).ok().map(|p| p.min(100))
    }

    /// Bytes per second across the whole download so far.
    ///
    /// Averaged from the first chunk rather than from the last pair: a
    /// per-chunk rate on a fast link swings by an order of magnitude between
    /// frames, and the number exists to answer "how long will this take",
    /// which a mean answers and an instantaneous sample does not.
    #[must_use]
    pub fn rate_bps(&self) -> Option<f64> {
        if self.samples < 2 {
            return None;
        }
        let first = self.first_ms?;
        let span = self.last_ms - first;
        if span <= 0.0 {
            return None;
        }
        Some(as_f64(self.loaded) / span * 1000.0)
    }

    /// Milliseconds left at the current rate, when both the rate and the
    /// total are known.
    #[must_use]
    pub fn eta_ms(&self) -> Option<f64> {
        let total = self.total.filter(|t| *t > self.loaded)?;
        let rate = self.rate_bps().filter(|r| *r > 0.0)?;
        Some(as_f64(total - self.loaded) / rate * 1000.0)
    }

    /// Whether nothing has arrived for `stall_ms`.
    ///
    /// Measured from the last chunk, or from the start when none has
    /// arrived: a request that never produces a first byte is exactly the
    /// case a stall notice exists for.
    #[must_use]
    pub fn stalled(&self, now_ms: f64, stall_ms: f64) -> bool {
        stall_ms > 0.0 && now_ms - self.last_ms >= stall_ms
    }
}

#[cfg(test)]
mod tests {
    use super::Progress;

    fn downloaded(total: Option<u64>, chunks: &[(f64, u64)]) -> Progress {
        let mut p = Progress::new(0.0);
        p.total = total;
        for (at, bytes) in chunks {
            p.chunk(*at, *bytes);
        }
        p
    }

    #[test]
    fn percent_is_none_without_a_declared_length() {
        let p = downloaded(None, &[(10.0, 1_000)]);
        assert_eq!(p.percent(), None);
        assert_eq!(p.loaded, 1_000);
    }

    #[test]
    fn percent_floors() {
        assert_eq!(downloaded(Some(300), &[(1.0, 0)]).percent(), Some(0));
        assert_eq!(downloaded(Some(300), &[(1.0, 1)]).percent(), Some(0));
        assert_eq!(downloaded(Some(300), &[(1.0, 100)]).percent(), Some(33));
        assert_eq!(downloaded(Some(300), &[(1.0, 300)]).percent(), Some(100));
    }

    #[test]
    fn a_body_larger_than_its_declared_length_discards_the_total() {
        let p = downloaded(Some(300), &[(1.0, 301)]);
        assert_eq!(p.loaded, 301);
        assert_eq!(p.total, None);
        assert_eq!(p.percent(), None);
        assert_eq!(p.eta_ms(), None);
    }

    #[test]
    fn percent_does_not_overflow_on_large_byte_counts() {
        assert_eq!(
            downloaded(Some(u64::MAX), &[(1.0, u64::MAX)]).percent(),
            Some(100)
        );
    }

    #[test]
    fn a_zero_length_response_has_no_percentage() {
        assert_eq!(downloaded(Some(0), &[(1.0, 0)]).percent(), None);
    }

    #[test]
    fn one_sample_gives_no_rate_and_no_eta() {
        let p = downloaded(Some(1_000), &[(100.0, 100)]);
        assert_eq!(p.rate_bps(), None);
        assert_eq!(p.eta_ms(), None);
    }

    #[test]
    fn the_rate_is_the_mean_since_the_first_chunk() {
        // 900 bytes over the 1000 ms between the first and last chunk.
        let p = downloaded(Some(2_000), &[(1_000.0, 100), (2_000.0, 800)]);
        assert_eq!(p.rate_bps(), Some(900.0));
        // 1100 bytes left at 900 B/s.
        let eta = p.eta_ms().expect("both the rate and the total are known");
        assert!((eta - 1_222.22).abs() < 0.01, "eta was {eta}");
    }

    #[test]
    fn eta_needs_a_total_and_stops_at_the_last_byte() {
        assert_eq!(downloaded(None, &[(1.0, 10), (2.0, 10)]).eta_ms(), None);
        assert_eq!(
            downloaded(Some(20), &[(1.0, 10), (1_001.0, 10)]).eta_ms(),
            None
        );
    }

    #[test]
    fn a_stall_is_measured_from_the_last_chunk() {
        let p = downloaded(Some(1_000), &[(1_000.0, 100)]);
        assert!(!p.stalled(5_999.0, 5_000.0));
        assert!(p.stalled(6_000.0, 5_000.0));
    }

    #[test]
    fn a_request_with_no_first_byte_stalls_from_its_start() {
        let p = Progress::new(1_000.0);
        assert!(!p.stalled(5_999.0, 5_000.0));
        assert!(p.stalled(6_000.0, 5_000.0));
    }
}
