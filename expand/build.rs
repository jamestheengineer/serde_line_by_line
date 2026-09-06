//! Builds the pinned `serde_derive` as an ordinary library.
//!
//! `serde_derive` is a proc-macro crate: its modules are private behind two
//! `#[proc_macro_derive]` entry points, and a proc-macro crate cannot be linked
//! as a normal dependency. So the vendored tree is copied into `OUT_DIR` and
//! four small edits are applied to `lib.rs` — nothing else in the crate's 8,975
//! lines is touched.
//!
//! The copy is what gets edited. `vendor/` is checksum-pinned and stays
//! byte-for-byte the published crate; editing it in place would break the gate
//! that makes every line range in this repo trustworthy.
//!
//! Each edit is a replacement that must apply. If upstream changes shape, the
//! build fails here naming the edit that no longer matches, rather than
//! silently producing an expander that is subtly not `serde_derive`.

use std::path::{Path, PathBuf};

#[derive(serde::Deserialize)]
struct Pin {
    #[serde(rename = "source")]
    sources: Vec<Source>,
}

#[derive(serde::Deserialize)]
struct Source {
    name: String,
    version: String,
}

/// `(what it is for, from, to)`.
const EDITS: &[(&str, &str, &str)] = &[
    (
        "drop the compiler-provided proc_macro crate",
        "extern crate proc_macro;\n",
        "",
    ),
    (
        "take proc-macro2's TokenStream, which works off the compiler thread",
        "use proc_macro::TokenStream;",
        "use proc_macro2::TokenStream;",
    ),
    (
        "parse_macro_input! needs proc_macro::TokenStream; parse2 does not",
        "use syn::parse_macro_input;\n",
        "",
    ),
    (
        "Serialize entry point: no derive attribute, parse2, no .into()",
        "\
#[proc_macro_derive(Serialize, attributes(serde))]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);
    ser::expand_derive_serialize(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}",
        "\
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let mut input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.into_compile_error(),
    };
    ser::expand_derive_serialize(&mut input).unwrap_or_else(syn::Error::into_compile_error)
}",
    ),
    (
        "Deserialize entry point: the same three changes",
        "\
#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(input as DeriveInput);
    de::expand_derive_deserialize(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}",
        "\
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let mut input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(err) => return err.into_compile_error(),
    };
    de::expand_derive_deserialize(&mut input).unwrap_or_else(syn::Error::into_compile_error)
}",
    ),
];

fn main() {
    // serde_derive gates some of its own code on cfgs its build script sets.
    // We do not run that build script — nothing here needs those branches —
    // but cargo's check-cfg does not know that, and this repo builds with
    // `-D warnings`.
    for name in ["check_cfg", "exhaustive"] {
        println!("cargo::rustc-check-cfg=cfg({name})");
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().expect("expand/ has a parent");
    let pin_path = repo.join("vendor").join("pin.toml");
    println!("cargo:rerun-if-changed={}", pin_path.display());

    let pin: Pin = toml::from_str(
        &std::fs::read_to_string(&pin_path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", pin_path.display())),
    )
    .expect("parsing vendor/pin.toml");
    let source = pin
        .sources
        .iter()
        .find(|s| s.name == "serde_derive")
        .expect("vendor/pin.toml does not pin serde_derive");

    let src = repo
        .join("vendor")
        .join(format!("{}-{}", source.name, source.version))
        .join("src");
    println!("cargo:rerun-if-changed={}", src.display());

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let dest = out.join("serde_derive_lib");
    let _ = std::fs::remove_dir_all(&dest);
    copy_dir(&src, &dest);

    let lib = dest.join("lib.rs");
    let mut text = std::fs::read_to_string(&lib).expect("reading the copied lib.rs");
    for (what, from, to) in EDITS {
        assert!(
            text.contains(from),
            "serde_derive {} no longer contains the text this edit expects.\n\
             edit: {what}\n\
             Upstream changed shape; see expand/build.rs.",
            source.version,
        );
        text = text.replace(from, to);
    }

    // The private module's name comes from the *compiling* crate's version, so
    // left alone it would read `__private1` from this crate's 0.1.0 rather than
    // `__private229` from serde_derive. Substituting the pinned patch level
    // makes the generated code correct by construction instead of by a manifest
    // field someone has to remember to keep in step.
    let patch = source
        .version
        .rsplit('.')
        .next()
        .expect("a version has a patch level");
    let version_macro = "env!(\"CARGO_PKG_VERSION_PATCH\")";
    assert!(
        text.contains(version_macro),
        "serde_derive {} no longer names its private module after CARGO_PKG_VERSION_PATCH; \
         the substitution in expand/build.rs is now wrong rather than unnecessary",
        source.version,
    );
    text = text.replace(version_macro, &format!("\"{patch}\""));

    std::fs::write(&lib, text).expect("writing the patched lib.rs");

    // serde_derive's own code addresses itself as `crate::`, which resolved to
    // its crate root and now resolves to ours. Every one of those becomes
    // `crate::serde_derive_lib::`. The two `$crate::` uses in `fragment.rs`
    // are rewritten by the same replacement and stay correct: `$crate` is the
    // crate the macro was defined in, which is this one.
    //
    // `pub(crate)` is untouched because it has no `::`.
    let rewritten = retarget_crate_paths(&dest);
    assert!(
        rewritten > 0,
        "no `crate::` paths found in the copied serde_derive tree — the copy is \
         probably empty or in the wrong place"
    );

    // `#[path]` needs a string literal, so the shim is generated with the
    // absolute path baked in and included by src/lib.rs.
    std::fs::write(
        out.join("derive_lib.rs"),
        format!(
            // Vendored code, held to upstream's standards rather than this
            // repo's `-D warnings`: the lints it silences for itself are inner
            // attributes in its own lib.rs, which cannot survive being made a
            // submodule.
            "#[path = {:?}]\n#[allow(warnings, clippy::all, clippy::pedantic)]\nmod serde_derive_lib;\n",
            lib.to_str().expect("OUT_DIR is valid UTF-8")
        ),
    )
    .expect("writing the module shim");

    generate_cases(&manifest, &out);

    println!("cargo:rustc-env=SLBL_DERIVE_VERSION={}", source.version);
}

/// Bakes `cases/*.rs` into the binary, so the browser build carries the same
/// inputs the native transcript is generated from.
fn generate_cases(manifest: &Path, out: &Path) {
    let dir = manifest.join("cases");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rs"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "expand/cases/ is empty");

    let mut src = String::from("[\n");
    for file in &names {
        let name = file.trim_end_matches(".rs");
        let path = dir.join(file);
        let body = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        src.push_str(&format!("    ({name:?}, {body:?}),\n"));
    }
    src.push(']');
    std::fs::write(out.join("cases.rs"), src).expect("writing the case table");
}

/// Rewrites `crate::` to `crate::serde_derive_lib::` across the copied tree,
/// returning how many it changed.
fn retarget_crate_paths(dir: &Path) -> usize {
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
    {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            n += retarget_crate_paths(&path);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        let count = text.matches("crate::").count();
        if count == 0 {
            continue;
        }
        let text = text.replace("crate::", "crate::serde_derive_lib::");
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        n += count;
    }
    n
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("creating {}: {e}", to.display()));
    for entry in
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("reading {}: {e}", from.display()))
    {
        let entry = entry.expect("a directory entry");
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &dest);
        } else {
            std::fs::copy(&path, &dest)
                .unwrap_or_else(|e| panic!("copying {}: {e}", path.display()));
        }
    }
}
