use ember_lab_heap::KernelDesc;

pub const OUTPUT_PAGE_SIDE: u16 = 256;

/// Uniform capacity an executor must offer so that both Julibrot descriptors register.
///
/// `GpuKernelExecutor::register_kernel` refuses any descriptor whose `uniform_size` exceeds the
/// executor's configured `kernel_uniform_bytes`, and that refusal is raised before a pipeline is
/// built, so it lands on every device and carries no driver diagnostic. A host that repeated the
/// number as its own literal would therefore hold a copy that silently goes stale the moment a
/// uniform grows here. Deriving the capacity from the descriptors keeps the two in step by
/// construction, rounded to the 16-byte multiple the executor's configuration check demands.
pub const KERNEL_UNIFORM_BYTES: u32 = {
    let shallow = shallow_kernel().uniform_size;
    let perturbation = perturbation_kernel().uniform_size;
    let widest = if shallow > perturbation {
        shallow
    } else {
        perturbation
    };
    widest.next_multiple_of(16)
};

const ESCAPE_FIELD: &[&str] = &["escape"];
const NO_ACCESSORS: &[&str] = &[];
const REFERENCE_ACCESSOR: &[&str] = &["reference"];
const SHALLOW_BODY: &str = include_str!("shallow.wgsl");
const PERTURB_BODY: &str = include_str!("perturb.wgsl");

/// Describes the gather-free shallow escape kernel to heap dialect v2.
#[must_use]
pub const fn shallow_kernel() -> KernelDesc<'static> {
    KernelDesc {
        name: "julibrot_shallow",
        body: SHALLOW_BODY,
        accessors: NO_ACCESSORS,
        output_fields: ESCAPE_FIELD,
        uniform_type: "ShallowUniform",
        uniform_size: 144,
        output_page_side: OUTPUT_PAGE_SIDE,
    }
}

/// Describes the one-input scaled perturbation kernel to heap dialect v2.
#[must_use]
pub const fn perturbation_kernel() -> KernelDesc<'static> {
    KernelDesc {
        name: "julibrot_perturbation",
        body: PERTURB_BODY,
        accessors: REFERENCE_ACCESSOR,
        output_fields: ESCAPE_FIELD,
        uniform_type: "PerturbUniform",
        uniform_size: 112,
        output_page_side: OUTPUT_PAGE_SIDE,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        KERNEL_UNIFORM_BYTES, PERTURB_BODY, SHALLOW_BODY, perturbation_kernel, shallow_kernel,
    };
    use ember_lab_heap::{DialectLimits, RegisteredKernel};

    const LIMITS: DialectLimits = DialectLimits {
        descriptor_capacity: 64,
        span_capacity: 16,
        handle_capacity: 64,
    };

    #[test]
    fn the_published_uniform_capacity_admits_both_descriptors() {
        for descriptor in [shallow_kernel(), perturbation_kernel()] {
            assert!(
                descriptor.uniform_size <= KERNEL_UNIFORM_BYTES,
                "{} needs {} uniform bytes but the published capacity is {KERNEL_UNIFORM_BYTES}",
                descriptor.name,
                descriptor.uniform_size,
            );
        }
        assert_ne!(KERNEL_UNIFORM_BYTES, 0);
        assert!(KERNEL_UNIFORM_BYTES.is_multiple_of(16));
    }

    #[test]
    fn both_bodies_register_through_dialect_v2() {
        let shallow = RegisteredKernel::register(&shallow_kernel(), LIMITS)
            .expect("shallow body satisfies dialect v2");
        let perturb = RegisteredKernel::register(&perturbation_kernel(), LIMITS)
            .expect("perturbation body satisfies dialect v2");
        assert_eq!(shallow.name(), "julibrot_shallow");
        assert_eq!(perturb.name(), "julibrot_perturbation");
        assert!(!shallow.source().contains("load_reference"));
        assert!(perturb.source().contains("load_reference"));
    }

    fn assert_translates_to_webgl2(
        kernel: &RegisteredKernel,
        shader_stage: naga::ShaderStage,
        entry_point: &str,
    ) {
        let source = kernel.source();
        let module = naga::front::wgsl::parse_str(source).expect("generated WGSL parses");
        let module_info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("generated WGSL validates");
        let options = naga::back::glsl::Options {
            version: naga::back::glsl::Version::Embedded {
                version: 300,
                is_webgl: true,
            },
            ..Default::default()
        };
        let pipeline_options = naga::back::glsl::PipelineOptions {
            shader_stage,
            entry_point: entry_point.to_string(),
            multiview: None,
        };
        let mut glsl = String::new();
        naga::back::glsl::Writer::new(
            &mut glsl,
            &module,
            &module_info,
            &options,
            &pipeline_options,
            naga::proc::BoundsCheckPolicies::default(),
        )
        .expect("generated WGSL lowers to WebGL2 GLSL")
        .write()
        .expect("WebGL2 GLSL writes");
        assert!(glsl.starts_with("#version 300 es"));
    }

    #[test]
    fn both_generated_kernels_translate_for_both_webgl2_stages() {
        for descriptor in [shallow_kernel(), perturbation_kernel()] {
            let kernel = RegisteredKernel::register(&descriptor, LIMITS)
                .expect("kernel body satisfies dialect v2");
            for (shader_stage, entry_point) in [
                (naga::ShaderStage::Vertex, "heap_kernel_vertex"),
                (naga::ShaderStage::Fragment, "heap_kernel_fragment"),
            ] {
                assert_translates_to_webgl2(&kernel, shader_stage, entry_point);
            }
        }
    }

    #[test]
    fn author_sources_are_entry_point_and_binding_free() {
        for source in [SHALLOW_BODY, PERTURB_BODY] {
            for forbidden in [
                "@vertex",
                "@fragment",
                "@compute",
                "@group",
                "@binding",
                "var<storage",
                "var<workgroup",
                "atomic",
                "Barrier",
            ] {
                assert!(!source.contains(forbidden), "found {forbidden}");
            }
            for forbidden in ["ldexp", "frexp"] {
                assert!(
                    source
                        .split(|character: char| {
                            !(character.is_ascii_alphanumeric() || character == '_')
                        })
                        .all(|token| token != forbidden),
                    "found forbidden builtin token {forbidden}"
                );
            }
        }
    }

    #[test]
    fn perturb_source_pins_the_clamp_boundary_and_repeats_normalization() {
        assert!(PERTURB_BODY.contains("if (exponent > 512i)"));
        assert!(PERTURB_BODY.contains("if (exponent < -512i)"));
        assert!(PERTURB_BODY.contains("0x7f800000u"));
        assert!(PERTURB_BODY.contains("let step = clamp(remaining, -126i, 127i);"));
        assert!(PERTURB_BODY.contains("let factor = bitcast<f32>(u32(step + 127i) << 23u);"));
        assert!(PERTURB_BODY.contains("if (steps > 67108863u)"));
        assert!(!PERTURB_BODY.contains("step < 4u"));
        assert!(!PERTURB_BODY.contains("3.402823466e38"));
    }

    #[test]
    fn screen_mapping_reuses_the_existing_builtin_dialect() {
        for source in [SHALLOW_BODY, PERTURB_BODY] {
            assert!(source.contains("0.000000476837158203125"));
            assert!(source.contains("quotient_error.x * quotient_error.x"));
            for absent in ["isFinite(", "determinant(", "inverse(", "length("] {
                assert!(!source.contains(absent), "introduced builtin {absent}");
            }
            assert!(source.contains("if (mapped.status == 2.0)"));
            assert!(source.contains("if (!mapped.sampleable)"));
            assert!(!source.contains("if (mapped.status != 0.0)"));
        }
    }
}
