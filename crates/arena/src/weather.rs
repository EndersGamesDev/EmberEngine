//! Cosmetic outdoor conditions. No weather state enters movement, the seeded
//! level, input packets or the server's fixed-step simulation.

use arena_core::shooter::Obstacle;
use ember_engine::environment::rain_particles;
use ember_engine::glam::{Vec2, Vec3};
use ember_engine::{Environment, Fog, Particle, Weather};

/// An authored map's lighting and rain, selected from the server's map name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Climate {
    Yard,
    City,
    Harbor,
}

impl Climate {
    pub fn for_map(map: &str) -> Self {
        if map == "trench-city" {
            Self::City
        } else if map == "harbor" {
            Self::Harbor
        } else {
            Self::Yard
        }
    }

    /// The slow cloud and rain variation is a local presentation clock. Keep
    /// a useful daylight sun throughout; map visibility never turns to night.
    pub fn conditions(self, time: f32, forced: Option<Weather>) -> (Environment, Fog, f32) {
        let time = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let weather = forced.unwrap_or(match self {
            Self::Yard => Weather::Clear,
            Self::City => Weather::Rain,
            Self::Harbor => Weather::Cloudy,
        });
        let mut environment = Environment::outdoor(weather, time);
        let passing = (time * 0.025).sin() * 0.5 + 0.5;
        // Wind is in metres per second. It moves both the cloud layer and
        // the rain field without consuming the simulation's seeded RNG.
        environment.wind = Vec2::new(1.4, 0.45);
        let rain = match weather {
            Weather::Clear | Weather::Cloudy => 0.0,
            Weather::Rain => {
                let intensity = if forced.is_some() {
                    0.85
                } else {
                    0.6 + passing * 0.35
                };
                environment.wetness = intensity;
                intensity
            }
        };
        if forced.is_none() {
            environment.cloud_coverage = match self {
                Self::Yard => 0.28 + passing * 0.16,
                Self::City => 0.58 + passing * 0.16,
                Self::Harbor => 0.43 + passing * 0.12,
            };
        }
        match self {
            Self::Yard => {
                environment.sun_direction = Vec3::new(-0.55, 0.65, -0.42).normalize();
                environment.sun_color = Vec3::new(1.0, 0.84, 0.63);
            }
            Self::City => {
                environment.sun_direction = Vec3::new(-0.48, 0.72, -0.50).normalize();
                environment.sun_color = Vec3::new(1.0, 0.92, 0.80);
            }
            Self::Harbor => {
                environment.sun_direction = Vec3::new(-0.48, 0.76, -0.30).normalize();
                environment.sun_color = Vec3::new(1.0, 0.96, 0.87);
                environment.wind = Vec2::new(2.1, 0.65);
                // Include the full terminal and the taller quay cranes.
                environment.shadow_extent = 75.0;
            }
        }
        // Fog is post-tonemap, unlike the environment's linear sky colors.
        let fog = if rain > 0.0 {
            Fog {
                color: [0.47, 0.54, 0.60],
                density: 0.009,
            }
        } else if self == Self::Harbor {
            Fog {
                color: [0.60, 0.69, 0.74],
                density: 0.0028,
            }
        } else {
            Fog {
                color: [0.66, 0.60, 0.51],
                density: 0.005,
            }
        };
        (environment, fog, rain)
    }
}

/// A capture can pin conditions while cloud motion still follows game time.
/// Browser builds use the map's authored climate.
pub fn capture_override() -> Option<Weather> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        parse_override(&std::env::var("EMBER_WEATHER").ok()?)
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

#[cfg(any(test, not(target_arch = "wasm32")))]
fn parse_override(value: &str) -> Option<Weather> {
    match value.trim() {
        "clear" => Some(Weather::Clear),
        "cloudy" => Some(Weather::Cloudy),
        "rain" => Some(Weather::Rain),
        _ => None,
    }
}

/// Rain starts above the nearest visible outdoor surface. An obstacle's top
/// blocks rain whether it is a solid crate or an elevated tunnel roof: using
/// `base` as the shelter height would incorrectly rain inside the roof slab.
fn exposed(position: Vec3, half_size: Vec2, obstacles: &[Obstacle]) -> bool {
    let bottom = position.y - half_size.y;
    bottom > 0.025
        && !obstacles.iter().any(|o| {
            bottom <= o.h
                && position.x + half_size.x >= o.min[0]
                && position.x - half_size.x <= o.max[0]
                && position.z + half_size.x >= o.min[1]
                && position.z - half_size.x <= o.max[1]
        })
}

pub fn rain(
    camera: Vec3,
    time: f32,
    intensity: f32,
    wind: Vec2,
    obstacles: &[Obstacle],
) -> Vec<Particle> {
    let mut particles = rain_particles(camera, time, intensity, wind);
    particles.retain(|p| exposed(p.position, p.size * 0.5, obstacles));
    particles
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_core::shooter::{Cover, Level};

    #[test]
    fn every_trench_roof_shelters_the_tunnel_and_allows_rain_above() {
        let level = Level::trench_city();
        let half_size = Vec2::new(0.01, 0.1);
        let roofs: Vec<_> = level
            .obstacles
            .iter()
            .filter(|o| o.kind == Cover::Roof)
            .collect();
        assert_eq!(roofs.len(), 4);
        for roof in roofs {
            let center = Vec3::new(
                f32::midpoint(roof.min[0], roof.max[0]),
                roof.base * 0.5,
                f32::midpoint(roof.min[1], roof.max[1]),
            );
            assert!(!exposed(center, half_size, &level.obstacles));
            assert!(!exposed(
                Vec3::new(center.x, roof.h, center.z),
                half_size,
                &level.obstacles
            ));
            assert!(exposed(
                Vec3::new(center.x, roof.h + 0.5, center.z),
                half_size,
                &level.obstacles
            ));
        }
    }

    #[test]
    fn rain_does_not_cross_cover_or_the_floor_and_remains_visible_outside_shelter() {
        let level = Level::trench_city();
        let camera = Vec3::new(0.0, 1.6, 12.5);
        let drops = rain(camera, 12.0, 0.85, Vec2::new(1.4, 0.45), &level.obstacles);
        assert!(
            !drops.is_empty(),
            "outdoor rain remains visible from a tunnel"
        );
        assert!(
            drops
                .iter()
                .all(|p| exposed(p.position, p.size * 0.5, &level.obstacles))
        );
        assert!(!exposed(
            Vec3::new(15.0, 0.05, 0.0),
            Vec2::new(0.01, 0.1),
            &[]
        ));
    }

    #[test]
    fn map_climates_keep_daylight_and_capture_overrides_control_rain() {
        for time in [0.0, 30.0, 120.0, 600.0] {
            let (yard, _, yard_rain) = Climate::for_map("freight-yard").conditions(time, None);
            let (city, _, city_rain) = Climate::for_map("trench-city").conditions(time, None);
            assert!(yard.enabled && city.enabled);
            assert!(yard.sun_direction.y > 0.5 && city.sun_direction.y > 0.5);
            assert!(yard_rain.abs() < f32::EPSILON);
            assert!(city_rain > 0.5);
            assert!(city.cloud_coverage > yard.cloud_coverage);
            let (harbor, fog, harbor_rain) = Climate::for_map("harbor").conditions(time, None);
            assert!(harbor.enabled && harbor.sun_direction.y > 0.5);
            assert_eq!(harbor_rain, 0.0);
            assert!(fog.density < 0.004, "keep long harbor routes readable");
            for climate in [Climate::Yard, Climate::City, Climate::Harbor] {
                assert!(climate.conditions(time, Some(Weather::Clear)).2.abs() < f32::EPSILON);
                assert!(climate.conditions(time, Some(Weather::Rain)).2 > 0.8);
            }
        }
        assert!(parse_override("rain").is_some());
        assert!(parse_override("cloudy").is_some());
        assert!(parse_override("clear").is_some());
        assert!(parse_override("storm").is_none());
    }
}
