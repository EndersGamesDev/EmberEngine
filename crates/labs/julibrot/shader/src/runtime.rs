use std::collections::{BTreeMap, btree_map::Entry};
use std::error::Error;
use std::fmt::{self, Write as _};

use minijinja::{Environment, ErrorKind, UndefinedBehavior};

use crate::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslType, WgslTypeDescription};

const INTERFACE_TEST_NAME: &str = "interface-test.wgsl.jinja";
const INTERFACE_TEST_SOURCE: &str = include_str!("../templates/interface-test.wgsl.jinja");
const EMBEDDED_TEMPLATES: [(&str, &str); 1] = [(INTERFACE_TEST_NAME, INTERFACE_TEST_SOURCE)];

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
}

/// A registration or template-rendering failure.
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
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Template(error) => Some(error),
            Self::ConflictingRegistration { .. } | Self::NonFiniteConstant { .. } => None,
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
        if let ShaderConstant::Float(number) = value
            && !number.is_finite()
        {
            return Err(RenderError::NonFiniteConstant { name });
        }
        register_once(&mut self.constant_values, name, value, "constant")
    }

    /// Renders one named embedded template through this context's filters.
    ///
    /// This stage performs template expansion only. The crate's validated `render` entry point is
    /// added with naga validation in the next structural step and is the path consumers use.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::Template`] when the embedded template is missing, invalid, or refers
    /// to a name absent from this context.
    pub fn render_template(&self, template_name: &str) -> Result<String, RenderError> {
        let environment = environment(self)?;
        Ok(environment.get_template(template_name)?.render(())?)
    }
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

fn environment(context: &ShaderContext) -> Result<Environment<'static>, RenderError> {
    let mut environment = Environment::empty();
    environment.set_debug(true);
    environment.set_undefined_behavior(UndefinedBehavior::Strict);

    let structures = context.structures.clone();
    environment.add_filter(
        "wgsl_type",
        move |name: String| -> Result<String, minijinja::Error> {
            let description = structures
                .get(name.as_str())
                .ok_or_else(|| missing_registration("type", &name))?;
            Ok(render_type(description))
        },
    );

    let variants = context.enumerations.clone();
    environment.add_filter(
        "wgsl_enum",
        move |name: String| -> Result<String, minijinja::Error> {
            let description = variants
                .get(name.as_str())
                .ok_or_else(|| missing_registration("enum", &name))?;
            Ok(render_enum(description))
        },
    );

    let slots = context.binding_slots.clone();
    environment.add_filter(
        "wgsl_binding",
        move |name: String| -> Result<String, minijinja::Error> {
            let WgslBinding { group, binding } = slots
                .get(name.as_str())
                .copied()
                .ok_or_else(|| missing_registration("binding", &name))?;
            Ok(format!("@group({group}) @binding({binding})"))
        },
    );

    let values = context.constant_values.clone();
    environment.add_filter(
        "wgsl_constant",
        move |name: String| -> Result<String, minijinja::Error> {
            let value = values
                .get(name.as_str())
                .ok_or_else(|| missing_registration("constant", &name))?;
            Ok(render_constant(&name, *value))
        },
    );

    for (name, source) in EMBEDDED_TEMPLATES {
        environment.add_template(name, source)?;
    }
    Ok(environment)
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
    }
}

#[cfg(test)]
mod tests {
    use bytemuck::{Pod, Zeroable};

    use super::{INTERFACE_TEST_NAME, ShaderConstant, ShaderContext};

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C, align(16))]
    struct TestUniform {
        colour: [f32; 4],
        transform: [[f32; 4]; 4],
    }

    crate::impl_wgsl_struct!(TestUniform, "TestUniform", {
        colour: [f32; 4],
        transform: [[f32; 4]; 4],
    });

    #[derive(Clone, Copy)]
    #[repr(u32)]
    enum TestMode {
        Preview = 2,
        Final = 9,
    }

    crate::impl_wgsl_enum!(TestMode, "TestMode", u32, { Preview, Final });

    #[test]
    fn embedded_template_uses_every_rust_owned_filter() {
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

        let source = context
            .render_template(INTERFACE_TEST_NAME)
            .expect("embedded interface template renders");
        assert!(
            source.contains("struct TestUniform { colour: vec4<f32>, transform: mat4x4<f32>, }")
        );
        assert!(source.contains("const TestMode_Preview: u32 = 2u;"));
        assert!(source.contains("const TestMode_Final: u32 = 9u;"));
        assert!(source.contains("@group(0) @binding(3) var values: texture_2d<f32>;"));
        assert!(source.contains("const TEST_SCALE: f32 = 2.5;"));
    }
}
