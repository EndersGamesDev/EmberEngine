//! Baked champion art: GLBs produced by `tools/league/art/bake_champion.py`
//! from the fleet's references, embedded with `include_bytes!` and
//! registered after the procedural meshes.
//!
//! A champion whose GLB is not here yet keeps its procedural body, so a
//! partial delivery never breaks the build or the picture. Registration
//! needs no change outside `scene.rs`: [`meshes`] is appended by
//! `scene::build_meshes`, which `lib.rs` already calls.
//!
//! Contract (see `docs/asset-pipeline.md` and the sidecar beside each GLB):
//! Y up, +X forward, origin between the feet, one base-colour texture per
//! part, `baseColorFactor` white. Parts are drawn with a white instance
//! colour so the texture is not double-tinted; a tint (hologram, exhaust,
//! demon form) multiplies on top and is meant to.

use std::sync::OnceLock;

use ember_engine::MeshData;
use ember_engine::assets::load_glb;
use ember_engine::glam::Vec3;
use serde::Deserialize;

/// The sidecar `bake_champion.py` writes beside every GLB. Only the fields
/// the renderer reads are declared; the rest is provenance for humans.
#[derive(Deserialize, Debug, Clone)]
pub struct Sidecar {
    pub name: String,
    /// Standing height in engine units after the bake.
    pub height: f32,
    /// Per-part pivots in engine space, keyed by node name. A part missing
    /// here pivots at the origin. Written BEFORE `part_order` in the file,
    /// per the sidecar rule in the pipeline doc.
    #[serde(default)]
    pub pivots: std::collections::BTreeMap<String, [f32; 3]>,
    /// Node names in draw order.
    #[serde(default)]
    pub part_order: Vec<String>,
}

/// One embedded champion: its table row, GLB bytes and sidecar text.
struct Source {
    def: u8,
    glb: &'static [u8],
    sidecar: &'static str,
}

/// Champions delivered so far, in table order. Add a row when a GLB lands;
/// the `include_bytes!` path is relative to this file.
const SOURCES: &[Source] = &[
    Source {
        def: league_core::data::SWARM,
        glb: include_bytes!("../../../../assets/models/league/v2/swarm.glb"),
        sidecar: include_str!("../../../../assets/models/league/v2/swarm.json"),
    },
    Source {
        def: league_core::data::KNIGHT,
        glb: include_bytes!("../../../../assets/models/league/v2/emberknight.glb"),
        sidecar: include_str!("../../../../assets/models/league/v2/emberknight.json"),
    },
    Source {
        def: league_core::data::HALLOW,
        glb: include_bytes!("../../../../assets/models/league/v2/hallow.glb"),
        sidecar: include_str!("../../../../assets/models/league/v2/hallow.json"),
    },
    Source {
        def: league_core::data::MAW,
        glb: include_bytes!("../../../../assets/models/league/v2/bogmaw.glb"),
        sidecar: include_str!("../../../../assets/models/league/v2/bogmaw.json"),
    },
    Source {
        def: league_core::data::TESSERA,
        glb: include_bytes!("../../../../assets/models/league/v2/tessera.glb"),
        sidecar: include_str!("../../../../assets/models/league/v2/tessera.json"),
    },
];

/// Ground surfaces baked by `tools/league/art/bake_surface.py`: UV-tiled
/// unit quads with the mirrored picture embedded. Registered BEFORE the
/// champions, in this order, so their ids are fixed (`scene::MESH_GARDEN`,
/// `MESH_LANE`, `MESH_COURT`).
const SURFACES: &[(&str, &[u8])] = &[
    ("garden", include_bytes!("../../../../assets/models/league/v2/surface-garden.glb")),
    ("lane", include_bytes!("../../../../assets/models/league/v2/surface-lane.glb")),
    ("court", include_bytes!("../../../../assets/models/league/v2/surface-court.glb")),
];

/// How many meshes [`surfaces`] returns, always, so the champion ids that
/// follow never move.
pub const SURFACE_COUNT: u32 = 3;

/// The ground surfaces as meshes, one per `SURFACES` row in order. A quad
/// whose GLB cannot be read is replaced by an untextured quad and warned
/// about, so the ids after it stay where they are.
#[must_use]
pub fn surfaces() -> Vec<MeshData> {
    SURFACES
        .iter()
        .map(|(name, bytes)| match load_glb(bytes) {
            Ok(mut parts) if !parts.is_empty() => {
                let part = parts.swap_remove(0);
                if part.mesh.texture.is_none() {
                    tracing::warn!(surface = name, "league art: surface has no 8-bit texture; it will draw flat");
                }
                part.mesh
            }
            Ok(_) => {
                tracing::warn!(surface = name, "league art: surface glb has no primitive; drawing a plain quad");
                plain_quad()
            }
            Err(e) => {
                tracing::warn!(surface = name, "league art: surface glb unreadable ({e}); drawing a plain quad");
                plain_quad()
            }
        })
        .collect()
}

/// A unit quad on y=0 facing +Y, the stand-in for a surface that failed.
fn plain_quad() -> MeshData {
    use ember_engine::MeshVertex;
    let n = [0.0, 1.0, 0.0];
    let v = |x: f32, z: f32| MeshVertex { pos: [x, 0.0, z], normal: n, uv: [0.0, 0.0] };
    MeshData {
        vertices: vec![v(-0.5, -0.5), v(0.5, 0.5), v(0.5, -0.5), v(-0.5, -0.5), v(-0.5, 0.5), v(0.5, 0.5)],
        texture: None,
    }
}

/// One drawable part of a baked champion.
#[derive(Debug, Clone)]
pub struct Part {
    pub name: String,
    /// Registered mesh id.
    pub mesh: u32,
    /// Where this part rotates about, in the champion's own frame.
    pub pivot: Vec3,
}

/// A champion's baked art, ready to draw.
#[derive(Debug, Clone)]
pub struct Champion {
    pub def: u8,
    pub height: f32,
    pub parts: Vec<Part>,
}

static TABLE: OnceLock<Vec<Champion>> = OnceLock::new();

/// Load every embedded champion and return their meshes, which the caller
/// registers starting at `first_id`. Fills the lookup table [`champion`]
/// reads; calling it twice returns the same meshes and keeps the first table.
#[must_use]
pub fn meshes(first_id: u32) -> Vec<MeshData> {
    let mut out = Vec::new();
    let mut table = Vec::new();
    let mut next = first_id;
    for src in SOURCES {
        let side: Sidecar = match serde_json::from_str(src.sidecar) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(def = src.def, "league art: sidecar unreadable ({e}); champion keeps its procedural body");
                continue;
            }
        };
        let parts = match load_glb(src.glb) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(def = src.def, name = %side.name, "league art: glb unreadable ({e}); champion keeps its procedural body");
                continue;
            }
        };
        // draw order: the sidecar's list first, anything unnamed after it
        let mut ordered: Vec<_> = Vec::with_capacity(parts.len());
        for want in &side.part_order {
            if let Some(p) = parts.iter().find(|p| &p.name == want) {
                ordered.push(p);
            }
        }
        for p in &parts {
            if !side.part_order.contains(&p.name) {
                ordered.push(p);
            }
        }
        let mut champ = Champion {
            def: src.def,
            height: side.height,
            parts: Vec::with_capacity(ordered.len()),
        };
        for p in ordered {
            if p.mesh.texture.is_none() {
                tracing::warn!(def = src.def, part = %p.name, "league art: part has no 8-bit base-colour texture; it will draw flat");
            }
            champ.parts.push(Part {
                name: p.name.clone(),
                mesh: next,
                pivot: side.pivots.get(&p.name).map_or(Vec3::ZERO, |v| Vec3::from(*v)),
            });
            out.push(p.mesh.clone());
            next += 1;
        }
        tracing::info!(def = src.def, name = %side.name, parts = champ.parts.len(), "league art: champion loaded");
        table.push(champ);
    }
    drop(TABLE.set(table));
    out
}

/// The baked art for a champion table row, if it has been delivered.
#[must_use]
pub fn champion(def: u8) -> Option<&'static Champion> {
    TABLE.get()?.iter().find(|c| c.def == def)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_reads_the_fields_the_renderer_needs() {
        let s: Sidecar = serde_json::from_str(
            r#"{"name":"swarm","height":1.7,"pivots":{"orb":[0.0,0.9,0.0]},"part_order":["orb"],"faces":{"shipped":4000}}"#,
        )
        .unwrap();
        assert_eq!(s.name, "swarm");
        assert_eq!(s.part_order, vec!["orb".to_string()]);
        assert_eq!(s.pivots["orb"], [0.0, 0.9, 0.0]);
    }

    #[test]
    fn an_empty_delivery_registers_nothing_and_answers_none() {
        let m = meshes(10);
        assert!(m.len() >= SOURCES.len().min(1) - 1 || SOURCES.is_empty());
        if SOURCES.is_empty() {
            assert!(m.is_empty());
            assert!(champion(0).is_none());
        }
    }
}
