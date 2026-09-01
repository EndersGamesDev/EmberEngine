//! Exact construction and reference math for the dodecahedral prism.

/// One edge, stored as endpoint vertex indices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    /// First endpoint.
    pub a: u32,
    /// Second endpoint.
    pub b: u32,
}

/// The fully derived 120-cell prism.
#[derive(Clone, Debug)]
pub struct Prism {
    /// Five-dimensional vertex coordinates.
    pub vertices: Vec<[f64; 5]>,
    /// All in-cap edges and struts.
    pub edges: Vec<Edge>,
    /// Edge length derived from the minimum cap-vertex separation.
    pub edge_length: f64,
    /// Common four-dimensional cap circumradius.
    pub cap_circumradius: f64,
}

/// Edge transform emitted by the second kernel.
#[derive(Clone, Copy, Debug)]
pub struct EdgePose<T> {
    /// Projected midpoint.
    pub midpoint: [T; 3],
    /// Unit direction used as an orientation axis.
    pub direction: [T; 3],
    /// Projected edge length.
    pub length: T,
    /// Fifth-axis hue coordinate in zero-to-one range.
    pub hue: T,
}

fn permutations() -> Vec<[usize; 4]> {
    let mut result = Vec::with_capacity(24);
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if a != b && a != c && a != d && b != c && b != d && c != d {
                        result.push([a, b, c, d]);
                    }
                }
            }
        }
    }
    result
}

fn even(permutation: [usize; 4]) -> bool {
    let mut inversions = 0;
    for left in 0..4 {
        for right in left + 1..4 {
            inversions += usize::from(permutation[left] > permutation[right]);
        }
    }
    inversions % 2 == 0
}

fn add_family(vertices: &mut Vec<[f64; 4]>, base: [f64; 4], even_only: bool) {
    for permutation in permutations() {
        if even_only && !even(permutation) {
            continue;
        }
        let mut permuted = [0.0; 4];
        for (destination, source) in permutation.into_iter().enumerate() {
            permuted[destination] = base[source];
        }
        let nonzero = permuted.iter().filter(|value| **value != 0.0).count();
        for signs in 0..(1_usize << nonzero) {
            let mut point = permuted;
            let mut sign_index = 0;
            for coordinate in &mut point {
                if *coordinate != 0.0 {
                    if signs & (1 << sign_index) != 0 {
                        *coordinate = -*coordinate;
                    }
                    sign_index += 1;
                }
            }
            if !vertices.contains(&point) {
                vertices.push(point);
            }
        }
    }
}

/// Constructs the 600 vertices of a regular 120-cell from its seven coordinate families.
#[must_use]
pub fn cap_vertices() -> Vec<[f64; 4]> {
    let sqrt5 = 5.0_f64.sqrt();
    let phi = f64::midpoint(1.0, sqrt5);
    let mut vertices = Vec::with_capacity(600);
    add_family(&mut vertices, [0.0, 0.0, 2.0, 2.0], false);
    add_family(&mut vertices, [1.0, 1.0, 1.0, sqrt5], false);
    add_family(&mut vertices, [phi.powi(-2), phi, phi, phi], false);
    add_family(
        &mut vertices,
        [phi.recip(), phi.recip(), phi.recip(), phi.powi(2)],
        false,
    );
    add_family(&mut vertices, [0.0, phi.powi(-2), 1.0, phi.powi(2)], true);
    add_family(&mut vertices, [0.0, phi.recip(), phi, sqrt5], true);
    add_family(&mut vertices, [phi.recip(), 1.0, phi, 2.0], true);
    vertices
}

fn squared_distance(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| (a - b) * (a - b)).sum()
}

fn derive_cap_edges(vertices: &[[f64; 4]]) -> (f64, Vec<Edge>) {
    let mut minimum = f64::INFINITY;
    for (left_index, left) in vertices.iter().enumerate() {
        for right in &vertices[left_index + 1..] {
            minimum = minimum.min(squared_distance(left, right));
        }
    }
    let tolerance = minimum * 1.0e-10;
    let mut edges = Vec::new();
    for (left_index, left) in vertices.iter().enumerate() {
        for (right_index, right) in vertices.iter().enumerate().skip(left_index + 1) {
            if (squared_distance(left, right) - minimum).abs() <= tolerance {
                if let (Ok(a), Ok(b)) = (u32::try_from(left_index), u32::try_from(right_index)) {
                    edges.push(Edge { a, b });
                }
            }
        }
    }
    (minimum.sqrt(), edges)
}

/// Derives both caps, all minimum-distance cap edges, and the vertex-to-vertex struts.
#[must_use]
pub fn prism() -> Prism {
    let cap = cap_vertices();
    let (edge_length, cap_edges) = derive_cap_edges(&cap);
    let half_strut = edge_length * 0.5;
    let mut vertices = Vec::with_capacity(1_200);
    vertices.extend(
        cap.iter()
            .map(|point| [point[0], point[1], point[2], point[3], -half_strut]),
    );
    vertices.extend(
        cap.iter()
            .map(|point| [point[0], point[1], point[2], point[3], half_strut]),
    );
    let mut edges = Vec::with_capacity(3_000);
    edges.extend(cap_edges.iter().copied());
    edges.extend(cap_edges.iter().map(|edge| Edge {
        a: edge.a + 600,
        b: edge.b + 600,
    }));
    edges.extend((0..600).map(|index| Edge {
        a: index,
        b: index + 600,
    }));
    let cap_circumradius = cap[0].iter().map(|value| value * value).sum::<f64>().sqrt();
    Prism {
        vertices,
        edges,
        edge_length,
        cap_circumradius,
    }
}

fn rotate(point: [f64; 5], time: f64) -> [f64; 5] {
    let theta_one = 0.4 * time;
    let theta_two = f64::midpoint(1.0, 5.0_f64.sqrt()) * theta_one;
    let (sin_one, cos_one) = theta_one.sin_cos();
    let (sin_two, cos_two) = theta_two.sin_cos();
    [
        point[1].mul_add(-sin_one, point[0] * cos_one),
        point[1].mul_add(cos_one, point[0] * sin_one),
        point[4].mul_add(-sin_two, point[2] * cos_two),
        point[3],
        point[4].mul_add(cos_two, point[2] * sin_two),
    ]
}

/// Applies the specified SO(5) rotation and the two perspective projections in `f64`.
#[must_use]
pub fn project_reference(point: [f64; 5], time: f64) -> ([f64; 3], f64) {
    let rotated = rotate(point, time);
    let scale_five = 8.0 / (8.0 - rotated[4]);
    let point_four = [
        rotated[0] * scale_five,
        rotated[1] * scale_five,
        rotated[2] * scale_five,
        rotated[3] * scale_five,
    ];
    let scale_four = 8.0 / (8.0 - point_four[3]);
    (
        [
            point_four[0] * scale_four,
            point_four[1] * scale_four,
            point_four[2] * scale_four,
        ],
        rotated[4],
    )
}

/// Mirrors the two WGSL kernels' `f32` arithmetic for native conformance tests.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::suboptimal_flops)]
pub fn project_gpu_path(point: [f64; 5], time: f64) -> ([f32; 3], f32) {
    let point = point.map(|value| value as f32);
    let theta_one = 0.4 * time as f32;
    let theta_two = f32::midpoint(1.0, 5.0_f32.sqrt()) * theta_one;
    let (sin_one, cos_one) = theta_one.sin_cos();
    let (sin_two, cos_two) = theta_two.sin_cos();
    let rotated = [
        point[0] * cos_one - point[1] * sin_one,
        point[0] * sin_one + point[1] * cos_one,
        point[2] * cos_two - point[4] * sin_two,
        point[3],
        point[2] * sin_two + point[4] * cos_two,
    ];
    let scale_five = 8.0 / (8.0 - rotated[4]);
    let point_four = [
        rotated[0] * scale_five,
        rotated[1] * scale_five,
        rotated[2] * scale_five,
        rotated[3] * scale_five,
    ];
    let scale_four = 8.0 / (8.0 - point_four[3]);
    (
        [
            point_four[0] * scale_four,
            point_four[1] * scale_four,
            point_four[2] * scale_four,
        ],
        rotated[4],
    )
}

/// Computes the `f64` reference edge transform and fixed-range fifth-axis hue.
#[must_use]
pub fn edge_reference(first: ([f64; 3], f64), second: ([f64; 3], f64)) -> EdgePose<f64> {
    let delta = [
        second.0[0] - first.0[0],
        second.0[1] - first.0[1],
        second.0[2] - first.0[2],
    ];
    let length = delta.iter().map(|value| value * value).sum::<f64>().sqrt();
    EdgePose {
        midpoint: [
            f64::midpoint(first.0[0], second.0[0]),
            f64::midpoint(first.0[1], second.0[1]),
            f64::midpoint(first.0[2], second.0[2]),
        ],
        direction: delta.map(|value| value / length),
        length,
        hue: (f64::midpoint(first.1, second.1) / 6.0 + 0.5).clamp(0.0, 1.0),
    }
}

/// Mirrors the edge WGSL's `f32` transform and hue arithmetic.
#[must_use]
pub fn edge_gpu_path(first: ([f32; 3], f32), second: ([f32; 3], f32)) -> EdgePose<f32> {
    let delta = [
        second.0[0] - first.0[0],
        second.0[1] - first.0[1],
        second.0[2] - first.0[2],
    ];
    let length = delta.iter().map(|value| value * value).sum::<f32>().sqrt();
    EdgePose {
        midpoint: [
            f32::midpoint(first.0[0], second.0[0]),
            f32::midpoint(first.0[1], second.0[1]),
            f32::midpoint(first.0[2], second.0[2]),
        ],
        direction: delta.map(|value| value / length),
        length,
        hue: (f32::midpoint(first.1, second.1) / 6.0 + 0.5).clamp(0.0, 1.0),
    }
}

/// Checks the construction invariants and both projection poles.
///
/// # Panics
///
/// Panics only if deterministic construction or reference arithmetic violates the charter.
pub fn assert_invariants() {
    let cap = cap_vertices();
    assert_eq!(cap.len(), 600, "120-cell cap vertex count");
    let prism = prism();
    assert_eq!(prism.vertices.len(), 1_200, "prism vertex count");
    assert_eq!(prism.edges.len(), 3_000, "prism edge count");
    let expected_length = 3.0 - 5.0_f64.sqrt();
    assert!((prism.edge_length - expected_length).abs() <= 1.0e-12);
    for edge in &prism.edges {
        let length = squared_distance(
            &prism.vertices[edge.a as usize],
            &prism.vertices[edge.b as usize],
        )
        .sqrt();
        assert!((length - prism.edge_length).abs() <= prism.edge_length * 1.0e-9);
    }
    for point in &cap {
        let radius = point.iter().map(|value| value * value).sum::<f64>().sqrt();
        assert!((radius - prism.cap_circumradius).abs() <= 1.0e-12);
    }
    let maximum_fifth_reach = prism
        .vertices
        .iter()
        .map(|point| point[2].hypot(point[4]))
        .fold(0.0_f64, f64::max);
    let maximum_fourth_projection = prism
        .vertices
        .iter()
        .map(|point| point[3].abs() * 8.0 / (8.0 - maximum_fifth_reach))
        .fold(0.0_f64, f64::max);
    assert!(
        maximum_fifth_reach < 8.0,
        "5D projection pole intersects object reach"
    );
    assert!(
        maximum_fourth_projection < 8.0,
        "4D projection pole intersects object reach"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_600_vertices_1200_cap_edges_3000_prism_edges_length_0p7639320225002102() {
        assert_invariants();
        let cap = cap_vertices();
        let (_, edges) = derive_cap_edges(&cap);
        assert_eq!(edges.len(), 1_200);
    }

    #[test]
    fn gpu_path_math_matches_f64_reference_within_4e_5() {
        let object = prism();
        for time in [0.0, 0.37, 2.5, 19.0] {
            for point in &object.vertices {
                let (reference, fifth) = project_reference(*point, time);
                let (gpu, gpu_fifth) = project_gpu_path(*point, time);
                for axis in 0..3 {
                    assert!((reference[axis] - f64::from(gpu[axis])).abs() <= 4.0e-5);
                }
                assert!((fifth - f64::from(gpu_fifth)).abs() <= 4.0e-5);
            }
            for edge in &object.edges {
                let first = project_reference(object.vertices[edge.a as usize], time);
                let second = project_reference(object.vertices[edge.b as usize], time);
                let gpu_first = project_gpu_path(object.vertices[edge.a as usize], time);
                let gpu_second = project_gpu_path(object.vertices[edge.b as usize], time);
                let reference = edge_reference(first, second);
                let gpu = edge_gpu_path(gpu_first, gpu_second);
                for axis in 0..3 {
                    assert!(
                        (reference.midpoint[axis] - f64::from(gpu.midpoint[axis])).abs() <= 4.0e-5
                    );
                    assert!(
                        (reference.direction[axis] - f64::from(gpu.direction[axis])).abs()
                            <= 4.0e-5
                    );
                }
                assert!((reference.length - f64::from(gpu.length)).abs() <= 4.0e-5);
                assert!((reference.hue - f64::from(gpu.hue)).abs() <= 4.0e-5);
            }
        }
    }
}
