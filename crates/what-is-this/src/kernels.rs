//! Fixed diagnostic kernels shared by the browser exports and native checks.

// Frozen benchmark generators intentionally convert bounded indices and reference values.
#![allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
// Contraction and fixed-order probes require the unfused expressions under test.
#![allow(clippy::suboptimal_flops)]

use std::hint::black_box;

use ember_game_what_is_this_v1::{FmaProbe, TranscendentalObservation};
use faer::linalg::{matmul::matmul, solvers::Solve};
use faer::{Accum, Mat, Par, Side};
use serde::Serialize;

const WARMUP_RUNS: u16 = 3;
const SAMPLE_RUNS: u16 = 15;
const PROJECTION_REPEATS: usize = 64;
const CHOLESKY_REPEATS: usize = 1_024;
const SPIN_REPEATS: usize = 8_192;
const TRANSCENDENTAL_ITERATIONS: usize = 65_536;
const MEMCPY_BYTES_PER_RUN: usize = 16 * 1_024 * 1_024;

/// Stable metadata for one timed browser kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct KernelSpec {
    /// Stable versioned kernel identifier.
    pub kernel_id: &'static str,
    /// Exact work performed by one timed invocation.
    pub workload: &'static str,
    /// Unit recorded by the browser around the invocation.
    pub unit: &'static str,
    /// Number of untimed invocations before sampling.
    pub warmup_runs: u16,
    /// Number of timed observations in the report.
    pub sample_runs: u16,
}

const KERNEL_SPECS: [KernelSpec; 17] = [
    KernelSpec {
        kernel_id: "cpu.rank4-soa.n16.v1",
        workload: "manual f64 SoA 4xN projection; N=16; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-soa.n64.v1",
        workload: "manual f64 SoA 4xN projection; N=64; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-soa.n256.v1",
        workload: "manual f64 SoA 4xN projection; N=256; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-soa.n1024.v1",
        workload: "manual f64 SoA 4xN projection; N=1024; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-faer-seq.n16.v1",
        workload: "faer 0.24.4 prepacked f64 4xN matmul with Par::Seq; N=16; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-faer-seq.n64.v1",
        workload: "faer 0.24.4 prepacked f64 4xN matmul with Par::Seq; N=64; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-faer-seq.n256.v1",
        workload: "faer 0.24.4 prepacked f64 4xN matmul with Par::Seq; N=256; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.rank4-faer-seq.n1024.v1",
        workload: "faer 0.24.4 prepacked f64 4xN matmul with Par::Seq; N=1024; 64 batches per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.cholesky6-fixed.v1",
        workload: "fixed f64 6x6 Cholesky factor-and-solve; 1024 solves per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.cholesky6-faer-llt-seq.v1",
        workload: "faer 0.24.4 f64 6x6 LLT factor-and-solve with sequential storage; 1024 solves per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.spin4-pair-fixed.v1",
        workload: "two direct f64 quaternion products forming one Spin(4) rotor pair; 8192 pairs per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.spin4-pair-faer-seq.v1",
        workload: "two faer 0.24.4 preallocated f64 4x4-by-4x1 products with Par::Seq; 8192 pairs per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.f32-transcendentals.v1",
        workload: "dependent f32 sin/cos/sqrt throughput loop; 65536 iterations per sample",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.memcpy.4k.v1",
        workload: "copy_from_slice sweep at 4096 bytes; 16 MiB transferred per sample",
        unit: "MiB/s",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.memcpy.64k.v1",
        workload: "copy_from_slice sweep at 65536 bytes; 16 MiB transferred per sample",
        unit: "MiB/s",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.memcpy.1m.v1",
        workload: "copy_from_slice sweep at 1048576 bytes; 16 MiB transferred per sample",
        unit: "MiB/s",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    KernelSpec {
        kernel_id: "cpu.memcpy.4m.v1",
        workload: "copy_from_slice sweep at 4194304 bytes; 16 MiB transferred per sample",
        unit: "MiB/s",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
];

/// Returns the complete stable CPU-kernel inventory in report order.
#[must_use]
pub const fn kernel_specs() -> &'static [KernelSpec] {
    &KERNEL_SPECS
}

/// Runs one short arithmetic chunk used to hold the main thread for a controlled duration.
#[must_use]
pub fn jank_chunk() -> f64 {
    let mut value = 0.375_f64;
    for index in 0..2_048 {
        value = (value * 1.000_000_119 + f64::from(index) * 0.000_000_031).sqrt();
    }
    black_box(value)
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn sample(seed: u64) -> f64 {
    let mantissa = splitmix64(seed) >> 11;
    mantissa as f64 * (1.0 / (1_u64 << 53) as f64) - 0.5
}

struct ProjectionKernel<const N: usize> {
    projection: [[f64; 4]; 4],
    points: [[f64; N]; 4],
    manual_out: [[f64; N]; 4],
    faer_projection: Mat<f64>,
    faer_points: Mat<f64>,
    faer_out: Mat<f64>,
}

impl<const N: usize> ProjectionKernel<N> {
    fn new() -> Self {
        let projection = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                if row == column {
                    0.75 + sample(0x5000 + (row * 4 + column) as u64) * 0.1
                } else {
                    sample(0x5000 + (row * 4 + column) as u64) * 0.2
                }
            })
        });
        let points = std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let value = splitmix64(0x6000 + (column * 4 + row) as u64) % 4_097;
                value as f64 - 2_048.0
            })
        });
        let faer_projection = Mat::from_fn(4, 4, |row, column| projection[row][column]);
        let faer_points = Mat::from_fn(4, N, |row, column| points[row][column]);
        Self {
            projection,
            points,
            manual_out: [[0.0; N]; 4],
            faer_projection,
            faer_points,
            faer_out: Mat::zeros(4, N),
        }
    }

    fn run_manual(&mut self) -> f64 {
        for _ in 0..PROJECTION_REPEATS {
            for (output_row, projection_row) in self.manual_out.iter_mut().zip(&self.projection) {
                for (column, output) in output_row.iter_mut().enumerate() {
                    *output = projection_row
                        .iter()
                        .zip(&self.points)
                        .map(|(coefficient, point_row)| coefficient * point_row[column])
                        .sum();
                }
            }
        }
        black_box(self.manual_out[0][N - 1])
    }

    fn run_faer(&mut self) -> f64 {
        for _ in 0..PROJECTION_REPEATS {
            matmul(
                self.faer_out.as_mut(),
                Accum::Replace,
                self.faer_projection.as_ref(),
                self.faer_points.as_ref(),
                1.0,
                Par::Seq,
            );
        }
        black_box(self.faer_out[(0, N - 1)])
    }
}

fn make_spd6() -> [[f64; 6]; 6] {
    let mut lower = [[0.0; 6]; 6];
    for (row, values) in lower.iter_mut().enumerate() {
        for (column, value) in values.iter_mut().take(row + 1).enumerate() {
            *value = if row == column {
                2.0 + row as f64 * 0.125
            } else {
                sample(0x5eed + (row * 6 + column) as u64) * 0.3
            };
        }
    }
    let mut output = [[0.0; 6]; 6];
    for (row, output_row) in output.iter_mut().enumerate() {
        for (column, value) in output_row.iter_mut().enumerate() {
            *value = lower[row]
                .iter()
                .zip(&lower[column])
                .map(|(lhs, rhs)| lhs * rhs)
                .sum();
        }
    }
    output
}

fn fixed_cholesky_solve(matrix: &[[f64; 6]; 6], rhs: &[f64; 6]) -> [f64; 6] {
    let mut lower = [[0.0; 6]; 6];
    let mut row = 0;
    while row < 6 {
        let mut column = 0;
        while column <= row {
            let prior: f64 = (0..column)
                .map(|inner| lower[row][inner] * lower[column][inner])
                .sum();
            let value = matrix[row][column] - prior;
            lower[row][column] = if row == column {
                value.sqrt()
            } else {
                value / lower[column][column]
            };
            column += 1;
        }
        row += 1;
    }
    let mut intermediate = [0.0; 6];
    let mut row = 0;
    while row < 6 {
        let prior: f64 = (0..row)
            .map(|inner| lower[row][inner] * intermediate[inner])
            .sum();
        intermediate[row] = (rhs[row] - prior) / lower[row][row];
        row += 1;
    }
    let mut output = [0.0; 6];
    let mut row = 6;
    while row > 0 {
        row -= 1;
        let prior: f64 = ((row + 1)..6)
            .map(|inner| lower[inner][row] * output[inner])
            .sum();
        output[row] = (intermediate[row] - prior) / lower[row][row];
    }
    output
}

fn quaternion_product(lhs: [f64; 4], rhs: [f64; 4]) -> [f64; 4] {
    let [lx, ly, lz, lw] = lhs;
    let [rx, ry, rz, rw] = rhs;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

fn quaternion_left_matrix(quaternion: [f64; 4]) -> [[f64; 4]; 4] {
    let [x, y, z, w] = quaternion;
    [[w, -z, y, x], [z, w, -x, y], [-y, x, w, z], [-x, -y, -z, w]]
}

struct TinyKernels {
    spd: [[f64; 6]; 6],
    rhs: [f64; 6],
    faer_spd: Mat<f64>,
    faer_rhs: Mat<f64>,
    spin_values: [[f64; 4]; 4],
    faer_spin_lhs: [Mat<f64>; 2],
    faer_spin_rhs: [Mat<f64>; 2],
    faer_spin_out: [Mat<f64>; 2],
}

impl TinyKernels {
    fn new() -> Self {
        let spd = make_spd6();
        let rhs = std::array::from_fn(|index| sample(0x1234 + index as u64));
        let spin_values = [
            [0.21, -0.33, 0.14, 0.91],
            [-0.11, 0.28, 0.37, 0.87],
            [-0.19, 0.09, 0.31, 0.92],
            [0.24, 0.17, -0.22, 0.93],
        ];
        Self {
            faer_spd: Mat::from_fn(6, 6, |row, column| spd[row][column]),
            faer_rhs: Mat::from_fn(6, 1, |row, _| rhs[row]),
            spd,
            rhs,
            spin_values,
            faer_spin_lhs: [
                Mat::from_fn(4, 4, |row, column| {
                    quaternion_left_matrix(spin_values[0])[row][column]
                }),
                Mat::from_fn(4, 4, |row, column| {
                    quaternion_left_matrix(spin_values[2])[row][column]
                }),
            ],
            faer_spin_rhs: [
                Mat::from_fn(4, 1, |row, _| spin_values[1][row]),
                Mat::from_fn(4, 1, |row, _| spin_values[3][row]),
            ],
            faer_spin_out: [Mat::zeros(4, 1), Mat::zeros(4, 1)],
        }
    }

    fn run_fixed_cholesky(&self) -> f64 {
        let mut checksum = 0.0;
        for _ in 0..CHOLESKY_REPEATS {
            checksum += fixed_cholesky_solve(black_box(&self.spd), black_box(&self.rhs))[0];
        }
        black_box(checksum)
    }

    fn run_faer_cholesky(&self) -> f64 {
        let mut checksum = 0.0;
        for _ in 0..CHOLESKY_REPEATS {
            let Ok(factor) = black_box(self.faer_spd.as_ref()).llt(Side::Lower) else {
                return f64::NAN;
            };
            checksum += factor.solve(black_box(self.faer_rhs.as_ref()))[(0, 0)];
        }
        black_box(checksum)
    }

    fn run_fixed_spin(&self) -> f64 {
        let mut checksum = 0.0;
        for _ in 0..SPIN_REPEATS {
            checksum += quaternion_product(
                black_box(self.spin_values[0]),
                black_box(self.spin_values[1]),
            )[0];
            checksum += quaternion_product(
                black_box(self.spin_values[2]),
                black_box(self.spin_values[3]),
            )[0];
        }
        black_box(checksum)
    }

    fn run_faer_spin(&mut self) -> f64 {
        for _ in 0..SPIN_REPEATS {
            for ((output, lhs), rhs) in self
                .faer_spin_out
                .iter_mut()
                .zip(&self.faer_spin_lhs)
                .zip(&self.faer_spin_rhs)
            {
                matmul(
                    output.as_mut(),
                    Accum::Replace,
                    lhs.as_ref(),
                    rhs.as_ref(),
                    1.0,
                    Par::Seq,
                );
            }
        }
        black_box(self.faer_spin_out[0][(0, 0)] + self.faer_spin_out[1][(0, 0)])
    }
}

struct MemcpyKernel {
    source: Vec<u8>,
    destination: Vec<u8>,
}

impl MemcpyKernel {
    fn new(size: usize) -> Self {
        let source = (0..size)
            .map(|index| splitmix64(index as u64).to_le_bytes()[0])
            .collect();
        Self {
            source,
            destination: vec![0; size],
        }
    }

    fn run(&mut self) -> f64 {
        let copies = MEMCPY_BYTES_PER_RUN / self.source.len();
        for _ in 0..copies {
            self.destination.copy_from_slice(black_box(&self.source));
        }
        f64::from(black_box(self.destination[self.destination.len() / 2]))
    }
}

/// Preallocated state for every fixed CPU kernel.
pub struct KernelSuite {
    rank16: ProjectionKernel<16>,
    rank64: ProjectionKernel<64>,
    rank256: ProjectionKernel<256>,
    rank1024: ProjectionKernel<1_024>,
    tiny: TinyKernels,
    memcpy4k: MemcpyKernel,
    memcpy64k: MemcpyKernel,
    memcpy1m: MemcpyKernel,
    memcpy4m: MemcpyKernel,
}

impl Default for KernelSuite {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelSuite {
    /// Builds deterministic inputs and reusable output buffers outside timed samples.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rank16: ProjectionKernel::new(),
            rank64: ProjectionKernel::new(),
            rank256: ProjectionKernel::new(),
            rank1024: ProjectionKernel::new(),
            tiny: TinyKernels::new(),
            memcpy4k: MemcpyKernel::new(4 * 1_024),
            memcpy64k: MemcpyKernel::new(64 * 1_024),
            memcpy1m: MemcpyKernel::new(1_024 * 1_024),
            memcpy4m: MemcpyKernel::new(4 * 1_024 * 1_024),
        }
    }

    /// Runs one exact workload by stable kernel identifier and returns an opaque checksum.
    ///
    /// # Errors
    ///
    /// Returns an error for an identifier outside [`kernel_specs`].
    pub fn run(&mut self, kernel_id: &str) -> Result<f64, String> {
        let checksum = match kernel_id {
            "cpu.rank4-soa.n16.v1" => self.rank16.run_manual(),
            "cpu.rank4-soa.n64.v1" => self.rank64.run_manual(),
            "cpu.rank4-soa.n256.v1" => self.rank256.run_manual(),
            "cpu.rank4-soa.n1024.v1" => self.rank1024.run_manual(),
            "cpu.rank4-faer-seq.n16.v1" => self.rank16.run_faer(),
            "cpu.rank4-faer-seq.n64.v1" => self.rank64.run_faer(),
            "cpu.rank4-faer-seq.n256.v1" => self.rank256.run_faer(),
            "cpu.rank4-faer-seq.n1024.v1" => self.rank1024.run_faer(),
            "cpu.cholesky6-fixed.v1" => self.tiny.run_fixed_cholesky(),
            "cpu.cholesky6-faer-llt-seq.v1" => self.tiny.run_faer_cholesky(),
            "cpu.spin4-pair-fixed.v1" => self.tiny.run_fixed_spin(),
            "cpu.spin4-pair-faer-seq.v1" => self.tiny.run_faer_spin(),
            "cpu.f32-transcendentals.v1" => run_transcendental_throughput(),
            "cpu.memcpy.4k.v1" => self.memcpy4k.run(),
            "cpu.memcpy.64k.v1" => self.memcpy64k.run(),
            "cpu.memcpy.1m.v1" => self.memcpy1m.run(),
            "cpu.memcpy.4m.v1" => self.memcpy4m.run(),
            _ => return Err(format!("unknown diagnostic kernel id: {kernel_id}")),
        };
        Ok(checksum)
    }
}

fn run_transcendental_throughput() -> f64 {
    let mut value = 0.125_f32;
    for index in 0..TRANSCENDENTAL_ITERATIONS {
        let phase = index as f32 * 0.000_031_25;
        value = (value + phase).sin() + (value - phase).cos() + (value.abs() + 1.0).sqrt();
        value *= 0.25;
    }
    f64::from(black_box(value))
}

/// Complete scalar contraction and transcendental fingerprint.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct FloatProbeResult {
    /// Multiply-add contraction observation.
    pub fma: FmaProbe,
    /// Frozen input table with baked f64 references and observed f32 bits.
    pub transcendentals: Vec<TranscendentalObservation>,
}

impl FloatProbeResult {
    /// Runs the fixed scalar fingerprint without using browser or user-agent information.
    #[must_use]
    pub fn measure() -> Self {
        let multiplicand = black_box(f32::from_bits(0x3f80_0001));
        let multiplier = black_box(f32::from_bits(0x3f80_0001));
        let addend = black_box(f32::from_bits(0xbf80_0002));
        let plain = multiplicand * multiplier + addend;
        let product = black_box(multiplicand * multiplier);
        let separated = product + addend;
        let fused = multiplicand.mul_add(multiplier, addend);
        let references = [
            (
                0xc47a_0000,
                -0.826_879_540_532_002_5,
                0.562_379_076_290_702_9,
            ),
            (
                0xc049_0fdb,
                8.742_278_000_372_475e-8,
                -0.999_999_999_999_996_2,
            ),
            (
                0xbf80_0000,
                -0.841_470_984_807_896_5,
                0.540_302_305_868_139_8,
            ),
            (
                0xbdcc_cccd,
                -0.099_833_418_129_499_9,
                0.995_004_165_129_262_4,
            ),
            (0x0000_0000, 0.0, 1.0),
            (0x0080_0000, 1.175_494_350_822_287_5e-38, 1.0),
            (
                0x3dcc_cccd,
                0.099_833_418_129_499_9,
                0.995_004_165_129_262_4,
            ),
            (
                0x3f80_0000,
                0.841_470_984_807_896_5,
                0.540_302_305_868_139_8,
            ),
            (
                0x3fc9_0fdb,
                0.999_999_999_999_999,
                -4.371_139_000_186_241e-8,
            ),
            (
                0x4049_0fdb,
                -8.742_278_000_372_475e-8,
                -0.999_999_999_999_996_2,
            ),
            (
                0x447a_0000,
                0.826_879_540_532_002_5,
                0.562_379_076_290_702_9,
            ),
        ];
        let transcendentals = references
            .into_iter()
            .map(|(input_bits, sin_reference_f64, cos_reference_f64)| {
                let input = f32::from_bits(input_bits);
                let sin_observed_bits = input.sin().to_bits();
                let cos_observed_bits = input.cos().to_bits();
                TranscendentalObservation {
                    input_bits,
                    sin_reference_f64,
                    sin_observed_bits,
                    sin_ulp: ulp_distance(sin_observed_bits, (sin_reference_f64 as f32).to_bits()),
                    cos_reference_f64,
                    cos_observed_bits,
                    cos_ulp: ulp_distance(cos_observed_bits, (cos_reference_f64 as f32).to_bits()),
                }
            })
            .collect();
        Self {
            fma: FmaProbe {
                plain_result_bits: plain.to_bits(),
                separated_result_bits: separated.to_bits(),
                fused_result_bits: fused.to_bits(),
                contracts_to_fma: plain.to_bits() == fused.to_bits()
                    && fused.to_bits() != separated.to_bits(),
            },
            transcendentals,
        }
    }
}

const fn ordered_float_bits(bits: u32) -> u32 {
    if bits & 0x8000_0000 == 0 {
        bits | 0x8000_0000
    } else {
        !bits
    }
}

const fn ulp_distance(lhs: u32, rhs: u32) -> u32 {
    ordered_float_bits(lhs).abs_diff(ordered_float_bits(rhs))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn kernel_ids_are_unique_and_versioned() {
        let ids: BTreeSet<_> = kernel_specs().iter().map(|spec| spec.kernel_id).collect();
        assert_eq!(ids.len(), kernel_specs().len());
        assert!(ids.iter().all(|kernel_id| {
            kernel_id
                .rsplit_once('.')
                .is_some_and(|(_, version)| version == "v1")
        }));
    }

    #[test]
    fn classic_fma_probe_separates_fused_rounding() {
        let result = FloatProbeResult::measure();
        assert_ne!(
            result.fma.fused_result_bits,
            result.fma.separated_result_bits
        );
        assert_eq!(result.transcendentals.len(), 11);
    }
}
