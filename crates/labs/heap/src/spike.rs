//! Browser output-path spike and native contract evidence.

use serde::Serialize;

const EXPECTED: [f32; 4] = [0.25, -0.5, 1.5, 7.0];
const SOURCE: [u32; 3] = [2, 3, 1];
const DESTINATION: [u32; 3] = [4, 5, 2];
const PRODUCER_SHADER: &str = r"
struct Header { source: vec4<u32> }
@group(0) @binding(0) var data_heap: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> header: Header;
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let points = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(points[index], 0.0, 1.0);
}
@fragment fn fragment_main() -> @location(0) vec4<f32> {
    return textureLoad(data_heap, vec2<i32>(header.source.xy), i32(header.source.z), 0);
}
";
const CONSUMER_SHADER: &str = r"
struct Header { source: vec4<u32> }
struct VertexOut { @builtin(position) position: vec4<f32>, @location(0) @interpolate(flat) value: vec4<f32> }
@group(0) @binding(0) var data_heap: texture_2d_array<f32>;
@group(0) @binding(1) var<uniform> header: Header;
@vertex fn vertex_main(@builtin(vertex_index) index: u32) -> VertexOut {
    let points = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return VertexOut(vec4(points[index], 0.0, 1.0), textureLoad(data_heap, vec2<i32>(header.source.xy), i32(header.source.z), 0));
}
@fragment fn fragment_main(input: VertexOut) -> @location(0) vec4<f32> { return input.value; }
";

#[derive(Debug, Serialize)]
struct CaseResult {
    name: &'static str,
    status: &'static str,
    detail: String,
}

fn aligned_header_upload(alignment: u32) -> (u32, Vec<u8>) {
    let alignment = alignment.max(16);
    let stride = 16_u32.div_ceil(alignment) * alignment;
    let mut bytes = vec![0_u8; stride as usize * 3];
    for (slot, value) in [
        [99, 98, 97, 0],
        [SOURCE[0], SOURCE[1], SOURCE[2], 0],
        [DESTINATION[0], DESTINATION[1], DESTINATION[2], 0],
    ]
    .into_iter()
    .enumerate()
    {
        let start = slot * stride as usize;
        bytes[start..start + 16].copy_from_slice(bytemuck::bytes_of(&value));
    }
    (stride, bytes)
}

fn overlap_case(observed: Result<Option<String>, String>) -> CaseResult {
    match observed {
        Ok(Some(diagnostic)) => CaseResult { name: "direct-overlap-refusal", status: "PASS", detail: format!("expected scoped validation refusal observed: {diagnostic}") },
        Ok(None) => CaseResult { name: "direct-overlap-refusal", status: "FAIL", detail: "unexpected acceptance: the sampled full DATA array and attached DATA layer produced no scoped validation error; this is evidence only and never authorizes the path".to_string() },
        Err(error) => CaseResult { name: "direct-overlap-refusal", status: "FAIL", detail: error },
    }
}

fn exact_case(name: &'static str, observed: Result<[f32; 4], String>, proof: &str) -> CaseResult {
    match observed {
        Ok(value) if value.map(f32::to_bits) == EXPECTED.map(f32::to_bits) => CaseResult {
            name,
            status: "PASS",
            detail: format!("exact readback {value:?}; {proof}"),
        },
        Ok(value) => CaseResult {
            name,
            status: "FAIL",
            detail: format!("expected {EXPECTED:?}, observed {value:?}; {proof}"),
        },
        Err(error) => CaseResult {
            name,
            status: "FAIL",
            detail: error,
        },
    }
}

#[cfg(target_arch = "wasm32")]
#[allow(clippy::cast_precision_loss, clippy::future_not_send)]
mod browser {
    use std::cell::Cell;
    use std::future::Future as _;
    use std::num::NonZeroU64;
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use serde::Serialize;
    use wasm_bindgen::prelude::*;
    use wgpu::util::DeviceExt;

    use super::{
        CONSUMER_SHADER, CaseResult, DESTINATION, EXPECTED, PRODUCER_SHADER, SOURCE,
        aligned_header_upload, exact_case, overlap_case,
    };

    const SIDE: u32 = 8;
    const LAYERS: u32 = 4;
    const DEADLINE_MS: f64 = 4_000.0;

    thread_local! { static GENERATION: Cell<u64> = const { Cell::new(0) }; }

    #[derive(Serialize)]
    struct SpikeReport {
        adapter: String,
        backend: String,
        header_stride: u32,
        source: [u32; 3],
        destination: [u32; 3],
        expected: [f32; 4],
        cases: Vec<CaseResult>,
    }

    struct Spike {
        device: wgpu::Device,
        queue: wgpu::Queue,
        lost: Arc<Mutex<Option<String>>>,
        producer: wgpu::RenderPipeline,
        consumer: wgpu::RenderPipeline,
        data_a: wgpu::Texture,
        data_b: wgpu::Texture,
        scratch: wgpu::Texture,
        group_a: wgpu::BindGroup,
        group_b: wgpu::BindGroup,
        header_stride: u32,
        _headers: wgpu::Buffer,
    }

    fn now() -> f64 {
        web_sys::window()
            .and_then(|window| window.performance())
            .map_or_else(js_sys::Date::now, |performance| performance.now())
    }

    async fn yield_to_browser() -> Result<(), String> {
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let Some(window) = web_sys::window() else {
                drop(reject.call1(
                    &JsValue::UNDEFINED,
                    &JsValue::from_str("window is unavailable"),
                ));
                return;
            };
            if let Err(error) =
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0)
            {
                drop(reject.call1(&JsValue::UNDEFINED, &error));
            }
        });
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .map(|_| ())
            .map_err(|error| format!("browser yield failed: {error:?}"))
    }

    fn check_generation(token: u64) -> Result<(), String> {
        let current = GENERATION.get();
        (current == token)
            .then_some(())
            .ok_or_else(|| format!("stale spike generation {token}; current is {current}"))
    }

    async fn wait_scope(
        device: &wgpu::Device,
        lost: &Arc<Mutex<Option<String>>>,
        token: u64,
    ) -> Result<Option<String>, String> {
        let mut future = Box::pin(device.pop_error_scope());
        let started = now();
        loop {
            check_generation(token)?;
            device.poll(wgpu::Maintain::Poll);
            if let Some(reason) = lost.lock().ok().and_then(|slot| slot.clone()) {
                return Err(format!("device lost: {reason}"));
            }
            let mut context = Context::from_waker(Waker::noop());
            if let Poll::Ready(error) = future.as_mut().poll(&mut context) {
                return Ok(error.map(|error| error.to_string()));
            }
            if now() - started >= DEADLINE_MS {
                return Err(format!("error scope exceeded {DEADLINE_MS} ms"));
            }
            yield_to_browser().await?;
        }
    }

    fn texture(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::TextureUsages,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: SIDE,
                height: SIDE,
                depth_or_array_layers: LAYERS,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage,
            view_formats: &[],
        })
    }

    fn layer_view(texture: &wgpu::Texture, layer: u32, label: &'static str) -> wgpu::TextureView {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(label),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: layer,
            array_layer_count: Some(1),
            ..Default::default()
        })
    }

    fn pipeline(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        label: &'static str,
        source: &'static str,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba32Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    impl Spike {
        #[allow(clippy::too_many_lines)]
        async fn new(
            canvas: web_sys::HtmlCanvasElement,
            token: u64,
        ) -> Result<(Self, String, String), String> {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::GL,
                ..Default::default()
            });
            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|error| format!("surface creation failed: {error}"))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .ok_or_else(|| "no WebGL2 adapter".to_string())?;
            check_generation(token)?;
            let info = adapter.get_info();
            if info.backend != wgpu::Backend::Gl {
                return Err(format!("requested GL but selected {:?}", info.backend));
            }
            let limits = adapter.limits();
            if limits.max_texture_array_layers < LAYERS {
                return Err(format!(
                    "array layer limit {} is below {LAYERS}",
                    limits.max_texture_array_layers
                ));
            }
            let required_usage = wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST;
            let features = adapter.get_texture_format_features(wgpu::TextureFormat::Rgba32Float);
            if !features.allowed_usages.contains(required_usage) {
                return Err(format!(
                    "RGBA32Float usages {:?} omit {:?}",
                    features.allowed_usages, required_usage
                ));
            }
            let required_limits =
                wgpu::Limits::downlevel_webgl2_defaults().using_resolution(limits);
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("heap output spike GL device"),
                        required_features: wgpu::Features::empty(),
                        required_limits,
                        memory_hints: wgpu::MemoryHints::MemoryUsage,
                    },
                    None,
                )
                .await
                .map_err(|error| format!("device request failed: {error}"))?;
            check_generation(token)?;
            let lost = Arc::new(Mutex::new(None));
            let lost_callback = Arc::clone(&lost);
            device.set_device_lost_callback(move |reason, message| {
                if let Ok(mut slot) = lost_callback.lock() {
                    *slot = Some(format!("{reason:?}: {message}"));
                }
            });
            crate::browser_error::install_logging_handler(&device, "heap spike");
            let header_stride = device.limits().min_uniform_buffer_offset_alignment.max(16);
            let (header_stride, header_bytes) = aligned_header_upload(header_stride);
            let headers = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("spike static dispatch headers"),
                contents: &header_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("spike immutable heap layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: NonZeroU64::new(16),
                        },
                        count: None,
                    },
                ],
            });
            let usage = required_usage;
            let data_a = texture(&device, "spike DATA A", usage);
            let data_b = texture(&device, "spike DATA B", usage);
            let scratch = texture(
                &device,
                "spike SCRATCH",
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            );
            for target in [&data_a, &data_b] {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: target,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: SOURCE[0],
                            y: SOURCE[1],
                            z: SOURCE[2],
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::bytes_of(&EXPECTED),
                    wgpu::TexelCopyBufferLayout::default(),
                    wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
            }
            let make_group = |texture: &wgpu::Texture, label| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&texture.create_view(
                                &wgpu::TextureViewDescriptor {
                                    dimension: Some(wgpu::TextureViewDimension::D2Array),
                                    array_layer_count: Some(LAYERS),
                                    ..Default::default()
                                },
                            )),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &headers,
                                offset: 0,
                                size: NonZeroU64::new(16),
                            }),
                        },
                    ],
                })
            };
            device.push_error_scope(wgpu::ErrorFilter::Validation);
            let producer = pipeline(&device, &layout, "spike producer", PRODUCER_SHADER);
            let consumer = pipeline(&device, &layout, "spike vertex consumer", CONSUMER_SHADER);
            let group_a = make_group(&data_a, "spike immutable heap group A");
            let group_b = make_group(&data_b, "spike immutable heap group B");
            if let Some(error) = wait_scope(&device, &lost, token).await? {
                return Err(format!("spike setup validation failed: {error}"));
            }
            Ok((
                Self {
                    device,
                    queue,
                    lost,
                    producer,
                    consumer,
                    data_a,
                    data_b,
                    scratch,
                    group_a,
                    group_b,
                    header_stride,
                    _headers: headers,
                },
                info.name,
                format!("{:?}", info.backend),
            ))
        }

        fn render(
            &self,
            encoder: &mut wgpu::CommandEncoder,
            group: &wgpu::BindGroup,
            target: &wgpu::TextureView,
            header: u32,
            label: &'static str,
        ) {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.producer);
            pass.set_bind_group(0, group, &[header]);
            pass.set_viewport(
                DESTINATION[0] as f32,
                DESTINATION[1] as f32,
                1.0,
                1.0,
                0.0,
                1.0,
            );
            pass.draw(0..3, 0..1);
        }

        async fn overlap(&self, token: u64) -> Result<Option<String>, String> {
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let target = layer_view(&self.data_a, DESTINATION[2], "overlapping DATA attachment");
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("direct overlap diagnostic"),
                });
            self.render(
                &mut encoder,
                &self.group_a,
                &target,
                self.header_stride,
                "sampled DATA plus DATA attachment",
            );
            self.queue.submit([encoder.finish()]);
            wait_scope(&self.device, &self.lost, token).await
        }

        #[allow(clippy::too_many_lines)]
        async fn output_path(&self, ping_pong: bool, token: u64) -> Result<[f32; 4], String> {
            self.device.push_error_scope(wgpu::ErrorFilter::Validation);
            let output = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("spike vertex-load target"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let output_view = output.create_view(&wgpu::TextureViewDescriptor::default());
            let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("spike exact readback"),
                size: 256,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("spike output path"),
                });
            if ping_pong {
                let target_b = layer_view(&self.data_b, DESTINATION[2], "ping B destination");
                self.render(
                    &mut encoder,
                    &self.group_a,
                    &target_b,
                    self.header_stride,
                    "ping A to B",
                );
                let target_a = layer_view(&self.data_a, DESTINATION[2], "pong A destination");
                self.render(
                    &mut encoder,
                    &self.group_b,
                    &target_a,
                    self.header_stride * 2,
                    "pong B to A",
                );
            } else {
                let scratch = layer_view(&self.scratch, 1, "SCRATCH output layer");
                self.render(
                    &mut encoder,
                    &self.group_a,
                    &scratch,
                    self.header_stride,
                    "DATA to SCRATCH",
                );
                encoder.copy_texture_to_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.scratch,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: DESTINATION[0],
                            y: DESTINATION[1],
                            z: 1,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.data_a,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: DESTINATION[0],
                            y: DESTINATION[1],
                            z: DESTINATION[2],
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                );
            }
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("vertex-stage DATA consumption"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &output_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                pass.set_pipeline(&self.consumer);
                pass.set_bind_group(0, &self.group_a, &[self.header_stride * 2]);
                pass.draw(0..3, 0..1);
            }
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &output,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            self.queue.submit([encoder.finish()]);
            if let Some(error) = wait_scope(&self.device, &self.lost, token).await? {
                return Err(format!("scoped validation error: {error}"));
            }
            self.map_value(&readback, token).await
        }

        async fn map_value(&self, buffer: &wgpu::Buffer, token: u64) -> Result<[f32; 4], String> {
            let state = Arc::new(Mutex::new(None));
            let callback = Arc::clone(&state);
            let slice = buffer.slice(..);
            slice.map_async(wgpu::MapMode::Read, move |result| {
                if let Ok(mut slot) = callback.lock() {
                    *slot = Some(result.map_err(|error| error.to_string()));
                }
            });
            let started = now();
            loop {
                check_generation(token)?;
                self.device.poll(wgpu::Maintain::Poll);
                if let Some(result) = state.lock().ok().and_then(|mut slot| slot.take()) {
                    result?;
                    let mapped = slice.get_mapped_range();
                    let mut value = [0.0; 4];
                    for (component, bytes) in value.iter_mut().zip(mapped[..16].as_chunks::<4>().0)
                    {
                        *component = f32::from_ne_bytes(*bytes);
                    }
                    drop(mapped);
                    buffer.unmap();
                    return Ok(value);
                }
                if let Some(reason) = self.lost.lock().ok().and_then(|slot| slot.clone()) {
                    buffer.unmap();
                    return Err(format!("device lost: {reason}"));
                }
                if now() - started >= DEADLINE_MS {
                    buffer.unmap();
                    return Err(format!("map_async exceeded {DEADLINE_MS} ms"));
                }
                yield_to_browser().await?;
            }
        }
    }

    /// Cancels any older spike run before its asynchronous result can publish.
    #[wasm_bindgen]
    pub fn cancel_heap_spike() {
        GENERATION.set(GENERATION.get().wrapping_add(1));
    }

    /// Runs all three GL output-path cases and returns their literal observations.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when GL initialization, generation validation, or report
    /// serialization fails; per-case GPU failures remain literal `FAIL` results in the report.
    #[wasm_bindgen]
    pub async fn run_heap_spike_json(
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<String, JsValue> {
        let token = GENERATION.get().wrapping_add(1);
        GENERATION.set(token);
        let (spike, adapter, backend) = Spike::new(canvas, token)
            .await
            .map_err(|error| JsValue::from_str(&error))?;
        let overlap = overlap_case(spike.overlap(token).await);
        let stride = spike.header_stride;
        let scratch = exact_case(
            "scratch-copy-dynamic-offset-vertex-load",
            spike.output_path(false, token).await,
            &format!(
                "SCRATCH layer 1 copied to DATA {:?}; producer header offset {stride}, vertex header offset {}",
                DESTINATION,
                stride * 2
            ),
        );
        let ping_pong = exact_case(
            "ping-pong-dynamic-offset-vertex-load",
            spike.output_path(true, token).await,
            &format!(
                "two immutable groups ran A-to-B then B-to-A at DATA {:?}; dynamic offsets {stride} and {}",
                DESTINATION,
                stride * 2
            ),
        );
        check_generation(token).map_err(|error| JsValue::from_str(&error))?;
        serde_json::to_string(&SpikeReport {
            adapter,
            backend,
            header_stride: stride,
            source: SOURCE,
            destination: DESTINATION,
            expected: EXPECTED,
            cases: vec![overlap, scratch, ping_pong],
        })
        .map_err(|error| JsValue::from_str(&format!("could not serialize spike report: {error}")))
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::{cancel_heap_spike, run_heap_spike_json};

#[cfg(test)]
mod tests {
    use super::{
        CONSUMER_SHADER, DESTINATION, EXPECTED, PRODUCER_SHADER, SOURCE, aligned_header_upload,
        exact_case, overlap_case,
    };

    #[test]
    fn spike_shaders_parse_and_validate() {
        for source in [PRODUCER_SHADER, CONSUMER_SHADER] {
            let module = naga::front::wgsl::parse_str(source).expect("fixed spike WGSL parses");
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .expect("fixed spike WGSL validates");
        }
    }

    #[test]
    fn static_headers_use_distinct_aligned_dynamic_offsets() {
        let (stride, bytes) = aligned_header_upload(256);
        assert_eq!(stride, 256);
        assert_eq!(bytes.len(), 768);
        assert_eq!(
            &bytes[stride as usize..stride as usize + 12],
            bytemuck::bytes_of(&[SOURCE[0], SOURCE[1], SOURCE[2]])
        );
        assert_eq!(
            &bytes[stride as usize * 2..stride as usize * 2 + 12],
            bytemuck::bytes_of(&[DESTINATION[0], DESTINATION[1], DESTINATION[2]])
        );
    }

    #[test]
    fn direct_overlap_pass_requires_an_observed_refusal() {
        assert_eq!(
            overlap_case(Ok(Some("usage conflict".to_string()))).status,
            "PASS"
        );
        assert_eq!(overlap_case(Ok(None)).status, "FAIL");
        assert_eq!(overlap_case(Err("deadline".to_string())).status, "FAIL");
    }

    #[test]
    fn exact_readback_uses_float_bits() {
        assert_eq!(exact_case("path", Ok(EXPECTED), "proof").status, "PASS");
        assert_eq!(
            exact_case("path", Ok([0.25, -0.5, 1.5, 7.000_001]), "proof").status,
            "FAIL"
        );
        assert_eq!(
            exact_case("path", Err("mapping".to_string()), "proof").status,
            "FAIL"
        );
    }
}
