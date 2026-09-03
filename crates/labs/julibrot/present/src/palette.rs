use bytemuck::{Pod, Zeroable};

/// Opaque debug tint used for glitches and malformed escape records.
pub const DEBUG_TINT: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

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
#[must_use]
#[allow(clippy::float_cmp)]
pub fn shade_escape_record(record: [f32; 4], selected: PaletteRecord) -> PaletteOutcome {
    let [smooth_iter, escaped, rebase_count, status] = record;
    let malformed = !is_binary(escaped)
        || !matches!(status, 0.0 | 1.0 | 2.0 | 3.0)
        || !rebase_count.is_finite()
        || rebase_count < 0.0
        || rebase_count.fract() != 0.0;
    if malformed || status == 1.0 {
        return PaletteOutcome {
            rgba: DEBUG_TINT,
            contract_violation: malformed,
        };
    }
    if status == 2.0 || status == 3.0 {
        return PaletteOutcome {
            rgba: selected.clear_rgba,
            contract_violation: false,
        };
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
        || smooth_iter < 0.0
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
    let hue = (smooth_iter / period + phase).rem_euclid(1.0);
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
    if outcome.rgba == DEBUG_TINT {
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
            assert_palette_contract(mode, glitch.rgba, DEBUG_TINT);
            assert!(!glitch.contract_violation);
            let malformed = shade_escape_record([-1.0, 0.5, 0.0, 0.0], CLASSIC_PALETTE);
            assert_palette_contract(mode, malformed.rgba, DEBUG_TINT);
            assert!(malformed.contract_violation);
        }
    }

    #[test]
    fn horizon_and_uncertain_records_use_the_palette_clear_colour() {
        for status in [2.0, 3.0] {
            let outcome = shade_escape_record([0.0, 0.0, 0.0, status], EMBER_PALETTE);
            assert_eq!(outcome.rgba, EMBER_PALETTE.clear_rgba);
            assert!(!outcome.contract_violation);
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
        let debug = shade_lit_escape_record([4.0, 0.0, 2.0, 1.0], CLASSIC_PALETTE, 0.7);
        assert_eq!(debug.rgba, DEBUG_TINT);
    }
}
