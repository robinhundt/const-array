//! Compile-fail tests: length requirements must be compile errors, and they
//! must surface during `cargo check`, not only after monomorphization.

#[test]
#[cfg_attr(miri, ignore)]
fn compile_fail() {
    // The expected errors are rustc's exact output, which changes between
    // releases. CI checks them on a pinned toolchain and sets this variable to
    // skip them on all others. Update the `.stderr` files with
    // `TRYBUILD=overwrite cargo test --test compile_fail`.
    if std::env::var_os("CONST_ARRAY_SKIP_COMPILE_FAIL").is_some() {
        eprintln!("skipping compile_fail tests, CONST_ARRAY_SKIP_COMPILE_FAIL is set");
        return;
    }
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
