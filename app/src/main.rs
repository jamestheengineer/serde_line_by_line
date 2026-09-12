//! Static site generator for serde_line_by_line (decision D3).
//!
//! Renders the annotation store and the pinned source into a directory of
//! plain HTML. Nothing is needed at request time: examples run as WASM and
//! highlighting is precomputed here.
//!
//!   cargo run -p app -- [outdir]     (default: site/)

mod highlight;
mod markdown;

use anyhow::{Context, Result};
use askama::Template;
use highlight::{Highlighter, Line};
use slbl_core::schema::{CourseUnit, DeriveKind, Kind, Supplement, UnitStatus};
use slbl_core::{vendor, Store, Unit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A contiguous run of source lines shown as one row: code on the left,
/// explanation on the right. Unannotated runs become `annotated = false`
/// blocks, so a partially covered file renders honestly instead of hiding the
/// gaps.
struct Block {
    annotated: bool,
    /// Lines elided from a gap preview. Zero when nothing was cut.
    hidden_lines: u32,
    id: String,
    title: String,
    kind: String,
    range_label: String,
    body_html: String,
    code: Vec<Line>,
    features: Vec<String>,
    examples: Vec<String>,
    /// Set on macro-use blocks: the macro's name and a link to the annotation
    /// that explains it. Empty strings mean "not a macro use" — the template
    /// tests them rather than carrying an `Option`.
    expands_name: String,
    expands_href: String,
    /// One row per invocation in a macro-use group. Empty for every other kind.
    uses: Vec<Use>,
    /// Borrowed vocabulary this annotation cites (D9). Rendered as links into
    /// the glossary page, which is where the definition lives — the annotated
    /// crate does not contain it.
    glossary: Vec<GlossLink>,
    /// What this span emits, for a `codegen` annotation that claims something.
    /// Sliced out of a real expansion at build time, never out of the store
    /// (D10), so the page does not build if the claim stops being true.
    emits: Vec<EmitBlock>,
}

/// One invocation inside a macro-use group.
///
/// This is the compression the plan turns on: a run of invocations sharing a
/// definition renders as rows under one heading rather than as one full-height
/// block each. `de/impls.rs` has 106 of them.
struct Use {
    id: String,
    label: String,
    range_label: String,
    body_html: String,
    start: u32,
    end: u32,
}

/// One annotation as it appears on a course-unit page: the reference-track
/// block, plus the context the course ordering needs — where the code lives,
/// and what it assumes that the course has not taught yet.
struct CourseBlock {
    block: Block,
    file: String,
    href: String,
    assumes: Vec<Assumes>,
}

/// A prereq whose own unit comes later in the course. The coverage gate warns
/// about these; the reader gets told too, rather than hitting an explanation
/// that leans on something twelve units away.
struct Assumes {
    title: String,
    unit_title: String,
    href: String,
}

/// A course unit in the sidebar and on the index.
struct NavUnit {
    id: String,
    number: String,
    name: String,
    title: String,
    href: String,
    summary_html: String,
    annotations: usize,
    supplement: String,
    supplement_class: String,
    supplement_note: String,
    planned: bool,
}

struct NavFile {
    path: String,
    href: String,
    short: String,
    lines: u32,
    percent: f64,
    percent_label: String,
    complete: bool,
    /// False for a file no annotation claims yet. It is still listed, with its
    /// zero — that is the progress the sidebar is for — but it gets no link,
    /// because a page of unexplained source is not a page.
    annotated: bool,
}

/// One annotated crate's files in the sidebar.
///
/// The sidebar is grouped since there are two of them (D12): a flat list of 47
/// files across two crates, three of which are called `lib.rs`, tells the
/// reader nothing about where they are.
struct NavGroup {
    name: String,
    version: String,
    percent: f64,
    percent_label: String,
    claimed_lines: u32,
    total_lines: u32,
    files: Vec<NavFile>,
}

#[derive(Template)]
#[template(path = "course.html")]
struct CoursePage {
    page_title: String,
    description: String,
    units: Vec<NavUnit>,
    written: usize,
    total_units: usize,
    course_annotations: usize,
    source_id: String,
    root: String,
    track: String,
}

#[derive(Template)]
#[template(path = "unit.html")]
struct UnitPage {
    page_title: String,
    description: String,
    units: Vec<NavUnit>,
    unit: NavUnit,
    body_html: String,
    blocks: Vec<CourseBlock>,
    features: Vec<String>,
    examples: Vec<String>,
    prereqs: Vec<NavUnit>,
    prev: Vec<NavUnit>,
    next: Vec<NavUnit>,
    source_id: String,
    root: String,
    track: String,
}

/// The 404 page. Standalone — see the comment at the top of the template for
/// why it carries its own styles instead of extending `base.html`.
#[derive(Template)]
#[template(path = "404.html")]
struct NotFoundPage;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    page_title: String,
    description: String,
    nav: Vec<NavGroup>,
    /// One row per annotated crate. There is deliberately no figure spanning
    /// them — see where this is built.
    sources: Vec<SourceSummary>,
    course_units: usize,
    derive: Narrated,
    glossary: Borrowed,
    source_id: String,
    root: String,
    current: String,
    track: String,
}

/// One annotated crate as the front page states it.
struct SourceSummary {
    name: String,
    version: String,
    total_lines: u32,
    claimed_lines: u32,
    percent: f64,
    percent_label: String,
    annotations: usize,
    files: usize,
    complete_files: usize,
}

/// What the derive track came to, counted once and reported in two places: on
/// its own index, and on the front page where the promise is restated.
#[derive(Default)]
struct Narrated {
    units: usize,
    steps: usize,
    cited_lines: u32,
    crossings: usize,
    version: String,
}

/// What the glossary came to. Same reason.
#[derive(Default)]
struct Borrowed {
    entries: usize,
    sources: usize,
    quoted_lines: u32,
}

#[derive(Template)]
#[template(path = "expand.html")]
struct ExpandPage {
    page_title: String,
    description: String,
    cases: Vec<ExpandCase>,
    first_source: String,
    first_expansion: String,
    derive_version: String,
    untouched_lines: u32,
    source_id: String,
    root: String,
    track: String,
}

struct ExpandCase {
    name: String,
    title: String,
    source: String,
    body_html: String,
    code: Vec<Line>,
}

#[derive(serde::Deserialize)]
struct CaseFile {
    #[serde(rename = "case")]
    cases: Vec<CaseMeta>,
}

#[derive(serde::Deserialize)]
struct CaseMeta {
    name: String,
    title: String,
    body: String,
}

#[derive(Template)]
#[template(path = "glossary.html")]
struct GlossaryPage {
    page_title: String,
    description: String,
    groups: Vec<GlossGroup>,
    entries: usize,
    quoted_lines: u32,
    sources: usize,
    borrowed_lines: u32,
    source_id: String,
    root: String,
    track: String,
}

struct GlossGroup {
    crate_name: String,
    version: String,
    anchor: String,
    entries: Vec<GlossEntry>,
}

struct GlossEntry {
    id: String,
    /// The id without its crate prefix, for the sidebar where the crate is
    /// already the heading above it.
    short: String,
    anchor: String,
    title: String,
    file: String,
    lines: String,
    upstream: String,
    body_html: String,
    code: Vec<Line>,
    see_also: Vec<GlossLink>,
}

struct GlossLink {
    id: String,
    anchor: String,
}

/// `syn::DeriveInput` → `syn-DeriveInput`, usable as a URL fragment.
fn gloss_anchor(id: &str) -> String {
    id.replace("::", "-")
}

fn gloss_links(ids: &[String]) -> Vec<GlossLink> {
    ids.iter()
        .map(|id| GlossLink {
            id: id.clone(),
            anchor: gloss_anchor(id),
        })
        .collect()
}

/// A narrative unit in the sidebar and on the track index.
struct NarrativeNav {
    id: String,
    number: String,
    title: String,
    href: String,
    summary_html: String,
    steps: usize,
    crossings: usize,
}

#[derive(Template)]
#[template(path = "narrative.html")]
struct NarrativePage {
    page_title: String,
    description: String,
    units: Vec<NarrativeNav>,
    steps: usize,
    cited_lines: u32,
    crossings: usize,
    derive_version: String,
    derive_lines: u32,
    source_id: String,
    root: String,
    track: String,
}

#[derive(Template)]
#[template(path = "narrative_unit.html")]
struct NarrativeUnitPage {
    page_title: String,
    description: String,
    units: Vec<NarrativeNav>,
    unit: NarrativeNav,
    body_html: String,
    /// The worked input this unit follows, highlighted. Empty when the unit
    /// names no case.
    case_name: String,
    case_code: Vec<Line>,
    steps: Vec<NarrativeStepBlock>,
    prev: Vec<NarrativeNav>,
    next: Vec<NarrativeNav>,
    source_id: String,
    root: String,
    track: String,
}

/// One stop on the walk, as rendered: the cited code, the prose, and — where
/// the step claims one — the generated code that citation produces.
struct NarrativeStepBlock {
    id: String,
    title: String,
    body_html: String,
    /// `serde_derive 1.0.229`, shown on every step because the walk crosses
    /// between two pinned trees and the reader has to know which one they are
    /// looking at.
    origin: String,
    file: String,
    range_label: String,
    code: Vec<Line>,
    glossary: Vec<GlossLink>,
    /// Set on a crossing into the annotated crate: a link to the annotation
    /// that claims these lines.
    annotation_href: String,
    annotation_title: String,
    /// Steps this one leans on, as links within the walk.
    leans_on: Vec<StepLink>,
    /// The emitted code, sliced out of a real expansion.
    emits: Vec<EmitBlock>,
}

struct StepLink {
    title: String,
    href: String,
}

/// Generated code quoted from an expansion produced at build time.
struct EmitBlock {
    derive: String,
    case: String,
    range_label: String,
    code: Vec<Line>,
}

#[derive(Template)]
#[template(path = "file.html")]
struct FilePage {
    page_title: String,
    description: String,
    nav: Vec<NavGroup>,
    /// Which annotated crate this file belongs to. Shown on the page because
    /// `src/lib.rs` is now an ambiguous title (D12).
    source: String,
    file: String,
    percent: f64,
    percent_label: String,
    lines: u32,
    blocks: Vec<Block>,
    source_id: String,
    root: String,
    current: String,
    track: String,
}

fn main() -> Result<()> {
    let repo = repo_root()?;
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo.join("site"));

    vendor::verify(&repo).context("vendor integrity")?;
    let store = slbl_core::load(&repo)?;
    let hl = Highlighter::new();

    if out.exists() {
        std::fs::remove_dir_all(&out).with_context(|| format!("clearing {}", out.display()))?;
    }
    std::fs::create_dir_all(out.join("static"))?;
    std::fs::create_dir_all(out.join("file"))?;
    std::fs::create_dir_all(out.join("course"))?;

    let pages = Pages::new(&store);
    let nav = build_nav(&store, &pages);
    let defs = macro_defs(&store);

    // Every case any claim about generated code needs, expanded once for the
    // whole build — the narrative's units and steps, and now the reference
    // track's codegen annotations. One cache, so the two tracks cannot read
    // different bytes and disagree about what the crate emits.
    let narrative = slbl_core::read_narrative(&repo)?;
    let mut cases: BTreeSet<String> = BTreeSet::new();
    for unit in &narrative {
        cases.extend(unit.unit.expand_case.iter().cloned());
        for step in &unit.steps {
            if step.step.emits.is_some() {
                cases.extend(step.step.emits_case.iter().cloned());
            }
        }
    }
    for unit in store.all_units() {
        if unit.annotation.emits.is_some() {
            cases.extend(unit.annotation.emits_case.iter().cloned());
        }
    }
    let expansions = expansions_for(cases.iter())?;

    // Highlighted once for the whole build: the reference track renders each
    // file on its own page, and the course track pulls spans out of a dozen
    // files into one unit page. Keyed by crate as well as path — two of the
    // annotated files are called `src/lib.rs`.
    let mut highlighted_files: BTreeMap<(String, String), Vec<Line>> = BTreeMap::new();
    for source in &store.sources {
        let root = repo.join("vendor").join(&source.source_id);
        for (file, _) in source.annotated_files() {
            let text = std::fs::read_to_string(root.join(file))
                .with_context(|| format!("reading {file}"))?;
            highlighted_files.insert(
                (source.name.clone(), file.clone()),
                hl.file(&text)
                    .with_context(|| format!("highlighting {file}"))?,
            );
        }
    }

    // A file gets a page once something is written about it. An unannotated
    // file is listed in the sidebar with its zero rather than published as 400
    // lines of source nobody has explained yet.
    let ctx = Render {
        highlighted: &highlighted_files,
        defs: &defs,
        pages: &pages,
        expansions: &expansions,
        hl: &hl,
    };
    let mut file_pages = 0usize;
    for source in &store.sources {
        for (file, lines) in source.annotated_files() {
            let key = (source.name.clone(), file.clone());
            let highlighted = &highlighted_files[&key];
            let units = source.units_for(file);

            let page = FilePage {
                page_title: format!("{file} — serde line by line"),
                description: format!(
                    "{file} from {} {}, annotated line by line: {} lines, {} explanations.",
                    source.name,
                    source.version,
                    lines,
                    units.len(),
                ),
                nav: nav_for(&nav),
                file: file.clone(),
                source: source.name.clone(),
                percent: percent_of(units, *lines),
                percent_label: format!("{:.1}", percent_of(units, *lines)),
                lines: *lines,
                blocks: blocks_for(units, highlighted, &ctx, file)?,
                source_id: source.source_id.clone(),
                root: "../".to_string(),
                current: pages.name(&source.name, file),
                track: "reference".to_string(),
            };
            write(
                &out.join("file").join(pages.name(&source.name, file)),
                &page.render()?,
            )?;
            file_pages += 1;
        }
    }

    write_course(&out, &store, &ctx)?;
    let derive = write_narrative(&out, &repo, &store, &pages, &expansions, &hl)?;
    let glossary = write_glossary(&out, &repo, &store, &hl)?;
    write_expand(&out, &repo, &store, &hl)?;

    // The front page is written last because it restates what every other
    // track claims, and those numbers are counted while the tracks are built
    // rather than kept in a second place that could disagree with them.
    let narrative_units = derive.units;
    let first = store.first();
    let index = IndexPage {
        page_title: "serde line by line".to_string(),
        description: one_line(&format!(
            "Every line of serde_core {} annotated: {} lines claimed by {} explanations, \
             beside the source, with runnable examples — a {}-unit Rust course read out of \
             the same annotations, and a {}-unit walk through what #[derive(Serialize)] \
             expands to.",
            first.version,
            first.total_lines(),
            first.annotations(),
            store.course.len(),
            derive.units,
        )),
        nav: build_nav(&store, &pages),
        // One row per annotated crate, each with its own figure. There is no
        // combined percentage anywhere on the site and this is the reason
        // (D12): the two crates promise the same thing about different amounts
        // of code, and 12,037 of 21,012 would be a number about neither.
        sources: store
            .sources
            .iter()
            .map(|s| SourceSummary {
                name: s.name.clone(),
                version: s.version.clone(),
                total_lines: s.total_lines(),
                claimed_lines: s.claimed_lines(),
                percent: s.percent(),
                percent_label: format!("{:.1}", s.percent()),
                annotations: s.annotations(),
                files: s.files.len(),
                complete_files: s.complete.len(),
            })
            .collect(),
        course_units: store.course.len(),
        derive,
        glossary,
        source_id: first.source_id.clone(),
        root: "./".to_string(),
        current: String::new(),
        track: "reference".to_string(),
    };
    write(&out.join("index.html"), &index.render()?)?;

    // Copied rather than embedded: the playground is a binary artefact, and it
    // may legitimately be absent — the site must build without a wasm
    // toolchain (see `cargo xtask wasm`).
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
        &out.join("static"),
    )?;
    write(&out.join("static").join("syntax.css"), &syntax_css(&hl)?)?;

    // GitHub Pages runs Jekyll over an uploaded tree unless this file exists,
    // which would silently drop anything beginning with an underscore.
    write(&out.join(".nojekyll"), "")?;

    // A shields.io endpoint badge, derived from the store on every build so
    // the README figure cannot drift from the coverage gate. Served from the
    // deployed site; shields fetches it and renders the badge.
    // `badge.json` stays the first source's, so the README badge that has
    // always pointed at it keeps working; the others are named.
    write(&out.join("badge.json"), &badge_json(store.first()))?;
    for source in store.sources.iter().skip(1) {
        write(
            &out.join(format!("badge-{}.json", source.name)),
            &badge_json(source),
        )?;
    }

    // Pages serves this for any unknown path under the site.
    write(&out.join("404.html"), &NotFoundPage.render()?)?;

    let links = check_links(&out)?;

    let claims: Vec<String> = store
        .sources
        .iter()
        .map(|s| format!("{} {:.1}%", s.name, s.percent()))
        .collect();
    println!(
        "wrote {} pages to {}  ({}, {} annotations)",
        // one per annotated file, one per course unit, one per narrative unit,
        // plus the front page, the course index, the derive index, the
        // glossary, the expansion page and the 404.
        file_pages + store.course.len() + narrative_units + 6,
        out.display(),
        claims.join(", "),
        store.annotations(),
    );
    println!("checked {links} internal links");
    Ok(())
}

/// Verifies every internal link in the generated site resolves — to a file
/// that exists and, where the link carries a fragment, to an `id` on that page.
/// Returns how many were checked.
///
/// The coverage gate validates the store; nothing validated the *rendering* of
/// it. A `{{ root }}` dropped from one `href` in a template produces a site
/// that works when served from the domain root and 404s under the `/<repo>/`
/// path Pages serves it from (D6), which is the worst place to find out. This
/// runs on every `cargo site`, so the deploy cannot publish a broken link.
fn check_links(out: &Path) -> Result<usize> {
    let mut pages = Vec::new();
    collect_html(out, &mut pages)?;

    let mut ids: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    for page in &pages {
        let html = std::fs::read_to_string(page)?;
        ids.insert(page.clone(), attr_values(&html, "id=\""));
    }

    let mut checked = 0;
    let mut broken = Vec::new();
    for page in &pages {
        let html = std::fs::read_to_string(page)?;
        let dir = page.parent().context("page has no parent")?;
        let mut refs = attr_values(&html, "href=\"");
        refs.extend(attr_values(&html, "src=\""));
        for r in refs {
            // Out of scope: anything not naming a file in this tree. The 404
            // page deliberately links to `/`, and the favicon is an inline
            // `data:` SVG so the site never requests a file for it.
            if r.starts_with("http")
                || r.starts_with("mailto:")
                || r.starts_with("data:")
                || r.starts_with('/')
            {
                continue;
            }
            checked += 1;
            let (path, fragment) = match r.split_once('#') {
                Some((p, f)) => (p, Some(f)),
                None => (r.as_str(), None),
            };
            let target = if path.is_empty() {
                page.clone()
            } else {
                normalize(&dir.join(path))
            };
            if !target.exists() {
                broken.push(format!("{}: {r} -> missing file", rel(out, page)));
                continue;
            }
            if let Some(f) = fragment {
                let has = ids.get(&target).is_some_and(|v| v.iter().any(|i| i == f));
                if !has {
                    broken.push(format!("{}: {r} -> no such id", rel(out, page)));
                }
            }
        }
    }

    if !broken.is_empty() {
        for b in broken.iter().take(20) {
            eprintln!("  broken link: {b}");
        }
        anyhow::bail!(
            "{} broken internal link(s) in {}",
            broken.len(),
            out.display()
        );
    }
    Ok(checked)
}

/// A summary flattened into one line for a `<meta>` tag: newlines collapsed,
/// cut at a word boundary near 200 characters. Markdown emphasis is left as
/// written — search results and link previews render it as plain text, and
/// stripping it would need a parser for the two asterisks it saves.
fn one_line(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= 200 {
        return flat;
    }
    let cut = flat
        .char_indices()
        .take_while(|(i, _)| *i < 200)
        .map(|(i, _)| i)
        .last()
        .unwrap_or(0);
    let trimmed = flat[..cut]
        .rsplit_once(' ')
        .map_or(&flat[..cut], |(a, _)| a);
    format!("{trimmed}…")
}

/// Every `.html` file under `dir`, recursively.
fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_html(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "html") {
            out.push(path);
        }
    }
    Ok(())
}

/// Values of one double-quoted attribute, found by scanning for `needle`
/// (e.g. `href="`). The generator writes the markup, so there is no
/// hand-authored HTML here to trip a scan this simple.
fn attr_values(html: &str, needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(needle) {
        rest = &rest[at + needle.len()..];
        match rest.find('"') {
            Some(end) => {
                out.push(rest[..end].to_string());
                rest = &rest[end + 1..];
            }
            None => break,
        }
    }
    out
}

/// Resolves `.` and `..` without touching the filesystem, so a link is checked
/// as written rather than as a symlink happens to resolve it.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn rel(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// The course track: an index and one page per unit.
///
/// Both are queries over the same annotation store the reference track renders.
/// Nothing here is a second copy of the content — a unit page is the unit's own
/// framing followed by its annotations, pulled out of up to a dozen files and
/// re-ordered by the prereq graph.
/// The expansion page: `serde_derive` run over a handful of inputs.
///
/// The expansions are computed here, at build time, by the same `expand` crate
/// the tests assert and the wasm module wraps. So the page is right with
/// JavaScript off, and the module only makes it editable.
fn write_expand(out: &Path, repo: &Path, store: &Store, hl: &Highlighter) -> Result<()> {
    let path = repo.join("expand").join("cases.toml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let meta: CaseFile =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;

    let mut cases = Vec::new();
    for m in &meta.cases {
        let source = expand::case(&m.name).with_context(|| {
            format!(
                "cases.toml describes {:?}, which is not in expand/cases/",
                m.name
            )
        })?;
        cases.push(ExpandCase {
            name: m.name.clone(),
            title: m.title.clone(),
            source: source.trim_end().to_string(),
            body_html: markdown::render(&m.body),
            code: hl
                .file(source)
                .with_context(|| format!("highlighting case {}", m.name))?,
        });
    }
    // The other direction: a case with no prose would render as an unlabelled
    // block, which is the kind of thing nobody notices for six months.
    for (name, _) in expand::CASES {
        anyhow::ensure!(
            meta.cases.iter().any(|m| m.name == *name),
            "expand/cases/{name}.rs has no entry in expand/cases.toml"
        );
    }

    let first = cases.first().context("expand/cases.toml is empty")?;
    let first_source = first.source.clone();
    let first_expansion = expand::expand(&first_source, expand::Derive::Serialize);

    let derive_source = vendor::load_pin(repo)?.get("serde_derive")?.clone();
    let derive_root = derive_source.dir(repo);
    let untouched_lines = vendor::source_files_in(&derive_root)?
        .iter()
        .filter(|f| *f != "src/lib.rs")
        .filter_map(|f| vendor::line_count_in(&derive_root, f).ok())
        .sum();

    let page = ExpandPage {
        page_title: "What #[derive(Serialize)] turns into — serde line by line".to_string(),
        description: format!(
            "serde_derive {} expanding {} worked examples, live in the browser.",
            derive_source.version,
            cases.len()
        ),
        cases,
        first_source,
        first_expansion,
        derive_version: derive_source.version.clone(),
        untouched_lines,
        source_id: store.first().source_id.clone(),
        root: "../".to_string(),
        track: "expand".to_string(),
    };
    let dir = out.join("expand");
    std::fs::create_dir_all(&dir)?;
    write(&dir.join("index.html"), &page.render()?)?;
    Ok(())
}

/// The narrative track (PLAN.md §11): `#[derive(Serialize)]` followed from a
/// `DeriveInput` to an emitted `impl`, one unit per stage.
///
/// The walk crosses two pinned trees, so a step carries its origin rather than
/// inheriting one from the page, and a step that lands in `serde_core` links to
/// the reference-track annotation that claims those lines instead of explaining
/// them a second time.
///
/// Where a step quotes generated code, the quotation in the store is used only
/// to *locate* it: the lines rendered are sliced out of a real expansion
/// computed here by the same `expand` crate the tests assert. A claim that no
/// longer matches what `serde_derive` emits fails this build naming the step,
/// which is the only way a sentence about generated code can stay true across a
/// version bump.
fn write_narrative(
    out: &Path,
    repo: &Path,
    store: &Store,
    pages: &Pages,
    expansions: &Expansions,
    hl: &Highlighter,
) -> Result<Narrated> {
    let units = slbl_core::read_narrative(repo)?;
    if units.is_empty() {
        return Ok(Narrated::default());
    }
    let by_id: BTreeMap<&str, &Unit> = store
        .all_units()
        .map(|u| (u.annotation.id.as_str(), u))
        .collect();
    let titles: BTreeMap<&str, &str> = units
        .iter()
        .flat_map(|u| u.steps.iter())
        .map(|s| (s.step.id.as_str(), s.step.title.as_str()))
        .collect();
    let unit_of: BTreeMap<&str, &str> = units
        .iter()
        .flat_map(|u| {
            u.steps
                .iter()
                .map(|s| (s.step.id.as_str(), u.unit.id.as_str()))
        })
        .collect();

    let nav: Vec<NarrativeNav> = units
        .iter()
        .map(|u| NarrativeNav {
            id: u.unit.id.clone(),
            number: u
                .unit
                .id
                .split_once('-')
                .map_or(String::new(), |(n, _)| n.to_string()),
            title: u.unit.title.clone(),
            href: format!("{}.html", u.unit.id),
            summary_html: markdown::render(&u.unit.summary),
            steps: u.steps.len(),
            crossings: u.steps.iter().filter(|s| s.is_coverage).count(),
        })
        .collect();

    let derive_source = vendor::load_pin(repo)?.get("serde_derive")?.clone();
    let derive_root = derive_source.dir(repo);
    let derive_lines: u32 = vendor::source_files_in(&derive_root)?
        .iter()
        .filter_map(|f| vendor::line_count_in(&derive_root, f).ok())
        .sum();

    let dir = out.join("derive");
    std::fs::create_dir_all(&dir)?;

    let stats = Narrated {
        units: units.len(),
        steps: nav.iter().map(|u| u.steps).sum(),
        cited_lines: units
            .iter()
            .flat_map(|u| u.steps.iter())
            .map(|s| s.range.line_count())
            .sum(),
        crossings: nav.iter().map(|u| u.crossings).sum(),
        version: derive_source.version.clone(),
    };

    let index = NarrativePage {
        page_title: "The derive track — serde line by line".to_string(),
        description: one_line(&format!(
            "One struct and one enum followed through serde_derive {}, from the tokens the \
             compiler hands over to the impl it gets back — {} units citing the pinned source \
             as the path goes through it.",
            derive_source.version,
            units.len()
        )),
        units: nav.iter().map(clone_nnav).collect(),
        steps: stats.steps,
        cited_lines: stats.cited_lines,
        crossings: stats.crossings,
        derive_version: derive_source.version.clone(),
        derive_lines,
        source_id: store.first().source_id.clone(),
        root: "../".to_string(),
        track: "derive".to_string(),
    };
    write(&dir.join("index.html"), &index.render()?)?;

    for (i, unit) in units.iter().enumerate() {
        let mut steps = Vec::new();
        for item in &unit.steps {
            let s = &item.step;
            let mut code = hl
                .file(&item.quoted)
                .with_context(|| format!("highlighting {}", s.id))?;
            for (offset, line) in code.iter_mut().enumerate() {
                line.number = item.range.start + offset as u32;
            }

            let (annotation_href, annotation_title) = match s.annotation.as_deref() {
                Some(id) => match by_id.get(id) {
                    Some(u) => (
                        format!("{}#{id}", pages.unit(u)),
                        u.annotation.title.clone(),
                    ),
                    None => (String::new(), String::new()),
                },
                None => (String::new(), String::new()),
            };

            let mut emits = Vec::new();
            let case = s.emits_case.as_ref().or(unit.unit.expand_case.as_ref());
            if let (Some(snippet), Some(kind), Some(case)) = (&s.emits, s.emits_from, case) {
                emits.push(emit_block(expansions, hl, &s.id, snippet, kind, case)?);
            }

            steps.push(NarrativeStepBlock {
                id: s.id.clone(),
                title: s.title.clone(),
                body_html: markdown::render(&s.body),
                origin: format!("{} {}", item.crate_name, item.version),
                file: s.file.clone(),
                range_label: label(item.range.start, item.range.end),
                code,
                glossary: gloss_links(&s.glossary),
                annotation_href,
                annotation_title,
                leans_on: s
                    .leans_on
                    .iter()
                    .filter_map(|p| {
                        let title = titles.get(p.as_str())?;
                        let unit_id = unit_of.get(p.as_str())?;
                        Some(StepLink {
                            title: (*title).to_string(),
                            href: format!("{unit_id}.html#{p}"),
                        })
                    })
                    .collect(),
                emits,
            });
        }

        let (case_name, case_code) = match &unit.unit.expand_case {
            Some(case) => {
                let source = expand::case(case).unwrap_or_default();
                (
                    case.clone(),
                    hl.file(source.trim_end())
                        .with_context(|| format!("highlighting case {case}"))?,
                )
            }
            None => (String::new(), Vec::new()),
        };

        let page = NarrativeUnitPage {
            page_title: format!("{} — serde line by line", unit.unit.title),
            description: one_line(&unit.unit.summary),
            units: nav.iter().map(clone_nnav).collect(),
            unit: clone_nnav(&nav[i]),
            body_html: markdown::render(&unit.unit.body),
            case_name,
            case_code,
            steps,
            prev: nav
                .get(i.wrapping_sub(1))
                .map(clone_nnav)
                .into_iter()
                .collect(),
            next: nav.get(i + 1).map(clone_nnav).into_iter().collect(),
            source_id: store.first().source_id.clone(),
            root: "../".to_string(),
            track: "derive".to_string(),
        };
        write(&dir.join(format!("{}.html", unit.unit.id)), &page.render()?)?;
    }
    Ok(stats)
}

fn clone_nnav(u: &NarrativeNav) -> NarrativeNav {
    NarrativeNav {
        id: u.id.clone(),
        number: u.number.clone(),
        title: u.title.clone(),
        href: u.href.clone(),
        summary_html: u.summary_html.clone(),
        steps: u.steps,
        crossings: u.crossings,
    }
}

/// Every expansion the build needs, computed once.
///
/// Both tracks quote generated code and neither stores it (D10). Ten claims
/// against the same case must not run `serde_derive` ten times, and the
/// reference track and the narrative must be reading the same bytes — if they
/// expanded separately, a claim could pass on one page and fail on the other.
type Expansions = BTreeMap<(String, DeriveKind), String>;

fn expansions_for<'a>(cases: impl Iterator<Item = &'a String>) -> Result<Expansions> {
    let mut out = Expansions::new();
    for case in cases {
        let source =
            expand::case(case).with_context(|| format!("no such expansion case {case:?}"))?;
        for (kind, derive) in [
            (DeriveKind::Serialize, expand::Derive::Serialize),
            (DeriveKind::Deserialize, expand::Derive::Deserialize),
        ] {
            out.entry((case.clone(), kind))
                .or_insert_with(|| expand::expand(source, derive));
        }
    }
    Ok(out)
}

/// One claim about generated code, rendered from the expansion rather than
/// from the claim.
///
/// `who` names whatever made the claim — an annotation id or a step id — and
/// appears in the error if the crate has stopped emitting it, which is the
/// whole value of doing this at build time.
fn emit_block(
    expansions: &Expansions,
    hl: &Highlighter,
    who: &str,
    snippet: &str,
    kind: DeriveKind,
    case: &str,
) -> Result<EmitBlock> {
    let expansion = expansions
        .get(&(case.to_string(), kind))
        .with_context(|| format!("{who}: no expansion computed for case {case:?}"))?;
    let (start, len) = locate(expansion, snippet).with_context(|| {
        format!(
            "{who}: the code it says serde_derive emits is not in the {} expansion of {case} \
             — the store's claim and the crate's output have parted company",
            kind.label()
        )
    })?;
    let text: Vec<&str> = expansion.lines().collect();
    let block = text[start..start + len].join("\n");
    let mut lines = hl
        .file(&block)
        .with_context(|| format!("highlighting the expansion at {who}"))?;
    for (offset, line) in lines.iter_mut().enumerate() {
        line.number = (start + offset + 1) as u32;
    }
    Ok(EmitBlock {
        derive: kind.label().to_string(),
        case: case.to_string(),
        range_label: label(start as u32 + 1, (start + len) as u32),
        code: lines,
    })
}

/// Finds `snippet` in `text` as a contiguous run of lines, returning
/// `(first line index, line count)`.
///
/// Lines are compared with surrounding whitespace stripped, and blank lines at
/// the ends of the snippet are ignored. Generated code is reindented whenever
/// anything above it changes shape, and a claim about what `serde_derive` emits
/// should not break because a block moved one level deeper. A change in the
/// tokens themselves still breaks it, which is the point.
fn locate(text: &str, snippet: &str) -> Option<(usize, usize)> {
    let want: Vec<&str> = snippet
        .lines()
        .map(str::trim)
        .skip_while(|l| l.is_empty())
        .collect();
    let want: Vec<&str> = {
        let mut w = want;
        while w.last().is_some_and(|l| l.is_empty()) {
            w.pop();
        }
        w
    };
    if want.is_empty() {
        return None;
    }
    let have: Vec<&str> = text.lines().map(str::trim).collect();
    have.windows(want.len())
        .position(|w| w == want.as_slice())
        .map(|at| (at, want.len()))
}

/// The borrowed-vocabulary page (D9).
///
/// One page rather than one per entry: a glossary is read by jumping into it,
/// and 62 pages of six paragraphs each would be 62 navigations for a reader
/// who wanted to compare two of them. The quoted code is read from the pinned
/// tree here, not from the store, so what the site shows is what the pin
/// protects.
fn write_glossary(out: &Path, repo: &Path, store: &Store, hl: &Highlighter) -> Result<Borrowed> {
    let items = slbl_core::read_glossary(repo)?;
    if items.is_empty() {
        return Ok(Borrowed::default());
    }
    let pin = vendor::load_pin(repo)?;

    let mut groups: Vec<GlossGroup> = Vec::new();
    let mut quoted_lines = 0u32;
    for source in pin.by_role(vendor::Role::Glossary) {
        let mine: Vec<_> = items
            .iter()
            .filter(|i| i.source_id == source.source_id())
            .collect();
        if mine.is_empty() {
            continue;
        }
        let mut entries = Vec::new();
        for item in mine {
            quoted_lines += item.range.line_count();
            // Highlighting the quoted span alone, then renumbering to where it
            // sits in the file: the reader can check the citation by opening
            // the crate at that line.
            let mut code = hl
                .file(&item.quoted)
                .with_context(|| format!("highlighting {}", item.entry.id))?;
            for (offset, line) in code.iter_mut().enumerate() {
                line.number = item.range.start + offset as u32;
            }
            let short = item
                .entry
                .id
                .rsplit("::")
                .next()
                .unwrap_or(&item.entry.id)
                .to_string();
            entries.push(GlossEntry {
                id: item.entry.id.clone(),
                short,
                anchor: gloss_anchor(&item.entry.id),
                title: item.entry.title.clone(),
                file: item.entry.file.clone(),
                lines: item.entry.lines.clone(),
                upstream: format!(
                    "https://docs.rs/{}/{}/{}",
                    source.name,
                    source.version,
                    source.name.replace('-', "_")
                ),
                body_html: markdown::render(&item.entry.body),
                code,
                see_also: item
                    .entry
                    .see_also
                    .iter()
                    .map(|id| GlossLink {
                        id: id.clone(),
                        anchor: gloss_anchor(id),
                    })
                    .collect(),
            });
        }
        groups.push(GlossGroup {
            crate_name: source.name.clone(),
            version: source.version.clone(),
            anchor: gloss_anchor(&source.name),
            entries,
        });
    }

    let borrowed_lines: u32 = pin
        .by_role(vendor::Role::Glossary)
        .map(|s| {
            let root = s.dir(repo);
            vendor::source_files_in(&root)
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|f| vendor::line_count_in(&root, f).ok())
                        .sum::<u32>()
                })
                .unwrap_or(0)
        })
        .sum();

    let entries = groups.iter().map(|g| g.entries.len()).sum();
    let stats = Borrowed {
        entries,
        sources: groups.len(),
        quoted_lines,
    };
    let page = GlossaryPage {
        page_title: "Borrowed vocabulary — serde line by line".to_string(),
        description: format!(
            "The {entries} types serde_derive reads but does not define, quoted from \
             pinned copies of syn, quote and proc-macro2."
        ),
        entries,
        quoted_lines,
        sources: groups.len(),
        groups,
        borrowed_lines,
        source_id: store.first().source_id.clone(),
        root: "../".to_string(),
        track: "glossary".to_string(),
    };
    let dir = out.join("glossary");
    std::fs::create_dir_all(&dir)?;
    write(&dir.join("index.html"), &page.render()?)?;
    Ok(stats)
}

fn write_course(out: &Path, store: &Store, ctx: &Render<'_>) -> Result<()> {
    let nav: Vec<NavUnit> = store
        .course
        .iter()
        .map(|u| nav_unit(u, store.course_annotations(&u.id).len()))
        .collect();

    let index = CoursePage {
        page_title: "Course track — serde line by line".to_string(),
        description: one_line(
            "A Rust course read out of serde_core: 14 units in teaching order, from why a \
             data model exists to the macros that generate a third of the crate. What it \
             cannot teach is labelled as supplementary.",
        ),
        units: nav.iter().map(clone_unit).collect(),
        written: store
            .course
            .iter()
            .filter(|u| u.status == UnitStatus::Written)
            .count(),
        total_units: store.course.len(),
        course_annotations: nav.iter().map(|u| u.annotations).sum(),
        source_id: store.first().source_id.clone(),
        root: "../".to_string(),
        track: "course".to_string(),
    };
    write(&out.join("course").join("index.html"), &index.render()?)?;

    // Which unit each annotation belongs to, and how far into the course that
    // unit is — the two facts a forward-reference warning needs.
    let placement: BTreeMap<&str, usize> = store
        .all_units()
        .filter_map(|u| {
            let unit = u.annotation.course_unit.as_deref()?;
            let at = store.course.iter().position(|c| c.id == unit)?;
            Some((u.annotation.id.as_str(), at))
        })
        .collect();
    let titles: BTreeMap<&str, &str> = store
        .all_units()
        .map(|u| (u.annotation.id.as_str(), u.annotation.title.as_str()))
        .collect();

    for (i, unit) in store.course.iter().enumerate() {
        let annotations = store.course_annotations(&unit.id);
        let blocks: Vec<CourseBlock> = annotations
            .iter()
            .map(|u| course_block(u, ctx, i, &placement, &titles, &store.course))
            .collect::<Result<_>>()?;

        let page = UnitPage {
            page_title: format!("{} — serde line by line", unit.title),
            description: one_line(&unit.summary),
            units: nav.iter().map(clone_unit).collect(),
            unit: clone_unit(&nav[i]),
            body_html: markdown::render(&unit.body),
            features: unit.rust_features.clone(),
            examples: unit.examples.clone(),
            prereqs: unit
                .prereqs
                .iter()
                .filter_map(|p| nav.iter().find(|n| &n.id == p).map(clone_unit))
                .collect(),
            // Askama has no `Option` conditional as clean as an empty list, and
            // the first and last units are the only ones that need one.
            prev: nav
                .get(i.wrapping_sub(1))
                .map(clone_unit)
                .into_iter()
                .collect(),
            next: nav.get(i + 1).map(clone_unit).into_iter().collect(),
            blocks,
            source_id: store.first().source_id.clone(),
            root: "../".to_string(),
            track: "course".to_string(),
        };
        write(
            &out.join("course").join(format!("{}.html", unit.id)),
            &page.render()?,
        )?;
    }
    Ok(())
}

fn nav_unit(unit: &CourseUnit, annotations: usize) -> NavUnit {
    let (number, name) = match unit.id.split_once('-') {
        Some((n, rest)) => (n.to_string(), rest.to_string()),
        None => (String::new(), unit.id.clone()),
    };
    let (supplement, supplement_note) = match unit.supplement {
        Supplement::None => (
            "from serde_core",
            "Every part of this unit is drawn from the crate.",
        ),
        Supplement::Partial => (
            "part supplementary",
            "serde_core shows part of this. The rest is written material, labelled as such.",
        ),
        Supplement::Full => (
            "supplementary",
            "serde_core does not exercise this. The whole unit is written from scratch \
             rather than pretending the crate teaches it.",
        ),
    };
    NavUnit {
        supplement_class: format!("b-{}", supplement.replace(' ', "-")),
        number,
        name,
        title: unit.title.clone(),
        href: format!("{}.html", unit.id),
        summary_html: markdown::render(&unit.summary),
        annotations,
        supplement: supplement.to_string(),
        supplement_note: supplement_note.to_string(),
        planned: unit.status == UnitStatus::Planned,
        id: unit.id.clone(),
    }
}

fn clone_unit(u: &NavUnit) -> NavUnit {
    NavUnit {
        id: u.id.clone(),
        number: u.number.clone(),
        name: u.name.clone(),
        title: u.title.clone(),
        href: u.href.clone(),
        summary_html: u.summary_html.clone(),
        annotations: u.annotations,
        supplement: u.supplement.clone(),
        supplement_class: u.supplement_class.clone(),
        supplement_note: u.supplement_note.clone(),
        planned: u.planned,
    }
}

/// What every block renderer needs and none of them owns: the highlighted
/// source for each annotated crate, the macro-definition index, and how a
/// `(source, file)` pair becomes a URL.
struct Render<'a> {
    highlighted: &'a BTreeMap<(String, String), Vec<Line>>,
    defs: &'a BTreeMap<String, MacroDef>,
    pages: &'a Pages,
    expansions: &'a Expansions,
    hl: &'a Highlighter,
}

fn course_block(
    unit: &Unit,
    ctx: &Render<'_>,
    position: usize,
    placement: &BTreeMap<&str, usize>,
    titles: &BTreeMap<&str, &str>,
    course: &[CourseUnit],
) -> Result<CourseBlock> {
    let a = &unit.annotation;
    let file = a.file.clone();
    let lines = ctx
        .highlighted
        .get(&(unit.source.clone(), file.clone()))
        .map_or(&[][..], Vec::as_slice);
    let mut block = annotated(unit, lines, ctx)?;

    // Course pages never merge macro uses — a unit picks out individual
    // invocations from across a file, so there is no contiguous run to merge —
    // but the link back to the definition still has to work.
    if a.kind == Kind::MacroUse {
        if let Some(d) = a.macro_def.as_deref().and_then(|id| ctx.defs.get(id)) {
            block.expands_name = d.name.clone();
            block.expands_href = format!(
                "{}#{}",
                ctx.pages.name(&d.source, &d.file),
                a.macro_def.as_deref().unwrap_or_default()
            );
        }
    }

    let assumes = a
        .prereqs
        .iter()
        .chain(a.macro_def.iter())
        .filter_map(|p| {
            let at = *placement.get(p.as_str())?;
            (at > position).then(|| Assumes {
                title: titles.get(p.as_str()).copied().unwrap_or(p).to_string(),
                unit_title: course[at].title.clone(),
                href: format!("{}.html#{}", course[at].id, p),
            })
        })
        .collect();

    Ok(CourseBlock {
        href: format!("{}#{}", ctx.pages.unit(unit), a.id),
        file,
        block,
        assumes,
    })
}

/// Where a `macro-def` annotation lives, and what it defines.
struct MacroDef {
    source: String,
    file: String,
    name: String,
}

/// Every `macro-def` annotation, by id.
///
/// Built once for the whole store rather than per file, because a definition
/// and its uses need not live in the same file — `macros.rs` defines three that
/// are invoked thirty times elsewhere.
fn macro_defs(store: &Store) -> BTreeMap<String, MacroDef> {
    store
        .all_units()
        .filter(|u| u.annotation.kind == Kind::MacroDef)
        .map(|u| {
            let a = &u.annotation;
            (
                a.id.clone(),
                MacroDef {
                    source: u.source.clone(),
                    file: a.file.clone(),
                    name: macro_name(&a.title),
                },
            )
        })
        .collect()
}

/// `primitive_impl! — one impl, sixteen times` -> `primitive_impl!`
///
/// Macro-definition titles lead with the macro's name by convention
/// (`docs/annotation-style.md`). A title that does not follow the convention
/// falls back to itself, which reads oddly but never renders as nothing.
fn macro_name(title: &str) -> String {
    match title.split_whitespace().next() {
        Some(first) if first.ends_with('!') => first.to_string(),
        _ => title.to_string(),
    }
}

/// Splits a file into alternating annotated and unannotated blocks.
///
/// Adjacent `macro-use` annotations sharing one definition are merged into a
/// single block whose rows are the individual invocations. Without that, the
/// sixteen `primitive_impl!` calls would be sixteen full-height blocks saying
/// almost the same thing, and `de/impls.rs` would be 106 of them.
fn blocks_for(
    units: &[Unit],
    highlighted: &[Line],
    ctx: &Render<'_>,
    file: &str,
) -> Result<Vec<Block>> {
    let total = highlighted.len() as u32;
    let mut blocks = Vec::new();
    let mut cursor = 1u32;
    let mut i = 0;

    while i < units.len() {
        let unit = &units[i];
        if unit.range.start > cursor {
            blocks.push(gap(cursor, unit.range.start - 1, highlighted));
        }

        let run = macro_run(units, i);
        blocks.push(if run > 1 || unit.annotation.kind == Kind::MacroUse {
            macro_block(&units[i..i + run], highlighted, ctx.defs, ctx.pages, file)
        } else {
            annotated(unit, highlighted, ctx)?
        });

        cursor = cursor.max(units[i + run - 1].range.end + 1);
        i += run;
    }
    if cursor <= total {
        blocks.push(gap(cursor, total, highlighted));
    }
    Ok(blocks)
}

/// How many units starting at `i` belong in one macro-use block: 1 for anything
/// that is not a macro use, otherwise the run of uses that share a definition
/// *and* are contiguous in the source. Contiguity matters — merging across a
/// hole would hide unannotated lines, which is the one thing this project
/// cannot do.
fn macro_run(units: &[Unit], i: usize) -> usize {
    let first = &units[i].annotation;
    if first.kind != Kind::MacroUse {
        return 1;
    }
    let mut n = 1;
    while i + n < units.len() {
        let next = &units[i + n];
        if next.annotation.kind != Kind::MacroUse
            || next.annotation.macro_def != first.macro_def
            || next.range.start != units[i + n - 1].range.end + 1
        {
            break;
        }
        n += 1;
    }
    n
}

fn annotated(unit: &Unit, highlighted: &[Line], ctx: &Render<'_>) -> Result<Block> {
    let a = &unit.annotation;
    // A codegen annotation's claim about what its span emits, sliced out of a
    // real expansion (D10). Checked here rather than in the coverage gate for
    // the same reason the narrative's is: the gate has no business compiling
    // `serde_derive`, and this build already has the expansion in hand.
    let mut emits = Vec::new();
    if let (Some(snippet), Some(kind), Some(case)) = (&a.emits, a.emits_from, &a.emits_case) {
        emits.push(emit_block(
            ctx.expansions,
            ctx.hl,
            &a.id,
            snippet,
            kind,
            case,
        )?);
    }
    Ok(Block {
        annotated: true,
        hidden_lines: 0,
        id: a.id.clone(),
        title: a.title.clone(),
        kind: kind_label(&format!("{:?}", a.kind)),
        range_label: label(unit.range.start, unit.range.end),
        body_html: markdown::render(&a.body),
        code: slice(highlighted, unit.range.start, unit.range.end),
        features: a.rust_features.clone(),
        examples: a.examples.clone(),
        expands_name: String::new(),
        expands_href: String::new(),
        uses: Vec::new(),
        glossary: gloss_links(&a.glossary),
        emits,
    })
}

fn macro_block(
    units: &[Unit],
    highlighted: &[Line],
    defs: &BTreeMap<String, MacroDef>,
    pages: &Pages,
    file: &str,
) -> Block {
    let start = units[0].range.start;
    let end = units[units.len() - 1].range.end;
    let def_id = units[0].annotation.macro_def.as_deref().unwrap_or_default();
    let def = defs.get(def_id);

    // A definition in another file is linked by page; one in this file by
    // anchor, so the reader is not sent on a round trip to land two screens up.
    let (name, href) = match def {
        Some(d) if d.file == file && d.source == units[0].source => {
            (d.name.clone(), format!("#{def_id}"))
        }
        Some(d) => (
            d.name.clone(),
            format!("{}#{def_id}", pages.name(&d.source, &d.file)),
        ),
        None => (String::new(), String::new()),
    };

    let uses: Vec<Use> = units
        .iter()
        .map(|u| Use {
            id: u.annotation.id.clone(),
            label: u.annotation.title.clone(),
            range_label: label(u.range.start, u.range.end),
            body_html: markdown::render(&u.annotation.body),
            start: u.range.start,
            end: u.range.end,
        })
        .collect();

    let kind = if units.len() > 1 {
        format!("macro use ×{}", units.len())
    } else {
        "macro use".to_string()
    };

    Block {
        annotated: true,
        hidden_lines: 0,
        // Not the first annotation's id: every row carries its own, so a link
        // to one invocation lands on that row rather than on the group.
        id: format!("uses-L{start}"),
        title: if name.is_empty() {
            "macro invocations".to_string()
        } else {
            name.clone()
        },
        kind,
        range_label: label(start, end),
        body_html: String::new(),
        code: slice(highlighted, start, end),
        features: union(units, |a| &a.rust_features),
        examples: union(units, |a| &a.examples),
        expands_name: name,
        expands_href: href,
        glossary: Vec::new(),
        uses,
        emits: Vec::new(),
    }
}

/// Collects one list field across a group, de-duplicated, first-seen order.
fn union(
    units: &[Unit],
    field: impl Fn(&slbl_core::schema::Annotation) -> &Vec<String>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in units {
        for v in field(&u.annotation) {
            if !out.iter().any(|seen| seen == v) {
                out.push(v.clone());
            }
        }
    }
    out
}

/// Unannotated runs are previewed rather than rendered in full.
///
/// Without this, a file with no annotations renders as one block containing
/// every line — 2.1 MB for `de/impls.rs`, which is exactly the whole-file cost
/// decision D2 set out to avoid. The unexplained source is the work remaining,
/// not the product, so a few lines and an honest count are enough.
const GAP_PREVIEW_LINES: u32 = 6;

fn gap(start: u32, end: u32, highlighted: &[Line]) -> Block {
    let total = end - start + 1;
    let shown_end = end.min(start + GAP_PREVIEW_LINES - 1);
    let hidden = total - (shown_end - start + 1);
    Block {
        annotated: false,
        hidden_lines: hidden,
        id: format!("L{start}"),
        title: format!("{total} lines not yet explained"),
        kind: "gap".to_string(),
        range_label: label(start, end),
        body_html: String::new(),
        code: slice(highlighted, start, shown_end),
        features: Vec::new(),
        examples: Vec::new(),
        expands_name: String::new(),
        expands_href: String::new(),
        uses: Vec::new(),
        glossary: Vec::new(),
        emits: Vec::new(),
    }
}

fn slice(highlighted: &[Line], start: u32, end: u32) -> Vec<Line> {
    let lo = (start as usize).saturating_sub(1);
    let hi = (end as usize).min(highlighted.len());
    highlighted.get(lo..hi).unwrap_or(&[]).to_vec()
}

fn label(start: u32, end: u32) -> String {
    if start == end {
        format!("line {start}")
    } else {
        format!("lines {start}\u{2013}{end}")
    }
}

/// `TraitItem` -> `trait item`
fn kind_label(debug_name: &str) -> String {
    let mut out = String::new();
    for (i, c) in debug_name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

fn percent_of(units: &[Unit], lines: u32) -> f64 {
    if lines == 0 {
        return 100.0;
    }
    let claimed: u32 = units.iter().map(|u| u.range.line_count()).sum();
    claimed.min(lines) as f64 * 100.0 / lines as f64
}

fn build_nav(store: &Store, pages: &Pages) -> Vec<NavGroup> {
    store
        .sources
        .iter()
        .map(|s| NavGroup {
            name: s.name.clone(),
            version: s.version.clone(),
            percent: s.percent(),
            percent_label: format!("{:.1}", s.percent()),
            claimed_lines: s.claimed_lines(),
            total_lines: s.total_lines(),
            files: s
                .files
                .iter()
                .map(|(path, lines)| {
                    let pct = percent_of(s.units_for(path), *lines);
                    NavFile {
                        href: pages.name(&s.name, path),
                        short: path.strip_prefix("src/").unwrap_or(path).to_string(),
                        percent: pct,
                        percent_label: format!("{:.0}", pct),
                        complete: s.complete.iter().any(|c| c == path),
                        annotated: !s.units_for(path).is_empty(),
                        path: path.clone(),
                        lines: *lines,
                    }
                })
                .collect(),
        })
        .collect()
}

fn nav_for(nav: &[NavGroup]) -> Vec<NavGroup> {
    nav.iter()
        .map(|g| NavGroup {
            name: g.name.clone(),
            version: g.version.clone(),
            percent: g.percent,
            percent_label: g.percent_label.clone(),
            claimed_lines: g.claimed_lines,
            total_lines: g.total_lines,
            files: g
                .files
                .iter()
                .map(|n| NavFile {
                    path: n.path.clone(),
                    href: n.href.clone(),
                    short: n.short.clone(),
                    lines: n.lines,
                    percent: n.percent,
                    percent_label: n.percent_label.clone(),
                    complete: n.complete,
                    annotated: n.annotated,
                })
                .collect(),
        })
        .collect()
}

/// How a `(source, file)` pair becomes a page name.
///
/// Built once from the store. The first annotated source keeps the
/// unqualified names the site has always served — `src-ser-mod.rs.html` — and
/// every other source is prefixed with its crate name. Both crates have a
/// `src/lib.rs`, so something has to distinguish them; prefixing the newer one
/// means the second reference track arrives without breaking a URL that is
/// already published (D12).
struct Pages {
    first: String,
}

impl Pages {
    fn new(store: &Store) -> Self {
        Pages {
            first: store.first().name.clone(),
        }
    }

    /// `("serde_core", "src/ser/mod.rs")` -> `src-ser-mod.rs.html`
    /// `("serde_derive", "src/ser.rs")`   -> `serde_derive-src-ser.rs.html`
    fn name(&self, source: &str, file: &str) -> String {
        let flat = file.replace('/', "-");
        if source == self.first {
            format!("{flat}.html")
        } else {
            format!("{source}-{flat}.html")
        }
    }

    fn unit(&self, unit: &Unit) -> String {
        self.name(&unit.source, &unit.annotation.file)
    }
}

/// Light theme at top level; dark scoped so an explicit toggle wins in both
/// directions and the system default still works with no attribute set.
fn syntax_css(hl: &Highlighter) -> Result<String> {
    let light = hl.theme_css("InspiredGitHub")?;
    let dark = hl.theme_css("base16-ocean.dark")?;
    Ok(format!(
        "/* generated by syntect at build time — do not edit */\n\
         {light}\n\
         @media (prefers-color-scheme: dark) {{\n  :root:not([data-theme=\"light\"]) {{\n{dark}\n  }}\n}}\n\
         :root[data-theme=\"dark\"] {{\n{dark}\n}}\n"
    ))
}

/// One annotated crate's coverage as a shields.io endpoint payload.
///
/// Green only at 100%: the promise this project makes is "every line", and a
/// badge that reads healthy at 97% would be advertising the wrong thing. One
/// file per source, never a combined badge, for the reason on the front page
/// (D12) — a badge is the shortest place a misleading number can hide.
fn badge_json(source: &slbl_core::SourceStore) -> String {
    let percent = source.percent();
    let colour = if percent >= 100.0 {
        "brightgreen"
    } else if percent >= 90.0 {
        "yellow"
    } else {
        "orange"
    };
    format!(
        "{{\"schemaVersion\":1,\"label\":\"{} annotated\",\
         \"message\":\"{percent:.1}%\",\"color\":\"{colour}\"}}\n",
        source.name
    )
}

/// Recursively copies `from` into `to`, creating directories as needed.
fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_dir(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)
                .with_context(|| format!("copying {} to {}", src.display(), dst.display()))?;
        }
    }
    Ok(())
}

fn write(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
}

fn repo_root() -> Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest
        .parent()
        .context("app has no parent directory")?
        .to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::locate;

    /// The point of matching on trimmed lines: `prettyplease` reindents
    /// generated code whenever anything enclosing it changes shape, and a
    /// claim about what `serde_derive` emits should survive that.
    #[test]
    fn indentation_does_not_affect_a_match() {
        let expansion = "fn main() {\n    if x {\n        go();\n    }\n}\n";
        let snippet = "if x {\n    go();\n}";
        assert_eq!(locate(expansion, snippet), Some((1, 3)));
    }

    /// …and the other half: a change to the tokens themselves must fail, or
    /// the check would be decorative.
    #[test]
    fn a_changed_token_does_not_match() {
        let expansion = "fn main() {\n    go_away();\n}\n";
        assert_eq!(locate(expansion, "go();"), None);
    }

    /// Blank lines around a `"""…"""` block in the store are an artifact of
    /// writing toml, not part of the claim.
    #[test]
    fn surrounding_blank_lines_are_ignored() {
        let expansion = "a;\nb;\nc;\n";
        assert_eq!(locate(expansion, "\n\nb;\nc;\n\n"), Some((1, 2)));
    }

    /// A run has to be contiguous. Two lines that both appear but with
    /// something between them are not the block the store described.
    #[test]
    fn a_run_must_be_contiguous() {
        let expansion = "a;\nb;\nc;\n";
        assert_eq!(locate(expansion, "a;\nc;"), None);
    }
}
