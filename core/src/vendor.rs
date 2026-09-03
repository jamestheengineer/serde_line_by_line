//! Vendor integrity.
//!
//! Annotations are keyed to line numbers in the pinned source, so any drift in
//! `vendor/` silently invalidates content. The pin file records a deterministic
//! hash of the vendored source tree; CI fails if it moves.
//!
//! Which version is pinned is read from `vendor/pin.toml` rather than compiled
//! in. A version bump rewrites the pin and the annotation store together (see
//! `slbl_core::remap` and `cargo xtask bump`), and a constant would mean the
//! tool that performs the migration disagrees with the tree it just wrote.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const CRATE_NAME: &str = "serde_core";

/// The identifier annotation files carry in their `source` field, and the name
/// of the directory under `vendor/`.
pub fn source_id(version: &str) -> String {
    format!("{CRATE_NAME}-{version}")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Pin {
    pub version: String,
    /// sha256 of the published .crate archive, as reported by crates.io.
    /// Informational: recorded for provenance, not recomputable from the
    /// extracted tree.
    pub crate_sha256: String,
    /// sha256 over the extracted source tree. This is the value that actually
    /// protects annotation line ranges.
    pub src_tree_sha256: String,
}

impl Pin {
    pub fn source_id(&self) -> String {
        source_id(&self.version)
    }
}

/// Where a given version's tree lives, whether or not it is the pinned one.
pub fn crate_dir(repo: &Path, version: &str) -> PathBuf {
    repo.join("vendor").join(source_id(version))
}

/// The pinned crate's directory.
pub fn vendor_root(repo: &Path) -> Result<PathBuf> {
    Ok(crate_dir(repo, &load_pin(repo)?.version))
}

pub fn pin_path(repo: &Path) -> PathBuf {
    repo.join("vendor").join("pin.toml")
}

/// Every `.rs` file under a crate root's `src/`, as paths relative to that root
/// ("src/ser/mod.rs"), sorted for determinism.
pub fn source_files_in(root: &Path) -> Result<Vec<String>> {
    let src = root.join("src");
    if !src.is_dir() {
        bail!("vendored source not found at {}", src.display());
    }
    let mut out = Vec::new();
    collect(&src, root, &mut out)?;
    out.sort();
    Ok(out)
}

pub fn source_files(repo: &Path) -> Result<Vec<String>> {
    source_files_in(&vendor_root(repo)?)
}

fn collect(dir: &Path, base: &Path, out: &mut Vec<String>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, base, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let rel = path
                .strip_prefix(base)?
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

/// Deterministic hash over (relative path, byte length, contents) for every
/// source file under a crate root.
pub fn tree_hash_of(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    for rel in source_files_in(root)? {
        let bytes = std::fs::read(root.join(&rel))?;
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes.len().to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn tree_hash(repo: &Path) -> Result<String> {
    tree_hash_of(&vendor_root(repo)?)
}

pub fn line_count(repo: &Path, rel: &str) -> Result<u32> {
    let path = vendor_root(repo)?.join(rel);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading source {}", path.display()))?;
    // A trailing newline does not create a final empty line for our purposes.
    let n = text.lines().count();
    Ok(n as u32)
}

pub fn load_pin(repo: &Path) -> Result<Pin> {
    let path = pin_path(repo);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading pin {} (run `cargo xtask pin`)", path.display()))?;
    toml::from_str(&text).context("parsing vendor/pin.toml")
}

/// Serializes a pin in the committed format, comments and all.
pub fn render_pin(pin: &Pin) -> String {
    format!(
        "# Regenerate with `cargo xtask pin`; change the version with `cargo xtask bump`.\n\
         # src_tree_sha256 protects annotation line ranges: if it moves, the\n\
         # coverage gate fails until the change is an explicit migration.\n\
         version = \"{}\"\n\
         crate_sha256 = \"{}\"\n\
         src_tree_sha256 = \"{}\"\n",
        pin.version, pin.crate_sha256, pin.src_tree_sha256
    )
}

pub fn verify(repo: &Path) -> Result<()> {
    let pin = load_pin(repo)?;
    let actual = tree_hash(repo)?;
    if actual != pin.src_tree_sha256 {
        bail!(
            "vendored source has drifted!\n  expected src_tree_sha256 {}\n  actual   src_tree_sha256 {}\n\
             \nAnnotation line ranges are keyed to the pinned tree. Either restore\n\
             vendor/{}/ or run `cargo xtask bump <version>` to migrate them.",
            pin.src_tree_sha256,
            actual,
            pin.source_id(),
        );
    }
    Ok(())
}
