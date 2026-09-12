//! The coverage gate.
//!
//! This is the mechanism that makes "every line" a property rather than a
//! slogan. It proves that every line of every pinned source file is claimed by
//! exactly one annotation, and that every cross-reference resolves.

use crate::harness;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use slbl_core::schema::{
    Annotation, CourseFile, CourseUnit, Kind, LineRange, Manifest, Supplement, Track, UnitStatus,
};
use slbl_core::vendor::{self, Pin};
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

/// One narrative unit's shape. There is no percentage here on purpose, and the
/// reason outlived the role that first gave it: the walk was over an unclaimed
/// crate (PLAN.md §11) and is now an ordering over a claimed one (§12), but
/// either way a percentage would be measuring the wrong thing — a path is not
/// a fraction of a crate. What is reported instead is the size of the walk and
/// how many of its stops land inside an annotation.
#[derive(Debug, Serialize)]
pub struct NarrativeUnitReport {
    pub id: String,
    pub title: String,
    pub steps: usize,
    pub cited_lines: u32,
    pub crossings: usize,
}

/// One annotated source's coverage.
///
/// Every percentage in this report hangs off one of these, and none off the
/// report itself. There are two annotated crates since D12, they promise the
/// same thing about different amounts of code, and a figure spanning both
/// would describe neither: 12,037 claimed lines out of 21,012 is not a fact
/// about anything a reader could check.
#[derive(Debug, Serialize)]
pub struct SourceCoverage {
    /// "serde_core-1.0.229".
    pub source: String,
    /// "serde_core".
    pub name: String,
    pub total_lines: u32,
    pub claimed_lines: u32,
    pub annotations: usize,
    pub files: Vec<FileCoverage>,
    pub kinds: BTreeMap<String, usize>,
    /// Files with no annotation at all. Reported as one line rather than as a
    /// warning each: nobody has started them, which is a position on the
    /// roadmap and not a defect.
    pub not_started: usize,
    pub not_started_lines: u32,
}

impl SourceCoverage {
    pub fn percent(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.claimed_lines as f64 * 100.0 / self.total_lines as f64
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// The annotated sources, in pin order.
    pub sources: Vec<SourceCoverage>,
    pub course: Vec<UnitCoverage>,
    pub narrative: Vec<NarrativeUnitReport>,
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
    check_vendor_dirs(repo, &mut diag)?;
    print_sources(repo)?;

    let pin = vendor::load_pin(repo)?;
    let glossary = check_glossary(repo, &mut diag)?;
    let features = load_feature_vocabulary(repo)?;
    let examples = load_example_names(repo)?;
    // The examples build against the first coverage source: they are
    // `serde_core` programs, and `serde_derive` is not something an example
    // calls, it is something the site expands.
    let first = pin.coverage()?[0];
    check_example_pins(repo, &first.name, &first.version, &mut diag)?;
    check_harness_pins(repo, &pin, &mut diag)?;

    // 2. Each annotated source's store, loaded before anything is checked, so
    //    that cross-references may point across crates. A course unit teaching
    //    what `quote!` emits leans on `Serializer`'s contract, which lives in
    //    the other one.
    let mut loaded: Vec<SourceInput> = Vec::new();
    for source in pin.coverage()? {
        let source_id = source.source_id();
        let root = source.dir(repo);
        let files = vendor::source_files_in(&root)?;

        let manifest = Manifest::load(&slbl_core::manifest_path(repo, &source.name))?;
        if manifest.source != source_id {
            bail!(
                "{}: manifest source {:?} does not match pinned source {source_id:?}",
                source.name,
                manifest.source,
            );
        }
        for f in &manifest.complete {
            if !files.contains(f) {
                diag.error(format!(
                    "{}: manifest marks unknown file complete: {f}",
                    source.name
                ));
            }
        }

        loaded.push(SourceInput {
            name: source.name.clone(),
            source_id: source_id.clone(),
            root,
            files,
            complete: manifest.complete.into_iter().collect(),
            annotations: slbl_core::read_annotations(repo, &source.name, &source_id)?,
        });
    }

    // 3. Identity and cross-reference integrity, over every store at once.
    //    Annotation ids are one namespace across the project: the narrative
    //    names them without saying which crate, and a prereq may cross.
    let mut by_id: HashMap<&str, &Annotation> = HashMap::new();
    let mut file_of: HashMap<&str, (&str, &str)> = HashMap::new();
    for input in &loaded {
        for a in &input.annotations {
            if by_id.insert(&a.id, a).is_some() {
                diag.error(format!("duplicate annotation id {:?}", a.id));
            }
            file_of.insert(&a.id, (input.name.as_str(), a.file.as_str()));
        }
    }

    for input in &loaded {
        let known_files: HashSet<&str> = input.files.iter().map(String::as_str).collect();
        for a in &input.annotations {
            if !known_files.contains(a.file.as_str()) {
                diag.error(format!(
                    "{}: unknown source file {:?} in {}",
                    a.id, a.file, input.name
                ));
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
            // A citation into borrowed vocabulary is the reader's only way to
            // the definition of a type the annotated crate does not define
            // (D9). One that resolves to nothing is a dead end the renderer
            // cannot show.
            for cited in &a.glossary {
                if !glossary.contains(cited) {
                    diag.error(format!(
                        "{}: cites glossary {cited:?}, which has no entry",
                        a.id
                    ));
                }
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
                    // A macro definition in the other crate cannot be the one
                    // this use expands: `macro-use` is the compression trick
                    // for a macro invoked many times in the crate that defines
                    // it, and crossing crates here means a wrong id that
                    // happened to resolve.
                    Some(_) => {
                        let here = file_of.get(a.id.as_str()).map(|(c, _)| *c);
                        let there = file_of.get(target).map(|(c, _)| *c);
                        if here != there {
                            diag.error(format!(
                                "{}: macro_def {target:?} is in {}, not {}",
                                a.id,
                                there.unwrap_or("?"),
                                here.unwrap_or("?")
                            ));
                        }
                    }
                },
                (kind, Some(target)) => diag.error(format!(
                    "{}: macro_def {target:?} on kind {kind:?}; only macro-use may set it",
                    a.id
                )),
                (_, None) => {}
            }
        }
    }

    // 4. The prereq graph must be acyclic so the course track can be ordered.
    if let Some(cycle) = find_cycle(&by_id) {
        diag.error(format!("prereq cycle: {}", cycle.join(" -> ")));
    }

    // 5. Line coverage, per source and then per file.
    let mut sources = Vec::new();
    for input in &loaded {
        sources.push(cover_source(input, &mut diag)?);
    }

    // 6. The course track: the registry, and every annotation that points at it.
    let course = check_course(repo, &loaded, &features, &examples, &mut diag)?;

    // 7. The narrative track, which is now an ordering over both annotated
    //    stores rather than a walk of an unclaimed one (PLAN.md §12).
    let narrative = check_narrative(repo, &by_id, &glossary, &mut diag)?;

    let report = Report {
        sources,
        course,
        narrative,
    };

    print_report(&report, &diag);

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

/// One annotated source as the gate reads it, before any checking.
struct SourceInput {
    name: String,
    source_id: String,
    root: std::path::PathBuf,
    files: Vec<String>,
    complete: HashSet<String>,
    annotations: Vec<Annotation>,
}

/// Line coverage for one source: overlaps, out-of-range claims, and gaps.
///
/// A gap is a warning until the file is named in that source's manifest, and a
/// hard failure after. That ramp is what lets a second annotated crate exist
/// at 0% without the gate lying about it or failing over it.
fn cover_source(input: &SourceInput, diag: &mut Diagnostics) -> Result<SourceCoverage> {
    let mut ranges: BTreeMap<&str, Vec<(LineRange, &str)>> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    for a in &input.annotations {
        *kinds
            .entry(format!("{:?}", a.kind).to_lowercase())
            .or_default() += 1;
        match LineRange::parse(&a.lines) {
            Ok(r) => ranges.entry(&a.file).or_default().push((r, &a.id)),
            Err(e) => diag.error(format!("{}: {e}", a.id)),
        }
    }

    let mut files = Vec::new();
    let (mut total_lines, mut claimed_lines) = (0u32, 0u32);
    let (mut not_started, mut not_started_lines) = (0usize, 0u32);

    for rel in &input.files {
        let n = vendor::line_count_in(&input.root, rel)?;
        total_lines += n;
        let is_complete = input.complete.contains(rel);

        let mut claimed = 0u32;
        let mut gaps = Vec::new();
        let mut count = 0usize;

        if let Some(list) = ranges.get_mut(rel.as_str()) {
            list.sort_by_key(|(r, _)| (r.start, r.end));
            count = list.len();

            for w in list.windows(2) {
                if w[0].0.overlaps(&w[1].0) {
                    diag.error(format!(
                        "{}/{rel}: annotations {} and {} both claim lines around {}",
                        input.name, w[0].1, w[1].1, w[1].0.start
                    ));
                }
            }
            if let Some((last, id)) = list.last() {
                if last.end > n {
                    diag.error(format!(
                        "{}/{rel}: {id} claims line {} but file has {n} lines",
                        input.name, last.end
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

        // Three states, and they are deliberately not the same thing. A file
        // named in the manifest must be whole: that is the promise, and a hole
        // in it fails the build. A file somebody has started and left a hole in
        // is a warning, because the hole is probably an oversight. A file
        // nobody has begun is neither — it is the roadmap, and printing 28
        // warnings for the 28 files R2 through R5 will write would drown the
        // one warning that means something.
        if !gaps.is_empty() {
            if is_complete {
                diag.error(format!(
                    "{}/{rel}: {} line(s) unclaimed [{}] (file is marked complete in manifest)",
                    input.name,
                    n - claimed.min(n),
                    summarize(&gaps)
                ));
            } else if count > 0 {
                diag.warn(format!(
                    "{}/{rel}: {} line(s) unclaimed [{}]",
                    input.name,
                    n - claimed.min(n),
                    summarize(&gaps)
                ));
            } else {
                not_started += 1;
                not_started_lines += n;
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

    Ok(SourceCoverage {
        not_started,
        not_started_lines,
        source: input.source_id.clone(),
        name: input.name.clone(),
        total_lines,
        claimed_lines,
        annotations: input.annotations.len(),
        files,
        kinds,
    })
}

/// Validates `annotations/course.toml` and every annotation that points into it.
///
/// The course track is the one part of the project with no line-coverage number
/// to keep it honest, so the checks here take its place: units must be ordered,
/// their prereqs must point backwards, and a unit's `supplement` must match
/// whether serde_core actually supplies any of it.
fn check_course(
    repo: &Path,
    loaded: &[SourceInput],
    features: &BTreeSet<String>,
    examples: &HashSet<String>,
    diag: &mut Diagnostics,
) -> Result<Vec<UnitCoverage>> {
    let path = repo.join("annotations").join("course.toml");
    let course = CourseFile::load(&path)?;
    let first = &loaded[0];
    if course.source != first.source_id {
        bail!(
            "course registry source {:?} does not match pinned source {:?}",
            course.source,
            first.source_id,
        );
    }

    // A reading-order entry is `src/ser/mod.rs` for the first source, or
    // `serde_derive:src/ser.rs` for any other. Both crates have a `src/lib.rs`,
    // so the bare form has to mean exactly one of them (D12).
    let mut known_files: HashSet<String> = HashSet::new();
    for (i, input) in loaded.iter().enumerate() {
        for f in &input.files {
            known_files.insert(format!("{}:{f}", input.name));
            if i == 0 {
                known_files.insert(f.clone());
            }
        }
    }

    // The reading order sequences annotations drawn from several files into one
    // unit, so a file missing from it would silently sort last. What has to be
    // listed is a file the course track actually draws from — demanding an
    // entry for all 28 files of a crate no unit has reached yet would fill the
    // registry with placeholders that mean nothing.
    let mut seen_files: HashSet<&str> = HashSet::new();
    for f in &course.reading_order {
        if !known_files.contains(f.as_str()) {
            diag.error(format!("course reading_order: unknown file {f:?}"));
        }
        if !seen_files.insert(f.as_str()) {
            diag.error(format!("course reading_order: {f:?} listed twice"));
        }
    }
    for (i, input) in loaded.iter().enumerate() {
        for a in &input.annotations {
            if !a.tracks.contains(&Track::Course) {
                continue;
            }
            let qualified = format!("{}:{}", input.name, a.file);
            if seen_files.contains(qualified.as_str())
                || (i == 0 && seen_files.contains(a.file.as_str()))
            {
                continue;
            }
            diag.error(format!(
                "course reading_order: {qualified:?} is missing, and {} teaches from it",
                a.id
            ));
        }
    }

    let annotations: Vec<&Annotation> = loaded.iter().flat_map(|i| i.annotations.iter()).collect();

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
    for a in &annotations {
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
    // reader to know something it has not taught yet, and the renderer shows
    // them a "leans on something the course has not reached" notice where it
    // happens. This was a warning while the count was working its way down
    // from 33; it is a hard failure now that it is zero, for the same reason
    // `manifest.toml` turns coverage gaps into failures once a file is
    // complete. The fix is never to re-order the units — unit 12 depends on
    // unit 11 eight times over, so swapping a pair trades one violation set
    // for a larger one — it is to move the annotation whose placement is
    // wrong, usually a framing annotation parked after the items that need it.
    let unit_of: HashMap<&str, &str> = annotations
        .iter()
        .filter_map(|a| Some((a.id.as_str(), a.course_unit.as_deref()?)))
        .collect();
    for a in &annotations {
        let Some(here) = a.course_unit.as_deref().and_then(|u| index.get(u)) else {
            continue;
        };
        for p in a.prereqs.iter().chain(a.macro_def.iter()) {
            let Some(there) = unit_of.get(p.as_str()).and_then(|u| index.get(u)) else {
                continue;
            };
            if there > here {
                diag.error(format!(
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

/// The narrative track (PLAN.md §11): `narrative/*.toml`.
///
/// The narrative makes no coverage promise, so there is no percentage here to
/// keep it honest and these checks stand in its place. Three of them matter:
///
/// * **D8, across two sources.** A step may not lean on one that comes later in
///   the walk. The graph now spans `serde_derive` and `serde_core`, which is
///   the only thing that changed — the rule and the reason are the course
///   track's.
/// * **A crossing lands on claimed ground.** A step citing the coverage source
///   must name the reference-track annotation containing it, and the range must
///   really be inside that annotation. The narrative gets to reuse 12,037
///   annotated lines instead of re-explaining them, and the link cannot rot.
/// * **A step cites something.** `read_narrative` has already proved the range
///   resolves in a pinned tree; what is left is the bookkeeping a reader would
///   notice.
///
/// `emits` — a step's claim about what the cited machinery generates — is
/// checked where the expansion actually happens, in the site generator, which
/// renders the real bytes rather than the string in the store. `cargo site` is
/// a CI gate and a pre-push gate, so the claim is enforced either way.
fn check_narrative(
    repo: &Path,
    by_id: &HashMap<&str, &Annotation>,
    glossary: &HashSet<String>,
    diag: &mut Diagnostics,
) -> Result<Vec<NarrativeUnitReport>> {
    let units = slbl_core::read_narrative(repo)?;
    if units.is_empty() {
        return Ok(Vec::new());
    }

    let cases = expand_case_names(repo)?;

    // Position in the walk, for the forward-reference check. Steps are numbered
    // across units rather than within them, because the walk is one sequence.
    let mut position: HashMap<&str, usize> = HashMap::new();
    let mut unit_of: HashMap<&str, &str> = HashMap::new();
    let mut n = 0;
    for unit in &units {
        for step in &unit.steps {
            if position.insert(step.step.id.as_str(), n).is_some() {
                diag.error(format!("duplicate narrative step id {:?}", step.step.id));
            }
            unit_of.insert(step.step.id.as_str(), unit.unit.id.as_str());
            n += 1;
        }
    }
    for pair in units.windows(2) {
        if pair[0].unit.id >= pair[1].unit.id {
            diag.error(format!(
                "narrative units out of order: {:?} before {:?}",
                pair[0].unit.id, pair[1].unit.id
            ));
        }
    }

    let mut out = Vec::new();
    for unit in &units {
        let u = &unit.unit;
        if u.title.trim().is_empty() || u.summary.trim().is_empty() || u.body.trim().is_empty() {
            diag.error(format!("{}: empty title, summary or body", u.id));
        }
        if unit.steps.is_empty() {
            diag.error(format!(
                "{}: no steps — a narrative unit is its citations plus its framing",
                u.id
            ));
        }
        if let Some(case) = &u.expand_case {
            if !cases.contains(case) {
                diag.error(format!(
                    "{}: expand_case {case:?} is not a file in expand/cases/",
                    u.id
                ));
            }
        }

        let mut crossings = 0;
        let mut cited_lines = 0;
        for item in &unit.steps {
            let s = &item.step;
            cited_lines += item.range.line_count();
            if s.title.trim().is_empty() || s.body.trim().is_empty() {
                diag.error(format!("{}: empty title or body", s.id));
            }
            for cited in &s.glossary {
                if !glossary.contains(cited) {
                    diag.error(format!(
                        "{}: cites glossary {cited:?}, which has no entry",
                        s.id
                    ));
                }
            }
            for p in &s.leans_on {
                match position.get(p.as_str()) {
                    None => diag.error(format!("{}: unknown leans_on {p:?}", s.id)),
                    Some(&there) if there >= position[s.id.as_str()] => diag.error(format!(
                        "{}: leans on {p}, which the narrative does not reach until {}",
                        s.id,
                        unit_of[p.as_str()]
                    )),
                    Some(_) => {}
                }
            }

            // The crossing rule. A step landing on claimed ground is the walk
            // crossing into territory a reference track already owns, so it
            // says which annotation it landed in and the gate proves it landed
            // there.
            //
            // "Claimed" is per file, not per source (see `NarrativeStepItem::
            // claimed`): `serde_derive` became an annotated crate in R1 and is
            // claimed file by file through R5, so a step into a file no
            // annotation covers yet is asked for nothing. The moment its file
            // is named in the manifest, it is asked for a name — which is how
            // the 85 steps convert without a flag day.
            match (item.claimed, s.annotation.as_deref()) {
                (true, None) => diag.error(format!(
                    "{}: cites {}, which is claimed ground, but names no annotation — a \
                     crossing into a reference track must say where it lands",
                    s.id, s.file
                )),
                (_, Some(id)) => match by_id.get(id) {
                    None => diag.error(format!("{}: unknown annotation {id:?}", s.id)),
                    Some(a) if a.file != s.file => diag.error(format!(
                        "{}: annotation {id} is in {}, not {}",
                        s.id, a.file, s.file
                    )),
                    Some(a) => {
                        let r = LineRange::parse(&a.lines)?;
                        if item.range.start < r.start || item.range.end > r.end {
                            diag.error(format!(
                                "{}: cites {}:{} but annotation {id} claims {} — a crossing \
                                 must sit inside the annotation it names",
                                s.id, s.file, s.lines, a.lines
                            ));
                        }
                    }
                },
                (false, None) => {}
            }
            // A step may name an annotation before the file is complete — the
            // annotation exists, it just is not yet compulsory — but it may
            // never name one in a crate that is not annotated at all.
            if !item.is_coverage && s.annotation.is_some() {
                diag.error(format!(
                    "{}: names an annotation, but {} is not an annotated source",
                    s.id, s.source
                ));
            }
            if s.annotation.is_some() {
                crossings += 1;
            }

            match (s.emits.is_some(), s.emits_from.is_some()) {
                (true, false) => diag.error(format!(
                    "{}: has emits but no emits_from — which half of the expansion?",
                    s.id
                )),
                (false, true) => diag.error(format!("{}: has emits_from but no emits", s.id)),
                _ => {}
            }
            match (&s.emits, s.emits_case.as_ref().or(u.expand_case.as_ref())) {
                (Some(_), None) => diag.error(format!(
                    "{}: quotes emitted code, but neither it nor {} names a case to quote \
                     it from",
                    s.id, u.id
                )),
                (_, Some(case)) if !cases.contains(case) => diag.error(format!(
                    "{}: emits_case {case:?} is not a file in expand/cases/",
                    s.id
                )),
                _ => {}
            }
            if s.emits.is_none() && s.emits_case.is_some() {
                diag.error(format!("{}: has emits_case but no emits", s.id));
            }
        }

        out.push(NarrativeUnitReport {
            id: u.id.clone(),
            title: u.title.clone(),
            steps: unit.steps.len(),
            cited_lines,
            crossings,
        });
    }
    Ok(out)
}

/// The names of the committed expansion inputs, read off disk rather than from
/// the `expand` crate: the gate has no business compiling `serde_derive` to
/// find out that a directory has six files in it.
fn expand_case_names(repo: &Path) -> Result<HashSet<String>> {
    let dir = repo.join("expand").join("cases");
    let mut out = HashSet::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "rs") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                out.insert(stem.to_string());
            }
        }
    }
    Ok(out)
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
fn check_example_pins(
    repo: &Path,
    name: &str,
    version: &str,
    diag: &mut Diagnostics,
) -> Result<()> {
    let want = format!("{name} = \"={version}\"");
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
        let Some(line) = text
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{name} ")))
        else {
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

/// Does this text declare `name`, rather than merely mention it?
///
/// The distinction matters because syn's rustdoc names the item it documents,
/// often several times, so "the quotation contains the word" passes for a
/// range that points at the prose above the definition instead of at the
/// definition. Both of this repo's first two macro entries did exactly that.
fn declares(text: &str, name: &str) -> bool {
    text.lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with("//"))
        .any(|l| {
            [
                "struct ", "enum ", "trait ", "fn ", "type ", "macro_rules! ", "union ", "const ",
            ]
            .iter()
            .any(|kw| {
                l.split(kw).skip(1).any(|rest| {
                    rest.trim_start()
                        .strip_prefix(name)
                        .is_some_and(|after| {
                            !after.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                        })
                })
            })
        })
        // `syn::Ident` is `pub use proc_macro2::Ident;` — a re-export is a
        // declaration, and its whole point is that there is nothing else.
        || text
            .lines()
            .map(str::trim_start)
            .any(|l| l.starts_with("pub use") && l.trim_end_matches(';').ends_with(name))
}

/// The expansion harness compiles the versions the pin names.
///
/// `expand/` is the one crate that builds *against* pinned sources rather than
/// reading them: `syn`, `quote` and `proc-macro2` are Cargo dependencies there
/// and vendored trees here. Nothing but this connects the two, and the failure
/// it prevents is a quiet one — the glossary quoting one release of `syn`
/// while the expansions on the site were produced by whatever release Cargo
/// felt like resolving. The same drift the vendor checksum exists to stop, arriving
/// through the dependency graph instead of the tree.
///
/// The manifest and the lockfile are both checked, because the manifest says
/// what is permitted and the lockfile says what is compiled.
fn check_harness_pins(repo: &Path, pin: &Pin, diag: &mut Diagnostics) -> Result<()> {
    let path = harness::manifest_path(repo);
    if !path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(&path)?;
    for source in &pin.sources {
        let Some((_, req)) = harness::dep_req(&text, &source.name) else {
            // Not a dependency of the harness. `serde_core` never is, and
            // `serde_derive` is compiled out of `vendor/` by build.rs rather
            // than resolved by Cargo.
            continue;
        };
        let want = format!("={}", source.version);
        if req != want {
            diag.error(format!(
                "{}: builds against `{} = \"{req}\"`, but the pinned tree is {} — pin it \
                 exactly (`\"{want}\"`) so the expander cannot run a release the glossary \
                 does not describe",
                harness::MANIFEST,
                source.name,
                source.version,
            ));
        } else if !harness::lock_has(repo, &source.name, &source.version)? {
            diag.error(format!(
                "Cargo.lock has no {} {}, so the harness is not built from the pinned tree; \
                 run `cargo update -p {} --precise {}`",
                source.name, source.version, source.name, source.version,
            ));
        }
    }
    Ok(())
}

/// The glossary resolves, and nothing cites an entry that is not there.
///
/// `read_glossary` has already proved the harder half — that every quoted range
/// exists in a tree pinned with `role = "glossary"` — so drift in `syn` breaks
/// this gate the same way drift in `serde_core` breaks coverage. What is left
/// is the bookkeeping a reader would notice: duplicate ids, and `see_also`
/// links pointing at nothing.
fn check_glossary(repo: &Path, diag: &mut Diagnostics) -> Result<HashSet<String>> {
    let items = slbl_core::read_glossary(repo)?;
    let mut ids: HashSet<String> = HashSet::new();
    for item in &items {
        if !ids.insert(item.entry.id.clone()) {
            diag.error(format!("duplicate glossary id {:?}", item.entry.id));
        }
        if item.entry.title.trim().is_empty() {
            diag.error(format!("{}: empty glossary title", item.entry.id));
        }
        if item.entry.body.trim().is_empty() {
            diag.error(format!("{}: empty glossary body", item.entry.id));
        }
        if item.quoted.trim().is_empty() {
            diag.error(format!(
                "{}: quotes {}:{}, which is blank",
                item.entry.id, item.entry.file, item.entry.lines
            ));
        }
        // The range has to actually contain the thing it claims to define. A
        // line range that still resolves after an upstream edit but now points
        // at the neighbouring type is the one drift the tree hash cannot
        // catch, because the hash is checked against the version the entry was
        // written for and both are the same file.
        let name = item.entry.id.rsplit("::").next().unwrap_or(&item.entry.id);
        if !declares(&item.quoted, name) {
            diag.error(format!(
                "{}: quotes {}:{}, which does not declare {name} — a range that lands \
                 in the rustdoc above a definition still resolves, and reads as one",
                item.entry.id, item.entry.file, item.entry.lines
            ));
        }
    }
    for item in &items {
        for other in &item.entry.see_also {
            if !ids.contains(other) {
                diag.error(format!(
                    "{}: see_also {other:?} is not a glossary entry",
                    item.entry.id
                ));
            }
        }
    }
    if !items.is_empty() {
        let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
        let mut quoted = 0u32;
        for item in &items {
            *by_source.entry(&item.source_id).or_default() += 1;
            quoted += item.range.line_count();
        }
        let parts: Vec<String> = by_source.iter().map(|(s, n)| format!("{s} {n}")).collect();
        println!(
            "\nglossary: {} entries quoting {quoted} lines ({})",
            items.len(),
            parts.join(", ")
        );
    }
    Ok(ids)
}

/// Every directory under `vendor/` is pinned, and every pinned source is on
/// disk. `verify` proves the second half; this proves the first, so a tree
/// nobody declared cannot sit in the repo being quoted by nothing and checked
/// by nothing.
fn check_vendor_dirs(repo: &Path, diag: &mut Diagnostics) -> Result<()> {
    let pin = vendor::load_pin(repo)?;
    let pinned: HashSet<String> = pin.sources.iter().map(|s| s.source_id()).collect();
    let dir = repo.join("vendor");
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !pinned.contains(&name) {
            diag.error(format!(
                "vendor/{name}/ is not listed in vendor/pin.toml — add it with a role, \
                 or remove it"
            ));
        }
    }
    Ok(())
}

/// What is pinned and what each gate asks of it.
fn print_sources(repo: &Path) -> Result<()> {
    let pin = vendor::load_pin(repo)?;
    println!(
        "\n{:<28}{:<11}{:>7}{:>9}  gate",
        "source", "role", "files", "lines"
    );
    for source in &pin.sources {
        let root = source.dir(repo);
        let files = vendor::source_files_in(&root)?;
        let mut lines = 0u32;
        for rel in &files {
            lines += vendor::line_count_in(&root, rel)?;
        }
        let gate = match source.role {
            vendor::Role::Coverage => "every line claimed",
            vendor::Role::Narrative => "annotated where the story goes",
            vendor::Role::Glossary => "quoted, never claimed",
        };
        println!(
            "{:<28}{:<11}{:>7}{:>9}  {gate}",
            source.source_id(),
            format!("{:?}", source.role).to_lowercase(),
            files.len(),
            lines
        );
    }
    Ok(())
}

fn print_report(report: &Report, diag: &Diagnostics) {
    for source in &report.sources {
        println!("\nsource: {}\n", source.source);
        println!(
            "{:<26}{:>8}{:>9}{:>8}{:>7}  status",
            "file", "lines", "claimed", "annots", "pct"
        );
        for f in &source.files {
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
            source.total_lines,
            source.claimed_lines,
            source.annotations,
            source.percent()
        );
        if source.not_started > 0 {
            println!(
                "{} file(s) not started, {} lines",
                source.not_started, source.not_started_lines
            );
        }
        if !source.kinds.is_empty() {
            let parts: Vec<String> = source
                .kinds
                .iter()
                .map(|(k, n)| format!("{k} {n}"))
                .collect();
            println!("by kind: {}", parts.join(", "));
        }
    }

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

    if !report.narrative.is_empty() {
        let steps: usize = report.narrative.iter().map(|u| u.steps).sum();
        let cited: u32 = report.narrative.iter().map(|u| u.cited_lines).sum();
        println!(
            "\nnarrative track: {} units, {steps} steps, {cited} lines cited\n",
            report.narrative.len()
        );
        println!(
            "{:<32}{:>8}{:>8}{:>12}",
            "unit", "steps", "lines", "crossings"
        );
        for u in &report.narrative {
            println!(
                "{:<32}{:>8}{:>8}{:>12}",
                u.id, u.steps, u.cited_lines, u.crossings
            );
        }
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

#[cfg(test)]
mod tests {
    use super::declares;

    /// The four ranges this check caught when it was added were all of this
    /// shape: pointing at syn's rustdoc, which names the item repeatedly, in
    /// the belief that they pointed at the definition below it.
    #[test]
    fn rustdoc_naming_an_item_is_not_declaring_it() {
        let doc = "\
/// Error returned when a Syn parser cannot parse the input tokens.
///
/// # Error reporting
///
/// See the documentation for [`Error::new`].";
        assert!(!declares(doc, "Error"));
    }

    #[test]
    fn a_definition_is_a_declaration() {
        assert!(declares(
            "pub struct Error {\n    messages: Vec<M>,\n}",
            "Error"
        ));
        assert!(declares(
            "#[macro_export]\nmacro_rules! quote_spanned {",
            "quote_spanned"
        ));
        assert!(declares("pub enum Data {", "Data"));
        assert!(declares(
            "pub fn parse2<T>(tokens: TokenStream) -> Result<T> {",
            "parse2"
        ));
        assert!(declares(
            "pub type ParseStream<'a> = &'a ParseBuffer<'a>;",
            "ParseStream"
        ));
    }

    /// `syn::Ident` is one line of re-export, and the one line is the point.
    #[test]
    fn a_re_export_declares() {
        assert!(declares("pub use proc_macro2::Ident;", "Ident"));
    }

    /// A prefix match would let `TokenStream` satisfy an entry for `Token`.
    #[test]
    fn a_longer_name_does_not_satisfy_a_shorter_one() {
        assert!(!declares("pub struct TokenStream {", "Token"));
        assert!(declares("pub struct TokenStream {", "TokenStream"));
    }
}
