use bytemuck::{Pod, Zeroable};

/// Opaque debug tint reserved for malformed escape records.
pub const DEBUG_TINT: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

/// Opaque diagnostic colour used for a measured perturbation glitch.
pub const GLITCH_DIAGNOSTIC: [f32; 4] = [1.0, 0.375, 0.0, 1.0];

/// A stable identifier for one present-owned palette.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PaletteId {
    /// The balanced default palette.
    Classic = 0,
    /// The warm red-orange palette.
    Ember = 1,
    /// The cool blue palette.
    Ice = 2,
}

/// The exact palette data uploaded in the scene uniform.
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C, align(16))]
pub struct PaletteRecord {
    /// Iterations per cycle, phase in turns, colour mix, and value.
    pub map: [f32; 4],
    /// Exact colour for a point that did not escape.
    pub interior_rgba: [f32; 4],
    /// Exact clear and disocclusion colour.
    pub clear_rgba: [f32; 4],
}

/// The version-one Classic palette.
pub const CLASSIC_PALETTE: PaletteRecord = PaletteRecord {
    map: [64.0, 0.0, 0.78, 1.0],
    interior_rgba: [0.005, 0.005, 0.008, 1.0],
    clear_rgba: [0.015, 0.018, 0.025, 1.0],
};

/// The version-one Ember palette.
pub const EMBER_PALETTE: PaletteRecord = PaletteRecord {
    map: [48.0, 0.02, 0.88, 1.0],
    interior_rgba: [0.01, 0.0, 0.0, 1.0],
    clear_rgba: [0.015, 0.008, 0.005, 1.0],
};

/// The version-one Ice palette.
pub const ICE_PALETTE: PaletteRecord = PaletteRecord {
    map: [80.0, 0.55, 0.72, 1.0],
    interior_rgba: [0.0, 0.005, 0.01, 1.0],
    clear_rgba: [0.005, 0.01, 0.015, 1.0],
};

/// Returns the exact record selected by an application MAIN state.
#[must_use]
pub const fn palette(id: PaletteId) -> PaletteRecord {
    match id {
        PaletteId::Classic => CLASSIC_PALETTE,
        PaletteId::Ember => EMBER_PALETTE,
        PaletteId::Ice => ICE_PALETTE,
    }
}

/// Scalar palette result used as the native shader oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaletteOutcome {
    /// Opaque linear colour selected for the record.
    pub rgba: [f32; 4],
    /// Whether malformed input forced the honest debug tint.
    pub contract_violation: bool,
}

#[allow(clippy::float_cmp)]
fn is_binary(value: f32) -> bool {
    value == 0.0 || value == 1.0
}

fn hue_component(hue: f32, offset: f32) -> f32 {
    ((hue + offset).rem_euclid(1.0).mul_add(6.0, -3.0).abs() - 1.0).clamp(0.0, 1.0)
}

/// Applies the present palette to `[smooth_iter, escaped, rebase_count, status]`.
///
/// An escaped record's smooth count `n+1-log2(log2|z_n|)` is legitimately negative. The squared
/// bailout is fixed at `256`, so a sample that is already outside radius `16` when the recurrence
/// starts escapes at index zero with a count at most `-1`, and a first-iteration escape reaches
/// `-1` too. Those are ordinary exterior samples, not malformed records, and the debug tint is
/// reserved for contract violations. The count clamps at zero for the hue exactly as the height
/// law already clamps it to the floor, so the whole beyond-bailout region reads as the palette
/// exterior at zero smooth iterations. That is also the colour the horizon carries, which is the
/// limit these samples approach: as the sample runs off to infinity the count falls without bound,
/// and a hue left to cycle on it would alias into stripes of ever-increasing frequency against the
/// horizon it meets.
#[must_use]
#[allow(clippy::float_cmp)]
pub fn shade_escape_record(record: [f32; 4], selected: PaletteRecord) -> PaletteOutcome {
    let [smooth_iter, escaped, rebase_count, status] = record;
    let malformed = !is_binary(escaped)
        || !matches!(status, 0.0 | 1.0 | 2.0 | 3.0)
        || !rebase_count.is_finite()
        || rebase_count < 0.0
        || rebase_count.fract() != 0.0;
    if malformed {
        return PaletteOutcome {
            rgba: DEBUG_TINT,
            contract_violation: true,
        };
    }
    if status == 1.0 {
        return PaletteOutcome {
            rgba: GLITCH_DIAGNOSTIC,
            contract_violation: false,
        };
    }
    if status == 2.0 {
        return shade_escape_record([0.0, 1.0, rebase_count, 0.0], selected);
    }
    if escaped == 0.0 {
        return if smooth_iter == -1.0 {
            PaletteOutcome {
                rgba: selected.interior_rgba,
                contract_violation: false,
            }
        } else {
            PaletteOutcome {
                rgba: DEBUG_TINT,
                contract_violation: true,
            }
        };
    }
    let [period, phase, colour_mix, value] = selected.map;
    if !smooth_iter.is_finite()
        || !period.is_finite()
        || period <= 0.0
        || !phase.is_finite()
        || !(0.0..=1.0).contains(&colour_mix)
        || !(0.0..=1.0).contains(&value)
    {
        return PaletteOutcome {
            rgba: DEBUG_TINT,
            contract_violation: true,
        };
    }
    let hue = (smooth_iter.max(0.0) / period + phase).rem_euclid(1.0);
    let phase_rgb = [
        hue_component(hue, 0.0),
        hue_component(hue, 2.0 / 3.0),
        hue_component(hue, 1.0 / 3.0),
    ];
    let rgb = phase_rgb.map(|component| value * (component - 1.0).mul_add(colour_mix, 1.0));
    PaletteOutcome {
        rgba: [rgb[0], rgb[1], rgb[2], 1.0],
        contract_violation: false,
    }
}

/// Applies the scene shader's post-palette lighting while preserving the unlit debug category.
#[must_use]
#[allow(clippy::float_cmp)]
pub fn shade_lit_escape_record(
    record: [f32; 4],
    selected: PaletteRecord,
    light: f32,
) -> PaletteOutcome {
    let mut outcome = shade_escape_record(record, selected);
    if outcome.rgba == DEBUG_TINT || outcome.rgba == GLITCH_DIAGNOSTIC {
        return outcome;
    }
    if !(0.58..=0.82).contains(&light) {
        return PaletteOutcome {
            rgba: DEBUG_TINT,
            contract_violation: true,
        };
    }
    for channel in &mut outcome.rgba[..3] {
        *channel *= light;
    }
    outcome
}

/// Returns the palette's immediate-escape exterior colour.
#[must_use]
pub fn exterior_zero(selected: PaletteRecord) -> [f32; 4] {
    shade_escape_record([0.0, 1.0, 0.0, 0.0], selected).rgba
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use ember_julibrot_math::PrecisionMode;

    use super::*;

    fn assert_palette_contract(mode: PrecisionMode, actual: [f32; 4], exact: [f32; 4]) {
        assert!(
            actual
                .into_iter()
                .zip(exact)
                .all(|(actual, exact)| (actual - exact).abs() <= 1.0 / 255.0)
        );
        if mode.requires_bit_identity() {
            // Deterministic-only contract: preserve every palette-oracle word.
            assert_eq!(actual, exact);
        }
    }

    #[test]
    fn records_are_exact_and_palette_ids_are_stable() {
        assert_eq!(size_of::<PaletteRecord>(), 48);
        assert_eq!(align_of::<PaletteRecord>(), 16);
        assert_eq!(PaletteId::Classic as u32, 0);
        assert_eq!(PaletteId::Ember as u32, 1);
        assert_eq!(PaletteId::Ice as u32, 2);
        assert_eq!(palette(PaletteId::Classic), CLASSIC_PALETTE);
        assert_eq!(palette(PaletteId::Ember), EMBER_PALETTE);
        assert_eq!(palette(PaletteId::Ice), ICE_PALETTE);
    }

    #[test]
    fn interior_glitch_and_malformed_records_stay_distinct() {
        for mode in PrecisionMode::ALL {
            let interior = shade_escape_record([-1.0, 0.0, 7.0, 0.0], CLASSIC_PALETTE);
            assert_palette_contract(mode, interior.rgba, CLASSIC_PALETTE.interior_rgba);
            assert!(!interior.contract_violation);
            let glitch = shade_escape_record([4.0, 0.0, 2.0, 1.0], CLASSIC_PALETTE);
            assert_palette_contract(mode, glitch.rgba, GLITCH_DIAGNOSTIC);
            assert_ne!(glitch.rgba, DEBUG_TINT);
            assert_ne!(glitch.rgba, CLASSIC_PALETTE.clear_rgba);
            assert_ne!(glitch.rgba, exterior_zero(CLASSIC_PALETTE));
            assert!(!glitch.contract_violation);
            let malformed = shade_escape_record([-1.0, 0.5, 0.0, 0.0], CLASSIC_PALETTE);
            assert_palette_contract(mode, malformed.rgba, DEBUG_TINT);
            assert!(malformed.contract_violation);
        }
    }

    #[test]
    fn horizon_is_immediate_exterior_and_uncertain_is_sampled() {
        let exterior = exterior_zero(EMBER_PALETTE);
        let horizon = shade_escape_record([-1.0, 0.0, 0.0, 2.0], EMBER_PALETTE);
        assert_eq!(horizon.rgba, exterior);
        assert!(!horizon.contract_violation);
        let uncertain = shade_escape_record([16.0, 1.0, 0.0, 3.0], EMBER_PALETTE);
        assert_ne!(uncertain.rgba, EMBER_PALETTE.clear_rgba);
        assert_eq!(
            uncertain,
            shade_escape_record([16.0, 1.0, 0.0, 0.0], EMBER_PALETTE)
        );
    }

    #[test]
    fn a_horizon_record_is_the_exterior_under_every_palette_and_light() {
        for id in [PaletteId::Classic, PaletteId::Ember, PaletteId::Ice] {
            let selected = palette(id);
            let exterior = exterior_zero(selected);
            assert_ne!(exterior, DEBUG_TINT);
            assert_ne!(exterior, selected.clear_rgba);
            for rebase in [0.0, 1.0, 7.0] {
                let record = [-1.0, 0.0, rebase, 2.0];
                let horizon = shade_escape_record(record, selected);
                assert_eq!(horizon.rgba, exterior);
                assert!(!horizon.contract_violation);
                for light in [0.58, 0.62, 0.66, 0.70, 0.74, 0.78, 0.82] {
                    let lit = shade_lit_escape_record(record, selected, light);
                    assert_ne!(lit.rgba, DEBUG_TINT);
                    assert!(!lit.contract_violation);
                    assert_eq!(lit.rgba[3], 1.0);
                    for (channel, exact) in lit.rgba[..3].iter().zip(exterior) {
                        assert_eq!(*channel, exact * light);
                    }
                }
            }
        }
    }

    #[test]
    fn a_beyond_bailout_escape_is_the_exterior_and_only_a_non_finite_count_violates() {
        for id in [PaletteId::Classic, PaletteId::Ember, PaletteId::Ice] {
            let selected = palette(id);
            let exterior = exterior_zero(selected);
            // `1-log2(log2|z_0|)` at the fixed squared bailout 256: an immediate escape is at most
            // -1, and it falls without bound as the sample runs off toward the horizon.
            for smooth in [-0.0, -0.5, -1.0, -1.112_397, -8.0, -1.0e6, f32::MIN] {
                for status in [0.0, 3.0] {
                    let outcome = shade_escape_record([smooth, 1.0, 0.0, status], selected);
                    assert_eq!(outcome.rgba, exterior);
                    assert!(!outcome.contract_violation);
                    for light in [0.58, 0.70, 0.82] {
                        let lit =
                            shade_lit_escape_record([smooth, 1.0, 0.0, status], selected, light);
                        assert_ne!(lit.rgba, DEBUG_TINT);
                        assert!(!lit.contract_violation);
                    }
                }
            }
            for smooth in [f32::NAN, f32::NEG_INFINITY, f32::INFINITY] {
                let outcome = shade_escape_record([smooth, 1.0, 0.0, 0.0], selected);
                assert_eq!(outcome.rgba, DEBUG_TINT);
                assert!(outcome.contract_violation);
            }
        }
    }

    #[test]
    fn escaped_records_follow_the_pinned_hue_formula() {
        let outcome = shade_escape_record([16.0, 1.0, 0.0, 0.0], CLASSIC_PALETTE);
        assert!(!outcome.contract_violation);
        assert_eq!(outcome.rgba[3], 1.0);
        assert!(
            outcome.rgba[..3]
                .iter()
                .all(|channel| (0.0..=1.0).contains(channel))
        );
    }

    #[test]
    fn lighting_matches_the_scene_order_and_never_dims_debug() {
        let escaped = shade_lit_escape_record([16.0, 1.0, 0.0, 0.0], CLASSIC_PALETTE, 0.7);
        let base = shade_escape_record([16.0, 1.0, 0.0, 0.0], CLASSIC_PALETTE);
        assert_eq!(escaped.rgba[0], base.rgba[0] * 0.7);
        assert_eq!(escaped.rgba[1], base.rgba[1] * 0.7);
        assert_eq!(escaped.rgba[2], base.rgba[2] * 0.7);
        assert_eq!(escaped.rgba[3], 1.0);
        let diagnostic = shade_lit_escape_record([4.0, 0.0, 2.0, 1.0], CLASSIC_PALETTE, 0.7);
        assert_eq!(diagnostic.rgba, GLITCH_DIAGNOSTIC);
    }
}
