//! Authored interaction timing in fixed-step simulation; geometry and rendering consume it.
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionKind {
    Board,
    Key,
    Lock,
    Sword,
}

impl InteractionKind {
    pub fn duration(self) -> f32 {
        match self {
            Self::Board => 1.45,
            Self::Key => 2.15,
            Self::Lock => 2.05,
            Self::Sword => 2.3,
        }
    }
    pub fn contact_time(self) -> f32 {
        match self {
            Self::Sword => 0.8,
            _ => 0.65,
        }
    }
    pub fn commit_time(self) -> f32 {
        match self {
            Self::Board => 1.08,
            Self::Key => 0.95,
            Self::Lock => 1.42,
            Self::Sword => 1.94,
        }
    }
    pub fn target(self) -> Vec3 {
        match self {
            Self::Board => board_grip(0.0),
            Self::Key => Vec3::new(-1.25, 0.055, 2.08),
            Self::Lock => Vec3::new(0.45, 1.10, 0.20),
            Self::Sword => Vec3::new(2.6, 1.55, -2.83),
        }
    }
    pub fn approach(self) -> Vec3 {
        match self {
            Self::Board => Vec3::new(-1.25, 0.0, 3.12),
            Self::Key => Vec3::new(-1.25, 0.0, 2.61),
            Self::Lock => Vec3::new(0.43, 0.0, 0.70),
            Self::Sword => Vec3::new(2.6, 0.0, -2.40),
        }
    }
    pub fn head_height(self) -> f32 {
        match self {
            Self::Board | Self::Key => 0.65,
            Self::Lock => 1.45,
            Self::Sword => 1.72,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Interaction {
    pub kind: InteractionKind,
    pub elapsed: f32,
    pub origin: Vec3,
    pub committed: bool,
}

pub fn ease(a: f32, b: f32, time: f32) -> f32 {
    let t = ((time - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
impl Interaction {
    pub fn focus(self) -> f32 {
        ease(0.0, 0.35, self.elapsed)
            * (1.0
                - ease(
                    self.kind.duration() - 0.3,
                    self.kind.duration(),
                    self.elapsed,
                ))
    }
    pub fn reach(self) -> f32 {
        ease(0.25, self.kind.contact_time(), self.elapsed)
    }
    pub fn grasp(self) -> f32 {
        ease(
            self.kind.contact_time() - 0.04,
            self.kind.contact_time() + 0.16,
            self.elapsed,
        )
    }
    pub fn manipulate(self) -> f32 {
        ease(
            self.kind.contact_time() + 0.16,
            self.kind.commit_time().max(self.kind.contact_time() + 0.3),
            self.elapsed,
        )
    }
    pub fn recover(self) -> f32 {
        ease(
            self.kind.duration() - 0.36,
            self.kind.duration(),
            self.elapsed,
        )
    }
}

pub fn board_pose(open: f32) -> (Vec3, glam::Quat) {
    let rot = glam::Quat::from_rotation_x(-0.10 * open);
    (
        Vec3::new(
            -1.25 + 0.42 * open,
            0.045 + 0.16 * (std::f32::consts::PI * open).sin() + 0.04 * open,
            2.1,
        ),
        rot,
    )
}
pub fn board_grip(open: f32) -> Vec3 {
    let (p, r) = board_pose(open);
    p + r * Vec3::new(0.0, 0.04, 0.55)
}
