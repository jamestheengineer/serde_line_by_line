//! Project automation for serde_line_by_line.
//!
//! Usage:
//!   cargo xtask coverage [--json]   verify annotations against the pinned source
//!   cargo xtask pin                 recompute vendor/pin.toml from the vendored tree
//!   cargo xtask stats               structural inventory of the pinned source
//!   cargo xtask wasm                build the example playground for the browser

mod coverage;
mod stats;
mod wasm;

use anyhow::{bail, Result};
use slbl_core::vendor;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let repo = repo_root()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("coverage");

    match cmd {
        "coverage" => {
            let json = args.iter().any(|a| a == "--json");
            coverage::run(&repo, json)?;
        }
        "pin" => pin(&repo)?,
        "wasm" => wasm::run(&repo)?,
        "stats" => stats::run(&repo)?,
        "-h" | "--help" | "help" => print_help(),
        other => {
            print_help();
            bail!("unknown command {other:?}");
        }
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "serde_line_by_line xtask\n\n\
         cargo xtask coverage [--json]   verify annotations against the pinned source\n\
         cargo xtask pin                 recompute vendor/pin.toml\n\
         cargo xtask stats               structural inventory of the pinned source\n\
         cargo xtask wasm                build the example playground for the browser"
    );
}

/// The workspace root, from CARGO_MANIFEST_DIR (xtask/) upward.
fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("xtask has no parent directory"))?
        .to_path_buf();
    if !root.join("Cargo.toml").is_file() {
        bail!("workspace root not found at {}", root.display());
    }
    Ok(root)
}

fn pin(repo: &Path) -> Result<()> {
    let hash = vendor::tree_hash(repo)?;
    let path = vendor::pin_path(repo);
    let existing = vendor::load_pin(repo).ok();
    let crate_sha = existing
        .as_ref()
        .map(|p| p.crate_sha256.clone())
        .unwrap_or_default();
    let version = existing
        .as_ref()
        .map(|p| p.version.clone())
        .unwrap_or_else(|| "1.0.229".to_string());

    let text = format!(
        "# Regenerate with `cargo xtask pin`.\n\
         # src_tree_sha256 protects annotation line ranges: if it moves, the\n\
         # coverage gate fails until the change is an explicit migration.\n\
         version = \"{version}\"\n\
         crate_sha256 = \"{crate_sha}\"\n\
         src_tree_sha256 = \"{hash}\"\n"
    );
    std::fs::write(&path, text)?;
    println!("src_tree_sha256 = {hash}");
    println!("wrote {}", path.display());
    Ok(())
}
