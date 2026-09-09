//! Small opaque scratch ribbons; no decal pass or per-instance texture copies.
use ember_engine::{Frame, Instance, MeshData, MeshVertex};
use end_game_core::{
    Dungeon,
    layout::{self, Aabb, Surface},
};
use glam::{Mat3, Quat, Vec3};

pub struct Marks {
    mesh: u32,
}
impl Marks {
    pub fn load(meshes: &mut Vec<MeshData>) -> Self {
        // A tapered, uneven gouge. Local X follows the actual blade tangent.
        let points = [
            (-0.5, 0.),
            (-0.25, -0.44),
            (0.18, -0.5),
            (0.5, 0.),
            (0.14, 0.30),
            (-0.30, 0.5),
        ];
        let mut vertices = Vec::new();
        for i in 1..points.len() - 1 {
            for k in [0, i, i + 1] {
                vertices.push(MeshVertex {
                    pos: [points[k].0, points[k].1, 0.],
                    normal: [0., 0., 1.],
                    uv: [0., 0.],
                });
            }
        }
        meshes.push(MeshData {
            vertices,
            texture: None,
        });
        Self {
            mesh: meshes.len() as u32,
        }
    }
    pub fn stamp(
        &self,
        frame: &mut Frame,
        point: Vec3,
        normal: Vec3,
        tangent: Vec3,
        length: f32,
        width: f32,
        iron: bool,
    ) {
        self.material_stamp(frame, point, normal, tangent, length, width, iron, false);
    }
    fn material_stamp(
        &self,
        frame: &mut Frame,
        point: Vec3,
        normal: Vec3,
        tangent: Vec3,
        length: f32,
        width: f32,
        iron: bool,
        timber: bool,
    ) {
        let normal = normal.normalize_or_zero();
        let tangent = (tangent - normal * tangent.dot(normal)).normalize_or_zero();
        if normal.length_squared() < 0.9 || tangent.length_squared() < 0.9 {
            return;
        }
        let across = normal.cross(tangent);
        let rotation = Quat::from_mat3(&Mat3::from_cols(tangent, across, normal));
        for (position, scale, color) in [
            (
                point,
                Vec3::new(length, width, 1.),
                if timber {
                    Vec3::new(0.31, 0.20, 0.10)
                } else {
                    Vec3::new(0.055, 0.043, 0.033)
                },
            ),
            (
                point + across * width * 0.31 + normal * 0.0007,
                Vec3::new(length * 0.91, width * 0.15, 1.),
                if iron {
                    Vec3::new(0.48, 0.49, 0.46)
                } else if timber {
                    Vec3::new(0.43, 0.30, 0.15)
                } else {
                    Vec3::new(0.34, 0.29, 0.20)
                },
            ),
        ] {
            frame.instances.push(
                Instance::new(position, scale, color)
                    .with_mesh(self.mesh)
                    .with_rot(rotation)
                    .with_surface(if iron { 0.56 } else { 0.95 }, if iron { 0.4 } else { 0. })
                    .without_shadow(),
            );
        }
    }
    pub fn draw_surfaces(&self, frame: &mut Frame, game: &Dungeon) {
        for hit in game.surface_impacts.impacts() {
            if !hit.persistent || hit.point.distance(frame.camera.eye) > 24. {
                continue;
            }
            let bounds = surface_bounds(hit.surface_id);
            let Some(bounds) = bounds else {
                continue;
            };
            let desired = 0.17 + hit.strength * 0.20;
            let width = 0.010 + hit.strength * 0.008;
            let Some(length) =
                face_length(bounds, hit.point, hit.normal, hit.tangent, desired, width)
            else {
                continue;
            };
            // Beveled architectural dressing extends .021 m beyond its collision
            // plane. Keep the gouge above it, rather than hidden in the backing.
            let lift = if matches!(hit.material, Surface::Stone | Surface::Paving) {
                0.027
            } else {
                0.004
            };
            let point = surface_point(hit, hit.point + hit.normal * lift);
            self.material_stamp(
                frame,
                point,
                hit.normal,
                hit.tangent,
                length,
                width,
                hit.material == Surface::Iron,
                hit.material == Surface::Timber,
            );
        }
    }
}
fn surface_point(hit: &end_game_core::sword::SurfaceImpact, default: Vec3) -> Vec3 {
    if hit.surface_id >= 10_000 && hit.material == Surface::Timber && hit.normal.y > 0.9 {
        // The fitted oak boards are centered at -.065 with thickness .13:
        // their broad top is exactly zero. The .015 collision allowance must
        // not lift a scratch above the wood. Keep a tight two-millimetre offset.
        Vec3::new(hit.point.x, 0.002, hit.point.z)
    } else {
        default
    }
}
fn surface_bounds(id: u32) -> Option<Aabb> {
    if id < 10_000 {
        layout::castle()
            .solids
            .iter()
            .find(|s| s.index as u32 == id)
            .map(|s| s.bounds)
    } else {
        layout::basement_strike_surfaces()
            .get((id - 10_000) as usize)
            .map(|s| s.bounds)
    }
}
fn face_length(
    bounds: Aabb,
    point: Vec3,
    normal: Vec3,
    tangent: Vec3,
    desired: f32,
    width: f32,
) -> Option<f32> {
    let across = normal.cross(tangent).normalize_or_zero();
    let mut half = desired * 0.5;
    for axis in 0..3 {
        if normal[axis].abs() > 0.9 {
            continue;
        }
        let margin = (point[axis] - bounds.min[axis]).min(bounds.max[axis] - point[axis])
            - across[axis].abs() * width * 0.5;
        if margin < 0. {
            return None;
        }
        if tangent[axis].abs() > 0.0001 {
            half = half.min(margin / tangent[axis].abs());
        }
    }
    (half >= 0.012).then_some(half * 2.)
}

/// Bounded additive response in the body's bind frame. Both renderers use the
/// same five anatomical slots; core carries interrupted contributions exactly.
#[derive(Clone, Copy, Debug, Default)]
pub struct Response {
    pub hip: Vec3,
    pub torso: Vec3,
    pub head: Vec3,
}
pub fn response(
    reaction: Option<end_game_core::HitReaction>,
    yaw: f32,
    remaining: f32,
) -> Response {
    let mut out = Response::default();
    let Some(reaction) = reaction else {
        return out;
    };
    let inverse = Quat::from_rotation_y(yaw);
    for (zone, v) in reaction.vectors().into_iter().enumerate() {
        let v = inverse * v * remaining;
        match zone {
            0 => {
                out.head += Vec3::new(v.y * 0.16 + v.z * 0.20, v.x * 0.12, -v.x * 0.30);
                out.torso += Vec3::new(v.z * 0.04, 0., -v.x * 0.025);
            }
            1 | 2 => {
                out.torso += Vec3::new(
                    v.y * 0.10 + v.z * 0.10,
                    v.z * if zone == 1 { 0.12 } else { -0.12 },
                    -v.x * 0.15,
                )
            }
            _ => {
                out.hip += Vec3::new(v.x * 0.020, v.y.min(0.) * 0.035, 0.);
                out.torso += Vec3::new(v.y * 0.025, 0., -v.x * 0.04);
            }
        }
    }
    out
}
pub fn response_rotation(v: Vec3) -> Quat {
    Quat::from_rotation_x(v.x) * Quat::from_rotation_y(v.y) * Quat::from_rotation_z(v.z)
}

/// Closest point on a triangle, including its boundary. Armor stamps project
/// onto loaded rigid geometry rather than floating on the broad damage box.
pub fn closest_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = (va + vb + vc).recip();
    a + ab * (vb * denom) + ac * (vc * denom)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn surface_ribbons_clip_to_the_contact_face() {
        let bounds = Aabb::new([-1., 0., -1.], [1., 2., 1.]);
        for normal in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X] {
            let tangent = normal.any_orthonormal_vector();
            let point = Vec3::new(0., 1., 0.) + normal;
            assert!(face_length(bounds, point, normal, tangent, 0.3, 0.02).is_some());
        }
        assert!(
            face_length(
                bounds,
                Vec3::new(0.999, 1., 1.),
                Vec3::Z,
                Vec3::X,
                0.3,
                0.02
            )
            .is_none()
        );
        let p = closest_triangle(Vec3::new(0.2, 0.3, 1.), Vec3::ZERO, Vec3::X, Vec3::Y);
        assert!(p.distance(Vec3::new(0.2, 0.3, 0.)) < 0.00001);
    }
    #[test]
    fn timber_mark_is_flush_with_actual_plank_top_and_exposes_wood() {
        use end_game_core::sword::SurfaceImpact;
        let hit = SurfaceImpact {
            id: 1,
            time: 0.,
            point: Vec3::new(0.2905514, 0.015, 0.32384756),
            normal: Vec3::Y,
            tangent: Vec3::X,
            material: Surface::Timber,
            surface_id: 10_000 + layout::basement_strike_surfaces().len() as u32 - 1,
            persistent: true,
            strength: 0.45,
        };
        let point = surface_point(&hit, hit.point + Vec3::Y * 0.004);
        let mut meshes = vec![
            MeshData::textured_box(1., None),
            MeshData::textured_box(1., None),
        ];
        let cell = super::super::cell::build(&mut meshes);
        let mut top = f32::NEG_INFINITY;
        for instance in &cell.world {
            if instance.mesh == 0 {
                continue;
            }
            for tri in meshes[instance.mesh as usize - 1].vertices.chunks_exact(3) {
                let v: Vec<_> = tri
                    .iter()
                    .map(|v| {
                        instance.position
                            + instance.rot * (Vec3::from_array(v.pos) * instance.scale)
                    })
                    .collect();
                let normal = (v[1] - v[0]).cross(v[2] - v[0]).normalize_or_zero();
                if normal.y < 0.9 {
                    continue;
                }
                let projected = closest_triangle(point, v[0], v[1], v[2]);
                if (projected.x - point.x).abs() < 0.00001
                    && (projected.z - point.z).abs() < 0.00001
                    && projected.y < 0.10
                {
                    top = top.max(projected.y);
                }
            }
        }
        assert!(top.abs() < 0.00001, "authored oak top changed: {top}");
        assert!((point.y - top - 0.002).abs() < 0.00001);
        let marks = Marks::load(&mut meshes);
        let mut frame = Frame::default();
        marks.material_stamp(
            &mut frame,
            point,
            hit.normal,
            hit.tangent,
            0.26,
            0.0136,
            false,
            true,
        );
        assert_eq!(frame.instances.len(), 2);
        assert_eq!(frame.instances[0].scale, Vec3::new(0.26, 0.0136, 1.));
    }
}
