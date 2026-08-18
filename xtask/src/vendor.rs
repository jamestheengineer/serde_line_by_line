//! Vendor integrity.
//!
//! Annotations are keyed to line numbers in the pinned source, so any drift in
//! `vendor/` silently invalidates content. The pin file records a deterministic
//! hash of the vendored source tree; CI fails if it moves.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const SOURCE_ID: &str = "serde_core-1.0.229";

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

pub fn vendor_root(repo: &Path) -> PathBuf {
    repo.join("vendor").join(SOURCE_ID)
}

pub fn pin_path(repo: &Path) -> PathBuf {
    repo.join("vendor").join("pin.toml")
}

/// Every `.rs` file under the vendored `src/`, as paths relative to the crate
/// root ("src/ser/mod.rs"), sorted for determinism.
pub fn source_files(repo: &Path) -> Result<Vec<String>> {
    let root = vendor_root(repo);
    let src = root.join("src");
    if !src.is_dir() {
        bail!("vendored source not found at {}", src.display());
    }
    let mut out = Vec::new();
    collect(&src, &root, &mut out)?;
    out.sort();
    Ok(out)
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
/// vendored source file.
pub fn tree_hash(repo: &Path) -> Result<String> {
    let root = vendor_root(repo);
    let mut hasher = Sha256::new();
    for rel in source_files(repo)? {
        let bytes = std::fs::read(root.join(&rel))?;
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(bytes.len().to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn line_count(repo: &Path, rel: &str) -> Result<u32> {
    let path = vendor_root(repo).join(rel);
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

pub fn verify(repo: &Path) -> Result<()> {
    let pin = load_pin(repo)?;
    let actual = tree_hash(repo)?;
    if actual != pin.src_tree_sha256 {
        bail!(
            "vendored source has drifted!\n  expected src_tree_sha256 {}\n  actual   src_tree_sha256 {}\n\
             \nAnnotation line ranges are keyed to the pinned tree. Either restore\n\
             vendor/{SOURCE_ID}/ or perform an explicit version migration.",
            pin.src_tree_sha256,
            actual
        );
    }
    Ok(())
}
