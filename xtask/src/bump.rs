//! `cargo xtask bump <version>` — moving the pinned source, on purpose.
//!
//! Every annotation in this repo is keyed to `(file, line-range)` in a pinned
//! tree. That is what makes "every line" checkable, and it is also what makes a
//! dependency update dangerous: swapping the source under the store leaves
//! hundreds of explanations pointing at whatever code happens to occupy those
//! lines now. Nothing would fail to compile. The site would render, confidently
//! wrong.
//!
//! PLAN.md's answer was that a bump must be "an explicit migration with a
//! remapping tool". This is that tool. It does the parts that can be done
//! mechanically — fetch, verify, diff, carry the ranges across, update every
//! file that names the version — and it is deliberately loud about the part
//! that cannot: prose describing code that changed underneath it.
//!
//! The order matters. Nothing in the repository is touched until the whole
//! migration has been planned against a staged copy of the new tree, so a
//! failure at any point before `apply` leaves a clean checkout.

use crate::store_edit::{self, Edit};
use anyhow::{bail, Context, Result};
use slbl_core::remap::{Change, FileMap};
use slbl_core::schema::{AnnotationFile, LineRange};
use slbl_core::vendor::{self, Pin};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Options {
    pub version: String,
    /// A local `.crate` archive to use instead of downloading one.
    pub archive: Option<PathBuf>,
    /// Expected sha256 of the archive. Fetched from the crates.io index when
    /// not given.
    pub sha256: Option<String>,
    pub dry_run: bool,
    /// Permit dropping annotations whose lines no longer exist.
    pub allow_orphans: bool,
    /// Leave the outgoing `vendor/serde_core-<old>/` directory in place.
    pub keep_old: bool,
}

pub fn parse_args(args: &[String]) -> Result<Options> {
    let mut opts = Options {
        version: String::new(),
        archive: None,
        sha256: None,
        dry_run: false,
        allow_orphans: false,
        keep_old: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dry-run" => opts.dry_run = true,
            "--allow-orphans" => opts.allow_orphans = true,
            "--keep-old" => opts.keep_old = true,
            "--archive" => {
                opts.archive = Some(PathBuf::from(it.next().context("--archive needs a path")?))
            }
            "--sha256" => {
                opts.sha256 = Some(it.next().context("--sha256 needs a hex digest")?.clone())
            }
            other if other.starts_with('-') => bail!("unknown flag {other:?}"),
            other if opts.version.is_empty() => opts.version = other.to_string(),
            other => bail!("unexpected argument {other:?}"),
        }
    }
    if opts.version.is_empty() {
        bail!("usage: cargo xtask bump <version> [--dry-run] [--archive PATH] [--sha256 HEX]");
    }
    Ok(opts)
}

pub fn run(repo: &Path, opts: &Options) -> Result<()> {
    let pin = vendor::load_pin(repo)?;
    let old_root = vendor::crate_dir(repo, &pin.version);
    if !old_root.join("src").is_dir() {
        bail!(
            "the pinned tree is missing from {}; restore it before migrating off it",
            old_root.display()
        );
    }
    if opts.version == pin.version {
        println!(
            "vendor/pin.toml already names {}. Nothing to migrate.",
            pin.version
        );
        return Ok(());
    }

    // 1. Get the archive and prove it is the published one. A migration that
    //    trusted an unverified download would be a supply-chain hole dressed
    //    up as tooling.
    let staging = repo.join("target").join("bump");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;

    let expected = match &opts.sha256 {
        Some(hex) => hex.trim().to_ascii_lowercase(),
        None => fetch_index_checksum(&opts.version).with_context(|| {
            format!(
                "looking up the sha256 of {} {} on the crates.io index (pass --sha256 to \
                 supply it directly when offline)",
                vendor::CRATE_NAME,
                opts.version
            )
        })?,
    };

    let archive = match &opts.archive {
        Some(path) => path.clone(),
        None => {
            let dest = staging.join(format!("{}-{}.crate", vendor::CRATE_NAME, opts.version));
            download(&opts.version, &dest)?;
            dest
        }
    };
    let actual = sha256_file(&archive)?;
    if actual != expected {
        bail!(
            "checksum mismatch for {}\n  expected {expected}\n  actual   {actual}",
            archive.display()
        );
    }
    println!("verified {} ({actual})", archive.display());

    // 2. Unpack beside the repo, not into it.
    let unpacked = staging.join("unpacked");
    std::fs::create_dir_all(&unpacked)?;
    extract(&archive, &unpacked)?;
    let new_root = unpacked.join(vendor::source_id(&opts.version));
    if !new_root.join("src").is_dir() {
        bail!(
            "{} does not contain the expected {}/src",
            archive.display(),
            vendor::source_id(&opts.version)
        );
    }

    // 3. Plan the whole migration before touching anything.
    let plan = Plan::build(repo, &pin, &old_root, &new_root, &opts.version)?;
    plan.print();

    if opts.dry_run {
        println!("\n--dry-run: nothing was written.");
        return Ok(());
    }
    if !plan.orphans.is_empty() && !opts.allow_orphans {
        bail!(
            "{} annotation(s) claim lines that no longer exist. Re-cut them by hand against \
             the new source, or re-run with --allow-orphans to drop them — their full text is \
             written to the migration report either way.",
            plan.orphans.len()
        );
    }

    plan.apply(repo, &pin, &new_root, &archive, &actual, opts)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The plan
// ---------------------------------------------------------------------------

/// One annotation that survived the move but no longer describes the same code.
struct Review {
    id: String,
    file: String,
    from: String,
    to: String,
    deleted: u32,
    inserted: u32,
}

/// One annotation with nothing left to point at.
struct Orphan {
    id: String,
    file: String,
    from: String,
    why: &'static str,
}

struct FileOutcome {
    file: String,
    old_lines: u32,
    new_lines: u32,
    shifted: usize,
    edited: usize,
    dropped: usize,
    unclaimed: u32,
}

struct Plan {
    old_version: String,
    new_version: String,
    /// Per annotation file, the edit for every record it holds.
    edits: BTreeMap<PathBuf, BTreeMap<String, Edit>>,
    files: Vec<FileOutcome>,
    added: Vec<(String, u32)>,
    removed: Vec<String>,
    reviews: Vec<Review>,
    orphans: Vec<Orphan>,
    reading_order: Vec<String>,
    complete: Vec<String>,
    /// Files still declared complete that the migration leaves with holes.
    now_incomplete: Vec<String>,
}

impl Plan {
    fn build(
        repo: &Path,
        pin: &Pin,
        old_root: &Path,
        new_root: &Path,
        new_version: &str,
    ) -> Result<Plan> {
        let old_files = vendor::source_files_in(old_root)?;
        let new_files = vendor::source_files_in(new_root)?;
        let old_set: BTreeSet<&str> = old_files.iter().map(String::as_str).collect();
        let new_set: BTreeSet<&str> = new_files.iter().map(String::as_str).collect();

        // One alignment per file, computed once and shared by every annotation
        // that lands in it.
        let mut maps: BTreeMap<String, FileMap> = BTreeMap::new();
        for rel in &old_files {
            if !new_set.contains(rel.as_str()) {
                continue;
            }
            let old_text = std::fs::read_to_string(old_root.join(rel))?;
            let new_text = std::fs::read_to_string(new_root.join(rel))?;
            let map =
                FileMap::build(&old_text, &new_text).with_context(|| format!("aligning {rel}"))?;
            maps.insert(rel.clone(), map);
        }

        let mut plan = Plan {
            old_version: pin.version.clone(),
            new_version: new_version.to_string(),
            edits: BTreeMap::new(),
            files: Vec::new(),
            added: Vec::new(),
            removed: old_files
                .iter()
                .filter(|f| !new_set.contains(f.as_str()))
                .cloned()
                .collect(),
            reviews: Vec::new(),
            orphans: Vec::new(),
            reading_order: Vec::new(),
            complete: Vec::new(),
            now_incomplete: Vec::new(),
        };
        for rel in &new_files {
            if !old_set.contains(rel.as_str()) {
                let n = std::fs::read_to_string(new_root.join(rel))?.lines().count();
                plan.added.push((rel.clone(), n as u32));
            }
        }

        // Walk the store file by file so each rewrite gets a complete edit set.
        let mut claimed: BTreeMap<String, Vec<LineRange>> = BTreeMap::new();
        let mut counts: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();

        for (path, parsed) in read_store(repo)? {
            let mut edits: BTreeMap<String, Edit> = BTreeMap::new();
            for a in &parsed.annotations {
                let range =
                    LineRange::parse(&a.lines).with_context(|| format!("annotation {}", a.id))?;
                let entry = counts.entry(a.file.clone()).or_default();

                let Some(map) = maps.get(&a.file) else {
                    entry.2 += 1;
                    plan.orphans.push(Orphan {
                        id: a.id.clone(),
                        file: a.file.clone(),
                        from: a.lines.clone(),
                        why: "the file is gone from the new release",
                    });
                    edits.insert(a.id.clone(), Edit::Drop);
                    continue;
                };

                let out = map
                    .remap(range)
                    .with_context(|| format!("annotation {} in {}", a.id, a.file))?;
                match (out.range, out.change) {
                    (None, _) | (_, Change::Deleted) => {
                        entry.2 += 1;
                        plan.orphans.push(Orphan {
                            id: a.id.clone(),
                            file: a.file.clone(),
                            from: a.lines.clone(),
                            why: "every line it claimed was deleted",
                        });
                        edits.insert(a.id.clone(), Edit::Drop);
                    }
                    (Some(new_range), change) => {
                        let text = fmt_range(new_range);
                        match change {
                            Change::Edited { deleted, inserted } => {
                                entry.1 += 1;
                                plan.reviews.push(Review {
                                    id: a.id.clone(),
                                    file: a.file.clone(),
                                    from: a.lines.clone(),
                                    to: text.clone(),
                                    deleted,
                                    inserted,
                                });
                            }
                            Change::Shifted => entry.0 += 1,
                            Change::Unmoved | Change::Deleted => {}
                        }
                        claimed.entry(a.file.clone()).or_default().push(new_range);
                        edits.insert(a.id.clone(), Edit::Retarget(text));
                    }
                }
            }
            plan.edits.insert(path, edits);
        }

        // What the new tree will look like to the coverage gate.
        for rel in &new_files {
            let new_lines = std::fs::read_to_string(new_root.join(rel))?.lines().count() as u32;
            let old_lines = match maps.get(rel) {
                Some(m) => m.old_len() as u32,
                None => 0,
            };
            let (shifted, edited, dropped) = counts.get(rel).copied().unwrap_or_default();
            let mut ranges = claimed.remove(rel).unwrap_or_default();
            ranges.sort_by_key(|r| (r.start, r.end));
            let mut unclaimed = 0u32;
            let mut cursor = 1u32;
            for r in &ranges {
                if r.start > cursor {
                    unclaimed += r.start - cursor;
                }
                cursor = cursor.max(r.end + 1);
            }
            unclaimed += new_lines.saturating_sub(cursor - 1);

            plan.files.push(FileOutcome {
                file: rel.clone(),
                old_lines,
                new_lines,
                shifted,
                edited,
                dropped,
                unclaimed,
            });
        }

        // The registries: drop what is gone, append what is new. Position in
        // `reading_order` is a teaching decision, so a new file goes last and
        // is reported rather than guessed at.
        let manifest = slbl_core::schema::Manifest::load(&manifest_path(repo))?;
        plan.complete = manifest
            .complete
            .into_iter()
            .filter(|f| new_set.contains(f.as_str()))
            .collect();
        for f in &plan.complete {
            if plan.files.iter().any(|o| &o.file == f && o.unclaimed > 0) {
                plan.now_incomplete.push(f.clone());
            }
        }
        let course = slbl_core::schema::CourseFile::load(&course_path(repo))?;
        plan.reading_order = course
            .reading_order
            .into_iter()
            .filter(|f| new_set.contains(f.as_str()))
            .collect();
        for (rel, _) in &plan.added {
            plan.reading_order.push(rel.clone());
        }

        Ok(plan)
    }

    fn print(&self) {
        println!(
            "\n{} -> {}\n",
            vendor::source_id(&self.old_version),
            vendor::source_id(&self.new_version)
        );
        println!(
            "{:<26}{:>11}{:>9}{:>8}{:>8}{:>11}",
            "file", "lines", "shifted", "edited", "dropped", "unclaimed"
        );
        for f in &self.files {
            let moved = f.shifted + f.edited + f.dropped;
            let lines = if f.old_lines == f.new_lines {
                format!("{}", f.new_lines)
            } else {
                format!("{} -> {}", f.old_lines, f.new_lines)
            };
            if moved == 0 && f.unclaimed == 0 && f.old_lines == f.new_lines {
                continue;
            }
            println!(
                "{:<26}{:>11}{:>9}{:>8}{:>8}{:>11}",
                f.file, lines, f.shifted, f.edited, f.dropped, f.unclaimed
            );
        }
        let untouched = self
            .files
            .iter()
            .filter(|f| f.shifted + f.edited + f.dropped == 0 && f.unclaimed == 0)
            .count();
        println!("\n{untouched} file(s) unchanged and left alone");

        for (rel, n) in &self.added {
            println!("  new file: {rel} ({n} lines, none annotated)");
        }
        for rel in &self.removed {
            println!("  removed:  {rel}");
        }

        if !self.reviews.is_empty() {
            println!(
                "\n{} annotation(s) still point at real code, but the code changed:",
                self.reviews.len()
            );
            for r in self.reviews.iter().take(40) {
                println!(
                    "  {:<20} {} {} -> {}  (-{} +{})",
                    r.id, r.file, r.from, r.to, r.deleted, r.inserted
                );
            }
            if self.reviews.len() > 40 {
                println!("  ... and {} more", self.reviews.len() - 40);
            }
        }
        if !self.orphans.is_empty() {
            println!(
                "\n{} annotation(s) have nothing left to point at:",
                self.orphans.len()
            );
            for o in &self.orphans {
                println!("  {:<20} {} {}  — {}", o.id, o.file, o.from, o.why);
            }
        }
        if !self.now_incomplete.is_empty() {
            println!(
                "\n{} file(s) are marked complete but will have unclaimed lines; \
                 coverage will fail until they are annotated:",
                self.now_incomplete.len()
            );
            for f in &self.now_incomplete {
                println!("  {f}");
            }
        }
    }

    fn apply(
        &self,
        repo: &Path,
        pin: &Pin,
        new_root: &Path,
        archive: &Path,
        archive_sha: &str,
        opts: &Options,
    ) -> Result<()> {
        let new_source = vendor::source_id(&self.new_version);
        let mut dropped_text: Vec<(String, String)> = Vec::new();

        // 1. The annotation store.
        for (path, edits) in &self.edits {
            let text = std::fs::read_to_string(path)?;
            let (out, outcome) = store_edit::rewrite(&text, &new_source, edits)
                .with_context(|| format!("rewriting {}", path.display()))?;
            dropped_text.extend(outcome.dropped_text);
            if outcome.retargeted == 0 && outcome.dropped > 0 {
                // Every record went away with the file it described.
                std::fs::remove_file(path)?;
                println!("removed {}", rel_display(repo, path));
            } else {
                std::fs::write(path, out)?;
            }
        }

        // 2. The registries.
        let manifest = manifest_path(repo);
        let text = std::fs::read_to_string(&manifest)?;
        let text = store_edit::set_scalar(&text, "source", &new_source)?;
        let text = store_edit::set_string_array(&text, "complete", &self.complete)?;
        std::fs::write(&manifest, text)?;

        let course = course_path(repo);
        let text = std::fs::read_to_string(&course)?;
        let text = store_edit::set_scalar(&text, "source", &new_source)?;
        let text = store_edit::set_string_array(&text, "reading_order", &self.reading_order)?;
        std::fs::write(&course, text)?;

        // 3. The vendored tree. Moved into place only now, so an earlier
        //    failure leaves the old tree and the old store consistent.
        let dest = vendor::crate_dir(repo, &self.new_version);
        let _ = std::fs::remove_dir_all(&dest);
        move_dir(new_root, &dest)?;
        if !opts.keep_old {
            std::fs::remove_dir_all(vendor::crate_dir(repo, &self.old_version))?;
        }

        // 4. The pin, which is what the coverage gate actually checks.
        let new_pin = Pin {
            version: self.new_version.clone(),
            crate_sha256: archive_sha.to_string(),
            src_tree_sha256: vendor::tree_hash_of(&dest)?,
        };
        std::fs::write(vendor::pin_path(repo), vendor::render_pin(&new_pin))?;
        std::fs::write(repo.join("vendor").join("NOTICE.md"), notice(&new_pin))?;

        // 5. Everything else that names the version.
        let bumped = bump_example_manifests(repo, &self.old_version, &self.new_version)?;

        let report = self.report(pin, archive, archive_sha, &dropped_text)?;
        let report_path = repo
            .join("docs")
            .join("migrations")
            .join(format!("{}-to-{}.md", self.old_version, self.new_version));
        std::fs::create_dir_all(report_path.parent().unwrap())?;
        std::fs::write(&report_path, report)?;

        println!("\nwrote {}", rel_display(repo, &vendor::pin_path(repo)));
        println!("wrote {}", rel_display(repo, &report_path));
        println!("updated {bumped} example manifest(s)");

        let stale = stale_mentions(repo, &self.old_version)?;
        if !stale.is_empty() {
            println!("\nstill mentioning {} by hand:", self.old_version);
            for (path, line, text) in stale.iter().take(30) {
                println!("  {path}:{line}: {}", text.trim());
            }
            if stale.len() > 30 {
                println!("  ... and {} more", stale.len() - 30);
            }
        }

        println!(
            "\nnext:\n  cargo update -p {} --precise {}\n  cargo xtask coverage\n  cargo test --workspace\n  review {}",
            vendor::CRATE_NAME,
            self.new_version,
            rel_display(repo, &report_path)
        );
        Ok(())
    }

    fn report(
        &self,
        pin: &Pin,
        archive: &Path,
        archive_sha: &str,
        dropped_text: &[(String, String)],
    ) -> Result<String> {
        let mut s = String::new();
        s.push_str(&format!(
            "# Migration: {} {} -> {}\n\n\
             Generated by `cargo xtask bump {}`. This file is the record of what the\n\
             migration did mechanically and what it left for a human.\n\n",
            vendor::CRATE_NAME,
            self.old_version,
            self.new_version,
            self.new_version
        ));
        s.push_str("## Provenance\n\n");
        s.push_str(&format!(
            "| | |\n|---|---|\n| archive | `{}` |\n",
            file_name(archive)
        ));
        s.push_str(&format!("| crate sha256 | `{archive_sha}` |\n"));
        s.push_str(&format!(
            "| previous crate sha256 | `{}` |\n",
            pin.crate_sha256
        ));
        s.push_str(&format!(
            "| previous src_tree_sha256 | `{}` |\n\n",
            pin.src_tree_sha256
        ));

        s.push_str("## Files\n\n");
        s.push_str("| file | lines | shifted | edited | dropped | unclaimed |\n");
        s.push_str("|---|---:|---:|---:|---:|---:|\n");
        for f in &self.files {
            if f.shifted + f.edited + f.dropped == 0
                && f.unclaimed == 0
                && f.old_lines == f.new_lines
            {
                continue;
            }
            let lines = if f.old_lines == f.new_lines {
                format!("{}", f.new_lines)
            } else {
                format!("{} → {}", f.old_lines, f.new_lines)
            };
            s.push_str(&format!(
                "| `{}` | {} | {} | {} | {} | {} |\n",
                f.file, lines, f.shifted, f.edited, f.dropped, f.unclaimed
            ));
        }
        s.push('\n');
        for (rel, n) in &self.added {
            s.push_str(&format!(
                "- **New file** `{rel}` ({n} lines). Appended to `reading_order` at the end; \
                 move it to where it belongs in the teaching order.\n"
            ));
        }
        for rel in &self.removed {
            s.push_str(&format!("- **Removed file** `{rel}`.\n"));
        }
        if !self.added.is_empty() || !self.removed.is_empty() {
            s.push('\n');
        }

        s.push_str("## Annotations to review\n\n");
        if self.reviews.is_empty() {
            s.push_str("None: every surviving annotation covers the same text it did before.\n\n");
        } else {
            s.push_str(
                "These ranges were carried across and still cover real code, but the code inside \
                 them changed. The prose has not been checked against it.\n\n\
                 | id | file | was | now | -lines | +lines |\n|---|---|---|---|---:|---:|\n",
            );
            for r in &self.reviews {
                s.push_str(&format!(
                    "| `{}` | `{}` | {} | {} | {} | {} |\n",
                    r.id, r.file, r.from, r.to, r.deleted, r.inserted
                ));
            }
            s.push('\n');
        }

        s.push_str("## Annotations dropped\n\n");
        if dropped_text.is_empty() {
            s.push_str("None.\n\n");
        } else {
            s.push_str(
                "Every line these claimed is gone from the new release, so there was no range \
                 left to give them. Their text is reproduced here in full — nothing about the \
                 removal is recoverable from the new tree.\n\n",
            );
            for o in &self.orphans {
                s.push_str(&format!(
                    "- `{}` — {} ({}), {}\n",
                    o.id, o.file, o.from, o.why
                ));
            }
            s.push('\n');
            for (id, text) in dropped_text {
                s.push_str(&format!(
                    "<details>\n<summary><code>{id}</code></summary>\n\n```toml\n"
                ));
                s.push_str(text);
                s.push_str("\n```\n\n</details>\n\n");
            }
        }

        s.push_str("## Checklist\n\n");
        s.push_str(&format!(
            "- [ ] `cargo update -p {} --precise {}`\n\
             - [ ] `cargo xtask coverage` is green\n\
             - [ ] `cargo test --workspace` is green (examples still produce their transcripts)\n\
             - [ ] every annotation in *Annotations to review* has been read against the new source\n",
            vendor::CRATE_NAME, self.new_version
        ));
        if !self.now_incomplete.is_empty() {
            for f in &self.now_incomplete {
                s.push_str(&format!(
                    "- [ ] `{f}` is marked complete but has unclaimed lines: annotate them\n"
                ));
            }
        }
        for (rel, _) in &self.added {
            s.push_str(&format!(
                "- [ ] `{rel}` is new and entirely unannotated; add it to `annotations/` and to \
                 `manifest.toml` once it is done\n"
            ));
        }
        s.push_str("- [ ] prose outside the store that names the version (PLAN.md, README.md, docs/) is current\n");
        Ok(s)
    }
}

// ---------------------------------------------------------------------------
// Fetching
// ---------------------------------------------------------------------------

/// The crates.io sparse index path for a crate name: `se/rd/serde_core`.
fn index_path(name: &str) -> String {
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => format!("3/{}/{name}", &name[..1]),
        _ => format!("{}/{}/{name}", &name[..2], &name[2..4]),
    }
}

/// The published sha256 for one version, from the index.
///
/// The index is the authority crates.io itself uses, and it is a flat file of
/// one JSON object per version — cheap enough to fetch on every bump.
fn fetch_index_checksum(version: &str) -> Result<String> {
    let url = format!("https://index.crates.io/{}", index_path(vendor::CRATE_NAME));
    let body = curl(&url)?;
    let mut known = Vec::new();
    for line in body.lines().filter(|l| !l.trim().is_empty()) {
        let entry: serde_json::Value = serde_json::from_str(line)?;
        let vers = entry["vers"].as_str().unwrap_or_default();
        known.push(vers.to_string());
        if vers == version {
            if entry["yanked"].as_bool() == Some(true) {
                bail!("{} {version} is yanked", vendor::CRATE_NAME);
            }
            return entry["cksum"]
                .as_str()
                .map(|s| s.to_ascii_lowercase())
                .context("index entry has no cksum");
        }
    }
    bail!(
        "{} has no version {version}. Published: {}",
        vendor::CRATE_NAME,
        known.join(", ")
    )
}

fn download(version: &str, dest: &Path) -> Result<()> {
    let url = format!(
        "https://static.crates.io/crates/{name}/{name}-{version}.crate",
        name = vendor::CRATE_NAME
    );
    println!("fetching {url}");
    let status = Command::new("curl")
        .args(["-sSfL", "--max-time", "120", "-o"])
        .arg(dest)
        .arg(&url)
        .status()
        .context("running curl (needed to fetch the crate; use --archive when offline)")?;
    if !status.success() {
        bail!("curl failed for {url}");
    }
    Ok(())
}

fn curl(url: &str) -> Result<String> {
    let out = Command::new("curl")
        .args(["-sSfL", "--max-time", "60", url])
        .output()
        .context("running curl")?;
    if !out.status.success() {
        bail!(
            "curl failed for {url}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn extract(archive: &Path, into: &Path) -> Result<()> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .status()
        .context("running tar")?;
    if !status.success() {
        bail!("tar failed to unpack {}", archive.display());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

// ---------------------------------------------------------------------------
// Odds and ends
// ---------------------------------------------------------------------------

fn manifest_path(repo: &Path) -> PathBuf {
    repo.join("annotations").join("manifest.toml")
}

fn course_path(repo: &Path) -> PathBuf {
    repo.join("annotations").join("course.toml")
}

/// Every annotation file, paired with its path, in a stable order.
fn read_store(repo: &Path) -> Result<Vec<(PathBuf, AnnotationFile)>> {
    let dir = repo.join("annotations");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "toml")
                && !p
                    .file_name()
                    .is_some_and(|n| n == "manifest.toml" || n == "course.toml")
        })
        .collect();
    entries.sort();

    let mut out = Vec::new();
    for path in entries {
        let text = std::fs::read_to_string(&path)?;
        let parsed: AnnotationFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        out.push((path, parsed));
    }
    Ok(out)
}

fn fmt_range(r: LineRange) -> String {
    if r.start == r.end {
        r.start.to_string()
    } else {
        format!("{}-{}", r.start, r.end)
    }
}

fn notice(pin: &Pin) -> String {
    let id = pin.source_id();
    format!(
        "# Vendored third-party source\n\n\
         ## {name} {version}\n\n\
         - Upstream: https://github.com/serde-rs/serde\n\
         - crates.io: https://crates.io/crates/{name}/{version}\n\
         - sha256: `{sha}`\n\
         - License: MIT OR Apache-2.0 (see `{id}/LICENSE-MIT`\n  and `{id}/LICENSE-APACHE`)\n\
         - Copyright: Erick Tryzelaar and David Tolnay\n\n\
         This directory contains an **unmodified** copy of the published crate, vendored\n\
         so that annotation line-ranges remain stable. It is checksum-verified in CI.\n\n\
         Do not edit anything under `{id}/`. All project-authored content\n\
         lives in `annotations/`, `examples/`, `app/`, and `xtask/`.\n",
        name = vendor::CRATE_NAME,
        version = pin.version,
        sha = pin.crate_sha256,
    )
}

/// Retargets the `serde_core = "=x.y.z"` pin in every example crate.
///
/// The examples build against the published crate, not the vendored copy, and
/// an example demonstrating behaviour from a different release than the one the
/// annotations describe is exactly the drift this repo exists to prevent.
fn bump_example_manifests(repo: &Path, old: &str, new: &str) -> Result<usize> {
    let from = format!("{} = \"={old}\"", vendor::CRATE_NAME);
    let to = format!("{} = \"={new}\"", vendor::CRATE_NAME);
    let mut n = 0;
    for entry in std::fs::read_dir(repo.join("examples"))? {
        let path = entry?.path().join("Cargo.toml");
        if !path.is_file() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        if text.contains(&from) {
            std::fs::write(&path, text.replace(&from, &to))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Where the outgoing version is still written out by hand.
///
/// Prose is not something a tool should rewrite — PLAN.md's measurements and
/// README's description mean things — but leaving it stale is how a repo starts
/// lying about itself, so the bump ends by pointing at every line.
fn stale_mentions(repo: &Path, old: &str) -> Result<Vec<(String, usize, String)>> {
    let mut out = Vec::new();
    let mut stack = vec![repo.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                // `migrations/` is the archive of past bumps: naming the
                // version it moved off is the whole point of those files.
                if matches!(
                    name.as_str(),
                    ".git" | "target" | "site" | "vendor" | "migrations"
                ) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !matches!(
                path.extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .as_deref(),
                Some("md" | "rs" | "toml" | "html" | "yml" | "yaml")
            ) || name == "Cargo.lock"
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if line.contains(old) {
                    out.push((rel_display(repo, &path), i + 1, line.to_string()));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

fn rel_display(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// `rename` across the `target/` boundary can cross a filesystem, so fall back
/// to a copy.
fn move_dir(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_dir(from, to)?;
    std::fs::remove_dir_all(from)?;
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_paths_follow_the_cargo_layout() {
        assert_eq!(index_path("a"), "1/a");
        assert_eq!(index_path("ab"), "2/ab");
        assert_eq!(index_path("abc"), "3/a/abc");
        assert_eq!(index_path("serde_core"), "se/rd/serde_core");
    }

    #[test]
    fn a_single_line_range_stays_single() {
        assert_eq!(fmt_range(LineRange { start: 7, end: 7 }), "7");
        assert_eq!(fmt_range(LineRange { start: 7, end: 9 }), "7-9");
    }
}
