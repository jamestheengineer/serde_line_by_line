//! The coverage gate.
//!
//! This is the mechanism that makes "every line" a property rather than a
//! slogan. It proves that every line of every pinned source file is claimed by
//! exactly one annotation, and that every cross-reference resolves.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use slbl_core::schema::{Annotation, Kind, LineRange, Manifest, Track};
use slbl_core::vendor::{self, SOURCE_ID};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct FileCoverage {
    pub file: String,
    pub total_lines: u32,
    pub claimed_lines: u32,
    pub annotations: usize,
    pub complete: bool,
    pub gaps: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub source: String,
    pub total_lines: u32,
    pub claimed_lines: u32,
    pub annotations: usize,
    pub files: Vec<FileCoverage>,
}

impl Report {
    pub fn percent(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.claimed_lines as f64 * 100.0 / self.total_lines as f64
    }
}

/// Errors are fatal; warnings are reported but do not fail the build.
#[derive(Default)]
struct Diagnostics {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Diagnostics {
    fn error(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

pub fn run(repo: &Path, write_json: bool) -> Result<Report> {
    let mut diag = Diagnostics::default();

    // 1. Vendor integrity. Everything downstream is keyed to these line numbers.
    vendor::verify(repo)?;

    let source_files = vendor::source_files(repo)?;
    let known_files: HashSet<&str> = source_files.iter().map(String::as_str).collect();

    let manifest = Manifest::load(&repo.join("annotations").join("manifest.toml"))?;
    if manifest.source != SOURCE_ID {
        bail!(
            "manifest source {:?} does not match pinned source {:?}",
            manifest.source,
            SOURCE_ID
        );
    }
    let complete: HashSet<&str> = manifest.complete.iter().map(String::as_str).collect();
    for f in &complete {
        if !known_files.contains(f) {
            diag.error(format!("manifest marks unknown file complete: {f}"));
        }
    }

    let features = load_feature_vocabulary(repo)?;
    let examples = load_example_names(repo)?;
    let annotations = slbl_core::read_annotations(repo)?;

    // 2. Identity and cross-reference integrity.
    let mut by_id: HashMap<&str, &Annotation> = HashMap::new();
    for a in &annotations {
        if by_id.insert(&a.id, a).is_some() {
            diag.error(format!("duplicate annotation id {:?}", a.id));
        }
    }
    for a in &annotations {
        if !known_files.contains(a.file.as_str()) {
            diag.error(format!("{}: unknown source file {:?}", a.id, a.file));
        }
        for e in &a.examples {
            if !examples.contains(e) {
                diag.error(format!("{}: unknown example {:?}", a.id, e));
            }
        }
        for p in &a.prereqs {
            if !by_id.contains_key(p.as_str()) {
                diag.error(format!("{}: unknown prereq {:?}", a.id, p));
            }
        }
        for f in &a.rust_features {
            if !features.contains(f) {
                diag.error(format!(
                    "{}: rust_feature {:?} not in docs/rust-features.md",
                    a.id, f
                ));
            }
        }
        if a.tracks.contains(&Track::Course) && a.course_unit.is_none() {
            diag.error(format!("{}: on course track but has no course_unit", a.id));
        }
        if a.body.trim().is_empty() {
            diag.error(format!("{}: empty body", a.id));
        }
        if a.title.trim().is_empty() {
            diag.error(format!("{}: empty title", a.id));
        }
        // A macro-use annotation is only cheap because it links back to the
        // macro-def that explains it. Without that link it is just an
        // unexplained span, and the renderer has nothing to collapse it
        // against — so this is an error, not a style note.
        match (a.kind, a.macro_def.as_deref()) {
            (Kind::MacroUse, None) => {
                diag.error(format!("{}: kind = macro-use but no macro_def", a.id));
            }
            (Kind::MacroUse, Some(target)) => match by_id.get(target) {
                None => diag.error(format!("{}: unknown macro_def {target:?}", a.id)),
                Some(def) if def.kind != Kind::MacroDef => diag.error(format!(
                    "{}: macro_def {target:?} has kind {:?}, expected macro-def",
                    a.id, def.kind
                )),
                Some(_) => {}
            },
            (kind, Some(target)) => diag.error(format!(
                "{}: macro_def {target:?} on kind {kind:?}; only macro-use may set it",
                a.id
            )),
            (_, None) => {}
        }
    }

    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for a in &annotations {
        *kinds
            .entry(format!("{:?}", a.kind).to_lowercase())
            .or_default() += 1;
    }

    // 3. The prereq graph must be acyclic so the course track can be ordered.
    if let Some(cycle) = find_cycle(&by_id) {
        diag.error(format!("prereq cycle: {}", cycle.join(" -> ")));
    }

    // 4. Line coverage, per file.
    let mut ranges: BTreeMap<&str, Vec<(LineRange, &str)>> = BTreeMap::new();
    for a in &annotations {
        match LineRange::parse(&a.lines) {
            Ok(r) => ranges.entry(&a.file).or_default().push((r, &a.id)),
            Err(e) => diag.error(format!("{}: {e}", a.id)),
        }
    }

    let mut files = Vec::new();
    let (mut total_lines, mut claimed_lines) = (0u32, 0u32);

    for rel in &source_files {
        let n = vendor::line_count(repo, rel)?;
        total_lines += n;
        let is_complete = complete.contains(rel.as_str());

        let mut claimed = 0u32;
        let mut gaps = Vec::new();
        let mut count = 0usize;

        if let Some(list) = ranges.get_mut(rel.as_str()) {
            list.sort_by_key(|(r, _)| (r.start, r.end));
            count = list.len();

            for w in list.windows(2) {
                if w[0].0.overlaps(&w[1].0) {
                    diag.error(format!(
                        "{rel}: annotations {} and {} both claim lines around {}",
                        w[0].1, w[1].1, w[1].0.start
                    ));
                }
            }
            if let Some((last, id)) = list.last() {
                if last.end > n {
                    diag.error(format!(
                        "{rel}: {id} claims line {} but file has {n} lines",
                        last.end
                    ));
                }
            }

            let mut cursor = 1u32;
            for (r, _) in list.iter() {
                if r.start > cursor {
                    gaps.push(fmt_gap(cursor, r.start - 1));
                }
                claimed += r
                    .line_count()
                    .min(n.saturating_sub(r.start).saturating_add(1));
                cursor = cursor.max(r.end + 1);
            }
            if cursor <= n {
                gaps.push(fmt_gap(cursor, n));
            }
        } else if n > 0 {
            gaps.push(fmt_gap(1, n));
        }

        claimed_lines += claimed.min(n);

        if !gaps.is_empty() {
            let msg = format!(
                "{rel}: {} line(s) unclaimed [{}]",
                n - claimed.min(n),
                summarize(&gaps)
            );
            if is_complete {
                diag.error(format!("{msg} (file is marked complete in manifest)"));
            } else {
                diag.warn(msg);
            }
        }

        files.push(FileCoverage {
            file: rel.clone(),
            total_lines: n,
            claimed_lines: claimed.min(n),
            annotations: count,
            complete: is_complete,
            gaps,
        });
    }

    let report = Report {
        source: SOURCE_ID.to_string(),
        total_lines,
        claimed_lines,
        annotations: annotations.len(),
        files,
    };

    print_report(&report, &diag, &kinds);

    if write_json {
        let path = repo.join("coverage.json");
        std::fs::write(&path, serde_json::to_string_pretty(&report)?)?;
        println!("wrote {}", path.display());
    }

    if !diag.errors.is_empty() {
        bail!("{} coverage error(s)", diag.errors.len());
    }
    Ok(report)
}

fn fmt_gap(a: u32, b: u32) -> String {
    if a == b {
        a.to_string()
    } else {
        format!("{a}-{b}")
    }
}

fn summarize(gaps: &[String]) -> String {
    const MAX: usize = 4;
    if gaps.len() <= MAX {
        gaps.join(", ")
    } else {
        format!("{}, +{} more", gaps[..MAX].join(", "), gaps.len() - MAX)
    }
}

/// The controlled vocabulary lives in `docs/rust-features.md` as list items of
/// the form "- `slug` — description", so the doc and the checker cannot drift.
fn load_feature_vocabulary(repo: &Path) -> Result<BTreeSet<String>> {
    let path = repo.join("docs").join("rust-features.md");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("- `") {
            if let Some((slug, _)) = rest.split_once('`') {
                out.insert(slug.to_string());
            }
        }
    }
    if out.is_empty() {
        bail!("no feature slugs found in {}", path.display());
    }
    Ok(out)
}

fn load_example_names(repo: &Path) -> Result<HashSet<String>> {
    let dir = repo.join("examples");
    let mut out = HashSet::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().is_dir() && entry.path().join("Cargo.toml").is_file() {
            out.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    Ok(out)
}

fn find_cycle(by_id: &HashMap<&str, &Annotation>) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Open,
        Done,
    }
    fn visit(
        id: &str,
        by_id: &HashMap<&str, &Annotation>,
        marks: &mut HashMap<String, Mark>,
        stack: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        match marks.get(id) {
            Some(Mark::Done) => return None,
            Some(Mark::Open) => {
                let at = stack.iter().position(|s| s == id).unwrap_or(0);
                let mut cycle = stack[at..].to_vec();
                cycle.push(id.to_string());
                return Some(cycle);
            }
            None => {}
        }
        marks.insert(id.to_string(), Mark::Open);
        stack.push(id.to_string());
        if let Some(a) = by_id.get(id) {
            // `macro_def` is a dependency edge just as much as a prereq is: the
            // use cannot be read before the def. Both must stay acyclic.
            for p in a.prereqs.iter().chain(a.macro_def.iter()) {
                if let Some(c) = visit(p, by_id, marks, stack) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        marks.insert(id.to_string(), Mark::Done);
        None
    }

    let mut marks = HashMap::new();
    let mut ids: Vec<&&str> = by_id.keys().collect();
    ids.sort();
    for id in ids {
        let mut stack = Vec::new();
        if let Some(c) = visit(id, by_id, &mut marks, &mut stack) {
            return Some(c);
        }
    }
    None
}

fn print_report(report: &Report, diag: &Diagnostics, kinds: &BTreeMap<String, usize>) {
    println!("\nsource: {}\n", report.source);
    println!(
        "{:<26}{:>8}{:>9}{:>8}{:>7}  status",
        "file", "lines", "claimed", "annots", "pct"
    );
    for f in &report.files {
        let pct = if f.total_lines == 0 {
            100.0
        } else {
            f.claimed_lines as f64 * 100.0 / f.total_lines as f64
        };
        let status = if f.complete {
            "complete"
        } else if f.claimed_lines == 0 {
            "-"
        } else {
            "in progress"
        };
        println!(
            "{:<26}{:>8}{:>9}{:>8}{:>6.1}%  {}",
            f.file, f.total_lines, f.claimed_lines, f.annotations, pct, status
        );
    }
    println!(
        "\n{:<26}{:>8}{:>9}{:>8}{:>6.1}%",
        "TOTAL",
        report.total_lines,
        report.claimed_lines,
        report.annotations,
        report.percent()
    );

    if !kinds.is_empty() {
        let parts: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
        println!("\nby kind: {}", parts.join(", "));
    }

    if !diag.warnings.is_empty() {
        println!("\n{} warning(s):", diag.warnings.len());
        for w in diag.warnings.iter().take(25) {
            println!("  warn: {w}");
        }
        if diag.warnings.len() > 25 {
            println!("  ... and {} more", diag.warnings.len() - 25);
        }
    }
    if !diag.errors.is_empty() {
        println!("\n{} error(s):", diag.errors.len());
        for e in &diag.errors {
            println!("  error: {e}");
        }
    }
    println!();
}
