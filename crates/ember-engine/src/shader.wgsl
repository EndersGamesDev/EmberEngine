struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

@group(1) @binding(0) var mesh_tex: texture_2d<f32>;
@group(1) @binding(1) var mesh_samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // Per-instance:
    @location(3) i_pos: vec3<f32>,
    @location(4) i_scale: vec3<f32>,
    @location(5) i_color: vec3<f32>,
    @location(6) i_yaw: f32,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let c = cos(in.i_yaw);
    let s = sin(in.i_yaw);
    let scaled = in.pos * in.i_scale;
    let rotated = vec3<f32>(
        scaled.x * c + scaled.z * s,
        scaled.y,
        -scaled.x * s + scaled.z * c,
    );
    let world = rotated + in.i_pos;
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    // Uniform-per-axis scale + Y rotation: rotate the normal the same way.
    out.normal = vec3<f32>(
        in.normal.x * c + in.normal.z * s,
        in.normal.y,
        -in.normal.x * s + in.normal.z * c,
    );
    out.color = in.i_color;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sun = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let ambient = 0.35;
    let diff = max(dot(normalize(in.normal), sun), 0.0);
    // Untextured meshes sample the shared 1x1 white pixel, so albedo
    // reduces to the instance color exactly as before.
    let albedo = textureSample(mesh_tex, mesh_samp, in.uv).rgb * in.color;
    let lit = albedo * (ambient + (1.0 - ambient) * diff);
    return vec4<f32>(lit, 1.0);
}
