use crate::{BigCentre, BigScalar, MathError, NavigationDelta, Plane, pixel_scale};

impl BigCentre {
    /// Applies drag and pointer-anchored zoom motion at the centre's Astro-float precision.
    ///
    /// Drag uses the after-zoom scale, so a combined edit first establishes the new zoom and then
    /// translates in pixels of that view.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite input, zero width, an unrepresentable scale, or Astro-float
    /// arithmetic failure.
    pub fn apply_navigation(
        &mut self,
        delta: &NavigationDelta,
        plane: &Plane,
        zoom_log2_before: f64,
        zoom_log2_after: f64,
        grid_width: u32,
    ) -> Result<(), MathError> {
        validate_navigation_input(delta, plane, zoom_log2_before, zoom_log2_after)?;
        let precision_bits = self.precision_bits;
        let scale_before =
            BigScalar::from_f64(pixel_scale(zoom_log2_before, grid_width)?, precision_bits)?;
        let scale_after =
            BigScalar::from_f64(pixel_scale(zoom_log2_after, grid_width)?, precision_bits)?;
        let scale_change = scale_before.sub(&scale_after, precision_bits)?;
        let anchor_x = BigScalar::from_f64(delta.anchor_canvas_px[0], precision_bits)?;
        let anchor_y = BigScalar::from_f64(delta.anchor_canvas_px[1], precision_bits)?;
        let pan_x = BigScalar::from_f64(delta.pan_canvas_px[0], precision_bits)?;
        let pan_y = BigScalar::from_f64(delta.pan_canvas_px[1], precision_bits)?;
        let mut next = self.coords.clone();
        for ((coordinate, basis_u), basis_v) in
            next.iter_mut().zip(plane.basis_u).zip(plane.basis_v)
        {
            let anchor =
                linear_combination(&anchor_x, basis_u, &anchor_y, basis_v, precision_bits)?;
            let pan = linear_combination(&pan_x, basis_u, &pan_y, basis_v, precision_bits)?;
            let zoom_shift = scale_change.mul(&anchor, precision_bits)?;
            let pan_shift = scale_after.mul(&pan, precision_bits)?;
            *coordinate = coordinate
                .add(&zoom_shift, precision_bits)?
                .sub(&pan_shift, precision_bits)?;
        }
        for coordinate in &next {
            let _finite_mirror = coordinate.to_f64()?;
        }
        self.coords = next;
        Ok(())
    }

    /// Projects this centre minus a reference centre onto `(u,v)` in units of `pixel_scale`.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched precision, invalid plane or scale, an out-of-range binary64
    /// result, or Astro-float arithmetic failure.
    pub fn displacement_px(
        &self,
        reference: &Self,
        plane: &Plane,
        pixel_scale: f64,
    ) -> Result<[f64; 2], MathError> {
        if self.precision_bits != reference.precision_bits {
            return Err(MathError::PrecisionMismatch);
        }
        if !pixel_scale.is_finite()
            || pixel_scale <= 0.0
            || !plane
                .basis_u
                .iter()
                .chain(&plane.basis_v)
                .all(|component| component.is_finite())
        {
            return Err(MathError::NonFinite);
        }
        let precision_bits = self.precision_bits;
        let scale = BigScalar::from_f64(pixel_scale, precision_bits)?;
        let delta = [
            self.coords[0].sub(&reference.coords[0], precision_bits)?,
            self.coords[1].sub(&reference.coords[1], precision_bits)?,
            self.coords[2].sub(&reference.coords[2], precision_bits)?,
            self.coords[3].sub(&reference.coords[3], precision_bits)?,
        ];
        let project = |basis: [f32; 4]| -> Result<f64, MathError> {
            let mut sum = BigScalar::zero(precision_bits)?;
            for (component, weight) in delta.iter().zip(basis) {
                let weight = BigScalar::from_f32(weight, precision_bits)?;
                sum = sum.add(&component.mul(&weight, precision_bits)?, precision_bits)?;
            }
            sum.div(&scale, precision_bits)?.to_f64()
        };
        Ok([project(plane.basis_u)?, project(plane.basis_v)?])
    }

    /// Returns the authoritative centre rounded directly to nearest binary64, ties to even.
    ///
    /// # Panics
    ///
    /// Panics only if a caller assembled a public `BigCentre` with a coordinate outside finite
    /// binary64 range; `from_f64` and successful `apply_navigation` preserve this invariant.
    #[must_use]
    pub fn to_f64_mirror(&self) -> [f64; 4] {
        let [a, b, c, d] = &self.coords;
        [
            a.to_f64().expect("validated centre has a finite mirror"),
            b.to_f64().expect("validated centre has a finite mirror"),
            c.to_f64().expect("validated centre has a finite mirror"),
            d.to_f64().expect("validated centre has a finite mirror"),
        ]
    }
}

fn linear_combination(
    x: &BigScalar,
    basis_x: f32,
    y: &BigScalar,
    basis_y: f32,
    precision_bits: u32,
) -> Result<BigScalar, MathError> {
    let basis_x = BigScalar::from_f32(basis_x, precision_bits)?;
    let basis_y = BigScalar::from_f32(basis_y, precision_bits)?;
    x.mul(&basis_x, precision_bits)?
        .add(&y.mul(&basis_y, precision_bits)?, precision_bits)
}

fn validate_navigation_input(
    delta: &NavigationDelta,
    plane: &Plane,
    zoom_log2_before: f64,
    zoom_log2_after: f64,
) -> Result<(), MathError> {
    if !zoom_log2_before.is_finite()
        || !zoom_log2_after.is_finite()
        || !delta.zoom_delta_log2.is_finite()
        || !delta
            .pan_canvas_px
            .iter()
            .chain(&delta.anchor_canvas_px)
            .all(|component| component.is_finite())
        || !plane
            .basis_u
            .iter()
            .chain(&plane.basis_v)
            .all(|component| component.is_finite())
    {
        return Err(MathError::NonFinite);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlaneAngles, construct_plane};

    /// The zero-angle seed, whose basis is exactly `(e₃,e₄)` with no rounding at all, so a
    /// navigation assertion measures navigation rather than the binary32 image of `cos(π/2)`.
    fn exact_seed_plane() -> Result<Plane, MathError> {
        construct_plane(PlaneAngles {
            theta_1: 0.0,
            theta_2: 0.0,
        })
    }

    fn ulp_distance(left: f64, right: f64) -> u64 {
        fn ordered(value: f64) -> u64 {
            let bits = value.to_bits();
            if bits >> 63 == 0 {
                bits | (1_u64 << 63)
            } else {
                !bits
            }
        }
        ordered(left).abs_diff(ordered(right))
    }

    #[test]
    fn anchored_zoom_preserves_the_anchor_through_depth() -> Result<(), MathError> {
        let plane = exact_seed_plane()?;
        let anchor = [19.5, -7.25];
        for zoom_before in [0.0, 40.0, 100.0] {
            let zoom_after = zoom_before + 1.0;
            let scale_before = pixel_scale(zoom_before, 1024)?;
            let scale_after = pixel_scale(zoom_after, 1024)?;
            let mut centre = BigCentre::from_f64([0.0; 4], 384)?;
            centre.apply_navigation(
                &NavigationDelta {
                    pan_canvas_px: [0.0; 2],
                    zoom_delta_log2: 1.0,
                    anchor_canvas_px: anchor,
                },
                &plane,
                zoom_before,
                zoom_after,
                1024,
            )?;
            let mirror = centre.to_f64_mirror();
            for axis in 0..2 {
                let before_point = scale_before * anchor[axis];
                let after_point = scale_after.mul_add(anchor[axis], mirror[axis + 2]);
                assert!(
                    ulp_distance(before_point, after_point) <= 1,
                    "zoom {zoom_before}, axis {axis}: {before_point:e} != {after_point:e}"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn navigation_holds_every_coordinate_at_the_declared_precision() -> Result<(), MathError> {
        let plane = exact_seed_plane()?;
        // Canvas-relative CSS pixels scaled by 960/1022.794: full 53-bit mantissas.
        let pixel_ratio = 960.0_f64 / 1_022.794_f64;
        for requested in [47_u32, 64, 90, 128, 384, 1_024] {
            let mut centre = BigCentre::from_f64([0.0; 4], requested)?;
            let declared = centre.precision_bits;
            assert_eq!(declared, (requested + 63) & !63, "requested {requested}");
            let mut zoom_log2 = 0.0_f64;
            for tick in 1..=6_i32 {
                let after = zoom_log2 + 0.2;
                centre.apply_navigation(
                    &NavigationDelta {
                        pan_canvas_px: [f64::from(tick) * pixel_ratio, -3.5 * pixel_ratio],
                        zoom_delta_log2: 0.2,
                        anchor_canvas_px: [
                            173.0 * pixel_ratio,
                            f64::from(-91 * tick) * pixel_ratio,
                        ],
                    },
                    &plane,
                    zoom_log2,
                    after,
                    960,
                )?;
                zoom_log2 = after;
                assert_eq!(centre.precision_bits, declared, "requested {requested}");
                for (axis, coordinate) in centre.coords.iter().enumerate() {
                    assert_eq!(
                        coordinate.precision_bits(),
                        declared,
                        "requested {requested}, tick {tick}, axis {axis}"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn drag_is_exactly_negative_current_scale_times_plane_motion() -> Result<(), MathError> {
        let plane = exact_seed_plane()?;
        let zoom = 40.0;
        let scale = pixel_scale(zoom, 1024)?;
        let pan = [3.5, -2.25];
        let mut centre = BigCentre::from_f64([0.0; 4], 384)?;
        centre.apply_navigation(
            &NavigationDelta {
                pan_canvas_px: pan,
                zoom_delta_log2: 0.0,
                anchor_canvas_px: [0.0; 2],
            },
            &plane,
            zoom,
            zoom,
            1024,
        )?;
        assert_eq!(
            centre.to_f64_mirror(),
            [0.0, 0.0, -scale * pan[0], -scale * pan[1]]
        );
        Ok(())
    }

    #[test]
    fn displacement_round_trips_plane_pixel_motion() -> Result<(), MathError> {
        let plane = exact_seed_plane()?;
        let zoom = 80.0;
        let scale = pixel_scale(zoom, 2048)?;
        let reference = BigCentre::from_f64([0.0; 4], 384)?;
        let mut centre = reference.clone();
        centre.apply_navigation(
            &NavigationDelta {
                pan_canvas_px: [-13.0, 9.0],
                zoom_delta_log2: 0.0,
                anchor_canvas_px: [0.0; 2],
            },
            &plane,
            zoom,
            zoom,
            2048,
        )?;
        assert_eq!(
            centre.displacement_px(&reference, &plane, scale)?,
            [13.0, -9.0]
        );
        Ok(())
    }
}
