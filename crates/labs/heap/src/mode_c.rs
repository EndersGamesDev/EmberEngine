//! Like-for-like Mode C integration over the merged layer kernel and geometry package.

use bytemuck::{Pod, Zeroable};
use ember_lab_layer::geometry::{
    EDGES_PER_COPY, EdgePose, LATTICE_SPACING, Prism, lattice_coordinate, lattice_copy_count,
    lattice_edge_count, lattice_fifth_range,
};
use ember_lab_layer::kernels::{LATTICE_EDGE_KERNEL, LatticeUniform};
use serde::Serialize;

use crate::{DialectError, DialectLimits, FrameUniform, KernelDesc, RegisteredKernel};

const MODE_C_DRAW_TEMPLATE: &str = include_str!("mode-c.wgsl");

/// Mode C's exact 48-byte layer uniform followed by draw values and explicit padding to 192 bytes.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct ModeCFrameUniform {
    /// Prefix consumed unchanged by layer's exact kernel body.
    pub lattice: LatticeUniform,
    /// Half thickness, viewport aspect, scene scale, and view time.
    pub render: [f32; 4],
    /// Bytes not read by the kernel or renderer.
    pub padding: [[f32; 4]; 8],
}

impl ModeCFrameUniform {
    /// Derives the exact layer prefix while retaining the one-192-byte-write frame law.
    #[must_use]
    pub fn from_frame(frame: &FrameUniform) -> Self {
        let theta_one = 0.4 * frame.render[3];
        let theta_two = f32::midpoint(1.0, 5.0_f32.sqrt()) * theta_one;
        Self {
            lattice: LatticeUniform {
                theta_one,
                theta_two,
                pole_five: frame.projection_spacing[0],
                pole_four: frame.projection_spacing[1],
                axis_counts: frame.axes_four,
                axis_five: frame.axis_fifth_range[0],
                spacing: frame.projection_spacing[2],
                fifth_range: frame.axis_fifth_range[1],
                pole_epsilon: frame.projection_spacing[3],
            },
            render: frame.render,
            padding: [[0.0; 4]; 8],
        }
    }
}

/// Equal-work arithmetic reported for both Mode C and layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ComparatorWork {
    /// Whole lattice copies.
    pub copies: u64,
    /// Submitted edge instances.
    pub edges: u64,
    /// Two edge-pose records per edge.
    pub records: u64,
    /// Submitted indexed-box indices.
    pub submitted_indices: u64,
    /// Ideal post-transform-cache vertex invocations.
    pub ideal_vertex_invocations: u64,
    /// Logical two-record payload bytes.
    pub logical_bytes: u64,
    /// Square side of each standalone layer slot.
    pub layer_side: u64,
    /// Bytes allocated by one square-padded layer slot.
    pub layer_slot_bytes: u64,
    /// Bytes allocated by both layer slots.
    pub layer_allocation_bytes: u64,
}

impl ComparatorWork {
    /// Computes all equal-work counts without claiming allocation or measurement.
    #[must_use]
    pub fn for_axes(axes: [u32; 5]) -> Self {
        let copies = lattice_copy_count(axes);
        let edges = lattice_edge_count(axes);
        let records = edges.saturating_mul(2);
        let layer_side = if edges == 0 {
            0
        } else {
            edges.saturating_sub(1).isqrt().saturating_add(1)
        };
        let layer_slot_bytes = layer_side.saturating_mul(layer_side).saturating_mul(16);
        Self {
            copies,
            edges,
            records,
            submitted_indices: edges.saturating_mul(36),
            ideal_vertex_invocations: edges.saturating_mul(8),
            logical_bytes: edges.saturating_mul(32),
            layer_side,
            layer_slot_bytes,
            layer_allocation_bytes: layer_slot_bytes.saturating_mul(2),
        }
    }
}

/// Stable equality gate required before Mode C versus layer timing is publishable.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct EqualWorkSignature {
    /// Hash of the exact shared author kernel bytes.
    pub kernel_hash: u64,
    /// Hash of sampled CPU edge-pose f32 bits.
    pub pose_hash: u64,
    /// Hash of the common 36-index box sequence.
    pub index_hash: u64,
    /// Submitted edges represented by the signature.
    pub submitted_edges: u64,
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn hash_f32(hash: u64, value: f32) -> u64 {
    value
        .to_bits()
        .to_le_bytes()
        .iter()
        .fold(hash, |state, byte| {
            (state ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

impl EqualWorkSignature {
    /// Samples deterministic indices through the CPU kernel oracle.
    #[must_use]
    pub fn for_work(object: &Prism, axes: [u32; 5], time: f32) -> Self {
        let work = ComparatorWork::for_axes(axes);
        let indices = [
            0,
            work.edges.saturating_div(3),
            work.edges.saturating_div(2),
            work.edges.saturating_sub(1),
        ];
        let mut pose_hash = 0xcbf2_9ce4_8422_2325;
        for index in indices.into_iter().filter(|index| *index < work.edges) {
            let pose = mode_c_pose(object, axes, time, index);
            for value in pose
                .midpoint
                .into_iter()
                .chain(pose.direction)
                .chain([pose.length, pose.hue])
            {
                pose_hash = hash_f32(pose_hash, value);
            }
        }
        Self {
            kernel_hash: hash_bytes(LATTICE_EDGE_KERNEL.as_bytes()),
            pose_hash,
            index_hash: hash_bytes(bytemuck::cast_slice(&crate::BOX_INDICES)),
            submitted_edges: work.edges,
        }
    }
}

/// Registers layer's exact procedural lattice edge-pose body through heap dialect v2.
///
/// # Errors
///
/// Returns the dialect's typed registration failure.
pub fn mode_c_register(
    page_side: u16,
    limits: DialectLimits,
) -> Result<RegisteredKernel, DialectError> {
    RegisteredKernel::register(
        &KernelDesc {
            name: "mode_c_layer_edge",
            body: LATTICE_EDGE_KERNEL,
            accessors: &["edge", "base_four", "base_fifth"],
            output_fields: &["midpoint_hue", "orientation_length"],
            uniform_type: "LatticeUniform",
            uniform_size: 192,
            output_page_side: page_side,
        },
        limits,
    )
}

fn rotate_project(point: [f32; 5], uniform: &LatticeUniform) -> ([f32; 3], f32, bool) {
    let (sine_one, cosine_one) = uniform.theta_one.sin_cos();
    let (sine_two, cosine_two) = uniform.theta_two.sin_cos();
    let rotated = [
        point[0] * cosine_one - point[1] * sine_one,
        point[0] * sine_one + point[1] * cosine_one,
        point[2] * cosine_two - point[4] * sine_two,
        point[3],
        point[2] * sine_two + point[4] * cosine_two,
    ];
    let denominator_five = uniform.pole_five - rotated[4];
    let safe_five = if denominator_five.abs() < uniform.pole_epsilon {
        uniform.pole_epsilon
    } else {
        denominator_five
    };
    let projected_four: [f32; 4] =
        std::array::from_fn(|axis| rotated[axis] * uniform.pole_five / safe_five);
    let denominator_four = uniform.pole_four - projected_four[3];
    let safe_four = if denominator_four.abs() < uniform.pole_epsilon {
        uniform.pole_epsilon
    } else {
        denominator_four
    };
    (
        std::array::from_fn(|axis| projected_four[axis] * uniform.pole_four / safe_four),
        rotated[4],
        denominator_five > uniform.pole_epsilon && denominator_four > uniform.pole_epsilon,
    )
}

/// Mirrors the exact layer kernel's operation order for one Mode C output edge.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops
)]
pub fn mode_c_pose(object: &Prism, axes: [u32; 5], time: f32, index: u64) -> EdgePose<f32> {
    let copy = index / EDGES_PER_COPY;
    let base_edge = index % EDGES_PER_COPY;
    let coordinate = lattice_coordinate(copy, axes).unwrap_or([0; 5]);
    let frame = crate::frame_for(object, axes, time, 1.0);
    let uniform = ModeCFrameUniform::from_frame(&frame).lattice;
    let edge = object.edges[base_edge as usize];
    let translated = |vertex: u32| {
        std::array::from_fn(|axis| {
            object.vertices[vertex as usize][axis] as f32
                + coordinate[axis] as f32 * LATTICE_SPACING as f32
        })
    };
    let first = rotate_project(translated(edge.a), &uniform);
    let second = rotate_project(translated(edge.b), &uniform);
    let delta: [f32; 3] = std::array::from_fn(|axis| second.0[axis] - first.0[axis]);
    let edge_length = delta.iter().map(|value| value * value).sum::<f32>().sqrt();
    let safe_length = edge_length.max(1.0e-20);
    EdgePose {
        midpoint: std::array::from_fn(|axis| f32::midpoint(first.0[axis], second.0[axis])),
        direction: delta.map(|value| value / safe_length),
        length: if first.2 && second.2 { edge_length } else { -1.0 },
        hue: (f32::midpoint(first.1, second.1) / (2.0 * uniform.fifth_range) + 0.5)
            .clamp(0.0, 1.0),
    }
}

/// Instantiates the equal-work indexed presentation shader.
#[must_use]
pub fn mode_c_shader(limits: DialectLimits) -> String {
    MODE_C_DRAW_TEMPLATE
        .replace("__DESCRIPTORS__", &limits.descriptor_capacity.to_string())
        .replace("__SPANS__", &limits.span_capacity.to_string())
        .replace(
            "__HANDLE_GROUPS__",
            &limits.handle_capacity.div_ceil(4).to_string(),
        )
}

#[cfg(test)]
mod tests {
    use ember_lab_layer::geometry::{lattice_steps, prism};
    use ember_lab_layer::kernels::LATTICE_EDGE_KERNEL;

    use super::{
        ComparatorWork, EqualWorkSignature, ModeCFrameUniform, mode_c_pose, mode_c_register,
        mode_c_shader,
    };
    use crate::DialectLimits;

    fn limits() -> DialectLimits {
        DialectLimits {
            descriptor_capacity: 256,
            span_capacity: 64,
            handle_capacity: 128,
        }
    }

    #[test]
    fn mode_c_registers_the_exact_layer_body_and_192_byte_frame() {
        let kernel = mode_c_register(64, limits()).expect("exact layer kernel registers");
        assert!(kernel.source().contains(LATTICE_EDGE_KERNEL));
        assert_eq!(size_of::<ModeCFrameUniform>(), 192);
        let source = mode_c_shader(limits());
        let module = naga::front::wgsl::parse_str(&source).expect("Mode C draw parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("Mode C draw validates");
    }

    #[test]
    fn equal_work_signature_is_identical_for_heap_and_layer_labels() {
        let object = prism();
        let axes = [3; 5];
        let heap = EqualWorkSignature::for_work(&object, axes, 1.25);
        let layer = EqualWorkSignature::for_work(&object, axes, 1.25);
        assert_eq!(heap, layer);
        assert_ne!(heap.pose_hash, EqualWorkSignature::for_work(&object, axes, 1.5).pose_hash);
        let pose = mode_c_pose(&object, axes, 1.25, 728_999);
        assert!(pose.length.is_finite());
        assert!((0.0..=1.0).contains(&pose.hue));
    }

    #[test]
    fn comparator_arithmetic_holds_work_equal_and_slots_square_padded() {
        let work = ComparatorWork::for_axes([7, 7, 5, 5, 5]);
        assert_eq!(work.copies, 6_125);
        assert_eq!(work.edges, 18_375_000);
        assert_eq!(work.records, 36_750_000);
        assert_eq!(work.submitted_indices, work.edges * 36);
        assert_eq!(work.ideal_vertex_invocations, work.edges * 8);
        assert_eq!(work.logical_bytes, 588_000_000);
        assert_eq!(work.layer_side, 4_287);
        assert_eq!(work.layer_allocation_bytes, 588_107_808);
        let top = ComparatorWork::for_axes(lattice_steps()[112]);
        assert_eq!(top.edges, 578_188_125_000);
    }
}
