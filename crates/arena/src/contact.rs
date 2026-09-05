//! Presentation-only static ambient occlusion for the three authored maps.
//! Source volumes follow drawn cover, including raised bottoms. They neither
//! replace collision nor include players, weapons or animated reward blocks.

use std::sync::Arc;

use arena_core::shooter::{Cover, MAP_FREIGHT_YARD, MAP_HARBOR, MAP_TRENCH_CITY, Obstacle};
use ember_engine::glam::Vec3;
use ember_engine::{OcclusionBox, OcclusionField, OcclusionSettings};

const CELL_SIZE: f32 = 0.5;
const RADIUS: f32 = 2.25;
const PADDING: f32 = 2.0;
const MIN_Y: f32 = -0.5;
const MAX_Y: f32 = 7.5;
const MAX_VOXELS: u64 = 1_048_576;
const MAX_AXIS: u16 = 256;
const MAX_BOXES: usize = 256;

struct BakeInput {
    settings: OcclusionSettings,
    boxes: Vec<OcclusionBox>,
    dimensions: [u32; 3],
}

impl BakeInput {
    /// Exact float-bit geometry key: no collision can silently reuse another
    /// map's occlusion. Box ordering is fixed by the authored level constructors.
    fn key(&self) -> Vec<u32> {
        let mut key = Vec::with_capacity(8 + self.boxes.len() * 6);
        key.extend(self.settings.min.to_array().map(f32::to_bits));
        key.extend(self.settings.max.to_array().map(f32::to_bits));
        key.push(self.settings.cell_size.to_bits());
        key.push(self.settings.radius.to_bits());
        for volume in &self.boxes {
            key.extend(volume.min.to_array().map(f32::to_bits));
            key.extend(volume.max.to_array().map(f32::to_bits));
        }
        key
    }
}

/// One cached map, with shared immutable pixels. Rejoining identical geometry
/// does not rebake or upload; a failed bake is cached too, until geometry changes.
#[derive(Default)]
pub struct Cache {
    key: Option<Vec<u32>>,
    field: Option<Arc<OcclusionField>>,
}

impl Cache {
    pub fn for_level(
        &mut self,
        map: &str,
        arena_half: f32,
        obstacles: &[Obstacle],
    ) -> Option<Arc<OcclusionField>> {
        if !matches!(map, MAP_HARBOR | MAP_FREIGHT_YARD | MAP_TRENCH_CITY) {
            self.key = None;
            self.field = None;
            return None;
        }
        let input = match source_geometry(map, arena_half, obstacles) {
            Ok(input) => input,
            Err(error) => {
                self.key = None;
                self.field = None;
                tracing::warn!(map, %error, "static contact shading disabled");
                return None;
            }
        };
        let key = input.key();
        if self.key.as_ref() == Some(&key) {
            return self.field.clone();
        }
        let started = web_time::Instant::now();
        self.field = match OcclusionField::bake(input.settings, &input.boxes) {
            Ok(field) => {
                debug_assert_eq!(field.dimensions(), input.dimensions);
                tracing::info!(
                    map,
                    boxes = input.boxes.len(),
                    dimensions = ?field.dimensions(),
                    bytes = field.texture_a().len() + field.texture_b().len(),
                    elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
                    "static contact shading baked"
                );
                Some(Arc::new(field))
            }
            Err(error) => {
                tracing::warn!(
                    map,
                    %error,
                    elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
                    "static contact shading disabled"
                );
                None
            }
        };
        self.key = Some(key);
        self.field.clone()
    }
}

fn source_geometry(
    map: &str,
    arena_half: f32,
    obstacles: &[Obstacle],
) -> Result<BakeInput, String> {
    if !arena_half.is_finite() || arena_half <= 0.0 {
        return Err("non-finite or non-positive arena extent".into());
    }
    let extent = arena_half + PADDING;
    let axis = (extent * 2.0 / CELL_SIZE).ceil() + 1.0;
    if !axis.is_finite() || axis > f32::from(MAX_AXIS) {
        return Err("arena exceeds static contact volume dimensions".into());
    }
    // Conversion is bounded by the preceding axis limit, never by server trust.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let axis = axis as u32;
    let dimensions = [axis, 17, axis];
    if dimensions.into_iter().map(u64::from).product::<u64>() > MAX_VOXELS {
        return Err("arena exceeds static contact voxel budget".into());
    }
    let settings = OcclusionSettings {
        min: Vec3::new(-extent, MIN_Y, -extent),
        max: Vec3::new(extent, MAX_Y, extent),
        cell_size: CELL_SIZE,
        radius: RADIUS,
    };
    let harbor = map == MAP_HARBOR;
    let mut boxes = Vec::with_capacity(obstacles.len().min(MAX_BOXES) + 6);
    for obstacle in obstacles {
        if obstacle.kind == Cover::Loot {
            continue;
        }
        push_box(
            &mut boxes,
            Vec3::new(obstacle.min[0], obstacle.base, obstacle.min[1]),
            Vec3::new(obstacle.max[0], obstacle.h, obstacle.max[1]),
        )?;
    }
    if harbor {
        // The asphalt extends beyond the terminal but ends at the quay's east
        // edge. A second thin slab follows the slightly higher concrete apron.
        // These are render heights, not changes to the simulation's y=0 floor.
        let reach = extent + RADIUS;
        push_box(
            &mut boxes,
            Vec3::new(-reach, -1.0, -reach),
            Vec3::new(arena_half, -0.025, reach),
        )?;
        push_box(
            &mut boxes,
            Vec3::new(30.0, -0.025, -reach),
            Vec3::new(arena_half, -0.012, reach),
        )?;
    } else {
        // Match the old-map cobble plane and the four visible boundary boxes
        // in online.rs. Their wall silhouettes are not in Level::obstacles.
        let floor_half = arena_half + 1.0;
        push_box(
            &mut boxes,
            Vec3::new(-floor_half, -1.0, -floor_half),
            Vec3::new(floor_half, 0.004, floor_half),
        )?;
        for (x, z, width, depth) in [
            (arena_half + 0.45, 0.0, 0.9, arena_half * 2.0 + 2.7),
            (-arena_half - 0.45, 0.0, 0.9, arena_half * 2.0 + 2.7),
            (0.0, arena_half + 0.45, arena_half * 2.0 + 2.7, 0.9),
            (0.0, -arena_half - 0.45, arena_half * 2.0 + 2.7, 0.9),
        ] {
            push_box(
                &mut boxes,
                Vec3::new(x - width * 0.5, 0.0, z - depth * 0.5),
                Vec3::new(x + width * 0.5, 3.5, z + depth * 0.5),
            )?;
        }
    }
    Ok(BakeInput {
        settings,
        boxes,
        dimensions,
    })
}

fn push_box(boxes: &mut Vec<OcclusionBox>, min: Vec3, max: Vec3) -> Result<(), String> {
    if !min.is_finite() || !max.is_finite() || min.cmpge(max).any() {
        return Err("invalid static contact occluder bounds".into());
    }
    if boxes.len() >= MAX_BOXES {
        return Err("level exceeds static contact occluder budget".into());
    }
    boxes.push(OcclusionBox { min, max });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::shooter::Level;

    #[test]
    fn all_three_authored_maps_fit_the_volume_budget_and_preserve_cover_bounds() {
        for (map, expected_boxes, expected_dimensions) in [
            (MAP_HARBOR, 72, [201, 17, 201]),
            (MAP_FREIGHT_YARD, 88, [105, 17, 105]),
            (MAP_TRENCH_CITY, 90, [105, 17, 105]),
        ] {
            let level = Level::named(map, 37);
            let input = source_geometry(map, level.arena_half, &level.obstacles).unwrap();
            assert_eq!(input.boxes.len(), expected_boxes, "{map}");
            assert_eq!(input.dimensions, expected_dimensions, "{map}");
            assert_eq!(
                input.key(),
                source_geometry(map, level.arena_half, &level.obstacles)
                    .unwrap()
                    .key()
            );
            for (obstacle, volume) in level
                .obstacles
                .iter()
                .filter(|o| o.kind != Cover::Loot)
                .zip(&input.boxes)
            {
                assert_eq!(
                    volume.min,
                    Vec3::new(obstacle.min[0], obstacle.base, obstacle.min[1])
                );
                assert_eq!(
                    volume.max,
                    Vec3::new(obstacle.max[0], obstacle.h, obstacle.max[1])
                );
            }
            let mut cache = Cache::default();
            let first = cache
                .for_level(map, level.arena_half, &level.obstacles)
                .expect(map);
            assert_eq!(first.dimensions(), expected_dimensions);
            let voxels = expected_dimensions
                .into_iter()
                .map(|n| usize::try_from(n).unwrap())
                .product::<usize>();
            assert_eq!(
                first.texture_a().len() + first.texture_b().len(),
                voxels * 8
            );
            let second = cache
                .for_level(map, level.arena_half, &level.obstacles)
                .unwrap();
            assert!(
                Arc::ptr_eq(&first, &second),
                "unchanged {map} must not rebake"
            );
        }
    }

    #[test]
    fn reward_boxes_do_not_enter_the_geometry_key_and_raised_roofs_keep_their_bottom() {
        let level = Level::trench_city();
        let with_rewards =
            source_geometry(MAP_TRENCH_CITY, level.arena_half, &level.obstacles).unwrap();
        let mut cover = level.obstacles.clone();
        cover.retain(|o| o.kind != Cover::Loot);
        let without_rewards = source_geometry(MAP_TRENCH_CITY, level.arena_half, &cover).unwrap();
        assert_eq!(with_rewards.key(), without_rewards.key());
        for (obstacle, volume) in cover.iter().zip(&without_rewards.boxes) {
            if obstacle.kind == Cover::Roof {
                assert!((volume.min.y - 2.5).abs() < 1e-6);
                assert!((volume.max.y - 2.9).abs() < 1e-6);
            }
        }
        cover[0].base += 0.1;
        let moved = source_geometry(MAP_TRENCH_CITY, level.arena_half, &cover).unwrap();
        assert_ne!(with_rewards.key(), moved.key());
    }

    #[test]
    fn oversized_or_invalid_levels_disable_contact_without_panicking() {
        for half in [f32::NAN, f32::INFINITY, 0.0, -1.0, 100.0] {
            assert!(source_geometry(MAP_FREIGHT_YARD, half, &[]).is_err());
        }
        let invalid = Obstacle::boxed(Cover::Crate, [0.0, 0.0], [1.0, 1.0], 2.0, 1.0);
        assert!(source_geometry(MAP_FREIGHT_YARD, 24.0, &[invalid]).is_err());
        let valid = Obstacle::boxed(Cover::Crate, [0.0, 0.0], [1.0, 1.0], 0.0, 1.0);
        assert!(source_geometry(MAP_FREIGHT_YARD, 24.0, &vec![valid; MAX_BOXES]).is_err());
        let mut cache = Cache::default();
        assert!(cache.for_level(MAP_FREIGHT_YARD, 100.0, &[]).is_none());
        assert!(cache.for_level("seeded", 24.0, &[valid]).is_none());
    }
}
