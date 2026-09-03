//! The coverage gate.
//!
//! This is the mechanism that makes "every line" a property rather than a
//! slogan. It proves that every line of every pinned source file is claimed by
//! exactly one annotation, and that every cross-reference resolves.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use slbl_core::schema::{
    Annotation, CourseFile, CourseUnit, Kind, LineRange, Manifest, Supplement, Track, UnitStatus,
};
use slbl_core::vendor;
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

/// One course unit's shape, for the report and for `coverage.json`. The course
/// track has no line-coverage number of its own — it is a re-ordering of the
/// same annotations — so what it reports is how much of it is written.
#[derive(Debug, Serialize)]
pub struct UnitCoverage {
    pub id: String,
    pub title: String,
    pub supplement: Supplement,
    pub status: UnitStatus,
    pub annotations: usize,
    pub lines: u32,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub source: String,
    pub total_lines: u32,
    pub claimed_lines: u32,
    pub annotations: usize,
    pub files: Vec<FileCoverage>,
    pub course: Vec<UnitCoverage>,
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
    let source_id = vendor::load_pin(repo)?.source_id();

    let source_files = vendor::source_files(repo)?;
    let known_files: HashSet<&str> = source_files.iter().map(String::as_str).collect();

    let manifest = Manifest::load(&repo.join("annotations").join("manifest.toml"))?;
    if manifest.source != source_id {
        bail!(
            "manifest source {:?} does not match pinned source {source_id:?}",
            manifest.source,
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
    check_example_pins(repo, &vendor::load_pin(repo)?.version, &mut diag)?;
    let annotations = slbl_core::read_annotations(repo, &source_id)?;

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

    // 5. The course track: the registry, and every annotation that points at it.
    let course = check_course(
        repo,
        &source_id,
        &annotations,
        &known_files,
        &features,
        &examples,
        &mut diag,
    )?;

    let report = Report {
        source: source_id.clone(),
        total_lines,
        claimed_lines,
        annotations: annotations.len(),
        files,
        course,
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

/// Validates `annotations/course.toml` and every annotation that points into it.
///
/// The course track is the one part of the project with no line-coverage number
/// to keep it honest, so the checks here take its place: units must be ordered,
/// their prereqs must point backwards, and a unit's `supplement` must match
/// whether serde_core actually supplies any of it.
fn check_course(
    repo: &Path,
    source_id: &str,
    annotations: &[Annotation],
    known_files: &HashSet<&str>,
    features: &BTreeSet<String>,
    examples: &HashSet<String>,
    diag: &mut Diagnostics,
) -> Result<Vec<UnitCoverage>> {
    let path = repo.join("annotations").join("course.toml");
    let course = CourseFile::load(&path)?;
    if course.source != source_id {
        bail!(
            "course registry source {:?} does not match pinned source {source_id:?}",
            course.source,
        );
    }

    // The reading order sequences annotations drawn from several files into one
    // unit, so a file missing from it would silently sort last.
    let mut seen_files: HashSet<&str> = HashSet::new();
    for f in &course.reading_order {
        if !known_files.contains(f.as_str()) {
            diag.error(format!("course reading_order: unknown file {f:?}"));
        }
        if !seen_files.insert(f.as_str()) {
            diag.error(format!("course reading_order: {f:?} listed twice"));
        }
    }
    for f in known_files {
        if !seen_files.contains(f) {
            diag.error(format!("course reading_order: {f:?} is missing"));
        }
    }

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, u) in course.units.iter().enumerate() {
        if index.insert(&u.id, i).is_some() {
            diag.error(format!("duplicate course unit {:?}", u.id));
        }
    }
    // Declaration order is teaching order, and ids carry a numeric prefix.
    // Requiring the two to agree keeps a renumbered unit from reading in one
    // order and sorting in another.
    for pair in course.units.windows(2) {
        if pair[0].id >= pair[1].id {
            diag.error(format!(
                "course units out of order: {:?} declared before {:?}",
                pair[0].id, pair[1].id
            ));
        }
    }

    let mut counts: BTreeMap<&str, (usize, u32)> = BTreeMap::new();
    for a in annotations {
        let Some(unit) = a.course_unit.as_deref() else {
            continue;
        };
        match index.get(unit) {
            None => diag.error(format!("{}: unknown course_unit {unit:?}", a.id)),
            Some(_) => {
                let entry = counts.entry(unit).or_default();
                entry.0 += 1;
                entry.1 += LineRange::parse(&a.lines).map_or(0, |r| r.line_count());
            }
        }
        if !a.tracks.contains(&Track::Course) {
            diag.error(format!(
                "{}: has course_unit {unit:?} but is not on the course track",
                a.id
            ));
        }
    }

    // A prereq that lives in a later unit means the course track asks the
    // reader to know something it has not taught yet. That is a content
    // problem, not a build failure, so it is a warning with enough detail to
    // fix: either move the annotation or re-order the units.
    let unit_of: HashMap<&str, &str> = annotations
        .iter()
        .filter_map(|a| Some((a.id.as_str(), a.course_unit.as_deref()?)))
        .collect();
    for a in annotations {
        let Some(here) = a.course_unit.as_deref().and_then(|u| index.get(u)) else {
            continue;
        };
        for p in a.prereqs.iter().chain(a.macro_def.iter()) {
            let Some(there) = unit_of.get(p.as_str()).and_then(|u| index.get(u)) else {
                continue;
            };
            if there > here {
                diag.warn(format!(
                    "{}: depends on {p}, which the course track does not reach until {}",
                    a.id, course.units[*there].id
                ));
            }
        }
    }

    let mut out = Vec::new();
    for (i, u) in course.units.iter().enumerate() {
        let (count, lines) = counts.get(u.id.as_str()).copied().unwrap_or((0, 0));
        check_unit(u, i, &index, count, features, examples, diag);
        out.push(UnitCoverage {
            id: u.id.clone(),
            title: u.title.clone(),
            supplement: u.supplement,
            status: u.status,
            annotations: count,
            lines,
        });
    }
    Ok(out)
}

fn check_unit(
    u: &CourseUnit,
    position: usize,
    index: &HashMap<&str, usize>,
    count: usize,
    features: &BTreeSet<String>,
    examples: &HashSet<String>,
    diag: &mut Diagnostics,
) {
    for p in &u.prereqs {
        match index.get(p.as_str()) {
            None => diag.error(format!("{}: unknown prereq unit {p:?}", u.id)),
            // Backward-only prereqs make the unit graph acyclic by
            // construction, so there is no separate cycle check here.
            Some(&at) if at >= position => {
                diag.error(format!("{}: prereq {p:?} is not an earlier unit", u.id))
            }
            Some(_) => {}
        }
    }
    for f in &u.rust_features {
        if !features.contains(f) {
            diag.error(format!(
                "{}: rust_feature {f:?} not in docs/rust-features.md",
                u.id
            ));
        }
    }
    for e in &u.examples {
        if !examples.contains(e) {
            diag.error(format!("{}: unknown example {e:?}", u.id));
        }
    }
    if u.summary.trim().is_empty() || u.body.trim().is_empty() {
        diag.error(format!("{}: empty summary or body", u.id));
    }
    // `supplement` is the claim the UI shows the reader. It has to match what
    // the store actually holds, or the honesty label is decoration.
    match (u.supplement, count) {
        (Supplement::Full, n) if n > 0 => diag.error(format!(
            "{}: supplement = \"full\" but {n} annotation(s) are tagged to it",
            u.id
        )),
        (Supplement::None | Supplement::Partial, 0) => diag.error(format!(
            "{}: supplement = {:?} but no annotations are tagged to it",
            u.id, u.supplement
        )),
        _ => {}
    }
    if u.status == UnitStatus::Planned {
        diag.warn(format!(
            "{}: unit is planned, not written{}",
            u.id,
            match u.supplement {
                Supplement::Full => " (nothing in serde_core to fall back on)",
                _ => "",
            }
        ));
    }
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

/// Every example that depends on the crate must pin the version the
/// annotations describe.
///
/// The examples build against the published crate rather than the vendored
/// copy, so nothing but this connects the two. An example running a different
/// release than the one being explained is the same drift the vendor checksum
/// exists to prevent, arriving through the other door — and `cargo xtask bump`
/// retargets these manifests, so a mismatch means a bump was left half-done.
fn check_example_pins(repo: &Path, version: &str, diag: &mut Diagnostics) -> Result<()> {
    let want = format!("{} = \"={version}\"", vendor::CRATE_NAME);
    let dir = repo.join("examples");
    if !dir.is_dir() {
        return Ok(());
    }
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path().join("Cargo.toml"))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        let Some(line) = text.lines().find(|l| {
            l.trim_start()
                .starts_with(&format!("{} ", vendor::CRATE_NAME))
        }) else {
            continue;
        };
        if line.trim() != want {
            diag.error(format!(
                "{}: pins `{}` but the vendored source is {version}",
                path.strip_prefix(repo).unwrap_or(&path).display(),
                line.trim(),
            ));
        }
    }
    Ok(())
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

    if !report.course.is_empty() {
        let written = report
            .course
            .iter()
            .filter(|u| u.status == UnitStatus::Written)
            .count();
        println!(
            "\ncourse track: {}/{} units written\n",
            written,
            report.course.len()
        );
        println!(
            "{:<32}{:>8}{:>8}  {:<12}status",
            "unit", "annots", "lines", "supplement"
        );
        for u in &report.course {
            println!(
                "{:<32}{:>8}{:>8}  {:<12}{}",
                u.id,
                u.annotations,
                u.lines,
                format!("{:?}", u.supplement).to_lowercase(),
                format!("{:?}", u.status).to_lowercase(),
            );
        }
    }

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
