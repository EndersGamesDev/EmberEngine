const SHADE_SHADER: &str = r"
struct PaletteUniform { map: vec4<f32>, interior_rgba: vec4<f32>, clear_rgba: vec4<f32>, }
@group(0) @binding(0) var presentation_values: texture_2d<f32>;
@group(0) @binding(1) var nearest_value: sampler;
@group(1) @binding(0) var<uniform> palette: PaletteUniform;
struct ShadeVertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, }
@vertex fn shade_vertex(@builtin(vertex_index) vertex: u32) -> ShadeVertex {
    var points = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var output: ShadeVertex;
    output.position = vec4<f32>(points[vertex], 0.0, 1.0);
    output.uv = vec2<f32>(0.5 * points[vertex].x + 0.5, 0.5 - 0.5 * points[vertex].y);
    return output;
}
fn finite(value: f32) -> bool { return abs(value) <= 3.402823e38; }
fn binary(value: f32) -> bool { return value == 0.0 || value == 1.0; }
fn hue_component(hue: f32, offset: f32) -> f32 {
    return clamp(abs(fract(hue + offset) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
}
fn exterior_zero() -> vec4<f32> {
    let hue = fract(palette.map.y);
    let phase_rgb = vec3<f32>(hue_component(hue, 0.0), hue_component(hue, 0.6666666667), hue_component(hue, 0.3333333333));
    return vec4<f32>(palette.map.w * mix(vec3<f32>(1.0), phase_rgb, palette.map.z), 1.0);
}
fn colour(value: vec4<f32>) -> vec4<f32> {
    let status = value.z;
    if (status == 7.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (status == 1.0) { return vec4<f32>(1.0, 0.375, 0.0, 1.0); }
    if (status == 4.0 || status == 5.0) { return palette.clear_rgba; }
    if (status == 2.0 || status == 6.0) { return exterior_zero(); }
    if (status != 0.0 && status != 3.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (!binary(value.y)) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    var base = vec4<f32>(0.0);
    if (value.y == 0.0) {
        if (value.x != -1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
        base = palette.interior_rgba;
    } else {
        if (!finite(value.x) || !finite(palette.map.x) || palette.map.x <= 0.0) {
            return vec4<f32>(1.0, 0.0, 1.0, 1.0);
        }
        let hue = fract(max(value.x, 0.0) / palette.map.x + palette.map.y);
        let phase_rgb = vec3<f32>(hue_component(hue, 0.0), hue_component(hue, 0.6666666667), hue_component(hue, 0.3333333333));
        let rgb = palette.map.w * mix(vec3<f32>(1.0), phase_rgb, palette.map.z);
        base = vec4<f32>(rgb, 1.0);
    }
    if (!finite(value.w) || value.w < 0.58 || value.w > 0.82) {
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    return vec4<f32>(base.rgb * value.w, base.a);
}
@fragment fn shade_fragment(input: ShadeVertex) -> @location(0) vec4<f32> {
    return colour(textureSample(presentation_values, nearest_value, input.uv));
}
";

/// Returns the sole value-to-colour presentation shader.
#[must_use]
pub const fn shade_shader() -> &'static str {
    SHADE_SHADER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shade_source_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(shade_shader()).expect("shade WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("shade WGSL validates");
    }

    #[test]
    fn shade_is_the_only_palette_reader_and_pins_every_status_colour() {
        let source = shade_shader();
        assert!(source.contains("var<uniform> palette: PaletteUniform"));
        assert!(source.contains("status == 4.0 || status == 5.0"));
        assert!(source.contains("status == 2.0 || status == 6.0"));
        assert!(source.contains("status == 7.0"));
        assert!(source.contains("status == 1.0"));
        assert!(source.contains("base = palette.interior_rgba;"));
        assert!(source.contains("textureSample(presentation_values, nearest_value, input.uv)"));
        assert!(!source.contains("textureSampleLevel"));
    }
}
