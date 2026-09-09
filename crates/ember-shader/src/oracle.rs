use bytemuck::{Pod, Zeroable};

use crate::{F32Vec3, F32Vec4, ShaderContext, U32Vec4, WgslType as _, render};

const ORACLE_TEMPLATE: &str = "oracle-test.wgsl.jinja";
const UNREGISTERED_TYPE_TEMPLATE: &str = "unregistered-type-test.wgsl.jinja";

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, align(16))]
struct OracleUniform {
    colour: F32Vec4,
    flags: U32Vec4,
}

crate::impl_wgsl_struct!(OracleUniform, "OracleUniform", {
    colour: F32Vec4,
    flags: U32Vec4,
});

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C, align(16))]
struct Vec3PaddedUniform {
    direction: F32Vec3,
    colour: F32Vec4,
}

crate::impl_wgsl_struct!(Vec3PaddedUniform, "Vec3PaddedUniform", {
    direction: F32Vec3,
    colour: F32Vec4,
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

#[test]
fn registered_type_layouts_match_naga_and_rust() {
    let context = oracle_context();
    let shader = render(ORACLE_TEMPLATE, &context)
        .expect("the common native audit accepts every oracle type");

    let padded = Vec3PaddedUniform::DESCRIPTION;
    assert_eq!(F32Vec3::DESCRIPTION.size, 16);
    assert_eq!(F32Vec3::DESCRIPTION.alignment, 16);
    assert_eq!(padded.fields[1].offset, 16);
    assert_eq!(padded.size, 32);
    assert!(shader.source().contains("@size(16) direction: vec3<f32>"));
}

#[test]
fn rendered_enum_values_match_every_registered_discriminant() {
    let context = oracle_context();
    render(ORACLE_TEMPLATE, &context)
        .expect("the common native audit accepts every oracle discriminant");
}

#[test]
fn unregistered_type_failure_names_the_missing_type() {
    let error = render(UNREGISTERED_TYPE_TEMPLATE, &ShaderContext::new())
        .expect_err("an unregistered type cannot render");
    assert!(error.to_string().contains("MissingUniform"));
}
