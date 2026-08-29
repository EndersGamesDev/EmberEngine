```wgsl
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
    // Rotation matrix construction from instance yaw
    let c = cos(in.i_yaw);
    let s = sin(in.i_yaw);
    
    // Transform position: Scale -> Rotate -> Translate
    let scaled = in.pos * in.i_scale;
    let rotated = vec3<f32>(
        scaled.x * c + scaled.z * s,
        scaled.y,
        -scaled.x * s + scaled.z * c,
    );
    let world = rotated + in.i_pos;
    
    var out: VsOut;
    out.clip = camera.view_proj * vec4<f32>(world, 1.0);
    
    // Transform normal: Apply the same Y-rotation to the incoming normal vector
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
    // --- Lighting Tuning Constants ---
    const SUN_DIR: vec3<f32> = normalize(vec3<f32>(0.4, 1.0, 0.3)); // Sun direction
    const AMBIENT_STRENGTH: f32 = 0.15; // Base fill light
    const SUN_INTENSITY: f32 = 1.5;    // Strength of direct sun light
    const RIM_STRENGTH: f32 = 0.8;      // Strength of rim light effect
    const RIM_POWER: f32 = 3.0;        // Sharpness of the rim light
    
    // --- Material Tuning ---
    const FOG_DENSITY: f32 = 0.005;    // Density of distance fog (higher = thicker fog)
    const FOG_COLOR: vec3<f32> = vec3<f32>(0.02, 0.02, 0.04); // Dark horizon color (deep blue/black)

    // 1. Texture Sampling & Albedo
    // *Color path preserved*: Multiply texture RGB by instance color.
    let albedo = textureSample(mesh_tex, mesh_samp, in.uv).rgb * in.color;

    // 2. Blinn-Phong Lighting
    let normal = normalize(in.normal);
    
    // Diffuse (Lambertian) component
    let half_vector = normalize(SUN_DIR + vec3<f32>(0.0, 1.0, 0.0)); // Light + View up (approximate view dir for rim)
    let NdotL = max(dot(normal, SUN_DIR), 0.0);
    let diffuse = AMBIENT_STRENGTH + SUN_INTENSITY * NdotL;
    
    // Specular (Blinn-Phong) component
    let NdotH = pow(max(dot(normal, half_vector), 0.0), RIM_POWER);
    let specular = RIM_STRENGTH * NdotH;
    
    // Combine lighting
    let lit = albedo * (diffuse + vec3<f32>(specular));

    // 3. Distance Fog (Linear approximation based on distance from camera origin)
    // Note: 'world' position is not passed to fragment shader in the current vertex output,
    // so we approximate distance using the camera's view vector or clip space depth.
    // Using the length of the camera position relative to the origin as a proxy for scene distance.
    let scene_dist = length(camera.view_proj[3].xyz); // Approximate world distance from camera
    let fog_factor = clamp(1.0 - exp(-scene_dist * FOG_DENSITY), 0.0, 1.0);
    
    // Mix albedo with fog color
    let final_color = mix(lit, FOG_COLOR, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}
```
