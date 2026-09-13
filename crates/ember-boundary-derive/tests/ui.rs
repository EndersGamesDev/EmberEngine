#[test]
fn rejected_shapes_are_compile_errors() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/*.rs");
}
