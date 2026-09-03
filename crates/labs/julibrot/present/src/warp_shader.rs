const WARP_SHADER: &str = r"
struct HotUniform { camera: vec4<f32>, view_scale: vec4<f32>, view_rotation: vec4<f32>, homography_row_0: vec4<f32>, homography_row_1: vec4<f32>, homography_row_2: vec4<f32>, clear_rgba: vec4<f32>, flags: vec4<u32>, }
@group(0) @binding(0) var source_scene: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(1) @binding(0) var<uniform> hot: HotUniform;
struct WarpVertex { @builtin(position) position: vec4<f32>, @location(0) chart: vec2<f32>, }
@vertex fn warp_vertex(@builtin(vertex_index) vertex: u32) -> WarpVertex {
    var points = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var output: WarpVertex;
    output.position = vec4<f32>(points[vertex], 0.0, 1.0);
    output.chart = points[vertex];
    return output;
}
fn finite(value: f32) -> bool { return abs(value) <= 3.402823e38; }
@fragment fn warp_fragment(input: WarpVertex) -> @location(0) vec4<f32> {
    if (hot.flags.z == 0u) { return hot.clear_rgba; }
    let point = vec3<f32>(input.chart, 1.0);
    let mapped = vec3<f32>(dot(hot.homography_row_0.xyz, point), dot(hot.homography_row_1.xyz, point), dot(hot.homography_row_2.xyz, point));
    if (!all(vec3<bool>(finite(mapped.x), finite(mapped.y), finite(mapped.z))) || abs(mapped.z) <= 1.0e-12) { return hot.clear_rgba; }
    let source_ndc = mapped.xy / mapped.z;
    if (any(source_ndc < vec2<f32>(-1.0)) || any(source_ndc > vec2<f32>(1.0))) { return hot.clear_rgba; }
    return textureSample(source_scene, source_sampler, (source_ndc + vec2<f32>(1.0)) * 0.5);
}
";

/// Returns the sole fullscreen warp-pass shader.
#[must_use]
pub const fn warp_shader() -> &'static str {
    WARP_SHADER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_source_parses_and_validates() {
        let module = naga::front::wgsl::parse_str(warp_shader()).expect("warp WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("warp WGSL validates");
    }

    #[test]
    fn warp_clears_invalid_poles_and_disocclusion() {
        let source = warp_shader();
        assert!(source.contains("hot.flags.z == 0u"));
        assert!(source.contains("abs(mapped.z) <= 1.0e-12"));
        assert!(source.contains("source_ndc < vec2<f32>(-1.0)"));
        assert!(source.contains("source_ndc > vec2<f32>(1.0)"));
        assert!(source.contains("return hot.clear_rgba;"));
        assert!(!source.contains("textureSampleLevel"));
    }
}
