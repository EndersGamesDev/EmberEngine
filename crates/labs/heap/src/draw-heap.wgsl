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

struct VertexIn {
    @location(0) placement: vec4<f32>,
    @location(1) resource_handle: u32,
    @location(2) shape: u32,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @interpolate(flat) @location(0) resource_handle: u32,
}

fn local_vertex(vertex: u32, shape: u32) -> vec2<f32> {
    let triangle = array<vec2<f32>, 3>(
        vec2<f32>(-0.80, -0.65),
        vec2<f32>(0.85, -0.55),
        vec2<f32>(0.0, 0.90),
    );
    let point = triangle[vertex];
    let skew = f32(shape & 3u) * 0.07;
    return vec2<f32>(point.x + point.y * skew, point.y - point.x * skew * 0.4);
}

@vertex
fn vertex_main(input: VertexIn, @builtin(vertex_index) vertex: u32) -> VertexOut {
    let local = local_vertex(vertex, input.shape);
    var output: VertexOut;
    output.clip = vec4<f32>(input.placement.xy + local * input.placement.zw, 0.0, 1.0);
    output.resource_handle = input.resource_handle;
    return output;
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let index = input.resource_handle & 1048575u;
    let record = descriptors.records[index];
    let layer = record.x & 65535u;
    let origin = vec2<u32>(record.x >> 16u, record.y & 65535u);
    let dimensions = vec2<f32>(textureDimensions(image_heap));
    let uv = (vec2<f32>(origin) + vec2<f32>(0.5)) / dimensions;
    return textureSampleLevel(image_heap, image_sampler, uv, i32(layer), 0.0);
}
