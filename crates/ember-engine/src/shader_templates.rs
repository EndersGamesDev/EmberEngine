//! Runtime rendering for the engine-owned scene and presentation shaders.

use ember_shader::{
    ENGINE_PRESENT_TEMPLATE, ENGINE_SCENE_TEMPLATE, RenderError, RenderedShader, ShaderConstant,
    ShaderContext, WgslBinding,
};

use crate::renderer::SceneUniform;

pub const SCENE_UNIFORM_BINDING: WgslBinding = WgslBinding {
    group: 0,
    binding: 0,
};
pub const MESH_TEXTURE_BINDING: WgslBinding = WgslBinding {
    group: 1,
    binding: 0,
};
pub const MESH_SAMPLER_BINDING: WgslBinding = WgslBinding {
    group: 1,
    binding: 1,
};
pub const SHADOW_TEXTURE_BINDING: WgslBinding = WgslBinding {
    group: 2,
    binding: 0,
};
pub const OCCLUSION_XY_BINDING: WgslBinding = WgslBinding {
    group: 3,
    binding: 0,
};
pub const OCCLUSION_Z_BINDING: WgslBinding = WgslBinding {
    group: 3,
    binding: 1,
};
pub const OCCLUSION_SAMPLER_BINDING: WgslBinding = WgslBinding {
    group: 3,
    binding: 2,
};
pub const PRESENT_TEXTURE_BINDING: WgslBinding = WgslBinding {
    group: 0,
    binding: 0,
};
pub const PRESENT_SAMPLER_BINDING: WgslBinding = WgslBinding {
    group: 0,
    binding: 1,
};

pub const MESH_POSITION_LOCATION: u32 = 0;
pub const MESH_NORMAL_LOCATION: u32 = 1;
pub const MESH_UV_LOCATION: u32 = 2;
pub const INSTANCE_POSITION_LOCATION: u32 = 3;
pub const INSTANCE_SCALE_LOCATION: u32 = 4;
pub const INSTANCE_COLOR_LOCATION: u32 = 5;
pub const INSTANCE_ROTATION_LOCATION: u32 = 6;
pub const INSTANCE_MATERIAL_LOCATION: u32 = 7;
pub const PARTICLE_POSITION_LOCATION: u32 = 0;
pub const PARTICLE_SIZE_LOCATION: u32 = 1;
pub const PARTICLE_COLOR_LOCATION: u32 = 2;
pub const PARTICLE_OPACITY_LOCATION: u32 = 3;

fn register_bindings(context: &mut ShaderContext) -> Result<(), RenderError> {
    for (name, binding) in [
        ("scene", SCENE_UNIFORM_BINDING),
        ("mesh_tex", MESH_TEXTURE_BINDING),
        ("mesh_samp", MESH_SAMPLER_BINDING),
        ("shadow_tex", SHADOW_TEXTURE_BINDING),
        ("occlusion_xy", OCCLUSION_XY_BINDING),
        ("occlusion_z", OCCLUSION_Z_BINDING),
        ("occlusion_sampler", OCCLUSION_SAMPLER_BINDING),
    ] {
        context.register_binding(name, binding.group, binding.binding)?;
    }
    Ok(())
}

fn register_locations(context: &mut ShaderContext) -> Result<(), RenderError> {
    for (name, location) in [
        ("MESH_POSITION_LOCATION", MESH_POSITION_LOCATION),
        ("MESH_NORMAL_LOCATION", MESH_NORMAL_LOCATION),
        ("MESH_UV_LOCATION", MESH_UV_LOCATION),
        ("INSTANCE_POSITION_LOCATION", INSTANCE_POSITION_LOCATION),
        ("INSTANCE_SCALE_LOCATION", INSTANCE_SCALE_LOCATION),
        ("INSTANCE_COLOR_LOCATION", INSTANCE_COLOR_LOCATION),
        ("INSTANCE_ROTATION_LOCATION", INSTANCE_ROTATION_LOCATION),
        ("INSTANCE_MATERIAL_LOCATION", INSTANCE_MATERIAL_LOCATION),
        ("PARTICLE_POSITION_LOCATION", PARTICLE_POSITION_LOCATION),
        ("PARTICLE_SIZE_LOCATION", PARTICLE_SIZE_LOCATION),
        ("PARTICLE_COLOR_LOCATION", PARTICLE_COLOR_LOCATION),
        ("PARTICLE_OPACITY_LOCATION", PARTICLE_OPACITY_LOCATION),
    ] {
        context.register_constant(name, ShaderConstant::Unsigned(location))?;
    }
    Ok(())
}

pub fn scene_shader() -> Result<RenderedShader, RenderError> {
    let mut context = ShaderContext::new();
    context.register_type::<SceneUniform>()?;
    register_bindings(&mut context)?;
    register_locations(&mut context)?;
    ember_shader::render(ENGINE_SCENE_TEMPLATE, &context)
}

pub fn present_shader() -> Result<RenderedShader, RenderError> {
    let mut context = ShaderContext::new();
    for (name, binding) in [
        ("scene_tex", PRESENT_TEXTURE_BINDING),
        ("scene_samp", PRESENT_SAMPLER_BINDING),
    ] {
        context.register_binding(name, binding.group, binding.binding)?;
    }
    ember_shader::render(ENGINE_PRESENT_TEMPLATE, &context)
}

#[cfg(test)]
mod tests {
    use super::{present_shader, scene_shader};

    #[test]
    fn scene_source_pins_rust_owned_interfaces_and_shipping_entrypoints() {
        let source = scene_shader().expect("the engine scene template renders");
        for pin in [
            "struct SceneUniform {",
            "view_proj_0: vec4<f32>",
            "occlusion_cell: vec4<f32>",
            "light_positions: array<vec4<f32>, 4>",
            "light_colors: array<vec4<f32>, 4>",
            "@group(0) @binding(0) var<uniform> scene: SceneUniform;",
            "@group(3) @binding(2) var occlusion_sampler: sampler;",
            "const INSTANCE_MATERIAL_LOCATION: u32 = 7u;",
            "@vertex\nfn vs_main",
            "@fragment\nfn fs_main",
            "fn vs_sky",
            "fn fs_sky",
            "fn vs_shadow",
            "fn fs_shadow",
            "fn vs_particle",
            "fn fs_particle",
        ] {
            assert!(
                source.source().contains(pin),
                "scene source lost structural pin: {pin}"
            );
        }
    }

    #[test]
    fn present_source_pins_rust_owned_bindings_and_shipping_entrypoints() {
        let source = present_shader().expect("the engine present template renders");
        for pin in [
            "@group(0) @binding(0) var scene_tex: texture_2d<f32>;",
            "@group(0) @binding(1) var scene_samp: sampler;",
            "@vertex\nfn vs_main",
            "@fragment\nfn fs_main",
        ] {
            assert!(
                source.source().contains(pin),
                "present source lost structural pin: {pin}"
            );
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod production_validation {
    use super::{present_shader, scene_shader};
    use ember_shader::{ENGINE_PRESENT_TEMPLATE, ENGINE_SCENE_TEMPLATE};

    ::ember_shader::production_template_test!(
        ENGINE_SCENE_TEMPLATE,
        scene_shader,
        production_template_scene_shader_renders_and_validates,
    );
    ::ember_shader::production_template_test!(
        ENGINE_PRESENT_TEMPLATE,
        present_shader,
        production_template_present_shader_renders_and_validates,
    );
}
