//! The world as ember draws it: procedural meshes registered in a fixed
//! order, and a frame builder that reads snapshot units and transient
//! effects. The scene pass has one base colour per mesh and no text —
//! identity is colour and silhouette, HUD text is the page's job.
//!
//! Mesh ids are allocated here in order and mirrored by `Meshes` below:
//! 0 is the engine cube, then plane, frustum, octahedron, disc. Adding a
//! mesh at the end is safe; inserting in the middle shifts everything.

use ember_engine::{glam::Quat, Camera, Fog, Frame, Instance, MeshData, MeshVertex};

use crate::world::{FxLite, UnitLite};

pub const MESH_PLANE: u32 = 1;
pub const MESH_FRUSTUM: u32 = 2;
pub const MESH_OCTA: u32 = 3;
pub const MESH_DISC: u32 = 4;

/// The camera sits high and back so the lane reads as a lane. Facing is
/// the sim's yaw convention: forward = (cos f, sin f) in (x, z).
#[must_use]
pub fn camera_for(fx_pos: (f32, f32)) -> Camera {
    let (x, z) = fx_pos;
    Camera {
        eye: (x - 8.0, 19.0, z + 10.5).into(),
        target: (x, 1.0, z).into(),
        fov_y_deg: 46.0,
    }
}

/// Team tint: blue side, red side. The page's scoreboard uses the same
/// values divided by 255 so the two agree.
#[must_use]
pub fn team_colour(team: u8) -> [f32; 3] {
    match team {
        0 => [0.42, 0.62, 1.0],
        1 => [1.0, 0.42, 0.38],
        _ => [0.75, 0.72, 0.55], // courts belong to nobody
    }
}

const SEG: usize = 16;

fn push_tri(v: &mut Vec<MeshVertex>, a: [f32; 3], b: [f32; 3], c: [f32; 3], n: [f32; 3]) {
    for p in [a, b, c] {
        v.push(MeshVertex {
            pos: p,
            normal: n,
            uv: [0.0, 0.0],
        });
    }
}

fn around(cx: f32, cz: f32, r: f32, y: f32, i: usize) -> [f32; 3] {
    let t = std::f32::consts::TAU * (i as f32) / (SEG as f32);
    [cx + t.cos() * r, y, cz + t.sin() * r]
}

/// The whole mesh set, in registration order.
#[must_use]
pub fn build_meshes() -> Vec<MeshData> {
    vec![plane_mesh(), frustum_mesh(), octa_mesh(), disc_mesh()]
}

fn plane_mesh() -> MeshData {
    // a unit quad on y=0, facing +Y, centred on origin
    let n = [0.0, 1.0, 0.0];
    let a = [-0.5, 0.0, -0.5];
    let b = [0.5, 0.0, -0.5];
    let c = [0.5, 0.0, 0.5];
    let d = [-0.5, 0.0, 0.5];
    let mut vertices = Vec::with_capacity(6);
    push_tri(&mut vertices, a, c, b, n);
    push_tri(&mut vertices, a, d, c, n);
    MeshData { vertices, texture: None }
}

/// A capped cone/cylinder centred on the origin, radius 1 at its base and
/// top, height 2 — scaled per instance like the cube is.
fn frustum_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(SEG * 12);
    for i in 0..SEG {
        let b0 = around(0.0, 0.0, 1.0, -1.0, i);
        let b1 = around(0.0, 0.0, 1.0, -1.0, i + 1);
        let t0 = around(0.0, 0.0, 1.0, 1.0, i);
        let t1 = around(0.0, 0.0, 1.0, 1.0, i + 1);
        let mid = [
            (b0[0] + b1[0] + t0[0] + t1[0]) / 4.0,
            0.0,
            (b0[2] + b1[2] + t0[2] + t1[2]) / 4.0,
        ];
        let n = [mid[0], 0.0, mid[2]];
        let n = norm_or_up(n);
        push_tri(&mut vertices, b0, b1, t1, n);
        push_tri(&mut vertices, b0, t1, t0, n);
        // caps
        push_tri(&mut vertices, [0.0, 1.0, 0.0], t0, t1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, [0.0, -1.0, 0.0], b1, b0, [0.0, -1.0, 0.0]);
    }
    MeshData { vertices, texture: None }
}

/// A diamond: two square-based pyramids, point up and down.
fn octa_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(24);
    let pts = [
        [-1.0, 0.0, 0.0],
        [0.0, 0.0, -1.0],
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let up = [0.0, 1.2, 0.0];
    let dn = [0.0, -1.2, 0.0];
    for i in 0..4 {
        let a = pts[i];
        let b = pts[(i + 1) % 4];
        let n1 = norm_or_up([up[0] + (a[0] + b[0]) * 0.5, 0.6, up[2] + (a[2] + b[2]) * 0.5]);
        push_tri(&mut vertices, a, b, up, n1);
        let n2 = [n1[0], -0.6, n1[2]];
        let n2 = norm_or_up(n2);
        push_tri(&mut vertices, b, a, dn, n2);
    }
    MeshData { vertices, texture: None }
}

/// A flat ring-ish disc: a thin cylinder at y=0 for team rings, fountains,
/// traps and stasis fields.
fn disc_mesh() -> MeshData {
    let mut vertices = Vec::with_capacity(SEG * 6 + 4);
    let h = 0.04;
    for i in 0..SEG {
        let a0 = around(0.0, 0.0, 1.0, -h, i);
        let a1 = around(0.0, 0.0, 1.0, -h, i + 1);
        let b0 = around(0.0, 0.0, 1.0, h, i);
        let b1 = around(0.0, 0.0, 1.0, h, i + 1);
        let mid = [(a0[0] + a1[0]) / 2.0, 0.0, (a0[2] + a1[2]) / 2.0];
        let n = norm_or_up(mid);
        push_tri(&mut vertices, a0, a1, b1, n);
        push_tri(&mut vertices, a0, b1, b0, n);
    }
    let c0 = [0.0, h, 0.0];
    let c1 = [0.0, -h, 0.0];
    for i in 0..SEG {
        let b0 = around(0.0, 0.0, 1.0, h, i);
        let b1 = around(0.0, 0.0, 1.0, h, i + 1);
        let a0 = around(0.0, 0.0, 1.0, -h, i);
        let a1 = around(0.0, 0.0, 1.0, -h, i + 1);
        push_tri(&mut vertices, c0, b0, b1, [0.0, 1.0, 0.0]);
        push_tri(&mut vertices, c1, a1, a0, [0.0, -1.0, 0.0]);
    }
    MeshData { vertices, texture: None }
}

fn norm_or_up(n: [f32; 3]) -> [f32; 3] {
    let d = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if d < 1e-6 {
        [0.0, 1.0, 0.0]
    } else {
        [n[0] / d, n[1] / d, n[2] / d]
    }
}

/// Instance with array colour: the engine takes Vec3, the renderer files
/// read better with arrays.
#[must_use]
fn ins(position: (f32, f32, f32), scale: (f32, f32, f32), color: [f32; 3]) -> Instance {
    Instance::new(
        position.into(),
        scale.into(),
        ember_engine::glam::Vec3::new(color[0], color[1], color[2]),
    )
}

/// Static ground geometry: dark field, lighter lane, the two fountain
/// pads. Drawn first, under everything.
fn push_ground(frame: &mut Frame) {
    frame.instances.push(
        ins((0.0, -0.05, 0.0).into(), (160.0, 1.0, 92.0).into(), [0.06, 0.10, 0.06])
            .with_mesh(MESH_PLANE),
    );
    // the lane corridor
    frame.instances.push(
        ins(
            (0.0, -0.02, 0.0).into(),
            (140.0, 1.0, 15.0).into(),
            [0.14, 0.15, 0.09],
        )
        .with_mesh(MESH_PLANE),
    );
    // walls at the field edge, so the dark field ends somewhere
    for z in [-40.0, 40.0] {
        frame.instances.push(
            ins((0.0, 0.5, z).into(), (160.0, 1.2, 1.6).into(), [0.10, 0.09, 0.16])
                .with_mesh(0),
        );
    }
    // fountain pads
    for x in [-62.0, 62.0] {
        frame.instances.push(
            ins((x, 0.0, 0.0).into(), (7.5, 1.0, 7.5).into(), [0.16, 0.20, 0.30])
                .with_mesh(MESH_DISC),
        );
    }
}

fn push_core(frame: &mut Frame, u: &UnitLite, t: f32, dead: bool) {
    let col = if dead { [0.18, 0.16, 0.16] } else { team_colour(u.t) };
    let bob = if dead { 0.0 } else { (t * 1.4).sin() * 0.25 };
    frame.instances.push(
        ins((u.x, 2.4 + bob, u.z).into(), (1.8, 2.2, 1.8).into(), col)
            .with_mesh(MESH_OCTA),
    );
    frame.instances.push(
        ins((u.x, 0.35, u.z).into(), (2.6, 0.35, 2.6).into(), [0.30, 0.30, 0.42])
            .with_mesh(MESH_FRUSTUM),
    );
    if !dead {
        frame.instances.push(
            ins((u.x, 0.06, u.z).into(), (5.5, 1.0, 5.5).into(), col)
                .with_mesh(MESH_DISC),
        );
    }
}

fn push_court(frame: &mut Frame, u: &UnitLite, t: f32) {
    if u.dead {
        // a taken court: a broken stump and its ring, waiting to respawn
        frame.instances.push(
            ins((u.x, 0.4, u.z).into(), (1.1, 0.4, 1.1).into(), [0.22, 0.20, 0.20])
                .with_mesh(MESH_FRUSTUM),
        );
        return;
    }
    let pulse = 0.85 + 0.15 * (t * 2.0).sin();
    frame.instances.push(
        ins((u.x, 2.6, u.z).into(), (1.15, 2.6, 1.15).into(), [0.55 * pulse, 0.62, 0.85 * pulse])
            .with_mesh(MESH_FRUSTUM),
    );
    frame.instances.push(
        ins((u.x, 5.5, u.z).into(), (0.8, 0.8, 0.8).into(), [0.95, 0.9, 0.6]).with_mesh(MESH_OCTA),
    );
    frame.instances.push(
        ins((u.x, 0.06, u.z).into(), (3.2, 1.0, 3.2).into(), [0.5, 0.55, 0.35])
            .with_mesh(MESH_DISC),
    );
}

fn push_minion(frame: &mut Frame, u: &UnitLite) {
    let col = team_colour(u.t);
    if u.k == 2 {
        // caster: a cone with a head-knot
        frame.instances.push(
            ins((u.x, 0.75, u.z).into(), (0.5, 0.75, 0.5).into(), col)
                .with_yaw(u.fa)
                .with_mesh(MESH_FRUSTUM),
        );
        frame.instances.push(
            ins((u.x, 1.7, u.z).into(), (0.22, 0.22, 0.22).into(), [1.0, 0.9, 0.5])
                .with_mesh(0),
        );
    } else {
        frame.instances.push(
            ins((u.x, 0.62, u.z).into(), (0.42, 0.62, 0.36).into(), col)
                .with_yaw(u.fa)
                .with_mesh(0),
        );
    }
}

fn champion_colour(u: &UnitLite) -> [f32; 3] {
    // per-champion silhouettes come from data::CHAMPS colours; the unit
    // wire carries `def` and the client re-reads the table (world.rs
    // bakes the colour into the lite unit so this file stays data-free).
    u.colour
}

fn push_champion(frame: &mut Frame, u: &UnitLite, t: f32) {
    let col = champion_colour(u);
    let team = team_colour(u.t);
    let bob = (t * 6.0 + u.id as f32).sin() * 0.05;
    // holograms flicker: same body, cyan cast, scale wobble
    let (col, wob) = if u.k == 3 {
        ([0.45 * col[0] + 0.25, 0.8, col[2] * 0.5 + 0.5], 1.0 + 0.04 * (t * 17.0).sin())
    } else {
        (col, 1.0)
    };
    // the ground ring says the team even when colours run together
    frame.instances.push(
        ins((u.x, 0.05, u.z).into(), (1.15 * wob, 1.0, 1.15 * wob).into(), team)
            .with_mesh(MESH_DISC),
    );
    frame.instances.push(
        ins((u.x, 0.85 + bob, u.z).into(), (0.62 * wob, 0.85 * wob, 0.5 * wob).into(), col)
            .with_yaw(u.fa)
            .with_mesh(0),
    );
    frame.instances.push(
        ins((u.x, 1.62 + bob, u.z).into(), (0.36 * wob, 0.36 * wob, 0.36 * wob).into(), [col[0] * 1.25, col[1] * 1.2, col[2] * 1.2])
            .with_yaw(u.fa)
            .with_mesh(MESH_OCTA),
    );
    // a weapon mark, rotated with facing, for the melee kits' read
    let (fx_, fz_) = (u.fa.cos(), u.fa.sin());
    frame.instances.push(
        ins((u.x + fx_ * 0.75, 1.0 + bob, u.z + fz_ * 0.75).into(), (0.12, 0.12, 0.62).into(), [col[0], col[1], col[2]])
            .with_yaw(u.fa)
            .with_mesh(0),
    );
}

fn push_hp_bar(frame: &mut Frame, u: &UnitLite) {
    if u.dead || u.mh <= 0.0 {
        return;
    }
    let frac = (u.hp / u.mh as f32).clamp(0.0, 1.0);
    if frac > 0.999 && (u.k == 4 || u.k == 5 || u.k == 6 || u.k == 7) {
        return; // objectives show their bar in the HUD, not in the world
    }
    let w = if u.k == 0 || u.k == 3 { 1.3 } else { 0.7 };
    let y = match u.k {
        0 | 3 => 2.5,
        4 | 5 => 6.6,
        6 | 7 => 4.4,
        _ => 1.7,
    };
    frame.instances.push(
        ins((u.x, y, u.z).into(), (w + 0.08, 0.10, 0.10).into(), [0.05, 0.05, 0.05])
            .with_mesh(0),
    );
    let col = if u.t == 2 {
        [0.8, 0.75, 0.4]
    } else {
        [0.2 + 0.6 * (1.0 - frac), 0.75 * frac + 0.1, 0.15]
    };
    frame.instances.push(
        ins(
            (u.x - w * (1.0 - frac) / 2.0, y, u.z).into(),
            (w * frac, 0.12, 0.12).into(),
            col,
        )
        .with_mesh(0),
    );
}

/// Transient effects, aged by the caller. `age` is 0..1 over the effect's
/// short life; `dur` scales expansion.
fn push_fx(frame: &mut Frame, fx: &FxLite, age: f32) {
    let fade = |c: f32| c * (1.0 - age * 0.6);
    match fx.k {
        0 => {
            // auto-attack mark at the struck unit
            frame.instances.push(
                ins((fx.x, 1.2, fx.z).into(), (0.4 * (1.0 - age), 0.4, 0.4 * (1.0 - age)).into(), [1.0, 0.9, 0.6])
                    .with_mesh(MESH_OCTA),
            );
        }
        1 => {
            // beam between two points
            let (dx, dz) = (fx.x2 - fx.x, fx.z2 - fx.z);
            let len = (dx * dx + dz * dz).sqrt().max(0.01);
            let yaw = dz.atan2(dx);
            frame.instances.push(
                ins(
                    ((fx.x + fx.x2) / 2.0, 1.05, (fx.z + fx.z2) / 2.0).into(),
                    (len / 2.0, 0.10, 0.10).into(),
                    [1.0, fade(0.5), fade(0.2)],
                )
                .with_yaw(yaw)
                .with_mesh(0),
            );
        }
        2 | 3 | 5 => {
            // expanding ring (explosion, zone spawn, death poof)
            let r = fx.v * (0.4 + age * 1.1);
            frame.instances.push(
                ins((fx.x, 0.12, fx.z).into(), (r, 1.0, r).into(), [fade(1.0), fade(0.6), fade(0.15)])
                    .with_mesh(MESH_DISC),
            );
        }
        4 => {
            // trap planted
            frame.instances.push(
                ins((fx.x, 0.09, fx.z).into(), (fx.v, 1.0, fx.v).into(), [fade(0.6), fade(0.5), 1.0])
                    .with_mesh(MESH_DISC),
            );
        }
        6 => {
            // level-up column
            frame.instances.push(
                ins((fx.x, 1.6 + age, fx.z).into(), (0.25, 1.6, 0.25).into(), [fade(0.9), fade(0.8), 1.0])
                    .with_mesh(MESH_FRUSTUM),
            );
        }
        7 => {
            // coin blink
            frame.instances.push(
                ins((fx.x, 2.2 + age * 0.8, fx.z).into(), (0.22, 0.22, 0.22).into(), [1.0, fade(0.85), fade(0.3)])
                    .with_mesh(0),
            );
        }
        8 => {
            // teleport streak between points
            let (dx, dz) = (fx.x2 - fx.x, fx.z2 - fx.z);
            let len = (dx * dx + dz * dz).sqrt().max(0.01);
            let yaw = dz.atan2(dx);
            frame.instances.push(
                ins(
                    ((fx.x + fx.x2) / 2.0, 0.9, (fx.z + fx.z2) / 2.0).into(),
                    (len / 2.0, 0.06, 0.35).into(),
                    [fade(0.5), fade(0.8), 1.0],
                )
                .with_yaw(yaw)
                .with_mesh(0),
            );
        }
        9 | 12 => {
            // heal / shield flash
            let col = if fx.k == 9 { [fade(0.5), 1.0, fade(0.5)] } else { [fade(0.6), fade(0.8), 1.0] };
            frame.instances.push(
                ins((fx.x, 1.1, fx.z).into(), (0.9 + age * 0.6, 1.4, 0.9 + age * 0.6).into(), col)
                    .with_mesh(MESH_FRUSTUM),
            );
        }
        10 => {
            // hook line
            let (dx, dz) = (fx.x2 - fx.x, fx.z2 - fx.z);
            let len = (dx * dx + dz * dz).sqrt().max(0.01);
            let yaw = dz.atan2(dx);
            frame.instances.push(
                ins(
                    ((fx.x + fx.x2) / 2.0, 0.8, (fx.z + fx.z2) / 2.0).into(),
                    (len / 2.0, 0.07, 0.07).into(),
                    [0.4, fade(0.8), 0.3],
                )
                .with_yaw(yaw)
                .with_mesh(0),
            );
        }
        11 => {
            // exhaust mark
            frame.instances.push(
                ins((fx.x, 1.9, fx.z).into(), (0.5 * (1.0 - age), 0.5, 0.5 * (1.0 - age)).into(), [fade(0.4), fade(0.4), fade(0.9)])
                    .with_mesh(MESH_OCTA),
            );
        }
        _ => {}
    }
}

/// The live zone bodies — tornadoes, traps and stasis fields that are in
/// the world right now, not just their spawn flash. `zones` carries them:
/// (kind 0 tornado / 1 trap / 2 stasis / 3 shroud, x, z, r).
#[must_use]
pub fn scene(
    units: &[UnitLite],
    zones: &[(u8, f32, f32, f32, f32)],
    fx: &[FxLite],
    time: f32,
    camera: Camera,
) -> Frame {
    let mut frame = Frame {
        camera,
        instances: Vec::with_capacity(units.len() * 6 + zones.len() + 64),
        fog: Fog::default(),
    };
    push_ground(&mut frame);
    for f in fx {
        let age = if f.life > 0.0 {
            ((f.life - f.left) / f.life).clamp(0.0, 1.0)
        } else {
            0.0
        };
        push_fx(&mut frame, f, age);
    }
    for (zk, x, z, r, spin) in zones {
        match zk {
            0 => {
                frame.instances.push(
                    ins((*x, 1.4, *z).into(), (*r * 0.5, 1.4, *r * 0.5).into(), [1.0, 0.45, 0.12])
                        .with_mesh(MESH_FRUSTUM)
                        .with_rot(Quat::from_rotation_y(*spin)),
                );
                frame.instances.push(
                    ins((*x, 0.07, *z).into(), (*r, 1.0, *r).into(), [0.8, 0.3, 0.08])
                        .with_mesh(MESH_DISC),
                );
            }
            1 => {
                frame.instances.push(
                    ins((*x, 0.08, *z).into(), (*r, 1.0, *r).into(), [0.5, 0.4, 1.0])
                        .with_mesh(MESH_DISC),
                );
            }
            2 => {
                frame.instances.push(
                    ins((*x, 0.9, *z).into(), (*r, 0.9, *r).into(), [0.35, 0.3, 0.85])
                        .with_mesh(MESH_FRUSTUM)
                        .with_rot(Quat::from_rotation_y(*spin)),
                );
            }
            _ => {
                frame.instances.push(
                    ins((*x, 0.7, *z).into(), (*r * 0.8, 0.7, *r * 0.8).into(), [0.25, 0.55, 0.2])
                        .with_mesh(MESH_FRUSTUM),
                );
            }
        }
    }
    for u in units {
        match u.k {
            6 | 7 => push_core(&mut frame, u, time, u.dead),
            4 | 5 => push_court(&mut frame, u, time),
            1 | 2 => push_minion(&mut frame, u),
            0 | 3 => {
                if !u.dead {
                    push_champion(&mut frame, u, time);
                }
            }
            _ => {}
        }
    }
    for u in units {
        push_hp_bar(&mut frame, u);
    }
    frame
}

/// Project a world point to NDC (-1..1, +y up) through the same camera
/// the frame was drawn with. Clicking an enemy is a projection contest:
/// the unit whose chest lands nearest the cursor is the one you meant.
#[must_use]
pub fn project(camera: &Camera, aspect: f32, x: f32, y: f32, z: f32) -> (f32, f32) {
    let vp = camera.view_proj(aspect);
    let clip = vp * ember_engine::glam::Vec4::new(x, y, z, 1.0);
    let w = clip.w.max(1e-6);
    (clip.x / w, clip.y / w)
}

/// Where the cursor meets the ground plane y=0. `None` when the ray does
/// not point at the floor (aimed at the sky).
#[must_use]
pub fn ground_point(camera: &Camera, aspect: f32, ndc: [f32; 2]) -> Option<(f32, f32)> {
    let eye = ember_engine::glam::Vec3::from(camera.eye);
    let tgt = ember_engine::glam::Vec3::from(camera.target);
    let fwd = (tgt - eye).normalize_or_zero();
    let right = fwd.cross(ember_engine::glam::Vec3::Y).normalize_or_zero();
    let up = right.cross(fwd);
    let half = (camera.fov_y_deg.to_radians() * 0.5).tan();
    let dir = (fwd + right * (ndc[0] * aspect * half) + up * (ndc[1] * half)).normalize_or_zero();
    if dir.y > -1e-3 {
        return None; // parallel or upward
    }
    let t = -eye.y / dir.y;
    let p = eye + dir * t;
    Some((p.x, p.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mesh_has_vertices_and_normals() {
        for (i, m) in build_meshes().iter().enumerate() {
            assert!(!m.vertices.is_empty(), "mesh {i} is empty");
            assert_eq!(m.vertices.len() % 3, 0, "mesh {i} is not a triangle list");
            for v in &m.vertices {
                let d = v.normal[0] * v.normal[0] + v.normal[1] * v.normal[1] + v.normal[2] * v.normal[2];
                assert!((d - 1.0).abs() < 0.01, "mesh {i} has an unnormalized or zero normal");
                assert!(v.pos[0].is_finite() && v.pos[1].is_finite() && v.pos[2].is_finite());
            }
        }
    }
}
