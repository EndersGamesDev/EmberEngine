use core::mem::{align_of, offset_of, size_of};
use core::num::NonZeroU32;

use ember_julibrot_math::{
    BigCentre, CentreSplit, EscapeGridRecord, EscapeParams, Homography, MathError, NavigationDelta,
    OrbitStep, Plane, PlaneAngles, Pose, ReferenceOrbitBuilder, ReferenceOrbitRecord, ViewControls,
    centre_from_reference_px, construct_plane, navigation_delta, pixel_scale, precision_for,
    reference_shift_px, scaled_pixel_scale, screen_to_plane,
};

type ApplyNavigationFn =
    fn(&mut BigCentre, &NavigationDelta, &Plane, f64, f64, u32) -> Result<(), MathError>;
type DisplacementFn = fn(&BigCentre, &BigCentre, &Plane, f64) -> Result<[f64; 2], MathError>;
type NavigationMapFn =
    fn(&Homography, [f64; 2], f64, [f64; 2]) -> Result<NavigationDelta, MathError>;

#[test]
fn shared_gpu_and_owner_discriminants_are_exact() {
    assert_eq!(size_of::<Plane>(), 32);
    assert_eq!(align_of::<Plane>(), 16);
    assert_eq!(offset_of!(Plane, basis_u), 0);
    assert_eq!(offset_of!(Plane, basis_v), 16);
    assert_eq!(size_of::<CentreSplit>(), 32);
    assert_eq!(align_of::<CentreSplit>(), 16);
    assert_eq!(offset_of!(CentreSplit, hi), 0);
    assert_eq!(offset_of!(CentreSplit, lo), 16);
    assert_eq!(size_of::<EscapeParams>(), 8);
    assert_eq!(offset_of!(EscapeParams, max_iter), 0);
    assert_eq!(offset_of!(EscapeParams, bailout), 4);
    assert_eq!(size_of::<ReferenceOrbitRecord>(), 8);
    assert_eq!(offset_of!(ReferenceOrbitRecord, re), 0);
    assert_eq!(offset_of!(ReferenceOrbitRecord, im), 4);
    assert_eq!(size_of::<EscapeGridRecord>(), 16);
    assert_eq!(offset_of!(EscapeGridRecord, smooth_iter), 0);
    assert_eq!(offset_of!(EscapeGridRecord, escaped), 4);
    assert_eq!(offset_of!(EscapeGridRecord, rebase_count), 8);
    assert_eq!(offset_of!(EscapeGridRecord, status), 12);
    assert_eq!(
        ViewControls::NEUTRAL.as_array(),
        [0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0]
    );
    assert!(ViewControls::NEUTRAL.is_valid());
    assert!(
        !ViewControls {
            height_scale: -0.001,
            ..ViewControls::NEUTRAL
        }
        .is_valid()
    );
    assert!(
        !ViewControls {
            distance_five: 0.0,
            ..ViewControls::NEUTRAL
        }
        .is_valid()
    );
}

#[test]
fn app_facing_function_signatures_stay_stable() {
    let _: fn(PlaneAngles) -> Result<Plane, MathError> = construct_plane;
    let _: fn(f64, u32) -> Result<ember_julibrot_math::ScaledPixelScale, MathError> =
        scaled_pixel_scale;
    let _: fn(f64, u32, u32) -> Result<ember_julibrot_math::PrecisionPlan, MathError> =
        precision_for;
    let _: fn(&Pose, &Pose) -> Result<ember_julibrot_math::WarpMatrix, MathError> =
        ember_julibrot_math::warp_matrix;
    let _: fn(f64, u32) -> Result<f64, MathError> = pixel_scale;
    let _: fn(&ViewControls, f64, u32, u32, f64) -> Result<Homography, MathError> = screen_to_plane;
    let _: NavigationMapFn = navigation_delta;
    let _: ApplyNavigationFn = BigCentre::apply_navigation;
    let _: DisplacementFn = BigCentre::displacement_px;
    let _: fn(&BigCentre) -> [f64; 4] = BigCentre::to_f64_mirror;
}

#[test]
fn shallow_deep_policy_boundary_has_representable_scaled_offsets() -> Result<(), MathError> {
    let below = scaled_pixel_scale(13.999, 4096)?;
    let at = scaled_pixel_scale(14.0, 4096)?;
    assert!((0.5..1.0).contains(&below.mantissa));
    assert!((0.5..1.0).contains(&at.mantissa));
    assert_eq!(at.mantissa, 0.5);
    assert_eq!(at.exponent, -23);
    Ok(())
}

#[test]
fn worker_can_drive_the_cooperative_orbit_contract() -> Result<(), MathError> {
    let centre = BigCentre::from_f64([0.0, 0.0, 2.0, 0.0], 192)?;
    let plan = precision_for(40.0, 1920, 64)?;
    let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(64))?;
    let chunk = NonZeroU32::new(1).ok_or(MathError::InvalidMaxIter)?;
    assert_eq!(builder.step(chunk)?, OrbitStep::Pending { stored: 1 });
    assert_eq!(builder.step(chunk)?, OrbitStep::Pending { stored: 2 });
    assert_eq!(builder.step(chunk)?, OrbitStep::Pending { stored: 3 });
    let OrbitStep::Complete(orbit) = builder.step(chunk)? else {
        return Err(MathError::InvalidOrbitState);
    };
    assert_eq!(orbit.length, 4);
    assert_eq!(orbit.records.len(), 4);
    assert_eq!(orbit.escape_index, Some(3));
    assert_eq!(usize::try_from(orbit.length), Ok(orbit.records.len()));
    Ok(())
}

#[test]
fn worker_displacements_have_the_ruled_directions() -> Result<(), MathError> {
    let old = BigCentre::from_f64([0.0; 4], 256)?;
    let new = BigCentre::from_f64([0.25, -0.5, 0.0, 0.0], 256)?;
    let quarter = core::f64::consts::FRAC_PI_2;
    let plane = construct_plane(PlaneAngles {
        theta_1: -quarter,
        theta_2: -quarter,
    })?;
    let desired = centre_from_reference_px(&new, &old, &plane, 0.0, 4)?;
    let shift = reference_shift_px(&old, &new, &plane, 0.0, 4)?;
    assert_eq!(desired, [0.25, -0.5]);
    assert_eq!(shift, desired);
    Ok(())
}
