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
    @location(6) i_rot: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
    // View-space depth (perspective w) for distance fog.
    @location(3) view_depth: f32,
};

// Rotate v by the unit quaternion q (xyzw layout, matching glam).
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let scaled = in.pos * in.i_scale;
    let world = quat_rotate(in.i_rot, scaled) + in.i_pos;
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    // Rotate the normal like the position; sign() undoes mirroring from
    // negative scale axes (magnitude of a nonuniform scale stays ignored,
    // as before).
    out.normal = quat_rotate(in.i_rot, in.normal * sign(in.i_scale));
    out.color = in.i_color;
    out.uv = in.uv;
    out.view_depth = out.clip.w;
    return out;
}

// ACES filmic tonemap (Narkowicz fit): soft highlight rolloff instead of
// clipping to white.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let v = x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14);
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Lighting tuning.
    let sun_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let sun_col = vec3<f32>(1.0, 0.95, 0.85);  // warm key light
    let sun_intensity = 1.15;
    // Cool fill from the opposite side. The sun points almost straight
    // down, so without this every vertical facade — walls, crates, the
    // skyline — falls to ambient alone and reads as a black silhouette.
    let fill_dir = normalize(vec3<f32>(-0.55, 0.30, -0.65));
    let fill_col = vec3<f32>(0.42, 0.50, 0.68);
    let fill_intensity = 0.55;
    let sheen_strength = 0.18;   // stylized top sheen (Blinn half-vector vs up)
    let sheen_power = 8.0;
    // Fog tuning: view-depth based, fades to a dark horizon. Light enough
    // that the skyline beyond the arena wall still reads.
    let fog_density = 0.005;
    let fog_color = vec3<f32>(0.012, 0.020, 0.045);

    // Untextured meshes sample the shared 1x1 white pixel, so albedo
    // reduces to the instance color exactly as before.
    let albedo = textureSample(mesh_tex, mesh_samp, in.uv).rgb * in.color;

    let n = normalize(in.normal);
    let ndotl = max(dot(n, sun_dir), 0.0);
    // Hemisphere ambient: cool sky from above, warm ground bounce below.
    let hemi = mix(
        vec3<f32>(0.060, 0.055, 0.048),
        vec3<f32>(0.115, 0.135, 0.180),
        n.y * 0.5 + 0.5,
    );
    let half_vec = normalize(sun_dir + vec3<f32>(0.0, 1.0, 0.0));
    let sheen = sheen_strength * pow(max(dot(n, half_vec), 0.0), sheen_power);
    let fill = fill_col * (fill_intensity * max(dot(n, fill_dir), 0.0));
    let lit = aces(albedo * (hemi + fill + sun_col * (sun_intensity * ndotl + sheen)));

    let fog = clamp(1.0 - exp(-in.view_depth * fog_density), 0.0, 1.0);
    return vec4<f32>(mix(lit, fog_color, fog), 1.0);
}
