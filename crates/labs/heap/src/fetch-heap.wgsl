struct DescriptorTable {
    records: array<vec4<u32>, __DESCRIPTOR_CAPACITY__>,
}

@group(0) @binding(0) var data_heap: texture_2d_array<f32>;
@group(0) @binding(1) var data_sampler: sampler;
@group(0) @binding(2) var image_heap: texture_2d_array<f32>;
@group(0) @binding(3) var image_sampler: sampler;
@group(0) @binding(4) var<uniform> descriptors: DescriptorTable;

struct FrameUniform {
    values: vec4<f32>,
}
@group(0) @binding(5) var<uniform> frame: FrameUniform;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
}

@vertex
fn vertex_main(@builtin(vertex_index) vertex: u32) -> VertexOut {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var output: VertexOut;
    output.clip = vec4<f32>(positions[vertex], 0.0, 1.0);
    return output;
}

fn logical_coordinate(pixel: vec2<u32>, fetch: u32) -> vec2<u32> {
    let seed = pixel.x * 1664525u + pixel.y * 1013904223u + fetch * 747796405u;
    return vec2<u32>((seed ^ (seed >> 16u)) & 63u, ((seed >> 6u) ^ (seed >> 19u)) & 63u);
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let record = descriptors.records[1u];
    let layer = record.x & 65535u;
    let origin = vec2<u32>(record.x >> 16u, record.y & 65535u);
    let extent = vec2<u32>(record.y >> 16u, record.z & 65535u);
    let pixel = vec2<u32>(input.clip.xy);
    var sum = vec4<f32>(0.0);
    for (var fetch = 0u; fetch < 16u; fetch += 1u) {
        let logical = logical_coordinate(pixel, fetch) % extent;
        sum += textureLoad(data_heap, vec2<i32>(origin + logical), i32(layer), 0);
    }
    return abs(fract(sum * 0.03125));
}
