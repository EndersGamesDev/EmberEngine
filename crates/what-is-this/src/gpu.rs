//! Compute-only WebGPU diagnostics with no surface or renderer dependency.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wgpu::util::DeviceExt;

const WARMUP_RUNS: u16 = 3;
const SAMPLE_RUNS: u16 = 15;
const PROJECTION_BATCHES: u32 = 64;
const BANDWIDTH_ELEMENTS: usize = 4 * 1_024 * 1_024 / size_of::<f32>();
const BANDWIDTH_DISPATCHES: u32 = 4;
const DISPATCH_ROUND_TRIPS: u32 = 8;
const MAX_REPEAT_COUNT: u32 = 4_096;

const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read> input_data: array<f32>;
@group(0) @binding(1) var<storage, read_write> output_data: array<f32>;

@compute @workgroup_size(64)
fn project(@builtin(global_invocation_id) gid: vec3<u32>) {
    let n = arrayLength(&input_data) / 4u;
    let i = gid.x;
    if (i >= n) { return; }
    let x = input_data[i];
    let y = input_data[n + i];
    let z = input_data[2u * n + i];
    let w = input_data[3u * n + i];
    output_data[i] = 0.75 * x + 0.10 * y - 0.05 * z + 0.02 * w;
    output_data[n + i] = -0.20 * x + 0.80 * y + 0.07 * z + 0.03 * w;
    output_data[2u * n + i] = 0.04 * x - 0.06 * y + 0.90 * z + 0.11 * w;
    output_data[3u * n + i] = 0.01 * x + 0.05 * y - 0.08 * z + 0.95 * w;
}

@compute @workgroup_size(256)
fn copy_storage(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i < arrayLength(&input_data)) { output_data[i] = input_data[i]; }
}

@compute @workgroup_size(1)
fn tiny_dispatch() {
    output_data[0] = input_data[0] * 2.0 + 1.0;
}
"#;

/// Stable metadata for one timed WebGPU compute workload.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct GpuKernelSpec {
    /// Stable versioned kernel identifier.
    pub(crate) kernel_id: &'static str,
    /// Exact work represented by one normalized base invocation.
    pub(crate) workload: &'static str,
    /// Unit recorded in the report.
    pub(crate) unit: &'static str,
    /// Number of untimed warmups, including the first dispatch and compilation cost.
    pub(crate) warmup_runs: u16,
    /// Number of timed observations.
    pub(crate) sample_runs: u16,
}

const GPU_KERNEL_SPECS: [GpuKernelSpec; 4] = [
    GpuKernelSpec {
        kernel_id: "gpu.compute-rank4-soa.n256.v1",
        workload: "WebGPU f32 SoA 4xN projection; N=256; 64 dispatches per base invocation; same shape and batch count as cpu.rank4-soa.n256.v2 but different precision and execution substrate",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    GpuKernelSpec {
        kernel_id: "gpu.compute-rank4-soa.n1024.v1",
        workload: "WebGPU f32 SoA 4xN projection; N=1024; 64 dispatches per base invocation; same shape and batch count as cpu.rank4-soa.n1024.v2 but different precision and execution substrate",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    GpuKernelSpec {
        kernel_id: "gpu.storage-copy.4m.v1",
        workload: "WebGPU storage-buffer copy at 4194304 bytes; 4 dispatches and 16 MiB copied per base invocation",
        unit: "MiB/s",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
    GpuKernelSpec {
        kernel_id: "gpu.dispatch-roundtrip.tiny.v1",
        workload: "8 sequential one-workgroup submit-to-completion round trips per base invocation; reported samples normalize to one round trip; a timestamp query, when available, separates the first shader dispatch without replacing the wall-clock measurement",
        unit: "ms",
        warmup_runs: WARMUP_RUNS,
        sample_runs: SAMPLE_RUNS,
    },
];

/// Returns the stable timed WebGPU kernel inventory.
#[must_use]
pub(crate) const fn kernel_specs() -> &'static [GpuKernelSpec] {
    &GPU_KERNEL_SPECS
}

#[derive(Serialize)]
struct AdapterFacts {
    kernel_id: &'static str,
    adapter_identity: String,
    name: String,
    vendor: String,
    architecture: String,
    device_class: String,
    backend: String,
    timestamp_query: bool,
    optional_features: Vec<&'static str>,
    all_exposed_features: String,
    max_buffer_size: u64,
    max_storage_buffer_binding_size: u32,
    max_storage_buffers_per_shader_stage: u32,
    max_compute_invocations_per_workgroup: u32,
    max_compute_workgroup_size_x: u32,
    max_compute_workgroups_per_dimension: u32,
}

#[derive(Serialize)]
struct GpuRunResult {
    checksum: f64,
    queue_elapsed_ms: f64,
    gpu_elapsed_ms: Option<f64>,
    timing_method: &'static str,
    dispatch_count: u32,
}

struct CallbackState<T> {
    value: Option<T>,
    waker: Option<Waker>,
    closed: bool,
}

struct CallbackFuture<T> {
    state: Arc<Mutex<CallbackState<T>>>,
}

impl<T> Future for CallbackFuture<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<T> {
        let Ok(mut state) = self.state.lock() else {
            return Poll::Pending;
        };
        if state.closed {
            return Poll::Pending;
        }
        if let Some(value) = state.value.take() {
            Poll::Ready(value)
        } else {
            state.waker = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

impl<T> Drop for CallbackFuture<T> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.value = None;
            state.waker = None;
        }
    }
}

fn callback_pair<T>() -> (CallbackFuture<T>, Arc<Mutex<CallbackState<T>>>) {
    let state = Arc::new(Mutex::new(CallbackState {
        value: None,
        waker: None,
        closed: false,
    }));
    (
        CallbackFuture {
            state: Arc::clone(&state),
        },
        state,
    )
}

fn finish_callback<T>(state: &Arc<Mutex<CallbackState<T>>>, value: T) {
    let waker = if let Ok(mut state) = state.lock() {
        if state.closed {
            return;
        }
        state.value = Some(value);
        state.waker.take()
    } else {
        None
    };
    if let Some(waker) = waker {
        waker.wake();
    }
}

fn performance_now() -> f64 {
    let global = js_sys::global();
    let Ok(performance) = js_sys::Reflect::get(&global, &JsValue::from_str("performance")) else {
        return js_sys::Date::now();
    };
    let Ok(now) = js_sys::Reflect::get(&performance, &JsValue::from_str("now")) else {
        return js_sys::Date::now();
    };
    let Some(now) = now.dyn_ref::<js_sys::Function>() else {
        return js_sys::Date::now();
    };
    now.call0(&performance)
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or_else(js_sys::Date::now)
}

fn navigator_has_gpu() -> bool {
    let global = js_sys::global();
    let Ok(navigator) = js_sys::Reflect::get(&global, &JsValue::from_str("navigator")) else {
        return false;
    };
    js_sys::Reflect::get(&navigator, &JsValue::from_str("gpu"))
        .is_ok_and(|gpu| !gpu.is_null() && !gpu.is_undefined())
}

fn projection_input(n: usize) -> Vec<f32> {
    let mut input = Vec::with_capacity(n * 4);
    for lane in 0..4 {
        for index in 0..n {
            input.push((lane as f32 + 1.0) * 0.125 + index as f32 * 0.000_125);
        }
    }
    input
}

fn projection_expected(input: &[f32]) -> Vec<f32> {
    let n = input.len() / 4;
    let mut output = vec![0.0; input.len()];
    for index in 0..n {
        let x = input[index];
        let y = input[n + index];
        let z = input[2 * n + index];
        let w = input[3 * n + index];
        output[index] = 0.75 * x + 0.10 * y - 0.05 * z + 0.02 * w;
        output[n + index] = -0.20 * x + 0.80 * y + 0.07 * z + 0.03 * w;
        output[2 * n + index] = 0.04 * x - 0.06 * y + 0.90 * z + 0.11 * w;
        output[3 * n + index] = 0.01 * x + 0.05 * y - 0.08 * z + 0.95 * w;
    }
    output
}

struct Workload {
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    completion_fence: wgpu::Buffer,
    expected: Vec<f32>,
    dispatch_x: u32,
    dispatches_per_base: u32,
}

impl Workload {
    fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        label: &'static str,
        entry_point: &'static str,
        input: &[f32],
        expected: Vec<f32>,
        dispatch_x: u32,
        dispatches_per_base: u32,
    ) -> Self {
        let input_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("what-is-this gpu input"),
            contents: bytemuck::cast_slice(input),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("what-is-this gpu output"),
            size: expected.len() as u64 * size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("what-is-this gpu validation readback"),
            size: expected.len() as u64 * size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let completion_fence = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("what-is-this gpu completion fence"),
            size: size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("what-is-this gpu compute bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("what-is-this gpu compute bind group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("what-is-this gpu compute pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            pipeline,
            bind_group,
            output,
            readback,
            completion_fence,
            expected,
            dispatch_x,
            dispatches_per_base,
        }
    }

    fn output_bytes(&self) -> u64 {
        self.expected.len() as u64 * size_of::<f32>() as u64
    }
}

struct TimestampResources {
    query_set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
}

impl TimestampResources {
    fn new(device: &wgpu::Device) -> Self {
        Self {
            query_set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("what-is-this gpu timestamps"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("what-is-this gpu timestamp resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("what-is-this gpu timestamp readback"),
                size: 16,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        }
    }
}

struct GpuSuite {
    device: wgpu::Device,
    queue: wgpu::Queue,
    lost: Arc<Mutex<Option<String>>>,
    timestamp: Option<TimestampResources>,
    projection_256: Workload,
    projection_1024: Workload,
    bandwidth: Workload,
    dispatch: Workload,
}

impl GpuSuite {
    async fn new() -> Result<(Self, AdapterFacts), String> {
        if !navigator_has_gpu() {
            return Err(
                "navigator.gpu is not exposed by this browser or browsing context".to_string(),
            );
        }
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| {
                "navigator.gpu was exposed, but requestAdapter returned no compatible compute adapter"
                    .to_string()
            })?;
        let info = adapter.get_info();
        let limits = adapter.limits();
        if limits.max_storage_buffer_binding_size < BANDWIDTH_ELEMENTS as u32 * 4 {
            return Err(format!(
                "adapter storage-buffer binding limit is {} bytes, below the fixed 4194304-byte bandwidth workload",
                limits.max_storage_buffer_binding_size
            ));
        }
        let supported = adapter.features();
        let timestamp_query = supported.contains(wgpu::Features::TIMESTAMP_QUERY);
        let required_features = if timestamp_query {
            wgpu::Features::TIMESTAMP_QUERY
        } else {
            wgpu::Features::empty()
        };
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("what-is-this compute device"),
                    required_features,
                    required_limits: wgpu::Limits::default().using_resolution(limits.clone()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await
            .map_err(|error| format!("requestDevice refused the compute device: {error}"))?;
        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!("WebGPU device lost ({reason:?}): {message}"));
            }
        });
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("what-is-this compute shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let input_256 = projection_input(256);
        let input_1024 = projection_input(1_024);
        let bandwidth_input: Vec<f32> = (0..BANDWIDTH_ELEMENTS)
            .map(|index| (index % 4_096) as f32 * 0.25 - 512.0)
            .collect();
        let projection_256 = Workload::new(
            &device,
            &shader,
            "what-is-this projection n256",
            "project",
            &input_256,
            projection_expected(&input_256),
            4,
            PROJECTION_BATCHES,
        );
        let projection_1024 = Workload::new(
            &device,
            &shader,
            "what-is-this projection n1024",
            "project",
            &input_1024,
            projection_expected(&input_1024),
            16,
            PROJECTION_BATCHES,
        );
        let bandwidth = Workload::new(
            &device,
            &shader,
            "what-is-this storage copy",
            "copy_storage",
            &bandwidth_input,
            bandwidth_input.clone(),
            (BANDWIDTH_ELEMENTS as u32).div_ceil(256),
            BANDWIDTH_DISPATCHES,
        );
        let dispatch = Workload::new(
            &device,
            &shader,
            "what-is-this tiny dispatch",
            "tiny_dispatch",
            &[1.125],
            vec![3.25],
            1,
            1,
        );
        if let Some(error) = device.pop_error_scope().await {
            return Err(format!(
                "WebGPU shader or pipeline validation failed during named compilation warmup: {error}"
            ));
        }
        let timestamp = timestamp_query.then(|| TimestampResources::new(&device));
        let optional_features = [
            (wgpu::Features::TIMESTAMP_QUERY, "timestamp-query"),
            (wgpu::Features::SHADER_F16, "shader-f16"),
            (wgpu::Features::SUBGROUP, "subgroups"),
        ]
        .into_iter()
        .filter_map(|(feature, name)| supported.contains(feature).then_some(name))
        .collect();
        let adapter_identity = if info.name.is_empty() {
            format!("vendor {:#06x}, {:?}", info.vendor, info.device_type)
        } else {
            format!("{} ({:?})", info.name, info.device_type)
        };
        let facts = AdapterFacts {
            kernel_id: "gpu.adapter-facts.v1",
            adapter_identity,
            name: if info.name.is_empty() {
                "not exposed".to_string()
            } else {
                info.name
            },
            vendor: if info.vendor == 0 {
                "not exposed".to_string()
            } else {
                format!("{:#06x}", info.vendor)
            },
            architecture: "not exposed by wgpu 24 AdapterInfo on the browser WebGPU backend"
                .to_string(),
            device_class: format!("{:?}", info.device_type),
            backend: format!("{:?}", info.backend),
            timestamp_query,
            optional_features,
            all_exposed_features: format!("{supported:?}"),
            max_buffer_size: limits.max_buffer_size,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
            max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        };
        Ok((
            Self {
                device,
                queue,
                lost,
                timestamp,
                projection_256,
                projection_1024,
                bandwidth,
                dispatch,
            },
            facts,
        ))
    }

    fn lost_reason(&self) -> Option<String> {
        self.lost.lock().ok().and_then(|reason| reason.clone())
    }

    fn workload(&self, kernel_id: &str) -> Result<&Workload, String> {
        match kernel_id {
            "gpu.compute-rank4-soa.n256.v1" => Ok(&self.projection_256),
            "gpu.compute-rank4-soa.n1024.v1" => Ok(&self.projection_1024),
            "gpu.storage-copy.4m.v1" => Ok(&self.bandwidth),
            "gpu.dispatch-roundtrip.tiny.v1" => Ok(&self.dispatch),
            _ => Err(format!("unknown WebGPU compute kernel id {kernel_id}")),
        }
    }

    async fn wait_for_completion(&self, workload: &Workload) -> Result<(), String> {
        // wgpu 24's browser-WebGPU backend leaves Queue::on_submitted_work_done unimplemented.
        // Mapping this four-byte copy is the equivalent supported completion fence: the copy is
        // ordered after the dispatch and any full validation copy in the same command buffer.
        Self::map_buffer(&workload.completion_fence).await?;
        self.lost_reason().map_or(Ok(()), Err)
    }

    async fn map_buffer(buffer: &wgpu::Buffer) -> Result<Vec<u8>, String> {
        let slice = buffer.slice(..);
        let (future, state) = callback_pair();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            finish_callback(&state, result);
        });
        future
            .await
            .map_err(|error| format!("WebGPU validation readback mapping failed: {error}"))?;
        let bytes = slice.get_mapped_range().to_vec();
        buffer.unmap();
        Ok(bytes)
    }

    fn validate_output(workload: &Workload, bytes: &[u8]) -> Result<f64, String> {
        let observed: &[f32] = bytemuck::try_cast_slice(bytes)
            .map_err(|error| format!("WebGPU readback had invalid f32 layout: {error}"))?;
        if observed.len() != workload.expected.len() {
            return Err(format!(
                "WebGPU readback returned {} f32 values; expected {}",
                observed.len(),
                workload.expected.len()
            ));
        }
        let mut checksum = 0.0_f64;
        for (index, (&actual, &expected)) in
            observed.iter().zip(workload.expected.iter()).enumerate()
        {
            if !actual.is_finite() {
                return Err(format!(
                    "WebGPU checksum validation found a non-finite value at output index {index}"
                ));
            }
            let tolerance = expected.abs().mul_add(0.000_02, 0.000_002);
            if (actual - expected).abs() > tolerance {
                return Err(format!(
                    "WebGPU result mismatch at output index {index}: observed {actual}, expected {expected}, tolerance {tolerance}"
                ));
            }
            checksum += f64::from(actual);
        }
        if checksum.is_finite() {
            Ok(checksum)
        } else {
            Err("WebGPU checksum accumulation was non-finite".to_string())
        }
    }

    async fn timestamp_ms(&self) -> Result<Option<f64>, String> {
        let Some(timestamp) = &self.timestamp else {
            return Ok(None);
        };
        let bytes = Self::map_buffer(&timestamp.readback).await?;
        let values: &[u64] = bytemuck::try_cast_slice(&bytes).map_err(|error| {
            format!("WebGPU timestamp readback had invalid u64 layout: {error}")
        })?;
        let [start, end] = values else {
            return Err("WebGPU timestamp readback did not contain exactly two values".to_string());
        };
        if end < start {
            return Err(format!(
                "WebGPU timestamp query moved backwards from {start} to {end}"
            ));
        }
        Ok(Some(
            (*end - *start) as f64 * f64::from(self.queue.get_timestamp_period()) / 1_000_000.0,
        ))
    }

    fn encode_batch(
        &self,
        workload: &Workload,
        repeat_count: u32,
        include_timestamp: bool,
        copy_output: bool,
    ) -> wgpu::CommandBuffer {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("what-is-this gpu compute batch"),
            });
        let timestamp_writes =
            self.timestamp
                .as_ref()
                .filter(|_| include_timestamp)
                .map(|timestamp| wgpu::ComputePassTimestampWrites {
                    query_set: &timestamp.query_set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("what-is-this gpu timed compute pass"),
                timestamp_writes,
            });
            pass.set_pipeline(&workload.pipeline);
            pass.set_bind_group(0, &workload.bind_group, &[]);
            for _ in 0..repeat_count.saturating_mul(workload.dispatches_per_base) {
                pass.dispatch_workgroups(workload.dispatch_x, 1, 1);
            }
        }
        if let Some(timestamp) = self.timestamp.as_ref().filter(|_| include_timestamp) {
            encoder.resolve_query_set(&timestamp.query_set, 0..2, &timestamp.resolve, 0);
            encoder.copy_buffer_to_buffer(&timestamp.resolve, 0, &timestamp.readback, 0, 16);
        }
        if copy_output {
            encoder.copy_buffer_to_buffer(
                &workload.output,
                0,
                &workload.readback,
                0,
                workload.output_bytes(),
            );
        }
        encoder.copy_buffer_to_buffer(
            &workload.output,
            0,
            &workload.completion_fence,
            0,
            size_of::<f32>() as u64,
        );
        encoder.finish()
    }

    async fn run_batched(
        &self,
        workload: &Workload,
        repeat_count: u32,
    ) -> Result<GpuRunResult, String> {
        let include_timestamp = self.timestamp.is_some();
        let commands = self.encode_batch(workload, repeat_count, include_timestamp, true);
        let started = performance_now();
        self.queue.submit([commands]);
        self.wait_for_completion(workload).await?;
        let queue_elapsed_ms = performance_now() - started;
        let output = Self::map_buffer(&workload.readback).await?;
        let checksum = Self::validate_output(workload, &output)?;
        let gpu_elapsed_ms = self.timestamp_ms().await?;
        Ok(GpuRunResult {
            checksum,
            queue_elapsed_ms,
            gpu_elapsed_ms,
            timing_method: if include_timestamp {
                "gpu_timestamp_query"
            } else {
                "submit_to_map_async_completion_fence_wall_clock"
            },
            dispatch_count: repeat_count.saturating_mul(workload.dispatches_per_base),
        })
    }

    async fn run_dispatch_roundtrips(
        &self,
        workload: &Workload,
        repeat_count: u32,
    ) -> Result<GpuRunResult, String> {
        let total = repeat_count.saturating_mul(DISPATCH_ROUND_TRIPS);
        let started = performance_now();
        for index in 0..total {
            let commands = self.encode_batch(
                workload,
                1,
                index == 0 && self.timestamp.is_some(),
                index + 1 == total,
            );
            self.queue.submit([commands]);
            self.wait_for_completion(workload).await?;
        }
        let queue_elapsed_ms = performance_now() - started;
        let output = Self::map_buffer(&workload.readback).await?;
        let checksum = Self::validate_output(workload, &output)?;
        let gpu_elapsed_ms = self.timestamp_ms().await?;
        Ok(GpuRunResult {
            checksum,
            queue_elapsed_ms,
            gpu_elapsed_ms,
            timing_method: if self.timestamp.is_some() {
                "submit_to_map_async_completion_fence_wall_clock_with_timestamp_probe"
            } else {
                "submit_to_map_async_completion_fence_wall_clock"
            },
            dispatch_count: total,
        })
    }

    async fn run(&self, kernel_id: &str, repeat_count: u32) -> Result<GpuRunResult, String> {
        if repeat_count == 0 || repeat_count > MAX_REPEAT_COUNT {
            return Err(format!(
                "WebGPU repeat count must be in 1..={MAX_REPEAT_COUNT}, got {repeat_count}"
            ));
        }
        if let Some(reason) = self.lost_reason() {
            return Err(reason);
        }
        let workload = self.workload(kernel_id)?;
        if kernel_id == "gpu.dispatch-roundtrip.tiny.v1" {
            self.run_dispatch_roundtrips(workload, repeat_count).await
        } else {
            self.run_batched(workload, repeat_count).await
        }
    }
}

thread_local! {
    static SUITE: RefCell<Option<GpuSuite>> = const { RefCell::new(None) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

pub(crate) fn reset() {
    cancel();
}

pub(crate) fn cancel() {
    GENERATION.set(GENERATION.get().wrapping_add(1));
    SUITE.with_borrow_mut(|slot| *slot = None);
}

pub(crate) async fn initialize() -> Result<String, String> {
    let generation = GENERATION.get();
    let (suite, facts) = GpuSuite::new().await?;
    if GENERATION.get() != generation {
        return Err(
            "WebGPU initialization finished after its diagnostic run was replaced".to_string(),
        );
    }
    let json = serde_json::to_string(&facts)
        .map_err(|error| format!("could not encode WebGPU adapter facts: {error}"))?;
    SUITE.with_borrow_mut(|slot| *slot = Some(suite));
    Ok(json)
}

pub(crate) fn status_json() -> String {
    SUITE.with_borrow(|slot| {
        let Some(suite) = slot.as_ref() else {
            return r#"{"available":false,"reason":"WebGPU compute suite is not initialized"}"#
                .to_string();
        };
        suite.lost_reason().map_or_else(
            || r#"{"available":true,"reason":null}"#.to_string(),
            |reason| serde_json::json!({ "available": false, "reason": reason }).to_string(),
        )
    })
}

pub(crate) async fn run(kernel_id: &str, repeat_count: u32) -> Result<String, String> {
    let generation = GENERATION.get();
    let suite = SUITE.with_borrow_mut(Option::take).ok_or_else(|| {
        "WebGPU compute suite is busy, unavailable, or not initialized".to_string()
    })?;
    let result = suite.run(kernel_id, repeat_count).await;
    if GENERATION.get() != generation {
        return Err("WebGPU compute work completed after its stage was cancelled".to_string());
    }
    SUITE.with_borrow_mut(|slot| *slot = Some(suite));
    let measured = result?;
    serde_json::to_string(&measured)
        .map_err(|error| format!("could not encode WebGPU kernel result: {error}"))
}
