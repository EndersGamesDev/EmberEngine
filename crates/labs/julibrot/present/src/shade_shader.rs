use bytemuck::{Pod, Zeroable};
use ember_julibrot_shader::{
    F32Vec4, PRESENT_SHADE_TEMPLATE, RenderError, RenderedShader, ShaderContext, render,
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
    render(PRESENT_SHADE_TEMPLATE, &context)
}

#[cfg(test)]
const LEGACY_SHADE_SOURCE: &str = r"
struct PaletteUniform { map: vec4<f32>, interior_rgba: vec4<f32>, clear_rgba: vec4<f32>, }
@group(0) @binding(0) var presentation_values: texture_2d<f32>;
@group(0) @binding(1) var nearest_value: sampler;
@group(1) @binding(0) var<uniform> palette: PaletteUniform;
struct ShadeVertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, }
@vertex fn shade_vertex(@builtin(vertex_index) vertex: u32) -> ShadeVertex {
    var points = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
    var output: ShadeVertex;
    output.position = vec4<f32>(points[vertex], 0.0, 1.0);
    output.uv = vec2<f32>(0.5 * points[vertex].x + 0.5, 0.5 - 0.5 * points[vertex].y);
    return output;
}
fn finite(value: f32) -> bool { return abs(value) <= 3.402823e38; }
fn binary(value: f32) -> bool { return value == 0.0 || value == 1.0; }
fn hue_component(hue: f32, offset: f32) -> f32 {
    return clamp(abs(fract(hue + offset) * 6.0 - 3.0) - 1.0, 0.0, 1.0);
}
fn exterior_zero() -> vec4<f32> {
    let hue = fract(palette.map.y);
    let phase_rgb = vec3<f32>(hue_component(hue, 0.0), hue_component(hue, 0.6666666667), hue_component(hue, 0.3333333333));
    return vec4<f32>(palette.map.w * mix(vec3<f32>(1.0), phase_rgb, palette.map.z), 1.0);
}
fn colour(value: vec4<f32>) -> vec4<f32> {
    let status = value.z;
    if (status == 7.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (status == 1.0) { return vec4<f32>(1.0, 0.375, 0.0, 1.0); }
    if (status == 4.0 || status == 5.0) { return palette.clear_rgba; }
    if (status == 2.0 || status == 6.0) { return exterior_zero(); }
    if (status != 0.0 && status != 3.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    if (!binary(value.y)) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
    var base = vec4<f32>(0.0);
    if (value.y == 0.0) {
        if (value.x != -1.0) { return vec4<f32>(1.0, 0.0, 1.0, 1.0); }
        base = palette.interior_rgba;
    } else {
        if (!finite(value.x) || !finite(palette.map.x) || palette.map.x <= 0.0) {
            return vec4<f32>(1.0, 0.0, 1.0, 1.0);
        }
        let hue = fract(max(value.x, 0.0) / palette.map.x + palette.map.y);
        let phase_rgb = vec3<f32>(hue_component(hue, 0.0), hue_component(hue, 0.6666666667), hue_component(hue, 0.3333333333));
        let rgb = palette.map.w * mix(vec3<f32>(1.0), phase_rgb, palette.map.z);
        base = vec4<f32>(rgb, 1.0);
    }
    if (!finite(value.w) || value.w < 0.58 || value.w > 0.82) {
        return vec4<f32>(1.0, 0.0, 1.0, 1.0);
    }
    return vec4<f32>(base.rgb * value.w, base.a);
}
@fragment fn shade_fragment(input: ShadeVertex) -> @location(0) vec4<f32> {
    return colour(textureSample(presentation_values, nearest_value, input.uv));
}
";

#[cfg(test)]
mod tests {
    use ember_julibrot_shader::{WgslEnum as _, WgslEnumDiscriminant, WgslType as _};

    use super::*;

    const RENDERED_SHADE_HASH: u64 = 0x57fe_070b_02fa_9c44;

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
    fn shade_source_parses_and_validates() {
        for record in [
            crate::CLASSIC_PALETTE,
            crate::EMBER_PALETTE,
            crate::ICE_PALETTE,
        ] {
            let uniform = palette_uniform(record);
            assert_eq!(bytemuck::bytes_of(&uniform), bytemuck::bytes_of(&record));
        }
        let shader = shade_shader().expect("shade template renders");
        let legacy_source: String = shader
            .source()
            .split_inclusive('\n')
            .filter(|line| !line.starts_with("const PaletteId_"))
            .collect();
        assert_eq!(legacy_source, LEGACY_SHADE_SOURCE);
        assert_eq!(
            shader.hash(),
            RENDERED_SHADE_HASH,
            "actual rendered shade hash: {:#018x}",
            shader.hash()
        );

        let module = naga::front::wgsl::parse_str(shader.source()).expect("shade WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("shade WGSL validates");
        assert_palette_layout(&module);
        assert_palette_discriminants(shader.source());
    }

    #[test]
    fn shade_is_the_only_palette_reader_and_pins_every_status_colour() {
        let shader = shade_shader().expect("shade template renders");
        let source = shader.source();
        assert!(source.contains("var<uniform> palette: PaletteUniform"));
        assert!(source.contains("status == 4.0 || status == 5.0"));
        assert!(source.contains("status == 2.0 || status == 6.0"));
        assert!(source.contains("status == 7.0"));
        assert!(source.contains("status == 1.0"));
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
