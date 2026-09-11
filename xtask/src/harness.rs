//! The expansion harness, and the versions it actually compiles.
//!
//! `expand/` builds the pinned `serde_derive` as an ordinary library (N3), and
//! it builds it against `syn`, `quote` and `proc-macro2` — the three sources
//! the glossary quotes. Nothing in Cargo connects those two facts.
//! `expand/build.rs` reads `vendor/pin.toml` to find the `serde_derive` tree,
//! but the parser it hands that tree to is whatever the manifest resolves to,
//! and a caret requirement resolves to whatever was published most recently.
//!
//! That is drift arriving through the other door, the one the vendor checksum
//! does not watch. The tree stays pinned, the quotations stay honest about the
//! tree — and the code the site runs is built from a different release than
//! the glossary describes. Nothing fails; the site is confidently wrong about
//! its own expander, which is the exact failure D7 exists to prevent for
//! `serde_core`.
//!
//! So the requirement is pinned with `=`, `cargo xtask bump` retargets it, and
//! `cargo xtask coverage` checks it against the pin and against the lockfile —
//! the lockfile because the manifest says what is allowed and the lock says
//! what is compiled.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Repo-relative, for messages.
pub const MANIFEST: &str = "expand/Cargo.toml";

pub fn manifest_path(repo: &Path) -> PathBuf {
    repo.join("expand").join("Cargo.toml")
}

/// The version requirement the harness gives one dependency, with the index of
/// the line carrying it.
///
/// Both spellings the manifest uses are understood: `name = "req"` and
/// `name = { version = "req", ... }`. A dependency table spread over several
/// lines still declares its version on the first, which is the one returned.
pub fn dep_req(text: &str, name: &str) -> Option<(usize, String)> {
    let (i, line) = text.lines().enumerate().find(|(_, l)| {
        l.strip_prefix(name)
            .is_some_and(|rest| rest.starts_with(' ') || rest.starts_with('='))
    })?;
    let value = match line.split_once("version") {
        // `name = { version = "..." }`
        Some((before, after)) if before.contains('{') => after.split_once('=')?.1,
        // `name = "..."`
        _ => line.split_once('=')?.1,
    };
    let req = value.trim().trim_start_matches('"');
    let req = req.split('"').next()?;
    Some((i, req.to_string()))
}

/// Retargets the harness's requirement on `name`, if it has one.
///
/// Returns whether anything was written. Most bumps write nothing here: the
/// coverage source is not a harness dependency, and `serde_derive` is compiled
/// out of `vendor/` rather than resolved by Cargo.
pub fn retarget(repo: &Path, name: &str, new_version: &str) -> Result<bool> {
    let path = manifest_path(repo);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let Some((i, req)) = dep_req(&text, name) else {
        return Ok(false);
    };
    let want = format!("={new_version}");
    if req == want {
        return Ok(false);
    }
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    lines[i] = lines[i].replace(&format!("\"{req}\""), &format!("\"{want}\""));
    let mut out = lines.join("\n");
    out.push('\n');
    std::fs::write(&path, out)?;
    Ok(true)
}

/// Whether `Cargo.lock` resolved `name` to exactly `version`.
///
/// A crate can appear more than once — `syn` 2 is pulled in by the
/// highlighter while the harness is on `syn` 3 — so this asks whether the
/// pinned version is among them, not whether it is the only one.
pub fn lock_has(repo: &Path, name: &str, version: &str) -> Result<bool> {
    let path = repo.join("Cargo.lock");
    if !path.is_file() {
        return Ok(true);
    }
    let text = std::fs::read_to_string(&path)?;
    let want_name = format!("name = \"{name}\"");
    let want_version = format!("version = \"{version}\"");
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim() == want_name && lines.next().is_some_and(|l| l.trim() == want_version) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST_TEXT: &str = r#"[dependencies]
proc-macro2 = { version = "=1.0.107", default-features = false }
syn = { version = "=3.0.3", default-features = false, features = [
    "derive",
] }
prettyplease = "0.3"

[build-dependencies]
serde = { version = "1", features = ["derive"] }
"#;

    #[test]
    fn both_spellings_of_a_requirement_are_read() {
        assert_eq!(dep_req(MANIFEST_TEXT, "syn").unwrap().1, "=3.0.3");
        assert_eq!(dep_req(MANIFEST_TEXT, "proc-macro2").unwrap().1, "=1.0.107");
        assert_eq!(dep_req(MANIFEST_TEXT, "prettyplease").unwrap().1, "0.3");
        assert!(dep_req(MANIFEST_TEXT, "quote").is_none());
    }

    /// `serde` is a build-dependency here and `serde_core` is not a dependency
    /// at all; neither may be matched by a prefix of the other's name.
    #[test]
    fn a_name_is_not_matched_by_a_prefix() {
        assert_eq!(dep_req(MANIFEST_TEXT, "serde").unwrap().1, "1");
        assert!(dep_req(MANIFEST_TEXT, "serde_core").is_none());
    }
}
