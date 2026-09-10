//! Boundary adapters between Julibrot controls and the exact camera record.

use ember_camera::{
    CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, MAX_EXPONENT_QUANTA,
    MIN_EXPONENT_QUANTA, Observer, Orientation, Turn, TwoStageProjection, View,
    orientation_from_frame, rebuild_basis,
};
use ember_julibrot_math::{
    BigCentre, BigScalar, ObjectAngles, Plane, ViewControls, decode_big_scalar, encode_big_scalar,
};

use crate::AppError;

pub const CAMERA_LIMBS: usize = 8;
pub const CAMERA_PRECISION_BITS: u32 = 512;
const CAMERA_FRACTION_BITS: i32 = 448;
const FIXED_LIMB_BITS: usize = 64;
const OBJECT_PLANES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
const TURN_RADIANS_PER_BIT: f64 = core::f64::consts::TAU / 4_294_967_296.0;

pub type ExactCentre = [Fixed<CAMERA_LIMBS>; 4];
pub type ExactView = View<4, CAMERA_LIMBS>;

/// Maps Julibrot's object-product convention onto the exact camera's image axes.
///
/// Julibrot's six row-major factors rotate the seed axes `e3` and `e4`. The camera factors use the
/// same product order but expose frame rows zero and one as `u` and `v`. Reordering the rebuilt
/// legacy frame as `(e3, e4, -e1, -e2)` preserves both image axes and handedness. Saved binary64
/// angles quantise directly into turns and rebuild this complete binary64 frame; the legacy
/// binary32 `Plane` is never an input to the camera record.
pub fn orientation_from_object(angles: &ObjectAngles) -> Result<Orientation<4>, AppError> {
    if !angles.is_valid() {
        return Err(AppError::Math("object angles are not valid".to_string()));
    }
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let legacy = legacy_orientation(angles)?;
    let basis = rebuild_basis(&legacy).map_err(camera_error)?;
    let frame = [
        basis.remaining[2],
        basis.remaining[3],
        basis.u.map(|component| -component),
        basis.v.map(|component| -component),
    ];
    orientation_from_frame::<4>(&frame).map_err(camera_error)
}

/// Reconstructs the canonical saved object-angle row from an exact camera orientation.
///
/// Saved rows in the canonical factor chart round-trip bit-exactly when every angle is an integer
/// multiple of one `Turn` quantum. Every signed zero enters the record as `Turn::ZERO`; both signs
/// of pi enter as the single `0x8000_0000` half-turn. Readout uses the half-open signed interval
/// `[-pi, pi)`, so that half-turn becomes negative pi. At a factorisation singularity an equivalent
/// half-turn can move between factors, but the first canonical readout is stable thereafter.
pub fn object_from_orientation(orientation: &Orientation<4>) -> Result<ObjectAngles, AppError> {
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let basis = rebuild_basis(orientation).map_err(camera_error)?;
    let legacy_frame = [
        basis.remaining[2].map(|component| -component),
        basis.remaining[3].map(|component| -component),
        basis.u,
        basis.v,
    ];
    let legacy = orientation_from_frame::<4>(&legacy_frame).map_err(camera_error)?;
    let values = OBJECT_PLANES
        .map(|(first, second)| legacy.angle(first, second).map_or(0.0, turn_to_radians));
    Ok(ObjectAngles {
        rho_12: values[0],
        rho_13: values[1],
        rho_14: values[2],
        rho_23: values[3],
        rho_24: values[4],
        rho_34: values[5],
    })
}

/// Rounds the exact camera's rebuilt image axes once into the unchanged renderer plane shape.
pub fn plane_from_orientation(orientation: &Orientation<4>) -> Result<Plane, AppError> {
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let basis = rebuild_basis(orientation).map_err(camera_error)?;
    let mut basis_u = [0.0_f32; 4];
    let mut basis_v = [0.0_f32; 4];
    for ((u, v), (exact_u, exact_v)) in basis_u
        .iter_mut()
        .zip(&mut basis_v)
        .zip(basis.u.into_iter().zip(basis.v))
    {
        *u = binary32_from_binary64(exact_u)?;
        *v = binary32_from_binary64(exact_v)?;
    }
    Ok(Plane { basis_u, basis_v })
}

fn binary32_from_binary64(value: f64) -> Result<f32, AppError> {
    if !value.is_finite() {
        return Err(AppError::Math(
            "camera basis component is not finite".to_string(),
        ));
    }
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 0 } else { 1_u32 << 31 };
    let exponent_field = (bits >> 52) & 0x7ff;
    let fraction = bits & ((1_u64 << 52) - 1);
    if exponent_field == 0 {
        return Ok(f32::from_bits(sign));
    }
    let mut exponent = i64::try_from(exponent_field).map_err(|_| fixed_range_error())? - 1_023;
    let significand = (1_u64 << 52) | fraction;
    if exponent < -149 {
        return Ok(f32::from_bits(sign));
    }
    let mut rounded = if exponent >= -126 {
        round_right_ties_even(significand, 29)
    } else {
        round_right_ties_even(
            significand,
            usize::try_from(-exponent - 97).map_err(|_| fixed_range_error())?,
        )
    };
    if exponent >= -126 && rounded == 1_u64 << 24 {
        rounded >>= 1;
        exponent += 1;
    }
    let encoded = if exponent >= -126 {
        let biased = u32::try_from(exponent + 127).map_err(|_| fixed_range_error())?;
        let fraction =
            u32::try_from(rounded & ((1_u64 << 23) - 1)).map_err(|_| fixed_range_error())?;
        sign | (biased << 23) | fraction
    } else if rounded == 1_u64 << 23 {
        sign | (1_u32 << 23)
    } else {
        sign | u32::try_from(rounded).map_err(|_| fixed_range_error())?
    };
    Ok(f32::from_bits(encoded))
}

const fn round_right_ties_even(value: u64, shift: usize) -> u64 {
    if shift == 0 {
        return value;
    }
    if shift >= 64 {
        return 0;
    }
    let retained = value >> shift;
    let halfway = 1_u64 << (shift - 1);
    let remainder = value & ((1_u64 << shift) - 1);
    let rounds_up = remainder > halfway || (remainder == halfway && retained & 1 != 0);
    // This lossless Boolean conversion is available during const evaluation on the pinned compiler.
    retained + rounds_up as u64
}

fn legacy_orientation(angles: &ObjectAngles) -> Result<Orientation<4>, AppError> {
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let mut turns = [[Turn::ZERO; 4]; 4];
    for ((first, second), radians) in OBJECT_PLANES.into_iter().zip(angles.as_array()) {
        turns[first][second] = Turn::from_radians(radians).map_err(camera_error)?;
    }
    Orientation::new(turns).map_err(camera_error)
}

fn turn_to_radians(turn: Turn) -> f64 {
    let signed_bits = i32::from_ne_bytes(turn.bits().to_ne_bytes());
    f64::from(signed_bits) * TURN_RADIANS_PER_BIT
}

/// Quantises one DOM zoom value by round-to-nearest, ties-to-even.
///
/// The unbiased tie rule makes equal opposing device deltas cancel instead of accumulating a
/// directional half-quantum bias. This is the sole binary64-to-exponent conversion; displayed and
/// persisted zoom values are derived from the resulting integer quanta.
pub fn quantize_zoom_log2(zoom_log2: f64) -> Result<Exponent, AppError> {
    let minimum = f64::from(MIN_EXPONENT_QUANTA) / f64::from(EXPONENT_QUANTA_PER_OCTAVE);
    let maximum = f64::from(MAX_EXPONENT_QUANTA) / f64::from(EXPONENT_QUANTA_PER_OCTAVE);
    if !zoom_log2.is_finite() || !(minimum..=maximum).contains(&zoom_log2) {
        return Err(AppError::Math(
            "zoom input is outside the camera range".to_string(),
        ));
    }
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    Exponent::new(quantize_zoom_delta_quanta(zoom_log2)?).map_err(camera_error)
}

/// Quantises one relative DOM zoom input without treating it as an absolute exponent.
///
/// Round-to-nearest, ties-to-even is unbiased: opposing device deltas cancel rather than building
/// a directional half-quantum drift. The resulting integer is range-checked again when the camera
/// adds it to the stored exponent.
pub fn quantize_zoom_delta_quanta(delta_log2: f64) -> Result<i32, AppError> {
    if !delta_log2.is_finite() {
        return Err(AppError::Math("zoom input is not finite".to_string()));
    }
    let scaled = delta_log2 * f64::from(EXPONENT_QUANTA_PER_OCTAVE);
    let rounded = scaled.round_ties_even();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(AppError::Math(
            "zoom input is outside the exponent representation".to_string(),
        ));
    }
    rounded_binary64_to_i32(rounded)
}

fn rounded_binary64_to_i32(value: f64) -> Result<i32, AppError> {
    let range_error =
        || AppError::Math("zoom input is outside the exponent representation".to_string());
    if value == 0.0 {
        return Ok(0);
    }
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let exponent_field = (bits >> 52) & 0x7ff;
    let exponent = i64::try_from(exponent_field).map_err(|_| range_error())? - 1_023;
    let significand = (1_u64 << 52) | (bits & ((1_u64 << 52) - 1));
    let magnitude = if exponent >= 52 {
        significand
            .checked_shl(u32::try_from(exponent - 52).map_err(|_| range_error())?)
            .ok_or_else(range_error)?
    } else {
        significand >> u32::try_from(52 - exponent).map_err(|_| range_error())?
    };
    let magnitude = i64::try_from(magnitude).map_err(|_| range_error())?;
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).map_err(|_| range_error())
}

pub fn zoom_log2_from_exponent(exponent: Exponent) -> f64 {
    f64::from(exponent.quanta()) / f64::from(EXPONENT_QUANTA_PER_OCTAVE)
}

/// Maps Julibrot's complete two-stage presentation observer without changing exact camera state.
///
/// The image axes come from the exact record. All ten presentation rotations, all five translation
/// components, and both perspective distances remain binary64 observer inputs. In particular,
/// `camera_translation[3]` and `[4]` change the five-to-four denominator rather than being silently
/// discarded. Relief amplitude is absent because targeting intersects the flat base plane.
pub fn observer_from_view(
    camera: &ExactView,
    view: &ViewControls,
) -> Result<Observer<5>, AppError> {
    if !view.is_valid() {
        return Err(AppError::Math(
            "presentation controls are not valid".to_string(),
        ));
    }
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let basis = rebuild_basis(&camera.orientation).map_err(camera_error)?;
    Observer::two_stage(TwoStageProjection {
        image_plane: [
            [basis.u[0], basis.u[1], basis.u[2], basis.u[3], 0.0],
            [basis.v[0], basis.v[1], basis.v[2], basis.v[3], 0.0],
        ],
        frame_angles: view.camera,
        translation: view.camera_translation,
        yaw: view.camera_yaw,
        pitch: view.camera_pitch,
        distance_five: view.distance_five,
        distance_four: view.distance_four,
    })
    .map_err(camera_error)
}

/// Converts the fixed camera centre into the unchanged full-width worker math shape exactly.
pub fn big_centre_from_fixed(centre: &ExactCentre) -> Result<BigCentre, AppError> {
    let [a, b, c, d] = *centre;
    Ok(BigCentre {
        coords: [
            big_scalar_from_fixed(a)?,
            big_scalar_from_fixed(b)?,
            big_scalar_from_fixed(c)?,
            big_scalar_from_fixed(d)?,
        ],
        precision_bits: CAMERA_PRECISION_BITS,
    })
}

fn big_scalar_from_fixed(value: Fixed<CAMERA_LIMBS>) -> Result<BigScalar, AppError> {
    let math_error = |error: ember_julibrot_math::MathError| AppError::Math(error.to_string());
    let negative = value.is_negative();
    let mut magnitude = value.to_le_bytes().map(u64::from_le_bytes);
    if negative {
        twos_complement(&mut magnitude);
    }
    let mut limbs = [0_u32; CAMERA_LIMBS * 2];
    for (index, word) in magnitude.into_iter().enumerate() {
        let bytes = word.to_le_bytes();
        limbs[index * 2] = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        limbs[index * 2 + 1] = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    }
    let used = limbs
        .iter()
        .rposition(|word| *word != 0)
        .map_or(0, |index| index + 1);
    if used == 0 {
        return BigScalar::zero(CAMERA_PRECISION_BITS).map_err(math_error);
    }
    decode_big_scalar(
        u32::from(negative),
        -CAMERA_FRACTION_BITS,
        &limbs[..used],
        CAMERA_PRECISION_BITS,
    )
    .map_err(math_error)
}

/// Converts an unchanged worker math centre back into the fixed camera record without rounding.
pub fn fixed_centre_from_big(centre: &BigCentre) -> Result<ExactCentre, AppError> {
    let [a, b, c, d] = &centre.coords;
    Ok([
        fixed_from_big_scalar(a)?,
        fixed_from_big_scalar(b)?,
        fixed_from_big_scalar(c)?,
        fixed_from_big_scalar(d)?,
    ])
}

fn fixed_from_big_scalar(value: &BigScalar) -> Result<Fixed<CAMERA_LIMBS>, AppError> {
    let math_error = |error: ember_julibrot_math::MathError| AppError::Math(error.to_string());
    let encoded = encode_big_scalar(value).map_err(math_error)?;
    let shift = i64::from(encoded.exponent) + i64::from(CAMERA_FRACTION_BITS);
    let mut magnitude = [0_u64; CAMERA_LIMBS];
    for (word_index, encoded_word) in encoded.limbs.iter().copied().enumerate() {
        let word_base = i64::try_from(word_index)
            .ok()
            .and_then(|index| index.checked_mul(i64::from(u32::BITS)))
            .ok_or_else(fixed_range_error)?;
        let mut remaining = encoded_word;
        while remaining != 0 {
            let source_bit = word_base
                .checked_add(i64::from(remaining.trailing_zeros()))
                .ok_or_else(fixed_range_error)?;
            let destination = source_bit
                .checked_add(shift)
                .ok_or_else(fixed_range_error)?;
            if destination < 0 {
                return Err(fixed_range_error());
            }
            let destination = usize::try_from(destination).map_err(|_| fixed_range_error())?;
            let Some(word) = magnitude.get_mut(destination / FIXED_LIMB_BITS) else {
                return Err(fixed_range_error());
            };
            *word |= 1_u64 << (destination % FIXED_LIMB_BITS);
            remaining &= remaining - 1;
        }
    }
    let sign_bit = 1_u64 << (u64::BITS - 1);
    if encoded.sign == 0 {
        if magnitude[CAMERA_LIMBS - 1] & sign_bit != 0 {
            return Err(fixed_range_error());
        }
    } else if encoded.sign == 1 {
        if magnitude[CAMERA_LIMBS - 1] > sign_bit
            || (magnitude[CAMERA_LIMBS - 1] == sign_bit
                && magnitude[..CAMERA_LIMBS - 1].iter().any(|word| *word != 0))
        {
            return Err(fixed_range_error());
        }
        twos_complement(&mut magnitude);
    } else {
        return Err(fixed_range_error());
    }
    Ok(Fixed::from_le_bytes(magnitude.map(u64::to_le_bytes)))
}

fn twos_complement(words: &mut [u64; CAMERA_LIMBS]) {
    let mut carry = true;
    for word in words {
        *word = !*word;
        if carry {
            let (next, overflow) = word.overflowing_add(1);
            *word = next;
            carry = overflow;
        }
    }
}

fn fixed_range_error() -> AppError {
    AppError::Math("centre is not exactly representable by Fixed<8>".to_string())
}

pub fn fixed_centre_from_f64(centre: [f64; 4]) -> Result<ExactCentre, AppError> {
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let mut fixed = [Fixed::ZERO; 4];
    for (output, coordinate) in fixed.iter_mut().zip(centre) {
        *output = Fixed::from_f64(coordinate).map_err(camera_error)?;
    }
    Ok(fixed)
}

pub fn centre_to_f64(centre: &ExactCentre) -> Result<[f64; 4], AppError> {
    let camera_error = |error: CameraError| AppError::Math(error.to_string());
    let [a, b, c, d] = centre;
    Ok([
        a.to_f64().map_err(camera_error)?,
        b.to_f64().map_err(camera_error)?,
        c.to_f64().map_err(camera_error)?,
        d.to_f64().map_err(camera_error)?,
    ])
}

#[cfg(test)]
mod tests {
    use ember_camera::{
        PROJECT_PIXEL_TOLERANCE_PIXELS, Screen, View, click, invert_perspective, pan,
        project_perspective, zoom_about,
    };
    use ember_julibrot_math::{
        NavigationDelta, ObjectAngles, construct_plane, navigation_delta, plane_to_screen,
        screen_to_plane,
    };
    use ember_julibrot_worker::EncodedCentre;

    use super::*;

    const OBSERVER_ORACLE_TOLERANCE_PIXELS: f64 = 1.0e-8;
    /// One stored turn bit is `2π / 2^32` radians.
    const TURN_QUANTUM_RADIANS: f64 = core::f64::consts::TAU / 4_294_967_296.0;
    /// At the width-1024 edge, one turn quantum moves a point by at most this many pixels.
    const TURN_QUANTUM_WIDTH_1024_EDGE_PIXELS: f64 = 7.490_140_565_847_857e-7;
    /// The sampled legacy binary32 planes differ by at most 18 turn quanta as they migrate.
    const LEGACY_F32_PROJECTOR_MIGRATION_BOUND_RADIANS: f64 = 18.0 * TURN_QUANTUM_RADIANS;

    fn sampled_objects() -> [ObjectAngles; 3] {
        [
            ObjectAngles::IDENTITY,
            ObjectAngles::JULIA,
            ObjectAngles {
                rho_12: 0.2,
                rho_13: -0.4,
                rho_14: 0.1,
                rho_23: 0.25,
                rho_24: -0.3,
                rho_34: 0.15,
            },
        ]
    }

    fn unrounded_mathematical_plane(object: ObjectAngles) -> [[f64; 4]; 2] {
        [
            rotate_unrounded_axis(2, object),
            rotate_unrounded_axis(3, object),
        ]
    }

    fn rotate_unrounded_axis(axis: usize, object: ObjectAngles) -> [f64; 4] {
        let mut value = [0.0; 4];
        value[axis] = 1.0;
        for (first, second, angle) in [
            (2, 3, object.rho_34),
            (1, 3, object.rho_24),
            (1, 2, object.rho_23),
            (0, 3, object.rho_14),
            (0, 2, object.rho_13),
            (0, 1, object.rho_12),
        ] {
            let (sine, cosine) = angle.sin_cos();
            let first_value = cosine.mul_add(value[first], -sine * value[second]);
            let second_value = sine.mul_add(value[first], cosine * value[second]);
            value[first] = first_value;
            value[second] = second_value;
        }
        value
    }

    fn projector_residual(left: [[f64; 4]; 2], right: [[f64; 4]; 2]) -> f64 {
        let projector = |plane: [[f64; 4]; 2]| -> [[f64; 4]; 4] {
            core::array::from_fn(|row| {
                core::array::from_fn(|column| {
                    plane[0][row].mul_add(plane[0][column], plane[1][row] * plane[1][column])
                })
            })
        };
        projector(left)
            .into_iter()
            .flatten()
            .zip(projector(right).into_iter().flatten())
            .map(|(left, right)| (left - right) * (left - right))
            .sum::<f64>()
            .sqrt()
            / core::f64::consts::SQRT_2
    }

    #[test]
    fn object_mapping_tracks_unrounded_math_within_one_turn_quantum() {
        assert_eq!(
            TURN_QUANTUM_RADIANS * 512.0,
            TURN_QUANTUM_WIDTH_1024_EDGE_PIXELS
        );
        for object in sampled_objects() {
            let exact =
                rebuild_basis(&orientation_from_object(&object).expect("mapped orientation"))
                    .expect("exact basis");
            let residual =
                projector_residual([exact.u, exact.v], unrounded_mathematical_plane(object));
            assert!(
                residual <= TURN_QUANTUM_RADIANS,
                "projector residual {residual:e} exceeds one turn quantum"
            );
        }
    }

    #[test]
    fn legacy_binary32_plane_has_a_separate_migration_bound() {
        for object in sampled_objects() {
            let orientation = orientation_from_object(&object).expect("mapped orientation");
            let exact = rebuild_basis(&orientation).expect("exact basis");
            let legacy = construct_plane(object).expect("legacy plane");
            let legacy_f64 = [legacy.basis_u.map(f64::from), legacy.basis_v.map(f64::from)];
            let residual = projector_residual([exact.u, exact.v], legacy_f64);
            assert!(
                residual <= LEGACY_F32_PROJECTOR_MIGRATION_BOUND_RADIANS,
                "legacy projector residual {residual:e} exceeds the migration bound"
            );
            let published = plane_from_orientation(&orientation).expect("camera plane boundary");
            assert_eq!(
                published.basis_u,
                exact
                    .u
                    .map(|value| binary32_from_binary64(value).expect("binary32 boundary"))
            );
            assert_eq!(
                published.basis_v,
                exact
                    .v
                    .map(|value| binary32_from_binary64(value).expect("binary32 boundary"))
            );
        }
    }

    #[test]
    fn exact_plane_identity_matches_the_frozen_rotation_oracle() {
        let general = ObjectAngles {
            rho_12: 0.2,
            rho_13: -0.4,
            rho_14: 0.1,
            rho_23: 0.25,
            rho_24: -0.3,
            rho_34: 0.15,
        };
        let cases = [
            (
                ObjectAngles::IDENTITY,
                ObjectAngles {
                    rho_34: 0.3,
                    ..ObjectAngles::IDENTITY
                },
            ),
            (
                ObjectAngles::IDENTITY,
                ObjectAngles {
                    rho_12: 0.5,
                    ..ObjectAngles::IDENTITY
                },
            ),
            (
                ObjectAngles::IDENTITY,
                ObjectAngles {
                    rho_14: core::f64::consts::PI,
                    ..ObjectAngles::IDENTITY
                },
            ),
            (
                ObjectAngles::IDENTITY,
                ObjectAngles {
                    rho_13: 0.3,
                    ..ObjectAngles::IDENTITY
                },
            ),
            (
                general,
                ObjectAngles {
                    rho_34: general.rho_34 + 0.3,
                    ..general
                },
            ),
        ];
        for (retained, requested) in cases {
            let expected = ember_julibrot_math::object_plane_relation(retained, requested)
                .expect("legacy plane relation")
                .is_some();
            let retained = orientation_from_object(&retained).expect("retained orientation");
            let requested = orientation_from_object(&requested).expect("requested orientation");
            assert_eq!(
                ember_camera::same_image_plane(&retained, &requested)
                    .expect("exact plane relation"),
                expected
            );
        }
    }

    #[test]
    fn launcher_angle_rows_round_trip_bit_exactly() {
        for object in [ObjectAngles::IDENTITY, ObjectAngles::JULIA] {
            let orientation = orientation_from_object(&object).expect("mapped orientation");
            let restored = object_from_orientation(&orientation).expect("saved angle readout");
            assert_eq!(
                restored.as_array().map(f64::to_bits),
                object.as_array().map(f64::to_bits)
            );
        }
    }

    #[test]
    fn saved_angle_quantum_and_boundary_canonicalisation_are_explicit() {
        let quantum_aligned = ObjectAngles {
            rho_12: turn_to_radians(Turn::from_bits(0x0123_4567)),
            ..ObjectAngles::IDENTITY
        };
        let restored = object_from_orientation(
            &orientation_from_object(&quantum_aligned).expect("quantum-aligned orientation"),
        )
        .expect("quantum-aligned saved row");
        assert_eq!(
            restored.as_array().map(f64::to_bits),
            quantum_aligned.as_array().map(f64::to_bits)
        );

        let negative_zero = ObjectAngles {
            rho_12: -0.0,
            ..ObjectAngles::IDENTITY
        };
        let zero_orientation = orientation_from_object(&negative_zero).expect("zero orientation");
        assert_eq!(
            zero_orientation,
            orientation_from_object(&ObjectAngles::IDENTITY).expect("identity orientation")
        );
        let canonical_zero =
            object_from_orientation(&zero_orientation).expect("canonical zero row");
        assert_eq!(canonical_zero.rho_12.to_bits(), 0.0_f64.to_bits());

        let positive_half_turn = ObjectAngles {
            rho_13: core::f64::consts::PI,
            ..ObjectAngles::IDENTITY
        };
        let negative_half_turn = ObjectAngles {
            rho_13: -core::f64::consts::PI,
            ..ObjectAngles::IDENTITY
        };
        let positive_orientation =
            orientation_from_object(&positive_half_turn).expect("positive half-turn orientation");
        let negative_orientation =
            orientation_from_object(&negative_half_turn).expect("negative half-turn orientation");
        assert_eq!(positive_orientation, negative_orientation);
        let canonical_positive =
            object_from_orientation(&positive_orientation).expect("canonical positive half-turn");
        let canonical_negative =
            object_from_orientation(&negative_orientation).expect("canonical negative half-turn");
        assert_eq!(
            canonical_positive.as_array().map(f64::to_bits),
            canonical_negative.as_array().map(f64::to_bits)
        );
        assert_eq!(
            orientation_from_object(&canonical_positive).expect("stable canonical half-turn"),
            positive_orientation
        );

        let half_turn = Turn::from_bits(0x8000_0000);
        assert_eq!(
            Turn::from_radians(core::f64::consts::PI).expect("positive pi"),
            half_turn
        );
        assert_eq!(
            Turn::from_radians(-core::f64::consts::PI).expect("negative pi"),
            half_turn
        );
        assert_eq!(
            turn_to_radians(half_turn).to_bits(),
            (-core::f64::consts::PI).to_bits()
        );
    }

    #[test]
    fn a_canonical_saved_angle_set_is_stable_after_first_quantisation() {
        let input = ObjectAngles {
            rho_12: 0.2,
            rho_13: -0.4,
            rho_14: 0.1,
            rho_23: 0.25,
            rho_24: -0.3,
            rho_34: 0.15,
        };
        let orientation = orientation_from_object(&input).expect("mapped saved row");
        let saved = object_from_orientation(&orientation).expect("canonical saved row");
        let reloaded = orientation_from_object(&saved).expect("reloaded saved row");
        let restored = object_from_orientation(&reloaded).expect("restored saved row");
        assert_eq!(reloaded, orientation);
        assert_eq!(
            restored.as_array().map(f64::to_bits),
            saved.as_array().map(f64::to_bits)
        );
    }

    #[test]
    fn zoom_quantisation_is_nearest_even_on_both_sides_of_every_tie() {
        let quantum = f64::from(EXPONENT_QUANTA_PER_OCTAVE);
        let lower_even = 10.5 / quantum;
        let upper_even = 11.5 / quantum;
        assert_eq!(
            quantize_zoom_log2(lower_even.next_down())
                .expect("below positive even tie")
                .quanta(),
            10
        );
        assert_eq!(
            quantize_zoom_log2(lower_even).expect("lower tie").quanta(),
            10
        );
        assert_eq!(
            quantize_zoom_log2(lower_even.next_up())
                .expect("above positive even tie")
                .quanta(),
            11
        );
        assert_eq!(
            quantize_zoom_log2(upper_even.next_down())
                .expect("below positive odd tie")
                .quanta(),
            11
        );
        assert_eq!(
            quantize_zoom_log2(upper_even).expect("upper tie").quanta(),
            12
        );
        assert_eq!(
            quantize_zoom_log2(upper_even.next_up())
                .expect("above positive odd tie")
                .quanta(),
            12
        );

        let negative_lower_even = -lower_even;
        let negative_upper_even = -upper_even;
        assert_eq!(
            quantize_zoom_log2(negative_lower_even.next_down())
                .expect("below negative even tie")
                .quanta(),
            -11
        );
        assert_eq!(
            quantize_zoom_log2(negative_lower_even)
                .expect("negative even tie")
                .quanta(),
            -10
        );
        assert_eq!(
            quantize_zoom_log2(negative_lower_even.next_up())
                .expect("above negative even tie")
                .quanta(),
            -10
        );
        assert_eq!(
            quantize_zoom_log2(negative_upper_even.next_down())
                .expect("below negative odd tie")
                .quanta(),
            -12
        );
        assert_eq!(
            quantize_zoom_log2(negative_upper_even)
                .expect("negative odd tie")
                .quanta(),
            -12
        );
        assert_eq!(
            quantize_zoom_log2(negative_upper_even.next_up())
                .expect("above negative odd tie")
                .quanta(),
            -11
        );
    }

    #[test]
    fn absolute_zoom_rejects_raw_values_outside_the_closed_camera_range() {
        let minimum = f64::from(MIN_EXPONENT_QUANTA) / f64::from(EXPONENT_QUANTA_PER_OCTAVE);
        let maximum = f64::from(MAX_EXPONENT_QUANTA) / f64::from(EXPONENT_QUANTA_PER_OCTAVE);
        assert_eq!(
            quantize_zoom_log2(minimum)
                .expect("minimum endpoint")
                .quanta(),
            MIN_EXPONENT_QUANTA
        );
        assert_eq!(
            quantize_zoom_log2(minimum.next_up())
                .expect("inside minimum endpoint")
                .quanta(),
            MIN_EXPONENT_QUANTA
        );
        assert!(quantize_zoom_log2(minimum.next_down()).is_err());
        assert_eq!(
            quantize_zoom_log2(maximum.next_down())
                .expect("inside maximum endpoint")
                .quanta(),
            MAX_EXPONENT_QUANTA
        );
        assert_eq!(
            quantize_zoom_log2(maximum)
                .expect("maximum endpoint")
                .quanta(),
            MAX_EXPONENT_QUANTA
        );
        assert!(quantize_zoom_log2(maximum.next_up()).is_err());
        assert!(quantize_zoom_log2(minimum - 0.5 / 1_024.0).is_err());
        assert!(quantize_zoom_log2(maximum + 0.5 / 1_024.0).is_err());
        assert!(quantize_zoom_log2(f64::NAN).is_err());

        for quanta in ember_camera::MIN_EXPONENT_QUANTA..=ember_camera::MAX_EXPONENT_QUANTA {
            let exponent = Exponent::new(quanta).expect("in-range exponent");
            let displayed = zoom_log2_from_exponent(exponent);
            assert_eq!(
                quantize_zoom_log2(displayed).expect("display round trip"),
                exponent
            );
        }
    }

    #[test]
    fn relative_zoom_quantises_once_without_using_the_absolute_range() {
        assert_eq!(
            quantize_zoom_delta_quanta(-3.0).expect("relative three-octave delta"),
            -3 * EXPONENT_QUANTA_PER_OCTAVE
        );
        assert_eq!(
            quantize_zoom_delta_quanta(0.49 / f64::from(EXPONENT_QUANTA_PER_OCTAVE))
                .expect("sub-half-quantum delta"),
            0
        );
    }

    #[test]
    fn observer_mapping_is_presentation_only_and_relief_independent() {
        let flat = ViewControls::MANDELBROT_FLAT;
        let lifted = ViewControls {
            height_scale: 4.0,
            ..flat
        };
        let camera = View::new(
            [Fixed::ZERO; 4],
            Exponent::ZERO,
            orientation_from_object(&ObjectAngles::IDENTITY).expect("identity mapping"),
        );
        let flat_observer = observer_from_view(&camera, &flat).expect("flat observer");
        assert_eq!(
            observer_from_view(&camera, &lifted).expect("lifted observer"),
            flat_observer
        );
        let screen = Screen::new(960, 540).expect("screen");
        let pixel = [137.0, -64.0];
        let inverted = invert_perspective(&flat_observer, screen, pixel).expect("base plane");
        assert!((inverted[0] - pixel[0]).abs() <= PROJECT_PIXEL_TOLERANCE_PIXELS);
        assert!((inverted[1] - pixel[1]).abs() <= PROJECT_PIXEL_TOLERANCE_PIXELS);
    }

    #[test]
    fn observer_mapping_matches_the_non_neutral_screen_oracle() {
        let object = ObjectAngles::IDENTITY;
        let orientation = orientation_from_object(&object).expect("identity mapping");
        let camera = View::new([Fixed::ZERO; 4], Exponent::ZERO, orientation);
        let mut controls = ViewControls::MANDELBROT_FLAT;
        controls.camera[0] = 0.13;
        controls.camera[8] = -0.21;
        controls.camera_translation = [0.2, -0.1, 0.3, -0.2, 0.15];
        controls.camera_yaw = 0.17;
        controls.camera_pitch = -0.12;
        controls.distance_five = 7.0;
        let screen = Screen::new(1_024, 576).expect("screen");
        let map = screen_to_plane(&object, &controls, 0.0, 1_024, 576, 16.0 / 9.0)
            .expect("legacy screen oracle");
        let observer = observer_from_view(&camera, &controls).expect("two-stage observer");
        for base_pixel in [[0.0; 2], [91.5, -37.25], [-211.0, 83.0]] {
            let expected = plane_to_screen(&map, base_pixel).expect("legacy forward map");
            let actual =
                project_perspective(&observer, screen, base_pixel).expect("camera forward map");
            assert!((actual[0] - expected[0]).abs() <= OBSERVER_ORACLE_TOLERANCE_PIXELS);
            assert!((actual[1] - expected[1]).abs() <= OBSERVER_ORACLE_TOLERANCE_PIXELS);
        }
        for screen_pixel in [[0.0; 2], [137.0, -64.0], [-311.5, 123.25]] {
            let expected = navigation_delta(&map, [0.0; 2], 0.0, screen_pixel)
                .expect("legacy inverse map")
                .anchor_canvas_px;
            let actual =
                invert_perspective(&observer, screen, screen_pixel).expect("camera inverse map");
            assert!((actual[0] - expected[0]).abs() <= OBSERVER_ORACLE_TOLERANCE_PIXELS);
            assert!((actual[1] - expected[1]).abs() <= OBSERVER_ORACLE_TOLERANCE_PIXELS);
        }
    }

    #[test]
    fn fixed_centre_bits_survive_the_unchanged_worker_codec() {
        let positive = Fixed::from_le_bytes([
            0x0123_4567_89ab_cdef_u64.to_le_bytes(),
            0xfedc_ba98_7654_3210_u64.to_le_bytes(),
            0x1357_9bdf_2468_ace0_u64.to_le_bytes(),
            0x0f0f_f0f0_55aa_aa55_u64.to_le_bytes(),
            0x0102_0304_0506_0708_u64.to_le_bytes(),
            0x8877_6655_4433_2211_u64.to_le_bytes(),
            0x7fff_ffff_ffff_ffff_u64.to_le_bytes(),
            0x1234_5678_9abc_def0_u64.to_le_bytes(),
        ]);
        let negative = Fixed::from_le_bytes([
            0xfedc_ba98_7654_3211_u64.to_le_bytes(),
            0x0123_4567_89ab_cdef_u64.to_le_bytes(),
            0xeca8_6420_db97_531f_u64.to_le_bytes(),
            0xf0f0_0f0f_aa55_55aa_u64.to_le_bytes(),
            0xfefd_fcfb_faf9_f8f7_u64.to_le_bytes(),
            0x7788_99aa_bbcc_ddee_u64.to_le_bytes(),
            0x8000_0000_0000_0000_u64.to_le_bytes(),
            0xedcb_a987_6543_210f_u64.to_le_bytes(),
        ]);
        let centre = [
            positive,
            negative,
            Fixed::ZERO,
            Fixed::from_f64(-2.5).expect("fixed"),
        ];
        let math = big_centre_from_fixed(&centre).expect("full-width publication");
        assert_eq!(math.precision_bits, CAMERA_PRECISION_BITS);
        let encoded = EncodedCentre::encode_math(&math, 19).expect("worker encoding");
        let decoded = encoded
            .decode_math(CAMERA_PRECISION_BITS)
            .expect("worker decoding");
        assert_eq!(
            fixed_centre_from_big(&decoded).expect("fixed decoding"),
            centre
        );
        assert_eq!(encoded.revision, 19);
    }

    #[test]
    fn integer_identity_navigation_agrees_before_the_old_path_is_removed() {
        let object = ObjectAngles::IDENTITY;
        let controls = ViewControls::MANDELBROT_FLAT;
        let map = screen_to_plane(&object, &controls, 0.0, 1_024, 512, 2.0)
            .expect("canonical screen map");
        assert_eq!(map, ember_julibrot_math::Homography::IDENTITY);
        let plane = construct_plane(object).expect("legacy plane");
        let orientation = orientation_from_object(&object).expect("mapped orientation");
        let basis = rebuild_basis(&orientation).expect("mapped basis");
        assert_eq!(basis.u, [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(basis.v, [0.0, 0.0, 0.0, 1.0]);
        let screen = Screen::new(1_024, 512).expect("screen");
        let anchor = [256.0, -128.0];

        let delta =
            navigation_delta(&map, [32.0, 16.0], 1.0, anchor).expect("identity navigation map");
        assert_eq!(delta.pan_canvas_px, [32.0, -16.0]);
        let mut legacy =
            BigCentre::from_f64([0.0; 4], CAMERA_PRECISION_BITS).expect("legacy centre");
        legacy
            .apply_navigation(&delta, &plane, 0.0, 1.0, screen.width())
            .expect("legacy navigation");
        let mut exact = View::new([Fixed::ZERO; 4], Exponent::ZERO, orientation);
        zoom_about(&mut exact, screen, anchor, EXPONENT_QUANTA_PER_OCTAVE).expect("exact zoom");
        pan(
            &mut exact,
            screen,
            delta.pan_canvas_px[0],
            delta.pan_canvas_px[1],
        )
        .expect("exact pan");
        assert_eq!(
            big_centre_from_fixed(&exact.centre).expect("published exact centre"),
            legacy
        );

        let point = click(
            &View::new([Fixed::ZERO; 4], Exponent::ZERO, orientation),
            screen,
            anchor,
        )
        .expect("exact click");
        let mut legacy_point =
            BigCentre::from_f64([0.0; 4], CAMERA_PRECISION_BITS).expect("legacy point");
        legacy_point
            .apply_navigation(
                &NavigationDelta {
                    pan_canvas_px: [-anchor[0], -anchor[1]],
                    ..NavigationDelta::default()
                },
                &plane,
                0.0,
                0.0,
                screen.width(),
            )
            .expect("legacy click");
        assert_eq!(
            big_centre_from_fixed(&point).expect("published exact point"),
            legacy_point
        );
    }
}
