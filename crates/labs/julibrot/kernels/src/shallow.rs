// CPU mirrors intentionally reproduce WGSL's fixed-width conversions and written operation order.
#![allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]

use ember_julibrot_math::{CentreSplit, EscapeGridRecord, EscapeParams, Homography, Plane};

use crate::{
    GridExtent, KernelError, RefinementLevel, SampleStatus, ShallowUniform,
    records::{pack_map_rows, pixel_offset},
};

/// CPU mirror result plus the conformance-only integer escape index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KernelSample {
    pub record: EscapeGridRecord,
    pub escape_index: Option<u32>,
}

impl ShallowUniform {
    /// Packs a checked shallow-dispatch payload.
    ///
    /// # Errors
    ///
    /// Returns a typed refusal for a zero or overflowing extent, a non-positive scale, or escape
    /// parameters other than the fixed squared bailout and a nonzero cap.
    pub fn pack(
        plane: Plane,
        screen_to_plane: &Homography,
        centre: CentreSplit,
        pixel_scale: f32,
        extent: GridExtent,
        params: EscapeParams,
        level: RefinementLevel,
    ) -> Result<Self, KernelError> {
        validate_extent(extent)?;
        validate_params(params)?;
        if !pixel_scale.is_finite() || pixel_scale <= 0.0 {
            return Err(KernelError::InvalidEscapeParams);
        }
        Ok(Self::from_parts(
            plane,
            pack_map_rows(screen_to_plane)?,
            centre,
            pixel_scale,
            extent,
            params.max_iter,
            level,
        ))
    }
}

/// Returns the checked pixel count for a non-empty grid extent.
///
/// # Errors
///
/// Returns [`KernelError::InvalidExtent`] for an empty extent and
/// [`KernelError::ArithmeticOverflow`] when the pixel count exceeds `u32`.
pub fn validate_extent(extent: GridExtent) -> Result<u32, KernelError> {
    if extent.width == 0 || extent.height == 0 {
        return Err(KernelError::InvalidExtent);
    }
    extent
        .width
        .checked_mul(extent.height)
        .ok_or(KernelError::ArithmeticOverflow)
}

/// Validates the fixed escape policy shared by both kernels.
///
/// # Errors
///
/// Returns [`KernelError::InvalidEscapeParams`] for a zero iteration cap or a bailout other than
/// [`EscapeParams::BAILOUT`].
pub const fn validate_params(params: EscapeParams) -> Result<(), KernelError> {
    if params.max_iter == 0 || params.bailout.to_bits() != EscapeParams::BAILOUT.to_bits() {
        return Err(KernelError::InvalidEscapeParams);
    }
    Ok(())
}

fn complex_square(value: [f32; 2]) -> [f32; 2] {
    let real = value[0] * value[0] - value[1] * value[1];
    let imaginary = 2.0 * value[0] * value[1];
    [real, imaginary]
}

fn radius_squared(value: [f32; 2]) -> f32 {
    value[0] * value[0] + value[1] * value[1]
}

fn log2_norm(value: [f32; 2]) -> f32 {
    let scale = value[0].abs().max(value[1].abs());
    let normalized = [value[0] / scale, value[1] / scale];
    scale.log2() + 0.5 * (normalized[0] * normalized[0] + normalized[1] * normalized[1]).log2()
}

fn smooth_iteration(iteration: u32, value: [f32; 2]) -> f32 {
    iteration as f32 + 1.0 - log2_norm(value).log2()
}

/// Mirrors the shallow WGSL recurrence for one already formed four-dimensional sample.
///
/// # Errors
///
/// Returns `InvalidEscapeParams` when the cap or squared bailout violates the fixed contract.
pub fn escape_shallow_point(
    point: [f32; 4],
    params: EscapeParams,
) -> Result<KernelSample, KernelError> {
    validate_params(params)?;
    let mut z = [point[0], point[1]];
    let c = [point[2], point[3]];
    for iteration in 0..params.max_iter {
        if radius_squared(z) > params.bailout {
            return Ok(KernelSample {
                record: EscapeGridRecord {
                    smooth_iter: smooth_iteration(iteration, z),
                    escaped: 1.0,
                    rebase_count: 0.0,
                    status: SampleStatus::Sampled.as_f32(),
                },
                escape_index: Some(iteration),
            });
        }
        if iteration + 1 < params.max_iter {
            let square = complex_square(z);
            z = [square[0] + c[0], square[1] + c[1]];
        }
    }
    Ok(KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: 0.0,
            status: SampleStatus::Sampled.as_f32(),
        },
        escape_index: None,
    })
}

/// Forms one bottom-up pixel centre in the pinned f32 order and mirrors the shallow kernel.
///
/// # Errors
///
/// Returns a typed extent refusal when `index` is outside the active grid, or an escape-parameter
/// refusal inherited from `escape_shallow_point`.
pub fn escape_shallow_pixel(
    uniforms: &ShallowUniform,
    index: u32,
) -> Result<KernelSample, KernelError> {
    let extent = GridExtent {
        width: uniforms.width,
        height: uniforms.height,
    };
    let active_len = validate_extent(extent)?;
    if index >= active_len {
        return Err(KernelError::InvalidExtent);
    }
    let mapped = match pixel_offset(
        index,
        extent,
        Plane {
            basis_u: uniforms.basis_u,
            basis_v: uniforms.basis_v,
        },
        [
            uniforms.screen_to_plane_row_0,
            uniforms.screen_to_plane_row_1,
            uniforms.screen_to_plane_row_2,
        ],
        uniforms.pixel_scale,
    ) {
        Ok(mapped) => mapped,
        Err(status) => return Ok(terminal_sample(status)),
    };
    let point = std::array::from_fn(|axis| {
        uniforms.centre_hi[axis] + (uniforms.centre_lo[axis] + mapped.offset[axis])
    });
    if !point.iter().all(|value| value.is_finite()) {
        return Ok(terminal_sample(SampleStatus::MapUncertain));
    }
    let mut sample = escape_shallow_point(
        point,
        EscapeParams {
            max_iter: uniforms.max_iter,
            bailout: uniforms.bailout,
        },
    )?;
    sample.record.status = mapped.status.as_f32();
    Ok(sample)
}

pub const fn terminal_sample(status: SampleStatus) -> KernelSample {
    if matches!(status, SampleStatus::MapUncertain) {
        return KernelSample {
            record: EscapeGridRecord {
                smooth_iter: 0.0,
                escaped: 1.0,
                rebase_count: 0.0,
                status: SampleStatus::MapUncertain.as_f32(),
            },
            escape_index: Some(0),
        };
    }
    KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: 0.0,
            status: status.as_f32(),
        },
        escape_index: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_shallow_pixel, escape_shallow_point};
    use crate::{GridExtent, KernelError, RefinementLevel, SampleStatus, ShallowUniform};
    use ember_julibrot_math::{CentreSplit, EscapeParams, Homography, Plane, PrecisionMode};

    #[test]
    fn known_current_state_indices_are_exact() {
        let params = EscapeParams::new(10);
        let interior = escape_shallow_point([0.0; 4], params).expect("fixed params are valid");
        assert_eq!(interior.escape_index, None);
        assert_eq!(interior.record.smooth_iter, -1.0);
        let immediate =
            escape_shallow_point([20.0, 0.0, 0.0, 0.0], params).expect("point is valid");
        assert_eq!(immediate.escape_index, Some(0));
        let c_two = escape_shallow_point([0.0, 0.0, 2.0, 0.0], params).expect("point is valid");
        assert_eq!(c_two.escape_index, Some(3));
        assert_eq!(c_two.record.escaped, 1.0);
    }

    #[test]
    fn pixel_mirror_uses_centre_hi_plus_lo_plus_offset_order() {
        let uniform = ShallowUniform::pack(
            Plane {
                basis_u: [0.0, 0.0, 1.0, 0.0],
                basis_v: [0.0, 0.0, 0.0, 1.0],
            },
            &Homography::IDENTITY,
            CentreSplit {
                hi: [0.0; 4],
                lo: [0.0; 4],
            },
            1.0,
            GridExtent {
                width: 1,
                height: 1,
            },
            EscapeParams::new(8),
            RefinementLevel::Final,
        )
        .expect("uniform is valid");
        let centre = escape_shallow_pixel(&uniform, 0).expect("centre pixel is in range");
        assert_eq!(centre.escape_index, None);
        assert_eq!(
            escape_shallow_pixel(&uniform, 1),
            Err(KernelError::InvalidExtent)
        );
    }

    #[test]
    fn invalid_uniform_inputs_are_typed_refusals() {
        let extent = GridExtent {
            width: 2,
            height: 1,
        };
        let result = ShallowUniform::pack(
            Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
            &Homography::IDENTITY,
            CentreSplit {
                hi: [0.0; 4],
                lo: [0.0; 4],
            },
            0.0,
            extent,
            EscapeParams::new(8),
            RefinementLevel::Preview,
        );
        assert_eq!(result, Err(KernelError::InvalidEscapeParams));
    }

    #[test]
    fn deterministic_points_match_the_math_oracle() {
        for mode in PrecisionMode::ALL {
            for point in [
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 2.0, 0.0],
                [20.0, 0.0, 0.0, 0.0],
            ] {
                let params = EscapeParams::new(64);
                let actual =
                    escape_shallow_point(point, params).expect("kernel mirror accepts point");
                let expected = ember_julibrot_math::escape_f32(point, params)
                    .expect("math oracle accepts point");
                assert_eq!(
                    crate::evaluate_shallow_conformance(mode, actual, expected).verdict,
                    crate::ConformanceVerdict::Pass
                );
            }
        }
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn preset_and_hybrid_pixels_match_the_math_oracle() {
        use ember_julibrot_math::{PlaneAngles, construct_plane, escape_f32};

        let quarter = core::f64::consts::FRAC_PI_2;
        let fixtures = [
            (
                PlaneAngles {
                    theta_1: 0.0,
                    theta_2: 0.0,
                },
                [0.0; 4],
            ),
            (
                PlaneAngles {
                    theta_1: -quarter,
                    theta_2: -quarter,
                },
                [0.0, 0.0, -0.8, 0.156],
            ),
            (
                PlaneAngles {
                    theta_1: 0.4,
                    theta_2: 0.7,
                },
                [0.0; 4],
            ),
        ];
        let extent = GridExtent {
            width: 3,
            height: 3,
        };
        let params = EscapeParams::new(64);
        for mode in PrecisionMode::ALL {
            for (angles, origin) in fixtures {
                let plane = construct_plane(angles).expect("math plane");
                let uniform = ShallowUniform::pack(
                    plane,
                    &Homography::IDENTITY,
                    CentreSplit {
                        hi: origin.map(|component| component as f32),
                        lo: [0.0; 4],
                    },
                    0.25,
                    extent,
                    params,
                    RefinementLevel::Final,
                )
                .expect("fixture uniform");
                for index in 0..9 {
                    let offset = crate::records::pixel_offset(
                        index,
                        extent,
                        plane,
                        [
                            uniform.screen_to_plane_row_0,
                            uniform.screen_to_plane_row_1,
                            uniform.screen_to_plane_row_2,
                        ],
                        0.25,
                    )
                    .expect("identity map has no terminal pixel")
                    .offset;
                    let point = std::array::from_fn(|axis| uniform.centre_hi[axis] + offset[axis]);
                    let observed = escape_shallow_pixel(&uniform, index).expect("kernel mirror");
                    let expected = escape_f32(point, params).expect("math oracle");
                    assert_eq!(
                        crate::evaluate_shallow_conformance(mode, observed, expected).verdict,
                        crate::ConformanceVerdict::Pass
                    );
                }
            }
        }
    }

    #[test]
    fn a_horizon_crossing_map_splits_terminal_records_from_beyond_bailout_escapes() {
        // One row whose denominator is the centred column coordinate: the left half is beyond the
        // horizon and terminal, the right half divides and lands outside the bailout radius 16, so
        // its escape index is zero and its smooth count `1-log2(log2|z_0|)` is below -1. Both sides
        // are well-formed records; neither is a contract violation for present to tint.
        let plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let centre = CentreSplit {
            hi: [0.0; 4],
            lo: [0.0; 4],
        };
        let extent = GridExtent {
            width: 8,
            height: 1,
        };
        let map = Homography {
            rows: [0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            inverse: Homography::IDENTITY.inverse,
            condition_number: 1.0,
            apron_scale: 1.0,
        };
        let uniform = ShallowUniform::pack(
            plane,
            &map,
            centre,
            1.0,
            extent,
            EscapeParams::new(64),
            RefinementLevel::Final,
        )
        .expect("finite map packs");
        let mut horizons = 0_u32;
        let mut beyond_bailout = 0_u32;
        for index in 0..8 {
            let sample = escape_shallow_pixel(&uniform, index).expect("pixel is in range");
            assert!(crate::record_is_well_formed(
                sample,
                crate::KernelMode::Shallow
            ));
            if index < 4 {
                assert_eq!(sample, super::terminal_sample(SampleStatus::Horizon));
                horizons += 1;
                continue;
            }
            assert_eq!(sample.record.status, SampleStatus::Sampled.as_f32());
            assert_eq!(sample.record.escaped, 1.0);
            assert_eq!(sample.escape_index, Some(0));
            assert!(sample.record.smooth_iter.is_finite());
            assert!(sample.record.smooth_iter < -1.0);
            beyond_bailout += 1;
            // The CPU mirror and math's binary32 oracle agree on the same negative count.
            let offset = crate::records::pixel_offset(
                index,
                extent,
                plane,
                [
                    uniform.screen_to_plane_row_0,
                    uniform.screen_to_plane_row_1,
                    uniform.screen_to_plane_row_2,
                ],
                1.0,
            )
            .expect("the positive-denominator half maps")
            .offset;
            let expected = ember_julibrot_math::escape_f32(offset, EscapeParams::new(64))
                .expect("math oracle accepts the point");
            for mode in PrecisionMode::ALL {
                assert_eq!(
                    crate::evaluate_shallow_conformance(mode, sample, expected).verdict,
                    crate::ConformanceVerdict::Pass
                );
            }
        }
        assert_eq!((horizons, beyond_bailout), (4, 4));
    }

    #[test]
    fn horizon_is_terminal_but_uncertain_pixels_are_sampled() {
        let plane = Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        };
        let centre = CentreSplit {
            hi: [0.0; 4],
            lo: [0.0; 4],
        };
        let extent = GridExtent {
            width: 2,
            height: 1,
        };
        for (rows, status) in [
            (
                [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0],
                SampleStatus::Horizon,
            ),
            (
                [1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.500_000_06],
                SampleStatus::MapUncertain,
            ),
        ] {
            let map = Homography {
                rows: rows.map(f64::from),
                inverse: Homography::IDENTITY.inverse,
                condition_number: 1.0,
                apron_scale: 1.0,
            };
            let uniform = ShallowUniform::pack(
                plane,
                &map,
                centre,
                1.0,
                extent,
                EscapeParams::new(8),
                RefinementLevel::Final,
            )
            .expect("finite map packs");
            let sample = escape_shallow_pixel(&uniform, 0).expect("pixel is in range");
            if status == SampleStatus::Horizon {
                assert_eq!(sample, super::terminal_sample(status));
            } else {
                assert_eq!(sample.record.status, SampleStatus::MapUncertain.as_f32());
                assert_eq!(sample.record.escaped, 1.0);
            }
            assert!(sample.record.smooth_iter.is_finite());
            assert!(sample.record.status.is_finite());
        }
    }
}
