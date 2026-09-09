use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Write as _};
use std::ops::Range;
use std::sync::{Arc, Mutex};

use minijinja::{Environment, ErrorKind, UndefinedBehavior};

use crate::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslType, WgslTypeDescription};

#[cfg(test)]
const INTERFACE_TEST_NAME: &str = "interface-test.wgsl.jinja";
#[cfg(test)]
const INTERFACE_TEST_SOURCE: &str = include_str!("../templates/interface-test.wgsl.jinja");
#[cfg(test)]
const INVALID_TEST_NAME: &str = "invalid-test.wgsl.jinja";
#[cfg(test)]
const INVALID_TEST_SOURCE: &str = include_str!("../templates/invalid-test.wgsl.jinja");
#[cfg(test)]
const INVALID_VALIDATION_TEST_NAME: &str = "invalid-validation-test.wgsl.jinja";
#[cfg(test)]
const INVALID_VALIDATION_TEST_SOURCE: &str =
    include_str!("../templates/invalid-validation-test.wgsl.jinja");
#[cfg(test)]
const HIDDEN_FILTER_TEST_NAME: &str = "hidden-filter-test.wgsl.jinja";
#[cfg(test)]
const HIDDEN_FILTER_TEST_SOURCE: &str = include_str!("../templates/hidden-filter-test.wgsl.jinja");
#[cfg(test)]
const LITERAL_MARKER_TEST_NAME: &str = "literal-marker-test.wgsl.jinja";
#[cfg(test)]
const LITERAL_MARKER_TEST_SOURCE: &str =
    include_str!("../templates/literal-marker-test.wgsl.jinja");
#[cfg(test)]
const ORACLE_TEST_NAME: &str = "oracle-test.wgsl.jinja";
#[cfg(test)]
const ORACLE_TEST_SOURCE: &str = include_str!("../templates/oracle-test.wgsl.jinja");
#[cfg(test)]
const PROBE_TEST_NAME: &str = "probe-test.wgsl.jinja";
#[cfg(test)]
const PROBE_TEST_SOURCE: &str = include_str!("../templates/probe-test.wgsl.jinja");
#[cfg(test)]
const UNREGISTERED_TYPE_TEST_NAME: &str = "unregistered-type-test.wgsl.jinja";
#[cfg(test)]
const UNREGISTERED_TYPE_TEST_SOURCE: &str =
    include_str!("../templates/unregistered-type-test.wgsl.jinja");
#[cfg(test)]
const UNTRACED_TYPE_TEST_NAME: &str = "untraced-type-test.wgsl.jinja";
#[cfg(test)]
const UNTRACED_TYPE_TEST_SOURCE: &str = include_str!("../templates/untraced-type-test.wgsl.jinja");
#[cfg(test)]
const UNTRACED_ENUM_TEST_NAME: &str = "untraced-enum-test.wgsl.jinja";
#[cfg(test)]
const UNTRACED_ENUM_TEST_SOURCE: &str = include_str!("../templates/untraced-enum-test.wgsl.jinja");
#[cfg(test)]
const UNTRACED_BINDING_TEST_NAME: &str = "untraced-binding-test.wgsl.jinja";
#[cfg(test)]
const UNTRACED_BINDING_TEST_SOURCE: &str =
    include_str!("../templates/untraced-binding-test.wgsl.jinja");
#[cfg(test)]
const UNTRACED_CONSTANT_TEST_NAME: &str = "untraced-constant-test.wgsl.jinja";
#[cfg(test)]
const UNTRACED_CONSTANT_TEST_SOURCE: &str =
    include_str!("../templates/untraced-constant-test.wgsl.jinja");
#[cfg(test)]
const REVERSED_EMISSION_TEST_NAME: &str = "reversed-emission-test.wgsl.jinja";
#[cfg(test)]
const REVERSED_EMISSION_TEST_SOURCE: &str =
    include_str!("../templates/reversed-emission-test.wgsl.jinja");
const EMBEDDED_TEMPLATES: &[(&str, &str)] = &[];
#[cfg(test)]
const TEST_TEMPLATES: &[(&str, &str)] = &[
    (INTERFACE_TEST_NAME, INTERFACE_TEST_SOURCE),
    (INVALID_TEST_NAME, INVALID_TEST_SOURCE),
    (INVALID_VALIDATION_TEST_NAME, INVALID_VALIDATION_TEST_SOURCE),
    (HIDDEN_FILTER_TEST_NAME, HIDDEN_FILTER_TEST_SOURCE),
    (LITERAL_MARKER_TEST_NAME, LITERAL_MARKER_TEST_SOURCE),
    (ORACLE_TEST_NAME, ORACLE_TEST_SOURCE),
    (PROBE_TEST_NAME, PROBE_TEST_SOURCE),
    (REVERSED_EMISSION_TEST_NAME, REVERSED_EMISSION_TEST_SOURCE),
    (UNREGISTERED_TYPE_TEST_NAME, UNREGISTERED_TYPE_TEST_SOURCE),
    (UNTRACED_TYPE_TEST_NAME, UNTRACED_TYPE_TEST_SOURCE),
    (UNTRACED_ENUM_TEST_NAME, UNTRACED_ENUM_TEST_SOURCE),
    (UNTRACED_BINDING_TEST_NAME, UNTRACED_BINDING_TEST_SOURCE),
    (UNTRACED_CONSTANT_TEST_NAME, UNTRACED_CONSTANT_TEST_SOURCE),
];

const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;
const EMISSION_MARKER_PREFIX: &str = "__EMBER_WGSL_EMISSION_";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EmissionKind {
    Structure,
    Enumeration,
    Binding,
    Constant,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Emission {
    kind: EmissionKind,
    name: String,
    range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingEmission {
    kind: EmissionKind,
    name: String,
    declaration: String,
}

#[derive(Debug)]
struct EmissionRecorder {
    /// Per-render correlation tag; integrity comes from the checked marker prefix and exact body
    /// comparison, not from this value being secret.
    render_tag: String,
    pending: Vec<PendingEmission>,
}

impl EmissionRecorder {
    const fn new() -> Self {
        Self {
            render_tag: String::new(),
            pending: Vec::new(),
        }
    }

    fn record(&mut self, kind: EmissionKind, name: &str, declaration: &str) -> String {
        let id = self.pending.len();
        self.pending.push(PendingEmission {
            kind,
            name: name.to_owned(),
            declaration: declaration.to_owned(),
        });
        let mut marked = String::new();
        marked.push_str(&emission_marker(&self.render_tag, id, "START"));
        marked.push_str(declaration);
        marked.push_str(&emission_marker(&self.render_tag, id, "END"));
        marked
    }
}

/// One bind-group and binding-number pair owned by Rust pipeline setup.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WgslBinding {
    /// Bind-group number.
    pub group: u32,
    /// Binding number within the group.
    pub binding: u32,
}

/// One typed constant supplied to a shader template by Rust.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShaderConstant {
    /// A WGSL `i32` constant.
    Signed(i32),
    /// A WGSL `u32` constant.
    Unsigned(u32),
    /// A finite WGSL `f32` constant.
    Float(f32),
    /// A finite WGSL `vec4<f32>` constant.
    Float4([f32; 4]),
}

/// Validated WGSL source and its deterministic content hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedShader {
    source: String,
    hash: u64,
}

impl RenderedShader {
    /// Returns the rendered WGSL source.
    ///
    /// Native rendering has already validated this source with naga. On wasm, wgpu validates it
    /// when the pipeline owner creates the shader module.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the stable FNV-1a hash of [`Self::source`].
    #[must_use]
    pub const fn hash(&self) -> u64 {
        self.hash
    }
}

/// A registration, template-rendering, or WGSL validation failure.
#[derive(Debug)]
pub enum RenderError {
    /// One name was registered with two different descriptions.
    ConflictingRegistration {
        /// Registry category.
        category: &'static str,
        /// Conflicting name.
        name: &'static str,
    },
    /// A non-finite Rust value cannot be emitted as a WGSL floating-point literal.
    NonFiniteConstant {
        /// Constant name.
        name: &'static str,
    },
    /// Minijinja rejected or failed to render an embedded template.
    Template(minijinja::Error),
    /// Naga could not parse the rendered WGSL.
    #[cfg(not(target_arch = "wasm32"))]
    WgslParse {
        /// Embedded template name.
        template: String,
        /// One-based line in the rendered WGSL when naga supplied a span.
        line: Option<u32>,
        /// Text of the rendered WGSL line when naga supplied a usable line number.
        excerpt: Option<String>,
        /// Naga's source diagnostic.
        diagnostic: String,
    },
    /// Naga parsed the WGSL but rejected its shader semantics.
    #[cfg(not(target_arch = "wasm32"))]
    WgslValidation {
        /// Embedded template name.
        template: String,
        /// One-based line in the rendered WGSL when naga supplied a span.
        line: Option<u32>,
        /// Text of the rendered WGSL line when naga supplied a usable line number.
        excerpt: Option<String>,
        /// Naga's source diagnostic.
        diagnostic: String,
    },
    /// Parsed WGSL disagrees with a Rust-owned context registration.
    #[cfg(not(target_arch = "wasm32"))]
    ContextAudit {
        /// Embedded template name.
        template: String,
        /// Registered declaration that disagreed with the module.
        declaration: String,
        /// Exact disagreement.
        diagnostic: String,
    },
}

impl fmt::Display for RenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingRegistration { category, name } => {
                write!(formatter, "conflicting {category} registration `{name}`")
            }
            Self::NonFiniteConstant { name } => {
                write!(formatter, "WGSL constant `{name}` is not finite")
            }
            Self::Template(error) => write!(formatter, "shader template failed: {error:#}"),
            #[cfg(not(target_arch = "wasm32"))]
            Self::WgslParse {
                template,
                line,
                excerpt,
                diagnostic,
            } => write_naga_error(
                formatter,
                "WGSL parsing",
                template,
                *line,
                excerpt.as_deref(),
                diagnostic,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::WgslValidation {
                template,
                line,
                excerpt,
                diagnostic,
            } => write_naga_error(
                formatter,
                "WGSL validation",
                template,
                *line,
                excerpt.as_deref(),
                diagnostic,
            ),
            #[cfg(not(target_arch = "wasm32"))]
            Self::ContextAudit {
                template,
                declaration,
                diagnostic,
            } => write!(
                formatter,
                "WGSL declaration `{declaration}` disagrees with its Rust registration in template `{template}`: {diagnostic}"
            ),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Template(error) => Some(error),
            Self::ConflictingRegistration { .. } | Self::NonFiniteConstant { .. } => None,
            #[cfg(not(target_arch = "wasm32"))]
            Self::WgslParse { .. } | Self::WgslValidation { .. } | Self::ContextAudit { .. } => {
                None
            }
        }
    }
}

impl From<minijinja::Error> for RenderError {
    fn from(error: minijinja::Error) -> Self {
        Self::Template(error)
    }
}

/// Rust-owned inputs from which an embedded shader template is rendered.
#[derive(Clone, Debug, Default)]
pub struct ShaderContext {
    structures: BTreeMap<&'static str, WgslTypeDescription>,
    enumerations: BTreeMap<&'static str, WgslEnumDescription>,
    binding_slots: BTreeMap<&'static str, WgslBinding>,
    constant_values: BTreeMap<&'static str, ShaderConstant>,
}

impl ShaderContext {
    /// Creates an empty shader context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one bytemuck-safe Rust type for template use.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ConflictingRegistration`] when the WGSL name already describes a
    /// different Rust layout.
    pub fn register_type<T: WgslType>(&mut self) -> Result<(), RenderError> {
        register_once(
            &mut self.structures,
            T::DESCRIPTION.name,
            T::DESCRIPTION,
            "type",
        )
    }

    /// Registers one Rust enum for template use.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ConflictingRegistration`] when the WGSL name already describes
    /// different Rust discriminants.
    pub fn register_enum<T: WgslEnum>(&mut self) -> Result<(), RenderError> {
        register_once(
            &mut self.enumerations,
            T::DESCRIPTION.name,
            T::DESCRIPTION,
            "enum",
        )
    }

    /// Registers a named binding owned by Rust pipeline setup.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::ConflictingRegistration`] when `name` already identifies a different
    /// binding pair.
    pub fn register_binding(
        &mut self,
        name: &'static str,
        group: u32,
        binding: u32,
    ) -> Result<(), RenderError> {
        register_once(
            &mut self.binding_slots,
            name,
            WgslBinding { group, binding },
            "binding",
        )
    }

    /// Registers one typed shader constant.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::NonFiniteConstant`] for a non-finite float, or
    /// [`RenderError::ConflictingRegistration`] when `name` already identifies a different value.
    pub fn register_constant(
        &mut self,
        name: &'static str,
        value: ShaderConstant,
    ) -> Result<(), RenderError> {
        let finite = match value {
            ShaderConstant::Float(number) => number.is_finite(),
            ShaderConstant::Float4(numbers) => numbers.into_iter().all(f32::is_finite),
            ShaderConstant::Signed(_) | ShaderConstant::Unsigned(_) => true,
        };
        if !finite {
            return Err(RenderError::NonFiniteConstant { name });
        }
        register_once(&mut self.constant_values, name, value, "constant")
    }

    fn expand(&self, template_name: &str) -> Result<(String, Vec<Emission>), RenderError> {
        let recorder = Arc::new(Mutex::new(EmissionRecorder::new()));
        let render_tag = format!("{:p}", Arc::as_ptr(&recorder));
        recorder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .render_tag
            .clone_from(&render_tag);
        let environment = environment(self, &recorder)?;
        let marked_source = environment.get_template(template_name)?.render(())?;
        let (render_tag, pending) = {
            let recorder = recorder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (recorder.render_tag.clone(), recorder.pending.clone())
        };
        strip_emission_markers(template_name, &marked_source, &render_tag, &pending)
    }
}

/// Renders one embedded template and computes its stable source hash.
///
/// Native builds reject WGSL that naga cannot parse or validate. Wasm builds leave that validation
/// to wgpu's shader-module creation so the browser does not pay for a duplicate validator path.
///
/// # Errors
///
/// Returns [`RenderError::Template`] for template failures. On native builds it also returns
/// [`RenderError::WgslParse`] for WGSL syntax failures or [`RenderError::WgslValidation`] for
/// invalid shader semantics.
pub fn render(template_name: &str, context: &ShaderContext) -> Result<RenderedShader, RenderError> {
    let (source, trace) = context.expand(template_name)?;
    #[cfg(not(target_arch = "wasm32"))]
    validate_wgsl(template_name, &source, context, &trace)?;
    #[cfg(target_arch = "wasm32")]
    drop(trace);
    Ok(RenderedShader {
        hash: stable_hash(&source),
        source,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn write_naga_error(
    formatter: &mut fmt::Formatter<'_>,
    phase: &str,
    template: &str,
    line: Option<u32>,
    excerpt: Option<&str>,
    diagnostic: &str,
) -> fmt::Result {
    match line {
        Some(number) => {
            write!(
                formatter,
                "{phase} failed in template `{template}` at rendered WGSL line {number}:"
            )?;
            if let Some(source_line) = excerpt {
                write!(formatter, "\n{source_line}")?;
            }
            write!(formatter, "\n{diagnostic}")
        }
        None => write!(
            formatter,
            "{phase} failed in template `{template}`:\n{diagnostic}"
        ),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate_wgsl(
    template_name: &str,
    source: &str,
    context: &ShaderContext,
    trace: &[Emission],
) -> Result<(), RenderError> {
    let module = parse_and_validate_wgsl(template_name, source)?;
    audit_context(template_name, &module, context, trace)?;
    Ok(())
}

/// Parses and validates rendered WGSL for the production-template test macro.
///
/// # Errors
///
/// Returns [`RenderError::WgslParse`] for WGSL syntax or name-resolution failures and
/// [`RenderError::WgslValidation`] for invalid shader semantics.
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub fn validate_rendered_wgsl(template_name: &str, source: &str) -> Result<(), RenderError> {
    parse_and_validate_wgsl(template_name, source)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_and_validate_wgsl(template_name: &str, source: &str) -> Result<naga::Module, RenderError> {
    let module = naga::front::wgsl::parse_str(source).map_err(|error| {
        let number = error.location(source).map(|location| location.line_number);
        RenderError::WgslParse {
            template: template_name.to_owned(),
            line: number,
            excerpt: rendered_line(source, number),
            diagnostic: error.emit_to_string(source),
        }
    })?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|error| {
        let number = error.location(source).map(|location| location.line_number);
        RenderError::WgslValidation {
            template: template_name.to_owned(),
            line: number,
            excerpt: rendered_line(source, number),
            diagnostic: error.emit_to_string(source),
        }
    })?;
    Ok(module)
}

#[cfg(not(target_arch = "wasm32"))]
fn rendered_line(source: &str, number: Option<u32>) -> Option<String> {
    let index = usize::try_from(number?.checked_sub(1)?).ok()?;
    source.lines().nth(index).map(str::to_owned)
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_context(
    template_name: &str,
    module: &naga::Module,
    context: &ShaderContext,
    trace: &[Emission],
) -> Result<(), RenderError> {
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).map_err(|error| {
        context_audit_error(
            template_name,
            "module layout",
            format!("naga could not compute layouts: {error}"),
        )
    })?;

    audit_bound_globals(template_name, module, context, trace)?;
    audit_module_constants(template_name, module, trace)?;
    for description in context.structures.values() {
        audit_structure(template_name, module, &layouter, trace, description)?;
    }
    for description in context.enumerations.values() {
        audit_enumeration(template_name, module, trace, description)?;
    }
    for (&name, expected) in &context.binding_slots {
        audit_binding(template_name, module, trace, name, *expected)?;
    }
    for (&name, expected) in &context.constant_values {
        audit_constant(template_name, module, trace, name, *expected)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_bound_globals(
    template_name: &str,
    module: &naga::Module,
    context: &ShaderContext,
    trace: &[Emission],
) -> Result<(), RenderError> {
    for (handle, variable) in module.global_variables.iter() {
        if variable.binding.is_none() {
            continue;
        }
        let Some(name) = variable.name.as_deref() else {
            return Err(context_audit_error(
                template_name,
                "unnamed binding",
                "WGSL contains an unnamed bound global",
            ));
        };
        if !context.binding_slots.contains_key(name)
            || !has_emission(
                trace,
                EmissionKind::Binding,
                name,
                module.global_variables.get_span(handle),
            )
        {
            return Err(context_audit_error(
                template_name,
                name,
                format!(
                    "WGSL binding '{name}' was declared without a registered wgsl_binding emission"
                ),
            ));
        }

        let shader_type = &module.types[variable.ty];
        if matches!(&shader_type.inner, naga::TypeInner::Struct { .. }) {
            let structure_name = shader_type.name.as_deref().unwrap_or("unnamed structure");
            if !context.structures.contains_key(structure_name) {
                return Err(context_audit_error(
                    template_name,
                    structure_name,
                    format!(
                        "WGSL structure '{structure_name}' backs a resource binding without a registered wgsl_type emission"
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_module_constants(
    template_name: &str,
    module: &naga::Module,
    trace: &[Emission],
) -> Result<(), RenderError> {
    for (handle, constant) in module.constants.iter() {
        let name = constant.name.as_deref().unwrap_or("unnamed constant");
        let span = module.constants.get_span(handle);
        let traced = has_emission(trace, EmissionKind::Constant, name, span)
            || trace.iter().any(|entry| {
                entry.kind == EmissionKind::Enumeration
                    && ranges_overlap(&entry.range, span.to_range().as_ref())
            });
        if !traced {
            return Err(context_audit_error(
                template_name,
                name,
                format!(
                    "WGSL constant '{name}' was declared without a registered wgsl_constant or wgsl_enum emission"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn has_emission(trace: &[Emission], kind: EmissionKind, name: &str, span: naga::Span) -> bool {
    trace.iter().any(|entry| {
        entry.kind == kind
            && entry.name == name
            && ranges_overlap(&entry.range, span.to_range().as_ref())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn ranges_overlap(emission: &Range<usize>, declaration: Option<&Range<usize>>) -> bool {
    declaration.is_some_and(|declaration| {
        emission.start < declaration.end && declaration.start < emission.end
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_structure(
    template_name: &str,
    module: &naga::Module,
    layouter: &naga::proc::Layouter,
    trace: &[Emission],
    description: &WgslTypeDescription,
) -> Result<(), RenderError> {
    let Some((handle, shader_type)) = module
        .types
        .iter()
        .find(|(_, shader_type)| shader_type.name.as_deref() == Some(description.name))
    else {
        return Err(context_audit_error(
            template_name,
            description.name,
            "registered structure is missing from the parsed module",
        ));
    };
    let naga::TypeInner::Struct { members, .. } = &shader_type.inner else {
        return Err(context_audit_error(
            template_name,
            description.name,
            "registered type is not a WGSL structure",
        ));
    };
    if !has_emission(
        trace,
        EmissionKind::Structure,
        description.name,
        module.types.get_span(handle),
    ) {
        return Err(context_audit_error(
            template_name,
            description.name,
            format!(
                "WGSL structure '{}' was declared without a registered wgsl_type emission",
                description.name
            ),
        ));
    }
    if members.len() != description.fields.len() {
        return Err(context_audit_error(
            template_name,
            description.name,
            format!(
                "WGSL has {} fields but Rust registered {}",
                members.len(),
                description.fields.len()
            ),
        ));
    }

    for (shader_field, rust_field) in members.iter().zip(description.fields) {
        if shader_field.name.as_deref() != Some(rust_field.name) {
            return Err(context_audit_error(
                template_name,
                description.name,
                format!(
                    "WGSL field {:?} does not match Rust field `{}`",
                    shader_field.name, rust_field.name
                ),
            ));
        }
        let expected_offset = u32::try_from(rust_field.offset).map_err(|_| {
            context_audit_error(
                template_name,
                description.name,
                format!(
                    "Rust offset for `{}` exceeds WGSL's address space",
                    rust_field.name
                ),
            )
        })?;
        if shader_field.offset != expected_offset {
            return Err(context_audit_error(
                template_name,
                description.name,
                format!(
                    "WGSL field `{}` starts at byte {} but Rust starts it at byte {expected_offset}",
                    rust_field.name, shader_field.offset
                ),
            ));
        }
        let actual_type = shader_type_name(module, shader_field.ty);
        if actual_type.as_deref() != Some(rust_field.wgsl_type) {
            return Err(context_audit_error(
                template_name,
                description.name,
                format!(
                    "WGSL field `{}` has type {} but Rust registered `{}`",
                    rust_field.name,
                    actual_type.as_deref().unwrap_or("an unsupported WGSL type"),
                    rust_field.wgsl_type
                ),
            ));
        }
    }

    audit_structure_layout(template_name, layouter, handle, description)
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_structure_layout(
    template_name: &str,
    layouter: &naga::proc::Layouter,
    handle: naga::Handle<naga::Type>,
    description: &WgslTypeDescription,
) -> Result<(), RenderError> {
    let layout = &layouter[handle];
    let expected_size = u32::try_from(description.size).map_err(|_| {
        context_audit_error(
            template_name,
            description.name,
            "Rust size exceeds WGSL's address space",
        )
    })?;
    if layout.size != expected_size {
        return Err(context_audit_error(
            template_name,
            description.name,
            format!(
                "WGSL size is {} bytes but Rust size is {expected_size} bytes",
                layout.size
            ),
        ));
    }
    let expected_alignment = u32::try_from(description.alignment)
        .ok()
        .and_then(naga::proc::Alignment::new)
        .ok_or_else(|| {
            context_audit_error(
                template_name,
                description.name,
                "Rust alignment is not representable as a WGSL alignment",
            )
        })?;
    if layout.alignment != expected_alignment {
        return Err(context_audit_error(
            template_name,
            description.name,
            format!(
                "WGSL alignment is {} but Rust alignment is {}",
                layout.alignment, description.alignment
            ),
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn shader_type_name(module: &naga::Module, handle: naga::Handle<naga::Type>) -> Option<String> {
    let shader_type = &module.types[handle];
    match &shader_type.inner {
        naga::TypeInner::Scalar(scalar) => scalar_name(*scalar).map(str::to_owned),
        naga::TypeInner::Vector { size, scalar } => {
            let scalar = scalar_name(*scalar)?;
            Some(format!("vec{}<{scalar}>", vector_width(*size)))
        }
        naga::TypeInner::Array { base, size, .. } => {
            let base = shader_type_name(module, *base)?;
            let naga::ArraySize::Constant(length) = size else {
                return None;
            };
            Some(format!("array<{base}, {}>", length.get()))
        }
        naga::TypeInner::Struct { .. } => shader_type.name.clone(),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn scalar_name(scalar: naga::Scalar) -> Option<&'static str> {
    match (scalar.kind, scalar.width) {
        (naga::ScalarKind::Float, 4) => Some("f32"),
        (naga::ScalarKind::Sint, 4) => Some("i32"),
        (naga::ScalarKind::Uint, 4) => Some("u32"),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
const fn vector_width(size: naga::VectorSize) -> u8 {
    match size {
        naga::VectorSize::Bi => 2,
        naga::VectorSize::Tri => 3,
        naga::VectorSize::Quad => 4,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_enumeration(
    template_name: &str,
    module: &naga::Module,
    trace: &[Emission],
    description: &WgslEnumDescription,
) -> Result<(), RenderError> {
    for variant in description.variants {
        let enum_name = description.name;
        let variant_name = variant.name;
        let name = format!("{enum_name}_{variant_name}");
        let Some((handle, constant)) = module
            .constants
            .iter()
            .find(|(_, constant)| constant.name.as_deref() == Some(name.as_str()))
        else {
            return Err(context_audit_error(
                template_name,
                name,
                "registered enum value is missing from the parsed module",
            ));
        };
        if !has_emission(
            trace,
            EmissionKind::Enumeration,
            description.name,
            module.constants.get_span(handle),
        ) {
            return Err(context_audit_error(
                template_name,
                &name,
                format!(
                    "WGSL enum '{}' was declared without a registered wgsl_enum emission",
                    description.name
                ),
            ));
        }
        let expression = &module.global_expressions[constant.init];
        let matches = match (variant.discriminant, expression) {
            (
                WgslEnumDiscriminant::Signed(expected),
                naga::Expression::Literal(naga::Literal::I32(actual)),
            ) => expected == *actual,
            (
                WgslEnumDiscriminant::Unsigned(expected),
                naga::Expression::Literal(naga::Literal::U32(actual)),
            ) => expected == *actual,
            _ => false,
        };
        if !matches {
            return Err(context_audit_error(
                template_name,
                name,
                "parsed WGSL value does not equal the Rust discriminant",
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_binding(
    template_name: &str,
    module: &naga::Module,
    trace: &[Emission],
    name: &str,
    expected: WgslBinding,
) -> Result<(), RenderError> {
    let Some((handle, variable)) = module
        .global_variables
        .iter()
        .find(|(_, variable)| variable.name.as_deref() == Some(name))
    else {
        return Err(context_audit_error(
            template_name,
            name,
            "registered binding is missing from the parsed module",
        ));
    };
    let Some(actual) = variable.binding.as_ref() else {
        return Err(context_audit_error(
            template_name,
            name,
            "registered binding has no WGSL resource binding",
        ));
    };
    if !has_emission(
        trace,
        EmissionKind::Binding,
        name,
        module.global_variables.get_span(handle),
    ) {
        return Err(context_audit_error(
            template_name,
            name,
            format!(
                "WGSL binding '{name}' was declared without a registered wgsl_binding emission"
            ),
        ));
    }
    if actual.group != expected.group || actual.binding != expected.binding {
        return Err(context_audit_error(
            template_name,
            name,
            format!(
                "WGSL uses @group({}) @binding({}) but Rust registered @group({}) @binding({})",
                actual.group, actual.binding, expected.group, expected.binding
            ),
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn audit_constant(
    template_name: &str,
    module: &naga::Module,
    trace: &[Emission],
    name: &str,
    expected: ShaderConstant,
) -> Result<(), RenderError> {
    let Some((handle, constant)) = module
        .constants
        .iter()
        .find(|(_, constant)| constant.name.as_deref() == Some(name))
    else {
        return Err(context_audit_error(
            template_name,
            name,
            "registered constant is missing from the parsed module",
        ));
    };
    if !has_emission(
        trace,
        EmissionKind::Constant,
        name,
        module.constants.get_span(handle),
    ) {
        return Err(context_audit_error(
            template_name,
            name,
            format!(
                "WGSL constant '{name}' was declared without a registered wgsl_constant emission"
            ),
        ));
    }
    let expression = &module.global_expressions[constant.init];
    let matches = match (expected, expression) {
        (
            ShaderConstant::Signed(expected),
            naga::Expression::Literal(naga::Literal::I32(actual)),
        ) => expected == *actual,
        (
            ShaderConstant::Unsigned(expected),
            naga::Expression::Literal(naga::Literal::U32(actual)),
        ) => expected == *actual,
        (
            ShaderConstant::Float(expected),
            naga::Expression::Literal(naga::Literal::F32(actual)),
        ) => expected.to_bits() == actual.to_bits(),
        (ShaderConstant::Float4(expected), naga::Expression::Compose { components, .. }) => {
            float4_expression_matches(module, components, expected)
        }
        _ => false,
    };
    if !matches {
        return Err(context_audit_error(
            template_name,
            name,
            "parsed WGSL value does not equal the Rust constant",
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn float4_expression_matches(
    module: &naga::Module,
    components: &[naga::Handle<naga::Expression>],
    expected: [f32; 4],
) -> bool {
    let [x, y, z, w] = components else {
        return false;
    };
    [*x, *y, *z, *w]
        .into_iter()
        .zip(expected)
        .all(|(handle, expected)| {
            let naga::Expression::Literal(naga::Literal::F32(actual)) =
                &module.global_expressions[handle]
            else {
                return false;
            };
            actual.to_bits() == expected.to_bits()
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn context_audit_error(
    template_name: &str,
    declaration: impl Into<String>,
    diagnostic: impl Into<String>,
) -> RenderError {
    RenderError::ContextAudit {
        template: template_name.to_owned(),
        declaration: declaration.into(),
        diagnostic: diagnostic.into(),
    }
}

fn stable_hash(source: &str) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in source.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn register_once<T: Copy + PartialEq>(
    registry: &mut BTreeMap<&'static str, T>,
    name: &'static str,
    value: T,
    category: &'static str,
) -> Result<(), RenderError> {
    match registry.entry(name) {
        Entry::Vacant(slot) => {
            slot.insert(value);
            Ok(())
        }
        Entry::Occupied(slot) if *slot.get() == value => Ok(()),
        Entry::Occupied(_) => Err(RenderError::ConflictingRegistration { category, name }),
    }
}

fn environment(
    context: &ShaderContext,
    recorder: &Arc<Mutex<EmissionRecorder>>,
) -> Result<Environment<'static>, RenderError> {
    let mut environment = Environment::empty();
    #[cfg(any(test, feature = "template-debug"))]
    environment.set_debug(true);
    environment.set_keep_trailing_newline(true);
    environment.set_undefined_behavior(UndefinedBehavior::Strict);

    let structures = context.structures.clone();
    let type_recorder = Arc::clone(recorder);
    environment.add_filter(
        "wgsl_type",
        move |name: String| -> Result<String, minijinja::Error> {
            let description = structures
                .get(name.as_str())
                .ok_or_else(|| missing_registration("type", &name))?;
            Ok(record_emission(
                &type_recorder,
                EmissionKind::Structure,
                &name,
                &render_type(description),
            ))
        },
    );

    let variants = context.enumerations.clone();
    let enum_recorder = Arc::clone(recorder);
    environment.add_filter(
        "wgsl_enum",
        move |name: String| -> Result<String, minijinja::Error> {
            let description = variants
                .get(name.as_str())
                .ok_or_else(|| missing_registration("enum", &name))?;
            Ok(record_emission(
                &enum_recorder,
                EmissionKind::Enumeration,
                &name,
                &render_enum(description),
            ))
        },
    );

    let slots = context.binding_slots.clone();
    let binding_recorder = Arc::clone(recorder);
    environment.add_filter(
        "wgsl_binding",
        move |name: String| -> Result<String, minijinja::Error> {
            let WgslBinding { group, binding } = slots
                .get(name.as_str())
                .copied()
                .ok_or_else(|| missing_registration("binding", &name))?;
            Ok(record_emission(
                &binding_recorder,
                EmissionKind::Binding,
                &name,
                &format!("@group({group}) @binding({binding})"),
            ))
        },
    );

    let values = context.constant_values.clone();
    let constant_recorder = Arc::clone(recorder);
    environment.add_filter(
        "wgsl_constant",
        move |name: String| -> Result<String, minijinja::Error> {
            let value = values
                .get(name.as_str())
                .ok_or_else(|| missing_registration("constant", &name))?;
            Ok(record_emission(
                &constant_recorder,
                EmissionKind::Constant,
                &name,
                &render_constant(&name, *value),
            ))
        },
    );

    for &(name, source) in EMBEDDED_TEMPLATES {
        environment.add_template(name, source)?;
    }
    #[cfg(test)]
    for &(name, source) in TEST_TEMPLATES {
        environment.add_template(name, source)?;
    }
    Ok(environment)
}

fn record_emission(
    recorder: &Mutex<EmissionRecorder>,
    kind: EmissionKind,
    name: &str,
    declaration: &str,
) -> String {
    recorder
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .record(kind, name, declaration)
}

fn strip_emission_markers(
    template_name: &str,
    marked_source: &str,
    render_tag: &str,
    pending: &[PendingEmission],
) -> Result<(String, Vec<Emission>), RenderError> {
    let mut source = String::with_capacity(marked_source.len());
    let mut trace = Vec::with_capacity(pending.len());
    let mut seen = BTreeSet::new();
    let marker_prefix = emission_marker_prefix(render_tag);
    let mut cursor = 0;
    while let Some(offset) = marked_source[cursor..].find(&marker_prefix) {
        let start = cursor + offset;
        source.push_str(&marked_source[cursor..start]);
        let (id, start_marker) = pending
            .iter()
            .enumerate()
            .find_map(|(id, _)| {
                let marker = emission_marker(render_tag, id, "START");
                marked_source[start..]
                    .starts_with(&marker)
                    .then_some((id, marker))
            })
            .ok_or_else(|| {
                emission_trace_error(template_name, pending.len(), "an unrecorded marker remains")
            })?;
        if !seen.insert(id) {
            return Err(emission_trace_error(
                template_name,
                id,
                "start marker occurs more than once",
            ));
        }
        let pending_emission = &pending[id];
        let end_marker = emission_marker(render_tag, id, "END");
        let declaration_start = start + start_marker.len();
        let end = marked_source[declaration_start..]
            .find(&end_marker)
            .map(|offset| declaration_start + offset)
            .ok_or_else(|| emission_trace_error(template_name, id, "end marker is missing"))?;
        let emitted_body = &marked_source[declaration_start..end];
        if emitted_body != pending_emission.declaration.as_str() {
            return Err(emission_trace_error(
                template_name,
                id,
                "emission body differs from Rust rendering",
            ));
        }
        let range_start = source.len();
        source.push_str(emitted_body);
        trace.push(Emission {
            kind: pending_emission.kind,
            name: pending_emission.name.clone(),
            range: range_start..source.len(),
        });
        cursor = end + end_marker.len();
    }
    source.push_str(&marked_source[cursor..]);
    if let Some(id) = (0..pending.len()).find(|id| !seen.contains(id)) {
        return Err(emission_trace_error(
            template_name,
            id,
            "start marker is missing",
        ));
    }
    if source.contains(EMISSION_MARKER_PREFIX) {
        return Err(emission_trace_error(
            template_name,
            pending.len(),
            "an unrecorded marker remains",
        ));
    }
    extend_binding_emissions(&source, &mut trace);
    Ok((source, trace))
}

fn extend_binding_emissions(source: &str, trace: &mut [Emission]) {
    for entry in trace {
        if entry.kind != EmissionKind::Binding {
            continue;
        }
        let trailing = &source[entry.range.end..];
        let declaration = trailing.trim_start();
        if !declaration.starts_with("var") {
            continue;
        }
        if let Some(semicolon) = declaration.find(';') {
            let leading_whitespace = trailing.len() - declaration.len();
            entry.range.end += leading_whitespace + semicolon + 1;
        }
    }
}

fn emission_marker_prefix(render_tag: &str) -> String {
    format!("/*{EMISSION_MARKER_PREFIX}{render_tag}_")
}

fn emission_marker(render_tag: &str, id: usize, boundary: &str) -> String {
    format!("{}{id}_{boundary}__*/", emission_marker_prefix(render_tag))
}

fn emission_trace_error(template_name: &str, id: usize, diagnostic: &str) -> RenderError {
    minijinja::Error::new(
        ErrorKind::InvalidOperation,
        format!("template `{template_name}` corrupted WGSL emission {id}: {diagnostic}"),
    )
    .into()
}

fn missing_registration(category: &str, name: &str) -> minijinja::Error {
    minijinja::Error::new(
        ErrorKind::InvalidOperation,
        format!("WGSL {category} `{name}` is not registered"),
    )
}

fn render_type(description: &WgslTypeDescription) -> String {
    if description.fields.is_empty() {
        return description.name.to_owned();
    }
    let mut declaration = String::new();
    declaration.push_str("struct ");
    declaration.push_str(description.name);
    declaration.push_str(" {");
    for field in description.fields {
        declaration.push(' ');
        if let Some(size) = field.member_size {
            write!(declaration, "@size({size}) ").expect("String writes are infallible");
        }
        declaration.push_str(field.name);
        declaration.push_str(": ");
        declaration.push_str(field.wgsl_type);
        declaration.push(',');
    }
    declaration.push_str(" }");
    declaration
}

fn render_enum(description: &WgslEnumDescription) -> String {
    let mut declarations = String::new();
    for variant in description.variants {
        declarations.push_str("const ");
        declarations.push_str(description.name);
        declarations.push('_');
        declarations.push_str(variant.name);
        match variant.discriminant {
            WgslEnumDiscriminant::Signed(value) => {
                writeln!(declarations, ": i32 = {value}i;").expect("String writes are infallible");
            }
            WgslEnumDiscriminant::Unsigned(value) => {
                writeln!(declarations, ": u32 = {value}u;").expect("String writes are infallible");
            }
        }
    }
    declarations
}

fn render_constant(name: &str, value: ShaderConstant) -> String {
    match value {
        ShaderConstant::Signed(number) => format!("const {name}: i32 = {number}i;"),
        ShaderConstant::Unsigned(number) => format!("const {name}: u32 = {number}u;"),
        ShaderConstant::Float(number) => format!("const {name}: f32 = {number};"),
        ShaderConstant::Float4([x, y, z, w]) => {
            format!("const {name}: vec4<f32> = vec4<f32>({x}, {y}, {z}, {w});")
        }
    }
}

#[cfg(test)]
mod tests {
    use bytemuck::{Pod, Zeroable};

    use crate::{F32Vec4, U32Vec4};

    use super::{
        EmissionKind, HIDDEN_FILTER_TEST_NAME, INTERFACE_TEST_NAME, INVALID_TEST_NAME,
        INVALID_VALIDATION_TEST_NAME, LITERAL_MARKER_TEST_NAME, PendingEmission,
        REVERSED_EMISSION_TEST_NAME, RenderError, ShaderConstant, ShaderContext,
        UNTRACED_BINDING_TEST_NAME, UNTRACED_CONSTANT_TEST_NAME, UNTRACED_ENUM_TEST_NAME,
        UNTRACED_TYPE_TEST_NAME, emission_marker, render, stable_hash, strip_emission_markers,
    };

    #[cfg(not(target_arch = "wasm32"))]
    use super::shader_type_name;

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C, align(16))]
    struct TestUniform {
        colour: F32Vec4,
        flags: U32Vec4,
    }

    crate::impl_wgsl_struct!(TestUniform, "TestUniform", {
        colour: F32Vec4,
        flags: U32Vec4,
    });

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn fixed_array_type_names_match_rust_metadata() {
        let module =
            naga::front::wgsl::parse_str("struct ArrayUniform { values: array<vec4<f32>, 4>, }")
                .expect("the fixed-array test module parses");
        let array = module
            .types
            .iter()
            .find_map(|(_, shader_type)| {
                let naga::TypeInner::Struct { members, .. } = &shader_type.inner else {
                    return None;
                };
                if shader_type.name.as_deref() != Some("ArrayUniform") {
                    return None;
                }
                members.first().map(|member| member.ty)
            })
            .expect("the fixed-array field has a parsed type");
        assert_eq!(
            shader_type_name(&module, array).as_deref(),
            Some(<[F32Vec4; 4] as crate::WgslType>::DESCRIPTION.name),
        );
    }

    #[derive(Clone, Copy)]
    #[repr(u32)]
    enum TestMode {
        Preview = 2,
        Final = 9,
    }

    crate::impl_wgsl_enum!(TestMode, "TestMode", u32, { Preview, Final });

    fn test_context() -> ShaderContext {
        let mut context = ShaderContext::new();
        context
            .register_type::<TestUniform>()
            .expect("test uniform has one description");
        context
            .register_enum::<TestMode>()
            .expect("test enum has one description");
        context
            .register_binding("values", 0, 3)
            .expect("test binding has one description");
        context
            .register_constant("TEST_SCALE", ShaderConstant::Float(2.5))
            .expect("test constant has one description");
        context
    }

    #[test]
    fn embedded_template_uses_every_rust_owned_filter() {
        let shader = render(INTERFACE_TEST_NAME, &test_context())
            .expect("embedded interface template renders");
        let source = shader.source();
        assert!(source.contains("struct TestUniform { colour: vec4<f32>, flags: vec4<u32>, }"));
        assert!(source.contains("const TestMode_Preview: u32 = 2u;"));
        assert!(source.contains("const TestMode_Final: u32 = 9u;"));
        assert!(source.contains("@group(0) @binding(3) var values: texture_2d<f32>;"));
        assert!(source.contains("const TEST_SCALE: f32 = 2.5;"));
    }

    #[test]
    fn rendered_source_hash_uses_a_pinned_stable_algorithm() {
        assert_eq!(stable_hash("WGSL"), 7_755_207_365_939_263_476);
        let first = render(INTERFACE_TEST_NAME, &test_context()).expect("first render validates");
        let second = render(INTERFACE_TEST_NAME, &test_context()).expect("second render validates");
        assert_eq!(first.hash(), second.hash());
    }

    #[test]
    fn every_native_render_audits_the_registered_rust_layout() {
        let mut context = test_context();
        context
            .structures
            .get_mut("TestUniform")
            .expect("test uniform is registered")
            .size = 16;

        let error = render(INTERFACE_TEST_NAME, &context)
            .expect_err("a parsed layout that disagrees with Rust must fail");
        assert!(matches!(error, RenderError::ContextAudit { .. }));
    }

    #[test]
    fn declarations_must_overlap_their_named_filter_emissions() {
        for (template_name, expected) in [
            (UNTRACED_TYPE_TEST_NAME, "wgsl_type emission"),
            (UNTRACED_ENUM_TEST_NAME, "wgsl_enum emission"),
            (UNTRACED_BINDING_TEST_NAME, "wgsl_binding emission"),
            (UNTRACED_CONSTANT_TEST_NAME, "wgsl_constant"),
        ] {
            let error = render(template_name, &test_context())
                .expect_err("a declaration outside its filter emission must fail");
            assert!(
                error.to_string().contains(expected),
                "{template_name} returned the wrong audit error: {error}"
            );
        }
    }

    #[test]
    fn a_hidden_filter_result_cannot_attest_a_declaration() {
        let error = render(HIDDEN_FILTER_TEST_NAME, &test_context())
            .expect_err("a hidden filter emission must not attest hand-written source");
        assert!(error.to_string().contains("start marker is missing"));
    }

    #[test]
    fn a_literal_marker_cannot_attest_a_declaration() {
        let error = render(LITERAL_MARKER_TEST_NAME, &test_context())
            .expect_err("a marker literal must not create an emission");
        assert!(error.to_string().contains("an unrecorded marker remains"));
    }

    #[test]
    fn an_emission_body_must_equal_the_rust_rendering() {
        let pending = [PendingEmission {
            kind: EmissionKind::Constant,
            name: "TEST_SCALE".to_owned(),
            declaration: "const TEST_SCALE: f32 = 2.5;".to_owned(),
        }];
        let marked = format!(
            "{}const TEST_SCALE: f32 = 9.5;{}",
            emission_marker("opaque", 0, "START"),
            emission_marker("opaque", 0, "END")
        );
        let error = strip_emission_markers("forged.wgsl.jinja", &marked, "opaque", &pending)
            .expect_err("mutating bytes inside a genuine marker must fail");
        assert!(
            error
                .to_string()
                .contains("emission body differs from Rust rendering")
        );
    }

    #[test]
    fn emissions_may_be_output_in_a_different_order_than_their_filters_run() {
        let shader = render(REVERSED_EMISSION_TEST_NAME, &test_context())
            .expect("reverse-order declaration output validates");
        let enum_position = shader
            .source()
            .find("const TestMode_Preview")
            .expect("the enum declaration is emitted");
        let type_position = shader
            .source()
            .find("struct TestUniform")
            .expect("the type declaration is emitted");
        assert!(enum_position < type_position);
    }

    #[test]
    fn naga_parse_error_names_the_rendered_wgsl_line() {
        let error = render(INVALID_TEST_NAME, &test_context()).expect_err("invalid WGSL must fail");
        assert!(matches!(
            &error,
            RenderError::WgslParse { line: Some(2), .. }
        ));
        let message = error.to_string();
        assert!(message.contains(INVALID_TEST_NAME));
        assert!(message.contains("rendered WGSL line 2:\nfn broken(: f32) {}"));
    }

    #[test]
    fn naga_validation_error_names_the_rendered_wgsl_line() {
        let error = render(INVALID_VALIDATION_TEST_NAME, &test_context())
            .expect_err("invalid shader semantics must fail");
        assert!(matches!(
            &error,
            RenderError::WgslValidation { line: Some(_), .. }
        ));
        let message = error.to_string();
        assert!(message.contains(INVALID_VALIDATION_TEST_NAME));
        assert!(message.contains("rendered WGSL line"));
        assert!(message.contains("var missing_binding: texture_2d<f32>;"));
    }
}
