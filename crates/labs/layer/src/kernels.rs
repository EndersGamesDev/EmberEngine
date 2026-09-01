//! The pillar's deliberately small clients of the kernel dialect.

use bytemuck::{Pod, Zeroable};

/// Per-frame vertex projection parameters; exactly 16 CPU-to-GPU bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct VertexUniform {
    /// Angle in the first/second-axis plane.
    pub theta_one: f32,
    /// Golden-ratio-related angle in the third/fifth-axis plane.
    pub theta_two: f32,
    /// Fifth-axis projection pole.
    pub pole_five: f32,
    /// Fourth-axis projection pole.
    pub pole_four: f32,
}

/// Fixed edge parameters; exactly 16 CPU-to-GPU bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EdgeUniform {
    /// Symmetric fixed range used to map post-rotation fifth position to hue.
    pub fifth_range: f32,
    /// Three-dimensional box half-thickness.
    pub half_thickness: f32,
    /// Uniform-layout padding.
    pub padding: [f32; 2],
}

/// Pure per-vertex 5D rotation and double-projection kernel body.
pub const VERTEX_KERNEL: &str = r#"
struct VertexUniform {
    theta_one: f32,
    theta_two: f32,
    pole_five: f32,
    pole_four: f32,
}
struct VertexResult {
    view_position: vec4<f32>,
    fifth_axis: vec4<f32>,
}
fn kernel(index: u32, uniforms: VertexUniform) -> VertexResult {
    let first = load_base_four(index);
    let tail = load_base_fifth(index);
    let cosine_one = cos(uniforms.theta_one);
    let sine_one = sin(uniforms.theta_one);
    let cosine_two = cos(uniforms.theta_two);
    let sine_two = sin(uniforms.theta_two);
    let rotated = vec4<f32>(
        first.x * cosine_one - first.y * sine_one,
        first.x * sine_one + first.y * cosine_one,
        first.z * cosine_two - tail.x * sine_two,
        first.w
    );
    let fifth = first.z * sine_two + tail.x * cosine_two;
    let projected_four = rotated * (uniforms.pole_five / (uniforms.pole_five - fifth));
    let projected_three = projected_four.xyz * (uniforms.pole_four / (uniforms.pole_four - projected_four.w));
    var result: VertexResult;
    result.view_position = vec4<f32>(projected_three, 1.0);
    result.fifth_axis = vec4<f32>(fifth, 0.0, 0.0, 0.0);
    return result;
}
"#;

/// Pure per-edge gather, instance transform, and fifth-axis hue kernel body.
pub const EDGE_KERNEL: &str = r#"
struct EdgeUniform {
    fifth_range: f32,
    half_thickness: f32,
    padding: vec2<f32>,
}
struct EdgeResult {
    midpoint_hue: vec4<f32>,
    orientation_length: vec4<f32>,
}
fn kernel(index: u32, uniforms: EdgeUniform) -> EdgeResult {
    let endpoints = load_edge(index);
    let first_index = u32(endpoints.x);
    let second_index = u32(endpoints.y);
    let first = load_view(first_index).xyz;
    let second = load_view(second_index).xyz;
    let delta = second - first;
    let edge_length = length(delta);
    let fifth = 0.5 * (load_fifth(first_index).x + load_fifth(second_index).x);
    let hue = clamp(fifth / (2.0 * uniforms.fifth_range) + 0.5, 0.0, 1.0);
    var result: EdgeResult;
    result.midpoint_hue = vec4<f32>(0.5 * (first + second), hue);
    result.orientation_length = vec4<f32>(delta / edge_length, edge_length);
    return result;
}
"#;
