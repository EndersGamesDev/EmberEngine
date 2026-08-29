//! Asset loading: GLB (binary glTF) → engine meshes. This is the start of
//! the roadmap's "Blender becomes the level editor" step: models are
//! authored (or script-generated) in Blender, exported as .glb, and loaded
//! here into flat triangle lists for the instanced renderer.
//!
//! Conventions: author with +X forward and +Z up in Blender; the default
//! Y-up glTF export then lands with +X forward / +Y up, matching the
//! engine's yaw convention. Materials contribute only their base color.

use glam::{Mat4, Vec3};

use crate::renderer::{MeshData, MeshVertex};

/// One named, single-colored piece of a model.
#[derive(Clone, Debug)]
pub struct GlbPart {
    pub name: String,
    pub mesh: MeshData,
    pub color: [f32; 3],
}

pub fn load_glb(bytes: &[u8]) -> Result<Vec<GlbPart>, String> {
    let (doc, buffers, _images) =
        gltf::import_slice(bytes).map_err(|e| format!("glb parse: {e}"))?;
    let mut parts = Vec::new();
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or("glb has no scene")?;
    for node in scene.nodes() {
        collect(&node, Mat4::IDENTITY, &buffers, &mut parts);
    }
    if parts.is_empty() {
        return Err("glb contains no mesh primitives".into());
    }
    Ok(parts)
}

fn collect(
    node: &gltf::Node,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    out: &mut Vec<GlbPart>,
) {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent * local;
    if let Some(mesh) = node.mesh() {
        let name = node.name().unwrap_or("part").to_string();
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let Some(positions) = reader.read_positions() else { continue };
            let positions: Vec<[f32; 3]> = positions.collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            // De-index into a flat triangle list (the renderer is unindexed).
            let indices: Vec<u32> = reader
                .read_indices()
                .map(|i| i.into_u32().collect())
                .unwrap_or_else(|| (0..positions.len() as u32).collect());

            let normal_mat = world.inverse().transpose();
            let vertices: Vec<MeshVertex> = indices
                .iter()
                .filter_map(|&i| {
                    let p = *positions.get(i as usize)?;
                    let n = *normals.get(i as usize).unwrap_or(&[0.0, 1.0, 0.0]);
                    Some(MeshVertex {
                        pos: world.transform_point3(Vec3::from(p)).to_array(),
                        normal: normal_mat
                            .transform_vector3(Vec3::from(n))
                            .normalize_or_zero()
                            .to_array(),
                    })
                })
                .collect();

            let color = prim
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            out.push(GlbPart {
                name: name.clone(),
                mesh: MeshData { vertices },
                color: [color[0], color[1], color[2]],
            });
        }
    }
    for child in node.children() {
        collect(&child, world, buffers, out);
    }
}
