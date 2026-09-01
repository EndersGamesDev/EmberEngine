//! The racing line: a closed Catmull-Rom loop, its arc-length table, and the
//! queries the simulation needs against it.
//!
//! Everything here is deterministic and allocation-free after `Track::new`.
//! The client and the authoritative server both run it, so a divergence here
//! is a desync — the sampling resolution is a constant, never a tunable.

use glam::Vec2;

/// Centreline samples per control-point span.
///
/// The arc-length table and every closest-point query walk this polyline, so
/// it fixes both accuracy and the cost of `Track::locate`. Shared by both
/// peers: do not make it a parameter.
pub const SAMPLES_PER_SPAN: usize = 24;

/// Where a car is relative to the racing line.
#[derive(Clone, Copy, Debug)]
pub struct Locate {
    /// Distance along the centreline from the start line, in metres.
    pub s: f32,
    /// Signed offset from the centreline: positive to the left of the
    /// direction of travel, negative to the right.
    pub lateral: f32,
    /// Unit tangent of the centreline at `s`.
    pub tangent: Vec2,
    /// Index of the polyline segment the car projected onto.
    pub segment: usize,
}

pub struct Track {
    /// Control points of the closed loop, in order.
    control: Vec<Vec2>,
    /// Densely sampled centreline. `points[0]` is the start line and the loop
    /// is implicit — the last point connects back to the first.
    points: Vec<Vec2>,
    /// Cumulative arc length; `cumulative[i]` is the distance from the start
    /// line to `points[i]`. Has `points.len() + 1` entries, the last being the
    /// full lap length.
    cumulative: Vec<f32>,
    half_width: f32,
}

/// Uniform Catmull-Rom, the standard centripetal-free form. Interpolates p1
/// at t=0 and p2 at t=1, and is C1 continuous across spans — which is what
/// makes a closed loop join without a visible kink, provided the wrap-around
/// neighbours are used at the seam (see `Track::new`).
fn catmull_rom(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

impl Track {
    /// Build from closed-loop control points. The caller supplies each point
    /// exactly once; the wrap-around is handled here, so passing a duplicated
    /// first/last point would put a zero-length span in the loop.
    ///
    /// # Panics
    ///
    /// Panics if fewer than four control points are supplied.
    #[must_use]
    pub fn new(control: Vec<Vec2>, half_width: f32) -> Self {
        assert!(control.len() >= 4, "a closed Catmull-Rom loop needs >= 4 control points");
        let n = control.len();
        let mut points = Vec::with_capacity(n * SAMPLES_PER_SPAN);
        for i in 0..n {
            // Indices wrap, which is the whole reason the seam is smooth: the
            // span ending at control[0] sees control[n-1] as its p0.
            let p0 = control[(i + n - 1) % n];
            let p1 = control[i];
            let p2 = control[(i + 1) % n];
            let p3 = control[(i + 2) % n];
            for k in 0..SAMPLES_PER_SPAN {
                // Both operands are tiny exact integers; preserve the shared simulation expression.
                #[allow(clippy::cast_precision_loss)]
                let t = k as f32 / SAMPLES_PER_SPAN as f32;
                points.push(catmull_rom(p0, p1, p2, p3, t));
            }
        }

        let mut cumulative = Vec::with_capacity(points.len() + 1);
        cumulative.push(0.0);
        let mut acc = 0.0;
        for i in 0..points.len() {
            acc += (points[(i + 1) % points.len()] - points[i]).length();
            cumulative.push(acc);
        }

        Self { control, points, cumulative, half_width }
    }

    #[must_use]
    pub fn control_points(&self) -> &[Vec2] {
        &self.control
    }

    #[must_use]
    pub fn centreline(&self) -> &[Vec2] {
        &self.points
    }

    #[must_use]
    pub const fn half_width(&self) -> f32 {
        self.half_width
    }

    /// Total lap distance in metres.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.cumulative[self.points.len()]
    }

    /// Centreline point and unit tangent at arc length `s` (wrapped).
    ///
    /// # Panics
    ///
    /// Panics if the requested distance is non-finite, or if the track has
    /// zero or non-finite length due to degenerate or non-finite points.
    #[must_use]
    pub fn at(&self, distance: f32) -> (Vec2, Vec2) {
        let len = self.length();
        let distance = distance.rem_euclid(len);
        // The table is monotonic, so a binary search lands in the right span.
        let index = match self
            .cumulative
            .binary_search_by(|candidate| candidate.partial_cmp(&distance).unwrap())
        {
            Ok(index) => index.min(self.points.len() - 1),
            Err(index) => index.saturating_sub(1).min(self.points.len() - 1),
        };
        let start = self.points[index];
        let end = self.points[(index + 1) % self.points.len()];
        let segment_length = self.cumulative[index + 1] - self.cumulative[index];
        let fraction = if segment_length > 1e-6 {
            (distance - self.cumulative[index]) / segment_length
        } else {
            0.0
        };
        let tangent = (end - start).normalize_or_zero();
        (start + (end - start) * fraction, tangent)
    }

    /// Project a world position onto the centreline.
    ///
    /// Brute force over the polyline: with a few hundred samples this is far
    /// cheaper than the spatial index it would take to beat it, and — more to
    /// the point — it is exactly reproducible on both peers.
    #[must_use]
    pub fn locate(&self, position: Vec2) -> Locate {
        let point_count = self.points.len();
        let (mut best_distance_squared, mut best_index, mut best_fraction) =
            (f32::INFINITY, 0usize, 0.0f32);
        for index in 0..point_count {
            let start = self.points[index];
            let end = self.points[(index + 1) % point_count];
            let segment = end - start;
            let length_squared = segment.length_squared();
            let fraction = if length_squared > 1e-9 {
                ((position - start).dot(segment) / length_squared).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let distance_squared =
                (position - (start + segment * fraction)).length_squared();
            if distance_squared < best_distance_squared {
                best_distance_squared = distance_squared;
                best_index = index;
                best_fraction = fraction;
            }
        }
        let start = self.points[best_index];
        let end = self.points[(best_index + 1) % point_count];
        let tangent = (end - start).normalize_or_zero();
        let segment_length = self.cumulative[best_index + 1] - self.cumulative[best_index];
        let s = self.cumulative[best_index] + segment_length * best_fraction;
        // 2D cross product: positive when position lies left of the tangent.
        let rel = position - (start + (end - start) * best_fraction);
        let lateral = tangent.x * rel.y - tangent.y * rel.x;
        Locate { s, lateral, tangent, segment: best_index }
    }

    /// True when the car is off the racing surface.
    #[must_use]
    pub fn off_track(&self, p: Vec2) -> bool {
        self.locate(p).lateral.abs() > self.half_width
    }

    /// Smallest radius of curvature anywhere on the centreline, in metres.
    ///
    /// Measured as the circumradius of three consecutive samples. A corner
    /// tighter than the car's minimum turning radius is not a hard corner, it
    /// is an impossible one — the car cannot follow it at any speed, and the
    /// track becomes a wall.
    #[must_use]
    pub fn min_curvature_radius(&self) -> f32 {
        let n = self.points.len();
        let mut best = f32::INFINITY;
        for i in 0..n {
            let a = self.points[(i + n - 1) % n];
            let b = self.points[i];
            let c = self.points[(i + 1) % n];
            let (ab, bc, ca) = ((b - a).length(), (c - b).length(), (a - c).length());
            // Twice the signed triangle area; zero for collinear samples,
            // which is a straight and therefore infinite radius.
            let cross = (b - a).x * (c - a).y - (b - a).y * (c - a).x;
            let area2 = cross.abs();
            if area2 < 1e-6 {
                continue;
            }
            best = best.min(ab * bc * ca / (2.0 * area2));
        }
        best
    }

    /// First pair of non-adjacent centreline segments that cross, if any.
    ///
    /// A self-intersecting centreline makes `locate` ambiguous — a car at the
    /// crossing projects onto whichever branch happens to be nearer, so lap
    /// progress can jump backwards by half a lap.
    #[must_use]
    pub fn self_intersection(&self) -> Option<(usize, usize)> {
        let n = self.points.len();
        let seg = |i: usize| (self.points[i], self.points[(i + 1) % n]);
        let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).x * (b - o).y - (a - o).y * (b - o).x;
        for i in 0..n {
            // Skip immediate neighbours: they share an endpoint by design.
            for j in (i + 2)..n {
                if i == 0 && j == n - 1 {
                    continue;
                }
                let ((p1, p2), (p3, p4)) = (seg(i), seg(j));
                let (d1, d2) = (cross(p3, p4, p1), cross(p3, p4, p2));
                let (d3, d4) = (cross(p1, p2, p3), cross(p1, p2, p4));
                if ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0)) {
                    return Some((i, j));
                }
            }
        }
        None
    }

    /// Smallest signed difference `a - b` on the loop, in metres, in
    /// (-len/2, len/2]. Used to ask "did progress move forward or backward"
    /// without the start line making every lap look like a huge jump back.
    #[must_use]
    pub fn delta_s(&self, a: f32, b: f32) -> f32 {
        let len = self.length();
        let mut d = (a - b).rem_euclid(len);
        if d > len * 0.5 {
            d -= len;
        }
        d
    }
}

/// Lap and checkpoint progression for one car.
///
/// The rule that makes a lap count exactly once: the loop is divided into
/// `n` equal checkpoint sectors, and a car must pass through **every** sector
/// in ascending order before crossing the line. Driving back and forth over
/// the start line cannot farm laps, because leaving sector 0 backwards does
/// not arm sector n-1.
#[derive(Clone, Copy, Debug)]
pub struct LapTracker {
    /// The next sector index this car must reach.
    pub next_sector: u16,
    pub sectors: u16,
    pub lap: u32,
    /// Arc length at the previous tick, for direction of travel.
    pub last_s: f32,
    /// Total distance travelled forward along the line — the race ordering
    /// key, monotonic across laps so it never ties at the start line.
    pub progress: f32,
}

impl LapTracker {
    /// # Panics
    ///
    /// Panics if fewer than two checkpoint sectors are requested.
    #[must_use]
    pub fn new(sectors: u16, start_s: f32) -> Self {
        assert!(sectors >= 2, "need at least two sectors or a lap cannot be gated");
        Self { next_sector: 1, sectors, lap: 0, last_s: start_s, progress: 0.0 }
    }

    /// Feed the car's current arc length. Returns true on the tick a lap
    /// completes.
    pub fn update(&mut self, track: &Track, s: f32) -> bool {
        let len = track.length();
        self.progress += track.delta_s(s, self.last_s);
        self.last_s = s;

        // `Track::locate` bounds the value to one lap; preserve the shared simulation casts.
        #[allow(
            clippy::cast_lossless,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )]
        let sector = ((s / len) * self.sectors as f32).floor() as u16 % self.sectors;
        let mut completed = false;
        if sector == self.next_sector {
            // Reached the sector we were waiting for: arm the next one.
            self.next_sector = (self.next_sector + 1) % self.sectors;
            if self.next_sector == 1 {
                // We just armed sector 1 again, i.e. we came through 0 having
                // visited every other sector in order. That is a lap.
                self.lap += 1;
                completed = true;
            }
        }
        completed
    }
}

#[cfg(test)]
// Test loop counters are small exact integers; casts keep the formulas legible.
#[allow(clippy::cast_precision_loss)]
mod tests {
    use super::*;

    fn oval() -> Track {
        Track::new(
            vec![
                Vec2::new(60.0, 0.0),
                Vec2::new(0.0, 40.0),
                Vec2::new(-60.0, 0.0),
                Vec2::new(0.0, -40.0),
            ],
            8.0,
        )
    }

    /// The seam is the classic Catmull-Rom bug: build a closed loop without
    /// wrapping the neighbour indices and it joins with a visible corner.
    ///
    /// Note what this does *not* assert. `at` reports the tangent of the
    /// sampled polyline, so around a tight control point two adjacent chords
    /// genuinely differ by ~13 degrees — that is discretisation, not a kink,
    /// and asserting the tangents are parallel across the seam only measures
    /// the sample spacing. The real property is that the seam is not special:
    /// the turn at the start line must match the turn at every other control
    /// point, because on this loop all four are congruent.
    #[test]
    fn the_seam_is_not_special() {
        // A square, not the oval: on a 120x80 diamond the long-axis corners
        // are genuinely sharper than the short-axis ones, so "all control
        // points turn equally" is only true for a shape with four-fold
        // symmetry. Using the oval here would test the shape, not the seam.
        let t = Track::new(
            vec![
                Vec2::new(50.0, 50.0),
                Vec2::new(-50.0, 50.0),
                Vec2::new(-50.0, -50.0),
                Vec2::new(50.0, -50.0),
            ],
            8.0,
        );
        let pts = t.centreline();
        let n = pts.len();
        let turn_at = |i: usize| {
            let a = (pts[i] - pts[(i + n - 1) % n]).normalize();
            let b = (pts[(i + 1) % n] - pts[i]).normalize();
            a.dot(b).clamp(-1.0, 1.0).acos()
        };
        // Index 0 is the seam; the other control points sit at multiples of
        // SAMPLES_PER_SPAN. A broken wrap shows up as a spike at 0 only.
        let seam = turn_at(0);
        for k in 1..4 {
            let other = turn_at(k * SAMPLES_PER_SPAN);
            assert!(
                (seam - other).abs() < 1e-3,
                "seam turn {seam} rad differs from control point {k} turn {other} rad"
            );
        }
        // Position must also be continuous across the wrap.
        let len = t.length();
        let (pa, _) = t.at(len - 0.001);
        let (pb, _) = t.at(0.001);
        assert!((pa - pb).length() < 0.05, "gap at the seam: {} m", (pa - pb).length());
    }

    /// The arc-length table must agree with `at`, or every distance-based
    /// query (checkpoints, AI targets, boost zones) drifts around the lap.
    #[test]
    fn arc_length_is_consistent() {
        let t = oval();
        let len = t.length();
        let mut walked = 0.0;
        let steps = 2000;
        let mut prev = t.at(0.0).0;
        for i in 1..=steps {
            let p = t.at(len * i as f32 / steps as f32).0;
            walked += (p - prev).length();
            prev = p;
        }
        let err = (walked - len).abs() / len;
        assert!(err < 0.01, "arc length off by {:.3}% ({walked} vs {len})", err * 100.0);
    }

    /// A point on the centreline must locate with ~zero lateral offset, and
    /// the sign convention must actually be left-positive.
    #[test]
    fn locate_projects_and_signs_correctly() {
        let t = oval();
        for i in 0..64 {
            let s = t.length() * i as f32 / 64.0;
            let (p, tan) = t.at(s);
            let loc = t.locate(p);
            assert!(loc.lateral.abs() < 0.15, "on-centreline point had lateral {}", loc.lateral);
            assert!(t.delta_s(loc.s, s).abs() < 1.0, "s mismatch: {} vs {s}", loc.s);
            // Step to the left of travel: +90 degrees is (-y, x).
            let left = Vec2::new(-tan.y, tan.x);
            let off = t.locate(p + left * 2.0);
            assert!(off.lateral > 1.0, "left of the line should be positive, got {}", off.lateral);
        }
    }

    #[test]
    fn off_track_respects_half_width() {
        let t = oval();
        let (p, tan) = t.at(10.0);
        let left = Vec2::new(-tan.y, tan.x);
        assert!(!t.off_track(p));
        assert!(!t.off_track(p + left * 7.0));
        assert!(t.off_track(p + left * 9.0));
        assert!(t.off_track(p - left * 9.0));
    }

    /// The bug the old code shipped: a lap counted on every frame the car was
    /// near the line. One clean lap must produce exactly one increment.
    #[test]
    fn one_lap_counts_once() {
        let t = oval();
        let mut lt = LapTracker::new(8, 0.0);
        let len = t.length();
        let mut laps = 0;
        // Three laps, sampled finely enough that the car is "at the line" for
        // many consecutive ticks — exactly the situation that broke before.
        for i in 1..=3000 {
            let s = (len * 3.0) * i as f32 / 3000.0;
            if lt.update(&t, s % len) {
                laps += 1;
            }
        }
        assert_eq!(laps, 3, "expected 3 laps, got {laps}");
        assert_eq!(lt.lap, 3);
    }

    /// Reversing over the start line must not farm laps.
    #[test]
    fn back_and_forth_over_the_line_farms_nothing() {
        let t = oval();
        let len = t.length();
        let mut lt = LapTracker::new(8, 0.0);
        for _ in 0..200 {
            // Creep forward past the line, then back behind it, repeatedly.
            for k in 0..20 {
                lt.update(&t, (k as f32 * 0.5).rem_euclid(len));
            }
            for k in (0..20).rev() {
                lt.update(&t, (len - 5.0 + k as f32 * 0.1).rem_euclid(len));
            }
        }
        assert_eq!(lt.lap, 0, "reversing over the line minted {} laps", lt.lap);
    }

    /// Progress must be monotonic for a forward-driving car and must not
    /// reset at the start line — it is the race ordering key.
    #[test]
    fn progress_is_monotonic_across_laps() {
        let t = oval();
        let len = t.length();
        let mut lt = LapTracker::new(8, 0.0);
        let mut prev = lt.progress;
        for i in 1..=2000 {
            let s = (len * 2.5) * i as f32 / 2000.0;
            lt.update(&t, s % len);
            assert!(lt.progress >= prev - 1e-3, "progress went backwards at s={s}");
            prev = lt.progress;
        }
        assert!((lt.progress - len * 2.5).abs() < len * 0.02, "progress {} vs {}", lt.progress, len * 2.5);
    }
}
