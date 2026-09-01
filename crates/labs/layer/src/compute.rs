//! Reusable fragment-compute dialect and frozen dispatch plans.

use std::fmt::{self, Write as _};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use serde::Serialize;
use thiserror::Error;
use wgpu::util::DeviceExt;

const SLOT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
#[cfg(target_arch = "wasm32")]
const COMPLETION_DEADLINE_MS: i32 = 4_000;
static NEXT_DEVICE_ID: AtomicU64 = AtomicU64::new(1);

/// Logical indexing domain and its backing texture rectangle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexSpace {
    /// A linear domain packed into the smallest square texture.
    Grid1D(u32),
    /// A domain whose logical and physical dimensions match.
    Grid2D(u32, u32),
}

impl IndexSpace {
    /// Number of logical elements.
    #[must_use]
    pub const fn len(self) -> u32 {
        match self {
            Self::Grid1D(length) => length,
            Self::Grid2D(width, height) => width.saturating_mul(height),
        }
    }

    /// Whether this domain has no logical elements.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// Physical RGBA32F texture dimensions.
    #[must_use]
    pub const fn rect(self) -> (u32, u32) {
        match self {
            Self::Grid1D(length) => {
                let mut side = 1_u32;
                while side.saturating_mul(side) < length {
                    side = side.saturating_add(1);
                }
                (side, side)
            }
            Self::Grid2D(width, height) => (width, height),
        }
    }
}

/// One forbidden operation recognized in parsed kernel-body IR.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ForbiddenConstruct {
    /// Module-scope workgroup storage.
    #[error("workgroup variable")]
    WorkgroupVariable,
    /// An atomic type or operation.
    #[error("atomic operation")]
    Atomic,
    /// A workgroup or storage barrier.
    #[error("barrier")]
    Barrier,
    /// A raw storage resource declaration.
    #[error("raw storage access")]
    RawStorageAccess,
}

/// A typed registration-time kernel-dialect failure.
#[derive(Debug, Error)]
pub enum DialectError {
    /// Descriptor names or shapes do not satisfy the dialect.
    #[error("kernel {kernel} has an invalid descriptor: {message}")]
    InvalidDescriptor {
        /// Stable kernel name.
        kernel: String,
        /// Exact problem.
        message: String,
    },
    /// The body is not parseable WGSL, even with accessor stubs.
    #[error("kernel {kernel} WGSL parse failed: {message}")]
    Parse {
        /// Stable kernel name.
        kernel: String,
        /// Naga diagnostic.
        message: String,
    },
    /// The body contains an operation outside the dialect.
    #[error("kernel {kernel} refused forbidden {construct}")]
    Forbidden {
        /// Stable kernel name.
        kernel: String,
        /// Typed refused construct.
        construct: ForbiddenConstruct,
    },
    /// The complete generated shader did not validate.
    #[error("kernel {kernel} assembled WGSL validation failed: {message}")]
    Validation {
        /// Stable kernel name.
        kernel: String,
        /// Naga diagnostic.
        message: String,
    },
}

/// Device, capability, resource, completion, or dialect failure.
#[derive(Debug, Error)]
pub enum LayerError {
    /// A kernel was refused before pipeline creation.
    #[error(transparent)]
    Dialect(#[from] DialectError),
    /// Required WebGL2 behavior was unavailable.
    #[error("fragment-compute capability refused: {0}")]
    Capability(String),
    /// wgpu rejected a generated shader or frozen pipeline.
    #[error("fragment-compute registration failed: {0}")]
    Pipeline(String),
    /// A buffer, binding, or dispatch argument was invalid.
    #[error("fragment-compute resource error: {0}")]
    Resource(String),
    /// The device was lost.
    #[error("fragment-compute device lost: {0}")]
    DeviceLost(String),
    /// A completion or read deadline elapsed.
    #[error("fragment-compute completion exceeded its {0} ms deadline")]
    Deadline(i32),
    /// A completion belongs to an older dispatch generation.
    #[error("fragment-compute completion generation {observed} is stale; current is {current}")]
    StaleGeneration {
        /// Token generation.
        observed: u64,
        /// Current generation.
        current: u64,
    },
    /// Mapping the ordered readback failed.
    #[error("fragment-compute mapping failed: {0}")]
    Mapping(String),
}

/// Facts proved before any user kernel is accepted.
#[derive(Clone, Debug, Serialize)]
pub struct CapabilityFacts {
    /// Stable format used by every slot.
    pub slot_format: &'static str,
    /// Exposed MRT attachment count.
    pub max_color_attachments: u32,
    /// Exposed sampled-texture count per shader stage.
    pub vertex_texture_units: u32,
    /// Exposed two-dimensional texture limit.
    pub max_texture_dimension_2d: u32,
    /// Whether RGBA32F advertises both rendering and sampling.
    pub rgba32f_render_and_sample: bool,
    /// Golden output checksum after rendering and mapping.
    pub golden_checksum: Option<f64>,
}

struct Slot {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

/// A vec4-granular `SoA` allocation backed by RGBA32F textures.
pub struct ComputeBuffer {
    owner: u64,
    id: u64,
    label: String,
    index_space: IndexSpace,
    slots: Vec<Slot>,
}

impl ComputeBuffer {
    /// Logical indexing domain.
    #[must_use]
    pub const fn index_space(&self) -> IndexSpace {
        self.index_space
    }

    /// Number of independent vec4 `SoA` slots.
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Texture view suitable for a consumer vertex-stage binding.
    ///
    /// # Errors
    ///
    /// Returns an error when `slot` is outside this allocation.
    pub fn as_vertex_texture(&self, slot: usize) -> Result<&wgpu::TextureView, LayerError> {
        self.slots
            .get(slot)
            .map(|entry| &entry.view)
            .ok_or_else(|| LayerError::Resource(format!("{} has no slot {slot}", self.label)))
    }
}

/// One generated texture accessor bound to a concrete input slot.
pub struct InputBinding<'a> {
    /// WGSL accessor function name.
    pub accessor: &'a str,
    /// Allocation containing the slot.
    pub buffer: &'a ComputeBuffer,
    /// `SoA` slot index.
    pub slot: usize,
}

/// One returned result field bound to an MRT output slot.
pub struct OutputBinding<'a> {
    /// Field name in the kernel's returned structure.
    pub field: &'a str,
    /// Allocation containing the destination slot.
    pub buffer: &'a ComputeBuffer,
    /// `SoA` slot index.
    pub slot: usize,
}

/// Complete registration description for one pure WGSL kernel body.
pub struct KernelDesc<'a> {
    /// Stable diagnostic and pipeline name.
    pub name: &'a str,
    /// Entry-point-agnostic WGSL declarations and `kernel` function.
    pub body: &'a str,
    /// Invocation domain and output rectangle.
    pub index_space: IndexSpace,
    /// Generated input accessor bindings.
    pub inputs: &'a [InputBinding<'a>],
    /// Returned fields lowered to MRT attachments.
    pub outputs: &'a [OutputBinding<'a>],
    /// WGSL name of the uniform structure passed to `kernel`.
    pub uniform_type: &'a str,
    /// Exact uniform structure size in bytes.
    pub uniform_size: u64,
}

/// Frozen render pipeline, attachments, resources, and binding plan.
pub struct Kernel {
    owner: u64,
    name: String,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
    uniform_size: u64,
    outputs: Vec<wgpu::TextureView>,
    width: u32,
    height: u32,
    source: String,
}

impl Kernel {
    /// Fully assembled source validated by Naga at registration.
    #[must_use]
    pub fn assembled_source(&self) -> &str {
        &self.source
    }
}

/// Opaque generation returned by a submitted dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchToken {
    owner: u64,
    generation: u64,
}

enum MapOutcome {
    Complete(Result<(), wgpu::BufferAsyncError>),
    #[cfg(target_arch = "wasm32")]
    Deadline,
}

struct CallbackState {
    value: Option<MapOutcome>,
    waker: Option<Waker>,
    closed: bool,
}

struct CallbackFuture {
    state: Arc<Mutex<CallbackState>>,
}

impl Future for CallbackFuture {
    type Output = MapOutcome;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let Ok(mut state) = self.state.lock() else {
            return Poll::Pending;
        };
        state.value.take().map_or_else(
            || {
                state.waker = Some(context.waker().clone());
                Poll::Pending
            },
            Poll::Ready,
        )
    }
}

impl Drop for CallbackFuture {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.value = None;
            state.waker = None;
        }
    }
}

fn callback_pair() -> (CallbackFuture, Arc<Mutex<CallbackState>>) {
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

fn finish_callback(state: &Arc<Mutex<CallbackState>>, value: MapOutcome) {
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

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn parse_body(kernel: &str, body: &str, accessors: &[&str]) -> Result<naga::Module, DialectError> {
    let mut scan = String::new();
    for accessor in accessors {
        writeln!(
            scan,
            "fn {accessor}(index: u32) -> vec4<f32> {{ return vec4<f32>(f32(index) * 0.0); }}"
        )
        .map_err(|error| DialectError::Parse {
            kernel: kernel.to_string(),
            message: format!("could not assemble accessor stub: {error}"),
        })?;
    }
    scan.push_str(body);
    naga::front::wgsl::parse_str(&scan).map_err(|error| DialectError::Parse {
        kernel: kernel.to_string(),
        message: error.emit_to_string(&scan),
    })
}

fn prescan(kernel: &str, body: &str, accessors: &[&str]) -> Result<(), DialectError> {
    let module = parse_body(kernel, body, accessors)?;
    let ir = format!("{module:#?}");
    let forbidden = [
        ("Atomic(", ForbiddenConstruct::Atomic),
        ("Atomic {", ForbiddenConstruct::Atomic),
        ("Barrier(", ForbiddenConstruct::Barrier),
        ("WorkGroupUniformLoad", ForbiddenConstruct::Barrier),
        ("space: WorkGroup", ForbiddenConstruct::WorkgroupVariable),
        ("space: Storage", ForbiddenConstruct::RawStorageAccess),
    ];
    for (needle, construct) in forbidden {
        if ir.contains(needle) {
            return Err(DialectError::Forbidden {
                kernel: kernel.to_string(),
                construct,
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn assemble(desc: &KernelDesc<'_>) -> Result<String, DialectError> {
    if !identifier(desc.name) || !identifier(desc.uniform_type) {
        return Err(DialectError::InvalidDescriptor {
            kernel: desc.name.to_string(),
            message: "kernel and uniform type must be WGSL identifiers".to_string(),
        });
    }
    if desc.index_space.len() == 0 || desc.outputs.is_empty() || desc.outputs.len() > 4 {
        return Err(DialectError::InvalidDescriptor {
            kernel: desc.name.to_string(),
            message: "index space must be nonempty and output count must be 1 through 4"
                .to_string(),
        });
    }
    if desc.uniform_size == 0 || !desc.uniform_size.is_multiple_of(16) {
        return Err(DialectError::InvalidDescriptor {
            kernel: desc.name.to_string(),
            message: "uniform size must be a nonzero multiple of 16 bytes".to_string(),
        });
    }
    let accessors: Vec<_> = desc.inputs.iter().map(|input| input.accessor).collect();
    if accessors.iter().any(|name| !identifier(name))
        || desc.outputs.iter().any(|output| !identifier(output.field))
    {
        return Err(DialectError::InvalidDescriptor {
            kernel: desc.name.to_string(),
            message: "accessor and result field names must be WGSL identifiers".to_string(),
        });
    }
    prescan(desc.name, desc.body, &accessors)?;
    let (width, _) = desc.index_space.rect();
    let mut source = String::new();
    for (binding, input) in desc.inputs.iter().enumerate() {
        writeln!(
            source,
            "@group(0) @binding({binding}) var layer_input_{binding}: texture_2d<f32>;"
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: format!("could not assemble texture binding: {error}"),
        })?;
        writeln!(
            source,
            "fn {}(index: u32) -> vec4<f32> {{ return textureLoad(layer_input_{binding}, vec2<i32>(i32(index % {width}u), i32(index / {width}u)), 0); }}",
            input.accessor
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: format!("could not assemble accessor: {error}"),
        })?;
    }
    source.push_str(desc.body);
    source.push('\n');
    let uniform_binding = desc.inputs.len();
    writeln!(
        source,
        "@group(0) @binding({uniform_binding}) var<uniform> layer_uniform: {};",
        desc.uniform_type
    )
    .map_err(|error| DialectError::Validation {
        kernel: desc.name.to_string(),
        message: format!("could not assemble uniform binding: {error}"),
    })?;
    source.push_str("struct LayerVertexOut { @builtin(position) position: vec4<f32>, }\n");
    source.push_str(
        "@vertex fn layer_vertex(@builtin(vertex_index) vertex: u32) -> LayerVertexOut {\n",
    );
    source.push_str("  var positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));\n");
    source.push_str("  var output: LayerVertexOut; output.position = vec4(positions[vertex], 0.0, 1.0); return output;\n}\n");
    source.push_str("struct LayerFragmentOut {\n");
    for (location, output) in desc.outputs.iter().enumerate() {
        writeln!(
            source,
            "  @location({location}) output_{location}: vec4<f32>, // {}",
            output.field
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: format!("could not assemble output declaration: {error}"),
        })?;
    }
    source.push_str("}\n@fragment fn layer_fragment(@builtin(position) position: vec4<f32>) -> LayerFragmentOut {\n");
    writeln!(
        source,
        "  let index = u32(position.y) * {width}u + u32(position.x); if (index >= {}u) {{ discard; }}",
        desc.index_space.len()
    )
    .map_err(|error| DialectError::Validation {
        kernel: desc.name.to_string(),
        message: format!("could not assemble index mapping: {error}"),
    })?;
    source.push_str("  let result = kernel(index, layer_uniform); var output: LayerFragmentOut;\n");
    for (location, output) in desc.outputs.iter().enumerate() {
        writeln!(
            source,
            "  output.output_{location} = result.{};",
            output.field
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: format!("could not assemble output assignment: {error}"),
        })?;
    }
    source.push_str("  return output;\n}\n");
    let module =
        naga::front::wgsl::parse_str(&source).map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: error.emit_to_string(&source),
        })?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|error| DialectError::Validation {
        kernel: desc.name.to_string(),
        message: error.to_string(),
    })?;
    Ok(source)
}

/// A WebGL2-compatible fragment-compute device.
pub struct ComputeDevice {
    owner: u64,
    next_buffer: Mutex<u64>,
    generation: Mutex<u64>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    marker: wgpu::Buffer,
    lost: Arc<Mutex<Option<String>>>,
    facts: CapabilityFacts,
}

impl ComputeDevice {
    /// Probes hard limits and RGBA32F usage before constructing a layer device.
    ///
    /// # Errors
    ///
    /// Refuses adapters without four MRTs, vertex texture units, or renderable RGBA32F slots.
    pub fn new(
        adapter: &wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
    ) -> Result<Self, LayerError> {
        let limits = adapter.limits();
        if limits.max_color_attachments < 4 {
            return Err(LayerError::Capability(format!(
                "adapter exposes {} color attachments; the dialect requires four",
                limits.max_color_attachments
            )));
        }
        if limits.max_sampled_textures_per_shader_stage < 4 {
            return Err(LayerError::Capability(format!(
                "adapter exposes {} sampled textures per stage; the pillar requires four vertex/fragment inputs",
                limits.max_sampled_textures_per_shader_stage
            )));
        }
        let format = adapter.get_texture_format_features(SLOT_FORMAT);
        let required = wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST;
        let rgba32f_render_and_sample = format.allowed_usages.contains(required);
        if !rgba32f_render_and_sample {
            return Err(LayerError::Capability(format!(
                "RGBA32Float usages {:?} do not include {:?}; packed RGBA8 fallback is unresolved",
                format.allowed_usages, required
            )));
        }
        let lost = Arc::new(Mutex::new(None));
        let lost_callback = Arc::clone(&lost);
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = lost_callback.lock() {
                *slot = Some(format!("{reason:?}: {message}"));
            }
        });
        let marker = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("fragment compute completion marker source"),
            contents: &[0x6a, 0x09, 0xe6, 0x67],
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        Ok(Self {
            owner: NEXT_DEVICE_ID.fetch_add(1, Ordering::Relaxed),
            next_buffer: Mutex::new(1),
            generation: Mutex::new(0),
            device,
            queue,
            marker,
            lost,
            facts: CapabilityFacts {
                slot_format: "RGBA32Float",
                max_color_attachments: limits.max_color_attachments,
                vertex_texture_units: limits.max_sampled_textures_per_shader_stage,
                max_texture_dimension_2d: limits.max_texture_dimension_2d,
                rgba32f_render_and_sample,
                golden_checksum: None,
            },
        })
    }

    /// Underlying device for a consumer render pipeline on the same adapter.
    #[must_use]
    pub const fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Underlying ordered queue shared by compute and rendering.
    #[must_use]
    pub const fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Capability and golden-test facts.
    #[must_use]
    pub const fn facts(&self) -> &CapabilityFacts {
        &self.facts
    }

    fn lost_reason(&self) -> Option<String> {
        self.lost.lock().ok().and_then(|reason| reason.clone())
    }

    fn check_owner(&self, owner: u64) -> Result<(), LayerError> {
        if owner == self.owner {
            self.lost_reason()
                .map_or(Ok(()), |reason| Err(LayerError::DeviceLost(reason)))
        } else {
            Err(LayerError::Resource(
                "resource belongs to another compute device".to_string(),
            ))
        }
    }

    /// Allocates vec4 `SoA` textures and optionally uploads logical elements.
    ///
    /// # Errors
    ///
    /// Refuses empty spaces, zero slots, oversized textures, or mismatched initial data.
    pub fn create_buffer(
        &self,
        label: &str,
        index_space: IndexSpace,
        slot_count: usize,
        initial: Option<&[&[[f32; 4]]]>,
    ) -> Result<ComputeBuffer, LayerError> {
        let (width, height) = index_space.rect();
        if index_space.len() == 0 || slot_count == 0 || width > self.facts.max_texture_dimension_2d
        {
            return Err(LayerError::Resource(format!(
                "{label} has invalid space {index_space:?} or slot count {slot_count}"
            )));
        }
        if let Some(values) = initial
            && (values.len() != slot_count
                || values
                    .iter()
                    .any(|slot| slot.len() != index_space.len() as usize))
        {
                return Err(LayerError::Resource(format!(
                    "{label} initial data does not match {slot_count} slots of {} elements",
                    index_space.len()
                )));
        }
        let mut slots = Vec::with_capacity(slot_count);
        for slot_index in 0..slot_count {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: SLOT_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            if let Some(values) = initial {
                let mut padded = vec![[0.0_f32; 4]; width as usize * height as usize];
                padded[..values[slot_index].len()].copy_from_slice(values[slot_index]);
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::cast_slice(&padded),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 16),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            }
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            slots.push(Slot { texture, view });
        }
        let mut next_buffer = self
            .next_buffer
            .lock()
            .map_err(|_| LayerError::Resource("buffer id lock was poisoned".to_string()))?;
        let id = *next_buffer;
        *next_buffer = id.wrapping_add(1);
        Ok(ComputeBuffer {
            owner: self.owner,
            id,
            label: label.to_string(),
            index_space,
            slots,
        })
    }

    /// Validates, compiles, binds, and freezes one dispatch plan.
    ///
    /// # Errors
    ///
    /// Returns typed dialect errors or a scoped wgpu pipeline diagnostic at registration time.
    #[allow(clippy::too_many_lines)]
    pub async fn create_kernel(&self, desc: KernelDesc<'_>) -> Result<Kernel, LayerError> {
        for input in desc.inputs {
            self.check_owner(input.buffer.owner)?;
            if input.buffer.slots.get(input.slot).is_none() {
                return Err(LayerError::Resource(format!(
                    "input {} selects missing slot {}",
                    input.accessor, input.slot
                )));
            }
        }
        let mut output_keys = Vec::with_capacity(desc.outputs.len());
        for output in desc.outputs {
            self.check_owner(output.buffer.owner)?;
            if output.buffer.index_space != desc.index_space {
                return Err(LayerError::Resource(format!(
                    "output {} index space differs from kernel {}",
                    output.field, desc.name
                )));
            }
            if output.buffer.slots.get(output.slot).is_none() {
                return Err(LayerError::Resource(format!(
                    "output {} selects missing slot {}",
                    output.field, output.slot
                )));
            }
            let key = (output.buffer.id, output.slot);
            if output_keys.contains(&key) {
                return Err(LayerError::Resource(
                    "MRT outputs alias the same slot".to_string(),
                ));
            }
            if desc
                .inputs
                .iter()
                .any(|input| (input.buffer.id, input.slot) == key)
            {
                return Err(LayerError::Resource(
                    "a dispatch cannot sample and render the same slot".to_string(),
                ));
            }
            output_keys.push(key);
        }
        let source = assemble(&desc)?;
        let uniform_binding = u32::try_from(desc.inputs.len()).map_err(|_| {
            LayerError::Resource("kernel has more inputs than binding indices".to_string())
        })?;
        self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(desc.name),
                source: wgpu::ShaderSource::Wgsl(source.clone().into()),
            });
        let mut layout_entries = Vec::with_capacity(desc.inputs.len() + 1);
        for binding in 0..desc.inputs.len() {
            let binding = u32::try_from(binding).map_err(|_| {
                LayerError::Resource("kernel has more inputs than binding indices".to_string())
            })?;
            layout_entries.push(wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
        }
        layout_entries.push(wgpu::BindGroupLayoutEntry {
            binding: uniform_binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(desc.uniform_size),
            },
            count: None,
        });
        let bind_layout = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(desc.name),
                entries: &layout_entries,
            });
        let uniform = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(desc.name),
            size: desc.uniform_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut bind_entries = Vec::with_capacity(desc.inputs.len() + 1);
        for (binding, input) in desc.inputs.iter().enumerate() {
            let binding = u32::try_from(binding).map_err(|_| {
                LayerError::Resource("kernel has more inputs than binding indices".to_string())
            })?;
            bind_entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(&input.buffer.slots[input.slot].view),
            });
        }
        bind_entries.push(wgpu::BindGroupEntry {
            binding: uniform_binding,
            resource: uniform.as_entire_binding(),
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(desc.name),
            layout: &bind_layout,
            entries: &bind_entries,
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(desc.name),
                bind_group_layouts: &[&bind_layout],
                push_constant_ranges: &[],
            });
        let targets: Vec<_> = desc
            .outputs
            .iter()
            .map(|_| {
                Some(wgpu::ColorTargetState {
                    format: SLOT_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(desc.name),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("layer_vertex"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("layer_fragment"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
        if let Some(error) = self.device.pop_error_scope().await {
            return Err(LayerError::Pipeline(error.to_string()));
        }
        let outputs = desc
            .outputs
            .iter()
            .map(|output| {
                output.buffer.slots[output.slot]
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default())
            })
            .collect();
        let (width, height) = desc.index_space.rect();
        Ok(Kernel {
            owner: self.owner,
            name: desc.name.to_string(),
            pipeline,
            bind_group,
            uniform,
            uniform_size: desc.uniform_size,
            outputs,
            width,
            height,
            source,
        })
    }

    /// Submits one frozen fragment-compute pass.
    ///
    /// # Errors
    ///
    /// Refuses wrong-device kernels, wrong-sized uniforms, and lost devices.
    pub fn dispatch(&self, kernel: &Kernel, uniform: &[u8]) -> Result<DispatchToken, LayerError> {
        self.check_owner(kernel.owner)?;
        if uniform.len() as u64 != kernel.uniform_size {
            return Err(LayerError::Resource(format!(
                "kernel {} expected {} uniform bytes, received {}",
                kernel.name,
                kernel.uniform_size,
                uniform.len()
            )));
        }
        self.queue.write_buffer(&kernel.uniform, 0, uniform);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&kernel.name),
            });
        let attachments: Vec<_> = kernel
            .outputs
            .iter()
            .map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            })
            .collect();
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&kernel.name),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&kernel.pipeline);
            pass.set_bind_group(0, &kernel.bind_group, &[]);
            let width = u16::try_from(kernel.width).map_err(|_| {
                LayerError::Resource("dispatch width exceeds WebGL2 viewport range".to_string())
            })?;
            let height = u16::try_from(kernel.height).map_err(|_| {
                LayerError::Resource("dispatch height exceeds WebGL2 viewport range".to_string())
            })?;
            pass.set_viewport(
                0.0,
                0.0,
                f32::from(width),
                f32::from(height),
                0.0,
                1.0,
            );
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        let mut current = self
            .generation
            .lock()
            .map_err(|_| LayerError::Resource("generation lock was poisoned".to_string()))?;
        let generation = current.wrapping_add(1);
        *current = generation;
        Ok(DispatchToken {
            owner: self.owner,
            generation,
        })
    }

    fn check_token(&self, token: DispatchToken) -> Result<(), LayerError> {
        self.check_owner(token.owner)?;
        let current = *self
            .generation
            .lock()
            .map_err(|_| LayerError::Resource("generation lock was poisoned".to_string()))?;
        if token.generation == current {
            Ok(())
        } else {
            Err(LayerError::StaleGeneration {
                observed: token.generation,
                current,
            })
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn arm_deadline(state: Arc<Mutex<CallbackState>>) -> Result<(), LayerError> {
        use wasm_bindgen::{JsCast, closure::Closure};
        let callback = Closure::once(move || finish_callback(&state, MapOutcome::Deadline));
        web_sys::window()
            .ok_or_else(|| LayerError::Mapping("window is unavailable".to_string()))?
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.as_ref().unchecked_ref(),
                COMPLETION_DEADLINE_MS,
            )
            .map_err(|error| LayerError::Mapping(format!("could not arm deadline: {error:?}")))?;
        callback.forget();
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn arm_deadline(_state: Arc<Mutex<CallbackState>>) {}

    async fn map(
        &self,
        buffer: &wgpu::Buffer,
        token: DispatchToken,
    ) -> Result<Vec<u8>, LayerError> {
        self.check_token(token)?;
        let slice = buffer.slice(..);
        let (future, state) = callback_pair();
        let callback_state = Arc::clone(&state);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            finish_callback(&callback_state, MapOutcome::Complete(result));
        });
        #[cfg(target_arch = "wasm32")]
        Self::arm_deadline(state)?;
        #[cfg(not(target_arch = "wasm32"))]
        Self::arm_deadline(state);
        match future.await {
            MapOutcome::Complete(Ok(())) => {}
            MapOutcome::Complete(Err(error)) => return Err(LayerError::Mapping(error.to_string())),
            #[cfg(target_arch = "wasm32")]
            MapOutcome::Deadline => return Err(LayerError::Deadline(COMPLETION_DEADLINE_MS)),
        }
        self.check_token(token)?;
        let bytes = slice.get_mapped_range().to_vec();
        buffer.unmap();
        Ok(bytes)
    }

    /// Waits for a submitted generation using an ordered four-byte mapped-copy fence.
    ///
    /// # Errors
    ///
    /// Returns on stale generation, device loss, mapping failure, or deadline expiry.
    pub async fn complete(&self, token: DispatchToken) -> Result<(), LayerError> {
        self.check_token(token)?;
        let fence = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fragment compute explicit completion fence"),
            size: 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fragment compute explicit completion fence"),
            });
        encoder.copy_buffer_to_buffer(&self.marker, 0, &fence, 0, 4);
        self.queue.submit([encoder.finish()]);
        self.map(&fence, token).await.map(|_| ())
    }

    /// Copies and maps one logical output slot after a dispatch.
    ///
    /// # Errors
    ///
    /// Returns on wrong-device resources, stale generations, mapping failure, or deadline expiry.
    pub async fn read(
        &self,
        buffer: &ComputeBuffer,
        slot: usize,
        token: DispatchToken,
    ) -> Result<Vec<[f32; 4]>, LayerError> {
        self.check_owner(buffer.owner)?;
        self.check_token(token)?;
        let source = buffer
            .slots
            .get(slot)
            .ok_or_else(|| LayerError::Resource(format!("{} has no slot {slot}", buffer.label)))?;
        let (width, height) = buffer.index_space.rect();
        let packed_row = width * 16;
        let aligned_row = packed_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fragment compute texture readback"),
            size: u64::from(aligned_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fragment compute texture readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &source.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let bytes = self.map(&readback, token).await?;
        let mut values = Vec::with_capacity(buffer.index_space.len() as usize);
        for row in 0..height as usize {
            let start = row * aligned_row as usize;
            let row_bytes = &bytes[start..start + packed_row as usize];
            for texel in row_bytes.as_chunks::<16>().0 {
                let mut value = [0.0_f32; 4];
                for (component, bytes) in value.iter_mut().zip(texel.as_chunks::<4>().0) {
                    *component = f32::from_ne_bytes(bytes.try_into().map_err(|_| {
                        LayerError::Mapping("readback component had the wrong size".to_string())
                    })?);
                }
                values.push(value);
            }
        }
        values.truncate(buffer.index_space.len() as usize);
        Ok(values)
    }

    /// Runs the known one-texel kernel and stores its mapped checksum.
    ///
    /// # Errors
    ///
    /// Returns if resource creation, registration, dispatch, completion, or checksum validation fails.
    pub async fn golden_self_test(&mut self) -> Result<(), LayerError> {
        let input_values = [[1.25_f32, -2.0, 0.5, 4.0]];
        let initial: [&[[f32; 4]]; 1] = [&input_values];
        let input = self.create_buffer("golden input", IndexSpace::Grid1D(1), 1, Some(&initial))?;
        let output = self.create_buffer("golden output", IndexSpace::Grid1D(1), 1, None)?;
        let inputs = [InputBinding {
            accessor: "load_golden",
            buffer: &input,
            slot: 0,
        }];
        let outputs = [OutputBinding {
            field: "value",
            buffer: &output,
            slot: 0,
        }];
        let kernel = self
            .create_kernel(KernelDesc {
                name: "golden",
                body: GOLDEN_BODY,
                index_space: IndexSpace::Grid1D(1),
                inputs: &inputs,
                outputs: &outputs,
                uniform_type: "GoldenUniform",
                uniform_size: 16,
            })
            .await?;
        let bias = [0.25_f32, 1.0, -0.5, 2.0];
        let token = self.dispatch(&kernel, bytemuck::cast_slice(&bias))?;
        let values = self.read(&output, 0, token).await?;
        let expected = [2.75_f32, -3.0, 0.5, 10.0];
        for (actual, expected) in values[0].iter().zip(expected) {
            if (*actual - expected).abs() > 1.0e-6 {
                return Err(LayerError::Capability(format!(
                    "golden RGBA32F result {:?} differs from {:?}",
                    values[0], expected
                )));
            }
        }
        self.facts.golden_checksum = Some(values[0].iter().map(|value| f64::from(*value)).sum());
        Ok(())
    }
}

impl fmt::Debug for Kernel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Kernel")
            .field("name", &self.name)
            .field("uniform_size", &self.uniform_size)
            .field("outputs", &self.outputs.len())
            .field("rect", &(self.width, self.height))
            .finish_non_exhaustive()
    }
}

const GOLDEN_BODY: &str = r"
struct GoldenUniform { bias: vec4<f32>, }
struct GoldenResult { value: vec4<f32>, }
fn kernel(index: u32, uniforms: GoldenUniform) -> GoldenResult {
    var result: GoldenResult;
    result.value = load_golden(index) * 2.0 + uniforms.bias;
    return result;
}
";

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(body: &str) -> ForbiddenConstruct {
        match prescan("refusal", body, &[]) {
            Err(DialectError::Forbidden { construct, .. }) => construct,
            result => panic!("expected typed refusal, received {result:?}"),
        }
    }

    #[test]
    fn dialect_refuses_workgroup_storage_at_registration() {
        assert_eq!(
            refusal("var<workgroup> shared: array<u32, 4>;"),
            ForbiddenConstruct::WorkgroupVariable
        );
    }

    #[test]
    fn dialect_refuses_raw_storage_at_registration() {
        assert_eq!(
            refusal("@group(1) @binding(0) var<storage, read> raw: array<u32>;"),
            ForbiddenConstruct::RawStorageAccess
        );
    }

    #[test]
    fn dialect_refuses_atomics_at_registration() {
        assert_eq!(
            refusal("var<workgroup> shared: atomic<u32>;"),
            ForbiddenConstruct::Atomic
        );
    }

    #[test]
    fn dialect_refuses_barriers_at_registration() {
        assert_eq!(
            refusal("fn helper() { workgroupBarrier(); }"),
            ForbiddenConstruct::Barrier
        );
    }
}
