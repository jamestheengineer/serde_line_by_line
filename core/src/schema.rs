//! The annotation schema.
//!
//! Annotations are data, not pages. Each record claims a closed line range in
//! one pinned source file. The coverage gate (see `coverage.rs`) relies on
//! ranges within a file being non-overlapping and, once a file is declared
//! complete, collectively exhaustive.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
pub struct AnnotationFile {
    pub schema: u32,
    /// Must match the pinned source id, e.g. "serde_core-1.0.229".
    pub source: String,
    #[serde(default, rename = "annotation")]
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annotation {
    pub id: String,
    /// Path relative to the vendored crate root, e.g. "src/ser/mod.rs".
    pub file: String,
    /// Closed range, "310-318", or a single line, "42".
    pub lines: String,
    pub title: String,
    pub kind: Kind,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub course_unit: Option<String>,
    #[serde(default)]
    pub rust_features: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub prereqs: Vec<String>,
    /// Glossary ids this annotation leans on: `["syn::DeriveInput"]`. Borrowed
    /// vocabulary is cited, never claimed (D9), so this is how a reader gets
    /// the definition of a type the annotated crate does not define.
    #[serde(default)]
    pub glossary: Vec<String>,
    /// For `kind = "macro-use"` only: the id of the `macro-def` annotation this
    /// span expands. Required there and rejected elsewhere — the renderer
    /// collapses runs of uses sharing one def, so this is a rendering input,
    /// not documentation.
    #[serde(default)]
    pub macro_def: Option<String>,
    pub body: String,
}

/// Drives rendering. `MacroUse` renders compactly and links back to its
/// `MacroDef` instead of repeating the explanation — this is what keeps
/// `de/impls.rs` from costing 3,174 paragraphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    TraitItem,
    MacroDef,
    MacroUse,
    Impl,
    DocContract,
    Plumbing,
    CfgGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Track {
    Reference,
    Course,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    pub fn parse(s: &str) -> Result<Self> {
        let (start, end) = match s.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (s.trim(), s.trim()),
        };
        let start: u32 = start
            .parse()
            .with_context(|| format!("bad line range {s:?}"))?;
        let end: u32 = end
            .parse()
            .with_context(|| format!("bad line range {s:?}"))?;
        if start == 0 {
            bail!("line numbers are 1-based, got {s:?}");
        }
        if start > end {
            bail!("inverted line range {s:?}");
        }
        Ok(LineRange { start, end })
    }

    /// Number of lines the range covers. Always at least 1 — a range is
    /// non-empty by construction, which is why this is not `len`/`is_empty`.
    pub fn line_count(&self) -> u32 {
        self.end - self.start + 1
    }

    pub fn overlaps(&self, other: &LineRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }
}

/// Per-file completion status. A file listed as `complete` turns coverage gaps
/// from a warning into a hard failure.
#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub source: String,
    #[serde(default)]
    pub complete: Vec<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing manifest {}", path.display()))
    }
}

/// A glossary file (`glossary/<crate>.toml`).
///
/// Glossary entries quote a **glossary source** — a crate `serde_derive` reads
/// as vocabulary and this project never annotates (D9). An entry is an
/// annotation's shape minus everything that implies a claim: no `kind`, no
/// `tracks`, no `course_unit`, and no participation in coverage. What it keeps
/// is the part that matters, a line range in a pinned tree, so a quotation
/// cannot drift away from the crate it says it came from.
#[derive(Debug, Deserialize)]
pub struct GlossaryFile {
    pub schema: u32,
    /// Must match a pinned source with `role = "glossary"`, e.g. "syn-3.0.5".
    pub source: String,
    #[serde(default, rename = "entry")]
    pub entries: Vec<GlossaryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlossaryEntry {
    /// The path an annotation cites: `"syn::DeriveInput"`. Crate-qualified
    /// because `Ident` means two different things depending on who you ask.
    pub id: String,
    /// Path relative to the vendored crate root, e.g. "src/derive.rs".
    pub file: String,
    /// Closed range over the definition, quoted verbatim when rendered.
    pub lines: String,
    pub title: String,
    /// Other glossary ids this entry leans on. Rendered as links; not a DAG
    /// and not ordered, because a glossary is read by jumping into it.
    #[serde(default)]
    pub see_also: Vec<String>,
    pub body: String,
}

impl GlossaryFile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading glossary {}", path.display()))?;
        let parsed: GlossaryFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if parsed.schema != SCHEMA_VERSION {
            bail!(
                "{}: schema {} but this build understands {SCHEMA_VERSION}",
                path.display(),
                parsed.schema
            );
        }
        Ok(parsed)
    }
}

/// The course track's unit registry (`annotations/course.toml`).
///
/// The units are content in their own right — the framing that turns a set of
/// annotations into a lesson — so they live in the store beside the
/// annotations rather than in the renderer.
#[derive(Debug, Deserialize)]
pub struct CourseFile {
    pub schema: u32,
    pub source: String,
    /// Dependency order over the pinned source files. Used to sequence
    /// annotations drawn from several files into one unit; path order would put
    /// `de/` before `ser/`, which is backwards.
    #[serde(default)]
    pub reading_order: Vec<String>,
    #[serde(default, rename = "unit")]
    pub units: Vec<CourseUnit>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CourseUnit {
    /// Slug with a numeric prefix, e.g. "03-associated-types". Annotations
    /// point at this through `course_unit`.
    pub id: String,
    pub title: String,
    /// How much of the unit serde_core can actually teach. The plan commits to
    /// labelling this in the UI rather than pretending the crate covers
    /// everything (PLAN.md §2).
    pub supplement: Supplement,
    pub status: UnitStatus,
    #[serde(default)]
    pub prereqs: Vec<String>,
    /// Glossary ids this annotation leans on: `["syn::DeriveInput"]`. Borrowed
    /// vocabulary is cited, never claimed (D9), so this is how a reader gets
    /// the definition of a type the annotated crate does not define.
    #[serde(default)]
    pub glossary: Vec<String>,
    #[serde(default)]
    pub rust_features: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
    pub summary: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Supplement {
    /// Taught entirely from serde_core.
    None,
    /// serde_core shows part of it; written material fills the rest.
    Partial,
    /// serde_core does not exercise this at all.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitStatus {
    Written,
    Planned,
}

impl CourseFile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading course registry {}", path.display()))?;
        let parsed: CourseFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if parsed.schema != SCHEMA_VERSION {
            bail!(
                "{}: schema {} but this build understands {SCHEMA_VERSION}",
                path.display(),
                parsed.schema
            );
        }
        Ok(parsed)
    }
}

/// One unit of the narrative track (`narrative/<id>.toml`).
///
/// The narrative walks `serde_derive` rather than claiming it (PLAN.md §11).
/// That is the whole difference between this type and [`Annotation`]: there is
/// no `kind`, no `tracks`, no coverage, and — because a walk may look at the
/// same code twice from two directions — no non-overlap rule. What it keeps is
/// the part that has protected every line range in this repo since phase 0: a
/// citation into a pinned tree.
///
/// One unit per file, and the file stem is the id. Declaration order within a
/// unit is reading order; the numeric prefix on the id orders the units. There
/// is no registry file, because a registry is a second place for the order to
/// live and the filenames already say it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeFile {
    pub schema: u32,
    pub unit: NarrativeUnit,
    #[serde(default, rename = "step")]
    pub steps: Vec<NarrativeStep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeUnit {
    /// Slug with a numeric prefix, e.g. `"03-the-attribute-dsl"`. Must equal
    /// the file's stem.
    pub id: String,
    pub title: String,
    pub summary: String,
    /// Name of an `expand/cases/*.rs` input this unit is following. Every step
    /// that quotes emitted code quotes it out of *this* case's expansion, so
    /// the unit is anchored to one worked example rather than to a mood.
    #[serde(default)]
    pub expand_case: Option<String>,
    pub body: String,
}

/// One stop on the walk: a range in a pinned tree, and why the reader is here.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NarrativeStep {
    pub id: String,
    /// Which pinned tree, e.g. `"serde_derive-1.0.229"`. A step may cite the
    /// narrative source or the coverage source; quoting a glossary source is
    /// the glossary's job (D9).
    pub source: String,
    /// Path relative to that source's crate root, e.g. `"src/ser.rs"`.
    pub file: String,
    pub lines: String,
    pub title: String,
    /// Earlier steps this one depends on having been read. The D8 rule applies
    /// unchanged: a step may not lean on one that comes later in the walk.
    #[serde(default)]
    pub leans_on: Vec<String>,
    /// Borrowed vocabulary this step reads (D9).
    #[serde(default)]
    pub glossary: Vec<String>,
    /// The reference-track annotation this step lands in. Required when the
    /// step cites the coverage source and rejected otherwise: the narrative may
    /// only walk into `serde_core` where the reference track has already
    /// claimed the ground, and the reader gets a link there.
    #[serde(default)]
    pub annotation: Option<String>,
    /// Generated code this step claims the cited machinery produces, verified
    /// against a real expansion of the unit's `expand_case` at render time —
    /// and rendered from that expansion, never from this string. Lines are
    /// matched contiguously, compared with leading and trailing whitespace
    /// stripped, so a change in generated indentation is not a failure but a
    /// change in generated code is.
    #[serde(default)]
    pub emits: Option<String>,
    /// Which half of the expansion `emits` is quoting.
    #[serde(default)]
    pub emits_from: Option<DeriveKind>,
    /// A case other than the unit's own to quote from. Rare and deliberate:
    /// the bounds unit follows the same struct as its neighbours, but that
    /// struct has no type parameters, so the one place a bound is visible is a
    /// second input. Saying which is better than quietly switching examples.
    #[serde(default)]
    pub emits_case: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeriveKind {
    Serialize,
    Deserialize,
}

impl DeriveKind {
    pub fn label(self) -> &'static str {
        match self {
            DeriveKind::Serialize => "Serialize",
            DeriveKind::Deserialize => "Deserialize",
        }
    }
}

impl NarrativeFile {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading narrative unit {}", path.display()))?;
        let parsed: NarrativeFile =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        if parsed.schema != SCHEMA_VERSION {
            bail!(
                "{}: schema {} but this build understands {SCHEMA_VERSION}",
                path.display(),
                parsed.schema
            );
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if parsed.unit.id != stem {
            bail!(
                "{}: unit id {:?} does not match the file name — the filename is what \
                 orders the narrative, so the two may not disagree",
                path.display(),
                parsed.unit.id
            );
        }
        Ok(parsed)
    }
}
