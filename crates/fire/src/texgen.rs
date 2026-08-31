//! Procedural base-colour textures, generated at startup.
//!
//! Why procedural rather than baked PNGs: the wasm build embeds every asset
//! via `include_bytes!` and the deploy ships one bundle with no runtime
//! fetch, so a megabyte of texture is a megabyte every web player downloads.
//! These cost a few hundred lines of code and zero download bytes.
//!
//! Three renderer constraints shape everything here:
//!   * the shader multiplies a per-instance colour in, so these are authored
//!     near-grey and tinted at the call site — never pre-tinted;
//!   * there are no mipmaps, so high-frequency detail shimmers at distance.
//!     Every generator below is deliberately low-frequency;
//!   * UVs tile, so every pattern must wrap. All noise is evaluated on a
//!     torus, which makes seams impossible by construction rather than by
//!     careful authoring.

use ember_engine::TextureData;

/// Integer hash -> [0,1). Wrapping on the lattice is what makes the noise
/// tile: callers reduce coordinates mod the period before sampling.
fn hash2(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841);
    h = h.wrapping_mul(0x2545_f491);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_f491);
    ((h >> 8) & 0x00ff_ffff) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise with period `period` lattice cells in both axes, so it tiles
/// exactly when sampled over `[0, period)`.
fn value_noise(u: f32, v: f32, period: i32) -> f32 {
    let (xi, yi) = (u.floor() as i32, v.floor() as i32);
    let (fx, fy) = (smooth(u - u.floor()), smooth(v - v.floor()));
    let w = |x: i32, y: i32| hash2(x.rem_euclid(period), y.rem_euclid(period));
    let (a, b) = (w(xi, yi), w(xi + 1, yi));
    let (c, d) = (w(xi, yi + 1), w(xi + 1, yi + 1));
    let top = a + (b - a) * fx;
    let bot = c + (d - c) * fx;
    top + (bot - top) * fy
}

/// Summed octaves, each with its own tiling period so the sum still tiles.
/// `u`/`v` are in [0,1); amplitude halves per octave.
fn fbm(u: f32, v: f32, base_period: i32, octaves: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 1.0, 0.0, base_period);
    for _ in 0..octaves {
        sum += value_noise(u * p as f32, v * p as f32, p) * amp;
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self { w, h, px: vec![0; (w * h * 4) as usize] }
    }

    fn set(&mut self, x: u32, y: u32, r: f32, g: f32, b: f32) {
        let i = ((y * self.w + x) * 4) as usize;
        self.px[i] = (r.clamp(0.0, 1.0) * 255.0) as u8;
        self.px[i + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
        self.px[i + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
        self.px[i + 3] = 255;
    }

    fn into_texture(self) -> TextureData {
        TextureData { width: self.w, height: self.h, rgba8: self.px }
    }
}

/// Cobblestone courtyard paving: jittered running-bond setts with mortar
/// gutters, a per-stone value spread, and a slow moss mottle over the top.
pub fn cobblestone(size: u32, cells: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    let (cf, ci) = (cells as f32, cells as i32);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let row = (v * cf).floor() as i32;
            let off = if row % 2 == 0 { 0.0 } else { 0.5 };
            let (gu, gv) = (u * cf + off, v * cf);
            let (cu, cv) = (gu.floor() as i32, gv.floor() as i32);
            let (fu, fv) = (gu - gu.floor(), gv - gv.floor());
            // Per-stone gutter width, so the underlying grid never reads.
            let gutter = 0.055 + hash2(cu.rem_euclid(ci), cv.rem_euclid(ci)) * 0.035;
            let edge = fu.min(1.0 - fu).min(fv).min(1.0 - fv);
            let stone = hash2(cu.rem_euclid(ci) * 7 + 3, cv.rem_euclid(ci) * 13 + 5);
            let moss = fbm(u, v, 4, 3);
            if edge < gutter {
                // Mortar sits well below the setts: without mipmaps the only
                // thing selling the joint at distance is value contrast.
                let k = 0.15 + moss * 0.07;
                c.set(x, y, k, k + moss * 0.05, k * 0.94);
            } else {
                let k = 0.34 + stone * 0.20 + fbm(u * 2.0, v * 2.0, 8, 2) * 0.09;
                c.set(x, y, k * 0.99, k * (1.0 - moss * 0.08), k * 1.03);
            }
        }
    }
    c.into_texture()
}

/// Ashlar castle wall: large dressed blocks in running bond, with vertical
/// weather streaking so tall walls do not read as flat.
pub fn castle_stone(size: u32, cols: u32, rows: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    let (cw, rh) = (cols as f32, rows as f32);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let row = (v * rh).floor() as i32;
            let off = if row % 2 == 0 { 0.0 } else { 0.5 };
            let (gu, gv) = (u * cw + off, v * rh);
            let (bu, bv) = (gu.floor() as i32, gv.floor() as i32);
            let (fu, fv) = (gu - gu.floor(), gv - gv.floor());
            // Courses are wider than they are tall, so the mortar band has to
            // be scaled per axis or the horizontal joints look twice as thick.
            let edge = fu.min(1.0 - fu).min(fv.min(1.0 - fv) * (rh / cw));
            let blk = hash2(bu.rem_euclid(cols as i32) * 11, bv.rem_euclid(rows as i32) * 17);
            let grain = fbm(u * 3.0, v * 3.0, 8, 3);
            if edge < 0.045 {
                let k = 0.34 + grain * 0.08;
                c.set(x, y, k, k * 0.99, k * 0.95);
            } else {
                let streak = fbm(u * 10.0, v * 0.6, 16, 2);
                let k = 0.60 + blk * 0.16 + grain * 0.12 - streak * 0.08;
                c.set(x, y, k * 1.02, k, k * 0.93);
            }
        }
    }
    c.into_texture()
}

/// Slate roof and tower caps: overlapping courses of dark tiles.
pub fn slate(size: u32, cols: u32, rows: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    let (cw, rh) = (cols as f32, rows as f32);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let row = (v * rh).floor() as i32;
            let off = if row % 2 == 0 { 0.0 } else { 0.5 };
            let (gu, gv) = (u * cw + off, v * rh);
            let (tu, tv) = (gu.floor() as i32, gv.floor() as i32);
            let (fu, fv) = (gu - gu.floor(), gv - gv.floor());
            let tile = hash2(tu.rem_euclid(cols as i32) * 5, tv.rem_euclid(rows as i32) * 19);
            // Darkening toward the bottom of each course reads as overlap
            // without a second pass and without any transparency.
            let shade = 0.72 + 0.28 * (1.0 - fv);
            let gap = if fu < 0.03 || fv > 0.94 { 0.55 } else { 1.0 };
            let k = (0.30 + tile * 0.10 + fbm(u * 4.0, v * 4.0, 8, 2) * 0.06) * shade * gap;
            c.set(x, y, k * 0.94, k * 0.97, k * 1.08);
        }
    }
    c.into_texture()
}

/// Rough timber for gates, barriers and hoardings.
pub fn timber(size: u32, planks: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    let pf = planks as f32;
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let gu = u * pf;
            let pi = gu.floor() as i32;
            let fu = gu - gu.floor();
            let plank = hash2(pi.rem_euclid(planks as i32) * 23, 7);
            // Grain runs along the plank, so the noise is stretched in v.
            let grain = fbm(u * 6.0, v * 1.2, 16, 3);
            let seam = if fu < 0.035 || fu > 0.965 { 0.55 } else { 1.0 };
            let k = (0.42 + plank * 0.14 + grain * 0.18) * seam;
            c.set(x, y, k * 1.10, k * 0.82, k * 0.58);
        }
    }
    c.into_texture()
}

/// Heraldic banner: a deep red field with gold chevrons. Opaque by necessity
/// — the renderer has no alpha blending, so a banner is a thin box, not a
/// cut-out quad.
pub fn banner(size: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let weave = 0.95 + fbm(u * 20.0, v * 20.0, 32, 2) * 0.10;
            let chev = ((u - 0.5).abs() * 1.6 + 0.18 - v).abs() < 0.09
                || ((u - 0.5).abs() * 1.6 + 0.52 - v).abs() < 0.09;
            if chev {
                c.set(x, y, 0.86 * weave, 0.70 * weave, 0.22 * weave);
            } else {
                c.set(x, y, 0.46 * weave, 0.09 * weave, 0.11 * weave);
            }
        }
    }
    c.into_texture()
}

/// Courtyard turf for the run-off outside the racing line.
pub fn turf(size: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / size as f32, y as f32 / size as f32);
            let k = 0.20 + fbm(u, v, 8, 4) * 0.18 + fbm(u * 0.5, v * 0.5, 4, 2) * 0.10;
            c.set(x, y, k * 0.72, k * 1.10, k * 0.52);
        }
    }
    c.into_texture()
}

/// Start/finish chequer. Deliberately coarse: with no mipmaps, a fine
/// chequer aliases into a shimmering mess a few metres down the road.
pub fn chequer(size: u32, squares: u32) -> TextureData {
    let mut c = Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let sx = (x * squares / size) % 2;
            let sy = (y * squares / size) % 2;
            let wear = fbm(x as f32 / size as f32 * 3.0, y as f32 / size as f32 * 3.0, 8, 2);
            let k = if sx == sy { 0.90 - wear * 0.12 } else { 0.13 + wear * 0.08 };
            c.set(x, y, k, k, k);
        }
    }
    c.into_texture()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texture decoder is unforgiving and silent, so assert the shape of
    /// every buffer we hand it rather than finding out as an untextured mesh.
    #[test]
    fn textures_are_well_formed() {
        let cases: Vec<(&str, TextureData)> = vec![
            ("cobblestone", cobblestone(128, 8)),
            ("castle_stone", castle_stone(128, 4, 6)),
            ("slate", slate(64, 8, 10)),
            ("timber", timber(64, 5)),
            ("banner", banner(64)),
            ("turf", turf(64)),
            ("chequer", chequer(64, 8)),
        ];
        for (name, t) in cases {
            assert_eq!(
                t.rgba8.len(),
                (t.width * t.height * 4) as usize,
                "{name}: buffer length does not match dimensions"
            );
            assert!(t.rgba8.chunks(4).all(|p| p[3] == 255), "{name}: not fully opaque");
        }
    }

    /// The whole point of evaluating noise on a torus: opposite edges must
    /// agree, or every tiled surface shows a seam grid.
    #[test]
    fn noise_tiles_seamlessly() {
        for period in [4, 8, 16] {
            for i in 0..16 {
                let t = i as f32 / 16.0;
                let p = period as f32;
                assert!(
                    (value_noise(t * p, 0.0, period) - value_noise(t * p, p, period)).abs() < 1e-5,
                    "period {period}: seam across v at {t}"
                );
                assert!(
                    (value_noise(0.0, t * p, period) - value_noise(p, t * p, period)).abs() < 1e-5,
                    "period {period}: seam across u at {t}"
                );
            }
        }
    }
}
