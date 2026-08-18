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

#[derive(Debug, Deserialize)]
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

    pub fn len(&self) -> u32 {
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
