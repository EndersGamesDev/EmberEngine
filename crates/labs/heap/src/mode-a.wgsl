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
    var first = heap_load(1u, index);
    var fifth = heap_load(2u, index).x;
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

struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) hue: f32, }

@vertex fn mode_a_vertex(@location(0) local: vec3<f32>, @builtin(instance_index) instance: u32) -> VertexOut {
    let edge_index = instance % 3000u;
    let copy_index = instance / 3000u;
    let edge = heap_load(0u, edge_index);
    let digits = centered_digits(copy_index);
    let digit_five = fifth_digit(copy_index);
    let first = endpoint(u32(edge.x), digits, digit_five);
    let second = endpoint(u32(edge.y), digits, digit_five);
    let delta = second.point - first.point;
    let edge_length = max(length(delta), 1.0e-20);
    let axis = delta / edge_length;
    let reference = select(vec3(0.0, 1.0, 0.0), vec3(1.0, 0.0, 0.0), abs(axis.y) > 0.9);
    let side = normalize(cross(reference, axis));
    let upward = cross(axis, side);
    let midpoint = 0.5 * (first.point + second.point);
    let point = midpoint + side * local.x * frame.render.x + upward * local.y * frame.render.x + axis * local.z * edge_length * 0.5;
    let cosine_yaw = cos(frame.axis_fifth_range.z);
    let sine_yaw = sin(frame.axis_fifth_range.z);
    let cosine_pitch = cos(frame.axis_fifth_range.w);
    let sine_pitch = sin(frame.axis_fifth_range.w);
    let yawed = vec3(point.x * cosine_yaw - point.z * sine_yaw, point.y, point.x * sine_yaw + point.z * cosine_yaw);
    let viewed = vec3(yawed.x, yawed.y * cosine_pitch - yawed.z * sine_pitch, yawed.y * sine_pitch + yawed.z * cosine_pitch);
    var output: VertexOut;
    let scale = frame.render.z;
    output.position = vec4(viewed.x * scale / frame.render.y, viewed.y * scale, 0.5 + viewed.z * 0.002, 1.0);
    if (!(first.valid && second.valid)) { output.position = vec4(2.0, 2.0, 2.0, 1.0); }
    output.hue = clamp((0.5 * (first.fifth + second.fifth)) / (2.0 * frame.axis_fifth_range.y) + 0.5, 0.0, 1.0);
    return output;
}

fn hue_rgb(hue: f32) -> vec3<f32> {
    return clamp(abs(fract(hue + vec3(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0) - 1.0, vec3(0.0), vec3(1.0));
}

@fragment fn mode_a_fragment(input: VertexOut) -> @location(0) vec4<f32> {
    return vec4(hue_rgb(input.hue), 0.82);
}
