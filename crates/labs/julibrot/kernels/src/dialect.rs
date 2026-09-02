use ember_lab_heap::KernelDesc;

pub const OUTPUT_PAGE_SIDE: u16 = 256;

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
        uniform_size: 96,
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
        uniform_size: 64,
        output_page_side: OUTPUT_PAGE_SIDE,
    }
}

#[cfg(test)]
mod tests {
    use super::{PERTURB_BODY, SHALLOW_BODY, perturbation_kernel, shallow_kernel};
    use ember_lab_heap::{DialectLimits, RegisteredKernel};

    const LIMITS: DialectLimits = DialectLimits {
        descriptor_capacity: 64,
        span_capacity: 16,
        handle_capacity: 64,
    };

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
        }
    }

    #[test]
    fn perturb_source_pins_the_clamp_boundary_and_repeats_normalization() {
        assert!(PERTURB_BODY.contains("if (exponent > 512i)"));
        assert!(PERTURB_BODY.contains("if (exponent < -512i)"));
        assert!(PERTURB_BODY.contains("0x7f800000u"));
        assert!(PERTURB_BODY.contains("if (steps > 67108863u)"));
        assert!(!PERTURB_BODY.contains("step < 4u"));
        assert_eq!(
            PERTURB_BODY
                .matches("return ldexp(value, exponent);")
                .count(),
            1
        );
    }
}
