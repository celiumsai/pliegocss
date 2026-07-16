//! Compile-time contract tests for public macros.

#[test]
fn compile_time_diagnostics_follow_the_contract() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/fail/*.rs");
    tests.pass("tests/ui/pass/*.rs");
}
