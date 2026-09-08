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
    /// Explicit WGSL member size when the Rust representation owns tail padding.
    pub member_size: Option<usize>,
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
    /// Explicit size to apply when this type is emitted as a WGSL struct member.
    pub member_size: Option<usize>,
}

/// A bytemuck-safe Rust type with an explicit WGSL representation.
pub trait WgslType: bytemuck::Pod {
    /// Static metadata from which templates emit and verify WGSL declarations.
    const DESCRIPTION: WgslTypeDescription;
}

macro_rules! impl_wgsl_builtin {
    (
        $rust_type:ty,
        $wgsl_name:literal,
        $wgsl_size:expr,
        $wgsl_alignment:expr,
        $member_size:expr
    ) => {
        const _: [(); $wgsl_size] = [(); size_of::<$rust_type>()];
        const _: [(); $wgsl_alignment] = [(); align_of::<$rust_type>()];

        impl WgslType for $rust_type {
            const DESCRIPTION: WgslTypeDescription = WgslTypeDescription {
                name: $wgsl_name,
                fields: &[],
                size: size_of::<Self>(),
                alignment: align_of::<Self>(),
                member_size: $member_size,
            };
        }
    };
}

impl_wgsl_builtin!(f32, "f32", 4, 4, None);
impl_wgsl_builtin!(i32, "i32", 4, 4, None);
impl_wgsl_builtin!(u32, "u32", 4, 4, None);

macro_rules! define_exact_vector {
    ($name:ident, $scalar:ty, $lanes:literal, $alignment:literal, $wgsl_name:literal) => {
        #[doc = concat!("An explicitly aligned Rust representation of WGSL `", $wgsl_name, "`.")]
        #[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        #[repr(C, align($alignment))]
        pub struct $name {
            lanes: [$scalar; $lanes],
        }

        impl $name {
            /// Creates the vector from lanes in WGSL order.
            #[must_use]
            pub const fn new(lanes: [$scalar; $lanes]) -> Self {
                Self { lanes }
            }

            /// Returns the vector lanes in WGSL order.
            #[must_use]
            pub const fn into_array(self) -> [$scalar; $lanes] {
                self.lanes
            }
        }

        impl_wgsl_builtin!($name, $wgsl_name, $lanes * 4, $alignment, None);
    };
}

macro_rules! define_padded_vec3 {
    ($name:ident, $scalar:ty, $zero:expr, $wgsl_name:literal) => {
        #[doc = concat!("An aligned, padded Rust representation of WGSL `", $wgsl_name, "`.")]
        #[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        #[repr(C, align(16))]
        pub struct $name {
            storage: [$scalar; 4],
        }

        impl $name {
            /// Creates the vector and initializes its storage padding.
            #[must_use]
            pub const fn new([x, y, z]: [$scalar; 3]) -> Self {
                Self {
                    storage: [x, y, z, $zero],
                }
            }

            /// Returns the three logical vector lanes in WGSL order.
            #[must_use]
            pub const fn into_array(self) -> [$scalar; 3] {
                let [x, y, z, _padding] = self.storage;
                [x, y, z]
            }
        }

        impl_wgsl_builtin!($name, $wgsl_name, 16, 16, Some(16));
    };
}

define_exact_vector!(F32Vec2, f32, 2, 8, "vec2<f32>");
define_padded_vec3!(F32Vec3, f32, 0.0, "vec3<f32>");
define_exact_vector!(F32Vec4, f32, 4, 16, "vec4<f32>");
define_exact_vector!(I32Vec2, i32, 2, 8, "vec2<i32>");
define_padded_vec3!(I32Vec3, i32, 0, "vec3<i32>");
define_exact_vector!(I32Vec4, i32, 4, 16, "vec4<i32>");
define_exact_vector!(U32Vec2, u32, 2, 8, "vec2<u32>");
define_padded_vec3!(U32Vec3, u32, 0, "vec3<u32>");
define_exact_vector!(U32Vec4, u32, 4, 16, "vec4<u32>");

impl Eq for I32Vec2 {}
impl Eq for I32Vec3 {}
impl Eq for I32Vec4 {}
impl Eq for U32Vec2 {}
impl Eq for U32Vec3 {}
impl Eq for U32Vec4 {}

/// Implements [`WgslType`](crate::WgslType) for a C-layout bytemuck type.
///
/// The field list is deliberately explicit: each field's Rust type supplies its WGSL spelling,
/// while `offset_of!`, `size_of`, and `align_of` read the host layout from the described type.
#[macro_export]
macro_rules! impl_wgsl_struct {
    ($rust_type:ty, $wgsl_name:literal, { $($field:ident: $field_type:ty),+ $(,)? }) => {
        const _: () = {
            fn assert_field_types(value: &$rust_type) {
                $(let _: &$field_type = &value.$field;)+
            }

            const _: fn(&$rust_type) = assert_field_types;
        };

        impl $crate::WgslType for $rust_type {
            const DESCRIPTION: $crate::WgslTypeDescription = $crate::WgslTypeDescription {
                name: $wgsl_name,
                fields: &[
                    $(
                        $crate::WgslField {
                            name: stringify!($field),
                            wgsl_type: <$field_type as $crate::WgslType>::DESCRIPTION.name,
                            offset: ::core::mem::offset_of!($rust_type, $field),
                            member_size: <$field_type as $crate::WgslType>::DESCRIPTION.member_size,
                        },
                    )+
                ],
                size: ::core::mem::size_of::<$rust_type>(),
                alignment: ::core::mem::align_of::<$rust_type>(),
                member_size: None,
            };
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{F32Vec2, F32Vec4, I32Vec3, U32Vec4, WgslType, WgslTypeDescription};
    use bytemuck::{Pod, Zeroable};

    #[derive(Clone, Copy, Pod, Zeroable)]
    #[repr(C, align(16))]
    struct TestUniform {
        point: F32Vec4,
        flags: U32Vec4,
    }

    crate::impl_wgsl_struct!(TestUniform, "TestUniform", {
        point: F32Vec4,
        flags: U32Vec4,
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
                member_size: None,
            },
        );
    }

    #[test]
    fn vector_primitives_have_host_shareable_rust_layouts() {
        let descriptions = [
            (F32Vec2::DESCRIPTION, "vec2<f32>", 8, 8, None),
            (I32Vec3::DESCRIPTION, "vec3<i32>", 16, 16, Some(16)),
            (U32Vec4::DESCRIPTION, "vec4<u32>", 16, 16, None),
        ];
        for (description, wgsl_name, size, alignment, member_size) in descriptions {
            assert_eq!(description.name, wgsl_name);
            assert_eq!(description.fields, &[]);
            assert_eq!(description.size, size);
            assert_eq!(description.alignment, alignment);
            assert_eq!(description.member_size, member_size);
        }
        assert_eq!(
            F32Vec4::new([1.0, 2.0, 3.0, 4.0]).into_array(),
            [1.0, 2.0, 3.0, 4.0],
        );
    }

    #[test]
    fn struct_macro_reads_field_offsets_and_layout_from_rust() {
        let description = TestUniform::DESCRIPTION;
        assert_eq!(description.name, "TestUniform");
        assert_eq!(description.size, 32);
        assert_eq!(description.alignment, 16);
        assert_eq!(description.fields[0].name, "point");
        assert_eq!(description.fields[0].wgsl_type, "vec4<f32>");
        assert_eq!(description.fields[0].offset, 0);
        assert_eq!(description.fields[1].offset, 16);
    }
}
