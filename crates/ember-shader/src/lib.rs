//! Rust-owned shader interface descriptions shared across the repository.

#![deny(missing_docs)]

mod enum_meta;
#[cfg(test)]
mod oracle;
mod runtime;
mod type_meta;

pub use enum_meta::{WgslEnum, WgslEnumDescription, WgslEnumDiscriminant, WgslEnumVariant};
#[cfg(not(target_arch = "wasm32"))]
#[doc(hidden)]
pub use runtime::validate_rendered_wgsl;
pub use runtime::{
    RenderError, RenderedShader, ShaderConstant, ShaderContext, WgslBinding, render,
};
pub use type_meta::{
    F32Vec2, F32Vec3, F32Vec4, I32Vec2, I32Vec3, I32Vec4, U32Vec2, U32Vec3, U32Vec4, WgslField,
    WgslType, WgslTypeDescription,
};

/// Defines the required native validation test for one production template.
///
/// Invoke this once as `::ember_shader::production_template_test!` in a
/// `production_validation` test module, with the production template constant, the production
/// renderer that builds its real [`ShaderContext`], and the deterministic test name
/// `production_template_<render_fn>_renders_and_validates`. For example, renderer `scene_shader`
/// has test `production_template_scene_shader_renders_and_validates`. The deployment checker
/// derives that name independently, authenticates the absolute extern path through locked Cargo
/// metadata, finds the test in the compiled native test binary, and runs it normally and in a
/// nonce-bearing probe mode. The normal test makes the renderer's exact returned source flow
/// through naga parsing and validation. Probe mode corrupts that source with an unresolved WGSL
/// identifier and reports the first diagnostic line only after the crate's validator rejects it.
#[macro_export]
macro_rules! production_template_test {
    ($template:ident, $render:path, $test_name:ident $(,)?) => {
        #[test]
        fn $test_name() {
            let template_name = $template;
            let shader = $render().unwrap_or_else(|error| {
                panic!("production template `{template_name}` failed to render: {error}")
            });
            if let Some(probe) = ::std::env::var("EMBER_SHADER_VALIDATION_PROBE")
                .ok()
                .filter(|value| !value.is_empty())
            {
                let mut corrupted = shader.source().to_owned();
                corrupted.push_str(
                    "\nconst ember_shader_validation_probe_value: u32 = \
                     ember_shader_validation_probe_missing;\n",
                );
                let error = $crate::validate_rendered_wgsl(template_name, &corrupted)
                    .expect_err("the validation probe WGSL must fail");
                let diagnostic = match error {
                    $crate::RenderError::WgslParse { diagnostic, .. }
                    | $crate::RenderError::WgslValidation { diagnostic, .. } => diagnostic,
                    other => panic!("validation probe returned a non-naga error: {other}"),
                };
                let first_line = diagnostic
                    .lines()
                    .next()
                    .filter(|line| !line.is_empty())
                    .expect("the validation probe has a naga diagnostic");
                let mut output = ::std::io::stdout().lock();
                ::std::io::Write::write_fmt(
                    &mut output,
                    format_args!("SHADER-VALIDATION-PROBE {probe} {template_name} {first_line}\n"),
                )
                .expect("the validation probe writes to stdout");
                ::std::io::Write::flush(&mut output).expect("the validation probe flushes stdout");
            } else {
                $crate::validate_rendered_wgsl(template_name, shader.source()).unwrap_or_else(
                    |error| {
                        panic!("production template `{template_name}` failed validation: {error}")
                    },
                );
            }
        }
    };
}

#[cfg(test)]
mod probe_tests {
    use std::process::Command;

    use crate::{RenderError, RenderedShader, ShaderContext};

    const PROBE_TEMPLATE: &str = "probe-test.wgsl.jinja";
    const PROBE_TEST: &str = "probe_tests::production_template_render_probe_renders_and_validates";

    fn render_probe() -> Result<RenderedShader, RenderError> {
        crate::render(PROBE_TEMPLATE, &ShaderContext::new())
    }

    crate::production_template_test!(
        PROBE_TEMPLATE,
        render_probe,
        production_template_render_probe_renders_and_validates,
    );

    #[test]
    fn production_template_macro_probe_names_nonce_and_missing_identifier() {
        let nonce = format!("ember-shader-crate-probe-{}", std::process::id());
        let output = Command::new(std::env::current_exe().expect("the test executable has a path"))
            .args(["--exact", PROBE_TEST, "--nocapture"])
            .env("EMBER_SHADER_VALIDATION_PROBE", nonce.as_str())
            .output()
            .expect("the probe test subprocess starts");
        let stdout = String::from_utf8(output.stdout).expect("the probe output is UTF-8");
        let stderr = String::from_utf8(output.stderr).expect("the probe errors are UTF-8");

        assert!(
            output.status.success(),
            "probe subprocess failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            stdout.contains(&format!("SHADER-VALIDATION-PROBE {nonce} {PROBE_TEMPLATE}")),
            "probe receipt did not bind its nonce and template:\n{stdout}"
        );
        assert!(
            stdout.contains("ember_shader_validation_probe_missing"),
            "probe diagnostic did not name the undeclared identifier:\n{stdout}"
        );
    }
}
