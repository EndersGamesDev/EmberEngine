//! Turning the generated GLB props into meshes this renderer can light.
//!
//! `gen3d_mv.py` runs Hunyuan3D's *shape* pipeline, so what lands in
//! `assets/models/fire/` is geometry and nothing else: POSITION only, no
//! NORMAL, no TEXCOORD_0, no material. `ember_engine::assets::load_glb`
//! tolerates that by defaulting every normal to +Y and every UV to (0,0) —
//! which loads, but renders as a flat silhouette with no shading at all and
//! samples a single texel of any texture you attach.
//!
//! Both gaps are filled here rather than in the engine. `load_glb`'s defaults
//! are shared with the arena's character parts, which are POSITION-only too;
//! "improving" them there would silently restyle a live game.

use ember_engine::glam::Vec3;
use ember_engine::{MeshData, TextureData};

/// Recompute per-face normals from the triangles themselves.
///
/// The renderer's meshes are de-indexed flat triangle lists, so each run of
/// three vertices is one face and gets that face's geometric normal. Faceted
/// rather than smooth, which suits hard-surface castle stone and reads better
/// than smoothing across a decimated mesh's creases.
pub fn face_normals(mut mesh: MeshData) -> MeshData {
    for tri in mesh.vertices.chunks_mut(3) {
        if tri.len() < 3 {
            break;
        }
        let a = Vec3::from(tri[0].pos);
        let b = Vec3::from(tri[1].pos);
        let c = Vec3::from(tri[2].pos);
        // Degenerate triangles survive decimation; normalize_or_zero would
        // leave them black, so fall back to +Y for those.
        let n = (b - a).cross(c - a).normalize_or_zero();
        let n = if n == Vec3::ZERO { Vec3::Y } else { n };
        for v in tri {
            v.normal = n.to_array();
        }
    }
    mesh
}

/// Project UVs from world position, per face, on whichever axis the face
/// most nearly faces. A cheap triplanar: it cannot match an artist's unwrap,
/// but it turns a tiling stone texture into something that follows the
/// geometry instead of smearing one texel across the whole prop.
///
/// `tiles_per_unit` is in mesh units — the props are normalised to roughly
/// two units on their longest axis, so ~2.0 gives a few courses of stone
/// across a wall.
pub fn planar_uvs(mut mesh: MeshData, tiles_per_unit: f32) -> MeshData {
    for tri in mesh.vertices.chunks_mut(3) {
        if tri.len() < 3 {
            break;
        }
        let n = Vec3::from(tri[0].normal).abs();
        // Dominant axis picks the projection plane.
        let axis = if n.x >= n.y && n.x >= n.z {
            0
        } else if n.y >= n.z {
            1
        } else {
            2
        };
        for v in tri {
            let p = Vec3::from(v.pos);
            let (u, w) = match axis {
                0 => (p.z, p.y),
                1 => (p.x, p.z),
                _ => (p.x, p.y),
            };
            v.uv = [u * tiles_per_unit, w * tiles_per_unit];
        }
    }
    mesh
}

/// Vertical offset needed to stand a prop on the ground, in mesh units.
/// The generator centres its output on the origin, so half of every castle
/// tower is below the courtyard until this is applied.
pub fn ground_offset(mesh: &MeshData) -> f32 {
    -mesh
        .vertices
        .iter()
        .map(|v| v.pos[1])
        .fold(f32::INFINITY, f32::min)
}

/// Longest-axis extent, so a prop can be scaled to a real size in metres.
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

/// Load a generated prop: first part of the GLB, faceted normals, and either
/// planar UVs plus a texture, or no texture at all (the instance colour then
/// supplies the whole look, which is what the car wants so each player can be
/// a different colour).
pub fn prop(bytes: &[u8], texture: Option<TextureData>, tiles_per_unit: f32) -> Result<MeshData, String> {
    let parts = ember_engine::assets::load_glb(bytes)?;
    // The generator emits a single part; if that ever changes, the largest
    // one is the prop and the rest is debris.
    let mesh = parts
        .into_iter()
        .max_by_key(|p| p.mesh.vertices.len())
        .ok_or("glb had no parts")?
        .mesh;
    let mesh = face_normals(mesh);
    let mut mesh = if texture.is_some() { planar_uvs(mesh, tiles_per_unit) } else { mesh };
    mesh.texture = texture;
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ember_engine::MeshVertex;

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
                .map(|&pos| MeshVertex { pos, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] })
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
                .map(|&pos| MeshVertex { pos, normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] })
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
                MeshVertex { pos: [0.0, 0.0, 0.0], normal: [0.0, 0.0, 0.0], uv: [0.0, 0.0] },
                MeshVertex { pos: [1.0, 0.0, 0.0], normal: [0.0, 0.0, 0.0], uv: [0.0, 0.0] },
                MeshVertex { pos: [2.0, 0.0, 0.0], normal: [0.0, 0.0, 0.0], uv: [0.0, 0.0] },
            ],
            texture: None,
        };
        let m = face_normals(m);
        for v in &m.vertices {
            let n = Vec3::from(v.normal);
            assert!((n.length() - 1.0).abs() < 1e-4, "collapsed triangle gave {n}");
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
            xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
                - xs.iter().cloned().fold(f32::INFINITY, f32::min)
        };
        assert!(span(&us) > 1.0, "u never varies: {us:?}");
        assert!(span(&vs) > 1.0, "v never varies: {vs:?}");
    }

    #[test]
    fn ground_offset_lifts_a_centred_prop_onto_the_floor() {
        let m = MeshData {
            vertices: vec![
                MeshVertex { pos: [0.0, -1.0, 0.0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
                MeshVertex { pos: [1.0, 1.0, 0.0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
                MeshVertex { pos: [0.0, 0.5, 1.0], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
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
            ("car", include_bytes!("../../../assets/models/fire/fire-car.glb")),
            ("gatehouse", include_bytes!("../../../assets/models/fire/fire-gatehouse.glb")),
            ("tower", include_bytes!("../../../assets/models/fire/fire-tower.glb")),
            ("fountain", include_bytes!("../../../assets/models/fire/fire-fountain.glb")),
        ];
        for (name, bytes) in props {
            let m = prop(bytes, None, 1.0).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!m.vertices.is_empty(), "{name}: no vertices");
            assert_eq!(m.vertices.len() % 3, 0, "{name}: not a triangle list");
            let mut up_only = true;
            for v in &m.vertices {
                let n = Vec3::from(v.normal);
                assert!(n.is_finite(), "{name}: non-finite normal {n}");
                assert!((n.length() - 1.0).abs() < 1e-3, "{name}: normal not unit: {n}");
                if n.y < 0.999 {
                    up_only = false;
                }
            }
            assert!(!up_only, "{name}: every normal still points up — face normals did not apply");
            assert!(longest_extent(&m) > 0.5, "{name}: suspiciously small");
        }
    }
}
