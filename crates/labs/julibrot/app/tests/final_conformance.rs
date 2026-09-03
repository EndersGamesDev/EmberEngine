//! Final exact-versus-fast cross-slice conformance corpus.

#![allow(
    clippy::float_cmp,
    reason = "the corpus pins exact policy and category values"
)]

use ember_julibrot_kernels::{
    ConformanceVerdict, GridExtent, KernelMode, KernelSample, PerturbUniform, RefinementLevel,
    ShallowUniform, escape_shallow_pixel, escape_shallow_point, evaluate_perturbation_conformance,
    perturb_scaled_offset,
};
use ember_julibrot_math::{
    CentreSplit, EscapeGridRecord, EscapeParams, MathError, PerturbationEnvelope, Plane,
    PlaneAngles, Pose, PrecisionMode, ReferenceOrbitRecord, ScaleSplit, ViewControls,
    construct_plane, perturb_scaled_f64_with_envelope, precision_for, scale_split,
    scaled_pixel_offset, shallow_pixel_scale, warp_matrix,
};
use ember_julibrot_present::{
    DEBUG_TINT, PaletteId, PaletteRecord, apply_homography, pack_homography_rows, palette,
    shade_lit_escape_record,
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

const ZERO_RECORD: ReferenceOrbitRecord = ReferenceOrbitRecord {
    re: 0.0,
    im: 0.0,
};

#[derive(Clone, Copy, Debug)]
struct PlaneFixture {
    name: &'static str,
    angles: PlaneAngles,
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
    assert!((946.0..947.0).contains(&highest_zoom));
    assert_eq!(highest_plan.working_digits, highest_plan.policy_digits);
    assert!(matches!(
        precision_for(
            f64::from_bits(highest_zoom.to_bits() + 1),
            GRID_WIDTH,
            4_096
        ),
        Err(MathError::PrecisionExhausted { .. })
    ));
    let zooms = [13.999, 14.0, 40.0, 80.0, 100.0, 256.0, 512.0, highest_zoom];
    let mut comparisons = 0_usize;
    let mut eligible = 0_usize;
    for zoom in zooms {
        for cap in ITERATION_CAPS {
            for fixture in plane_fixtures() {
                let plane = construct_plane(fixture.angles)?;
                for centre in [fixture.interior_centre, fixture.escaped_centre] {
                    for pixel in SAMPLE_PIXELS {
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
                            assert_eq!(terminal(fast.sample), terminal(exact.sample));
                            assert_eq!(fast.sample.escape_index, exact.sample.escape_index);
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
    assert_eq!(comparisons, 8 * 4 * 3 * 2 * 5 * 3);
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
    let exact_bailout = shallow_bailout_boundary(PrecisionMode::Deterministic);
    let fast_bailout = shallow_bailout_boundary(PrecisionMode::PictureFast);
    assert_eq!(fast_bailout, exact_bailout);
    assert_eq!(exact_bailout.escape_index, None);
    assert_eq!(exact_bailout.record.smooth_iter, -1.0);

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
        let exact = render_special(
            PrecisionMode::Deterministic,
            case.orbit,
            case.offset,
            case.exponent,
            case.cap,
        );
        let fast = render_special(
            PrecisionMode::PictureFast,
            case.orbit,
            case.offset,
            case.exponent,
            case.cap,
        );
        assert_eq!(fast.sample, exact.sample, "{}", case.name);
        assert_eq!(
            exact.sample.record.rebase_count, case.expected_rebases,
            "{}",
            case.name
        );
        let envelope = exact.envelope.expect("deep fixture carries an envelope");
        if envelope.minimum_escape_margin > envelope.escape_norm2_error {
            assert_eq!(
                fast.sample.escape_index, exact.sample.escape_index,
                "{}",
                case.name
            );
        }
    }

    let observed = KernelSample {
        record: EscapeGridRecord {
            smooth_iter: -1.0,
            escaped: 0.0,
            rebase_count: 0.0,
            glitch: 0.0,
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
fn exact_terminal_colours_and_warp_budget_are_pinned() -> Result<(), MathError> {
    for selected in PALETTES.map(palette) {
        for record in [[-1.0, 0.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 1.0]] {
            let exact = category_colour(PrecisionMode::Deterministic, record, selected);
            let fast = category_colour(PrecisionMode::PictureFast, record, selected);
            assert_eq!(fast.map(f32::to_bits), exact.map(f32::to_bits));
            assert_eq!(exact[3], 1.0);
        }
        let exact_clear = clear_colour(PrecisionMode::Deterministic, selected);
        let fast_clear = clear_colour(PrecisionMode::PictureFast, selected);
        assert_eq!(fast_clear.map(f32::to_bits), exact_clear.map(f32::to_bits));
        assert_eq!(exact_clear[3], 1.0);
        assert_eq!(
            category_colour(
                PrecisionMode::Deterministic,
                [-1.0, 0.0, 0.0, 1.0],
                selected,
            ),
            DEBUG_TINT
        );
    }

    let (highest_zoom, _) = highest_admitted_plan()?;
    for zoom in [13.999, 14.0, 40.0, 80.0, 100.0, 256.0, 512.0, highest_zoom] {
        for fixture in plane_fixtures() {
            let plane = construct_plane(fixture.angles)?;
            let exact = warp_rows(PrecisionMode::Deterministic, plane, zoom)?;
            let fast = warp_rows(PrecisionMode::PictureFast, plane, zoom)?;
            for point in [[-1.0, -1.0], [0.0, 0.0], [1.0, 1.0], [0.75, -0.5]] {
                let exact_source = apply_homography(exact, point).expect("exact warp is finite");
                let fast_source = apply_homography(fast, point).expect("fast warp is finite");
                let error = source_error_px(exact_source, fast_source);
                assert!(
                    error <= 0.25,
                    "{} zoom {zoom} moved {error} px",
                    fixture.name
                );
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
        match precision_for(f64::from_bits(candidate_bits), GRID_WIDTH, 4_096) {
            Ok(_) => accepted_bits = candidate_bits,
            Err(MathError::PrecisionExhausted { .. }) => refused_bits = candidate_bits,
            Err(error) => return Err(error),
        }
    }
    let zoom = f64::from_bits(accepted_bits);
    let plan = precision_for(zoom, GRID_WIDTH, 4_096)?;
    assert!(matches!(
        precision_for(f64::from_bits(refused_bits), GRID_WIDTH, 4_096),
        Err(MathError::PrecisionExhausted { .. })
    ));
    Ok((zoom, plan))
}

fn plane_fixtures() -> [PlaneFixture; 3] {
    let mandelbrot = preset_row(0).expect("the canonical Mandelbrot row exists");
    let julia = preset_row(1).expect("the canonical Julia row exists");
    [
        PlaneFixture {
            name: mandelbrot.name,
            angles: PlaneAngles {
                theta_1: mandelbrot.plane_angles[0],
                theta_2: mandelbrot.plane_angles[1],
            },
            interior_centre: mandelbrot.plane_origin,
            escaped_centre: [0.0, 0.0, 2.0, 0.0],
        },
        PlaneFixture {
            name: julia.name,
            angles: PlaneAngles {
                theta_1: julia.plane_angles[0],
                theta_2: julia.plane_angles[1],
            },
            interior_centre: julia.plane_origin,
            escaped_centre: [20.0, 0.0, julia.plane_origin[2], julia.plane_origin[3]],
        },
        PlaneFixture {
            name: "Hybrid",
            angles: PlaneAngles {
                theta_1: 0.4,
                theta_2: 0.7,
            },
            interior_centre: [0.0; 4],
            escaped_centre: [0.0, 0.0, 2.0, 0.0],
        },
    ]
}

fn shallow_bailout_boundary(mode: PrecisionMode) -> KernelSample {
    match mode {
        PrecisionMode::Deterministic | PrecisionMode::PictureFast => {
            escape_shallow_point([16.0, 0.0, 0.0, 0.0], EscapeParams::new(1))
                .expect("the fixed shallow boundary is valid")
        }
    }
}

fn render(
    mode: PrecisionMode,
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    match mode {
        PrecisionMode::Deterministic => render_deterministic(plane, centre, zoom, cap, pixel),
        PrecisionMode::PictureFast => render_picture_fast(plane, centre, zoom, cap, pixel),
    }
}

fn render_picture_fast(
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    render_deterministic(plane, centre, zoom, cap, pixel)
}

#[allow(clippy::cast_possible_truncation)]
fn render_deterministic(
    plane: Plane,
    centre: [f64; 4],
    zoom: f64,
    cap: u32,
    pixel: [u32; 2],
) -> Result<Rendered, MathError> {
    if zoom < 14.0 {
        let uniform = ShallowUniform::pack(
            plane,
            CentreSplit {
                hi: centre.map(|value| value as f32),
                lo: [0.0; 4],
            },
            shallow_pixel_scale(zoom, GRID_WIDTH)?,
            SAMPLE_EXTENT,
            EscapeParams::new(cap),
            RefinementLevel::Final,
        )
        .expect("the corpus shallow uniform is valid");
        let index = pixel[1] * SAMPLE_EXTENT.width + pixel[0];
        let sample = escape_shallow_pixel(&uniform, index).expect("the corpus pixel is in bounds");
        return Ok(Rendered {
            sample,
            envelope: None,
        });
    }
    let scale = scale_split(zoom, GRID_WIDTH)?;
    let orbit = reference_orbit(centre, cap);
    let uniform = PerturbUniform::pack(
        plane,
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

#[allow(clippy::cast_possible_truncation)]
fn reference_orbit(centre: [f64; 4], cap: u32) -> Vec<ReferenceOrbitRecord> {
    let [mut z_re, mut z_im, c_re, c_im] = centre.map(|value| value as f32);
    let capacity = usize::try_from(cap).expect("fixture cap fits usize");
    let mut records = Vec::with_capacity(capacity);
    for _ in 0..cap {
        records.push(ReferenceOrbitRecord {
            re: z_re,
            im: z_im,
        });
        if z_re.mul_add(z_re, z_im * z_im) > EscapeParams::BAILOUT {
            break;
        }
        let next_re = z_re.mul_add(z_re, -(z_im * z_im)) + c_re;
        let next_im = (2.0 * z_re).mul_add(z_im, c_im);
        z_re = next_re;
        z_im = next_im;
    }
    records
}

fn render_special(
    mode: PrecisionMode,
    orbit: &[ReferenceOrbitRecord],
    offset: [f32; 4],
    exponent: i32,
    cap: u32,
) -> Rendered {
    match mode {
        PrecisionMode::Deterministic => render_special_deterministic(orbit, offset, exponent, cap),
        PrecisionMode::PictureFast => render_special_picture_fast(orbit, offset, exponent, cap),
    }
}

fn render_special_picture_fast(
    orbit: &[ReferenceOrbitRecord],
    offset: [f32; 4],
    exponent: i32,
    cap: u32,
) -> Rendered {
    render_special_deterministic(orbit, offset, exponent, cap)
}

fn render_special_deterministic(
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
    if sample.record.glitch == 1.0 {
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
        record.glitch,
    ]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn rgba8(rgba: [f32; 4]) -> [u8; 4] {
    rgba.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn category_colour(mode: PrecisionMode, record: [f32; 4], selected: PaletteRecord) -> [f32; 4] {
    match mode {
        PrecisionMode::Deterministic => category_colour_deterministic(record, selected),
        PrecisionMode::PictureFast => category_colour_picture_fast(record, selected),
    }
}

fn category_colour_picture_fast(record: [f32; 4], selected: PaletteRecord) -> [f32; 4] {
    category_colour_deterministic(record, selected)
}

fn category_colour_deterministic(record: [f32; 4], selected: PaletteRecord) -> [f32; 4] {
    shade_lit_escape_record(record, selected, LIGHT).rgba
}

const fn clear_colour(mode: PrecisionMode, selected: PaletteRecord) -> [f32; 4] {
    match mode {
        PrecisionMode::Deterministic => selected.clear_rgba,
        PrecisionMode::PictureFast => clear_colour_picture_fast(selected),
    }
}

const fn clear_colour_picture_fast(selected: PaletteRecord) -> [f32; 4] {
    selected.clear_rgba
}

fn warp_rows(mode: PrecisionMode, plane: Plane, zoom: f64) -> Result<[f64; 9], MathError> {
    match mode {
        PrecisionMode::Deterministic => warp_rows_deterministic(plane, zoom),
        PrecisionMode::PictureFast => warp_rows_picture_fast(plane, zoom),
    }
}

fn warp_rows_picture_fast(plane: Plane, zoom: f64) -> Result<[f64; 9], MathError> {
    warp_rows_deterministic(plane, zoom)
}

fn warp_rows_deterministic(plane: Plane, zoom: f64) -> Result<[f64; 9], MathError> {
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
        plane_theta_1: 0.0,
        plane_theta_2: 0.0,
        zoom_log2,
        view: ViewControls::NEUTRAL,
        grid_width: GRID_WIDTH,
        grid_height: GRID_HEIGHT,
        centre_from_reference_px: displacement,
    }
}

fn source_error_px(exact: [f64; 2], fast: [f64; 2]) -> f64 {
    let dx = (exact[0] - fast[0]).abs() * f64::from(GRID_WIDTH) * 0.5;
    let dy = (exact[1] - fast[1]).abs() * f64::from(GRID_HEIGHT) * 0.5;
    dx.max(dy)
}
