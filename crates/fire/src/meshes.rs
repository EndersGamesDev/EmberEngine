//! Turning the generated GLB props into meshes this renderer can light.
//!
//! `gen3d_mv.py` runs `Hunyuan3D`'s *shape* pipeline, so what lands in
//! `assets/models/fire/` is geometry and nothing else: POSITION only, no
//! `NORMAL`, no `TEXCOORD_0`, no material. `ember_engine::assets::load_glb`
//! tolerates that by defaulting every normal to +Y and every UV to (0,0) —
//! which loads, but renders as a flat silhouette with no shading at all and
//! samples a single texel of any texture you attach.
//!
//! Both gaps are filled by opt-in passes rather than inside `load_glb`: its
//! defaults are shared with the arena's character parts, which are
//! POSITION-only too, and "improving" them there would silently restyle a
//! live game. The two passes started life in this file; when the arena's
//! generated props (v13) became their second consumer they moved to
//! `ember_engine::assets`, and this module re-exports them so nothing here
//! changed in behaviour.

use ember_engine::glam::Vec3;
use ember_engine::{MeshData, MeshVertex, TextureData};

/// A hand-authored cross-section shell in metres, with +Z as the nose.
/// Separate glass and wheels avoid the merged generated asset's flat livery.
#[must_use]
#[allow(clippy::too_many_lines)] // Authored cross sections and seam closure stay together.
pub fn car_body(vehicle: u8, glass: bool) -> MeshData {
    let sections: Vec<[f32; 4]> = match (vehicle, glass) {
        (1, false) => vec![
            [-1.98, 0.79, 0.30, 0.62],
            [-1.45, 0.90, 0.29, 0.76],
            [-0.55, 0.86, 0.29, 0.72],
            [0.65, 0.85, 0.29, 0.65],
            [1.52, 0.89, 0.30, 0.60],
            [2.00, 0.74, 0.34, 0.49],
        ],
        (2, false) => vec![
            [-2.40, 0.91, 0.32, 0.88],
            [-1.60, 1.00, 0.30, 0.92],
            [-0.45, 0.99, 0.30, 0.91],
            [0.90, 0.98, 0.30, 0.90],
            [1.85, 0.98, 0.32, 0.84],
            [2.42, 0.91, 0.35, 0.74],
        ],
        (_, false) => vec![
            [-2.27, 0.87, 0.30, 0.76],
            [-1.55, 0.98, 0.29, 0.88],
            [-0.60, 0.94, 0.28, 0.83],
            [0.82, 0.94, 0.28, 0.77],
            [1.75, 0.97, 0.30, 0.68],
            [2.30, 0.79, 0.35, 0.54],
        ],
        (1, true) => vec![
            [-0.95, 0.69, 0.65, 0.76],
            [-0.48, 0.62, 0.67, 1.12],
            [0.10, 0.59, 0.65, 1.10],
            [0.72, 0.68, 0.61, 0.68],
        ],
        (2, true) => vec![
            [-1.42, 0.78, 0.85, 0.91],
            [-0.78, 0.72, 0.86, 1.42],
            [0.10, 0.71, 0.85, 1.40],
            [0.86, 0.80, 0.84, 0.91],
        ],
        (_, true) => vec![
            [-1.48, 0.74, 0.78, 0.86],
            [-0.69, 0.66, 0.78, 1.32],
            [0.11, 0.64, 0.74, 1.29],
            [0.88, 0.74, 0.69, 0.79],
        ],
    };
    let sections = if glass {
        sections
    } else {
        curved_sections(&sections)
    };
    let mut vertices = Vec::new();
    let ring = |[z, w, base, top]: [f32; 4]| {
        [
            Vec3::new(-w * 0.92, base, z),
            Vec3::new(-w, base + 0.10, z),
            Vec3::new(-w * 0.96, top - 0.10, z),
            Vec3::new(-w * 0.76, top, z),
            Vec3::new(w * 0.76, top, z),
            Vec3::new(w * 0.96, top - 0.10, z),
            Vec3::new(w, base + 0.10, z),
            Vec3::new(w * 0.92, base, z),
        ]
    };
    for pair in sections.windows(2) {
        let a = ring(pair[0]);
        let b = ring(pair[1]);
        for i in 0..8 {
            let j = (i + 1) % 8;
            triangle(&mut vertices, a[i], b[i], b[j]);
            triangle(&mut vertices, a[i], b[j], a[j]);
        }
    }
    for (index, section) in [sections[0], sections[sections.len() - 1]]
        .iter()
        .enumerate()
    {
        let points = ring(*section);
        for i in 1..7 {
            if index == 0 {
                triangle(&mut vertices, points[0], points[i], points[i + 1]);
            } else {
                triangle(&mut vertices, points[0], points[i + 1], points[i]);
            }
        }
    }
    let mut mesh = face_normals(MeshData {
        vertices,
        texture: None,
    });
    if !glass {
        smooth_normals(&mut mesh);
    }
    for vertex in &mut mesh.vertices {
        vertex.uv = [(vertex.pos[2] + 2.5) / 5.0, vertex.pos[1] / 1.5];
    }
    mesh.texture = Some(crate::texgen::car_finish(128, glass));
    mesh
}

#[allow(clippy::many_single_char_names)] // Standard four-control-point Catmull-Rom formula.
fn curved_sections(sections: &[[f32; 4]]) -> Vec<[f32; 4]> {
    let mut result = Vec::new();
    for i in 0..sections.len() - 1 {
        let [a, b, c, d] = [
            sections[i.saturating_sub(1)],
            sections[i],
            sections[i + 1],
            sections[(i + 2).min(sections.len() - 1)],
        ];
        for step in 0_u8..4 {
            let t = f32::from(step) / 4.0;
            let mut row = [0.0; 4];
            for component in 0..4 {
                let [a, b, c, d] = [a[component], b[component], c[component], d[component]];
                row[component] = 0.5
                    * (2.0 * b
                        + (-a + c) * t
                        + (2.0 * a - 5.0 * b + 4.0 * c - d) * t * t
                        + (-a + 3.0 * b - 3.0 * c + d) * t * t * t);
            }
            result.push(row);
        }
    }
    result.push(sections[sections.len() - 1]);
    result
}

fn smooth_normals(mesh: &mut MeshData) {
    let mut sums = std::collections::BTreeMap::<[u32; 3], Vec3>::new();
    for vertex in &mesh.vertices {
        *sums.entry(vertex.pos.map(f32::to_bits)).or_default() += Vec3::from(vertex.normal);
    }
    for vertex in &mut mesh.vertices {
        let n = sums[&vertex.pos.map(f32::to_bits)].normalize_or_zero();
        if n.length_squared() > 0.1 {
            vertex.normal = n.to_array();
        }
    }
}

fn triangle(vertices: &mut Vec<MeshVertex>, a: Vec3, b: Vec3, c: Vec3) {
    for (p, uv) in [(a, [0.0, 0.0]), (b, [0.0, 1.0]), (c, [1.0, 1.0])] {
        vertices.push(MeshVertex {
            pos: p.to_array(),
            normal: Vec3::Y.to_array(),
            uv,
        });
    }
}

/// Cylinder on the X axle, allowing independent wheel spin and steering.
#[must_use]
pub fn wheel(radius: f32, width: f32) -> MeshData {
    let mut vertices = Vec::new();
    for i in 0_u16..20 {
        let a = f32::from(i) * std::f32::consts::TAU / 20.0;
        let b = f32::from(i + 1) * std::f32::consts::TAU / 20.0;
        let p = |x: f32, angle: f32| Vec3::new(x, angle.cos() * radius, angle.sin() * radius);
        let [a0, a1, b0, b1] = [
            p(-width * 0.5, a),
            p(width * 0.5, a),
            p(-width * 0.5, b),
            p(width * 0.5, b),
        ];
        triangle(&mut vertices, a0, b0, b1);
        triangle(&mut vertices, a0, b1, a1);
        triangle(&mut vertices, Vec3::X * (-width * 0.5), b0, a0);
        triangle(&mut vertices, Vec3::X * (width * 0.5), a1, b1);
    }
    face_normals(MeshData {
        vertices,
        texture: None,
    })
}

/// Opaque contact patch; the renderer has no alpha blending.
#[must_use]
pub fn disc() -> MeshData {
    let mut vertices = Vec::new();
    for i in 0_u16..24 {
        let a = f32::from(i) * std::f32::consts::TAU / 24.0;
        let b = f32::from(i + 1) * std::f32::consts::TAU / 24.0;
        triangle(
            &mut vertices,
            Vec3::ZERO,
            Vec3::new(a.cos(), 0.0, a.sin()),
            Vec3::new(b.cos(), 0.0, b.sin()),
        );
    }
    MeshData {
        vertices,
        texture: None,
    }
}

pub use ember_engine::assets::{face_normals, planar_uvs};

/// An enclosing sky with identical lighting on every side. The scene pass
/// supports opaque geometry; this stays behind every playable object.
#[must_use]
pub fn sky() -> MeshData {
    let mut vertices = Vec::new();
    let point = |latitude: u16, longitude: u16| {
        let latitude = f32::from(latitude) * std::f32::consts::PI / 16.0;
        let longitude = f32::from(longitude) * std::f32::consts::TAU / 24.0;
        Vec3::new(
            latitude.sin() * longitude.cos(),
            latitude.cos(),
            latitude.sin() * longitude.sin(),
        )
    };
    for latitude in 0..16 {
        for longitude in 0..24 {
            let [a, b, c, d] = [
                point(latitude, longitude),
                point(latitude + 1, longitude),
                point(latitude + 1, longitude + 1),
                point(latitude, longitude + 1),
            ];
            triangle(&mut vertices, a, b, c);
            triangle(&mut vertices, a, c, d);
            let u0 = f32::from(longitude) / 24.0;
            let u1 = f32::from(longitude + 1) / 24.0;
            let v0 = f32::from(latitude) / 16.0;
            let v1 = f32::from(latitude + 1) / 16.0;
            let start = vertices.len() - 6;
            for (vertex, uv) in vertices[start..].iter_mut().zip([
                [u0, v0],
                [u0, v1],
                [u1, v1],
                [u0, v0],
                [u1, v1],
                [u1, v0],
            ]) {
                vertex.uv = uv;
            }
        }
    }
    MeshData {
        vertices,
        texture: Some(crate::texgen::sky(512)),
    }
}

#[must_use]
pub fn foliage() -> MeshData {
    let mut mesh = sky();
    mesh.texture = Some(crate::texgen::turf(128));
    for vertex in &mut mesh.vertices {
        let p = Vec3::from(vertex.pos);
        vertex.normal = p.normalize_or_zero().to_array();
        let shape = 1.0 + (p.x * 17.0 + p.y * 7.0).sin() * 0.065 + (p.z * 13.0).cos() * 0.04;
        vertex.pos = (p * shape).to_array();
        vertex.uv = [p.x * 0.6 + 0.5, p.y * 0.6 + 0.5];
    }
    mesh
}

/// Vertical offset needed to stand a prop on the ground, in mesh units.
/// The generator centres its output on the origin, so half of every castle
/// tower is below the courtyard until this is applied.
#[must_use]
pub fn ground_offset(mesh: &MeshData) -> f32 {
    -mesh
        .vertices
        .iter()
        .map(|v| v.pos[1])
        .fold(f32::INFINITY, f32::min)
}

/// Longest-axis extent, so a prop can be scaled to a real size in metres.
#[must_use]
pub fn longest_extent(mesh: &MeshData) -> f32 {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for v in &mesh.vertices {
        let p = Vec3::from(v.pos);
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (hi - lo).max_element()
}

/// Load a generated prop.
///
/// Selects the largest part of the GLB, builds faceted normals, and applies
/// either planar UVs plus a texture or no texture at all. The latter lets the
/// instance colour supply the whole look so each player can have a different
/// car colour.
///
/// # Errors
///
/// Returns an error if the GLB cannot be decoded or contains no mesh parts.
pub fn prop(
    bytes: &[u8],
    texture: Option<TextureData>,
    tiles_per_unit: f32,
) -> Result<MeshData, String> {
    let parts = ember_engine::assets::load_glb(bytes)?;
    // The generator emits a single part; if that ever changes, the largest
    // one is the prop and the rest is debris.
    let mesh = parts
        .into_iter()
        .max_by_key(|p| p.mesh.vertices.len())
        .ok_or("glb had no parts")?
        .mesh;
    let mesh = face_normals(mesh);
    let mut mesh = if texture.is_some() {
        planar_uvs(mesh, tiles_per_unit)
    } else {
        mesh
    };
    mesh.texture = texture;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::MeshVertex;

    #[test]
    fn cars_have_distinct_metred_silhouettes_and_valid_lighting() {
        let mut lengths = Vec::new();
        for vehicle in 0..3 {
            let body = car_body(vehicle, false);
            let glass = car_body(vehicle, true);
            let length = longest_extent(&body);
            assert!((3.8..5.0).contains(&length));
            assert!(
                body.vertices.len() < 1800,
                "body exceeded the browser budget"
            );
            lengths.push(length);
            for mesh in [&body, &glass] {
                for vertex in &mesh.vertices {
                    let p = Vec3::from(vertex.pos);
                    let n = Vec3::from(vertex.normal);
                    assert!(p.is_finite() && n.is_finite());
                    assert!((n.length() - 1.0).abs() < 1e-4);
                    assert!(p.y >= 0.27 && p.y < 1.5);
                }
            }
        }
        assert!(lengths[1] < lengths[0] && lengths[0] < lengths[2]);
        let tyre = wheel(0.36, 0.29);
        assert!((ground_offset(&tyre) - 0.36).abs() < 1e-5);
    }

    /// A unit quad in the XZ plane, as two triangles, with deliberately wrong
    /// normals — exactly the shape `load_glb` hands back for a generated prop.
    fn flat_quad() -> MeshData {
        let p = [
            [-1.0f32, 0.0, -1.0],
            [1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, -1.0],
            [1.0, 0.0, 1.0],
            [-1.0, 0.0, 1.0],
        ];
        MeshData {
            vertices: p
                .iter()
                .map(|&pos| MeshVertex {
                    pos,
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                })
                .collect(),
            texture: None,
        }
    }

    /// A wall standing in the XY plane — its true normal is ±Z, so the +Y
    /// default the loader supplies is flatly wrong and lighting goes uniform.
    fn wall() -> MeshData {
        let p = [
            [-1.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [-1.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [-1.0, 2.0, 0.0],
        ];
        MeshData {
            vertices: p
                .iter()
                .map(|&pos| MeshVertex {
                    pos,
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                })
                .collect(),
            texture: None,
        }
    }

    #[test]
    fn face_normals_replace_the_loaders_default() {
        let m = face_normals(wall());
        for v in &m.vertices {
            let n = Vec3::from(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-4, "normal not unit: {n}");
            assert!(n.y.abs() < 1e-4, "wall normal still points up: {n}");
            assert!(n.z.abs() > 0.99, "wall normal should face Z: {n}");
        }
    }

    #[test]
    fn face_normals_are_unit_and_axis_aligned_for_a_flat_quad() {
        let m = face_normals(flat_quad());
        for v in &m.vertices {
            let n = Vec3::from(v.normal);
            assert!(n.y.abs() > 0.99, "floor normal should face Y: {n}");
        }
    }

    /// Degenerate triangles survive decimation. They must not produce a zero
    /// normal, which would render as an unlit black wedge.
    #[test]
    fn degenerate_triangles_get_a_usable_normal() {
        let m = MeshData {
            vertices: vec![
                MeshVertex {
                    pos: [0.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 0.0],
                    uv: [0.0, 0.0],
                },
                MeshVertex {
                    pos: [1.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 0.0],
                    uv: [0.0, 0.0],
                },
                MeshVertex {
                    pos: [2.0, 0.0, 0.0],
                    normal: [0.0, 0.0, 0.0],
                    uv: [0.0, 0.0],
                },
            ],
            texture: None,
        };
        let m = face_normals(m);
        for v in &m.vertices {
            let n = Vec3::from(v.normal);
            assert!(
                (n.length() - 1.0).abs() < 1e-4,
                "collapsed triangle gave {n}"
            );
        }
    }

    /// The loader leaves every UV at (0,0), so a texture samples one texel.
    /// Planar projection must actually spread them out.
    #[test]
    fn planar_uvs_span_the_surface() {
        let m = planar_uvs(face_normals(wall()), 2.0);
        let us: Vec<f32> = m.vertices.iter().map(|v| v.uv[0]).collect();
        let vs: Vec<f32> = m.vertices.iter().map(|v| v.uv[1]).collect();
        let span = |xs: &[f32]| {
            xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                - xs.iter().copied().fold(f32::INFINITY, f32::min)
        };
        assert!(span(&us) > 1.0, "u never varies: {us:?}");
        assert!(span(&vs) > 1.0, "v never varies: {vs:?}");
    }

    #[test]
    fn ground_offset_lifts_a_centred_prop_onto_the_floor() {
        let m = MeshData {
            vertices: vec![
                MeshVertex {
                    pos: [0.0, -1.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                },
                MeshVertex {
                    pos: [1.0, 1.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                },
                MeshVertex {
                    pos: [0.0, 0.5, 1.0],
                    normal: [0.0, 1.0, 0.0],
                    uv: [0.0, 0.0],
                },
            ],
            texture: None,
        };
        assert!((ground_offset(&m) - 1.0).abs() < 1e-6);
        assert!((longest_extent(&m) - 2.0).abs() < 1e-6);
    }

    /// The real thing: every generated prop must survive the pipeline with
    /// finite, unit normals. This is the test that would have caught the
    /// flat-lighting problem before it reached a screenshot.
    #[test]
    fn generated_props_load_and_light() {
        let props: [(&str, &[u8]); 4] = [
            (
                "car",
                include_bytes!("../../../assets/models/fire/fire-car.glb"),
            ),
            (
                "gatehouse",
                include_bytes!("../../../assets/models/fire/fire-gatehouse.glb"),
            ),
            (
                "tower",
                include_bytes!("../../../assets/models/fire/fire-tower.glb"),
            ),
            (
                "fountain",
                include_bytes!("../../../assets/models/fire/fire-fountain.glb"),
            ),
        ];
        for (name, bytes) in props {
            let m = prop(bytes, None, 1.0).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!m.vertices.is_empty(), "{name}: no vertices");
            assert_eq!(m.vertices.len() % 3, 0, "{name}: not a triangle list");
            let mut up_only = true;
            for v in &m.vertices {
                let n = Vec3::from(v.normal);
                assert!(n.is_finite(), "{name}: non-finite normal {n}");
                assert!(
                    (n.length() - 1.0).abs() < 1e-3,
                    "{name}: normal not unit: {n}"
                );
                if n.y < 0.999 {
                    up_only = false;
                }
            }
            assert!(
                !up_only,
                "{name}: every normal still points up — face normals did not apply"
            );
            assert!(longest_extent(&m) > 0.5, "{name}: suspiciously small");
        }
    }
}
