use ember_julibrot_math::{EscapeParams, PrecisionMode};

use crate::{
    GridExtent, KernelError, KernelMode, OUTPUT_PAGE_SIDE, RefinementLevel,
    shallow::{validate_extent, validate_params},
};

const PREVIEW_DIVISOR: u32 = 4;
const FAST_PREVIEW_DIVISOR: u32 = 8;
const INTERACTIVE_DIVISOR: u32 = 2;
const PREVIEW_CAP: u32 = 64;
const FAST_PREVIEW_CAP: u32 = 32;
const INTERACTIVE_CAP: u32 = 256;
const FINAL_CAP: u32 = 4_096;
const RECORD_BYTES: u64 = 16;

/// One exact kernel-defined refinement level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LevelSpec {
    pub level: RefinementLevel,
    pub extent: GridExtent,
    pub iteration_cap: u32,
}

/// Requested controls and the deterministic delivered refinement sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefinementPlan {
    pub requested_extent: GridExtent,
    pub delivered_extent: GridExtent,
    pub extent_divisor: u32,
    pub requested_max_iter: u32,
    pub delivered_max_iter: u32,
    pub page_side: u16,
    pub precision_mode: PrecisionMode,
    pub levels: [LevelSpec; 3],
}

impl RefinementPlan {
    /// Returns the exact specification for a closed refinement discriminant.
    #[must_use]
    pub const fn level(&self, level: RefinementLevel) -> LevelSpec {
        match level {
            RefinementLevel::Preview => self.levels[0],
            RefinementLevel::Interactive => self.levels[1],
            RefinementLevel::Final => self.levels[2],
        }
    }

    /// Applies the selected precision policy to the kernel-owned ladder.
    #[must_use]
    pub fn with_precision_mode(mut self, precision_mode: PrecisionMode) -> Self {
        self.precision_mode = precision_mode;
        self.levels = levels(
            self.delivered_extent,
            self.requested_max_iter,
            precision_mode,
        );
        self
    }
}

/// Returns the next scheduled level for the selected precision policy.
#[must_use]
pub const fn next_refinement_level(
    precision_mode: PrecisionMode,
    completed: RefinementLevel,
) -> Option<RefinementLevel> {
    match (precision_mode, completed) {
        (PrecisionMode::PictureFast, RefinementLevel::Preview)
        | (_, RefinementLevel::Interactive) => Some(RefinementLevel::Final),
        (_, RefinementLevel::Preview) => Some(RefinementLevel::Interactive),
        (_, RefinementLevel::Final) => None,
    }
}

/// Arithmetic and copied owner facts for one encoded logical level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchFacts {
    pub owner_epoch: u64,
    pub precision_mode: &'static str,
    pub mode: KernelMode,
    pub level: RefinementLevel,
    pub requested_extent: GridExtent,
    pub delivered_extent: GridExtent,
    pub requested_max_iter: u32,
    pub delivered_max_iter: u32,
    pub active_pixels: u32,
    pub worst_case_pixel_iterations: u64,
    pub page_passes: u32,
    pub copy_commands: u32,
    pub gpu_copy_bytes: u64,
    pub logical_heap_bytes: u64,
    pub reserved_heap_bytes: u64,
    pub scratch_bytes: u64,
    pub orbit_generation: Option<u32>,
    pub orbit_length: u32,
    pub draft_pixels_discarded: u32,
    pub draft_iterations_discarded: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrefixPage {
    pub global_base: u32,
    pub valid_length: u32,
}

const fn divided_extent(extent: GridExtent, divisor: u32) -> GridExtent {
    GridExtent {
        width: extent.width.div_ceil(divisor),
        height: extent.height.div_ceil(divisor),
    }
}

fn levels(
    extent: GridExtent,
    requested_max_iter: u32,
    precision_mode: PrecisionMode,
) -> [LevelSpec; 3] {
    let (preview_divisor, preview_cap) = match precision_mode {
        PrecisionMode::Deterministic => (PREVIEW_DIVISOR, PREVIEW_CAP),
        PrecisionMode::PictureFast => (FAST_PREVIEW_DIVISOR, FAST_PREVIEW_CAP),
    };
    [
        LevelSpec {
            level: RefinementLevel::Preview,
            extent: divided_extent(extent, preview_divisor),
            iteration_cap: requested_max_iter.min(preview_cap),
        },
        LevelSpec {
            level: RefinementLevel::Interactive,
            extent: divided_extent(extent, INTERACTIVE_DIVISOR),
            iteration_cap: requested_max_iter.min(INTERACTIVE_CAP),
        },
        LevelSpec {
            level: RefinementLevel::Final,
            extent,
            iteration_cap: requested_max_iter.min(FINAL_CAP),
        },
    ]
}

pub fn validate_plan(plan: &RefinementPlan) -> Result<(), KernelError> {
    validate_extent(plan.requested_extent)?;
    validate_extent(plan.delivered_extent)?;
    if plan.extent_divisor == 0
        || !plan.extent_divisor.is_power_of_two()
        || divided_extent(plan.requested_extent, plan.extent_divisor) != plan.delivered_extent
        || plan.requested_max_iter == 0
        || plan.delivered_max_iter != plan.requested_max_iter.min(FINAL_CAP)
        || plan.page_side != OUTPUT_PAGE_SIDE
        || plan.levels
            != levels(
                plan.delivered_extent,
                plan.requested_max_iter,
                plan.precision_mode,
            )
    {
        return Err(KernelError::Dispatch);
    }
    Ok(())
}

/// Builds the deterministic levels using an exact caller-supplied capacity predicate.
///
/// # Errors
///
/// Returns a typed refusal for invalid controls, fixed-width overflow, or failure at the minimum
/// representable extent.
pub fn plan_refinement(
    requested_extent: GridExtent,
    params: EscapeParams,
    mut accepts_records: impl FnMut(u32) -> bool,
) -> Result<RefinementPlan, KernelError> {
    validate_extent(requested_extent)?;
    validate_params(params)?;
    let mut divisor = 1_u32;
    loop {
        let delivered_extent = divided_extent(requested_extent, divisor);
        if let Some(records) = delivered_extent.width.checked_mul(delivered_extent.height)
            && accepts_records(records)
        {
            return Ok(RefinementPlan {
                requested_extent,
                delivered_extent,
                extent_divisor: divisor,
                requested_max_iter: params.max_iter,
                delivered_max_iter: params.max_iter.min(FINAL_CAP),
                page_side: OUTPUT_PAGE_SIDE,
                precision_mode: PrecisionMode::Deterministic,
                levels: levels(
                    delivered_extent,
                    params.max_iter,
                    PrecisionMode::Deterministic,
                ),
            });
        }
        if delivered_extent.width == 1 && delivered_extent.height == 1 {
            return Err(KernelError::Heap);
        }
        divisor = divisor
            .checked_mul(2)
            .ok_or(KernelError::ArithmeticOverflow)?;
    }
}

fn prefix_pages(active_len: u32, page_records: u32) -> Result<Vec<PrefixPage>, KernelError> {
    if active_len == 0 || page_records == 0 {
        return Err(KernelError::InvalidExtent);
    }
    let page_count = active_len.div_ceil(page_records);
    Ok((0..page_count)
        .map(|page| {
            let global_base = page * page_records;
            PrefixPage {
                global_base,
                valid_length: (active_len - global_base).min(page_records),
            }
        })
        .collect())
}

fn copy_commands(active_len: u32, page_side: u32) -> u32 {
    let page_records = page_side * page_side;
    let complete_pages = active_len / page_records;
    let remainder = active_len % page_records;
    complete_pages
        + u32::from(remainder / page_side > 0)
        + u32::from(!remainder.is_multiple_of(page_side))
}

/// Derives honest arithmetic facts for a level without claiming a measured duration.
///
/// # Errors
///
/// Returns a typed refusal if the plan's active or reserved byte arithmetic overflows.
pub fn dispatch_facts(
    plan: &RefinementPlan,
    level: RefinementLevel,
    mode: KernelMode,
    owner_epoch: u64,
    precision_mode: PrecisionMode,
    scratch_bytes: u64,
    orbit: Option<(u32, u32)>,
) -> Result<DispatchFacts, KernelError> {
    let selected = plan.level(level);
    let active_pixels = validate_extent(selected.extent)?;
    let final_pixels = validate_extent(plan.delivered_extent)?;
    let page_side = u32::from(plan.page_side);
    if !page_side.is_power_of_two() {
        return Err(KernelError::InvalidExtent);
    }
    let page_records = page_side
        .checked_mul(page_side)
        .ok_or(KernelError::ArithmeticOverflow)?;
    let pages = prefix_pages(active_pixels, page_records)?;
    let last_page = pages.last().ok_or(KernelError::InvalidExtent)?;
    debug_assert_eq!(
        last_page.global_base + last_page.valid_length,
        active_pixels
    );
    let reserved_records = final_pixels
        .div_ceil(page_records)
        .checked_mul(page_records)
        .ok_or(KernelError::ArithmeticOverflow)?;
    let bytes = |records: u32| {
        u64::from(records)
            .checked_mul(RECORD_BYTES)
            .ok_or(KernelError::ArithmeticOverflow)
    };
    let (orbit_generation, orbit_length) = match orbit {
        Some((generation, length)) => (Some(generation), length),
        None => (None, 0),
    };
    let worst_case_pixel_iterations = u64::from(active_pixels)
        .checked_mul(u64::from(selected.iteration_cap))
        .ok_or(KernelError::ArithmeticOverflow)?;
    let draft = level != RefinementLevel::Final;
    Ok(DispatchFacts {
        owner_epoch,
        precision_mode: precision_mode.as_str(),
        mode,
        level,
        requested_extent: plan.requested_extent,
        delivered_extent: selected.extent,
        requested_max_iter: plan.requested_max_iter,
        delivered_max_iter: selected.iteration_cap,
        active_pixels,
        worst_case_pixel_iterations,
        page_passes: u32::try_from(pages.len()).map_err(|_| KernelError::ArithmeticOverflow)?,
        copy_commands: copy_commands(active_pixels, page_side),
        gpu_copy_bytes: bytes(active_pixels)?,
        logical_heap_bytes: bytes(active_pixels)?,
        reserved_heap_bytes: bytes(reserved_records)?,
        scratch_bytes,
        orbit_generation,
        orbit_length,
        draft_pixels_discarded: if draft { active_pixels } else { 0 },
        draft_iterations_discarded: if draft {
            worst_case_pixel_iterations
        } else {
            0
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{dispatch_facts, next_refinement_level, plan_refinement, prefix_pages};
    use crate::{GridExtent, KernelError, KernelMode, RefinementLevel};
    use ember_julibrot_math::{EscapeParams, PrecisionMode};

    #[test]
    fn exact_three_levels_and_caps_are_pinned() {
        let plan = plan_refinement(
            GridExtent {
                width: 1_921,
                height: 1_081,
            },
            EscapeParams::new(5_000),
            |_| true,
        )
        .expect("requested extent fits");
        assert_eq!(plan.extent_divisor, 1);
        assert_eq!(plan.delivered_max_iter, 4_096);
        assert_eq!(
            plan.level(RefinementLevel::Preview).extent,
            GridExtent {
                width: 480,
                height: 270
            }
        );
        assert_eq!(plan.level(RefinementLevel::Preview).iteration_cap, 64);
        assert_eq!(
            plan.level(RefinementLevel::Interactive).extent,
            GridExtent {
                width: 960,
                height: 540
            }
        );
        assert_eq!(plan.level(RefinementLevel::Interactive).iteration_cap, 256);
        assert_eq!(plan.level(RefinementLevel::Final).iteration_cap, 4_096);
        assert_eq!(
            next_refinement_level(PrecisionMode::Deterministic, RefinementLevel::Preview),
            Some(RefinementLevel::Interactive)
        );
    }

    #[test]
    fn picture_fast_uses_one_eighth_preview_and_omits_interactive() {
        let plan = plan_refinement(
            GridExtent {
                width: 1_920,
                height: 1_080,
            },
            EscapeParams::new(5_000),
            |_| true,
        )
        .expect("requested extent fits")
        .with_precision_mode(PrecisionMode::PictureFast);
        assert_eq!(
            plan.level(RefinementLevel::Preview).extent,
            GridExtent {
                width: 241,
                height: 136,
            }
        );
        assert_eq!(plan.level(RefinementLevel::Preview).iteration_cap, 32);
        assert_eq!(
            plan.level(RefinementLevel::Final).extent,
            plan.delivered_extent
        );
        assert_eq!(plan.level(RefinementLevel::Final).iteration_cap, 4_096);
        assert_eq!(
            next_refinement_level(PrecisionMode::PictureFast, RefinementLevel::Preview),
            Some(RefinementLevel::Final)
        );
        assert_eq!(
            next_refinement_level(PrecisionMode::PictureFast, RefinementLevel::Final),
            None
        );
        let preview = dispatch_facts(
            &plan,
            RefinementLevel::Preview,
            KernelMode::Shallow,
            1,
            16_777_216,
            None,
        )
        .expect("fast preview facts fit fixed widths");
        assert_eq!(preview.draft_pixels_discarded, 32_776);
        assert_eq!(preview.draft_iterations_discarded, 1_048_832);
    }

    #[test]
    fn capacity_uses_the_first_power_of_two_delivery() {
        let mut attempts = Vec::new();
        let plan = plan_refinement(
            GridExtent {
                width: 25,
                height: 17,
            },
            EscapeParams::new(100),
            |records| {
                attempts.push(records);
                records <= 100
            },
        )
        .expect("quarter extent fits");
        assert_eq!(attempts, [425, 117, 35]);
        assert_eq!(plan.extent_divisor, 4);
        assert_eq!(
            plan.delivered_extent,
            GridExtent {
                width: 7,
                height: 5
            }
        );
        assert_eq!(
            plan_refinement(
                GridExtent {
                    width: 1,
                    height: 1
                },
                EscapeParams::new(64),
                |_| false
            ),
            Err(KernelError::Heap)
        );
    }

    #[test]
    fn prefix_shapes_end_at_the_active_length() {
        let pages = prefix_pages(518_400, 65_536).expect("nonzero shape is valid");
        assert_eq!(pages.len(), 8);
        assert_eq!(pages[0].global_base, 0);
        assert_eq!(pages[0].valid_length, 65_536);
        assert_eq!(pages[7].global_base, 458_752);
        assert_eq!(pages[7].valid_length, 59_648);
    }

    #[test]
    fn dispatch_facts_match_the_documented_grid_arithmetic() {
        let plan = plan_refinement(
            GridExtent {
                width: 960,
                height: 540,
            },
            EscapeParams::new(1_000),
            |_| true,
        )
        .expect("extent fits");
        let preview = dispatch_facts(
            &plan,
            RefinementLevel::Preview,
            KernelMode::Shallow,
            9,
            PrecisionMode::PictureFast,
            16_777_216,
            None,
        )
        .expect("facts fit fixed widths");
        assert_eq!(preview.active_pixels, 32_400);
        assert_eq!(preview.precision_mode, "PictureFast");
        assert_eq!(preview.gpu_copy_bytes, 518_400);
        assert_eq!(preview.page_passes, 1);
        assert_eq!(preview.copy_commands, 2);
        assert_eq!(preview.reserved_heap_bytes, 8_388_608);
        assert_eq!(preview.scratch_bytes, 16_777_216);
        assert_eq!(preview.draft_pixels_discarded, 32_400);
        assert_eq!(preview.draft_iterations_discarded, 2_073_600);
        let final_level = dispatch_facts(
            &plan,
            RefinementLevel::Final,
            KernelMode::Perturbation,
            10,
            PrecisionMode::Deterministic,
            16_777_216,
            Some((7, 900)),
        )
        .expect("facts fit fixed widths");
        assert_eq!(final_level.active_pixels, 518_400);
        assert_eq!(final_level.page_passes, 8);
        assert_eq!(final_level.copy_commands, 8);
        assert_eq!(final_level.gpu_copy_bytes, 8_294_400);
        assert_eq!(final_level.orbit_generation, Some(7));
        assert_eq!(final_level.orbit_length, 900);
        assert_eq!(final_level.draft_pixels_discarded, 0);
        assert_eq!(final_level.draft_iterations_discarded, 0);
    }
}
