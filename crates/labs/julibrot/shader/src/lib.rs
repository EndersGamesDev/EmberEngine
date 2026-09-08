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
