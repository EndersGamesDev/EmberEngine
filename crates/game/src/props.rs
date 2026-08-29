//! Static arena props (cover): loaded from assets/layouts/arena.json,
//! rendered as textured boxes. Purely visual for now — the server does not
//! yet know about them (collision lands with the sim-side collider work).

use ember_engine::glam::Vec3;
use ember_engine::{Frame, Instance};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PropDef {
    pub kind: String,
    pub x: f32,
    pub z: f32,
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default = "one")]
    pub scale: f32,
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct Layout {
    pub name: String,
    pub props: Vec<PropDef>,
}

#[derive(Debug, Deserialize)]
pub struct Layouts {
    pub layouts: Vec<Layout>,
}

/// Load layouts from assets/layouts/arena.json (workspace or cwd relative).
pub fn load_layouts() -> Option<Layouts> {
    let candidates = [
        format!(
            "{}/../../assets/layouts/arena.json",
            env!("CARGO_MANIFEST_DIR")
        ),
        "assets/layouts/arena.json".to_string(),
    ];
    for path in candidates {
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Layouts>(&text) {
                Ok(l) if l.layouts.is_empty() => {
                    tracing::warn!(path, "layout file has no layouts; ignoring");
                }
                Ok(l) => {
                    tracing::info!(path, layouts = l.layouts.len(), "arena layouts loaded");
                    return Some(l);
                }
                Err(e) => tracing::error!(path, error = %e, "arena layout parse failed"),
            }
        }
    }
    tracing::warn!("no arena layouts found; arena renders without props");
    None
}

/// Pick a layout: EMBER_LAYOUT=<name> (case-insensitive) or the first one.
pub fn pick(layouts: &Layouts) -> &Layout {
    if let Ok(want) = std::env::var("EMBER_LAYOUT") {
        if let Some(l) = layouts
            .layouts
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(&want))
        {
            return l;
        }
        tracing::warn!(want, "EMBER_LAYOUT not found; using first layout");
    }
    &layouts.layouts[0]
}

/// Push a layout's props. `mesh_stone` gets pillar/barricade (basalt),
/// `mesh_metal` gets crate/barrel (armor plate).
pub fn push_props(frame: &mut Frame, layout: &Layout, mesh_stone: u32, mesh_metal: u32) {
    for p in &layout.props {
        // (size, center-y, mesh, tint) per kind; unknown kinds are skipped.
        let (size, y, mesh, tint) = match p.kind.as_str() {
            "crate" => (
                Vec3::new(1.2, 1.2, 1.2),
                0.6,
                mesh_metal,
                Vec3::new(0.75, 0.72, 0.65),
            ),
            "barrel" => (
                Vec3::new(0.8, 1.1, 0.8),
                0.55,
                mesh_metal,
                Vec3::new(0.55, 0.58, 0.62),
            ),
            "pillar" => (
                Vec3::new(1.0, 3.0, 1.0),
                1.5,
                mesh_stone,
                Vec3::new(0.9, 0.9, 0.9),
            ),
            "barricade" => (
                Vec3::new(2.4, 1.1, 0.5),
                0.55,
                mesh_stone,
                Vec3::new(0.7, 0.7, 0.72),
            ),
            other => {
                tracing::debug!(kind = other, "unknown prop kind skipped");
                continue;
            }
        };
        frame.instances.push(
            Instance::new(Vec3::new(p.x, y * p.scale, p.z), size * p.scale, tint)
                .with_yaw(p.yaw_deg.to_radians())
                .with_mesh(mesh),
        );
    }
}
