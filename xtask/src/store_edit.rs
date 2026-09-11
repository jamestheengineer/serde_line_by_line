//! Rewriting the content stores in place.
//!
//! A bump changes two things in a store file — which source it names and most
//! of the `lines` values — and must change nothing else. Round-tripping
//! through a TOML serializer would reflow every `body = """..."""` block,
//! reorder keys, and drop comments, producing a diff in which the migration is
//! invisible among a hundred thousand lines of noise. The whole point of
//! reviewing a bump is being able to read that diff.
//!
//! So the edit is textual, and the parser here is only as deep as it needs to
//! be: enough to know whether a line is inside a `"""` body (where `lines = `
//! is prose, not a key) and which record it belongs to.
//!
//! Three stores are keyed to pinned line ranges, and they name their source
//! differently, so the rewriter is told which shape it is looking at
//! ([`Spec`]):
//!
//! | store | record | names its source |
//! |---|---|---|
//! | `annotations/*.toml` | `[[annotation]]` | once, in the file header |
//! | `glossary/*.toml` | `[[entry]]` | once, in the file header |
//! | `narrative/*.toml` | `[[step]]` | per record — one file cites two sources |
//!
//! That last row is why [`Edit::Keep`] exists. A narrative file holds steps
//! citing `serde_derive` next to steps citing `serde_core`, and a bump of
//! either one has to leave the other's ranges exactly where they are.

use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// What to do with one record.
#[derive(Debug, Clone)]
pub enum Edit {
    /// Give it this `lines` value, and — if the record names its own source —
    /// the new source id.
    Retarget(String),
    /// Remove the record entirely. Its text is reproduced in the migration
    /// report first — this is only reachable when the lines it claimed no
    /// longer exist.
    Drop,
    /// Leave it byte for byte. It cites a source this bump is not moving.
    Keep,
}

/// Which store is being rewritten.
#[derive(Debug, Clone, Copy)]
pub struct Spec<'a> {
    /// The table header that opens one record: `[[annotation]]`, `[[entry]]`,
    /// `[[step]]`.
    pub header: &'a str,
    /// The source id the store is moving to.
    pub new_source: &'a str,
    /// Whether the file names its source once, in the header above the first
    /// record. False for `narrative/`, where each step names its own and a
    /// file-level rewrite would retarget the steps this bump is leaving alone.
    pub file_source: bool,
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub retargeted: usize,
    pub dropped: usize,
    pub kept: usize,
    /// The full text of every dropped record, so the migration report can
    /// carry the prose forward even though the file no longer does.
    pub dropped_text: Vec<(String, String)>,
}

/// Applies `edits` to one store file's text.
///
/// Every id in `edits` must appear in the file, and every record in the file
/// must appear in `edits` — as [`Edit::Keep`] if this bump does not touch it:
/// a silent mismatch here is a line range left pointing at the previous
/// version's source.
pub fn rewrite(
    text: &str,
    spec: Spec<'_>,
    edits: &BTreeMap<String, Edit>,
) -> Result<(String, Outcome)> {
    let records = scan(text, spec.header)?;
    let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
    for r in &records {
        seen.insert(r.id.as_str(), ());
        if !edits.contains_key(&r.id) {
            bail!("no edit supplied for annotation {:?}", r.id);
        }
    }
    for id in edits.keys() {
        if !seen.contains_key(id.as_str()) {
            bail!("edit supplied for {id:?}, which is not in this file");
        }
    }
    let new_source = spec.new_source;

    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut outcome = Outcome::default();

    let body_start = records.first().map_or(lines.len(), |r| r.start);
    for line in &lines[..body_start] {
        out.push(
            match strip_key(line, "source").filter(|_| spec.file_source) {
                Some(prefix) => format!("{prefix}\"{new_source}\""),
                None => (*line).to_string(),
            },
        );
    }

    for record in &records {
        let span = &lines[record.start..record.end];
        match &edits[&record.id] {
            Edit::Drop => {
                outcome.dropped += 1;
                outcome
                    .dropped_text
                    .push((record.id.clone(), span.join("\n")));
            }
            Edit::Retarget(value) => {
                outcome.retargeted += 1;
                for (i, line) in span.iter().enumerate() {
                    let absolute = record.start + i;
                    if Some(absolute) == record.lines_at {
                        let prefix = strip_key(line, "lines").expect("located by the scanner");
                        out.push(format!("{prefix}\"{value}\""));
                    } else if Some(absolute) == record.source_at {
                        // A record that names its own source is being carried
                        // across, so it names the new one.
                        let prefix = strip_key(line, "source").expect("located by the scanner");
                        out.push(format!("{prefix}\"{new_source}\""));
                    } else {
                        out.push((*line).to_string());
                    }
                }
            }
            Edit::Keep => {
                outcome.kept += 1;
                out.extend(span.iter().map(|l| (*l).to_string()));
            }
        }
    }

    // Dropping the last record would otherwise leave the blank line that
    // separated it from the one before.
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    out.push(String::new());
    Ok((out.join("\n"), outcome))
}

struct Record {
    id: String,
    /// Half-open line span, including the blank line that follows the record.
    start: usize,
    end: usize,
    /// Index of the record's `lines = ` line.
    lines_at: Option<usize>,
    /// Index of the record's own `source = ` line, for the stores that have
    /// one. `None` for a store whose file header names the source instead.
    source_at: Option<usize>,
}

/// Which lines are TOML structure rather than the inside of a `"""` body.
///
/// Every editor here needs this: `course.toml` has unit bodies that quote the
/// schema, and annotation bodies quote their own keys. Without the mask, an
/// edit lands in someone's prose.
fn top_level(text: &str) -> Vec<bool> {
    let mut mask = Vec::with_capacity(text.lines().count());
    let mut in_body = false;
    for line in text.lines() {
        if in_body {
            mask.push(false);
            // A multi-line basic string ends at the first line that closes it;
            // a body line can never itself be `"""`, because that would have
            // terminated the string.
            if line.trim_end() == "\"\"\"" {
                in_body = false;
            }
        } else {
            let opens = line.trim_end().ends_with("\"\"\"") && line.contains('=');
            mask.push(!opens);
            in_body = opens;
        }
    }
    mask
}

/// Replaces the value of a top-level scalar key, keeping the rest of the line
/// — and the rest of the file — byte for byte.
pub fn set_scalar(text: &str, key: &str, value: &str) -> Result<String> {
    let mask = top_level(text);
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    for (i, line) in text.lines().enumerate() {
        match strip_key(line, key).filter(|_| mask[i] && !done) {
            Some(prefix) => {
                done = true;
                out.push(format!("{prefix}\"{value}\""));
            }
            None => out.push(line.to_string()),
        }
    }
    if !done {
        bail!("no top-level `{key}` key to set");
    }
    out.push(String::new());
    Ok(out.join("\n"))
}

/// Replaces the contents of a top-level array of strings, written one item per
/// line the way the committed files write them.
pub fn set_string_array(text: &str, key: &str, items: &[String]) -> Result<String> {
    let mask = top_level(text);
    let lines: Vec<&str> = text.lines().collect();
    let start = (0..lines.len())
        .find(|&i| {
            mask[i]
                && strip_key(lines[i], key)
                    .is_some_and(|p| lines[i][p.len()..].trim_start().starts_with('['))
        })
        .ok_or_else(|| anyhow::anyhow!("no top-level `{key}` array to set"))?;
    let end = (start..lines.len())
        .find(|&i| mask[i] && lines[i].trim_end().ends_with(']'))
        .ok_or_else(|| anyhow::anyhow!("`{key}` array is not closed"))?;

    let mut out: Vec<String> = lines[..start].iter().map(|s| s.to_string()).collect();
    out.push(format!("{key} = ["));
    for item in items {
        out.push(format!("    \"{item}\","));
    }
    out.push("]".to_string());
    out.extend(lines[end + 1..].iter().map(|s| s.to_string()));
    out.push(String::new());
    Ok(out.join("\n"))
}

/// Splits a file into its records, skipping over `"""` bodies.
fn scan(text: &str, header: &str) -> Result<Vec<Record>> {
    let mut records: Vec<Record> = Vec::new();
    let mask = top_level(text);
    let total = text.lines().count();

    for (i, line) in text.lines().enumerate() {
        if !mask[i] {
            continue;
        }
        if line.trim() == header {
            if let Some(prev) = records.last_mut() {
                prev.end = i;
            }
            records.push(Record {
                id: String::new(),
                start: i,
                end: total,
                lines_at: None,
                source_at: None,
            });
            continue;
        }
        let Some(current) = records.last_mut() else {
            continue;
        };
        if let Some(prefix) = strip_key(line, "id") {
            current.id = quoted(&line[prefix.len()..])
                .ok_or_else(|| anyhow::anyhow!("line {}: unparsable id", i + 1))?
                .to_string();
        } else if strip_key(line, "lines").is_some() {
            if current.lines_at.is_some() {
                bail!(
                    "line {}: record {:?} has two `lines` keys",
                    i + 1,
                    current.id
                );
            }
            current.lines_at = Some(i);
        } else if strip_key(line, "source").is_some() {
            if current.source_at.is_some() {
                bail!(
                    "line {}: record {:?} has two `source` keys",
                    i + 1,
                    current.id
                );
            }
            current.source_at = Some(i);
        }
    }

    for r in &records {
        if r.id.is_empty() {
            bail!("a {header} near line {} has no id", r.start + 1);
        }
        if r.lines_at.is_none() {
            bail!("record {:?} has no `lines` key", r.id);
        }
    }
    Ok(records)
}

/// If `line` assigns `key` at the top level, returns everything up to and
/// including the `=` and its trailing space, so the caller can substitute the
/// value while keeping the file's column alignment.
fn strip_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?;
    let spaces = rest.len() - rest.trim_start_matches(' ').len();
    let rest = &rest[spaces..];
    let rest = rest.strip_prefix('=')?;
    let after = rest.len() - rest.trim_start_matches(' ').len();
    Some(&line[..line.len() - rest.len() + after])
}

fn quoted(s: &str) -> Option<&str> {
    let s = s.trim();
    s.strip_prefix('"')?.strip_suffix('"')
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"schema = 1
source = "serde_core-1.0.1"

[[annotation]]
id    = "a-0001"
file  = "src/lib.rs"
lines = "1-9"
title = "First"
kind  = "plumbing"
body = """
A body that talks about itself:

lines = "999"
id    = "not-a-record"
[[annotation]]
"""

[[annotation]]
id    = "a-0002"
file  = "src/lib.rs"
lines = "10"
title = "Second"
kind  = "plumbing"
body = """
Short.
"""
"#;

    /// The narrative store: two sources in one file, and a `source` key on
    /// every record rather than a header above them all.
    const STEPS: &str = r#"schema = 1

[unit]
id = "01-unit"
body = """
Framing.
"""

[[step]]
id = "s01-a"
source = "serde_derive-1.0.1"
file = "src/lib.rs"
lines = "69-81"
title = "First"
body = """
Short.
"""

[[step]]
id = "s01-b"
source = "serde_core-1.0.229"
file = "src/ser/mod.rs"
lines = "310-318"
title = "The crossing"
body = """
Short.
"""
"#;

    const ANNOTATIONS: Spec<'static> = Spec {
        header: "[[annotation]]",
        new_source: "serde_core-1.0.2",
        file_source: true,
    };

    fn edits(pairs: &[(&str, Edit)]) -> BTreeMap<String, Edit> {
        pairs
            .iter()
            .map(|(id, e)| (id.to_string(), e.clone()))
            .collect()
    }

    /// The scanner has to ignore keys that appear inside a body, or a record
    /// whose prose quotes the schema would be rewritten into nonsense.
    #[test]
    fn keys_inside_a_body_are_prose() {
        let records = scan(FILE, "[[annotation]]").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, "a-0001");
        assert_eq!(records[1].id, "a-0002");
        assert_eq!(
            FILE.lines().nth(records[0].lines_at.unwrap()).unwrap(),
            "lines = \"1-9\""
        );
    }

    #[test]
    fn retargeting_touches_only_the_source_and_lines_keys() {
        let (out, outcome) = rewrite(
            FILE,
            ANNOTATIONS,
            &edits(&[
                ("a-0001", Edit::Retarget("3-11".into())),
                ("a-0002", Edit::Retarget("12".into())),
            ]),
        )
        .unwrap();
        assert_eq!(outcome.retargeted, 2);
        assert_eq!(outcome.dropped, 0);

        let expected = FILE
            .replace(
                "source = \"serde_core-1.0.1\"",
                "source = \"serde_core-1.0.2\"",
            )
            .replace("lines = \"1-9\"", "lines = \"3-11\"")
            .replacen(
                "lines = \"10\"\ntitle = \"Second\"",
                "lines = \"12\"\ntitle = \"Second\"",
                1,
            );
        assert_eq!(out, expected);
        assert!(out.contains("lines = \"999\""), "the body was left alone");
    }

    #[test]
    fn dropping_a_record_removes_it_whole_and_keeps_its_text() {
        let (out, outcome) = rewrite(
            FILE,
            ANNOTATIONS,
            &edits(&[
                ("a-0001", Edit::Drop),
                ("a-0002", Edit::Retarget("1".into())),
            ]),
        )
        .unwrap();
        assert_eq!(outcome.dropped, 1);
        assert!(!out.contains("a-0001"));
        assert!(out.contains("a-0002"));
        assert!(outcome.dropped_text[0]
            .1
            .contains("A body that talks about itself"));
        assert!(
            out.ends_with("Short.\n\"\"\"\n"),
            "no trailing blank line left behind"
        );
    }

    /// A record the caller forgot about would keep a line range from the
    /// previous version and still parse, so the mismatch has to be fatal.
    #[test]
    fn every_record_must_be_accounted_for() {
        let err = rewrite(
            FILE,
            ANNOTATIONS,
            &edits(&[("a-0001", Edit::Retarget("1".into()))]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("a-0002"), "{err}");
    }

    /// The narrative case. Bumping `serde_derive` moves one step and must
    /// leave the `serde_core` step beside it — range and source both —
    /// exactly as it found it.
    #[test]
    fn a_mixed_source_file_moves_only_the_records_being_bumped() {
        let (out, outcome) = rewrite(
            STEPS,
            Spec {
                header: "[[step]]",
                new_source: "serde_derive-1.0.2",
                file_source: false,
            },
            &edits(&[
                ("s01-a", Edit::Retarget("70-82".into())),
                ("s01-b", Edit::Keep),
            ]),
        )
        .unwrap();
        assert_eq!((outcome.retargeted, outcome.kept), (1, 1));

        let expected = STEPS
            .replace("serde_derive-1.0.1", "serde_derive-1.0.2")
            .replace("lines = \"69-81\"", "lines = \"70-82\"");
        assert_eq!(out, expected);
        assert!(out.contains("source = \"serde_core-1.0.229\""));
        assert!(out.contains("lines = \"310-318\""));
    }

    /// `[unit]` sits above the first `[[step]]` and carries no `source` key.
    /// Were one ever added there, `file_source: false` is what keeps this
    /// store's header out of the rewrite.
    #[test]
    fn a_records_own_source_is_the_one_that_moves() {
        let records = scan(STEPS, "[[step]]").unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            STEPS.lines().nth(records[0].source_at.unwrap()).unwrap(),
            "source = \"serde_derive-1.0.1\""
        );
        assert!(scan(FILE, "[[annotation]]").unwrap()[0].source_at.is_none());
    }

    #[test]
    fn a_scalar_inside_a_body_is_not_the_key_being_set() {
        let text =
            "source = \"old\"\n\n[[unit]]\nbody = \"\"\"\nsource = \"quoted in prose\"\n\"\"\"\n";
        let out = set_scalar(text, "source", "new").unwrap();
        assert!(out.starts_with("source = \"new\"\n"));
        assert!(out.contains("source = \"quoted in prose\""));
    }

    #[test]
    fn an_array_is_replaced_wholesale() {
        let text = "# a comment\nreading_order = [\n    \"a\",\n    \"b\",\n]\n\nschema = 1\n";
        let out = set_string_array(text, "reading_order", &["b".into(), "c".into()]).unwrap();
        assert_eq!(
            out,
            "# a comment\nreading_order = [\n    \"b\",\n    \"c\",\n]\n\nschema = 1\n"
        );
    }

    #[test]
    fn an_unknown_id_is_rejected() {
        let err = rewrite(
            FILE,
            ANNOTATIONS,
            &edits(&[
                ("a-0001", Edit::Retarget("1".into())),
                ("a-0002", Edit::Retarget("2".into())),
                ("a-0003", Edit::Retarget("3".into())),
            ]),
        )
        .unwrap_err();
        assert!(err.to_string().contains("a-0003"), "{err}");
    }
}
