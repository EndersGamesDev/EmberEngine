//! Rust-owned shader interface descriptions for the Julibrot lab.

#![deny(missing_docs)]

mod enum_meta;
mod runtime;
mod type_meta;

pub use enum_meta::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslEnumVariant};
pub use runtime::{RenderError, ShaderConstant, ShaderContext, WgslBinding};
pub use type_meta::{WgslField, WgslType, WgslTypeDescription};
