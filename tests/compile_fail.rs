//! Compile-fail tests: length requirements must be compile errors, and they
//! must surface during `cargo check`, not only after monomorphization.

#[test]
#[cfg_attr(miri, ignore)]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
