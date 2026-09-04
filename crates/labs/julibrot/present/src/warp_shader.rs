const WARP_SHADER: &str = r"
struct HotUniform { camera_rotation_pairs_0: vec4<f32>, camera_rotation_pairs_1: vec4<f32>, camera_rotation_pairs_2: vec4<f32>, camera_rotation_pairs_3: vec4<f32>, camera_rotation_pairs_4: vec4<f32>, camera_translation_0: vec4<f32>, camera_translation_1: vec4<f32>, observer_rotation: vec4<f32>, view_scale: vec4<f32>, homography_row_0: vec4<f32>, homography_row_1: vec4<f32>, homography_row_2: vec4<f32>, screen_to_plane_row_0: vec4<f32>, screen_to_plane_row_1: vec4<f32>, screen_to_plane_row_2: vec4<f32>, exterior_zero_rgba: vec4<f32>, clear_rgba: vec4<f32>, flags: vec4<u32>, }
struct SceneUniform { grid: vec4<u32>, span: vec4<u32>, basis_u: vec4<f32>, basis_v: vec4<f32>, screen_to_plane_row_0: vec4<f32>, screen_to_plane_row_1: vec4<f32>, screen_to_plane_row_2: vec4<f32>, palette_map: vec4<f32>, interior_rgba: vec4<f32>, clear_rgba: vec4<f32>, }
@group(0) @binding(0) var source_scene: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;
@group(1) @binding(0) var<uniform> hot: HotUniform;
@group(1) @binding(1) var<uniform> scene: SceneUniform;
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
    if (hot.flags.w != 0u) { return hot.exterior_zero_rgba; }
    if (hot.flags.z == 0u) { return hot.clear_rgba; }
    let destination = vec3<f32>(input.chart * vec2<f32>(scene.grid.xy) * 0.5, 1.0);
    let plane = vec3<f32>(dot(hot.screen_to_plane_row_0.xyz, destination), dot(hot.screen_to_plane_row_1.xyz, destination), dot(hot.screen_to_plane_row_2.xyz, destination));
    if (!all(vec3<bool>(finite(plane.x), finite(plane.y), finite(plane.z))) || plane.z <= 0.0) { return hot.exterior_zero_rgba; }
    let mapped = vec3<f32>(dot(hot.homography_row_0.xyz, destination), dot(hot.homography_row_1.xyz, destination), dot(hot.homography_row_2.xyz, destination));
    if (!all(vec3<bool>(finite(mapped.x), finite(mapped.y), finite(mapped.z))) || mapped.z <= 0.0) { return hot.exterior_zero_rgba; }
    let source_pixel = mapped.xy / mapped.z;
    let source_extent = vec2<f32>(textureDimensions(source_scene));
    let source_uv = vec2<f32>(source_pixel.x / source_extent.x + 0.5, 0.5 - source_pixel.y / source_extent.y);
    if (any(source_uv < vec2<f32>(0.0)) || any(source_uv > vec2<f32>(1.0))) { return hot.clear_rgba; }
    return textureSample(source_scene, source_sampler, source_uv);
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
    fn warp_paints_sky_but_clears_only_missing_source_and_disocclusion() {
        let source = warp_shader();
        assert!(source.contains("hot.flags.w != 0u"));
        assert!(source.contains("hot.flags.z == 0u"));
        assert!(source.contains("plane.z <= 0.0"));
        assert!(source.contains("mapped.z <= 0.0"));
        assert!(source.contains("source_uv < vec2<f32>(0.0)"));
        assert!(source.contains("source_uv > vec2<f32>(1.0)"));
        assert!(source.contains("return hot.clear_rgba;"));
        assert!(source.contains("return hot.exterior_zero_rgba;"));
        assert!(!source.contains("quotient_error > vec2<f32>(0.25)"));
        assert!(!source.contains("textureSampleLevel"));
    }

    #[test]
    fn warp_composes_current_screen_pixels_to_retained_pixels() {
        let source = warp_shader();
        assert!(source.contains("input.chart * vec2<f32>(scene.grid.xy) * 0.5"));
        assert!(source.contains("hot.screen_to_plane_row_0.xyz"));
        assert!(source.contains("hot.homography_row_0.xyz"));
        assert!(source.contains("0.5 - source_pixel.y / source_extent.y"));
    }
}
