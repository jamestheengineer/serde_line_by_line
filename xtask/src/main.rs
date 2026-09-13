//! Project automation for serde_line_by_line.
//!
//! Usage:
//!   cargo xtask coverage [--json]   verify annotations against the pinned source
//!   cargo xtask bump [--source NAME] <version>
//!                                   migrate the stores to a new release of a pinned source
//!   cargo xtask pin                 rehash every vendored tree into pin.toml + NOTICE.md
//!   cargo xtask stats [--dir PATH]  structural inventory of the pinned source,
//!                                   or of an unpinned candidate tree
//!   cargo xtask wasm                build the example playground for the browser

mod bump;
mod coverage;
mod harness;
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
        "stats" => stats::run(&repo, stats_dir(&args[1..])?.as_deref())?,
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
         cargo xtask bump <version>      migrate the stores to a new release\n\
         \x20   --source NAME              which pinned source to move (default: the\n\
         \x20                              coverage source)\n\
         \x20   --dry-run                  report the migration without writing anything\n\
         \x20   --archive PATH             use a local .crate instead of downloading\n\
         \x20   --sha256 HEX               expected archive checksum, when offline\n\
         \x20   --allow-orphans            drop records whose lines no longer exist\n\
         \x20   --keep-old                 leave the previous vendor/ tree in place\n\
         cargo xtask pin                 rehash vendor/pin.toml and NOTICE.md\n\
         cargo xtask stats               structural inventory of the pinned source\n\
         \x20   --dir PATH                 measure an unpinned tree instead (any\n\
         \x20                              directory with a src/)\n\
         cargo xtask wasm                build the example playground for the browser"
    );
}

/// `--dir PATH` for `stats`, which is the one command that can run outside the
/// pin: a candidate crate is measured before anyone decides to vendor it.
fn stats_dir(args: &[String]) -> Result<Option<PathBuf>> {
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--dir" {
            let path = it
                .next()
                .ok_or_else(|| anyhow::anyhow!("--dir takes a path"))?;
            let path = PathBuf::from(path);
            if !path.join("src").is_dir() {
                bail!("{} has no src/ directory", path.display());
            }
            return Ok(Some(path));
        }
    }
    Ok(None)
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

/// Recomputes the tree hash of every source the pin already names.
///
/// This deliberately cannot change a version: doing that without also
/// remapping every line range would leave a tree that hashes correctly and an
/// annotation store pointing at the wrong lines. `cargo xtask bump` is the
/// command that moves a version.
fn pin(repo: &Path) -> Result<()> {
    let mut existing = vendor::load_pin(repo)?;
    for source in &mut existing.sources {
        let root = source.dir(repo);
        if !root.is_dir() {
            anyhow::bail!("vendor/{} is missing", source.source_id());
        }
        source.src_tree_sha256 = vendor::tree_hash_of(&root)?;
        println!("{:<28} {}", source.source_id(), source.src_tree_sha256);
    }
    let path = vendor::pin_path(repo);
    std::fs::write(&path, vendor::render_pin(&existing))?;
    println!("wrote {}", path.display());

    // NOTICE.md is generated from the same pin, so a source can never be
    // vendored and left unattributed.
    let notice_path = repo.join("vendor").join("NOTICE.md");
    std::fs::write(&notice_path, bump::notice(repo, &existing)?)?;
    println!("wrote {}", notice_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn stats_without_dir_measures_the_pin() {
        assert!(stats_dir(&args(&[])).unwrap().is_none());
    }

    #[test]
    fn stats_dir_takes_any_tree_with_a_src() {
        let core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../core");
        let got = stats_dir(&args(&["--dir", core.to_str().unwrap()])).unwrap();
        assert_eq!(got, Some(core));
    }

    /// A typo'd path measured as if it were empty would report a crate with no
    /// files and no error, which is the one outcome a scoping measurement must
    /// not produce.
    #[test]
    fn stats_dir_rejects_a_tree_with_no_src() {
        let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs");
        let err = stats_dir(&args(&["--dir", docs.to_str().unwrap()])).unwrap_err();
        assert!(err.to_string().contains("no src/"), "{err}");
    }

    #[test]
    fn stats_dir_needs_a_path() {
        assert!(stats_dir(&args(&["--dir"])).is_err());
    }
}
