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
    // View-space depth (perspective w) for distance fog.
    @location(3) view_depth: f32,
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
    out.view_depth = out.clip.w;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Lighting tuning.
    let sun_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let ambient = 0.22;          // base fill
    let sun_intensity = 0.95;    // direct sun
    let sheen_strength = 0.35;   // stylized top sheen (Blinn half-vector vs up)
    let sheen_power = 8.0;
    // Fog tuning: view-depth based, fades to a dark horizon.
    let fog_density = 0.012;
    let fog_color = vec3<f32>(0.012, 0.020, 0.045);

    // Untextured meshes sample the shared 1x1 white pixel, so albedo
    // reduces to the instance color exactly as before.
    let albedo = textureSample(mesh_tex, mesh_samp, in.uv).rgb * in.color;

    let n = normalize(in.normal);
    let ndotl = max(dot(n, sun_dir), 0.0);
    let half_vec = normalize(sun_dir + vec3<f32>(0.0, 1.0, 0.0));
    let sheen = sheen_strength * pow(max(dot(n, half_vec), 0.0), sheen_power);
    let lit = albedo * (ambient + sun_intensity * ndotl) + albedo * sheen;

    let fog = clamp(1.0 - exp(-in.view_depth * fog_density), 0.0, 1.0);
    return vec4<f32>(mix(lit, fog_color, fog), 1.0);
}
