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
use slbl_core::{vendor, Store, Unit};
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
}

struct NavFile {
    path: String,
    href: String,
    short: String,
    lines: u32,
    percent: f64,
    percent_label: String,
    complete: bool,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexPage {
    nav: Vec<NavFile>,
    total_lines: u32,
    claimed_lines: u32,
    percent: f64,
    percent_label: String,
    annotations: usize,
    source_id: String,
    root: String,
    current: String,
}

#[derive(Template)]
#[template(path = "file.html")]
struct FilePage {
    nav: Vec<NavFile>,
    file: String,
    percent: f64,
    percent_label: String,
    lines: u32,
    blocks: Vec<Block>,
    source_id: String,
    root: String,
    current: String,
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

    let nav = build_nav(&store);

    let index = IndexPage {
        nav: build_nav(&store),
        total_lines: store.total_lines(),
        claimed_lines: store.claimed_lines(),
        percent: store.percent(),
        percent_label: format!("{:.1}", store.percent()),
        annotations: store.by_file.values().map(Vec::len).sum(),
        source_id: vendor::SOURCE_ID.to_string(),
        root: "./".to_string(),
        current: String::new(),
    };
    write(&out.join("index.html"), &index.render()?)?;

    let root = vendor::vendor_root(&repo);
    for (file, lines) in &store.files {
        let text =
            std::fs::read_to_string(root.join(file)).with_context(|| format!("reading {file}"))?;
        let highlighted = hl
            .file(&text)
            .with_context(|| format!("highlighting {file}"))?;
        let units = store.units_for(file);

        let page = FilePage {
            nav: nav_for(&nav),
            file: file.clone(),
            percent: percent_of(units, *lines),
            percent_label: format!("{:.1}", percent_of(units, *lines)),
            lines: *lines,
            blocks: blocks_for(units, &highlighted),
            source_id: vendor::SOURCE_ID.to_string(),
            root: "../".to_string(),
            current: file.clone(),
        };
        write(&out.join("file").join(page_name(file)), &page.render()?)?;
    }

    // Copied rather than embedded: the playground is a binary artefact, and it
    // may legitimately be absent — the site must build without a wasm
    // toolchain (see `cargo xtask wasm`).
    copy_dir(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
        &out.join("static"),
    )?;
    write(&out.join("static").join("syntax.css"), &syntax_css(&hl)?)?;

    println!(
        "wrote {} pages to {}  ({:.1}% annotated, {} annotations)",
        store.files.len() + 1,
        out.display(),
        store.percent(),
        index_count(&store),
    );
    Ok(())
}

fn index_count(store: &Store) -> usize {
    store.by_file.values().map(Vec::len).sum()
}

/// Splits a file into alternating annotated and unannotated blocks.
fn blocks_for(units: &[Unit], highlighted: &[Line]) -> Vec<Block> {
    let total = highlighted.len() as u32;
    let mut blocks = Vec::new();
    let mut cursor = 1u32;

    for unit in units {
        if unit.range.start > cursor {
            blocks.push(gap(cursor, unit.range.start - 1, highlighted));
        }
        let a = &unit.annotation;
        blocks.push(Block {
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
        });
        cursor = cursor.max(unit.range.end + 1);
    }
    if cursor <= total {
        blocks.push(gap(cursor, total, highlighted));
    }
    blocks
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

fn build_nav(store: &Store) -> Vec<NavFile> {
    store
        .files
        .iter()
        .map(|(path, lines)| NavFile {
            href: page_name(path),
            short: path.strip_prefix("src/").unwrap_or(path).to_string(),
            percent: percent_of(store.units_for(path), *lines),
            percent_label: format!("{:.0}", percent_of(store.units_for(path), *lines)),
            complete: store.complete.iter().any(|c| c == path),
            path: path.clone(),
            lines: *lines,
        })
        .collect()
}

fn nav_for(nav: &[NavFile]) -> Vec<NavFile> {
    nav.iter()
        .map(|n| NavFile {
            path: n.path.clone(),
            href: n.href.clone(),
            short: n.short.clone(),
            lines: n.lines,
            percent: n.percent,
            percent_label: n.percent_label.clone(),
            complete: n.complete,
        })
        .collect()
}

/// `src/ser/mod.rs` -> `src-ser-mod.rs.html`
fn page_name(file: &str) -> String {
    format!("{}.html", file.replace('/', "-"))
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
