//! Asserts the example's output matches the committed transcript.
//!
//! This is the guarantee that an explanation can never drift from what the code
//! actually does. Regenerate with: cargo test -p collect_str -- --ignored
#[test]
fn matches_expected() {
    let actual = collect_str::run();
    let expected = include_str!("../expected.txt");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
#[ignore = "regenerates the committed transcript"]
fn regenerate() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/expected.txt");
    std::fs::write(path, collect_str::run()).unwrap();
}
