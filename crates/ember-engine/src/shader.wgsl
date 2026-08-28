struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // Per-instance:
    @location(2) i_pos: vec3<f32>,
    @location(3) i_scale: vec3<f32>,
    @location(4) i_color: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let world = in.pos * in.i_scale + in.i_pos;
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    // Axis-aligned scaling only, so the normal is unchanged in direction.
    out.normal = in.normal;
    out.color = in.i_color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let sun = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let ambient = 0.35;
    let diff = max(dot(normalize(in.normal), sun), 0.0);
    let lit = in.color * (ambient + (1.0 - ambient) * diff);
    return vec4<f32>(lit, 1.0);
}
