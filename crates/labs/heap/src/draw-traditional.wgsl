@group(0) @binding(0) var material_texture: texture_2d<f32>;
@group(0) @binding(1) var material_sampler: sampler;

struct MaterialUniform {
    values: vec4<f32>,
}
@group(0) @binding(2) var<uniform> material: MaterialUniform;

struct VertexIn {
    @location(0) placement: vec4<f32>,
    @location(1) resource_handle: u32,
    @location(2) shape: u32,
}

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
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
    let shape = u32(material.values.y);
    let local = local_vertex(vertex, shape);
    var output: VertexOut;
    output.clip = vec4<f32>(input.placement.xy + local * input.placement.zw, 0.0, 1.0);
    return output;
}

@fragment
fn fragment_main() -> @location(0) vec4<f32> {
    return textureSampleLevel(material_texture, material_sampler, vec2<f32>(0.5), 0.0);
}
