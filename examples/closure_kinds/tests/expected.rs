//! Asserts the example's output matches the committed transcript.
//!
//! This is the guarantee that an explanation can never drift from what the code
//! actually does. Regenerate with: cargo test -p closure_kinds -- --ignored
#[test]
fn matches_expected() {
    let actual = closure_kinds::run();
    let expected = include_str!("../expected.txt");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
#[ignore = "regenerates the committed transcript"]
fn regenerate() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/expected.txt");
    std::fs::write(path, closure_kinds::run()).unwrap();
}
