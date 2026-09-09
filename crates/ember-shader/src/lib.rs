//! Rust-owned shader interface descriptions shared across the repository.

#![deny(missing_docs)]

mod enum_meta;
mod runtime;
mod type_meta;

pub use enum_meta::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslEnumVariant};
pub use runtime::{
    RenderError, RenderedShader, ShaderConstant, ShaderContext, WgslBinding, render,
};
pub use type_meta::{
    F32Vec2, F32Vec3, F32Vec4, I32Vec2, I32Vec3, I32Vec4, U32Vec2, U32Vec3, U32Vec4, WgslField,
    WgslType, WgslTypeDescription,
};
