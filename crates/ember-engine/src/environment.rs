//! Reusable outdoor lighting and presentation-only weather.
//!
//! Weather never feeds the fixed-step simulation or its RNG. Rain is evaluated
//! from a fixed spatial seed and elapsed presentation time, so frame rate does
//! not change the number of drops and a paused camera does not stop the sky.

use glam::{Vec2, Vec3};

/// Lighting and sky state consumed by the renderer. Opt in with `outdoor`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Environment {
    pub enabled: bool,
    /// Unit vector from the world toward the sun, shared by sky and shadows.
    pub sun_direction: Vec3,
    pub sun_color: Vec3,
    pub sun_intensity: f32,
    pub sky_zenith: Vec3,
    pub sky_horizon: Vec3,
    pub cloud_coverage: f32,
    /// Horizontal wind (world X/Z metres per second).
    pub wind: Vec2,
    pub time: f32,
    /// Wet reflection strength on instances explicitly marked wettable.
    pub wetness: f32,
    /// Half-width in metres of the camera-centred directional shadow volume.
    pub shadow_extent: f32,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            enabled: false,
            sun_direction: Vec3::new(0.4, 1.0, 0.3).normalize(),
            sun_color: Vec3::new(1.0, 0.95, 0.85),
            sun_intensity: 1.15,
            sky_zenith: Vec3::new(0.12, 0.32, 0.62),
            sky_horizon: Vec3::new(0.64, 0.76, 0.87),
            cloud_coverage: 0.35,
            wind: Vec2::new(1.4, 0.5),
            time: 0.0,
            wetness: 0.0,
            shadow_extent: 55.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weather {
    #[default]
    Clear,
    Cloudy,
    Rain,
}

impl Environment {
    #[must_use]
    pub fn outdoor(weather: Weather, time: f32) -> Self {
        let mut env = Self {
            enabled: true,
            time: finite_time(time),
            sun_direction: Vec3::new(-0.55, 0.7, 0.45).normalize(),
            ..Self::default()
        };
        match weather {
            Weather::Clear => {}
            Weather::Cloudy => {
                env.cloud_coverage = 0.65;
                env.sun_intensity = 0.85;
                env.sky_zenith = Vec3::new(0.22, 0.34, 0.49);
            }
            Weather::Rain => {
                env.cloud_coverage = 0.82;
                env.sun_intensity = 0.55;
                env.sky_zenith = Vec3::new(0.12, 0.20, 0.30);
                env.sky_horizon = Vec3::new(0.42, 0.51, 0.60);
                env.sun_color = Vec3::new(0.85, 0.91, 1.0);
                env.wind = Vec2::new(2.2, 0.7);
                env.wetness = 0.85;
            }
        }
        env
    }
}

/// A soft camera-facing quad, in world metres. No collision or authority.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    pub color: Vec3,
    pub size: Vec2,
    pub opacity: f32,
}

pub const MAX_RAIN_PARTICLES: u16 = 320;
const RAIN_RADIUS: f32 = 18.0;
const RAIN_HEIGHT: f32 = 18.0;

fn finite_time(time: f32) -> f32 {
    // Presentation clocks wrap daily to keep hash coordinates well conditioned.
    if time.is_finite() {
        time.max(0.0) % 86_400.0
    } else {
        0.0
    }
}

// A small integer hash is evaluated only from the drop index. No mutable RNG.
#[allow(clippy::cast_precision_loss)]
fn fraction(seed: u32) -> f32 {
    let mut n = seed.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    n = ((n >> ((n >> 28) + 4)) ^ n).wrapping_mul(277_803_737);
    ((n ^ (n >> 22)) & 0xffff) as f32 / 65_536.0
}

/// Camera-local rain volume with a fixed population and analytic trajectories.
/// The caller can discard drops beneath world roofs before rendering them.
#[must_use]
pub fn rain_particles(camera: Vec3, time: f32, intensity: f32, wind: Vec2) -> Vec<Particle> {
    if !camera.is_finite() || !intensity.is_finite() || intensity <= 0.0 {
        return Vec::new();
    }
    let intensity = intensity.min(1.0);
    let time = finite_time(time);
    let wind = if wind.is_finite() {
        wind.clamp(Vec2::splat(-50.0), Vec2::splat(50.0))
    } else {
        Vec2::ZERO
    };
    let origin = Vec2::new(camera.x, camera.z);
    let mut particles = Vec::with_capacity(usize::from(MAX_RAIN_PARTICLES));
    for id in 0..MAX_RAIN_PARTICLES {
        if f32::from(id) >= f32::from(MAX_RAIN_PARTICLES) * intensity {
            break;
        }
        let seed = u32::from(id) * 3;
        let x = (wind.x.mul_add(time, fraction(seed) * RAIN_RADIUS * 2.0) - origin.x)
            .rem_euclid(RAIN_RADIUS * 2.0)
            - RAIN_RADIUS
            + origin.x;
        let z = (wind.y.mul_add(time, fraction(seed + 1) * RAIN_RADIUS * 2.0) - origin.y)
            .rem_euclid(RAIN_RADIUS * 2.0)
            - RAIN_RADIUS
            + origin.y;
        let fall_rate = fraction(seed + 2).mul_add(5.0, 12.0);
        let y = (fall_rate.mul_add(-time, fraction(seed + 2) * RAIN_HEIGHT) - camera.y)
            .rem_euclid(RAIN_HEIGHT)
            + camera.y
            - 3.0;
        let distance = Vec2::new(x, z).distance(origin);
        let fade = (1.0 - distance / RAIN_RADIUS).clamp(0.0, 1.0);
        particles.push(Particle {
            position: Vec3::new(x, y, z),
            color: Vec3::new(0.68, 0.80, 0.92),
            size: Vec2::new(0.022, fraction(seed).mul_add(0.20, 0.40)),
            opacity: 0.48 * fade,
        });
    }
    particles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_is_opt_in_and_weather_uses_one_sun() {
        assert!(!Environment::default().enabled);
        for weather in [Weather::Clear, Weather::Cloudy, Weather::Rain] {
            let env = Environment::outdoor(weather, 40.0);
            assert!(env.enabled);
            assert!((env.sun_direction.length() - 1.0).abs() < 0.0001);
            assert!(env.sun_direction.y > 0.0);
            assert_eq!(env.time, 40.0);
        }
        assert!(Environment::outdoor(Weather::Rain, 0.0).wetness > 0.5);
    }

    #[test]
    fn rain_has_a_bounded_population_and_repeatable_motion() {
        let eye = Vec3::new(5.0, 2.0, -3.0);
        let a = rain_particles(eye, 3.0, 1.0, Vec2::X);
        assert_eq!(a.len(), usize::from(MAX_RAIN_PARTICLES));
        assert_eq!(a, rain_particles(eye, 3.0, 1.0, Vec2::X));
        assert_ne!(a, rain_particles(eye, 3.1, 1.0, Vec2::X));
        for p in a {
            assert!((p.position.x - eye.x).abs() <= RAIN_RADIUS);
            assert!((p.position.z - eye.z).abs() <= RAIN_RADIUS);
            assert!(p.position.y >= eye.y - 3.0 && p.position.y < eye.y + 15.0);
            assert!((0.0..=1.0).contains(&p.opacity));
        }
    }

    #[test]
    fn weather_inputs_cannot_create_unbounded_or_invalid_particles() {
        assert_eq!(rain_particles(Vec3::ZERO, 0.0, 0.0, Vec2::ZERO), []);
        assert_eq!(rain_particles(Vec3::NAN, 0.0, 1.0, Vec2::ZERO), []);
        assert_eq!(rain_particles(Vec3::ZERO, 0.0, f32::NAN, Vec2::ZERO), []);
        let rain = rain_particles(Vec3::ZERO, f32::NAN, 10.0, Vec2::NAN);
        assert_eq!(rain.len(), usize::from(MAX_RAIN_PARTICLES));
        assert!(rain.iter().all(|p| p.position.is_finite()));
    }
}
