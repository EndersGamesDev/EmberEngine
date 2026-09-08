use bytemuck::{Pod, Zeroable};
use ember_julibrot_shader::{
    F32Vec4, PRESENT_SHADE_TEMPLATE, RenderError, RenderedShader, ShaderConstant, ShaderContext,
    render,
};

use crate::palette::{
    CLEAR_STATUS, DEBUG_TINT, EXPOSED_STATUS, GLITCH_DIAGNOSTIC, GLITCH_STATUS, HORIZON_STATUS,
    MALFORMED_STATUS, MAP_UNCERTAIN_STATUS, MAX_SCENE_LIGHT, MIN_SCENE_LIGHT, REGULAR_STATUS,
    SKY_STATUS,
};
use crate::{PaletteId, PaletteRecord};

pub const SHADE_VALUES_GROUP: u32 = 0;
pub const PRESENTATION_VALUES_BINDING: u32 = 0;
pub const NEAREST_VALUE_BINDING: u32 = 1;
pub const SHADE_PALETTE_GROUP: u32 = 1;
pub const PALETTE_UNIFORM_BINDING: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct PaletteUniform {
    map: F32Vec4,
    interior_rgba: F32Vec4,
    clear_rgba: F32Vec4,
}

ember_julibrot_shader::impl_wgsl_struct!(PaletteUniform, "PaletteUniform", {
    map: F32Vec4,
    interior_rgba: F32Vec4,
    clear_rgba: F32Vec4,
});

pub fn palette_uniform_bytes() -> u64 {
    u64::try_from(core::mem::size_of::<PaletteUniform>())
        .expect("palette uniform size fits wgpu's address space")
}

pub const fn palette_uniform(record: PaletteRecord) -> PaletteUniform {
    PaletteUniform {
        map: F32Vec4::new(record.map),
        interior_rgba: F32Vec4::new(record.interior_rgba),
        clear_rgba: F32Vec4::new(record.clear_rgba),
    }
}

/// Returns the sole value-to-colour presentation shader.
///
/// # Errors
///
/// Returns the template, registration, parsing, or validation failure detected before wgpu sees
/// the source.
pub fn shade_shader() -> Result<RenderedShader, RenderError> {
    let mut context = ShaderContext::new();
    context.register_type::<PaletteUniform>()?;
    context.register_enum::<PaletteId>()?;
    context.register_binding(
        "presentation_values",
        SHADE_VALUES_GROUP,
        PRESENTATION_VALUES_BINDING,
    )?;
    context.register_binding("nearest_value", SHADE_VALUES_GROUP, NEAREST_VALUE_BINDING)?;
    context.register_binding("palette", SHADE_PALETTE_GROUP, PALETTE_UNIFORM_BINDING)?;
    for (name, value) in [
        ("REGULAR_STATUS", REGULAR_STATUS),
        ("GLITCH_STATUS", GLITCH_STATUS),
        ("HORIZON_STATUS", HORIZON_STATUS),
        ("MAP_UNCERTAIN_STATUS", MAP_UNCERTAIN_STATUS),
        ("CLEAR_STATUS", CLEAR_STATUS),
        ("EXPOSED_STATUS", EXPOSED_STATUS),
        ("SKY_STATUS", SKY_STATUS),
        ("MALFORMED_STATUS", MALFORMED_STATUS),
        ("MIN_SCENE_LIGHT", MIN_SCENE_LIGHT),
        ("MAX_SCENE_LIGHT", MAX_SCENE_LIGHT),
    ] {
        context.register_constant(name, ShaderConstant::Float(value))?;
    }
    context.register_constant("DEBUG_TINT", ShaderConstant::Float4(DEBUG_TINT))?;
    context.register_constant(
        "GLITCH_DIAGNOSTIC",
        ShaderConstant::Float4(GLITCH_DIAGNOSTIC),
    )?;
    render(PRESENT_SHADE_TEMPLATE, &context)
}

#[cfg(test)]
mod production_validation {
    use super::{PRESENT_SHADE_TEMPLATE, shade_shader};

    ember_julibrot_shader::production_template_test!(PRESENT_SHADE_TEMPLATE, shade_shader);
}

#[cfg(test)]
mod tests {
    use ember_julibrot_shader::{WgslEnum as _, WgslEnumDiscriminant, WgslType as _};

    use super::*;

    fn assert_palette_layout(module: &naga::Module) {
        let description = PaletteUniform::DESCRIPTION;
        let shader_type = module
            .types
            .iter()
            .find_map(|(_, shader_type)| {
                (shader_type.name.as_deref() == Some(description.name)).then_some(shader_type)
            })
            .expect("rendered palette type exists");
        let naga::TypeInner::Struct { members, span } = &shader_type.inner else {
            panic!("rendered palette type must be a struct");
        };
        assert_eq!(members.len(), description.fields.len());
        for (shader_field, rust_field) in members.iter().zip(description.fields) {
            assert_eq!(shader_field.name.as_deref(), Some(rust_field.name));
            assert_eq!(
                shader_field.offset,
                u32::try_from(rust_field.offset).expect("palette field offset fits WGSL"),
            );
        }
        assert_eq!(
            *span,
            u32::try_from(description.size).expect("palette size fits WGSL"),
        );
    }

    fn assert_palette_discriminants(source: &str) {
        for variant in PaletteId::DESCRIPTION.variants {
            let WgslEnumDiscriminant::Unsigned(value) = variant.discriminant else {
                panic!("palette discriminants are unsigned");
            };
            let variant_name = variant.name;
            let expected = format!("const PaletteId_{variant_name}: u32 = {value}u;");
            assert!(source.lines().any(|line| line == expected));
        }
    }

    #[test]
    fn shade_source_uses_the_registered_palette_metadata() {
        for record in [
            crate::CLASSIC_PALETTE,
            crate::EMBER_PALETTE,
            crate::ICE_PALETTE,
        ] {
            let uniform = palette_uniform(record);
            assert_eq!(bytemuck::bytes_of(&uniform), bytemuck::bytes_of(&record));
        }
        let shader = shade_shader().expect("shade template renders");
        let module = naga::front::wgsl::parse_str(shader.source()).expect("shade WGSL parses");
        assert_palette_layout(&module);
        assert_palette_discriminants(shader.source());
    }

    #[test]
    fn shade_is_the_only_palette_reader_and_pins_every_status_colour() {
        let shader = shade_shader().expect("shade template renders");
        let source = shader.source();
        assert!(source.contains("var<uniform> palette: PaletteUniform"));
        assert!(source.contains("status == CLEAR_STATUS || status == EXPOSED_STATUS"));
        assert!(source.contains("status == HORIZON_STATUS || status == SKY_STATUS"));
        assert!(source.contains("status == MALFORMED_STATUS"));
        assert!(source.contains("status == GLITCH_STATUS"));
        assert!(source.contains("return DEBUG_TINT;"));
        assert!(source.contains("return GLITCH_DIAGNOSTIC;"));
        assert!(source.contains("base = palette.interior_rgba;"));
        assert!(source.contains("textureSample(presentation_values, nearest_value, input.uv)"));
        assert!(!source.contains("textureSampleLevel"));
        let scene = crate::scene_shader(ember_lab_heap::DialectLimits {
            descriptor_capacity: 2,
            span_capacity: 2,
            handle_capacity: 4,
        });
        for value_source in [scene.as_str(), crate::warp_shader()] {
            assert!(!value_source.contains("PaletteUniform"));
            assert!(!value_source.contains("palette."));
        }
    }
}
