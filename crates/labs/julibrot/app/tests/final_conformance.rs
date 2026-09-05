//! Final exact-versus-fast cross-slice conformance corpus.

#![allow(
    clippy::float_cmp,
    reason = "the corpus pins exact policy and category values"
)]

use core::num::NonZeroU32;

use ember_julibrot_kernels::{
    ConformanceVerdict, DETERMINISTIC_VERIFICATION_DIGITS, GridExtent, KernelMode, KernelSample,
    PerturbUniform, RefinementLevel, SampleStatus, ShallowUniform, escape_shallow_pixel,
    escape_shallow_point, evaluate_perturbation_conformance, perturb_scaled_offset,
};
use ember_julibrot_math::{
    BigCentre, EscapeGridRecord, EscapeParams, Homography, MathError, ObjectAngles, OrbitStep,
    PerturbationEnvelope, Plane, PlaneAngles, Pose, PoseMap, PrecisionMode, ReferenceOrbitBuilder,
    ReferenceOrbitRecord, ReferencePass, ScaleSplit, ViewControls, construct_plane,
    perturb_scaled_f64_with_envelope, precision_for, scale_split, scaled_pixel_offset,
    shallow_pixel_scale, split_centre, warp_matrix,
};
use ember_julibrot_present::{
    DEBUG_TINT, GLITCH_DIAGNOSTIC, PaletteId, PaletteRecord, apply_homography,
    pack_homography_rows, palette, shade_lit_escape_record,
};
use ember_lab_julibrot::preset_row;

const GRID_WIDTH: u32 = 960;
const GRID_HEIGHT: u32 = 540;
const SAMPLE_EXTENT: GridExtent = GridExtent {
    width: 3,
    height: 3,
};
const ITERATION_CAPS: [u32; 4] = [64, 256, 512, 4_096];
const PALETTES: [PaletteId; 3] = [PaletteId::Classic, PaletteId::Ember, PaletteId::Ice];
const SAMPLE_PIXELS: [[u32; 2]; 5] = [[0, 0], [2, 0], [1, 1], [0, 2], [2, 2]];
const LIGHT: f32 = 0.7;

const ZERO_RECORD: ReferenceOrbitRecord = ReferenceOrbitRecord { re: 0.0, im: 0.0 };

#[derive(Clone, Copy, Debug)]
struct PlaneFixture {
    name: &'static str,
    angles: ObjectAngles,
    interior_centre: [f64; 4],
    escaped_centre: [f64; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalClass {
    Escaped,
    Interior,
    Debug,
}

#[derive(Clone, Copy, Debug)]
struct Rendered {
    sample: KernelSample,
    envelope: Option<PerturbationEnvelope>,
}

#[derive(Clone, Copy, Debug)]
struct DeepFixture<'a> {
    name: &'static str,
    orbit: &'a [ReferenceOrbitRecord],
    offset: [f32; 4],
    exponent: i32,
    cap: u32,
    expected_rebases: f32,
}

#[test]
fn final_image_corpus_meets_the_picture_contract() -> Result<(), MathError> {
    let (highest_zoom, highest_plan) = highest_admitted_plan()?;
    assert!((893.0..894.0).contains(&highest_zoom));
    assert_eq!(
        highest_plan.working_digits + DETERMINISTIC_VERIFICATION_DIGITS,
        highest_plan.policy_digits
    );
    assert!(matches!(
        deterministic_precision_for(
            f64::from_bits(highest_zoom.to_bits() + 1),
            GRID_WIDTH,
            4_096
        ),
        Err(MathError::PrecisionExhausted { .. })
    ));
    let zooms = [13.999, 14.0, 40.0, 80.0, 100.0, 256.0, 512.0, highest_zoom];
    let mut comparisons = 0_usize;
    let mut eligible = 0_usize;
    let mut shared_shallow_checks = 0_usize;
    for zoom in zooms {
        for cap in ITERATION_CAPS {
            for fixture in plane_fixtures() {
                let plane = construct_plane(fixture.angles)?;
                for centre in [fixture.interior_centre, fixture.escaped_centre] {
                    for pixel in SAMPLE_PIXELS {
                        if zoom < 14.0 {
                            let shared = render_shallow(plane, centre, zoom, cap, pixel)?;
                            assert!(shared.envelope.is_none());
                            shared_shallow_checks += 1;
                            continue;
                        }
                        let exact = render(
                            PrecisionMode::Deterministic,
                            plane,
                            centre,
                            zoom,
                            cap,
                            pixel,
                        )?;
                        let fast =
                            render(PrecisionMode::PictureFast, plane, centre, zoom, cap, pixel)?;
                        let outside = exact.envelope.is_none_or(|envelope| {
                            envelope.minimum_escape_margin > envelope.escape_norm2_error
                        });
                        if outside {
                            eligible += 1;
                            assert_eq!(
                                terminal(fast.sample),
                                terminal(exact.sample),
                                "{} zoom={zoom} cap={cap} centre={centre:?} pixel={pixel:?}",
                                fixture.name
                            );
                            assert_eq!(
                                fast.sample.escape_index, exact.sample.escape_index,
                                "{} zoom={zoom} cap={cap} centre={centre:?} pixel={pixel:?}",
                                fixture.name
                            );
                        }
                        for palette_id in PALETTES {
                            assert_picture_colour(exact.sample, fast.sample, palette(palette_id));
                            comparisons += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 7 * 4 * 3 * 2 * 5 * 3);
    assert_eq!(shared_shallow_checks, 4 * 3 * 2 * 5);
    assert!(eligible > 0);
    Ok(())
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keeping the five deep fixtures beside both inherited boundary laws makes omissions visible"
)]
fn forced_rescale_rebase_and_boundary_fixtures_remain_explicit() {
    assert_eq!(KernelMode::for_zoom(13.999), KernelMode::Shallow);
    assert_eq!(KernelMode::for_zoom(14.0), KernelMode::Perturbation);
    let bailout = shallow_bailout_boundary();
    assert_eq!(bailout.escape_index, None);
    assert_eq!(bailout.record.smooth_iter, -1.0);

    let escaped_orbit = [
        ZERO_RECORD,
        ReferenceOrbitRecord {
            re: 2.0,
            ..ZERO_RECORD
        },
        ReferenceOrbitRecord {
            re: 6.0,
            ..ZERO_RECORD
        },
        ReferenceOrbitRecord {
            re: 38.0,
            ..ZERO_RECORD
        },
    ];
    let one_orbit = [ReferenceOrbitRecord {
        re: 1.0,
        ..ZERO_RECORD
    }; 4];
    let zero_orbit = [ZERO_RECORD; 2];
    let cases = [
        DeepFixture {
            name: "upward",
            orbit: &zero_orbit,
            offset: [2.0_f32.powi(80), 0.0, 0.0, 0.0],
            exponent: -80,
            cap: 2,
            expected_rebases: 0.0,
        },
        DeepFixture {
            name: "downward",
            orbit: &zero_orbit,
            offset: [2.0_f32.powi(-80), 0.0, 0.0, 0.0],
            exponent: 80,
            cap: 2,
            expected_rebases: 0.0,
        },
        DeepFixture {
            name: "zero-rebase",
            orbit: &escaped_orbit,
            offset: [0.0; 4],
            exponent: -900,
            cap: 4,
            expected_rebases: 0.0,
        },
        DeepFixture {
            name: "nonzero-rebase",
            orbit: &one_orbit[..2],
            offset: [-0.75, 0.0, 0.0, 0.0],
            exponent: 0,
            cap: 2,
            expected_rebases: 1.0,
        },
        DeepFixture {
            name: "repeated-rebase",
            orbit: &one_orbit,
            offset: [-0.75, 0.0, 0.0, 0.0],
            exponent: 0,
            cap: 4,
            expected_rebases: 3.0,
        },
    ];
    for case in cases {
        let rendered = render_special(case.orbit, case.offset, case.exponent, case.cap);
        assert_eq!(
            rendered.sample.record.rebase_count, case.expected_rebases,
            "{}",
            case.name
        );
        let envelope = rendered.envelope.expect("deep fixture carries an envelope");
        assert!(envelope.escape_norm2_error.is_finite(), "{}", case.name);
    }

    let observed = KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: 0.0,
            status: SampleStatus::Sampled.as_f32(),
        },
        escape_index: None,
    };
    let expected = ember_julibrot_math::PerturbSample {
        smooth_iter: -1.0,
        escaped: true,
        escape_index: Some(4),
        rebase_count: 0,
        glitch: false,
    };
    let boundary = evaluate_perturbation_conformance(
        PrecisionMode::Deterministic,
        observed,
        expected,
        PerturbationEnvelope {
            delta_abs_error: 1.0,
            escape_norm2_error: 1.0,
            smooth_error: 0.0,
            minimum_escape_margin: 0.5,
        },
    );
    assert_eq!(boundary.verdict, ConformanceVerdict::Boundary);
    assert!(boundary.boundary);
}

#[test]
fn policy_independent_terminal_colours_clear_and_warp_are_pinned() -> Result<(), MathError> {
    for selected in PALETTES.map(palette) {
        for record in [[-1.0, 0.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 1.0]] {
            assert_eq!(category_colour(record, selected)[3], 1.0);
        }
        assert_eq!(clear_colour(selected)[3], 1.0);
        assert_eq!(
            category_colour([-1.0, 0.0, 0.0, 1.0], selected),
            GLITCH_DIAGNOSTIC
        );
        assert_ne!(category_colour([-1.0, 0.0, 0.0, 1.0], selected), DEBUG_TINT);
    }

    let (highest_zoom, _) = highest_admitted_plan()?;
    for zoom in [13.999, 14.0, 40.0, 80.0, 100.0, 256.0, 512.0, highest_zoom] {
        for fixture in plane_fixtures() {
            let plane = construct_plane(fixture.angles)?;
            let rows = warp_rows(plane, zoom)?;
            for point in [[-1.0, -1.0], [0.0, 0.0], [1.0, 1.0], [0.75, -0.5]] {
                let source = apply_homography(rows, point).expect("shared warp is finite");
                assert!(source.into_iter().all(f64::is_finite));
            }
        }
    }
    Ok(())
}

fn highest_admitted_plan() -> Result<(f64, ember_julibrot_math::PrecisionPlan), MathError> {
    let mut accepted_bits = 0.0_f64.to_bits();
    let mut refused_bits = 1_024.0_f64.to_bits();
    while accepted_bits + 1 < refused_bits {
        let candidate_bits = accepted_bits + (refused_bits - accepted_bits) / 2;
        match deterministic_precision_for(f64::from_bits(candidate_bits), GRID_WIDTH, 4_096) {
            Ok(_) => accepted_bits = candidate_bits,
            Err(MathError::PrecisionExhausted { .. }) => refused_bits = candidate_bits,
            Err(error) => return Err(error),
        }
    }
    let zoom = f64::from_bits(accepted_bits);
    let plan = deterministic_precision_for(zoom, GRID_WIDTH, 4_096)?;
    assert!(matches!(
        deterministic_precision_for(f64::from_bits(refused_bits), GRID_WIDTH, 4_096),
        Err(MathError::PrecisionExhausted { .. })
    ));
    Ok((zoom, plan))
}

fn deterministic_precision_for(
    zoom: f64,
    grid_width: u32,
    max_iter: u32,
) -> Result<ember_julibrot_math::PrecisionPlan, MathError> {
    let plan = precision_for(zoom, grid_width, max_iter)?;
    let requested_digits = plan
        .working_digits
        .checked_add(DETERMINISTIC_VERIFICATION_DIGITS)
        .ok_or(MathError::CounterOverflow)?;
    if requested_digits > plan.policy_digits {
        return Err(MathError::PrecisionExhausted {
            requested_digits,
            policy_digits: plan.policy_digits,
        });
    }
    Ok(plan)
}

fn plane_fixtures() -> [PlaneFixture; 3] {
    let mandelbrot = preset_row(0).expect("the canonical Mandelbrot row exists");
    let julia = preset_row(1).expect("the canonical Julia row exists");
    [
        PlaneFixture {
            name: mandelbrot.name,
            angles: mandelbrot.object_angles,
            interior_centre: mandelbrot.plane_origin,
            escaped_centre: [0.0, 0.0, 2.0, 0.0],
        },
        PlaneFixture {
            name: julia.name,
            angles: julia.object_angles,
            interior_centre: julia.plane_origin,
            escaped_centre: [20.0, 0.0, julia.plane_origin[2], julia.plane_origin[3]],
        },
        PlaneFixture {
            name: "Hybrid",
            angles: ObjectAngles::from(PlaneAngles {
                theta_1: 0.4,
                theta_2: 0.7,
            }),
            interior_centre: [0.0; 4],
            escaped_centre: [0.0, 0.0, 2.0, 0.0],
        },
    ]
}

fn shallow_bailout_boundary() -> KernelSample {
    escape_shallow_point([16.0, 0.0, 0.0, 0.0], EscapeParams::new(1))
        .expect("the policy-independent shallow boundary is valid")
}

fn render(
    mode: PrecisionMode,
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    if zoom < 14.0 {
        return render_shallow(plane, centre, zoom, cap, pixel);
    }
    match mode {
        PrecisionMode::Deterministic => render_deterministic(plane, centre, zoom, cap, pixel),
        PrecisionMode::PictureFast => render_picture_fast(plane, centre, zoom, cap, pixel),
    }
}

fn render_shallow(
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    let centre = BigCentre::from_f64(centre, 1_024)?;
    let uniform = ShallowUniform::pack(
        plane,
        &Homography::IDENTITY,
        split_centre(&centre)?,
        shallow_pixel_scale(zoom, GRID_WIDTH)?,
        SAMPLE_EXTENT,
        EscapeParams::new(cap),
        RefinementLevel::Final,
    )
    .expect("the policy-independent corpus shallow uniform is valid");
    let index = pixel[1] * SAMPLE_EXTENT.width + pixel[0];
    let sample =
        escape_shallow_pixel(&uniform, index).expect("the shared corpus pixel is in bounds");
    Ok(Rendered {
        sample,
        envelope: None,
    })
}

fn render_picture_fast(
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    let scale = scale_split(zoom, GRID_WIDTH)?;
    let orbit = picture_fast_reference_orbit(centre, zoom, cap)?;
    let uniform = PerturbUniform::pack(
        plane,
        &Homography::IDENTITY,
        scale,
        SAMPLE_EXTENT,
        EscapeParams::new(cap),
        u32::try_from(orbit.len()).map_err(|_| MathError::OrbitTooLong)?,
        RefinementLevel::Final,
    )
    .expect("the PictureFast corpus perturbation uniform is valid");
    let offset = scaled_pixel_offset(
        plane,
        scale,
        [SAMPLE_EXTENT.width, SAMPLE_EXTENT.height],
        pixel,
    )?;
    let sample = perturb_scaled_offset(&uniform, &orbit, offset)
        .map_err(|_| MathError::InvalidOrbitState)?;
    let (_, envelope) = perturb_scaled_f64_with_envelope(
        &orbit,
        offset.map(f64::from),
        scale.exponent,
        EscapeParams::new(cap),
    )?;
    Ok(Rendered {
        sample,
        envelope: Some(envelope),
    })
}

fn picture_fast_reference_orbit(
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
) -> Result<Vec<ReferenceOrbitRecord>, MathError> {
    let plan = precision_for(zoom, GRID_WIDTH, cap)?;
    let centre = BigCentre::from_f64(centre, 1_024)?;
    let mut builder = ReferenceOrbitBuilder::new_with_policy(
        &centre,
        plan,
        EscapeParams::new(cap),
        PrecisionMode::PictureFast,
        ReferencePass::Preview,
    )?;
    let chunk = NonZeroU32::new(cap).expect("the conformance caps are nonzero");
    loop {
        match builder.step(chunk)? {
            OrbitStep::Pending { .. } => {}
            OrbitStep::Complete(orbit) => return Ok(orbit.records),
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
fn render_deterministic(
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    let scale = scale_split(zoom, GRID_WIDTH)?;
    let orbit = deterministic_reference_orbit(centre, zoom, cap)?;
    let uniform = PerturbUniform::pack(
        plane,
        &Homography::IDENTITY,
        scale,
        SAMPLE_EXTENT,
        EscapeParams::new(cap),
        u32::try_from(orbit.len()).map_err(|_| MathError::OrbitTooLong)?,
        RefinementLevel::Final,
    )
    .expect("the corpus perturbation uniform is valid");
    let offset = scaled_pixel_offset(
        plane,
        scale,
        [SAMPLE_EXTENT.width, SAMPLE_EXTENT.height],
        pixel,
    )?;
    let sample = perturb_scaled_offset(&uniform, &orbit, offset)
        .map_err(|_| MathError::InvalidOrbitState)?;
    let (_, envelope) = perturb_scaled_f64_with_envelope(
        &orbit,
        offset.map(f64::from),
        scale.exponent,
        EscapeParams::new(cap),
    )?;
    Ok(Rendered {
        sample,
        envelope: Some(envelope),
    })
}

fn deterministic_reference_orbit(
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
) -> Result<Vec<ReferenceOrbitRecord>, MathError> {
    let plan = deterministic_precision_for(zoom, GRID_WIDTH, cap)?;
    let centre = BigCentre::from_f64(centre, 1_024)?;
    let mut builder = ReferenceOrbitBuilder::new(&centre, plan, EscapeParams::new(cap))?;
    let chunk = NonZeroU32::new(cap).expect("the conformance caps are nonzero");
    loop {
        match builder.step(chunk)? {
            OrbitStep::Pending { .. } => {}
            OrbitStep::Complete(orbit) => return Ok(orbit.records),
        }
    }
}

fn render_special(
    orbit: &[ReferenceOrbitRecord],
    offset: [f32; 4],
    exponent: i32,
    cap: u32,
) -> Rendered {
    let uniform = PerturbUniform::pack(
        Plane {
            basis_u: [1.0, 0.0, 0.0, 0.0],
            basis_v: [0.0, 1.0, 0.0, 0.0],
        },
        &Homography::IDENTITY,
        ScaleSplit {
            mantissa: 0.5,
            exponent,
        },
        GridExtent {
            width: 1,
            height: 1,
        },
        EscapeParams::new(cap),
        u32::try_from(orbit.len()).expect("fixture orbit length fits u32"),
        RefinementLevel::Final,
    )
    .expect("special fixture uniform is valid");
    let sample = perturb_scaled_offset(&uniform, orbit, offset).expect("special mirror accepts");
    let (_, envelope) = perturb_scaled_f64_with_envelope(
        orbit,
        offset.map(f64::from),
        exponent,
        EscapeParams::new(cap),
    )
    .expect("special oracle accepts");
    Rendered {
        sample,
        envelope: Some(envelope),
    }
}

fn terminal(sample: KernelSample) -> TerminalClass {
    if sample.record.status == SampleStatus::Glitch.as_f32() {
        TerminalClass::Debug
    } else if sample.record.escaped == 1.0 {
        TerminalClass::Escaped
    } else {
        TerminalClass::Interior
    }
}

fn assert_picture_colour(exact: KernelSample, fast: KernelSample, selected: PaletteRecord) {
    let exact_rgba = shade_lit_escape_record(record_lanes(exact), selected, LIGHT).rgba;
    let fast_rgba = shade_lit_escape_record(record_lanes(fast), selected, LIGHT).rgba;
    assert_eq!(fast_rgba[3].to_bits(), exact_rgba[3].to_bits());
    if terminal(exact) == TerminalClass::Escaped {
        for (fast_channel, exact_channel) in rgba8(fast_rgba).into_iter().zip(rgba8(exact_rgba)) {
            assert!(fast_channel.abs_diff(exact_channel) <= 1);
        }
    } else {
        assert_eq!(fast_rgba.map(f32::to_bits), exact_rgba.map(f32::to_bits));
    }
}

const fn record_lanes(sample: KernelSample) -> [f32; 4] {
    let record = sample.record;
    [
        record.smooth_iter,
        record.escaped,
        record.rebase_count,
        record.status,
    ]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rgba8(rgba: [f32; 4]) -> [u8; 4] {
    rgba.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn category_colour(record: [f32; 4], selected: PaletteRecord) -> [f32; 4] {
    shade_lit_escape_record(record, selected, LIGHT).rgba
}

const fn clear_colour(selected: PaletteRecord) -> [f32; 4] {
    selected.clear_rgba
}

fn warp_rows(plane: Plane, zoom: f64) -> Result<[f64; 9], MathError> {
    let from = pose(plane, zoom, [13.25, -7.5]);
    let to = pose(plane, zoom + 0.025, [-4.0, 9.125]);
    let rows =
        pack_homography_rows(warp_matrix(&from, &to)?.forward).ok_or(MathError::DegenerateWarp)?;
    Ok([
        f64::from(rows[0][0]),
        f64::from(rows[0][1]),
        f64::from(rows[0][2]),
        f64::from(rows[1][0]),
        f64::from(rows[1][1]),
        f64::from(rows[1][2]),
        f64::from(rows[2][0]),
        f64::from(rows[2][1]),
        f64::from(rows[2][2]),
    ])
}

const fn pose(plane: Plane, zoom_log2: f64, displacement: [f64; 2]) -> Pose {
    Pose {
        epoch: 1,
        orbit_generation: 1,
        plane,
        object: ObjectAngles::IDENTITY,
        plane_origin: [0.0; 4],
        zoom_log2,
        view: ViewControls::MANDELBROT_FLAT,
        grid_width: GRID_WIDTH,
        grid_height: GRID_HEIGHT,
        map: PoseMap::Mapped(Homography::IDENTITY),
        centre_from_reference_px: displacement,
    }
}
