use bytemuck::{Pod, Zeroable};

use crate::{
    ShaderContext, WgslEnumDescription, WgslEnumDiscriminant, WgslField, WgslType,
    WgslTypeDescription, render,
};

const ORACLE_TEMPLATE: &str = "oracle-test.wgsl.jinja";
const UNREGISTERED_TYPE_TEMPLATE: &str = "unregistered-type-test.wgsl.jinja";

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, align(16))]
struct OracleUniform {
    colour: [f32; 4],
    transform: [[f32; 4]; 4],
}

crate::impl_wgsl_struct!(OracleUniform, "OracleUniform", {
    colour: [f32; 4],
    transform: [[f32; 4]; 4],
});

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, align(16))]
struct Vec3PaddedUniform {
    direction: [f32; 3],
    direction_padding: f32,
    colour: [f32; 4],
}

crate::impl_wgsl_struct!(Vec3PaddedUniform, "Vec3PaddedUniform", {
    direction: [f32; 3],
    direction_padding: f32,
    colour: [f32; 4],
});

#[derive(Clone, Copy)]
#[repr(u32)]
enum OracleMode {
    Preview = 2,
    Final = 9,
}

crate::impl_wgsl_enum!(OracleMode, "OracleMode", u32, { Preview, Final });

#[derive(Clone, Copy)]
#[repr(i32)]
enum SignedMode {
    Reverse = -3,
    Forward = 5,
}

crate::impl_wgsl_enum!(SignedMode, "SignedMode", i32, { Reverse, Forward });

fn oracle_context() -> ShaderContext {
    let mut context = ShaderContext::new();
    context
        .register_type::<OracleUniform>()
        .expect("oracle uniform has one description");
    context
        .register_type::<Vec3PaddedUniform>()
        .expect("padded uniform has one description");
    context
        .register_enum::<OracleMode>()
        .expect("unsigned oracle enum has one description");
    context
        .register_enum::<SignedMode>()
        .expect("signed oracle enum has one description");
    context
}

fn assert_struct_layout(module: &naga::Module, description: &WgslTypeDescription) {
    let shader_type = module
        .types
        .iter()
        .find_map(|(_, shader_type)| {
            (shader_type.name.as_deref() == Some(description.name)).then_some(shader_type)
        })
        .unwrap_or_else(|| panic!("naga did not retain WGSL type `{}`", description.name));
    let naga::TypeInner::Struct { members, span } = &shader_type.inner else {
        panic!(
            "registered WGSL type `{}` is not a struct",
            description.name
        );
    };

    assert_eq!(
        members.len(),
        description.fields.len(),
        "field count drifted for `{}`",
        description.name,
    );
    for (shader_field, rust_field) in members.iter().zip(description.fields) {
        assert_field_layout(description, shader_field, rust_field);
    }
    assert_eq!(
        *span,
        u32::try_from(description.size).expect("Rust struct size fits WGSL's address space"),
        "struct size drifted for `{}`",
        description.name,
    );
}

fn assert_field_layout(
    structure: &WgslTypeDescription,
    shader_field: &naga::StructMember,
    rust_field: &WgslField,
) {
    assert_eq!(
        shader_field.name.as_deref(),
        Some(rust_field.name),
        "field name drifted in `{}`",
        structure.name,
    );
    assert_eq!(
        shader_field.offset,
        u32::try_from(rust_field.offset).expect("Rust field offset fits WGSL's address space"),
        "field offset drifted for `{}.{}`",
        structure.name,
        rust_field.name,
    );
}

fn assert_enum_values(source: &str, description: &WgslEnumDescription) {
    for variant in description.variants {
        let enum_name = description.name;
        let variant_name = variant.name;
        let expected = match variant.discriminant {
            WgslEnumDiscriminant::Signed(value) => {
                format!("const {enum_name}_{variant_name}: i32 = {value}i;")
            }
            WgslEnumDiscriminant::Unsigned(value) => {
                format!("const {enum_name}_{variant_name}: u32 = {value}u;")
            }
        };
        assert!(
            source.lines().any(|line| line == expected),
            "rendered enum declaration `{expected}` was missing",
        );
    }
}

#[test]
fn registered_type_layouts_match_naga_and_rust() {
    let context = oracle_context();
    let shader = render(ORACLE_TEMPLATE, &context).expect("oracle template renders and validates");
    let module = naga::front::wgsl::parse_str(shader.source()).expect("oracle source parses");

    for description in context.registered_types() {
        assert_struct_layout(&module, description);
    }

    let padded = Vec3PaddedUniform::DESCRIPTION;
    assert_eq!(padded.fields[1].offset, 12);
    assert_eq!(padded.fields[2].offset, 16);
    assert_eq!(padded.size, 32);
}

#[test]
fn rendered_enum_values_match_every_registered_discriminant() {
    let context = oracle_context();
    let shader = render(ORACLE_TEMPLATE, &context).expect("oracle template renders and validates");

    for description in context.registered_enums() {
        assert_enum_values(shader.source(), description);
    }
}

#[test]
fn unregistered_type_failure_names_the_missing_type() {
    let error = render(UNREGISTERED_TYPE_TEMPLATE, &ShaderContext::new())
        .expect_err("an unregistered type cannot render");
    assert!(error.to_string().contains("MissingUniform"));
}
