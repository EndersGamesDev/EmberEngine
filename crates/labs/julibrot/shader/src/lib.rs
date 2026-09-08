//! Rust-owned shader interface descriptions for the Julibrot lab.
//!
//! Native rendering parses and validates the result with naga. Wasm rendering returns the source
//! and its stable hash; wgpu performs the browser path's single naga validation when it creates the
//! shader module.

#![deny(missing_docs)]

mod enum_meta;
#[cfg(test)]
mod oracle;
mod runtime;
mod type_meta;

pub use enum_meta::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslEnumVariant};
pub use runtime::{
    PRESENT_SHADE_TEMPLATE, RenderError, RenderedShader, ShaderConstant, ShaderContext,
    WgslBinding, render,
};
pub use type_meta::{
    F32Vec2, F32Vec3, F32Vec4, I32Vec2, I32Vec3, I32Vec4, U32Vec2, U32Vec3, U32Vec4, WgslField,
    WgslType, WgslTypeDescription,
};

/// Defines the required native validation test for one production template.
///
/// Invoke this once, in its own test module, with the production template constant and the
/// production renderer that builds its real [`ShaderContext`]. The generated test makes the
/// renderer's exact returned source flow through naga parsing and validation.
#[macro_export]
macro_rules! production_template_test {
    ($template:ident, $render:path) => {
        #[test]
        fn production_template_renders_and_validates() {
            let template_name = $template;
            let shader = $render().unwrap_or_else(|error| {
                panic!("production template `{template_name}` failed to render: {error}")
            });
            let module = ::naga::front::wgsl::parse_str(shader.source()).unwrap_or_else(|error| {
                panic!("production template `{template_name}` failed to parse: {error}")
            });
            ::naga::valid::Validator::new(
                ::naga::valid::ValidationFlags::all(),
                ::naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|error| {
                panic!("production template `{template_name}` failed validation: {error}")
            });
        }
    };
}
