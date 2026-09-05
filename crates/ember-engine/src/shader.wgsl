// Scene, sky, shadow and particles share one atomically hot-reloaded module.
struct SceneUniform {
    view_proj: mat4x4<f32>,
    fog: vec4<f32>,
    inverse_view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    sun_direction: vec4<f32>, // xyz TOWARD sun; w enables environment
    sun_color: vec4<f32>,     // rgb radiance, w intensity
    sky_zenith: vec4<f32>,    // rgb colour, w cloud coverage
    sky_horizon: vec4<f32>,   // rgb colour, w wetness
    wind_time: vec4<f32>,     // xy wind, z visual time, w shadow extent
    camera_right: vec4<f32>,
    camera_up: vec4<f32>,
};
@group(0) @binding(0) var<uniform> scene: SceneUniform;
@group(1) @binding(0) var mesh_tex: texture_2d<f32>;
@group(1) @binding(1) var mesh_samp: sampler;
// textureLoad avoids optional float filtering and depth-sampler differences.
@group(2) @binding(0) var shadow_tex: texture_2d<f32>;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) i_pos: vec3<f32>,
    @location(4) i_scale: vec3<f32>,
    @location(5) i_color: vec3<f32>,
    @location(6) i_rot: vec4<f32>,
    @location(7) i_material: vec2<f32>, // wettable, casts/receives shadow
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) view_depth: f32,
    @location(4) world: vec3<f32>,
    @location(5) @interpolate(flat) material: vec2<f32>,
};

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}
fn safe_normalize(v: vec3<f32>) -> vec3<f32> {
    return v * inverseSqrt(max(dot(v, v), 0.000001));
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let world = quat_rotate(in.i_rot, in.pos * in.i_scale) + in.i_pos;
    var out: VsOut;
    out.clip = scene.view_proj * vec4<f32>(world, 1.0);
    // Outdoor geometry gets inverse-transpose normals for nonuniform scale;
    // the disabled path retains the old scene's exact lighting calculation.
    var normal_scale = sign(in.i_scale);
    if scene.sun_direction.w > 0.5 {
        normal_scale = sign(in.i_scale) / max(abs(in.i_scale), vec3<f32>(0.0001));
    }
    out.normal = quat_rotate(in.i_rot, in.normal * normal_scale);
    out.color = in.i_color;
    out.uv = in.uv;
    out.view_depth = out.clip.w;
    out.world = world;
    out.material = in.i_material;
    return out;
}
fn aces(x: vec3<f32>) -> vec3<f32> {
    let v = x * (2.51 * x + 0.03) / (x * (2.43 * x + 0.59) + 0.14);
    return clamp(v, vec3<f32>(0.0), vec3<f32>(1.0));
}
fn noise_hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}
fn noise2(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(noise_hash(cell), noise_hash(cell + vec2<f32>(1.0, 0.0)), u.x),
        mix(noise_hash(cell + vec2<f32>(0.0, 1.0)), noise_hash(cell + vec2<f32>(1.0)), u.x), u.y);
}
fn cloud_noise(p: vec2<f32>) -> f32 {
    return noise2(p) * 0.57 + noise2(p * 2.03 + 13.7) * 0.29
        + noise2(p * 4.11 + 41.3) * 0.14;
}

// Shared by sky and wet-surface environment reflections. Reflects the
// sky/sun/clouds only, not scene geometry; this is not screen-space reflection.
fn sky_radiance(direction: vec3<f32>) -> vec3<f32> {
    let elevation = clamp(direction.y, 0.0, 1.0);
    var sky = mix(scene.sky_horizon.rgb, scene.sky_zenith.rgb, pow(elevation, 0.55));
    let sun_dot = clamp(dot(direction, scene.sun_direction.xyz), 0.0, 1.0);
    let horizon_gate = smoothstep(-0.05, 0.03, direction.y);
    let disk = smoothstep(0.99935, 0.99965, sun_dot) * horizon_gate;
    let halo = pow(sun_dot, 48.0) * 0.22 * horizon_gate;
    sky += scene.sun_color.rgb * scene.sun_color.w * (disk * 4.5 + halo);
    // Wind scrolls a virtual high cloud layer in world directions.
    let cloud_uv = direction.xz / max(direction.y + 0.08, 0.13) * 1.8
        + scene.wind_time.xy * scene.wind_time.z * 0.003;
    let field = cloud_noise(cloud_uv);
    let threshold = mix(1.03, 0.19, scene.sky_zenith.w);
    let density = smoothstep(threshold - 0.13, threshold + 0.16, field)
        * smoothstep(0.025, 0.20, direction.y) * smoothstep(0.0, 0.07, scene.sky_zenith.w);
    let cloud_light = mix(vec3<f32>(0.32, 0.37, 0.43), vec3<f32>(1.03, 1.01, 0.97),
        clamp(field * 0.75 + 0.22 + sun_dot * 0.10, 0.0, 1.0));
    sky = mix(sky, cloud_light * (0.55 + 0.35 * scene.sun_color.w), density * 0.95);
    return sky;
}

fn shadow_visibility(world: vec3<f32>, normal: vec3<f32>, ndotl: f32) -> f32 {
    let light = scene.light_view_proj * vec4<f32>(world + normal * 0.035, 1.0);
    let ndc = light.xyz / light.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if any(uv < vec2<f32>(0.002)) || any(uv > vec2<f32>(0.998)) || ndc.z <= 0.0 || ndc.z >= 1.0 {
        return 1.0;
    }
    let dimensions = vec2<i32>(textureDimensions(shadow_tex));
    let texel = vec2<i32>(uv * vec2<f32>(dimensions));
    // Each PCF tap samples a different point on the receiver plane. Comparing
    // every tap with the centre depth self-shadows even perfectly flat ground,
    // especially in a wider shadow volume. Transform the normal by inverse
    // transpose using the orthogonal rows of our directional-light matrix.
    // This analytic slope needs no screen derivatives or extra GPU features.
    let row_x = vec3<f32>(scene.light_view_proj[0].x, scene.light_view_proj[1].x, scene.light_view_proj[2].x);
    let row_y = vec3<f32>(scene.light_view_proj[0].y, scene.light_view_proj[1].y, scene.light_view_proj[2].y);
    let row_z = vec3<f32>(scene.light_view_proj[0].z, scene.light_view_proj[1].z, scene.light_view_proj[2].z);
    let plane = vec3<f32>(dot(normal, row_x) / dot(row_x, row_x),
        dot(normal, row_y) / dot(row_y, row_y), dot(normal, row_z) / dot(row_z, row_z));
    if abs(plane.z) < 0.00001 {
        return 1.0;
    }
    // Texture V points down while clip Y points up.
    let depth_gradient = vec2<f32>(-2.0 * plane.x, 2.0 * plane.y) / plane.z;
    let compare_depth = ndc.z - (0.00008 + 0.00010 * (1.0 - ndotl));
    var sum = 0.0;
    for (var y = -1; y <= 1; y += 1) {
        for (var x = -1; x <= 1; x += 1) {
            let coord = clamp(texel + vec2<i32>(x, y), vec2<i32>(0), dimensions - vec2<i32>(1));
            let packed = textureLoad(shadow_tex, coord, 0).rgb;
            let depth = dot(packed, vec3<f32>(1.0, 1.0 / 255.0, 1.0 / 65025.0));
            let tap_uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / vec2<f32>(dimensions);
            let receiver_depth = compare_depth + dot(depth_gradient, tap_uv - uv);
            sum += select(0.0, 1.0, receiver_depth <= depth);
        }
    }
    let border = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y));
    return mix(1.0, sum / 9.0, smoothstep(0.005, 0.06, border));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let outdoors = scene.sun_direction.w > 0.5;
    var sun_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    var sun_col = vec3<f32>(1.0, 0.95, 0.85);
    var sun_intensity = 1.15;
    if outdoors {
        sun_dir = scene.sun_direction.xyz;
        sun_col = scene.sun_color.xyz;
        sun_intensity = scene.sun_color.w;
    }
    let fill_dir = normalize(vec3<f32>(-0.55, 0.30, -0.65));
    let fill_col = vec3<f32>(0.42, 0.50, 0.68);
    let albedo = textureSample(mesh_tex, mesh_samp, in.uv).rgb * in.color;
    let n = safe_normalize(in.normal);
    let ndotl = max(dot(n, sun_dir), 0.0);
    var hemi = mix(vec3<f32>(0.060, 0.055, 0.048), vec3<f32>(0.115, 0.135, 0.180), n.y * 0.5 + 0.5);
    var visibility = 1.0;
    if outdoors {
        hemi = mix(vec3<f32>(0.08, 0.072, 0.058), scene.sky_horizon.rgb * 0.27, n.y * 0.5 + 0.5);
        if in.material.y > 0.5 && ndotl > 0.0 {
            visibility = shadow_visibility(in.world, n, ndotl);
        }
    }
    let half_vec = safe_normalize(sun_dir + vec3<f32>(0.0, 1.0, 0.0));
    let sheen = 0.18 * pow(max(dot(n, half_vec), 0.0), 8.0);
    let fill = fill_col * (0.55 * max(dot(n, fill_dir), 0.0));
    var radiance = albedo * (hemi + fill + sun_col * (sun_intensity * ndotl + sheen) * visibility);
    if outdoors && in.material.x > 0.5 && scene.sky_horizon.w > 0.001 {
        let view = safe_normalize(scene.eye.xyz - in.world);
        let wetness = scene.sky_horizon.w;
        let ripple = vec3<f32>(sin(in.world.x * 2.1 + scene.wind_time.z * 1.2), 0.0,
            sin(in.world.z * 1.8 - scene.wind_time.z * 0.9)) * 0.018 * max(n.y, 0.0) * wetness;
        let wet_normal = safe_normalize(n + ripple);
        let reflected = reflect(-view, wet_normal);
        let fresnel = 0.04 + 0.70 * pow(1.0 - max(dot(wet_normal, view), 0.0), 5.0);
        let reflectance = wetness * fresnel * smoothstep(-0.1, 0.12, reflected.y);
        radiance *= 1.0 - wetness * 0.20;
        radiance = mix(radiance, sky_radiance(reflected), reflectance);
        let specular_half = safe_normalize(sun_dir + view);
        radiance += sun_col * sun_intensity * wetness * visibility
            * pow(max(dot(wet_normal, specular_half), 0.0), 96.0) * 0.65;
    }
    let lit = aces(radiance);
    let fog = clamp(1.0 - exp(-in.view_depth * scene.fog.w), 0.0, 1.0);
    return vec4<f32>(mix(lit, scene.fog.rgb, fog), 1.0);
}

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};
@vertex
fn vs_sky(@builtin(vertex_index) index: u32) -> SkyOut {
    let xy = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    var out: SkyOut;
    out.clip = vec4<f32>(xy, 0.99999, 1.0);
    out.ndc = xy;
    return out;
}
@fragment
fn fs_sky(in: SkyOut) -> @location(0) vec4<f32> {
    let world = scene.inverse_view_proj * vec4<f32>(in.ndc, 0.9999, 1.0);
    let direction = safe_normalize(world.xyz / world.w - scene.eye.xyz);
    return vec4<f32>(aces(sky_radiance(direction)), 1.0);
}

@vertex
fn vs_shadow(in: VsIn) -> @builtin(position) vec4<f32> {
    let world = quat_rotate(in.i_rot, in.pos * in.i_scale) + in.i_pos;
    return scene.light_view_proj * vec4<f32>(world, 1.0);
}
@fragment
fn fs_shadow(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    // Pack depth into three 8-bit channels of a guaranteed RGBA8 target.
    var packed = fract(min(clip.z, 0.999999) * vec3<f32>(1.0, 255.0, 65025.0));
    packed -= packed.yzz * vec3<f32>(1.0 / 255.0, 1.0 / 255.0, 0.0);
    return vec4<f32>(packed, 1.0);
}

struct ParticleIn {
    @location(0) position: vec3<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec3<f32>,
    @location(3) opacity: f32,
};
struct ParticleOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) opacity: f32,
    @location(3) view_depth: f32,
};
@vertex
fn vs_particle(in: ParticleIn, @builtin(vertex_index) index: u32) -> ParticleOut {
    var corners = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
    let corner = corners[index];
    let world = in.position + scene.camera_right.xyz * corner.x * in.size.x * 0.5
        + scene.camera_up.xyz * corner.y * in.size.y * 0.5;
    var out: ParticleOut;
    out.clip = scene.view_proj * vec4<f32>(world, 1.0);
    out.uv = corner;
    out.color = in.color;
    out.opacity = in.opacity;
    out.view_depth = out.clip.w;
    return out;
}
@fragment
fn fs_particle(in: ParticleOut) -> @location(0) vec4<f32> {
    let alpha = (1.0 - smoothstep(0.12, 1.0, dot(in.uv, in.uv))) * in.opacity;
    if alpha < 0.003 { discard; }
    let fog = clamp(1.0 - exp(-in.view_depth * scene.fog.w), 0.0, 1.0);
    return vec4<f32>(mix(aces(in.color), scene.fog.rgb, fog), alpha);
}
