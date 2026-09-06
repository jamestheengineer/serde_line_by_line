//! Vendor integrity.
//!
//! Annotations and glossary entries are keyed to line numbers in a pinned
//! source, so any drift in `vendor/` silently invalidates content. The pin
//! file records a deterministic hash of every vendored tree; CI fails if one
//! moves.
//!
//! What is pinned is read from `vendor/pin.toml` rather than compiled in. A
//! version bump rewrites the pin and the annotation store together (see
//! `slbl_core::remap` and `cargo xtask bump`), and a constant would mean the
//! tool that performs the migration disagrees with the tree it just wrote.
//!
//! Sources carry a [`Role`], because the gates ask different things of them.
//! That distinction is the whole of D9: a crate can be quoted without being
//! claimed, so long as the pin says which one it is.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// What the gates ask of a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Every line must be claimed once `manifest.toml` calls a file complete.
    /// The reference track's promise, and the only role coverage percentages
    /// mean anything for.
    Coverage,
    /// Annotated where the story goes, never asked to be exhaustive
    /// (PLAN.md §11).
    Narrative,
    /// Never annotated; quoted verbatim by glossary entries (D9). The pin is
    /// what keeps a quotation honest.
    Glossary,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Source {
    /// The crates.io name, hyphens and all: "proc-macro2".
    pub name: String,
    pub version: String,
    pub role: Role,
    /// sha256 of the published .crate archive, as reported by crates.io.
    /// Informational: recorded for provenance, not recomputable from the
    /// extracted tree.
    pub crate_sha256: String,
    /// sha256 over the extracted source tree. This is the value that actually
    /// protects line ranges.
    pub src_tree_sha256: String,
}

impl Source {
    /// The identifier annotation files carry in their `source` field, and the
    /// name of the directory under `vendor/`.
    pub fn source_id(&self) -> String {
        source_id(&self.name, &self.version)
    }

    pub fn dir(&self, repo: &Path) -> PathBuf {
        crate_dir(repo, &self.name, &self.version)
    }
}

/// `("serde_core", "1.0.229")` → `"serde_core-1.0.229"`.
pub fn source_id(name: &str, version: &str) -> String {
    format!("{name}-{version}")
}

/// Where a given version's tree lives, whether or not it is the pinned one.
pub fn crate_dir(repo: &Path, name: &str, version: &str) -> PathBuf {
    repo.join("vendor").join(source_id(name, version))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pin {
    #[serde(rename = "source")]
    pub sources: Vec<Source>,
}

impl Pin {
    /// The one `coverage` source. Everything written before PLAN.md §11
    /// assumes a single pinned crate, and for those callers this is it —
    /// exactly one is allowed, because two crates each promising 100% would
    /// make "the" coverage table ambiguous with no reader-visible gain.
    pub fn primary(&self) -> Result<&Source> {
        let mut it = self.sources.iter().filter(|s| s.role == Role::Coverage);
        let first = it
            .next()
            .context("vendor/pin.toml has no source with role = \"coverage\"")?;
        if let Some(second) = it.next() {
            bail!(
                "vendor/pin.toml has two coverage sources ({} and {}); \
                 exactly one is supported",
                first.name,
                second.name
            );
        }
        Ok(first)
    }

    pub fn get(&self, name: &str) -> Result<&Source> {
        self.sources
            .iter()
            .find(|s| s.name == name)
            .with_context(|| format!("no source named {name:?} in vendor/pin.toml"))
    }

    pub fn by_role(&self, role: Role) -> impl Iterator<Item = &Source> {
        self.sources.iter().filter(move |s| s.role == role)
    }
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

/// The primary (coverage) source's directory.
pub fn vendor_root(repo: &Path) -> Result<PathBuf> {
    Ok(load_pin(repo)?.primary()?.dir(repo))
}

pub fn pin_path(repo: &Path) -> PathBuf {
    repo.join("vendor").join("pin.toml")
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
    line_count_in(&vendor_root(repo)?, rel)
}

/// Lines in one file of a given crate root.
pub fn line_count_in(root: &Path, rel: &str) -> Result<u32> {
    let path = root.join(rel);
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
    let mut out = String::from(
        "# Regenerate with `cargo xtask pin`; change a version with `cargo xtask bump`.\n\
         # src_tree_sha256 protects annotation and glossary line ranges: if it moves,\n\
         # the coverage gate fails until the change is an explicit migration.\n\
         #\n\
         # role decides what the gates ask of a source:\n\
         #   coverage  — every line must be claimed once manifest.toml calls a file\n\
         #               complete. The reference track's promise.\n\
         #   narrative — annotated where the story goes, never asked to be exhaustive\n\
         #               (PLAN.md §11).\n\
         #   glossary  — never annotated at all; quoted verbatim by glossary entries\n\
         #               (D9). The pin is what keeps a quotation honest.\n",
    );
    for source in &pin.sources {
        let role = match source.role {
            Role::Coverage => "coverage",
            Role::Narrative => "narrative",
            Role::Glossary => "glossary",
        };
        out.push_str(&format!(
            "\n[[source]]\n\
             name = \"{}\"\n\
             version = \"{}\"\n\
             role = \"{role}\"\n\
             crate_sha256 = \"{}\"\n\
             src_tree_sha256 = \"{}\"\n",
            source.name, source.version, source.crate_sha256, source.src_tree_sha256
        ));
    }
    out
}

/// Every pinned tree still hashes to what the pin claims.
///
/// All roles are checked, not just `coverage`: a glossary quotation is a line
/// range in a pinned tree exactly as an annotation is, and drift breaks it in
/// the same silent way.
pub fn verify(repo: &Path) -> Result<()> {
    let pin = load_pin(repo)?;
    for source in &pin.sources {
        let root = source.dir(repo);
        if !root.is_dir() {
            bail!(
                "vendor/{} is missing — the pin lists it as a {:?} source",
                source.source_id(),
                source.role
            );
        }
        let actual = tree_hash_of(&root)?;
        if actual != source.src_tree_sha256 {
            bail!(
                "vendored source has drifted!\n  source   {}\n  expected src_tree_sha256 {}\n  \
                 actual   src_tree_sha256 {}\n\
                 \nLine ranges are keyed to the pinned tree. Either restore\n\
                 vendor/{}/ or run `cargo xtask bump <version>` to migrate them.",
                source.source_id(),
                source.src_tree_sha256,
                actual,
                source.source_id(),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIN: &str = r#"
[[source]]
name = "serde_core"
version = "1.0.229"
role = "coverage"
crate_sha256 = "aa"
src_tree_sha256 = "bb"

[[source]]
name = "syn"
version = "3.0.3"
role = "glossary"
crate_sha256 = "cc"
src_tree_sha256 = "dd"
"#;

    #[test]
    fn the_coverage_source_is_the_primary_one() {
        let pin: Pin = toml::from_str(PIN).unwrap();
        assert_eq!(pin.primary().unwrap().name, "serde_core");
        assert_eq!(pin.get("syn").unwrap().role, Role::Glossary);
        assert_eq!(pin.by_role(Role::Glossary).count(), 1);
    }

    #[test]
    fn source_ids_are_the_vendor_directory_names() {
        let pin: Pin = toml::from_str(PIN).unwrap();
        assert_eq!(pin.primary().unwrap().source_id(), "serde_core-1.0.229");
        // Hyphenated crate names survive: the id is name-version, not a slug.
        assert_eq!(source_id("proc-macro2", "1.0.107"), "proc-macro2-1.0.107");
    }

    #[test]
    fn two_coverage_sources_are_refused() {
        let doubled = PIN.replace("role = \"glossary\"", "role = \"coverage\"");
        let pin: Pin = toml::from_str(&doubled).unwrap();
        let err = pin.primary().unwrap_err().to_string();
        assert!(err.contains("two coverage sources"), "{err}");
    }

    /// A role the gates do not know is a parse failure, not a source that
    /// quietly gets checked by nothing.
    #[test]
    fn an_unknown_role_fails_to_parse() {
        let bad = PIN.replace("role = \"glossary\"", "role = \"decorative\"");
        let err = toml::from_str::<Pin>(&bad).unwrap_err().to_string();
        assert!(err.contains("decorative"), "{err}");
    }

    #[test]
    fn rendering_a_pin_round_trips() {
        let pin: Pin = toml::from_str(PIN).unwrap();
        let rendered = render_pin(&pin);
        let back: Pin = toml::from_str(&rendered).unwrap();
        assert_eq!(back.sources.len(), 2);
        assert_eq!(back.sources[1].name, "syn");
        assert_eq!(back.sources[1].role, Role::Glossary);
        assert_eq!(back.sources[0].src_tree_sha256, "bb");
    }
}
