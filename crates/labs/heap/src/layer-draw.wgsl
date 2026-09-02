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
@group(0) @binding(0) var midpoint_hue_texture: texture_2d<f32>;
@group(0) @binding(1) var orientation_length_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> frame: ModeCFrame;

struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) hue: f32, @location(1) light: f32, }

@vertex fn layer_vertex(@location(0) local: vec3<f32>, @builtin(instance_index) instance: u32) -> VertexOut {
    let width = textureDimensions(midpoint_hue_texture).x;
    let coordinate = vec2<i32>(i32(instance % width), i32(instance / width));
    let midpoint_hue = textureLoad(midpoint_hue_texture, coordinate, 0);
    let orientation_length = textureLoad(orientation_length_texture, coordinate, 0);
    if (orientation_length.w < 0.0) {
        var invalid: VertexOut;
        invalid.position = vec4(2.0, 2.0, 2.0, 1.0);
        invalid.hue = 0.0;
        invalid.light = 0.0;
        return invalid;
    }
    let direction = orientation_length.xyz;
    let axis = direction * orientation_length.w;
    let helper = select(vec3(0.0, 1.0, 0.0), vec3(0.0, 0.0, 1.0), abs(direction.z) < 0.9);
    let side = normalize(cross(direction, helper));
    let upward = normalize(cross(side, direction));
    let thickness = 0.013;
    let world = midpoint_hue.xyz + 0.5 * local.z * axis + thickness * local.x * side + thickness * local.y * upward;
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
    output.hue = midpoint_hue.w;
    output.light = 0.58 + 0.24 * abs(dot(normalize(side + upward), normalize(vec3(0.4, 0.7, 0.6))));
    return output;
}

fn hue_rgb(hue: f32) -> vec3<f32> {
    let phase = abs(fract(hue + vec3(0.0, 0.6666667, 0.3333333)) * 6.0 - 3.0);
    return clamp(phase - 1.0, vec3(0.0), vec3(1.0));
}

@fragment fn layer_fragment(input: VertexOut) -> @location(0) vec4<f32> {
    let rgb = mix(vec3(1.0), hue_rgb(input.hue), 0.78) * input.light;
    return vec4(rgb, 1.0);
}
