struct HeapDescriptors { entries: array<vec4<u32>, __DESCRIPTORS__>, }
struct HeapDirectory { spans: array<vec4<u32>, __SPANS__>, handles: array<vec4<u32>, __HANDLE_GROUPS__>, }
struct HeapResources { inputs: array<vec4<u32>, 8>, }
struct LatticeUniform {
    theta_one: f32,
    theta_two: f32,
    pole_five: f32,
    pole_four: f32,
    axis_counts: vec4<f32>,
    axis_five: f32,
    spacing: f32,
    fifth_range: f32,
    pole_epsilon: f32,
}
struct ModeCFrame {
    lattice: LatticeUniform,
    render: vec4<f32>,
    padding: array<vec4<f32>, 8>,
}
@group(0) @binding(0) var heap_data: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> heap_descriptors: HeapDescriptors;
@group(0) @binding(2) var<uniform> heap_directory: HeapDirectory;
@group(0) @binding(4) var<uniform> heap_resources: HeapResources;
@group(0) @binding(5) var<uniform> frame: ModeCFrame;

fn heap_handle(slot: u32) -> u32 {
    return heap_directory.handles[slot / 4u][slot % 4u];
}

fn heap_load(binding_slot: u32, index: u32) -> vec4<f32> {
    let selected = heap_resources.inputs[binding_slot];
    if (index >= selected.y) { return vec4<f32>(0.0); }
    let span = heap_directory.spans[selected.x];
    let page = index / span.x;
    let local = index % span.x;
    let heap_id = heap_handle(span.z + page);
    let descriptor = heap_descriptors.entries[heap_id & 1048575u];
    let width = descriptor.y >> 16u;
    let origin = vec2<u32>(descriptor.x >> 16u, descriptor.y & 65535u);
    let coordinate = origin + vec2<u32>(local % width, local / width);
    return textureLoad(heap_data, vec2<i32>(coordinate), i32(descriptor.x & 65535u), 0);
}

struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) hue: f32, }

@vertex fn mode_c_vertex(@location(0) local: vec3<f32>, @builtin(instance_index) instance: u32) -> VertexOut {
    let midpoint_hue = heap_load(0u, instance);
    let orientation_length = heap_load(1u, instance);
    let axis = orientation_length.xyz;
    let reference = select(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), abs(axis.y) > 0.9);
    let side = normalize(cross(reference, axis));
    let upward = cross(axis, side);
    let point = midpoint_hue.xyz + side * local.x * frame.render.x + upward * local.y * frame.render.x + axis * local.z * orientation_length.w * 0.5;
    let yaw = 0.21 * frame.render.w;
    let pitch = 0.13 * frame.render.w;
    let yawed = vec3(point.x * cos(yaw) - point.z * sin(yaw), point.y, point.x * sin(yaw) + point.z * cos(yaw));
    let viewed = vec3(yawed.x, yawed.y * cos(pitch) - yawed.z * sin(pitch), yawed.y * sin(pitch) + yawed.z * cos(pitch));
    var output: VertexOut;
    output.position = vec4(viewed.x * frame.render.z / frame.render.y, viewed.y * frame.render.z, 0.5 + viewed.z * 0.002, 1.0);
    if (orientation_length.w < 0.0) { output.position = vec4(2.0, 2.0, 2.0, 1.0); }
    output.hue = midpoint_hue.w;
    return output;
}

fn hue_rgb(hue: f32) -> vec3<f32> {
    return clamp(abs(fract(hue + vec3(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0) - 1.0, vec3(0.0), vec3(1.0));
}

@fragment fn mode_c_fragment(input: VertexOut) -> @location(0) vec4<f32> {
    return vec4(hue_rgb(input.hue), 0.82);
}
