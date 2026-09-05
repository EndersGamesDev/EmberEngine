//! The pair of pixel lattices one warp plan maps between.
//!
//! The warp fragment reads a plan's rows as destination pixels in, source pixels out, and then
//! divides the source pixel by `textureDimensions(source_scene)` (`warp_shader.rs`, the
//! `source_pixel` and `source_uv` lines). Two extents therefore decide where a plan puts the
//! picture, and neither of them is in the rows: the destination lattice the fragment builds its
//! destination point from (`scene.grid.xy`), and the source texture's own extent, which is the
//! delivered extent of the scene drawn into it. Rows built from one of the two, or left at
//! identity by default, assert that the two are equal. Nothing in the presenter makes them equal:
//! a scene is submitted at the MAIN grid's extent while the pose it is planned against carries
//! whatever lattice was published to HOT, and the refinement ladder delivers reduced extents on
//! purpose.
//!
//! Naming both extents is what this type is for. A path that builds rows names a `LatticePair`
//! and asks it for the map; the mirror then asks the same pair whether those rows keep the
//! destination inside the source. That question has one answer per plan and it is checkable on
//! the CPU, which is the whole reason a frame at the wrong scale was invisible to every test
//! before: the coverage mirror measures scenes and the error ceiling measures warps in pixels of
//! displacement, and neither of them looks at scale.

/// The rows that map a destination lattice onto a source lattice of the same extent.
///
/// This is the map for one pair only — the equal one. It is not a default: on any other pair it
/// places the source picture at its own texel size in the centre of the destination and paints
/// clear around it. Ask a `LatticePair` for `covering_rows` instead of reaching for this.
#[must_use]
pub const fn identity_warp_rows() -> [[f32; 4]; 3] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]
}

/// The two pixel lattices one warp plan's rows map between.
///
/// `source` is the extent of the texture the plan samples, which is the delivered extent of the
/// scene drawn into it. `destination` is the lattice the fragment builds its destination point
/// from, which is the grid of the pose the plan was solved against. Equal extents are the
/// ordinary case and give the identity map; they are not the only case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatticePair {
    source: [u32; 2],
    destination: [u32; 2],
}

/// The destination points a covering plan is checked at: the four corners and the centre.
///
/// The corners are where a scale error shows first, because a map that is wrong by a ratio moves
/// them furthest. The centre is there because it is the one point a centred thumbnail leaves in
/// the right place, so a check made only at the centre would have passed the morph thumbnail; it
/// is kept to pin the opposite error, a map that has drifted off the middle of the source.
pub const COVERAGE_CHART_POINTS: [[f64; 2]; 5] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [-1.0, 1.0],
    [1.0, 1.0],
    [0.0, 0.0],
];

/// Half a source texel, the reach of the source picture beyond its outermost sample centre.
///
/// The source's outermost samples sit exactly on the half-extent and own the half pixel past it,
/// so a destination landing inside that footprint is still covered. It is the same reach the
/// planner's exposure test already allows in the source pose's pixels
/// (`planner.rs`, `RETAINED_TEXEL_REACH_PX`), and taking a different one here would refuse plans
/// the exposure test calls covering: a bounded pan that lands the frame edge exactly on the source
/// edge would go to clear rather than to the half texel it owns.
///
/// It also absorbs the f32 rounding the rows carry. The mirror reads the rounded rows, so a map
/// that covers the destination exactly in f64 lands a corner a few parts in ten million outside
/// it, which is orders below a half texel of any extent the ladder produces. Neither slack hides
/// a scale error: a wrong scale is off by a ratio, at least several per cent on every pairing.
pub const SOURCE_TEXEL_REACH_PX: f64 = 0.5;

impl LatticePair {
    /// Names both extents of a plan.
    ///
    /// A zero extent is not a lattice: it names no pixels, the covering map divides by it, and the
    /// fragment would sample a texture that cannot exist. Refusing it here is what lets every
    /// caller treat a pair as a pair of real lattices.
    #[must_use]
    pub const fn new(source: [u32; 2], destination: [u32; 2]) -> Option<Self> {
        if source[0] == 0 || source[1] == 0 || destination[0] == 0 || destination[1] == 0 {
            return None;
        }
        Some(Self {
            source,
            destination,
        })
    }

    /// The extent of the texture the plan samples.
    #[must_use]
    pub const fn source(self) -> [u32; 2] {
        self.source
    }

    /// The lattice the plan's destination point is built from.
    #[must_use]
    pub const fn destination(self) -> [u32; 2] {
        self.destination
    }

    /// The map that lays the whole source picture across the whole destination, row-major f64.
    ///
    /// This is the lattice-free identity — every destination point reads the source point at the
    /// same place in the picture — expressed in the pixel units the fragment works in. The two
    /// lattices cover the same field of view whatever their pixel counts, so the ratio between
    /// the extents is the whole of the map.
    ///
    /// The ratios are independent per axis, where a reprojection scales by the uniform width
    /// ratio on both axes (`math/src/warp.rs` lines 15 to 16). The difference is not cosmetic,
    /// because the two lattices are not proportional: a reduced extent rounds up per axis, so a
    /// 960 by 540 surface at the `PictureFast` divisor 8 delivers 120 by 68 rather than 120 by
    /// 67.5. Its 68 rows carry 544 destination pixels of ground onto a 540-pixel destination. The
    /// per-axis ratio lands the frame edge on the frame edge and absorbs the difference as a
    /// stretch of up to `(k-1)/2` destination pixels for divisor `k` — 3.5 px at the top of the
    /// `PictureFast` ladder, 2 px at this pairing — while the uniform width ratio would keep the
    /// picture unstretched and, in the opposite rounding case, put its edge past the source and
    /// expose that many pixels of clear at the frame edge. A covering plan takes the bounded
    /// stretch: it exists precisely to avoid replacing the picture with clear, and a few pixels
    /// of stretch is inaccurate where a band of clear at the edge reads as a disocclusion that no
    /// scene is coming to fill.
    #[must_use]
    pub fn covering_map(self) -> [f64; 9] {
        let scale = [
            f64::from(self.source[0]) / f64::from(self.destination[0]),
            f64::from(self.source[1]) / f64::from(self.destination[1]),
        ];
        [scale[0], 0.0, 0.0, 0.0, scale[1], 0.0, 0.0, 0.0, 1.0]
    }

    /// The covering map packed for upload, or `None` when it does not round to a usable map.
    ///
    /// A pair whose ratio underflows to zero in f32 would place the picture on a single point, so
    /// it is refused rather than uploaded: a clear plan asserts no geometry, while a picture at a
    /// scale the geometry does not have asserts geometry that does not exist.
    #[must_use]
    pub fn covering_rows(self) -> Option<[[f32; 4]; 3]> {
        let rows = crate::pack_homography_rows(self.covering_map())?;
        (rows[0][0] > 0.0 && rows[1][1] > 0.0).then_some(rows)
    }

    /// The CPU mirror of the warp fragment's source lookup at one chart point.
    ///
    /// The fragment builds its destination point from the chart corner and the destination
    /// lattice, applies the plan rows, and normalises the mapped source pixel by the source
    /// texture's dimensions. `None` is what the fragment paints as exterior sky: a non-finite
    /// result or a point behind the map's own denominator. A point outside the unit square is
    /// returned rather than hidden, because that is exactly what the coverage check is asking
    /// about — the fragment paints it clear.
    #[must_use]
    pub fn source_uv(self, rows: [[f32; 4]; 3], chart: [f64; 2]) -> Option<[f64; 2]> {
        let destination = [
            chart[0] * f64::from(self.destination[0]) * 0.5,
            chart[1] * f64::from(self.destination[1]) * 0.5,
        ];
        let mapped = rows.map(|row| {
            f64::from(row[0]).mul_add(
                destination[0],
                f64::from(row[1]).mul_add(destination[1], f64::from(row[2])),
            )
        });
        if !mapped.iter().all(|value| value.is_finite()) || mapped[2] <= 0.0 {
            return None;
        }
        let source_pixel = [mapped[0] / mapped[2], mapped[1] / mapped[2]];
        Some([
            source_pixel[0] / f64::from(self.source[0]) + 0.5,
            0.5 - source_pixel[1] / f64::from(self.source[1]),
        ])
    }

    /// Whether these rows keep every checked destination point inside the source picture.
    ///
    /// This is the scale invariant. A plan that claims to cover the destination and maps a
    /// destination corner outside the source is showing the picture at a scale the geometry does
    /// not have — a centred thumbnail with clear around it, or a magnified crop — and under the
    /// rendering rule a moving frame may be very inaccurate but never wrong. A plan that declares
    /// exposure is making the opposite claim and is not checked here; a plan that samples nothing
    /// asserts no geometry and is not checked either.
    #[must_use]
    pub fn covers_destination(self, rows: [[f32; 4]; 3]) -> bool {
        let slack = self.coverage_slack();
        COVERAGE_CHART_POINTS.into_iter().all(|chart| {
            self.source_uv(rows, chart).is_some_and(|uv| {
                uv.into_iter()
                    .zip(slack)
                    .all(|(value, slack)| (-slack..=1.0 + slack).contains(&value))
            })
        })
    }

    /// The half-texel reach expressed in the normalised units the coverage check works in.
    #[must_use]
    pub fn coverage_slack(self) -> [f64; 2] {
        self.source
            .map(|extent| SOURCE_TEXEL_REACH_PX / f64::from(extent))
    }
}

/// Composes two row-major 3-by-3 maps, applying `inner` first and then `outer`.
///
/// A plan solved in one pixel lattice is carried to another by composing it with that pair's
/// covering map, which is the only way a solved map and an extent change meet without either of
/// them being restated in the other's terms.
#[must_use]
pub fn compose_homography(outer: [f64; 9], inner: [f64; 9]) -> [f64; 9] {
    core::array::from_fn(|index| {
        let (row, column) = (index / 3, index % 3);
        (0..3).fold(0.0, |sum, term| {
            outer[row * 3 + term].mul_add(inner[term * 3 + column], sum)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURFACE: [u32; 2] = [960, 540];
    const PICTURE_FAST: [u32; 2] = [120, 68];

    #[test]
    fn a_zero_extent_is_not_a_lattice() {
        assert_eq!(LatticePair::new([0, 68], SURFACE), None);
        assert_eq!(LatticePair::new(PICTURE_FAST, [960, 0]), None);
        assert!(LatticePair::new(PICTURE_FAST, SURFACE).is_some());
    }

    #[test]
    fn equal_extents_cover_by_identity() {
        let pair = LatticePair::new(SURFACE, SURFACE).expect("a real lattice pair");
        assert_eq!(pair.covering_rows(), Some(crate::identity_warp_rows()));
        assert!(pair.covers_destination(crate::identity_warp_rows()));
    }

    #[test]
    fn a_covering_map_lands_the_frame_edge_on_the_frame_edge_in_both_directions() {
        for (source, destination) in [(PICTURE_FAST, SURFACE), (SURFACE, PICTURE_FAST)] {
            let pair = LatticePair::new(source, destination).expect("a real lattice pair");
            let rows = pair.covering_rows().expect("a usable covering map");
            assert!(
                pair.covers_destination(rows),
                "a covering map on {source:?} to {destination:?} leaves the source"
            );
            let corner = pair
                .source_uv(rows, [1.0, 1.0])
                .expect("the destination corner is in front of the source");
            let slack = pair.coverage_slack();
            assert!(
                (corner[0] - 1.0).abs() < slack[0] && corner[1].abs() < slack[1],
                "the destination corner samples {corner:?} rather than the source corner"
            );
        }
    }

    #[test]
    fn identity_rows_on_unequal_extents_do_not_cover_the_destination() {
        let pair = LatticePair::new(PICTURE_FAST, SURFACE).expect("a real lattice pair");
        assert!(!pair.covers_destination(crate::identity_warp_rows()));
        let corner = pair
            .source_uv(crate::identity_warp_rows(), [1.0, 1.0])
            .expect("the destination corner is in front of the source");
        assert!(
            corner[0] > 4.0,
            "identity on a reduced source samples {corner:?}: the picture sits in the centre at \
             its own texel size"
        );
    }

    #[test]
    fn composition_carries_a_solved_map_between_lattices() {
        let solved = [2.0, 0.0, 3.0, 0.0, 2.0, -1.0, 0.0, 0.0, 1.0];
        let delivery = LatticePair::new(PICTURE_FAST, SURFACE).expect("a real lattice pair");
        let composed = compose_homography(delivery.covering_map(), solved);
        let ratio = f64::from(PICTURE_FAST[0]) / f64::from(SURFACE[0]);
        assert!(2.0_f64.mul_add(-ratio, composed[0]).abs() < 1.0e-12);
        assert!(3.0_f64.mul_add(-ratio, composed[2]).abs() < 1.0e-12);
        assert_eq!(composed[8], 1.0);
        let identity = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        assert_eq!(compose_homography(identity, solved), solved);
    }

    /// A bounded pan that lands the frame edge exactly on the source edge still covers.
    ///
    /// This is the pairing between the coverage check and the exposure test. The planner calls a
    /// warp unexposed while every destination sample stays inside the source's half-extent plus
    /// half a texel; a coverage check with a tighter slack would send exactly those plans to
    /// clear, which is the disocclusion the reach exists to prevent, read off the other side.
    #[test]
    fn the_half_texel_reach_is_covered_rather_than_refused() {
        for extent in [SURFACE, PICTURE_FAST] {
            let pair = LatticePair::new(extent, extent).expect("a real lattice pair");
            let mut rows = identity_warp_rows();
            rows[0][2] = 0.5;
            let corner = pair
                .source_uv(rows, [1.0, 1.0])
                .expect("the destination corner is in front of the source");
            assert!(
                corner[0] > 1.0,
                "the pan puts the corner past the source edge"
            );
            assert!(
                pair.covers_destination(rows),
                "a half-texel pan on {extent:?} is refused where the exposure test calls it covering"
            );
            rows[0][2] = 2.0;
            assert!(
                !pair.covers_destination(rows),
                "a two-pixel pan on {extent:?} reaches past the half texel and is not covering"
            );
        }
    }
}
