//! Project automation for serde_line_by_line.
//!
//! Usage:
//!   cargo xtask coverage [--json]   verify annotations against the pinned source
//!   cargo xtask bump <version>      migrate the store to a new serde_core release
//!   cargo xtask pin                 recompute vendor/pin.toml from the vendored tree
//!   cargo xtask stats               structural inventory of the pinned source
//!   cargo xtask wasm                build the example playground for the browser

mod bump;
mod coverage;
mod stats;
mod store_edit;
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
        "bump" => bump::run(&repo, &bump::parse_args(&args[1..])?)?,
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
         cargo xtask bump <version>      migrate the annotation store to a new release\n\
         \x20   --dry-run                  report the migration without writing anything\n\
         \x20   --archive PATH             use a local .crate instead of downloading\n\
         \x20   --sha256 HEX               expected archive checksum, when offline\n\
         \x20   --allow-orphans            drop annotations whose lines no longer exist\n\
         \x20   --keep-old                 leave the previous vendor/ tree in place\n\
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

/// Recomputes the tree hash for the version the pin already names.
///
/// This deliberately cannot change the version: doing that without also
/// remapping every line range would leave a tree that hashes correctly and an
/// annotation store pointing at the wrong lines. `cargo xtask bump` is the
/// command that moves a version.
fn pin(repo: &Path) -> Result<()> {
    let mut existing = vendor::load_pin(repo)?;
    existing.src_tree_sha256 = vendor::tree_hash(repo)?;
    let path = vendor::pin_path(repo);
    std::fs::write(&path, vendor::render_pin(&existing))?;
    println!("src_tree_sha256 = {}", existing.src_tree_sha256);
    println!("wrote {}", path.display());
    Ok(())
}
