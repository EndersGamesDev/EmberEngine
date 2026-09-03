use ember_lab_heap::DialectLimits;

const HEAP_SCENE_PREFIX: &str = r"
struct HeapDescriptors { entries: array<vec4<u32>, __DESCRIPTORS__>, }
struct HeapDirectory { spans: array<vec4<u32>, __SPANS__>, handles: array<vec4<u32>, __HANDLE_GROUPS__>, }
struct SceneUniform { grid: vec4<u32>, span: vec4<u32>, basis_u: vec4<f32>, basis_v: vec4<f32>, screen_to_plane_row_0: vec4<f32>, screen_to_plane_row_1: vec4<f32>, screen_to_plane_row_2: vec4<f32>, palette_map: vec4<f32>, interior_rgba: vec4<f32>, clear_rgba: vec4<f32>, }
struct HotUniform { camera_rotation_pairs_0: vec4<f32>, camera_rotation_pairs_1: vec4<f32>, camera_rotation_pairs_2: vec4<f32>, camera_rotation_pairs_3: vec4<f32>, camera_rotation_pairs_4: vec4<f32>, camera_translation_0: vec4<f32>, camera_translation_1: vec4<f32>, observer_rotation: vec4<f32>, view_scale: vec4<f32>, homography_row_0: vec4<f32>, homography_row_1: vec4<f32>, homography_row_2: vec4<f32>, screen_to_plane_row_0: vec4<f32>, screen_to_plane_row_1: vec4<f32>, screen_to_plane_row_2: vec4<f32>, exterior_zero_rgba: vec4<f32>, clear_rgba: vec4<f32>, flags: vec4<u32>, }
@group(0) @binding(0) var heap_data: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> heap_descriptors: HeapDescriptors;
@group(0) @binding(2) var<uniform> heap_directory: HeapDirectory;
@group(1) @binding(0) var<uniform> scene: SceneUniform;
@group(1) @binding(1) var<uniform> hot: HotUniform;
fn heap_handle(slot: u32) -> u32 { return heap_directory.handles[slot / 4u][slot % 4u]; }
fn load_escape(index: u32) -> vec4<f32> {
    if (index >= scene.span.y) { return vec4<f32>(0.0); }
    let span = heap_directory.spans[scene.span.x];
    let page = index / span.x;
    let local = index % span.x;
    let heap_id = heap_handle(span.z + page);
    let descriptor = heap_descriptors.entries[heap_id & 1048575u];
    let width = descriptor.y >> 16u;
    let origin = vec2<u32>(descriptor.x >> 16u, descriptor.y & 65535u);
    let coordinate = origin + vec2<u32>(local % width, local / width);
    return textureLoad(heap_data, vec2<i32>(coordinate), i32(descriptor.x & 65535u), 0);
}
fn binary(value: f32) -> bool { return value == 0.0 || value == 1.0; }
fn finite(value: f32) -> bool { return abs(value) <= 3.402823e38; }
fn terminal_status(value: f32) -> bool { return value == 0.0 || value == 1.0 || value == 2.0 || value == 3.0; }
fn malformed(record: vec4<f32>) -> bool {
    return !binary(record.y) || !terminal_status(record.w) || !finite(record.z) || record.z < 0.0 || record.z != floor(record.z);
}
fn hue_component(hue: f32, offset: f32) -> f32 {
    return clamp(abs(fract(hue + offset) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
}
fn shade(record: vec4<f32>) -> vec4<f32> {
    if (malformed(record) || record.w == 1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (record.w == 2.0) { return hot.exterior_zero_rgba; }
    if (record.y == 0.0) {
        if (record.x == -1.0) { return scene.interior_rgba; }
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    if (!finite(record.x) || record.x < 0.0 || !finite(scene.palette_map.x) || scene.palette_map.x <= 0.0) {
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    let hue = fract(record.x / scene.palette_map.x + scene.palette_map.y);
    let phase_rgb = vec3<f32>(hue_component(hue, 0.0), hue_component(hue, 0.6666666667), hue_component(hue, 0.3333333333));
    let rgb = scene.palette_map.w * mix(vec3<f32>(1.0), phase_rgb, scene.palette_map.z);
    return vec4<f32>(rgb, 1.0);
}
fn record_height(record: vec4<f32>) -> f32 {
    if (malformed(record) || record.w == 1.0 || record.w == 2.0) { return 0.0; }
    if (record.y == 0.0) {
        if (record.x == -1.0) { return -2.0; }
        return 0.0;
    }
    if (!finite(record.x) || record.x < 0.0) { return 0.0; }
    return 4.0 * clamp(record.x / max(f32(scene.grid.w), 1.0), 0.0, 1.0) - 2.0;
}
";
const SCENE_BODY: &str = r"
struct SceneVertex { @builtin(position) position: vec4<f32>, @location(0) world: vec3<f32>, @location(1) grid_coordinate: vec2<f32>, @location(2) valid: f32, }
struct Ambient5 { low: vec4<f32>, fifth: f32, }
fn rotate_12(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.x = pair.x * value.low.x - pair.y * value.low.y; out.low.y = pair.y * value.low.x + pair.x * value.low.y; return out; }
fn rotate_13(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.x = pair.x * value.low.x - pair.y * value.low.z; out.low.z = pair.y * value.low.x + pair.x * value.low.z; return out; }
fn rotate_14(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.x = pair.x * value.low.x - pair.y * value.low.w; out.low.w = pair.y * value.low.x + pair.x * value.low.w; return out; }
fn rotate_23(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.y = pair.x * value.low.y - pair.y * value.low.z; out.low.z = pair.y * value.low.y + pair.x * value.low.z; return out; }
fn rotate_24(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.y = pair.x * value.low.y - pair.y * value.low.w; out.low.w = pair.y * value.low.y + pair.x * value.low.w; return out; }
fn rotate_34(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.z = pair.x * value.low.z - pair.y * value.low.w; out.low.w = pair.y * value.low.z + pair.x * value.low.w; return out; }
fn rotate_15(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.x = pair.x * value.low.x - pair.y * value.fifth; out.fifth = pair.y * value.low.x + pair.x * value.fifth; return out; }
fn rotate_25(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.y = pair.x * value.low.y - pair.y * value.fifth; out.fifth = pair.y * value.low.y + pair.x * value.fifth; return out; }
fn rotate_35(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.z = pair.x * value.low.z - pair.y * value.fifth; out.fifth = pair.y * value.low.z + pair.x * value.fifth; return out; }
fn rotate_45(value: Ambient5, pair: vec2<f32>) -> Ambient5 { var out = value; out.low.w = pair.x * value.low.w - pair.y * value.fifth; out.fifth = pair.y * value.low.w + pair.x * value.fifth; return out; }
fn ambient_camera(value: Ambient5) -> Ambient5 {
    var rotated = rotate_45(value, hot.camera_rotation_pairs_4.zw);
    rotated = rotate_35(rotated, hot.camera_rotation_pairs_4.xy);
    rotated = rotate_25(rotated, hot.camera_rotation_pairs_3.zw);
    rotated = rotate_15(rotated, hot.camera_rotation_pairs_3.xy);
    rotated = rotate_34(rotated, hot.camera_rotation_pairs_2.zw);
    rotated = rotate_24(rotated, hot.camera_rotation_pairs_2.xy);
    rotated = rotate_23(rotated, hot.camera_rotation_pairs_1.zw);
    rotated = rotate_14(rotated, hot.camera_rotation_pairs_1.xy);
    rotated = rotate_13(rotated, hot.camera_rotation_pairs_0.zw);
    rotated = rotate_12(rotated, hot.camera_rotation_pairs_0.xy);
    rotated.low += hot.camera_translation_0;
    rotated.fifth += hot.camera_translation_1.x;
    return rotated;
}
@vertex fn scene_vertex(@builtin(vertex_index) index: u32) -> SceneVertex {
    let column = index % scene.grid.x;
    let row = index / scene.grid.x;
    let record = load_escape(index);
    let screen_x = f32(column) + 0.5 - 0.5 * f32(scene.grid.x);
    let screen_y = f32(row) + 0.5 - 0.5 * f32(scene.grid.y);
    let direct_ndc = vec2<f32>(2.0 * screen_x / f32(scene.grid.x), 2.0 * screen_y / f32(scene.grid.y));
    var output: SceneVertex;
    output.world = vec3<f32>(0.0);
    output.grid_coordinate = vec2<f32>(f32(column), f32(row));
    output.valid = 1.0;
    output.position = vec4<f32>(direct_ndc, 0.0, 1.0);
    if (scene.span.z != 0u || record.w == 2.0 || hot.view_scale.x == 0.0) { return output; }
    let screen = vec3<f32>(screen_x, screen_y, 1.0);
    let plane_homogeneous = vec3<f32>(dot(scene.screen_to_plane_row_0.xyz, screen), dot(scene.screen_to_plane_row_1.xyz, screen), dot(scene.screen_to_plane_row_2.xyz, screen));
    if (!all(vec3<bool>(finite(plane_homogeneous.x), finite(plane_homogeneous.y), finite(plane_homogeneous.z))) || plane_homogeneous.z <= 0.0) { return output; }
    let plane_offset = plane_homogeneous.xy / plane_homogeneous.z;
    if (!all(vec2<bool>(finite(plane_offset.x), finite(plane_offset.y)))) { return output; }
    let chart_scale = 4.0 / f32(scene.grid.x);
    let display = chart_scale * (plane_offset.x * scene.basis_u + plane_offset.y * scene.basis_v);
    let height = hot.view_scale.x * record_height(record);
    let ambient = ambient_camera(Ambient5(display, height));
    output.valid = 0.0;
    output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let distance_five = hot.view_scale.y;
    let distance_four = hot.view_scale.z;
    let denominator_five = distance_five - ambient.fifth;
    if (denominator_five <= 1.0e-4) { return output; }
    let scale_five = distance_five / denominator_five;
    let projected_four = ambient.low * scale_five;
    let denominator_four = distance_four - projected_four.w;
    if (denominator_four <= 1.0e-4) { return output; }
    let scale_four = distance_four / denominator_four;
    let world = projected_four.xyz * scale_four;
    output.world = world;
    output.valid = 1.0;
    let camera_yaw_cosine = hot.observer_rotation.x;
    let camera_yaw_sine = hot.observer_rotation.y;
    let camera_pitch_cosine = hot.observer_rotation.z;
    let camera_pitch_sine = hot.observer_rotation.w;
    let camera_near = 0.1;
    let camera_far = 4.0 * distance_four;
    let yawed = vec3<f32>(camera_yaw_cosine * world.x + camera_yaw_sine * world.z, world.y, -camera_yaw_sine * world.x + camera_yaw_cosine * world.z);
    let view = vec3<f32>(yawed.x, camera_pitch_cosine * yawed.y - camera_pitch_sine * yawed.z, camera_pitch_sine * yawed.y + camera_pitch_cosine * yawed.z - distance_four);
    if (-view.z <= 1.0e-4) { output.valid = 0.0; output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0); return output; }
    let clip_depth = (camera_far / (camera_near - camera_far)) * view.z + camera_far * camera_near / (camera_near - camera_far);
    let aspect = f32(scene.grid.x) / f32(scene.grid.y);
    let perspective_scale = aspect * distance_four * 0.5;
    output.position = vec4<f32>(perspective_scale * view.x / aspect, perspective_scale * view.y, clip_depth, -view.z);
    return output;
}
@fragment fn scene_fragment(input: SceneVertex) -> @location(0) vec4<f32> {
    if (input.valid < 0.999999) { discard; }
    let limit = vec2<f32>(f32(scene.grid.x - 1u), f32(scene.grid.y - 1u));
    let coordinate = vec2<u32>(clamp(floor(input.grid_coordinate + vec2<f32>(0.5)), vec2<f32>(0.0), limit));
    let record = load_escape(coordinate.y * scene.grid.x + coordinate.x);
    if (malformed(record) || record.w == 1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (record.w == 2.0 || scene.span.z != 0u) { return hot.exterior_zero_rgba; }
    let normal_cross = cross(dpdx(input.world), dpdy(input.world));
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    if (dot(normal_cross, normal_cross) > 1.0e-12) { normal = normalize(normal_cross); }
    let light = 0.58 + 0.24 * abs(dot(normal, normalize(vec3<f32>(0.4, 0.7, 0.6))));
    let base = shade(record);
    return vec4<f32>(base.rgb * light, base.a);
}
";

/// Instantiates the one scene shader at immutable heap capacities.
#[must_use]
pub fn scene_shader(limits: DialectLimits) -> String {
    let mut source = HEAP_SCENE_PREFIX
        .replace("__DESCRIPTORS__", &limits.descriptor_capacity.to_string())
        .replace("__SPANS__", &limits.span_capacity.to_string())
        .replace(
            "__HANDLE_GROUPS__",
            &limits.handle_capacity.div_ceil(4).to_string(),
        );
    source.push_str(SCENE_BODY);
    source
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> DialectLimits {
        DialectLimits {
            descriptor_capacity: 256,
            span_capacity: 64,
            handle_capacity: 128,
        }
    }

    #[test]
    fn the_one_scene_source_parses_and_validates() {
        let source = scene_shader(limits());
        let module = naga::front::wgsl::parse_str(&source).expect("presentation WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("presentation WGSL validates");
    }

    fn assert_translates_to_webgl2(source: &str, stage: naga::ShaderStage, entry_point: &str) {
        let module = naga::front::wgsl::parse_str(source).expect("presentation WGSL parses");
        let module_info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("presentation WGSL validates");
        let options = naga::back::glsl::Options {
            version: naga::back::glsl::Version::Embedded {
                version: 300,
                is_webgl: true,
            },
            ..Default::default()
        };
        let pipeline_options = naga::back::glsl::PipelineOptions {
            shader_stage: stage,
            entry_point: entry_point.to_string(),
            multiview: None,
        };
        let mut glsl = String::new();
        naga::back::glsl::Writer::new(
            &mut glsl,
            &module,
            &module_info,
            &options,
            &pipeline_options,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .expect("presentation WGSL lowers to WebGL2 GLSL")
        .write()
        .expect("WebGL2 GLSL writes");
        assert!(glsl.starts_with("#version 300 es"));
    }

    #[test]
    fn both_present_shaders_translate_for_webgl2() {
        let scene = scene_shader(limits());
        for (source, entries) in [
            (
                scene.as_str(),
                [
                    (naga::ShaderStage::Vertex, "scene_vertex"),
                    (naga::ShaderStage::Fragment, "scene_fragment"),
                ],
            ),
            (
                crate::warp_shader(),
                [
                    (naga::ShaderStage::Vertex, "warp_vertex"),
                    (naga::ShaderStage::Fragment, "warp_fragment"),
                ],
            ),
        ] {
            for (stage, entry_point) in entries {
                assert_translates_to_webgl2(source, stage, entry_point);
            }
        }
    }

    #[test]
    fn the_scene_loads_bottom_row_and_debugs_before_escape() {
        let source = scene_shader(limits());
        assert!(source.contains("let row = index / scene.grid.x;"));
        assert!(source.contains("malformed(record) || record.w == 1.0"));
        assert!(source.contains("if (record.w == 2.0) { return hot.exterior_zero_rgba; }"));
        assert!(!source.contains("record.w == 2.0 || record.w == 3.0"));
        assert!(source.contains("!finite(record.x) || record.x < 0.0"));
        assert!(source.contains("vec4<f32>(1.0, 0.0, 1.0, 1.0)"));
        assert!(source.contains("textureLoad(heap_data"));
    }

    #[test]
    fn the_scene_pins_heap_rotation_poles_camera_depth_and_light() {
        let source = scene_shader(limits());
        for required in [
            "let screen_x = f32(column) + 0.5 - 0.5 * f32(scene.grid.x);",
            "let screen_y = f32(row) + 0.5 - 0.5 * f32(scene.grid.y);",
            "let plane_homogeneous = vec3<f32>(dot(scene.screen_to_plane_row_0.xyz, screen), dot(scene.screen_to_plane_row_1.xyz, screen), dot(scene.screen_to_plane_row_2.xyz, screen));",
            "let display = chart_scale * (plane_offset.x * scene.basis_u + plane_offset.y * scene.basis_v);",
            "let height = hot.view_scale.x * record_height(record);",
            "scene.span.z != 0u || record.w == 2.0 || hot.view_scale.x == 0.0",
            "output.position = vec4<f32>(direct_ndc, 0.0, 1.0);",
            "rotate_45(value, hot.camera_rotation_pairs_4.zw)",
            "rotate_13(rotated, hot.camera_rotation_pairs_0.zw)",
            "rotate_12(rotated, hot.camera_rotation_pairs_0.xy)",
            "rotated.low += hot.camera_translation_0;",
            "rotated.fifth += hot.camera_translation_1.x;",
            "let distance_five = hot.view_scale.y;",
            "let distance_four = hot.view_scale.z;",
            "let denominator_five = distance_five - ambient.fifth;",
            "let denominator_four = distance_four - projected_four.w;",
            "if (denominator_five <= 1.0e-4) { return output; }",
            "if (denominator_four <= 1.0e-4) { return output; }",
            "let camera_yaw_cosine = hot.observer_rotation.x;",
            "let camera_pitch_cosine = hot.observer_rotation.z;",
            "let camera_near = 0.1;",
            "let camera_far = 4.0 * distance_four;",
            "let yawed = vec3<f32>(camera_yaw_cosine * world.x + camera_yaw_sine * world.z, world.y, -camera_yaw_sine * world.x + camera_yaw_cosine * world.z);",
            "let view = vec3<f32>(yawed.x, camera_pitch_cosine * yawed.y - camera_pitch_sine * yawed.z, camera_pitch_sine * yawed.y + camera_pitch_cosine * yawed.z - distance_four);",
            "let perspective_scale = aspect * distance_four * 0.5;",
            // The three places d4 reaches the picture. At height zero the clip-space w is d4 and
            // the scale is proportional to d4, so the two cancel and leave the chart map, and the
            // browser agrees once the picture is read at rest. The earlier report of a d4-dependent
            // height-zero framing sampled the picture mid-refresh; the same wrong framing appears
            // with d4 held fixed while d5 moves, so it is not d4's. These lines stay pinned because
            // the cancellation is the whole reason the height-zero image is exact.
            "camera_pitch_sine * yawed.y + camera_pitch_cosine * yawed.z - distance_four",
            "output.position = vec4<f32>(perspective_scale * view.x / aspect, perspective_scale * view.y, clip_depth, -view.z);",
            "0.58 + 0.24",
            "normalize(vec3<f32>(0.4, 0.7, 0.6))",
        ] {
            assert!(
                source.contains(required),
                "missing scene literal {required}"
            );
        }
        assert!(!source.contains("0.013"));
        // The retired mount and the retired lane names must name nothing.
        for forbidden in [
            "0.9396926208",
            "0.3420201433",
            "0.9659258263",
            "0.2588190451",
            "1.72",
            "hot.plane_u",
            "hot.plane_v",
            "hot.view_rotation",
            "flat_vertex",
            "flat_fragment",
        ] {
            assert!(
                !source.contains(forbidden),
                "the scene shader must not carry {forbidden}"
            );
        }
    }
}
