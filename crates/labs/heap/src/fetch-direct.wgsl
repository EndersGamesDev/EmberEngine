@group(0) @binding(0) var payload: texture_2d<f32>;

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

fn coordinate(pixel: vec2<u32>, fetch: u32) -> vec2<i32> {
    let seed = pixel.x * 1664525u + pixel.y * 1013904223u + fetch * 747796405u;
    return vec2<i32>(i32((seed ^ (seed >> 16u)) & 63u), i32(((seed >> 6u) ^ (seed >> 19u)) & 63u));
}

@fragment
fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> {
    let pixel = vec2<u32>(input.clip.xy);
    var sum = vec4<f32>(0.0);
    for (var fetch = 0u; fetch < 16u; fetch += 1u) {
        sum += textureLoad(payload, coordinate(pixel, fetch), 0);
    }
    return abs(fract(sum * 0.03125));
}
