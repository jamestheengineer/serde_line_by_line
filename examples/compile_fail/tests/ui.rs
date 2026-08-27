//! Runs every case in `tests/ui/` and diffs the diagnostics against the
//! committed `.stderr` files.
//!
//! A case that starts compiling is a failure here, which is the point: these
//! files exist to prove the compiler still rejects what the course says it
//! rejects.
//!
//! Regenerate the expected output after a rustc upgrade with:
//!   TRYBUILD=overwrite cargo test -p compile_fail
#[test]
fn ui() {
    trybuild::TestCases::new().compile_fail("tests/ui/*.rs");
}
