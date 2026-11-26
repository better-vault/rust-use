//! Diagnostics for invalid macro inputs.

#[test]
fn diagnostics() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
