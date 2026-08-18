//! Shared model for serde_line_by_line.
//!
//! The annotation store is the project's expensive artifact, so its schema and
//! loading live here rather than inside any one consumer. Both the coverage
//! gate (`xtask`) and the site generator (`app`) read through this crate.

pub mod schema;
pub mod vendor;

use anyhow::{Context, Result};
use schema::{Annotation, AnnotationFile, LineRange, Manifest, SCHEMA_VERSION};
use std::collections::BTreeMap;
use std::path::Path;
use vendor::SOURCE_ID;

/// One annotation resolved against the pinned source: its parsed line range and
/// the source text it claims.
#[derive(Debug, Clone)]
pub struct Unit {
    pub annotation: Annotation,
    pub range: LineRange,
}

/// Everything the site generator needs, already grouped and ordered.
#[derive(Debug, Default)]
pub struct Store {
    /// Annotations by source file, each sorted by starting line.
    pub by_file: BTreeMap<String, Vec<Unit>>,
    /// Every source file in the pinned tree, in path order, with its line count.
    pub files: Vec<(String, u32)>,
    pub complete: Vec<String>,
}

impl Store {
    pub fn total_lines(&self) -> u32 {
        self.files.iter().map(|(_, n)| n).sum()
    }

    pub fn claimed_lines(&self) -> u32 {
        self.by_file
            .values()
            .flat_map(|units| units.iter())
            .map(|u| u.range.line_count())
            .sum()
    }

    pub fn percent(&self) -> f64 {
        let total = self.total_lines();
        if total == 0 {
            return 0.0;
        }
        self.claimed_lines() as f64 * 100.0 / total as f64
    }

    pub fn units_for(&self, file: &str) -> &[Unit] {
        self.by_file.get(file).map_or(&[], Vec::as_slice)
    }
}

/// Loads every `annotations/*.toml`, validating schema and source id, and
/// resolves each record against the pinned tree.
///
/// This does *not* enforce coverage — that is the coverage gate's job. The site
/// generator must be able to render a partially annotated crate.
pub fn load(repo: &Path) -> Result<Store> {
    let mut store = Store::default();

    for rel in vendor::source_files(repo)? {
        let n = vendor::line_count(repo, &rel)?;
        store.files.push((rel, n));
    }

    let manifest = Manifest::load(&repo.join("annotations").join("manifest.toml"))?;
    anyhow::ensure!(
        manifest.source == SOURCE_ID,
        "manifest source {:?} does not match pinned source {SOURCE_ID:?}",
        manifest.source
    );
    store.complete = manifest.complete;

    for annotation in read_annotations(repo)? {
        let range = LineRange::parse(&annotation.lines)
            .with_context(|| format!("annotation {}", annotation.id))?;
        store
            .by_file
            .entry(annotation.file.clone())
            .or_default()
            .push(Unit { annotation, range });
    }

    for units in store.by_file.values_mut() {
        units.sort_by_key(|u| (u.range.start, u.range.end));
    }

    Ok(store)
}

/// Reads and validates every annotation file, without resolving line ranges.
pub fn read_annotations(repo: &Path) -> Result<Vec<Annotation>> {
    let dir = repo.join("annotations");
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.path());

    let mut out = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        if path.file_name().is_some_and(|n| n == "manifest.toml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let parsed: AnnotationFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        anyhow::ensure!(
            parsed.schema == SCHEMA_VERSION,
            "{}: schema {} but this build understands {SCHEMA_VERSION}",
            path.display(),
            parsed.schema
        );
        anyhow::ensure!(
            parsed.source == SOURCE_ID,
            "{}: source {:?} does not match pinned {SOURCE_ID:?}",
            path.display(),
            parsed.source
        );
        out.extend(parsed.annotations);
    }
    Ok(out)
}
