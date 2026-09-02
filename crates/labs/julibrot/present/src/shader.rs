use ember_lab_heap::DialectLimits;

const HEAP_SCENE_PREFIX: &str = r"
struct HeapDescriptors { entries: array<vec4<u32>, __DESCRIPTORS__>, }
struct HeapDirectory { spans: array<vec4<u32>, __SPANS__>, handles: array<vec4<u32>, __HANDLE_GROUPS__>, }
struct SceneUniform { grid: vec4<u32>, span: vec4<u32>, palette_map: vec4<f32>, interior_rgba: vec4<f32>, clear_rgba: vec4<f32>, }
struct HotUniform { plane_u: vec4<f32>, plane_v: vec4<f32>, view_rotation: vec4<f32>, homography_row_0: vec4<f32>, homography_row_1: vec4<f32>, homography_row_2: vec4<f32>, clear_rgba: vec4<f32>, flags: vec4<u32>, }
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
fn malformed(record: vec4<f32>) -> bool {
    return !binary(record.y) || !binary(record.w) || !finite(record.z) || record.z < 0.0 || record.z != floor(record.z);
}
fn hue_component(hue: f32, offset: f32) -> f32 {
    return clamp(abs(fract(hue + offset) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
}
fn shade(record: vec4<f32>) -> vec4<f32> {
    if (malformed(record) || record.w == 1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
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
    if (malformed(record) || record.w == 1.0) { return 0.0; }
    if (record.y == 0.0) { return -2.0; }
    return 4.0 * clamp(record.x / max(f32(scene.grid.w), 1.0), 0.0, 1.0) - 2.0;
}
";

const FLAT_BODY: &str = r"
struct FlatVertex { @builtin(position) position: vec4<f32>, }
@vertex fn flat_vertex(@builtin(vertex_index) vertex: u32) -> FlatVertex {
    var points = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var output: FlatVertex;
    output.position = vec4<f32>(points[vertex], 0.0, 1.0);
    return output;
}
@fragment fn flat_fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let column = min(u32(position.x), scene.grid.x - 1u);
    let top_row = min(u32(position.y), scene.grid.y - 1u);
    let row = scene.grid.y - 1u - top_row;
    return shade(load_escape(row * scene.grid.x + column));
}
";

const TUMBLED_BODY: &str = r"
struct TumbledVertex { @builtin(position) position: vec4<f32>, @location(0) world: vec3<f32>, @location(1) grid_coordinate: vec2<f32>, @location(2) valid: f32, }
@vertex fn tumbled_vertex(@builtin(vertex_index) index: u32) -> TumbledVertex {
    let column = index % scene.grid.x;
    let row = index / scene.grid.x;
    let record = load_escape(index);
    let q_u = 4.0 * ((f32(column) + 0.5) / f32(scene.grid.x) - 0.5);
    let q_v = 4.0 * (f32(row) + 0.5 - f32(scene.grid.y) * 0.5) / f32(scene.grid.x);
    let plane = q_u * hot.plane_u + q_v * hot.plane_v;
    let p_3 = plane.z * hot.view_rotation.z - record_height(record) * hot.view_rotation.w;
    let p_5 = plane.z * hot.view_rotation.w + record_height(record) * hot.view_rotation.z;
    let p_1 = plane.x * hot.view_rotation.x - plane.y * hot.view_rotation.y;
    let p_2 = plane.x * hot.view_rotation.y + plane.y * hot.view_rotation.x;
    var output: TumbledVertex;
    output.world = vec3<f32>(0.0);
    output.grid_coordinate = vec2<f32>(f32(column), f32(row));
    output.valid = 0.0;
    output.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
    let denominator_five = 8.0 - p_5;
    if (denominator_five <= 1.0e-4) { return output; }
    let scale_five = 8.0 / denominator_five;
    let projected_four = vec4<f32>(p_1, p_2, p_3, plane.w) * scale_five;
    let denominator_four = 8.0 - projected_four.w;
    if (denominator_four <= 1.0e-4) { return output; }
    let scale_four = 8.0 / denominator_four;
    let world = projected_four.xyz * scale_four;
    output.world = world;
    output.valid = 1.0;
    const camera_yaw_cosine = 0.9396926208;
    const camera_yaw_sine = 0.3420201433;
    const camera_pitch_cosine = 0.9659258263;
    const camera_pitch_sine = 0.2588190451;
    const camera_near = 0.1;
    const camera_far = 30.0;
    let yawed = vec3<f32>(camera_yaw_cosine * world.x + camera_yaw_sine * world.z, world.y, -camera_yaw_sine * world.x + camera_yaw_cosine * world.z);
    let view = vec3<f32>(yawed.x, camera_pitch_cosine * yawed.y - camera_pitch_sine * yawed.z, camera_pitch_sine * yawed.y + camera_pitch_cosine * yawed.z - 9.0);
    let clip_depth = (camera_far / (camera_near - camera_far)) * view.z + camera_far * camera_near / (camera_near - camera_far);
    let aspect = f32(scene.grid.x) / f32(scene.grid.y);
    output.position = vec4<f32>(1.72 * view.x / aspect, 1.72 * view.y, clip_depth, -view.z);
    return output;
}
@fragment fn tumbled_fragment(input: TumbledVertex) -> @location(0) vec4<f32> {
    if (input.valid < 0.999999) { discard; }
    let limit = vec2<f32>(f32(scene.grid.x - 1u), f32(scene.grid.y - 1u));
    let coordinate = vec2<u32>(clamp(floor(input.grid_coordinate + vec2<f32>(0.5)), vec2<f32>(0.0), limit));
    let record = load_escape(coordinate.y * scene.grid.x + coordinate.x);
    if (malformed(record) || record.w == 1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    let normal_cross = cross(dpdx(input.world), dpdy(input.world));
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    if (dot(normal_cross, normal_cross) > 1.0e-12) { normal = normalize(normal_cross); }
    let light = 0.58 + 0.24 * abs(dot(normal, normalize(vec3<f32>(0.4, 0.7, 0.6))));
    let base = shade(record);
    return vec4<f32>(base.rgb * light, base.a);
}
";

/// Fully instantiated seam-independent scene shader sources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShaderSources {
    /// Fullscreen flat scene shader.
    pub flat: String,
    /// Indexed tumbled scene shader.
    pub tumbled: String,
}

fn instantiate_scene(body: &str, limits: DialectLimits) -> String {
    let mut source = HEAP_SCENE_PREFIX
        .replace("__DESCRIPTORS__", &limits.descriptor_capacity.to_string())
        .replace("__SPANS__", &limits.span_capacity.to_string())
        .replace(
            "__HANDLE_GROUPS__",
            &limits.handle_capacity.div_ceil(4).to_string(),
        );
    source.push_str(body);
    source
}

/// Instantiates both scene shaders at immutable heap capacities.
#[must_use]
pub fn scene_shaders(limits: DialectLimits) -> ShaderSources {
    ShaderSources {
        flat: instantiate_scene(FLAT_BODY, limits),
        tumbled: instantiate_scene(TUMBLED_BODY, limits),
    }
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

    fn validate(source: &str) {
        let module = naga::front::wgsl::parse_str(source).expect("presentation WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("presentation WGSL validates");
    }

    #[test]
    fn both_scene_sources_parse_and_validate() {
        let shaders = scene_shaders(limits());
        validate(&shaders.flat);
        validate(&shaders.tumbled);
    }

    #[test]
    fn flat_shader_loads_bottom_row_and_debugs_before_escape() {
        let source = scene_shaders(limits()).flat;
        assert!(source.contains("let row = scene.grid.y - 1u - top_row;"));
        assert!(source.contains("malformed(record) || record.w == 1.0"));
        assert!(source.contains("vec4<f32>(1.0, 0.0, 1.0, 1.0)"));
        assert!(source.contains("textureLoad(heap_data"));
    }

    #[test]
    fn tumbled_shader_pins_heap_rotation_poles_camera_depth_and_light() {
        let source = scene_shaders(limits()).tumbled;
        for required in [
            "plane.z * hot.view_rotation.z - record_height(record) * hot.view_rotation.w",
            "plane.x * hot.view_rotation.x - plane.y * hot.view_rotation.y",
            "let denominator_five = 8.0 - p_5;",
            "let denominator_four = 8.0 - projected_four.w;",
            "if (denominator_five <= 1.0e-4) { return output; }",
            "if (denominator_four <= 1.0e-4) { return output; }",
            "const camera_yaw_cosine = 0.9396926208;",
            "const camera_yaw_sine = 0.3420201433;",
            "const camera_pitch_cosine = 0.9659258263;",
            "const camera_pitch_sine = 0.2588190451;",
            "const camera_near = 0.1;",
            "const camera_far = 30.0;",
            "1.72 * view.x / aspect",
            "0.58 + 0.24",
            "normalize(vec3<f32>(0.4, 0.7, 0.6))",
        ] {
            assert!(
                source.contains(required),
                "missing tumbled literal {required}"
            );
        }
        assert!(!source.contains("0.013"));
    }
}
