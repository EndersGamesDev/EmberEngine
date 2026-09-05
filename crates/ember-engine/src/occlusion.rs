//! Immutable, deterministic directional ambient visibility for static geometry.
//!
//! Baking is CPU-only and consumes world-space boxes, not game or renderer
//! state. Two linear RGBA8 volumes hold the six axis-facing hemispheres. Their
//! lattice includes both bounds, with X contiguous, then Y, then Z; no sRGB
//! conversion or mip generation may be applied to these visibility bytes.

use glam::Vec3;

const MAX_VOXELS: u32 = 1_048_576;
const MAX_DIMENSION: u32 = 256;
const MAX_BOXES: usize = 256;
const BIN_NODES: u32 = 4;
const RAY_COUNT: usize = 26;

/// A solid static world-space box, including its boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionBox {
    pub min: Vec3,
    pub max: Vec3,
}

/// Inclusive lattice bounds and the maximum requested node spacing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionSettings {
    pub min: Vec3,
    pub max: Vec3,
    /// At least 0.25 metres; effective spacing fits the inclusive bounds.
    pub cell_size: f32,
    /// Maximum occluder distance, from 0.25 through 8 metres.
    pub radius: f32,
}

/// Baked visibility, immutable after construction.
#[derive(Clone, Debug, PartialEq)]
pub struct OcclusionField {
    min: Vec3,
    max: Vec3,
    dimensions: [u32; 3],
    cell_size: Vec3,
    radius: f32,
    texture_a: Vec<u8>,
    texture_b: Vec<u8>,
}

impl OcclusionField {
    /// Bake a bounded field from static boxes; nothing is silently truncated.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or inverted bounds, invalid spacing/radius, more
    /// than 256 boxes, dimensions over 256, or more than 1,048,576 nodes.
    /// The two output volumes occupy at most 8 MiB together.
    pub fn bake(settings: OcclusionSettings, boxes: &[OcclusionBox]) -> Result<Self, String> {
        let (dimensions, cell_size, count) = validate(settings, boxes)?;
        let mut field = Self {
            min: settings.min,
            max: settings.max,
            dimensions,
            cell_size,
            radius: settings.radius,
            texture_a: vec![255; count as usize * 4],
            texture_b: vec![255; count as usize * 4],
        };
        if boxes.is_empty() {
            return Ok(field);
        }
        let bins = CandidateBins::new(settings, dimensions, cell_size, boxes);
        let rays = rays();
        for z in 0..dimensions[2] {
            for y in 0..dimensions[1] {
                for x in 0..dimensions[0] {
                    let mask = bins.at([x, y, z]);
                    if mask == [0; 4] {
                        continue;
                    }
                    let position = lattice_position(settings, dimensions, cell_size, [x, y, z]);
                    let visibility = probe(position, settings.radius, boxes, mask, &rays);
                    let at = ((z * dimensions[1] + y) * dimensions[0] + x) as usize * 4;
                    field.texture_a[at..at + 4].copy_from_slice(&visibility[..4]);
                    field.texture_b[at..at + 2].copy_from_slice(&visibility[4..]);
                }
            }
        }
        Ok(field)
    }

    #[must_use]
    pub const fn min(&self) -> Vec3 {
        self.min
    }

    #[must_use]
    pub const fn max(&self) -> Vec3 {
        self.max
    }

    #[must_use]
    pub const fn dimensions(&self) -> [u32; 3] {
        self.dimensions
    }

    #[must_use]
    pub const fn cell_size(&self) -> Vec3 {
        self.cell_size
    }

    #[must_use]
    pub const fn radius(&self) -> f32 {
        self.radius
    }

    /// Linear visibility in RGBA order: +X, -X, +Y, -Y.
    #[must_use]
    pub fn texture_a(&self) -> &[u8] {
        &self.texture_a
    }

    /// Linear visibility in RGBA order: +Z, -Z, unused 255, unused 255.
    #[must_use]
    pub fn texture_b(&self) -> &[u8] {
        &self.texture_b
    }
}

fn validate(
    settings: OcclusionSettings,
    boxes: &[OcclusionBox],
) -> Result<([u32; 3], Vec3, u32), String> {
    if !settings.min.is_finite()
        || !settings.max.is_finite()
        || !settings.max.cmpgt(settings.min).all()
    {
        return Err("occlusion bounds must be finite and strictly increasing on every axis".into());
    }
    if !settings.cell_size.is_finite() || settings.cell_size < 0.25 {
        return Err("occlusion node spacing must be finite and at least 0.25 metres".into());
    }
    if !settings.radius.is_finite() || !(0.25..=8.0).contains(&settings.radius) {
        return Err("occlusion radius must be finite and between 0.25 and 8 metres".into());
    }
    if boxes.len() > MAX_BOXES {
        return Err(format!(
            "occlusion has {} boxes; limit is {MAX_BOXES}",
            boxes.len()
        ));
    }
    for (index, bounds) in boxes.iter().enumerate() {
        if !bounds.min.is_finite() || !bounds.max.is_finite() || !bounds.max.cmpgt(bounds.min).all()
        {
            return Err(format!(
                "occlusion box {index} must have finite, strictly increasing bounds"
            ));
        }
    }
    let span = settings.max - settings.min;
    if !span.is_finite() {
        return Err("occlusion span exceeds finite world coordinates".into());
    }
    let mut dimensions = [0; 3];
    let mut cell_size = Vec3::ZERO;
    for axis in 0..3 {
        let intervals = (f64::from(span[axis]) / f64::from(settings.cell_size))
            .ceil()
            .max(1.0);
        if intervals >= f64::from(MAX_DIMENSION) {
            return Err(format!(
                "occlusion axis {axis} exceeds {MAX_DIMENSION} inclusive nodes"
            ));
        }
        // Validated above: the rounded integer is in 1..=255.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let intervals = intervals as u32;
        dimensions[axis] = intervals + 1;
        cell_size[axis] =
            span[axis] / f32::from(u8::try_from(intervals).expect("at most255 intervals"));
    }
    let count = dimensions[0]
        .checked_mul(dimensions[1])
        .and_then(|count| count.checked_mul(dimensions[2]))
        .ok_or_else(|| "occlusion voxel count overflowed".to_string())?;
    if count > MAX_VOXELS {
        return Err(format!(
            "occlusion requires {count} nodes; limit is {MAX_VOXELS}"
        ));
    }
    Ok((dimensions, cell_size, count))
}

// Keep the verified non-fused lattice arithmetic aligned with WGSL sampling;
// baseline WebAssembly has no fused instruction and would emulate mul_add.
#[allow(clippy::suboptimal_flops)]
fn lattice_position(
    settings: OcclusionSettings,
    dimensions: [u32; 3],
    cell_size: Vec3,
    node: [u32; 3],
) -> Vec3 {
    let mut point = settings.min;
    for axis in 0..3 {
        point[axis] = if node[axis] + 1 == dimensions[axis] {
            settings.max[axis]
        } else {
            settings.min[axis]
                + f32::from(u8::try_from(node[axis]).expect("node index fits u8")) * cell_size[axis]
        };
    }
    point
}

/// Coarse bins hold a fixed-size mask, not a per-voxel list of every box.
struct CandidateBins {
    dimensions: [u32; 3],
    masks: Vec<[u64; 4]>,
}

impl CandidateBins {
    fn new(
        settings: OcclusionSettings,
        dimensions: [u32; 3],
        cell_size: Vec3,
        boxes: &[OcclusionBox],
    ) -> Self {
        let bin_dimensions = dimensions.map(|size| size.div_ceil(BIN_NODES));
        let mut bins = Self {
            dimensions: bin_dimensions,
            masks: vec![
                [0; 4];
                (bin_dimensions[0] * bin_dimensions[1] * bin_dimensions[2]) as usize
            ],
        };
        for (index, bounds) in boxes.iter().enumerate() {
            let expanded_min = bounds.min - Vec3::splat(settings.radius);
            let expanded_max = bounds.max + Vec3::splat(settings.radius);
            if expanded_max.cmplt(settings.min).any() || expanded_min.cmpgt(settings.max).any() {
                continue;
            }
            let low = ((expanded_min.max(settings.min) - settings.min) / cell_size).floor();
            let high = ((expanded_max.min(settings.max) - settings.min) / cell_size).ceil();
            let to_bin = |point: Vec3| {
                std::array::from_fn::<_, 3, _>(|axis| {
                    // Clamped finite lattice coordinates, at most255.
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let node = point[axis] as u32;
                    node.min(dimensions[axis] - 1) / BIN_NODES
                })
            };
            let (low, high) = (to_bin(low), to_bin(high));
            for z in low[2]..=high[2] {
                for y in low[1]..=high[1] {
                    for x in low[0]..=high[0] {
                        let at = bins.index([x, y, z]);
                        bins.masks[at][index / 64] |= 1 << (index % 64);
                    }
                }
            }
        }
        bins
    }

    const fn index(&self, cell: [u32; 3]) -> usize {
        ((cell[2] * self.dimensions[1] + cell[1]) * self.dimensions[0] + cell[0]) as usize
    }

    fn at(&self, node: [u32; 3]) -> [u64; 4] {
        self.masks[self.index(node.map(|index| index / BIN_NODES))]
    }
}

#[derive(Clone, Copy, Default)]
struct Ray {
    direction: Vec3,
    reciprocal: Vec3,
    weights: [f32; 6],
}

fn rays() -> [Ray; RAY_COUNT] {
    let mut rays = [Ray::default(); RAY_COUNT];
    let mut totals = [0.0; 6];
    let mut index = 0;
    for z in [-1.0, 0.0, 1.0] {
        for y in [-1.0, 0.0, 1.0] {
            for x in [-1.0, 0.0, 1.0] {
                let vector = Vec3::new(x, y, z);
                if vector == Vec3::ZERO {
                    continue;
                }
                let direction = vector.normalize();
                let weights = [
                    direction.x,
                    -direction.x,
                    direction.y,
                    -direction.y,
                    direction.z,
                    -direction.z,
                ]
                .map(|weight| weight.max(0.0));
                let reciprocal = Vec3::from_array(
                    direction.to_array().map(
                        |value| {
                            if value == 0.0 { 0.0 } else { value.recip() }
                        },
                    ),
                );
                rays[index] = Ray {
                    direction,
                    reciprocal,
                    weights,
                };
                for axis in 0..6 {
                    totals[axis] += weights[axis];
                }
                index += 1;
            }
        }
    }
    for ray in &mut rays {
        for (weight, total) in ray.weights.iter_mut().zip(totals) {
            *weight /= total;
        }
    }
    rays
}

fn ray_entry(origin: Vec3, ray: &Ray, bounds: OcclusionBox, radius: f32) -> Option<f32> {
    let mut near = 0.0_f32;
    let mut far = radius;
    for axis in 0..3 {
        if ray.direction[axis] == 0.0 {
            if origin[axis] < bounds.min[axis] || origin[axis] > bounds.max[axis] {
                return None;
            }
        } else {
            let a = (bounds.min[axis] - origin[axis]) * ray.reciprocal[axis];
            let b = (bounds.max[axis] - origin[axis]) * ray.reciprocal[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return None;
            }
        }
    }
    Some(near)
}

fn probe(
    position: Vec3,
    radius: f32,
    boxes: &[OcclusionBox],
    mask: [u64; 4],
    rays: &[Ray; RAY_COUNT],
) -> [u8; 6] {
    let mut distances = [radius; RAY_COUNT];
    for (word, mut bits) in mask.into_iter().enumerate() {
        while bits != 0 {
            let index = word * 64 + bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let bounds = boxes[index];
            if position.cmpge(bounds.min).all() && position.cmple(bounds.max).all() {
                return [0; 6];
            }
            // The bin test is conservative; reject diagonal/far candidates
            // before doing any of the 26 ray/box intersections.
            let separation =
                (bounds.min - position).max(Vec3::ZERO) + (position - bounds.max).max(Vec3::ZERO);
            if separation.length_squared() >= radius * radius {
                continue;
            }
            for (distance, ray) in distances.iter_mut().zip(rays) {
                if let Some(hit) = ray_entry(position, ray, bounds, *distance) {
                    *distance = hit;
                }
            }
        }
    }
    let mut occluded = [0.0; 6];
    for (distance, ray) in distances.into_iter().zip(rays) {
        let fraction = (distance / radius).clamp(0.0, 1.0);
        // Preserve the verified bake's non-fused interpolation arithmetic;
        // mul_add would require software FMA on baseline WebAssembly.
        #[allow(clippy::suboptimal_flops)]
        let obstruction = 1.0 - fraction * fraction * (3.0 - 2.0 * fraction);
        for (sum, weight) in occluded.iter_mut().zip(ray.weights) {
            #[allow(clippy::suboptimal_flops)]
            {
                *sum += obstruction * weight;
            }
        }
    }
    occluded.map(|amount| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            ((1.0 - amount).clamp(0.0, 1.0) * 255.0).round() as u8
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> OcclusionSettings {
        OcclusionSettings {
            min: Vec3::splat(-3.0),
            max: Vec3::splat(3.0),
            cell_size: 0.5,
            radius: 2.25,
        }
    }

    fn sample(field: &OcclusionField, node: [u32; 3]) -> [u8; 6] {
        let [x, y, z] = node;
        let [width, height, _] = field.dimensions();
        let at = ((z * height + y) * width + x) as usize * 4;
        [
            field.texture_a[at],
            field.texture_a[at + 1],
            field.texture_a[at + 2],
            field.texture_a[at + 3],
            field.texture_b[at],
            field.texture_b[at + 1],
        ]
    }

    #[test]
    fn empty_space_is_neutral_and_inclusive_layout_is_exact() {
        let input = OcclusionSettings {
            min: Vec3::new(-2.0, -0.5, 4.0),
            max: Vec3::new(3.0, 1.0, 6.0),
            cell_size: 0.7,
            ..settings()
        };
        let field = OcclusionField::bake(input, &[]).unwrap();
        assert_eq!(field.dimensions(), [9, 4, 4]);
        assert_eq!(field.cell_size(), Vec3::new(0.625, 0.5, 2.0 / 3.0));
        assert_eq!(field.min(), input.min);
        assert_eq!(field.max(), input.max);
        assert_eq!(field.radius(), 2.25);
        assert_eq!(field.texture_a().len(), 9 * 4 * 4 * 4);
        assert_eq!(field.texture_b().len(), field.texture_a().len());
        assert!(
            field
                .texture_a()
                .iter()
                .chain(field.texture_b())
                .all(|&v| v == 255)
        );
        assert_eq!(
            lattice_position(input, field.dimensions(), field.cell_size(), [8, 3, 3]),
            input.max
        );
    }

    #[test]
    fn axis_visibility_faces_nearby_cover_and_occupied_nodes_are_zero() {
        let wall = OcclusionBox {
            min: Vec3::new(0.5, -3.0, -3.0),
            max: Vec3::new(1.0, 3.0, 3.0),
        };
        let field = OcclusionField::bake(settings(), &[wall]).unwrap();
        let near = sample(&field, [6, 6, 6]);
        assert!(near[0] < 90, "toward wall: {near:?}");
        assert_eq!(near[1], 255, "away from wall: {near:?}");
        assert_eq!(near[2], near[3]);
        assert_eq!(near[4], near[5]);
        assert_eq!(
            sample(&field, [7, 6, 6]),
            [0; 6],
            "surface node is occupied"
        );
        assert_eq!(sample(&field, [8, 6, 6]), [0; 6]);
        assert!(
            field
                .texture_b()
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[2..] == [255, 255])
        );
        let outside = sample(&field, [9, 6, 6]);
        assert_eq!(outside[0], 255);
        assert!(outside[1] < 90);
    }

    #[test]
    fn raised_blockers_occlude_upwards_without_filling_the_tunnel() {
        let roof = OcclusionBox {
            min: Vec3::new(-3.0, 0.5, -3.0),
            max: Vec3::new(3.0, 1.0, 3.0),
        };
        let field = OcclusionField::bake(settings(), &[roof]).unwrap();
        let under = sample(&field, [6, 6, 6]);
        assert!(under[2] < 90);
        assert_eq!(under[3], 255);
        assert_ne!(under, [0; 6]);
        assert_eq!(sample(&field, [6, 7, 6]), [0; 6]);
    }

    #[test]
    fn directional_channels_match_all_six_world_axes() {
        for axis in 0..3 {
            for negative in [false, true] {
                let mut bounds = OcclusionBox {
                    min: Vec3::splat(-3.0),
                    max: Vec3::splat(3.0),
                };
                bounds.min[axis] = if negative { -1.0 } else { 0.5 };
                bounds.max[axis] = if negative { -0.5 } else { 1.0 };
                let field = OcclusionField::bake(settings(), &[bounds]).unwrap();
                let channels = sample(&field, [6, 6, 6]);
                let toward = axis * 2 + usize::from(negative);
                assert!(
                    channels[toward] < 90,
                    "axis {axis}, negative {negative}: {channels:?}"
                );
                assert_eq!(channels[toward ^ 1], 255);
            }
        }
    }

    #[test]
    fn distant_cover_is_neutral_and_distance_fade_is_smooth() {
        let ray_set = rays();
        let value = |distance| {
            let wall = OcclusionBox {
                min: Vec3::new(distance, -8.0, -8.0),
                max: Vec3::new(distance + 0.2, 8.0, 8.0),
            };
            probe(Vec3::ZERO, 2.25, &[wall], [1, 0, 0, 0], &ray_set)[0]
        };
        let values = [
            value(0.25),
            value(0.75),
            value(1.5),
            value(2.20),
            value(2.25),
        ];
        assert!(
            values.windows(2).all(|pair| pair[0] <= pair[1]),
            "{values:?}"
        );
        assert!(values[0] < 30 && values[3] >= 253);
        assert_eq!(values[4], 255);
        let far = OcclusionBox {
            min: Vec3::splat(20.0),
            max: Vec3::splat(21.0),
        };
        let field = OcclusionField::bake(settings(), &[far]).unwrap();
        assert!(
            field
                .texture_a()
                .iter()
                .chain(field.texture_b())
                .all(|&v| v == 255)
        );
    }

    #[test]
    fn slab_intersection_handles_parallel_boundary_inside_and_behind_rays() {
        let set = rays();
        let positive_x = set.iter().find(|r| r.direction == Vec3::X).unwrap();
        let bounds = OcclusionBox {
            min: Vec3::new(1.0, -1.0, -1.0),
            max: Vec3::new(2.0, 1.0, 1.0),
        };
        assert_eq!(ray_entry(Vec3::ZERO, positive_x, bounds, 3.0), Some(1.0));
        assert_eq!(ray_entry(Vec3::Y, positive_x, bounds, 3.0), Some(1.0));
        assert_eq!(ray_entry(Vec3::Y * 1.01, positive_x, bounds, 3.0), None);
        assert_eq!(ray_entry(Vec3::X * 3.0, positive_x, bounds, 3.0), None);
        assert_eq!(ray_entry(Vec3::X * 1.5, positive_x, bounds, 3.0), Some(0.0));
        assert_eq!(ray_entry(Vec3::ZERO, positive_x, bounds, 0.5), None);
        for ray in &set {
            assert!((ray.direction.length() - 1.0).abs() < 1e-6);
            assert!(ray.reciprocal.is_finite());
        }
        for axis in 0..6 {
            assert!((set.iter().map(|r| r.weights[axis]).sum::<f32>() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn box_order_and_duplicate_boxes_do_not_change_the_bake() {
        let a = OcclusionBox {
            min: Vec3::new(0.5, -2.0, -1.0),
            max: Vec3::new(1.0, 2.0, 1.0),
        };
        let b = OcclusionBox {
            min: Vec3::new(-2.0, 1.0, -2.0),
            max: Vec3::new(2.0, 1.5, 2.0),
        };
        let forward = OcclusionField::bake(settings(), &[a, b]).unwrap();
        assert_eq!(forward, OcclusionField::bake(settings(), &[b, a]).unwrap());
        assert_eq!(
            forward,
            OcclusionField::bake(settings(), &[a, b, a]).unwrap()
        );
    }

    #[test]
    fn binned_candidates_match_an_all_box_reference_at_every_node() {
        let input = settings();
        let boxes = [
            OcclusionBox {
                min: Vec3::new(-2.3, -2.1, -1.2),
                max: Vec3::new(-1.7, -0.9, 0.7),
            },
            OcclusionBox {
                min: Vec3::new(0.13, 0.78, -2.91),
                max: Vec3::new(2.6, 1.2, -0.1),
            },
            OcclusionBox {
                min: Vec3::new(3.7, -1.0, 0.0),
                max: Vec3::new(4.2, 2.0, 2.0),
            },
        ];
        let field = OcclusionField::bake(input, &boxes).unwrap();
        let ray_set = rays();
        let bins = CandidateBins::new(input, field.dimensions, field.cell_size, &boxes);
        assert_ne!(
            bins.at([0, 0, 0]),
            [7, 0, 0, 0],
            "distant boxes must not reach every probe"
        );
        for z in 0..field.dimensions[2] {
            for y in 0..field.dimensions[1] {
                for x in 0..field.dimensions[0] {
                    let node = [x, y, z];
                    let point = lattice_position(input, field.dimensions, field.cell_size, node);
                    assert_eq!(
                        sample(&field, node),
                        probe(point, input.radius, &boxes, [7, 0, 0, 0], &ray_set),
                        "{node:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn invalid_inputs_and_capacity_overflows_fail_closed() {
        for input in [
            OcclusionSettings {
                min: Vec3::NAN,
                ..settings()
            },
            OcclusionSettings {
                max: Vec3::INFINITY,
                ..settings()
            },
            OcclusionSettings {
                max: Vec3::splat(-3.0),
                ..settings()
            },
            OcclusionSettings {
                cell_size: 0.249,
                ..settings()
            },
            OcclusionSettings {
                cell_size: f32::NAN,
                ..settings()
            },
            OcclusionSettings {
                radius: f32::INFINITY,
                ..settings()
            },
            OcclusionSettings {
                radius: 0.2,
                ..settings()
            },
            OcclusionSettings {
                radius: 8.01,
                ..settings()
            },
            OcclusionSettings {
                min: Vec3::ZERO,
                max: Vec3::splat(128.0),
                ..settings()
            },
            OcclusionSettings {
                min: Vec3::ZERO,
                max: Vec3::splat(100.0),
                ..settings()
            },
        ] {
            assert!(OcclusionField::bake(input, &[]).is_err(), "{input:?}");
        }
        for bounds in [
            OcclusionBox {
                min: Vec3::NAN,
                max: Vec3::ONE,
            },
            OcclusionBox {
                min: Vec3::ZERO,
                max: Vec3::INFINITY,
            },
            OcclusionBox {
                min: Vec3::ONE,
                max: Vec3::ZERO,
            },
            OcclusionBox {
                min: Vec3::ZERO,
                max: Vec3::ZERO,
            },
        ] {
            assert!(OcclusionField::bake(settings(), &[bounds]).is_err());
        }
        let bounds = OcclusionBox {
            min: Vec3::ZERO,
            max: Vec3::ONE,
        };
        assert!(OcclusionField::bake(settings(), &[bounds; MAX_BOXES + 1]).is_err());
    }

    #[test]
    fn the_inclusive_capacity_limit_is_accepted_without_truncation() {
        let field = OcclusionField::bake(
            OcclusionSettings {
                min: Vec3::ZERO,
                max: Vec3::new(127.5, 127.5, 7.5),
                ..settings()
            },
            &[],
        )
        .unwrap();
        assert_eq!(field.dimensions(), [256, 256, 16]);
        assert_eq!(
            field.texture_a().len() + field.texture_b().len(),
            8 * MAX_VOXELS as usize
        );
    }

    #[test]
    #[ignore = "CPU bake benchmark; run at minimum priority with --release --nocapture"]
    #[allow(clippy::print_stdout)]
    fn occlusion_cpu_bake_benchmark() {
        for half in [26.0, 50.0] {
            let input = OcclusionSettings {
                min: Vec3::new(-half, -0.5, -half),
                max: Vec3::new(half, 7.5, half),
                cell_size: 0.5,
                radius: 2.25,
            };
            let mut boxes = vec![OcclusionBox {
                min: Vec3::new(-half, -1.0, -half),
                max: Vec3::new(half, 0.0, half),
            }];
            for z in [-27.0, -13.0, 13.0, 27.0] {
                for x in [-15.0, 0.0, 15.0] {
                    boxes.push(OcclusionBox {
                        min: Vec3::new(x - 6.095, 0.0, z - 1.219),
                        max: Vec3::new(x + 6.095, 2.591, z + 1.219),
                    });
                }
            }
            for z in [-20.6, -15.4, 15.4, 20.6] {
                for x in [32.0, 44.0] {
                    boxes.push(OcclusionBox {
                        min: Vec3::new(x - 0.6, 0.0, z - 0.6),
                        max: Vec3::new(x + 0.6, 12.0, z + 0.6),
                    });
                }
            }
            let started = web_time::Instant::now();
            let field = OcclusionField::bake(input, &boxes).unwrap();
            println!(
                "AO CPU bake: half={half}, boxes={}, dimensions={:?}, bytes={}, elapsed_ms={:.3}",
                boxes.len(),
                field.dimensions(),
                field.texture_a().len() + field.texture_b().len(),
                started.elapsed().as_secs_f64() * 1000.0
            );
            std::hint::black_box(field);
        }
    }
}
