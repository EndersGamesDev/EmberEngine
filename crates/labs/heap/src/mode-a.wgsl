struct HeapDescriptors { entries: array<vec4<u32>, __DESCRIPTORS__>, }
struct HeapDirectory { spans: array<vec4<u32>, __SPANS__>, handles: array<vec4<u32>, __HANDLE_GROUPS__>, }
struct HeapResources { inputs: array<vec4<u32>, 8>, }
struct FrameUniform {
    rotation: vec4<f32>,
    projection_spacing: vec4<f32>,
    render: vec4<f32>,
    axes_four: vec4<f32>,
    axis_fifth_range: vec4<f32>,
    basis_four: array<vec4<f32>, 5>,
    basis_fifth: array<vec4<f32>, 2>,
}
@group(0) @binding(0) var heap_data: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> heap_descriptors: HeapDescriptors;
@group(0) @binding(2) var<uniform> heap_directory: HeapDirectory;
@group(0) @binding(4) var<uniform> heap_resources: HeapResources;
@group(0) @binding(5) var<uniform> frame: FrameUniform;

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

fn fifth_basis(axis: u32) -> f32 {
    return frame.basis_fifth[axis / 4u][axis % 4u];
}

fn centered_digits(copy_index: u32) -> vec4<i32> {
    var remaining = copy_index;
    let counts = vec4<u32>(frame.axes_four);
    let first = i32(remaining % counts.x) - i32(counts.x / 2u);
    remaining = remaining / counts.x;
    let second = i32(remaining % counts.y) - i32(counts.y / 2u);
    remaining = remaining / counts.y;
    let third = i32(remaining % counts.z) - i32(counts.z / 2u);
    remaining = remaining / counts.z;
    let fourth = i32(remaining % counts.w) - i32(counts.w / 2u);
    return vec4(first, second, third, fourth);
}

fn fifth_digit(copy_index: u32) -> i32 {
    let counts = vec4<u32>(frame.axes_four);
    var remaining = copy_index / (counts.x * counts.y * counts.z * counts.w);
    let count = u32(frame.axis_fifth_range.x);
    return i32(remaining % count) - i32(count / 2u);
}

struct Projected { point: vec3<f32>, fifth: f32, valid: bool, }

fn endpoint(index: u32, digits: vec4<i32>, digit_five: i32) -> Projected {
    var first = heap_load(3u, index);
    var fifth = heap_load(4u, index).x;
    let weights = vec4<f32>(digits) * frame.projection_spacing.z;
    for (var axis = 0u; axis < 4u; axis++) {
        first += weights[axis] * frame.basis_four[axis];
        fifth += weights[axis] * fifth_basis(axis);
    }
    let weight_five = f32(digit_five) * frame.projection_spacing.z;
    first += weight_five * frame.basis_four[4];
    fifth += weight_five * fifth_basis(4u);
    let denominator_five = frame.projection_spacing.x - fifth;
    let safe_five = select(denominator_five, frame.projection_spacing.w, abs(denominator_five) < frame.projection_spacing.w);
    let projected_four = first * (frame.projection_spacing.x / safe_five);
    let denominator_four = frame.projection_spacing.y - projected_four.w;
    let safe_four = select(denominator_four, frame.projection_spacing.w, abs(denominator_four) < frame.projection_spacing.w);
    var result: Projected;
    result.point = projected_four.xyz * (frame.projection_spacing.y / safe_four);
    result.fifth = fifth;
    result.valid = denominator_five > frame.projection_spacing.w && denominator_four > frame.projection_spacing.w;
    return result;
}

struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) hue: f32, @location(1) light: f32, }

@vertex fn mode_a_vertex(@location(0) local: vec3<f32>, @builtin(instance_index) instance: u32) -> VertexOut {
    let edge_index = instance % 3000u;
    let copy_index = instance / 3000u;
    let edge = heap_load(2u, edge_index);
    let digits = centered_digits(copy_index);
    let digit_five = fifth_digit(copy_index);
    let first = endpoint(u32(edge.x), digits, digit_five);
    let second = endpoint(u32(edge.y), digits, digit_five);
    if (!(first.valid && second.valid)) {
        var invalid: VertexOut;
        invalid.position = vec4(2.0, 2.0, 2.0, 1.0);
        invalid.hue = 0.0;
        invalid.light = 0.0;
        return invalid;
    }
    let axis = second.point - first.point;
    let direction = normalize(axis);
    let helper = select(vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), abs(direction.z) < 0.9);
    let side = normalize(cross(direction, helper));
    let upward = normalize(cross(side, direction));
    let thickness = 0.013;
    let world = 0.5 * (first.point + second.point) + 0.5 * local.z * axis + thickness * local.x * side + thickness * local.y * upward;
    const camera_yaw_cosine = 0.9396926208;
    const camera_yaw_sine = 0.3420201433;
    const camera_pitch_cosine = 0.9659258263;
    const camera_pitch_sine = 0.2588190451;
    let yawed = vec3(camera_yaw_cosine * world.x + camera_yaw_sine * world.z, world.y, -camera_yaw_sine * world.x + camera_yaw_cosine * world.z);
    let view = vec3(yawed.x, camera_pitch_cosine * yawed.y - camera_pitch_sine * yawed.z, camera_pitch_sine * yawed.y + camera_pitch_cosine * yawed.z - 9.0);
    const camera_near = 0.1;
    const camera_far = 30.0;
    var output: VertexOut;
    output.position = vec4(1.72 * view.x / frame.render.y, 1.72 * view.y, (camera_far / (camera_near - camera_far)) * view.z + camera_far * camera_near / (camera_near - camera_far), -view.z);
    output.hue = clamp((0.5 * (first.fifth + second.fifth)) / (2.0 * frame.axis_fifth_range.y) + 0.5, 0.0, 1.0);
    output.light = 0.58 + 0.24 * abs(dot(normalize(side + upward), normalize(vec3(0.4, 0.7, 0.6))));
    return output;
}

fn hue_rgb(hue: f32) -> vec3<f32> {
    let phase = abs(fract(hue + vec3(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return clamp(phase - 1.0, vec3(0.0), vec3(1.0));
}

@fragment fn mode_a_fragment(input: VertexOut) -> @location(0) vec4<f32> {
    let rgb = mix(vec3(1.0), hue_rgb(input.hue), 0.78) * input.light;
    return vec4(rgb, 1.0);
}
