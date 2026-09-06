//! Runs the pinned `serde_derive` on demand.
//!
//! The point of this crate is that the expansion a reader sees is produced by
//! `serde_derive` itself, not by a description of it. `build.rs` makes the
//! vendored proc-macro crate linkable (see there for the five edits); this
//! module is the thin layer that turns a snippet of Rust into the code the
//! compiler would really have generated.
//!
//! It has no `proc-macro` feature anywhere in its dependency tree, which is
//! what lets the same code run under `cargo test` and in the browser.

include!(concat!(env!("OUT_DIR"), "/derive_lib.rs"));

/// The `serde_derive` release this expander is built from, from `vendor/pin.toml`.
pub const DERIVE_VERSION: &str = env!("SLBL_DERIVE_VERSION");

/// Which derive to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Derive {
    Serialize,
    Deserialize,
}

impl Derive {
    pub fn parse(name: &str) -> Option<Derive> {
        match name {
            "Serialize" => Some(Derive::Serialize),
            "Deserialize" => Some(Derive::Deserialize),
            _ => None,
        }
    }
}

/// Expands one derive over `input`, returning formatted Rust.
///
/// Errors come back as source too, as a `compile_error!` invocation — that is
/// what `serde_derive` really emits, and showing a reader serde's own
/// diagnostic is more honest than paraphrasing it into a message of ours.
pub fn expand(input: &str, derive: Derive) -> String {
    let tokens: proc_macro2::TokenStream = match input.parse() {
        Ok(tokens) => tokens,
        Err(err) => return format!("// not valid Rust tokens: {err}"),
    };
    let expanded = match derive {
        Derive::Serialize => serde_derive_lib::derive_serialize(tokens),
        Derive::Deserialize => serde_derive_lib::derive_deserialize(tokens),
    };
    pretty(expanded)
}

/// Formats generated tokens, falling back to the unformatted stream.
///
/// The fallback matters for the error path: `compile_error! { "…" }` is not a
/// parseable `syn::File`, so an unformattable expansion is expected rather than
/// exceptional.
fn pretty(tokens: proc_macro2::TokenStream) -> String {
    match syn::parse2::<syn::File>(tokens.clone()) {
        Ok(file) => prettyplease::unparse(&file),
        Err(_) => tokens.to_string(),
    }
}

/// The committed cases, as `(name, source)`.
///
/// Generated from `expand/cases/`, so adding a case cannot silently fail to
/// appear on the site — the same reason the playground generates its dispatch
/// table rather than listing examples by hand.
pub const CASES: &[(&str, &str)] = &include!(concat!(env!("OUT_DIR"), "/cases.rs"));

pub fn case(name: &str) -> Option<&'static str> {
    CASES.iter().find(|(n, _)| *n == name).map(|(_, src)| *src)
}

/// Every case expanded both ways, in the transcript's format.
///
/// One function shared by the native test and the browser check, so "the site
/// shows what CI asserts" is a property of there being one implementation
/// rather than of two staying in step.
pub fn transcript() -> String {
    let mut out = String::new();
    for (name, source) in CASES {
        for (label, derive) in [
            ("Serialize", Derive::Serialize),
            ("Deserialize", Derive::Deserialize),
        ] {
            out.push_str(&format!("### {name} {label}\n"));
            out.push_str(expand(source, derive).trim_end());
            out.push_str("\n\n");
        }
    }
    out
}
