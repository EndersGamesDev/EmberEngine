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

/// Per-frame procedural lattice parameters; exactly 32 CPU-to-GPU bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LatticeUniform {
    /// Angle in the first/second-axis plane.
    pub theta_one: f32,
    /// Golden-ratio-related angle in the third/fifth-axis plane.
    pub theta_two: f32,
    /// Fifth-axis projection pole.
    pub pole_five: f32,
    /// Fourth-axis projection pole.
    pub pole_four: f32,
    /// Odd lattice radix as an exactly representable float.
    pub lattice_m: f32,
    /// Center-to-center lattice spacing.
    pub spacing: f32,
    /// Symmetric post-rotation fifth-axis hue range.
    pub fifth_range: f32,
    /// Positive denominator threshold for per-vertex projection validity.
    pub pole_epsilon: f32,
}

/// Pure per-vertex 5D rotation and double-projection kernel body.
pub const VERTEX_KERNEL: &str = r"
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
";

/// Pure per-edge gather, instance transform, and fifth-axis hue kernel body.
pub const EDGE_KERNEL: &str = r"
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
";

/// Procedural whole-lattice edge kernel with no per-copy input allocation.
pub const LATTICE_EDGE_KERNEL: &str = r"
struct LatticeUniform {
    theta_one: f32,
    theta_two: f32,
    pole_five: f32,
    pole_four: f32,
    lattice_m: f32,
    spacing: f32,
    fifth_range: f32,
    pole_epsilon: f32,
}
struct LatticeCenter {
    first_four: vec4<f32>,
    fifth: f32,
}
struct ProjectedVertex {
    point: vec3<f32>,
    fifth: f32,
    valid: f32,
}
struct EdgeResult {
    midpoint_hue: vec4<f32>,
    orientation_length: vec4<f32>,
}
fn decode_center(copy_index: u32, m: u32, spacing: f32) -> LatticeCenter {
    var remaining = copy_index;
    let half = i32(m / 2u);
    let first = i32(remaining % m) - half;
    remaining = remaining / m;
    let second = i32(remaining % m) - half;
    remaining = remaining / m;
    let third = i32(remaining % m) - half;
    remaining = remaining / m;
    let fourth = i32(remaining % m) - half;
    remaining = remaining / m;
    let fifth = i32(remaining % m) - half;
    var center: LatticeCenter;
    center.first_four = vec4<f32>(f32(first), f32(second), f32(third), f32(fourth)) * spacing;
    center.fifth = f32(fifth) * spacing;
    return center;
}
fn project_vertex(first: vec4<f32>, tail: vec4<f32>, center: LatticeCenter, uniforms: LatticeUniform) -> ProjectedVertex {
    let point = first + center.first_four;
    let fifth_input = tail.x + center.fifth;
    let cosine_one = cos(uniforms.theta_one);
    let sine_one = sin(uniforms.theta_one);
    let cosine_two = cos(uniforms.theta_two);
    let sine_two = sin(uniforms.theta_two);
    let rotated = vec4<f32>(
        point.x * cosine_one - point.y * sine_one,
        point.x * sine_one + point.y * cosine_one,
        point.z * cosine_two - fifth_input * sine_two,
        point.w
    );
    let fifth = point.z * sine_two + fifth_input * cosine_two;
    let denominator_five = uniforms.pole_five - fifth;
    let safe_five = select(denominator_five, uniforms.pole_epsilon, abs(denominator_five) < uniforms.pole_epsilon);
    let projected_four = rotated * (uniforms.pole_five / safe_five);
    let denominator_four = uniforms.pole_four - projected_four.w;
    let safe_four = select(denominator_four, uniforms.pole_epsilon, abs(denominator_four) < uniforms.pole_epsilon);
    var result: ProjectedVertex;
    result.point = projected_four.xyz * (uniforms.pole_four / safe_four);
    result.fifth = fifth;
    result.valid = select(0.0, 1.0, denominator_five > uniforms.pole_epsilon && denominator_four > uniforms.pole_epsilon);
    return result;
}
fn kernel(index: u32, uniforms: LatticeUniform) -> EdgeResult {
    let copy_index = index / 3000u;
    let base_edge = index % 3000u;
    let center = decode_center(copy_index, u32(uniforms.lattice_m), uniforms.spacing);
    let endpoints = load_edge(base_edge);
    let first_index = u32(endpoints.x);
    let second_index = u32(endpoints.y);
    let first = project_vertex(load_base_four(first_index), load_base_fifth(first_index), center, uniforms);
    let second = project_vertex(load_base_four(second_index), load_base_fifth(second_index), center, uniforms);
    let delta = second.point - first.point;
    let edge_length = length(delta);
    let safe_length = max(edge_length, 1.0e-20);
    let fifth = 0.5 * (first.fifth + second.fifth);
    let hue = clamp(fifth / (2.0 * uniforms.fifth_range) + 0.5, 0.0, 1.0);
    let submitted_length = select(-1.0, edge_length, first.valid * second.valid > 0.5);
    var result: EdgeResult;
    result.midpoint_hue = vec4<f32>(0.5 * (first.point + second.point), hue);
    result.orientation_length = vec4<f32>(delta / safe_length, submitted_length);
    return result;
}
";
