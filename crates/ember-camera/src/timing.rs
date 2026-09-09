use crate::{
    CameraError, EXPONENT_QUANTA_PER_OCTAVE, Exponent, Fixed, Orientation, Screen, Turn, View,
    reference_displacement, rotate_about, zoom_about,
};
use std::hint::black_box;
use std::io::Write;
use std::time::{Duration, Instant};

/// Width pinned by the first consumer and therefore measured by the release timing contract.
const TIMING_LIMBS: usize = 8;

/// Ambient dimension of the first consumer and the largest initial camera workload.
const TIMING_DIMENSIONS: usize = 5;

/// Combined release ceiling for one worst-case edit and one per-frame displacement.
const RELEASE_TIMING_BOUND: Duration = Duration::from_millis(1);

#[test]
fn timing_fractional_rotation_edit_and_reference_displacement() -> Result<(), CameraError> {
    let screen = Screen::new(1_920, 1_080)?;
    let mut angles = [[Turn::ZERO; TIMING_DIMENSIONS]; TIMING_DIMENSIONS];
    angles[0][1] = Turn::from_bits(0x0123_4567);
    angles[0][2] = Turn::from_bits(0x1234_5678);
    angles[0][3] = Turn::from_bits(0x2345_6789);
    angles[0][4] = Turn::from_bits(0x3456_789a);
    angles[1][2] = Turn::from_bits(0x4567_89ab);
    angles[1][3] = Turn::from_bits(0x5678_9abc);
    angles[1][4] = Turn::from_bits(0x6789_abcd);
    angles[2][3] = Turn::from_bits(0x789a_bcde);
    angles[2][4] = Turn::from_bits(0x89ab_cdef);
    angles[3][4] = Turn::from_bits(0x9abc_def0);
    let rotation = Orientation::new(angles)?;
    let mut view = View::new(
        [Fixed::<TIMING_LIMBS>::ZERO; TIMING_DIMENSIONS],
        Exponent::new(60 * EXPONENT_QUANTA_PER_OCTAVE)?,
        Orientation::IDENTITY,
    );
    let reference = view.centre;
    let anchor = [713.25, -401.5];

    let edit_started = Instant::now();
    zoom_about(&mut view, screen, anchor, EXPONENT_QUANTA_PER_OCTAVE - 1)?;
    rotate_about(&mut view, screen, anchor, &rotation)?;
    black_box(&view);
    let edit_wall = edit_started.elapsed();

    let displacement_started = Instant::now();
    let displacement = reference_displacement(&view, &reference, screen)?;
    black_box(displacement);
    let displacement_wall = displacement_started.elapsed();
    let combined_wall = edit_wall
        .checked_add(displacement_wall)
        .ok_or(CameraError::Overflow)?;

    let report_written = writeln!(
        std::io::stderr().lock(),
        "camera timing: edit_ns={}, reference_displacement_ns={}, combined_ns={}, bound_ns={}",
        edit_wall.as_nanos(),
        displacement_wall.as_nanos(),
        combined_wall.as_nanos(),
        RELEASE_TIMING_BOUND.as_nanos()
    )
    .is_ok();
    assert!(report_written, "timing result could not be written");

    #[cfg(not(debug_assertions))]
    assert!(
        combined_wall < RELEASE_TIMING_BOUND,
        "camera edit and displacement took {combined_wall:?}, bound {RELEASE_TIMING_BOUND:?}"
    );
    Ok(())
}
