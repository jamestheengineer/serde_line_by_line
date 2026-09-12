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

/// One annotation resolved against the pinned source: its parsed line range,
/// and which annotated crate it claims a piece of.
#[derive(Debug, Clone)]
pub struct Unit {
    pub annotation: Annotation,
    pub range: LineRange,
    /// The crate name — "serde_core" or "serde_derive". Two annotated sources
    /// both have a `src/lib.rs`, so a file path alone stopped identifying
    /// anything when the second one arrived (D12).
    pub source: String,
}

/// One annotated source and everything the store claims about it.
///
/// There is one of these per `role = "coverage"` row in `vendor/pin.toml`, and
/// every coverage figure lives here rather than on [`Store`]: two crates make
/// two promises, and a number spanning both would describe neither (D12).
#[derive(Debug, Clone, Default)]
pub struct SourceStore {
    /// The crates.io name: "serde_core".
    pub name: String,
    pub version: String,
    /// `"serde_core-1.0.229"` — what this source's annotation files must name.
    pub source_id: String,
    /// Annotations by source file, each sorted by starting line.
    pub by_file: BTreeMap<String, Vec<Unit>>,
    /// Every source file in the pinned tree, in path order, with its line count.
    pub files: Vec<(String, u32)>,
    pub complete: Vec<String>,
}

impl SourceStore {
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

    pub fn annotations(&self) -> usize {
        self.by_file.values().map(Vec::len).sum()
    }

    pub fn units_for(&self, file: &str) -> &[Unit] {
        self.by_file.get(file).map_or(&[], Vec::as_slice)
    }

    /// Files with at least one annotation, which is what the renderer will
    /// give a page to. A file nobody has written about yet is listed with its
    /// zero rather than published as a page of unexplained source.
    pub fn annotated_files(&self) -> impl Iterator<Item = &(String, u32)> {
        self.files
            .iter()
            .filter(|(f, _)| self.by_file.contains_key(f))
    }
}

/// Everything the site generator needs, already grouped and ordered.
#[derive(Debug, Default)]
pub struct Store {
    /// The annotated sources, in pin order — which is reading order, because
    /// `serde_derive` generates calls into `serde_core`.
    pub sources: Vec<SourceStore>,
    /// The course track's units, in teaching order. One track over all of the
    /// sources, not one per source: a reader learning what `quote!` emits needs
    /// `Serializer`'s contract behind them, and that is in the other crate.
    pub course: Vec<CourseUnit>,
    /// Dependency order over the source files, from the course registry. Used
    /// to sequence annotations drawn from several files into one unit. Entries
    /// are `src/ser/mod.rs` for the first coverage source, or
    /// `serde_derive:src/ser.rs` to name another one.
    pub reading_order: Vec<String>,
}

impl Store {
    pub fn source(&self, name: &str) -> Option<&SourceStore> {
        self.sources.iter().find(|s| s.name == name)
    }

    /// The first coverage source in pin order. Used where a caller genuinely
    /// means "the crate this project started with" — the examples' pin, the
    /// front page's lead figure — and never as a stand-in for "the" annotated
    /// crate, of which there is no longer one.
    pub fn first(&self) -> &SourceStore {
        self.sources.first().expect("at least one coverage source")
    }

    pub fn total_lines(&self) -> u32 {
        self.sources.iter().map(SourceStore::total_lines).sum()
    }

    pub fn claimed_lines(&self) -> u32 {
        self.sources.iter().map(SourceStore::claimed_lines).sum()
    }

    pub fn annotations(&self) -> usize {
        self.sources.iter().map(SourceStore::annotations).sum()
    }

    /// Every annotation, across every source, in no particular order.
    pub fn all_units(&self) -> impl Iterator<Item = &Unit> {
        self.sources
            .iter()
            .flat_map(|s| s.by_file.values())
            .flatten()
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
            .all_units()
            .filter(|u| u.annotation.course_unit.as_deref() == Some(unit_id))
            .collect();
        pool.sort_by_key(|u| (self.file_rank(&u.source, &u.annotation.file), u.range.start));

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
    ///
    /// A file may be listed bare (`src/ser/mod.rs`, meaning the first coverage
    /// source) or qualified (`serde_derive:src/ser.rs`). Bare entries kept the
    /// registry unchanged when the second source arrived, and qualifying is
    /// what stops `src/lib.rs` — which both crates have — from matching twice.
    fn file_rank(&self, source: &str, file: &str) -> usize {
        let qualified = format!("{source}:{file}");
        let bare = source == self.first().name;
        self.reading_order
            .iter()
            .position(|f| f == &qualified || (bare && f == file))
            .unwrap_or(usize::MAX)
    }

    pub fn course_unit(&self, id: &str) -> Option<&CourseUnit> {
        self.course.iter().find(|u| u.id == id)
    }
}

/// Loads every annotated source's store, validating schema and source id, and
/// resolves each record against the pinned tree it claims.
///
/// This does *not* enforce coverage — that is the coverage gate's job. The site
/// generator must be able to render a partially annotated crate, which since
/// D12 is the normal state of the newer of the two.
pub fn load(repo: &Path) -> Result<Store> {
    let pin = vendor::load_pin(repo)?;
    let mut store = Store::default();

    for source in pin.coverage()? {
        let source_id = source.source_id();
        let root = source.dir(repo);
        let mut s = SourceStore {
            name: source.name.clone(),
            version: source.version.clone(),
            source_id: source_id.clone(),
            ..SourceStore::default()
        };

        for rel in vendor::source_files_in(&root)? {
            let n = vendor::line_count_in(&root, &rel)?;
            s.files.push((rel, n));
        }

        let manifest = Manifest::load(&manifest_path(repo, &source.name))?;
        anyhow::ensure!(
            manifest.source == source_id,
            "manifest source {:?} does not match pinned source {source_id:?}",
            manifest.source
        );
        s.complete = manifest.complete;

        for annotation in read_annotations(repo, &source.name, &source_id)? {
            let range = LineRange::parse(&annotation.lines)
                .with_context(|| format!("annotation {}", annotation.id))?;
            s.by_file
                .entry(annotation.file.clone())
                .or_default()
                .push(Unit {
                    annotation,
                    range,
                    source: source.name.clone(),
                });
        }
        for units in s.by_file.values_mut() {
            units.sort_by_key(|u| (u.range.start, u.range.end));
        }
        store.sources.push(s);
    }

    // One registry for the whole course track, so it lives beside the stores
    // rather than inside one of them. It names the source it was written
    // against, which is the first one — the track predates the second.
    let course = CourseFile::load(&repo.join("annotations").join("course.toml"))?;
    let first = store.first().source_id.clone();
    anyhow::ensure!(
        course.source == first,
        "course registry source {:?} does not match pinned source {first:?}",
        course.source
    );
    store.reading_order = course.reading_order;
    store.course = course.units;

    Ok(store)
}

/// Where one annotated source's store lives: `annotations/<crate>/`.
///
/// The stores were one flat directory while there was one annotated crate. Two
/// of them share file names — both crates have a `src/lib.rs` and a `ser`
/// module — so the split is what keeps an annotation file's name meaningful,
/// and it is what lets a bump rewrite exactly one store (D11).
pub fn store_dir(repo: &Path, name: &str) -> std::path::PathBuf {
    repo.join("annotations").join(name)
}

pub fn manifest_path(repo: &Path, name: &str) -> std::path::PathBuf {
    store_dir(repo, name).join("manifest.toml")
}

/// Reads and validates one source's annotation files, without resolving line
/// ranges.
///
/// `source_id` is passed in rather than read from the pin so that the bump
/// tool can load a store still keyed to the outgoing version.
pub fn read_annotations(repo: &Path, name: &str, source_id: &str) -> Result<Vec<Annotation>> {
    let dir = store_dir(repo, name);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
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
        // The manifest lives in the same directory but is not an annotation
        // file.
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
            parsed.source == source_id,
            "{}: source {:?} does not match pinned {source_id:?}",
            path.display(),
            parsed.source
        );
        out.extend(parsed.annotations);
    }
    Ok(out)
}

/// One glossary entry, resolved against the pinned tree it quotes.
#[derive(Debug, Clone)]
pub struct GlossaryItem {
    pub entry: schema::GlossaryEntry,
    pub range: LineRange,
    /// `"syn-3.0.5"` — which pinned tree the quotation comes from.
    pub source_id: String,
    /// The quoted lines, read from the pinned tree at load time. Never stored
    /// in the toml: a copy in the store is a copy that can rot, and the whole
    /// point of pinning the tree is that it does not have to be trusted twice.
    pub quoted: String,
}

/// Reads every `glossary/*.toml`, checking each entry against the glossary
/// source it names.
///
/// Two things are enforced here rather than in the gate, because a glossary
/// the site cannot render is not a warning: the named source must be pinned
/// with `role = "glossary"`, and every line range must exist in that tree.
pub fn read_glossary(repo: &Path) -> Result<Vec<GlossaryItem>> {
    let dir = repo.join("glossary");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let pin = vendor::load_pin(repo)?;
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.path());

    let mut out = Vec::new();
    for dir_entry in entries {
        let path = dir_entry.path();
        if path.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let parsed = schema::GlossaryFile::load(&path)?;

        let source = pin
            .sources
            .iter()
            .find(|s| s.source_id() == parsed.source)
            .with_context(|| {
                format!(
                    "{}: source {:?} is not pinned in vendor/pin.toml",
                    path.display(),
                    parsed.source
                )
            })?;
        anyhow::ensure!(
            source.role == vendor::Role::Glossary,
            "{}: {} is pinned as {:?}, but a glossary may only quote a glossary source \
             (D9) — annotate it instead, or change its role",
            path.display(),
            parsed.source,
            source.role
        );

        let root = source.dir(repo);
        for entry in parsed.entries {
            let range = LineRange::parse(&entry.lines)
                .with_context(|| format!("{}: entry {}", path.display(), entry.id))?;
            let file = root.join(&entry.file);
            let text = std::fs::read_to_string(&file).with_context(|| {
                format!(
                    "{}: entry {} quotes {}, which is not in the pinned tree",
                    path.display(),
                    entry.id,
                    entry.file
                )
            })?;
            let lines: Vec<&str> = text.lines().collect();
            anyhow::ensure!(
                range.end as usize <= lines.len(),
                "{}: entry {} quotes {}:{} but the file has {} lines",
                path.display(),
                entry.id,
                entry.file,
                entry.lines,
                lines.len()
            );
            let quoted = lines[range.start as usize - 1..range.end as usize].join("\n");
            out.push(GlossaryItem {
                entry,
                range,
                source_id: parsed.source.clone(),
                quoted,
            });
        }
    }
    Ok(out)
}

/// One narrative unit, resolved against the trees its steps cite.
#[derive(Debug, Clone)]
pub struct NarrativeItem {
    pub unit: schema::NarrativeUnit,
    pub steps: Vec<NarrativeStepItem>,
}

/// One step, with its citation resolved.
#[derive(Debug, Clone)]
pub struct NarrativeStepItem {
    pub step: schema::NarrativeStep,
    pub range: LineRange,
    /// The crate and version cited, split out because the reader is told which
    /// tree they are looking at on every step — the walk crosses between two.
    pub crate_name: String,
    pub version: String,
    /// True when this step cites an annotated crate, which is what makes it a
    /// crossing into a reference track rather than a stop in the walk.
    pub is_coverage: bool,
    /// True when the cited *file* is declared complete in that source's
    /// manifest, so a reference-track annotation is guaranteed to contain the
    /// range and the step is required to name it.
    ///
    /// This is finer than `is_coverage` because of the order §12 does the work
    /// in: `serde_derive` becomes an annotated crate in R1 and is annotated
    /// file by file through R5, so for most of that time a step cites a
    /// coverage source over ground no annotation claims yet. Requiring a name
    /// there would ask the walk to point at something that does not exist;
    /// requiring it the moment the file is declared complete is what converts
    /// the 85 steps without a flag day.
    pub claimed: bool,
    /// The cited lines, read from the pinned tree at load time. Never stored in
    /// the toml, for the same reason a glossary quotation is not (see
    /// [`GlossaryItem::quoted`]).
    pub quoted: String,
}

/// Reads every `narrative/*.toml`, in filename order, resolving each citation
/// against the pinned tree it names.
///
/// Filename order is reading order (see [`schema::NarrativeFile`]). What is
/// enforced here rather than in the gate is what the site cannot render
/// without: the named source must be pinned, it must not be a glossary source,
/// and every line range must exist.
pub fn read_narrative(repo: &Path) -> Result<Vec<NarrativeItem>> {
    let dir = repo.join("narrative");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let pin = vendor::load_pin(repo)?;
    // Which files are claimed ground, by source id. Read once: a unit cites
    // both annotated crates and the answer cannot depend on which step asks.
    let mut claimed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for source in pin.coverage()? {
        let manifest = Manifest::load(&manifest_path(repo, &source.name))?;
        claimed.insert(source.source_id(), manifest.complete);
    }

    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "toml"))
        .collect();
    paths.sort();

    let mut out = Vec::new();
    for path in paths {
        let parsed = schema::NarrativeFile::load(&path)?;
        let mut steps = Vec::new();
        for step in parsed.steps {
            let source = pin
                .sources
                .iter()
                .find(|s| s.source_id() == step.source)
                .with_context(|| {
                    format!(
                        "{}: step {} cites {:?}, which is not pinned in vendor/pin.toml",
                        path.display(),
                        step.id,
                        step.source
                    )
                })?;
            anyhow::ensure!(
                source.role != vendor::Role::Glossary,
                "{}: step {} cites {}, which is pinned as a glossary source — borrowed \
                 vocabulary is quoted in glossary/, not walked (D9)",
                path.display(),
                step.id,
                step.source
            );
            let range = LineRange::parse(&step.lines)
                .with_context(|| format!("{}: step {}", path.display(), step.id))?;
            let file = source.dir(repo).join(&step.file);
            let text = std::fs::read_to_string(&file).with_context(|| {
                format!(
                    "{}: step {} cites {}, which is not in the pinned {} tree",
                    path.display(),
                    step.id,
                    step.file,
                    step.source
                )
            })?;
            let lines: Vec<&str> = text.lines().collect();
            anyhow::ensure!(
                range.end as usize <= lines.len(),
                "{}: step {} cites {}:{} but the file has {} lines",
                path.display(),
                step.id,
                step.file,
                step.lines,
                lines.len()
            );
            steps.push(NarrativeStepItem {
                range,
                crate_name: source.name.clone(),
                version: source.version.clone(),
                is_coverage: source.role == vendor::Role::Coverage,
                claimed: claimed
                    .get(&step.source)
                    .is_some_and(|files| files.contains(&step.file)),
                quoted: lines[range.start as usize - 1..range.end as usize].join("\n"),
                step,
            });
        }
        out.push(NarrativeItem {
            unit: parsed.unit,
            steps,
        });
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
            source: "serde_core".to_string(),
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
                glossary: Vec::new(),
                macro_def: None,
                emits: None,
                emits_from: None,
                emits_case: None,
                body: "body".to_string(),
            },
        }
    }

    /// A store with one annotated source, built from `(file, units)` pairs.
    fn store_of(name: &str, reading_order: &[&str], files: &[(&str, Vec<Unit>)]) -> Store {
        let mut source = SourceStore {
            name: name.to_string(),
            ..SourceStore::default()
        };
        for (file, units) in files {
            source.files.push((file.to_string(), 1000));
            source.by_file.insert(file.to_string(), units.clone());
        }
        Store {
            sources: vec![source],
            reading_order: reading_order.iter().map(|s| s.to_string()).collect(),
            ..Store::default()
        }
    }

    fn order_of(store: &Store) -> Vec<String> {
        store
            .course_annotations("01-unit")
            .iter()
            .map(|u| u.annotation.id.clone())
            .collect()
    }

    /// Teaching order is the prereq graph first and position second.
    #[test]
    fn course_order_respects_prereqs_then_reading_order() {
        let store = store_of(
            "serde_core",
            &["src/ser/mod.rs", "src/de/impls.rs"],
            &[
                (
                    "src/ser/mod.rs",
                    vec![
                        annotation("c", "src/ser/mod.rs", 100, &["a"]),
                        annotation("b", "src/ser/mod.rs", 500, &[]),
                    ],
                ),
                (
                    "src/de/impls.rs",
                    vec![annotation("a", "src/de/impls.rs", 10, &[])],
                ),
            ],
        );

        // `b` first: earliest position with nothing to wait for. `c` sits
        // ahead of it in the source but cannot precede its own prereq.
        assert_eq!(order_of(&store), ["b", "a", "c"]);
    }

    /// A file the registry forgot must not silently sort into the middle.
    #[test]
    fn unlisted_files_sort_last() {
        let store = store_of(
            "serde_core",
            &["src/ser/mod.rs"],
            &[
                (
                    "src/de/mod.rs",
                    vec![annotation("x", "src/de/mod.rs", 1, &[])],
                ),
                (
                    "src/ser/mod.rs",
                    vec![annotation("y", "src/ser/mod.rs", 900, &[])],
                ),
            ],
        );
        assert_eq!(order_of(&store), ["y", "x"]);
    }

    /// D12. Both annotated crates have a `src/lib.rs`, so a bare registry
    /// entry must rank only the first source's copy — otherwise the second
    /// crate's file inherits a position written about someone else's.
    #[test]
    fn a_bare_reading_order_entry_does_not_rank_the_other_source() {
        let mut store = store_of(
            "serde_core",
            &["src/lib.rs", "serde_derive:src/ser.rs"],
            &[(
                "src/lib.rs",
                vec![annotation("core-lib", "src/lib.rs", 10, &[])],
            )],
        );
        let mut derive = SourceStore {
            name: "serde_derive".to_string(),
            ..SourceStore::default()
        };
        let mut lib = annotation("derive-lib", "src/lib.rs", 5, &[]);
        lib.source = "serde_derive".to_string();
        let mut ser = annotation("derive-ser", "src/ser.rs", 5, &[]);
        ser.source = "serde_derive".to_string();
        derive.by_file.insert("src/lib.rs".into(), vec![lib]);
        derive.by_file.insert("src/ser.rs".into(), vec![ser]);
        store.sources.push(derive);

        // `core-lib` matches the bare entry at rank 0; `derive-ser` matches the
        // qualified entry at rank 1; `derive-lib` matches nothing and sorts
        // last, rather than sharing rank 0 with a file in the other crate.
        assert_eq!(order_of(&store), ["core-lib", "derive-ser", "derive-lib"]);
    }
}
