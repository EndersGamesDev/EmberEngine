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

macro_rules! impl_wgsl_builtin {
    ($rust_type:ty, $wgsl_name:literal) => {
        impl WgslType for $rust_type {
            const DESCRIPTION: WgslTypeDescription = WgslTypeDescription {
                name: $wgsl_name,
                fields: &[],
                size: size_of::<Self>(),
                alignment: align_of::<Self>(),
            };
        }
    };
}

impl_wgsl_builtin!(f32, "f32");
impl_wgsl_builtin!(i32, "i32");
impl_wgsl_builtin!(u32, "u32");

impl_wgsl_builtin!([f32; 2], "vec2<f32>");
impl_wgsl_builtin!([f32; 3], "vec3<f32>");
impl_wgsl_builtin!([f32; 4], "vec4<f32>");
impl_wgsl_builtin!([i32; 2], "vec2<i32>");
impl_wgsl_builtin!([i32; 3], "vec3<i32>");
impl_wgsl_builtin!([i32; 4], "vec4<i32>");
impl_wgsl_builtin!([u32; 2], "vec2<u32>");
impl_wgsl_builtin!([u32; 3], "vec3<u32>");
impl_wgsl_builtin!([u32; 4], "vec4<u32>");

impl_wgsl_builtin!([[f32; 2]; 2], "mat2x2<f32>");
impl_wgsl_builtin!([[f32; 3]; 2], "mat2x3<f32>");
impl_wgsl_builtin!([[f32; 4]; 2], "mat2x4<f32>");
impl_wgsl_builtin!([[f32; 2]; 3], "mat3x2<f32>");
impl_wgsl_builtin!([[f32; 3]; 3], "mat3x3<f32>");
impl_wgsl_builtin!([[f32; 4]; 3], "mat3x4<f32>");
impl_wgsl_builtin!([[f32; 2]; 4], "mat4x2<f32>");
impl_wgsl_builtin!([[f32; 3]; 4], "mat4x3<f32>");
impl_wgsl_builtin!([[f32; 4]; 4], "mat4x4<f32>");

/// Implements [`WgslType`](crate::WgslType) for a C-layout bytemuck type.
///
/// The field list is deliberately explicit: each field's Rust type supplies its WGSL spelling,
/// while `offset_of!`, `size_of`, and `align_of` read the host layout from the described type.
#[macro_export]
macro_rules! impl_wgsl_struct {
    ($rust_type:ty, $wgsl_name:literal, { $($field:ident: $field_type:ty),+ $(,)? }) => {
        impl $crate::WgslType for $rust_type {
            const DESCRIPTION: $crate::WgslTypeDescription = $crate::WgslTypeDescription {
                name: $wgsl_name,
                fields: &[
                    $(
                        $crate::WgslField {
                            name: stringify!($field),
                            wgsl_type: <$field_type as $crate::WgslType>::DESCRIPTION.name,
                            offset: ::core::mem::offset_of!($rust_type, $field),
                        },
                    )+
                ],
                size: ::core::mem::size_of::<$rust_type>(),
                alignment: ::core::mem::align_of::<$rust_type>(),
            };
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{WgslType, WgslTypeDescription};
    use bytemuck::{Pod, Zeroable};

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C, align(16))]
    struct TestUniform {
        point: [f32; 4],
        flags: [u32; 4],
        transform: [[f32; 4]; 4],
    }

    crate::impl_wgsl_struct!(TestUniform, "TestUniform", {
        point: [f32; 4],
        flags: [u32; 4],
        transform: [[f32; 4]; 4],
    });

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

    #[test]
    fn vector_and_matrix_primitives_have_wgsl_spellings() {
        let descriptions = [
            (<[f32; 2] as WgslType>::DESCRIPTION, "vec2<f32>"),
            (<[i32; 3] as WgslType>::DESCRIPTION, "vec3<i32>"),
            (<[u32; 4] as WgslType>::DESCRIPTION, "vec4<u32>"),
            (<[[f32; 2]; 2] as WgslType>::DESCRIPTION, "mat2x2<f32>"),
            (<[[f32; 3]; 3] as WgslType>::DESCRIPTION, "mat3x3<f32>"),
            (<[[f32; 4]; 4] as WgslType>::DESCRIPTION, "mat4x4<f32>"),
        ];
        for (description, wgsl_name) in descriptions {
            assert_eq!(description.name, wgsl_name);
            assert_eq!(description.fields, &[]);
        }
    }

    #[test]
    fn struct_macro_reads_field_offsets_and_layout_from_rust() {
        let description = TestUniform::DESCRIPTION;
        assert_eq!(description.name, "TestUniform");
        assert_eq!(description.size, 96);
        assert_eq!(description.alignment, 16);
        assert_eq!(description.fields[0].name, "point");
        assert_eq!(description.fields[0].wgsl_type, "vec4<f32>");
        assert_eq!(description.fields[0].offset, 0);
        assert_eq!(description.fields[1].offset, 16);
        assert_eq!(description.fields[2].offset, 32);
    }
}
