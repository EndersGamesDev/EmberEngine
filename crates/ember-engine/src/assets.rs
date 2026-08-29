//! Asset loading: GLB (binary glTF) → engine meshes. This is the start of
//! the roadmap's "Blender becomes the level editor" step: models are
//! authored (or script-generated) in Blender, exported as .glb, and loaded
//! here into flat triangle lists for the instanced renderer.
//!
//! Conventions: author with +X forward and +Z up in Blender; the default
//! Y-up glTF export then lands with +X forward / +Y up, matching the
//! engine's yaw convention. Materials contribute only their base color.

use glam::{Mat4, Vec3};

use crate::renderer::{MeshData, MeshVertex, TextureData};

/// One named, single-colored piece of a model.
#[derive(Clone, Debug)]
pub struct GlbPart {
    pub name: String,
    pub mesh: MeshData,
    pub color: [f32; 3],
}

pub fn load_glb(bytes: &[u8]) -> Result<Vec<GlbPart>, String> {
    let (doc, buffers, images) =
        gltf::import_slice(bytes).map_err(|e| format!("glb parse: {e}"))?;
    let mut parts = Vec::new();
    let scene = doc
        .default_scene()
        .or_else(|| doc.scenes().next())
        .ok_or("glb has no scene")?;
    for node in scene.nodes() {
        collect(&node, Mat4::IDENTITY, &buffers, &images, &mut parts);
    }
    if parts.is_empty() {
        return Err("glb contains no mesh primitives".into());
    }
    Ok(parts)
}

/// Decode one embedded glTF image into engine RGBA8 (8-bit formats only).
fn image_to_rgba8(img: &gltf::image::Data) -> Option<TextureData> {
    use gltf::image::Format;
    let rgba8: Vec<u8> = match img.format {
        Format::R8G8B8A8 => img.pixels.clone(),
        Format::R8G8B8 => img
            .pixels
            .chunks_exact(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        Format::R8 => img.pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        _ => return None,
    };
    Some(TextureData {
        width: img.width,
        height: img.height,
        rgba8,
    })
}

fn collect(
    node: &gltf::Node,
    parent: Mat4,
    buffers: &[gltf::buffer::Data],
    images: &[gltf::image::Data],
    out: &mut Vec<GlbPart>,
) {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
    let world = parent * local;
    if let Some(mesh) = node.mesh() {
        let name = node.name().unwrap_or("part").to_string();
        for prim in mesh.primitives() {
            let reader = prim.reader(|b| buffers.get(b.index()).map(|d| &d.0[..]));
            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let positions: Vec<[f32; 3]> = positions.collect();
            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map(|n| n.collect())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_default();
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
                        uv: uvs.get(i as usize).copied().unwrap_or([0.0, 0.0]),
                    })
                })
                .collect();

            let pbr = prim.material().pbr_metallic_roughness();
            let color = pbr.base_color_factor();
            // Embedded base-color texture, when present and 8-bit.
            let texture = pbr
                .base_color_texture()
                .and_then(|info| images.get(info.texture().source().index()))
                .and_then(image_to_rgba8);
            out.push(GlbPart {
                name: name.clone(),
                mesh: MeshData { vertices, texture },
                color: [color[0], color[1], color[2]],
            });
        }
    }
    for child in node.children() {
        collect(&child, world, buffers, images, out);
    }
}
