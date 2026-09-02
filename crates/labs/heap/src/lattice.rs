//! Algebraic lattice records, frame layout, indexed box, and Mode A shaders.

use bytemuck::{Pod, Zeroable};
use ember_lab_layer::geometry::{LATTICE_SPACING, Prism, lattice_fifth_range};

use crate::DialectLimits;

/// Exact author body registered for the 1,200-record Mode A rotation pass.
pub const MODE_A_ROTATION_KERNEL: &str = r"
struct FrameUniform {
    rotation: vec4<f32>,
    projection_spacing: vec4<f32>,
    render: vec4<f32>,
    axes_four: vec4<f32>,
    axis_fifth_range: vec4<f32>,
    basis_four: array<vec4<f32>, 5>,
    basis_fifth: array<vec4<f32>, 2>,
}
struct RotationResult {
    rotated_four: vec4<f32>,
    rotated_fifth: vec4<f32>,
}
fn kernel(index: u32, uniforms: FrameUniform) -> RotationResult {
    let point = load_base_four(index);
    let tail = load_base_fifth(index);
    var result: RotationResult;
    result.rotated_four = vec4<f32>(
        point.x * uniforms.rotation.x - point.y * uniforms.rotation.y,
        point.x * uniforms.rotation.y + point.y * uniforms.rotation.x,
        point.z * uniforms.rotation.z - tail.x * uniforms.rotation.w,
        point.w
    );
    result.rotated_fifth = vec4<f32>(
        point.z * uniforms.rotation.w + tail.x * uniforms.rotation.z,
        0.0,
        0.0,
        0.0
    );
    return result;
}
";

const MODE_A_DRAW_TEMPLATE: &str = include_str!("mode-a.wgsl");

/// CPU-to-GPU frame payload; exactly 192 bytes and the only dynamic write per frame.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct FrameUniform {
    /// Cosine/sine pairs for the first/second and third/fifth rotation planes.
    pub rotation: [f32; 4],
    /// Fifth pole, fourth pole, lattice spacing, and validity epsilon.
    pub projection_spacing: [f32; 4],
    /// Half thickness, viewport aspect, scene scale, and time.
    pub render: [f32; 4],
    /// Copy counts for axes one through four.
    pub axes_four: [f32; 4],
    /// Fifth count, fifth hue range, view yaw, and view pitch.
    pub axis_fifth_range: [f32; 4],
    /// First four coordinates of the five rotated basis vectors.
    pub basis_four: [[f32; 4]; 5],
    /// Fifth coordinates of the five bases followed by three zero padding values.
    pub basis_fifth: [[f32; 4]; 2],
}

/// One unique indexed-box corner.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct BoxVertex {
    /// Signed side, up, and endpoint selectors.
    pub local: [f32; 3],
}

/// Twelve triangles over the eight unique box corners.
pub const BOX_INDICES: [u16; 36] = [
    0, 2, 3, 0, 3, 1, 4, 5, 7, 4, 7, 6, 0, 1, 5, 0, 5, 4, 2, 6, 7, 2, 7, 3, 0, 4, 6, 0, 6, 2, 1, 3,
    7, 1, 7, 5,
];

/// Static DATA records uploaded once at scene construction.
#[derive(Clone, Debug)]
pub struct ModeARecordSet {
    /// Coordinates one through four, 1,200 records.
    pub base_four: Vec<[f32; 4]>,
    /// Fifth coordinate plus zero padding, 1,200 records.
    pub base_fifth: Vec<[f32; 4]>,
    /// Exactly integral endpoint indices, 3,000 records.
    pub edges: Vec<[f32; 4]>,
}

/// CPU oracle for one reconstructed and double-projected Mode A endpoint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModeAEndpoint {
    /// Projected three-dimensional point.
    pub point: [f32; 3],
    /// Post-rotation fifth coordinate used for hue.
    pub fifth: f32,
    /// Both perspective denominators exceeded the positive epsilon.
    pub valid: bool,
}

/// Returns all eight unique corners used with [`BOX_INDICES`].
#[must_use]
pub const fn box_vertices() -> [BoxVertex; 8] {
    [
        BoxVertex {
            local: [-1.0, -1.0, -1.0],
        },
        BoxVertex {
            local: [1.0, -1.0, -1.0],
        },
        BoxVertex {
            local: [-1.0, 1.0, -1.0],
        },
        BoxVertex {
            local: [1.0, 1.0, -1.0],
        },
        BoxVertex {
            local: [-1.0, -1.0, 1.0],
        },
        BoxVertex {
            local: [1.0, -1.0, 1.0],
        },
        BoxVertex {
            local: [-1.0, 1.0, 1.0],
        },
        BoxVertex {
            local: [1.0, 1.0, 1.0],
        },
    ]
}

fn rotate(point: [f32; 5], coefficients: [f32; 4]) -> [f32; 5] {
    [
        point[0] * coefficients[0] - point[1] * coefficients[1],
        point[0] * coefficients[1] + point[1] * coefficients[0],
        point[2] * coefficients[2] - point[4] * coefficients[3],
        point[3],
        point[2] * coefficients[3] + point[4] * coefficients[2],
    ]
}

fn basis_records(coefficients: [f32; 4]) -> ([[f32; 4]; 5], [[f32; 4]; 2]) {
    let mut first = [[0.0; 4]; 5];
    let mut fifth = [[0.0; 4]; 2];
    for axis in 0..5 {
        let mut basis = [0.0; 5];
        basis[axis] = 1.0;
        let rotated = rotate(basis, coefficients);
        first[axis].copy_from_slice(&rotated[..4]);
        fifth[axis / 4][axis % 4] = rotated[4];
    }
    (first, fifth)
}

/// Builds the 192-byte frame layout and CPU-rotated lattice bases.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn frame_for(object: &Prism, axes: [u32; 5], time: f32, aspect: f32) -> FrameUniform {
    let theta_one = 0.4 * time;
    let theta_two = f32::midpoint(1.0, 5.0_f32.sqrt()) * theta_one;
    let (sine_one, cosine_one) = theta_one.sin_cos();
    let (sine_two, cosine_two) = theta_two.sin_cos();
    let rotation = [cosine_one, sine_one, cosine_two, sine_two];
    let (basis_four, basis_fifth) = basis_records(rotation);
    FrameUniform {
        rotation,
        projection_spacing: [8.0, 8.0, LATTICE_SPACING as f32, 1.0e-4],
        render: [0.012, aspect, 0.075, time],
        axes_four: std::array::from_fn(|axis| axes[axis] as f32),
        axis_fifth_range: [
            axes[4] as f32,
            lattice_fifth_range(object, axes) as f32,
            0.21 * time,
            0.13 * time,
        ],
        basis_four,
        basis_fifth,
    }
}

/// Converts the shared 120-cell prism into the three static DATA spans.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn mode_a_records(object: &Prism) -> ModeARecordSet {
    ModeARecordSet {
        base_four: object
            .vertices
            .iter()
            .map(|point| std::array::from_fn(|axis| point[axis] as f32))
            .collect(),
        base_fifth: object
            .vertices
            .iter()
            .map(|point| [point[4] as f32, 0.0, 0.0, 0.0])
            .collect(),
        edges: object
            .edges
            .iter()
            .map(|edge| [edge.a as f32, edge.b as f32, 0.0, 0.0])
            .collect(),
    }
}

fn basis_fifth(frame: &FrameUniform, axis: usize) -> f32 {
    frame.basis_fifth[axis / 4][axis % 4]
}

/// Reconstructs and double-projects one endpoint in the shipped f32 operation order.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::suboptimal_flops)]
pub fn mode_a_endpoint(
    base: [f64; 5],
    coordinate: [i32; 5],
    frame: &FrameUniform,
) -> ModeAEndpoint {
    let mut rotated = rotate(base.map(|value| value as f32), frame.rotation);
    for (axis, digit) in coordinate.into_iter().enumerate() {
        let weight = digit as f32 * frame.projection_spacing[2];
        for (component, basis) in rotated[..4].iter_mut().zip(frame.basis_four[axis]) {
            *component += weight * basis;
        }
        rotated[4] += weight * basis_fifth(frame, axis);
    }
    let denominator_five = frame.projection_spacing[0] - rotated[4];
    let safe_five = if denominator_five.abs() < frame.projection_spacing[3] {
        frame.projection_spacing[3]
    } else {
        denominator_five
    };
    let scale_five = frame.projection_spacing[0] / safe_five;
    let projected_four: [f32; 4] = std::array::from_fn(|axis| rotated[axis] * scale_five);
    let denominator_four = frame.projection_spacing[1] - projected_four[3];
    let safe_four = if denominator_four.abs() < frame.projection_spacing[3] {
        frame.projection_spacing[3]
    } else {
        denominator_four
    };
    let scale_four = frame.projection_spacing[1] / safe_four;
    ModeAEndpoint {
        point: std::array::from_fn(|axis| projected_four[axis] * scale_four),
        fifth: rotated[4],
        valid: denominator_five > frame.projection_spacing[3]
            && denominator_four > frame.projection_spacing[3],
    }
}

/// Instantiates the presentation shader at the runtime descriptor and directory capacities.
#[must_use]
pub fn mode_a_shader(limits: DialectLimits) -> String {
    MODE_A_DRAW_TEMPLATE
        .replace("__DESCRIPTORS__", &limits.descriptor_capacity.to_string())
        .replace("__SPANS__", &limits.span_capacity.to_string())
        .replace(
            "__HANDLE_GROUPS__",
            &limits.handle_capacity.div_ceil(4).to_string(),
        )
}

#[cfg(test)]
mod tests {
    use ember_lab_layer::geometry::{
        EDGES_PER_COPY, LATTICE_SPACING, assert_invariants, lattice_coordinate, lattice_edge_count,
        lattice_steps, prism, project_gpu_path,
    };

    use super::{
        BOX_INDICES, FrameUniform, MODE_A_ROTATION_KERNEL, box_vertices, frame_for,
        mode_a_endpoint, mode_a_records, mode_a_shader,
    };
    use crate::{DialectLimits, KernelDesc, RegisteredKernel};

    #[test]
    fn geometry_ladder_frame_and_indexed_box_pin_the_contract() {
        assert_invariants();
        let object = prism();
        let records = mode_a_records(&object);
        assert_eq!(
            (records.base_four.len(), records.base_fifth.len()),
            (1_200, 1_200)
        );
        assert_eq!(records.edges.len(), 3_000);
        assert_eq!(size_of::<FrameUniform>(), 192);
        assert_eq!((box_vertices().len(), BOX_INDICES.len()), (8, 36));
        let steps = lattice_steps();
        assert_eq!(steps.len(), 113);
        assert_eq!(steps[111], [45; 5]);
        assert_eq!(steps[112], [47, 45, 45, 45, 45]);
        assert_eq!(lattice_edge_count(steps[112]), 578_188_125_000);
        assert_eq!(EDGES_PER_COPY, 3_000);
    }

    #[test]
    fn linear_decomposition_matches_direct_rotated_translation() {
        let object = prism();
        let axes = [7, 5, 3, 3, 1];
        let frame = frame_for(&object, axes, 2.375, 1.5);
        for copy in [0, 1, 53, 314] {
            let coordinate = lattice_coordinate(copy, axes).expect("copy is in range");
            for vertex in [0, 17, 599, 600, 1_199] {
                let algebraic = mode_a_endpoint(object.vertices[vertex], coordinate, &frame);
                let mut translated = object.vertices[vertex];
                for axis in 0..5 {
                    translated[axis] += f64::from(coordinate[axis]) * LATTICE_SPACING;
                }
                let direct = project_gpu_path(translated, 2.375);
                for component in 0..3 {
                    assert!((algebraic.point[component] - direct.0[component]).abs() < 5.0e-4);
                }
                assert!((algebraic.fifth - direct.1).abs() < 5.0e-4);
            }
        }
    }

    #[test]
    fn projection_is_not_linear_and_validity_tracks_both_poles() {
        let object = prism();
        let frame = frame_for(&object, [1; 5], 0.0, 1.0);
        let first = mode_a_endpoint([1.0, 0.0, 1.0, 1.0, 1.0], [0; 5], &frame);
        let second = mode_a_endpoint([0.0, 1.0, 1.0, 1.0, 1.0], [0; 5], &frame);
        let sum = mode_a_endpoint([1.0, 1.0, 2.0, 2.0, 2.0], [0; 5], &frame);
        assert_ne!(
            sum.point,
            std::array::from_fn(|axis| first.point[axis] + second.point[axis])
        );
        let rejected = mode_a_endpoint([0.0, 0.0, 0.0, 0.0, 8.0], [0; 5], &frame);
        assert!(!rejected.valid);
    }

    #[test]
    fn both_mode_a_shaders_parse_and_validate() {
        let limits = DialectLimits {
            descriptor_capacity: 64,
            span_capacity: 16,
            handle_capacity: 32,
        };
        RegisteredKernel::register(
            &KernelDesc {
                name: "mode_a_rotation",
                body: MODE_A_ROTATION_KERNEL,
                accessors: &["base_four", "base_fifth"],
                output_fields: &["rotated_four", "rotated_fifth"],
                uniform_type: "FrameUniform",
                uniform_size: 192,
                output_page_side: 64,
            },
            limits,
        )
        .expect("rotation kernel validates");
        let source = mode_a_shader(limits);
        let module = naga::front::wgsl::parse_str(&source).expect("draw shader parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("draw shader validates");
    }
}
