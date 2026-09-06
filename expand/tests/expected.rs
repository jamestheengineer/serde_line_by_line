//! Asserts the expansions match the committed transcript.
//!
//! Same guarantee the examples have, applied to generated code: what the site
//! shows a reader cannot drift from what `serde_derive` actually emits.
//! Regenerate with: cargo test -p expand -- --ignored

#[test]
fn matches_expected() {
    let actual = expand::transcript();
    let expected = include_str!("../expected.txt");
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "expansions differ from expected.txt — regenerate with \
         `cargo test -p expand -- --ignored` and read the diff before committing it"
    );
}

/// The trap from the wasm spike, made permanent.
///
/// `serde_derive` names its private module with `CARGO_PKG_VERSION_PATCH`.
/// Compiled as part of this crate that macro would read *our* patch level, so
/// `build.rs` substitutes the pinned one. If that substitution ever stops
/// happening, every expansion on the site is subtly wrong in a way that still
/// compiles — which is exactly the kind of failure a transcript diff would be
/// blamed on and this test names.
#[test]
fn the_private_module_carries_the_pinned_version() {
    let patch = expand::DERIVE_VERSION
        .rsplit('.')
        .next()
        .expect("a version has a patch level");
    let out = expand::expand("struct S { a: u8 }", expand::Derive::Serialize);
    assert!(
        out.contains(&format!("__private{patch}")),
        "expansion does not name __private{patch}; serde_derive {} was expected",
        expand::DERIVE_VERSION
    );
}

/// Every non-error case produces parseable Rust.
///
/// `pretty` falls back to the raw token stream when the output is not a
/// `syn::File`, which is right for `compile_error!` and wrong for everything
/// else — and the fallback is invisible in a transcript unless you know that
/// one long line means it happened.
#[test]
fn expansions_are_real_rust() {
    for (name, source) in expand::CASES {
        if *name == "attribute_error" {
            continue;
        }
        for derive in [expand::Derive::Serialize, expand::Derive::Deserialize] {
            let out = expand::expand(source, derive);
            assert!(
                out.lines().count() > 3,
                "{name} {derive:?} came out as one line, so prettyplease could not \
                 parse it — the expansion is not valid Rust"
            );
            assert!(
                out.contains("_serde"),
                "{name} {derive:?} does not mention _serde"
            );
        }
    }
}

#[test]
#[ignore = "regenerates the committed transcript"]
fn regenerate() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/expected.txt");
    std::fs::write(path, expand::transcript()).unwrap();
}
