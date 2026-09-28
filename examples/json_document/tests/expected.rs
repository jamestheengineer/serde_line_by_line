//! Asserts the example's output matches the committed transcript.
//!
//! This is the guarantee that an explanation can never drift from what the code
//! actually does. Regenerate with: cargo test -p json_document -- --ignored
#[test]
fn matches_expected() {
    let actual = json_document::run();
    let expected = include_str!("../expected.txt");
    assert_eq!(actual.trim_end(), expected.trim_end());
}

#[test]
#[ignore = "regenerates the committed transcript"]
fn regenerate() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/expected.txt");
    std::fs::write(path, json_document::run()).unwrap();
}

/// The struct the example runs is the struct `expand/` expands.
///
/// The walk quotes generated code out of `expand/cases/json_document.rs` and
/// quotes output out of this example. Those are two copies of one definition,
/// and nothing else would notice them diverging: each gate would go on passing
/// against its own copy while the unit described an impl that no longer runs.
#[test]
fn the_struct_is_the_expanded_case() {
    let case = include_str!("../../../expand/cases/json_document.rs");
    let source = include_str!("../src/lib.rs");
    assert!(
        source.contains(&format!("#[derive(Debug, Deserialize)]\n{case}")),
        "examples/json_document no longer defines expand/cases/json_document.rs verbatim"
    );
}
