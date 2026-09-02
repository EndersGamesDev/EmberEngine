//! Heap-handle kernel dialect registration and page-dispatch validation.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use thiserror::Error;

use crate::{DataSpan, Handle, SpanArena, StaticHeaders};

const MAX_INPUTS: usize = 8;
const HANDLE_INDEX_MASK: u32 = (1 << 20) - 1;

/// Fixed shader capacities derived from the created uniform buffers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DialectLimits {
    /// Descriptor-table records visible to generated accessors.
    pub descriptor_capacity: u32,
    /// Span-directory records visible to generated accessors.
    pub span_capacity: u32,
    /// Page-handle slots visible to generated accessors.
    pub handle_capacity: u32,
}

/// Registration descriptor containing names and shape, but no heap handles.
pub struct KernelDesc<'a> {
    /// Stable diagnostic and pipeline name.
    pub name: &'a str,
    /// Entry-point-free WGSL declarations and `kernel` function.
    pub body: &'a str,
    /// Generated `load_name(index)` accessor suffixes in argument order.
    pub accessors: &'a [&'a str],
    /// Result-structure fields mapped to MRT locations.
    pub output_fields: &'a [&'a str],
    /// Author-declared WGSL uniform type.
    pub uniform_type: &'a str,
    /// Exact uniform byte count.
    pub uniform_size: u32,
    /// Constant buddy page side required by every output span.
    pub output_page_side: u16,
}

/// One forbidden operation recognized in a parsed author body.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ForbiddenConstruct {
    /// Module-scope workgroup storage.
    #[error("workgroup variable")]
    WorkgroupVariable,
    /// Atomic type or operation.
    #[error("atomic operation")]
    Atomic,
    /// Workgroup or storage barrier.
    #[error("barrier")]
    Barrier,
    /// Raw storage resource declaration.
    #[error("raw storage access")]
    RawStorageAccess,
    /// Author-defined shader entry point.
    #[error("entry point")]
    EntryPoint,
    /// Author-defined resource binding.
    #[error("resource binding")]
    ResourceBinding,
}

/// Typed registration-time kernel-dialect failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialectError {
    /// Descriptor names or shapes violate the dialect contract.
    #[error("kernel {kernel} has an invalid descriptor: {message}")]
    InvalidDescriptor {
        /// Stable kernel name.
        kernel: String,
        /// Exact problem.
        message: String,
    },
    /// Author WGSL does not parse with generated accessor signatures.
    #[error("kernel {kernel} WGSL parse failed: {message}")]
    Parse {
        /// Stable kernel name.
        kernel: String,
        /// Naga diagnostic.
        message: String,
    },
    /// Parsed author WGSL contains a refused operation.
    #[error("kernel {kernel} refused forbidden {construct}")]
    Forbidden {
        /// Stable kernel name.
        kernel: String,
        /// Typed refused construct.
        construct: ForbiddenConstruct,
    },
    /// Generated full shader does not validate.
    #[error("kernel {kernel} assembled WGSL validation failed: {message}")]
    Validation {
        /// Stable kernel name.
        kernel: String,
        /// Naga diagnostic.
        message: String,
    },
}

/// Typed dispatch-time shape, resource, or alias failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DispatchError {
    /// Input count differs from registration.
    #[error("kernel expects {expected} inputs but dispatch supplied {actual}")]
    InputCount {
        /// Registered input count.
        expected: usize,
        /// Supplied input count.
        actual: usize,
    },
    /// Output count differs from registration.
    #[error("kernel expects {expected} outputs but dispatch supplied {actual}")]
    OutputCount {
        /// Registered output count.
        expected: usize,
        /// Supplied output count.
        actual: usize,
    },
    /// Uniform payload differs from its registration.
    #[error("kernel expects {expected} uniform bytes but dispatch supplied {actual}")]
    UniformSize {
        /// Registered byte count.
        expected: u32,
        /// Supplied byte count.
        actual: usize,
    },
    /// Outputs disagree on length or constant page class.
    #[error("output spans must have one nonzero logical length and identical page shape")]
    OutputShapeMismatch,
    /// An output uses a different buddy class than registration.
    #[error("output page class is {actual} records; kernel registered {expected}")]
    OutputClassMismatch {
        /// Registered records per page.
        expected: u32,
        /// Supplied records per page.
        actual: u32,
    },
    /// Input and output page handles alias.
    #[error("input and output spans alias descriptor handle {0:#010x}")]
    AliasedInputOutput(u32),
    /// A CPU-side generation check rejected a page handle.
    #[error("dispatch handle validation failed: {0}")]
    InvalidHandle(String),
    /// Static header count or offsets disagree with the output plan.
    #[error("static dispatch headers do not match output pages")]
    HeaderMismatch,
}

/// One fragment pass selected by a static dynamic-uniform offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagePass {
    /// Dynamic byte offset into the step header buffer.
    pub header_offset: u32,
    /// First logical result index.
    pub global_base: u32,
    /// Valid records written by the pass.
    pub valid_length: u32,
    /// One SCRATCH-to-DATA destination page per MRT output.
    pub destinations: Vec<Handle>,
}

/// Fully validated resource block and page sequence for one dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchPlan {
    /// Step-setup resource words: `{span index, logical length, 0, 0}` per input.
    pub resource_words: [[u32; 4]; MAX_INPUTS],
    /// Static page passes in logical order.
    pub passes: Vec<PagePass>,
    /// Shared logical output index space.
    pub logical_len: u32,
    /// Bytes copied on the GPU from SCRATCH into DATA.
    pub gpu_copy_bytes: u64,
}

/// Validated generated source and immutable pipeline shape.
#[derive(Clone, Debug)]
pub struct RegisteredKernel {
    name: String,
    source: String,
    input_count: usize,
    output_count: usize,
    uniform_size: u32,
    output_page_side: u16,
    output_page_records: u32,
}

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte == b'_' || byte.is_ascii_alphabetic())
        && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
}

fn parse_author(desc: &KernelDesc<'_>) -> Result<naga::Module, DialectError> {
    let mut source = String::new();
    for accessor in desc.accessors {
        writeln!(
            source,
            "fn load_{accessor}(index: u32) -> vec4<f32> {{ return vec4<f32>(f32(index) * 0.0); }}"
        )
        .map_err(|error| DialectError::Parse {
            kernel: desc.name.to_string(),
            message: error.to_string(),
        })?;
    }
    source.push_str(desc.body);
    naga::front::wgsl::parse_str(&source).map_err(|error| DialectError::Parse {
        kernel: desc.name.to_string(),
        message: error.emit_to_string(&source),
    })
}

fn reject_forbidden(desc: &KernelDesc<'_>, module: &naga::Module) -> Result<(), DialectError> {
    if !module.entry_points.is_empty() {
        return Err(DialectError::Forbidden {
            kernel: desc.name.to_string(),
            construct: ForbiddenConstruct::EntryPoint,
        });
    }
    if module
        .global_variables
        .iter()
        .any(|(_, variable)| variable.binding.is_some())
    {
        return Err(DialectError::Forbidden {
            kernel: desc.name.to_string(),
            construct: ForbiddenConstruct::ResourceBinding,
        });
    }
    let ir = format!("{module:#?}");
    for (needle, construct) in [
        ("Atomic(", ForbiddenConstruct::Atomic),
        ("Atomic {", ForbiddenConstruct::Atomic),
        ("Barrier(", ForbiddenConstruct::Barrier),
        ("WorkGroupUniformLoad", ForbiddenConstruct::Barrier),
        ("space: WorkGroup", ForbiddenConstruct::WorkgroupVariable),
        ("space: Storage", ForbiddenConstruct::RawStorageAccess),
    ] {
        if ir.contains(needle) {
            return Err(DialectError::Forbidden {
                kernel: desc.name.to_string(),
                construct,
            });
        }
    }
    Ok(())
}

fn invalid(desc: &KernelDesc<'_>, message: &str) -> DialectError {
    DialectError::InvalidDescriptor {
        kernel: desc.name.to_string(),
        message: message.to_string(),
    }
}

impl RegisteredKernel {
    /// Registers and validates one handle-based kernel without freezing resource handles.
    ///
    /// # Errors
    ///
    /// Returns a typed descriptor, parse, forbidden-construct, or generated-validation failure.
    pub fn register(desc: &KernelDesc<'_>, limits: DialectLimits) -> Result<Self, DialectError> {
        if !identifier(desc.name) || !identifier(desc.uniform_type) {
            return Err(invalid(
                desc,
                "kernel and uniform type must be WGSL identifiers",
            ));
        }
        if desc.accessors.len() > MAX_INPUTS
            || desc.accessors.iter().any(|name| !identifier(name))
            || desc.output_fields.iter().any(|name| !identifier(name))
        {
            return Err(invalid(
                desc,
                "accessor and result names must be unique WGSL identifiers",
            ));
        }
        let names: BTreeSet<_> = desc.accessors.iter().chain(desc.output_fields).collect();
        if names.len() != desc.accessors.len() + desc.output_fields.len() {
            return Err(invalid(
                desc,
                "accessor and result names must be unique WGSL identifiers",
            ));
        }
        if desc.output_fields.is_empty() || desc.output_fields.len() > 4 {
            return Err(invalid(desc, "output count must be one through four"));
        }
        if desc.uniform_size == 0 || !desc.uniform_size.is_multiple_of(16) {
            return Err(invalid(
                desc,
                "uniform size must be a nonzero multiple of 16 bytes",
            ));
        }
        if desc.output_page_side == 0 || !desc.output_page_side.is_power_of_two() {
            return Err(invalid(
                desc,
                "output page side must be a nonzero power of two",
            ));
        }
        if limits.descriptor_capacity < 2
            || limits.span_capacity == 0
            || limits.handle_capacity == 0
        {
            return Err(invalid(
                desc,
                "shader capacities must be nonzero and include a live descriptor",
            ));
        }
        let author = parse_author(desc)?;
        reject_forbidden(desc, &author)?;
        let source = assemble(desc, limits)?;
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
        Ok(Self {
            name: desc.name.to_string(),
            source,
            input_count: desc.accessors.len(),
            output_count: desc.output_fields.len(),
            uniform_size: desc.uniform_size,
            output_page_side: desc.output_page_side,
            output_page_records: u32::from(desc.output_page_side).pow(2),
        })
    }

    /// Stable kernel name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Fully generated WGSL consumed by the immutable render pipeline.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Number of RGBA32F outputs written through MRT.
    #[must_use]
    pub const fn output_count(&self) -> usize {
        self.output_count
    }

    /// Exact uniform payload size in bytes.
    #[must_use]
    pub const fn uniform_size(&self) -> u32 {
        self.uniform_size
    }

    /// Constant square side of every output page.
    #[must_use]
    pub const fn output_page_side(&self) -> u16 {
        self.output_page_side
    }

    /// Validates dynamic span handles and constructs the static page sequence.
    ///
    /// # Errors
    ///
    /// Returns a typed count, shape, uniform, alias, header, or generation failure.
    pub fn plan_dispatch(
        &self,
        arena: &SpanArena,
        inputs: &[&DataSpan],
        outputs: &[&DataSpan],
        uniform_bytes: &[u8],
        headers: &StaticHeaders,
    ) -> Result<DispatchPlan, DispatchError> {
        if inputs.len() != self.input_count {
            return Err(DispatchError::InputCount {
                expected: self.input_count,
                actual: inputs.len(),
            });
        }
        if outputs.len() != self.output_count {
            return Err(DispatchError::OutputCount {
                expected: self.output_count,
                actual: outputs.len(),
            });
        }
        if uniform_bytes.len() != self.uniform_size as usize {
            return Err(DispatchError::UniformSize {
                expected: self.uniform_size,
                actual: uniform_bytes.len(),
            });
        }
        let first = outputs.first().ok_or(DispatchError::OutputShapeMismatch)?;
        if first.logical_len == 0
            || outputs.iter().any(|span| {
                span.logical_len != first.logical_len
                    || span.page_records != first.page_records
                    || span.page_count != first.page_count
            })
        {
            return Err(DispatchError::OutputShapeMismatch);
        }
        if first.page_records != self.output_page_records {
            return Err(DispatchError::OutputClassMismatch {
                expected: self.output_page_records,
                actual: first.page_records,
            });
        }
        if headers.offsets.len() != first.page_count as usize {
            return Err(DispatchError::HeaderMismatch);
        }
        let input_handles: BTreeSet<_> = inputs
            .iter()
            .flat_map(|span| span.handles())
            .map(|handle| handle.raw())
            .collect();
        for span in inputs.iter().chain(outputs) {
            for handle in span.handles() {
                arena
                    .heap()
                    .resolve(*handle)
                    .map_err(|error| DispatchError::InvalidHandle(error.to_string()))?;
                if outputs.contains(span) && input_handles.contains(&handle.raw()) {
                    return Err(DispatchError::AliasedInputOutput(handle.raw()));
                }
            }
        }
        let mut resource_words = [[0_u32; 4]; MAX_INPUTS];
        for (words, span) in resource_words.iter_mut().zip(inputs) {
            *words = [span.directory_index, span.logical_len, 0, 0];
        }
        let mut passes = Vec::with_capacity(first.page_count as usize);
        for page in 0..first.page_count {
            let global_base = page * first.page_records;
            passes.push(PagePass {
                header_offset: headers.offsets[page as usize],
                global_base,
                valid_length: (first.logical_len - global_base).min(first.page_records),
                destinations: outputs
                    .iter()
                    .map(|span| span.handles()[page as usize])
                    .collect(),
            });
        }
        Ok(DispatchPlan {
            resource_words,
            passes,
            logical_len: first.logical_len,
            gpu_copy_bytes: u64::from(first.logical_len) * outputs.len() as u64 * 16,
        })
    }
}

fn assemble(desc: &KernelDesc<'_>, limits: DialectLimits) -> Result<String, DialectError> {
    let handle_groups = limits.handle_capacity.div_ceil(4);
    let mut source = format!(
        "struct HeapDescriptors {{ entries: array<vec4<u32>, {}>, }}\nstruct HeapDirectory {{ spans: array<vec4<u32>, {}>, handles: array<vec4<u32>, {}>, }}\nstruct HeapResources {{ inputs: array<vec4<u32>, {MAX_INPUTS}>, }}\nstruct HeapHeader {{ global_base: u32, valid_length: u32, padding: vec2<u32>, }}\n@group(0) @binding(0) var heap_data: texture_2d_array<f32>;\n@group(0) @binding(1) var<uniform> heap_descriptors: HeapDescriptors;\n@group(0) @binding(2) var<uniform> heap_directory: HeapDirectory;\n@group(0) @binding(3) var<uniform> heap_header: HeapHeader;\n@group(0) @binding(4) var<uniform> heap_resources: HeapResources;\n",
        limits.descriptor_capacity, limits.span_capacity, handle_groups
    );
    source.push_str("fn heap_handle(slot: u32) -> u32 { return heap_directory.handles[slot / 4u][slot % 4u]; }\n");
    for (slot, accessor) in desc.accessors.iter().enumerate() {
        writeln!(
            source,
            "fn load_{accessor}(index: u32) -> vec4<f32> {{ let selected = heap_resources.inputs[{slot}]; if (index >= selected.y) {{ return vec4<f32>(0.0); }} let span = heap_directory.spans[selected.x]; let page = index / span.x; let local = index % span.x; let heap_id = heap_handle(span.z + page); let descriptor = heap_descriptors.entries[heap_id & {HANDLE_INDEX_MASK}u]; let width = descriptor.y >> 16u; let origin = vec2<u32>(descriptor.x >> 16u, descriptor.y & 65535u); let coordinate = origin + vec2<u32>(local % width, local / width); return textureLoad(heap_data, vec2<i32>(coordinate), i32(descriptor.x & 65535u), 0); }}"
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: error.to_string(),
        })?;
    }
    source.push_str(desc.body);
    source.push('\n');
    writeln!(
        source,
        "@group(0) @binding(5) var<uniform> kernel_uniforms: {};",
        desc.uniform_type
    )
    .map_err(|error| DialectError::Validation {
        kernel: desc.name.to_string(),
        message: error.to_string(),
    })?;
    source.push_str("struct HeapVertexOut { @builtin(position) position: vec4<f32>, }\n@vertex fn heap_kernel_vertex(@builtin(vertex_index) vertex: u32) -> HeapVertexOut { var points = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0)); var output: HeapVertexOut; output.position = vec4(points[vertex], 0.0, 1.0); return output; }\nstruct HeapFragmentOut {\n");
    for (location, field) in desc.output_fields.iter().enumerate() {
        writeln!(
            source,
            "@location({location}) output_{location}: vec4<f32>, // {field}"
        )
        .map_err(|error| DialectError::Validation {
            kernel: desc.name.to_string(),
            message: error.to_string(),
        })?;
    }
    source.push_str("}\n@fragment fn heap_kernel_fragment(@builtin(position) position: vec4<f32>) -> HeapFragmentOut {\n");
    writeln!(
        source,
        "let local = u32(position.y) * {}u + u32(position.x); if (local >= heap_header.valid_length) {{ discard; }} let result = kernel(heap_header.global_base + local, kernel_uniforms); var output: HeapFragmentOut;",
        desc.output_page_side
    )
    .map_err(|error| DialectError::Validation {
        kernel: desc.name.to_string(),
        message: error.to_string(),
    })?;
    for (location, field) in desc.output_fields.iter().enumerate() {
        writeln!(source, "output.output_{location} = result.{field};").map_err(|error| {
            DialectError::Validation {
                kernel: desc.name.to_string(),
                message: error.to_string(),
            }
        })?;
    }
    source.push_str("return output;\n}\n");
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::{
        DialectError, DialectLimits, DispatchError, ForbiddenConstruct, KernelDesc,
        RegisteredKernel,
    };
    use crate::{SpanArena, StaticHeaders};

    const BODY: &str = r"
struct Parameters { scale: f32, padding: vec3<f32>, }
struct ResultValue { value: vec4<f32>, }
fn kernel(index: u32, uniforms: Parameters) -> ResultValue {
    var result: ResultValue;
    result.value = load_source(index) * uniforms.scale;
    return result;
}
";

    fn register(page_side: u16) -> RegisteredKernel {
        RegisteredKernel::register(
            &KernelDesc {
                name: "scale",
                body: BODY,
                accessors: &["source"],
                output_fields: &["value"],
                uniform_type: "Parameters",
                uniform_size: 16,
                output_page_side: page_side,
            },
            DialectLimits {
                descriptor_capacity: 64,
                span_capacity: 16,
                handle_capacity: 32,
            },
        )
        .expect("valid dialect body registers")
    }

    #[test]
    fn generated_accessor_uses_its_dispatch_span_and_own_descriptor_width() {
        let kernel = register(8);
        assert!(
            kernel
                .source()
                .contains("selected = heap_resources.inputs[0]")
        );
        assert!(kernel.source().contains("let width = descriptor.y >> 16u"));
        assert!(!kernel.source().contains("output_page_width"));
        let module = naga::front::wgsl::parse_str(kernel.source()).expect("generated WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("generated WGSL validates");
    }

    #[test]
    fn forbidden_entry_point_and_raw_storage_are_typed_refusals() {
        for (body, expected) in [
            (
                "@compute @workgroup_size(1) fn kernel() {}",
                ForbiddenConstruct::EntryPoint,
            ),
            (
                "@group(1) @binding(0) var<storage, read> raw: array<u32>; fn kernel(index: u32, uniforms: Parameters) -> ResultValue { var result: ResultValue; result.value = vec4<f32>(f32(raw[index])); return result; } struct Parameters { value: vec4<f32>, } struct ResultValue { value: vec4<f32>, }",
                ForbiddenConstruct::ResourceBinding,
            ),
        ] {
            let error = RegisteredKernel::register(
                &KernelDesc {
                    name: "refused",
                    body,
                    accessors: &[],
                    output_fields: &["value"],
                    uniform_type: "Parameters",
                    uniform_size: 16,
                    output_page_side: 4,
                },
                DialectLimits {
                    descriptor_capacity: 8,
                    span_capacity: 4,
                    handle_capacity: 4,
                },
            )
            .expect_err("construct is outside dialect");
            assert!(matches!(
                error,
                DialectError::Forbidden { construct, .. } if construct == expected
            ));
        }
    }

    #[test]
    fn dispatch_replaces_handles_without_replacing_registered_source() {
        let mut arena =
            SpanArena::new(16, 4, 64, 16 * 16 + 4 * 32, 16).expect("arena configuration fits");
        let input_small = arena.allocate_span(17, 4).expect("input spans pages");
        let input_wide = arena
            .allocate_span(17, 8)
            .expect("replacement spans differently");
        let output = arena.allocate_span(130, 8).expect("output has three pages");
        let headers = StaticHeaders::for_span(&output, 256).expect("headers align");
        let kernel = register(8);
        let source = kernel.source().to_string();
        let first = kernel
            .plan_dispatch(&arena, &[&input_small], &[&output], &[0; 16], &headers)
            .expect("first handle dispatch validates");
        let second = kernel
            .plan_dispatch(&arena, &[&input_wide], &[&output], &[0; 16], &headers)
            .expect("replacement handle dispatch validates");
        assert_ne!(first.resource_words[0][0], second.resource_words[0][0]);
        assert_eq!(kernel.source(), source);
        assert_eq!(first.passes.len(), 3);
        assert_eq!(first.passes[2].valid_length, 2);
        assert_eq!(first.gpu_copy_bytes, 130 * 16);
    }

    #[test]
    fn dispatch_rejects_output_alias_and_shape_drift() {
        let mut arena =
            SpanArena::new(8, 4, 32, 16 * 8 + 4 * 16, 8).expect("arena configuration fits");
        let aliased = arena.allocate_span(20, 4).expect("span fits");
        let headers = StaticHeaders::for_span(&aliased, 256).expect("headers align");
        let kernel = register(4);
        assert!(matches!(
            kernel.plan_dispatch(&arena, &[&aliased], &[&aliased], &[0; 16], &headers),
            Err(DispatchError::AliasedInputOutput(_))
        ));
        let wrong_class = arena.allocate_span(20, 8).expect("wide span fits");
        let wrong_headers = StaticHeaders::for_span(&wrong_class, 256).expect("headers align");
        assert!(matches!(
            kernel.plan_dispatch(
                &arena,
                &[&aliased],
                &[&wrong_class],
                &[0; 16],
                &wrong_headers
            ),
            Err(DispatchError::OutputClassMismatch { .. })
        ));
    }
}
