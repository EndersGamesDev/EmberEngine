//! Rust-owned shader interface descriptions for the Julibrot lab.

#![deny(missing_docs)]

use core::mem::{align_of, size_of};

/// One named field in a Rust type's WGSL representation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WgslField {
    /// Rust and WGSL field name.
    pub name: &'static str,
    /// WGSL spelling of the field type.
    pub wgsl_type: &'static str,
    /// Byte offset from the beginning of the Rust value.
    pub offset: usize,
}

/// Complete host-shareable layout metadata for one Rust type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WgslTypeDescription {
    /// WGSL spelling of the type.
    pub name: &'static str,
    /// Struct fields in declaration order, or an empty slice for a WGSL built-in.
    pub fields: &'static [WgslField],
    /// Rust size in bytes.
    pub size: usize,
    /// Rust alignment in bytes.
    pub alignment: usize,
}

/// A bytemuck-safe Rust type with an explicit WGSL representation.
pub trait WgslType: bytemuck::Pod {
    /// Static metadata from which templates emit and verify WGSL declarations.
    const DESCRIPTION: WgslTypeDescription;
}

impl WgslType for f32 {
    const DESCRIPTION: WgslTypeDescription = WgslTypeDescription {
        name: "f32",
        fields: &[],
        size: size_of::<Self>(),
        alignment: align_of::<Self>(),
    };
}

#[cfg(test)]
mod tests {
    use super::{WgslType, WgslTypeDescription};

    #[test]
    fn scalar_description_comes_from_the_rust_layout() {
        assert_eq!(
            <f32 as WgslType>::DESCRIPTION,
            WgslTypeDescription {
                name: "f32",
                fields: &[],
                size: 4,
                alignment: 4,
            },
        );
    }
}
