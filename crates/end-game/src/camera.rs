//! Keep stair treads physical while the first-person eye eases over each riser.
#[derive(Default)]
pub struct StairEye {
    previous_height: Option<f32>,
    lift: f32,
}

impl StairEye {
    pub fn update(&mut self, height: f32, grounded: bool, dt: f32) -> f32 {
        let previous = self.previous_height.replace(height).unwrap_or(height);
        let rise = height - previous;
        if !grounded || rise.abs() > 0.8 {
            self.lift = 0.0;
        } else {
            self.lift = (self.lift - rise).clamp(-0.35, 0.35);
            self.lift *= (-14.0 * dt.clamp(0.0, 0.1)).exp();
        }
        self.lift
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stair_eye_eases_both_directions_without_moving_the_feet() {
        for rise in [-0.25, 0.25] {
            let mut eye = StairEye::default();
            eye.update(6.0, true, 1.0 / 60.0);
            let lift = eye.update(6.0 + rise, true, 1.0 / 60.0);
            assert!(lift * rise < 0.0);
            assert!(lift.abs() < rise.abs());
            let mut previous = lift.abs();
            for _ in 0..60 {
                let next = eye.update(6.0 + rise, true, 1.0 / 60.0).abs();
                assert!(next <= previous);
                previous = next;
            }
            assert!(previous < 0.00001);
        }
    }
    #[test]
    fn pause_holds_the_eye_and_airborne_or_relocated_views_reset() {
        let mut eye = StairEye::default();
        eye.update(0.0, true, 0.016);
        let lift = eye.update(0.25, true, 0.016);
        assert_eq!(eye.update(0.25, true, 0.0), lift);
        assert_eq!(eye.update(0.4, false, 0.016), 0.0);
        assert_eq!(eye.update(22.0, true, 0.016), 0.0);
    }
}
