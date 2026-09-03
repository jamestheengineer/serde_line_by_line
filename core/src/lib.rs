//! Shared model for serde_line_by_line.
//!
//! The annotation store is the project's expensive artifact, so its schema and
//! loading live here rather than inside any one consumer. Both the coverage
//! gate (`xtask`) and the site generator (`app`) read through this crate.

pub mod remap;
pub mod schema;
pub mod vendor;

use anyhow::{Context, Result};
use schema::{
    Annotation, AnnotationFile, CourseFile, CourseUnit, LineRange, Manifest, SCHEMA_VERSION,
};
use std::collections::BTreeMap;
use std::path::Path;

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
    /// The course track's units, in teaching order.
    pub course: Vec<CourseUnit>,
    /// Dependency order over the source files, from the course registry. Used
    /// to sequence annotations drawn from several files into one unit.
    pub reading_order: Vec<String>,
    /// The pinned crate version, e.g. "1.0.229". Read from `vendor/pin.toml`
    /// rather than compiled in, so a bump moves it in one place.
    pub version: String,
    /// `"serde_core-1.0.229"` — what every annotation file's `source` field
    /// must match.
    pub source_id: String,
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

    /// Every annotation tagged with `unit_id`, in teaching order.
    ///
    /// Teaching order is the prereq graph first and position second: an
    /// annotation never precedes one it lists as a `prereq` or as its
    /// `macro_def`, and everything otherwise unconstrained falls back to
    /// `reading_order` then line number. Sorting by position alone would open
    /// `03-associated-types` with `de/value.rs`, four hundred lines into the
    /// machinery, instead of with `type Ok`.
    pub fn course_annotations(&self, unit_id: &str) -> Vec<&Unit> {
        let mut pool: Vec<&Unit> = self
            .by_file
            .values()
            .flatten()
            .filter(|u| u.annotation.course_unit.as_deref() == Some(unit_id))
            .collect();
        pool.sort_by_key(|u| (self.file_rank(&u.annotation.file), u.range.start));

        // Kahn's algorithm, always taking the position-earliest ready node, so
        // the result is deterministic and as close to source order as the
        // dependencies allow. Edges pointing outside the unit are ignored: a
        // prereq in an earlier unit is already satisfied by the time the reader
        // arrives, and one in a later unit is a forward reference the coverage
        // gate reports rather than something to reorder around.
        let ids: BTreeMap<&str, usize> = pool
            .iter()
            .enumerate()
            .map(|(i, u)| (u.annotation.id.as_str(), i))
            .collect();
        let mut pending: Vec<Vec<usize>> = pool
            .iter()
            .map(|u| {
                u.annotation
                    .prereqs
                    .iter()
                    .chain(u.annotation.macro_def.iter())
                    .filter_map(|p| ids.get(p.as_str()).copied())
                    .collect()
            })
            .collect();

        let mut out = Vec::with_capacity(pool.len());
        let mut taken = vec![false; pool.len()];
        while out.len() < pool.len() {
            let next = (0..pool.len()).find(|&i| !taken[i] && pending[i].iter().all(|&d| taken[d]));
            // A cycle would strand every remaining node. The coverage gate
            // rejects cycles, so this is a fallback rather than a policy: emit
            // the rest in position order instead of looping forever.
            match next {
                Some(i) => {
                    taken[i] = true;
                    out.push(pool[i]);
                }
                None => {
                    for i in 0..pool.len() {
                        if !taken[i] {
                            taken[i] = true;
                            pending[i].clear();
                            out.push(pool[i]);
                        }
                    }
                }
            }
        }
        out
    }

    /// Position of a file in `reading_order`; unlisted files sort last, by path.
    fn file_rank(&self, file: &str) -> usize {
        self.reading_order
            .iter()
            .position(|f| f == file)
            .unwrap_or(usize::MAX)
    }

    pub fn course_unit(&self, id: &str) -> Option<&CourseUnit> {
        self.course.iter().find(|u| u.id == id)
    }
}

/// Loads every `annotations/*.toml`, validating schema and source id, and
/// resolves each record against the pinned tree.
///
/// This does *not* enforce coverage — that is the coverage gate's job. The site
/// generator must be able to render a partially annotated crate.
pub fn load(repo: &Path) -> Result<Store> {
    let pin = vendor::load_pin(repo)?;
    let source_id = pin.source_id();
    let mut store = Store {
        version: pin.version.clone(),
        source_id: source_id.clone(),
        ..Store::default()
    };

    for rel in vendor::source_files(repo)? {
        let n = vendor::line_count(repo, &rel)?;
        store.files.push((rel, n));
    }

    let manifest = Manifest::load(&repo.join("annotations").join("manifest.toml"))?;
    anyhow::ensure!(
        manifest.source == source_id,
        "manifest source {:?} does not match pinned source {source_id:?}",
        manifest.source
    );
    store.complete = manifest.complete;

    let course = CourseFile::load(&repo.join("annotations").join("course.toml"))?;
    anyhow::ensure!(
        course.source == source_id,
        "course registry source {:?} does not match pinned source {source_id:?}",
        course.source
    );
    store.reading_order = course.reading_order;
    store.course = course.units;

    for annotation in read_annotations(repo, &source_id)? {
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
///
/// `source_id` is passed in rather than read from the pin so that the bump
/// tool can load a store still keyed to the outgoing version.
pub fn read_annotations(repo: &Path, source_id: &str) -> Result<Vec<Annotation>> {
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
        // The manifest and the course registry live in the same directory but
        // are not annotation files.
        if path
            .file_name()
            .is_some_and(|n| n == "manifest.toml" || n == "course.toml")
        {
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
            parsed.source == source_id,
            "{}: source {:?} does not match pinned {source_id:?}",
            path.display(),
            parsed.source
        );
        out.extend(parsed.annotations);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use schema::{Kind, Track};

    fn annotation(id: &str, file: &str, line: u32, prereqs: &[&str]) -> Unit {
        Unit {
            range: LineRange::parse(&line.to_string()).unwrap(),
            annotation: Annotation {
                id: id.to_string(),
                file: file.to_string(),
                lines: line.to_string(),
                title: id.to_string(),
                kind: Kind::TraitItem,
                tracks: vec![Track::Course],
                course_unit: Some("01-unit".to_string()),
                rust_features: Vec::new(),
                examples: Vec::new(),
                prereqs: prereqs.iter().map(|s| s.to_string()).collect(),
                macro_def: None,
                body: "body".to_string(),
            },
        }
    }

    /// Teaching order is the prereq graph first and position second.
    #[test]
    fn course_order_respects_prereqs_then_reading_order() {
        let mut store = Store {
            reading_order: vec!["src/ser/mod.rs".into(), "src/de/impls.rs".into()],
            ..Store::default()
        };
        store.by_file.insert(
            "src/ser/mod.rs".into(),
            vec![
                annotation("c", "src/ser/mod.rs", 100, &["a"]),
                annotation("b", "src/ser/mod.rs", 500, &[]),
            ],
        );
        store.by_file.insert(
            "src/de/impls.rs".into(),
            vec![annotation("a", "src/de/impls.rs", 10, &[])],
        );

        let order: Vec<&str> = store
            .course_annotations("01-unit")
            .iter()
            .map(|u| u.annotation.id.as_str())
            .collect();

        // `b` first: earliest position with nothing to wait for. `c` sits
        // ahead of it in the source but cannot precede its own prereq.
        assert_eq!(order, ["b", "a", "c"]);
    }

    /// A file the registry forgot must not silently sort into the middle.
    #[test]
    fn unlisted_files_sort_last() {
        let mut store = Store {
            reading_order: vec!["src/ser/mod.rs".into()],
            ..Store::default()
        };
        store.by_file.insert(
            "src/de/mod.rs".into(),
            vec![annotation("x", "src/de/mod.rs", 1, &[])],
        );
        store.by_file.insert(
            "src/ser/mod.rs".into(),
            vec![annotation("y", "src/ser/mod.rs", 900, &[])],
        );

        let order: Vec<&str> = store
            .course_annotations("01-unit")
            .iter()
            .map(|u| u.annotation.id.as_str())
            .collect();
        assert_eq!(order, ["y", "x"]);
    }
}
