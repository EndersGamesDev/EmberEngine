// CPU mirrors intentionally reproduce WGSL's fixed-width conversions and written operation order.
#![allow(clippy::cast_precision_loss, clippy::suboptimal_flops)]

use ember_julibrot_math::{CentreSplit, EscapeGridRecord, EscapeParams, Plane};

use crate::{GridExtent, KernelError, RefinementLevel, ShallowUniform, records::pixel_offset};

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
                    glitch: 0.0,
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
            glitch: 0.0,
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
    let offset = pixel_offset(
        index,
        extent,
        Plane {
            basis_u: uniforms.basis_u,
            basis_v: uniforms.basis_v,
        },
        uniforms.pixel_scale,
    );
    let point = std::array::from_fn(|axis| {
        uniforms.centre_hi[axis] + (uniforms.centre_lo[axis] + offset[axis])
    });
    escape_shallow_point(
        point,
        EscapeParams {
            max_iter: uniforms.max_iter,
            bailout: uniforms.bailout,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{escape_shallow_pixel, escape_shallow_point};
    use crate::{GridExtent, KernelError, RefinementLevel, ShallowUniform};
    use ember_julibrot_math::{CentreSplit, EscapeParams, Plane};

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
            width: 1,
            height: 1,
        };
        let result = ShallowUniform::pack(
            Plane {
                basis_u: [1.0, 0.0, 0.0, 0.0],
                basis_v: [0.0, 1.0, 0.0, 0.0],
            },
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

    #[cfg(feature = "math-oracles")]
    #[test]
    fn deterministic_points_match_the_math_oracle() {
        for point in [
            [0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [20.0, 0.0, 0.0, 0.0],
        ] {
            let params = EscapeParams::new(64);
            let actual = escape_shallow_point(point, params).expect("kernel mirror accepts point");
            let expected =
                ember_julibrot_math::escape_f32(point, params).expect("math oracle accepts point");
            assert_eq!(actual.escape_index, expected.escape_index);
            assert_eq!(actual.record.escaped == 1.0, expected.escaped);
            assert!((actual.record.smooth_iter - expected.smooth_iter).abs() <= 1.0e-4);
        }
    }
}
