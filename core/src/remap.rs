//! Carrying annotations across a source version bump.
//!
//! Annotations are keyed to `(file, line-range)` in a pinned tree, which is the
//! whole reason `vendor/` is checksum-gated: if the source moves, every range
//! in the store is quietly wrong. A version bump is therefore not a `cargo
//! update` but a migration, and this module is the part of it that can be
//! mechanical.
//!
//! The load-bearing property is that a remap must preserve *tiling*. Coverage
//! demands that a complete file's annotations be non-overlapping and
//! collectively exhaustive; a remapper that moved each range independently
//! would leave one-line gaps and overlaps all over a file that had merely
//! shifted. So ranges are not moved — **boundaries** are. Each annotation's
//! endpoints are looked up in a single boundary map derived from the diff, and
//! two ranges that were adjacent in the old tree stay adjacent in the new one
//! by construction.
//!
//! What cannot be mechanical is the prose. A range whose lines changed still
//! points at real code, but the explanation may now describe something that is
//! no longer there — so every remapped annotation is classified (§[`Change`])
//! and the ones that need a human are reported rather than silently accepted.

use crate::schema::LineRange;
use anyhow::{bail, Result};
use std::collections::HashMap;

/// Refuse a diff whose unmatched middle is larger than this many cells. The
/// LCS below is quadratic, and a file that has been rewritten this thoroughly
/// has no meaningful line correspondence left to recover anyway — remapping it
/// automatically would produce plausible-looking ranges over unrelated code.
const MAX_DIFF_CELLS: usize = 8_000_000;

/// A line-level correspondence between one file's old and new contents.
#[derive(Debug, Clone)]
pub struct FileMap {
    old_len: usize,
    new_len: usize,
    /// `boundaries[b]` is the new-side boundary corresponding to old-side
    /// boundary `b`, where boundary `b` sits immediately before line `b`
    /// (0-based). Length `old_len + 1`, non-decreasing, pinned at both ends.
    boundaries: Vec<usize>,
    old_matched: Vec<bool>,
    new_matched: Vec<bool>,
}

/// What happened to one annotation's span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// Same lines, same place. Nothing to review.
    Unmoved,
    /// Same lines, shifted by edits elsewhere in the file.
    Shifted,
    /// Lines inside the span were added, removed, or rewritten. The range is
    /// still valid; whether the prose is, is a question for a human.
    Edited { deleted: u32, inserted: u32 },
    /// Every line the annotation claimed is gone. There is nothing left to
    /// point at, so the record has to be deleted or rewritten by hand.
    Deleted,
}

impl Change {
    /// Whether a human has to look at this one before the bump can land.
    pub fn needs_review(&self) -> bool {
        matches!(self, Change::Edited { .. } | Change::Deleted)
    }
}

/// One annotation's span, carried across.
#[derive(Debug, Clone, Copy)]
pub struct Remapped {
    /// The new span, or `None` when every line it claimed was deleted.
    pub range: Option<LineRange>,
    pub change: Change,
}

impl FileMap {
    /// Aligns two versions of one file.
    pub fn build(old: &str, new: &str) -> Result<Self> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();
        let matches = lcs_matches(&old_lines, &new_lines)?;

        let (old_len, new_len) = (old_lines.len(), new_lines.len());
        let mut old_matched = vec![false; old_len];
        let mut new_matched = vec![false; new_len];
        for &(o, n) in &matches {
            old_matched[o] = true;
            new_matched[n] = true;
        }

        // Each old boundary `b` has a window of new positions it could
        // legitimately map to: `lo`, just after the last surviving old line
        // before it, and `hi`, at the first surviving old line after it. The
        // lines between the two are insertions, and the window is the question
        // of which side of the boundary they landed on.
        //
        // Picking one end for every boundary is not good enough. Always taking
        // `lo` hands every insertion to the following span, which turns the
        // most ordinary edit there is — a line rewritten in place — into a span
        // whose old line is deleted and whose replacement belongs to its
        // neighbour. So the deletions decide: an insertion that sits against
        // deleted lines on only one side is that side's replacement text, and
        // the boundary moves to keep them together. Everything else takes `lo`.
        //
        // Both ends are pinned regardless: boundary 0 is 0 and boundary
        // `old_len` is `new_len`, so lines gained at the top or bottom of a
        // file are still claimed by something. Without that, exhaustiveness
        // would break at exactly the two places new code most often lands.
        let mut boundaries = vec![0usize; old_len + 1];
        let mut lo = 0usize;
        let mut at = 0usize;
        for b in 0..=old_len {
            while at < matches.len() && matches[at].0 < b {
                lo = matches[at].1 + 1;
                at += 1;
            }
            let hi = matches.get(at).map_or(new_len, |&(_, n)| n);
            let deleted_before = b > 0 && !old_matched[b - 1];
            let deleted_after = b < old_len && !old_matched[b];
            boundaries[b] = if deleted_before && !deleted_after {
                hi
            } else {
                lo
            };
        }
        boundaries[0] = 0;
        boundaries[old_len] = new_len;

        Ok(FileMap {
            old_len,
            new_len,
            boundaries,
            old_matched,
            new_matched,
        })
    }

    /// A file that did not change at all. Common enough during a patch-level
    /// bump — most of the tree — that it is worth reporting separately.
    pub fn is_identity(&self) -> bool {
        self.old_len == self.new_len && self.old_matched.iter().all(|&m| m)
    }

    pub fn old_len(&self) -> usize {
        self.old_len
    }

    pub fn new_len(&self) -> usize {
        self.new_len
    }

    /// Lines present in the new file that have no counterpart in the old one.
    /// These are what a bump adds to the annotation backlog.
    pub fn inserted_lines(&self) -> u32 {
        self.new_matched.iter().filter(|m| !**m).count() as u32
    }

    pub fn deleted_lines(&self) -> u32 {
        self.old_matched.iter().filter(|m| !**m).count() as u32
    }

    /// Carries one annotation's span across.
    ///
    /// Adjacent input ranges produce adjacent output ranges, so applying this
    /// to a tiling partition yields a tiling partition — minus any spans that
    /// were deleted outright, whose neighbours still meet.
    pub fn remap(&self, range: LineRange) -> Result<Remapped> {
        if range.end as usize > self.old_len {
            bail!(
                "range {}-{} runs past the old file, which has {} lines",
                range.start,
                range.end,
                self.old_len
            );
        }
        // 1-based closed range [start, end] occupies 0-based boundaries
        // [start - 1, end].
        let lo = self.boundaries[range.start as usize - 1];
        let hi = self.boundaries[range.end as usize];

        if lo >= hi {
            return Ok(Remapped {
                range: None,
                change: Change::Deleted,
            });
        }

        let deleted = (range.start as usize - 1..range.end as usize)
            .filter(|&i| !self.old_matched[i])
            .count() as u32;
        let inserted = (lo..hi).filter(|&i| !self.new_matched[i]).count() as u32;

        let change = if deleted > 0 || inserted > 0 {
            Change::Edited { deleted, inserted }
        } else if lo + 1 == range.start as usize {
            Change::Unmoved
        } else {
            Change::Shifted
        };

        Ok(Remapped {
            // Boundaries are 0-based and half-open; a 1-based closed range
            // starts one past the low boundary and ends at the high one.
            range: Some(LineRange {
                start: lo as u32 + 1,
                end: hi as u32,
            }),
            change,
        })
    }
}

/// Matched line pairs between two files, as `(old_index, new_index)`, strictly
/// increasing in both.
///
/// Common prefix and suffix are trimmed before the quadratic part runs. For the
/// diffs this tool actually sees — a patch bump of a 12,000-line crate — that
/// reduces the table to a few hundred cells.
fn lcs_matches(old: &[&str], new: &[&str]) -> Result<Vec<(usize, usize)>> {
    let mut head = 0usize;
    while head < old.len() && head < new.len() && old[head] == new[head] {
        head += 1;
    }
    let mut tail = 0usize;
    while tail < old.len() - head
        && tail < new.len() - head
        && old[old.len() - 1 - tail] == new[new.len() - 1 - tail]
    {
        tail += 1;
    }

    let a = &old[head..old.len() - tail];
    let b = &new[head..new.len() - tail];

    let cells = a.len().saturating_mul(b.len());
    if cells > MAX_DIFF_CELLS {
        bail!(
            "the changed region is {}x{} lines; this is a rewrite, not a bump, \
             and its line ranges have to be re-cut by hand",
            a.len(),
            b.len()
        );
    }

    let mut out: Vec<(usize, usize)> = (0..head).map(|i| (i, i)).collect();

    if !a.is_empty() && !b.is_empty() {
        // Interning turns the inner comparison into a `u32` equality. Line
        // text repeats heavily in this crate — `    }`, blank lines, the same
        // macro arm forty times — so the table walk is where the time goes.
        let mut ids: HashMap<&str, u32> = HashMap::new();
        let mut ax = Vec::with_capacity(a.len());
        let mut bx = Vec::with_capacity(b.len());
        for (src, dst) in [(a, &mut ax), (b, &mut bx)] {
            for line in src {
                let next = ids.len() as u32;
                dst.push(*ids.entry(*line).or_insert(next));
            }
        }

        let w = b.len() + 1;
        let mut table = vec![0u32; (a.len() + 1) * w];
        for i in (0..a.len()).rev() {
            for j in (0..b.len()).rev() {
                table[i * w + j] = if ax[i] == bx[j] {
                    table[(i + 1) * w + j + 1] + 1
                } else {
                    table[(i + 1) * w + j].max(table[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0usize, 0usize);
        while i < a.len() && j < b.len() {
            if ax[i] == bx[j] {
                out.push((head + i, head + j));
                i += 1;
                j += 1;
            } else if table[(i + 1) * w + j] >= table[i * w + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }

    for t in 0..tail {
        out.push((old.len() - tail + t, new.len() - tail + t));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(old: &str, new: &str) -> FileMap {
        FileMap::build(old, new).unwrap()
    }

    fn r(s: u32, e: u32) -> LineRange {
        LineRange { start: s, end: e }
    }

    fn span(m: &FileMap, s: u32, e: u32) -> (Option<(u32, u32)>, Change) {
        let out = m.remap(r(s, e)).unwrap();
        (out.range.map(|x| (x.start, x.end)), out.change)
    }

    #[test]
    fn identical_files_map_to_themselves() {
        let text = "a\nb\nc\nd\n";
        let m = map(text, text);
        assert!(m.is_identity());
        assert_eq!(span(&m, 2, 3), (Some((2, 3)), Change::Unmoved));
    }

    #[test]
    fn an_insertion_above_shifts_the_ranges_below_it() {
        let m = map("a\nb\nc\n", "a\nNEW\nNEW\nb\nc\n");
        assert!(!m.is_identity());
        assert_eq!(m.inserted_lines(), 2);
        // The first span absorbs nothing; the inserted lines land at the
        // boundary and so belong to the span that follows.
        assert_eq!(
            span(&m, 1, 1),
            (Some((1, 1)), Change::Unmoved),
            "line 1 did not move"
        );
        assert_eq!(
            span(&m, 2, 3),
            (
                Some((2, 5)),
                Change::Edited {
                    deleted: 0,
                    inserted: 2
                }
            )
        );
    }

    #[test]
    fn a_deletion_inside_a_span_narrows_it() {
        let m = map("a\nb\nc\nd\n", "a\nb\nd\n");
        assert_eq!(m.deleted_lines(), 1);
        assert_eq!(
            span(&m, 2, 3),
            (
                Some((2, 2)),
                Change::Edited {
                    deleted: 1,
                    inserted: 0
                }
            ),
            "the span keeps b and loses c"
        );
        assert_eq!(
            span(&m, 4, 4),
            (Some((3, 3)), Change::Shifted),
            "d moved up but is otherwise untouched"
        );
    }

    #[test]
    fn a_span_whose_lines_all_vanish_is_reported_not_guessed() {
        let m = map("keep\ngone\ngone\nkeep\n", "keep\nkeep\n");
        let out = m.remap(r(2, 3)).unwrap();
        assert!(out.range.is_none());
        assert_eq!(out.change, Change::Deleted);
        assert!(out.change.needs_review());
    }

    /// The property the coverage gate depends on: a partition stays a
    /// partition. Without it, a bump would turn one shifted file into dozens
    /// of one-line gaps.
    #[test]
    fn a_tiling_partition_stays_tiling() {
        let old = "1\n2\n3\n4\n5\n6\n7\n8\n";
        let new = "1\n2\nX\n4\n5\n6\nY\nZ\n7\n8\nW\n";
        let m = map(old, new);

        let partition = [r(1, 2), r(3, 3), r(4, 6), r(7, 8)];
        let mut cursor = 1u32;
        for range in partition {
            let out = m.remap(range).unwrap();
            let got = out.range.expect("nothing was deleted outright");
            assert_eq!(got.start, cursor, "gap or overlap before {range:?}");
            cursor = got.end + 1;
        }
        assert_eq!(
            cursor as usize - 1,
            m.new_len(),
            "the last span must reach the end of the file"
        );
    }

    /// Lines appended to the end of a file have no boundary after them, so
    /// they can only be claimed by extending the final span. Coverage would
    /// fail otherwise, and it would fail at the place new code most often
    /// lands.
    #[test]
    fn trailing_insertions_extend_the_final_span() {
        let m = map("a\nb\n", "a\nb\nc\nd\n");
        assert_eq!(
            span(&m, 2, 2),
            (
                Some((2, 4)),
                Change::Edited {
                    deleted: 0,
                    inserted: 2
                }
            )
        );
    }

    #[test]
    fn a_rewritten_line_is_flagged_rather_than_passed_through() {
        let m = map("a\nold\nc\n", "a\nnew\nc\n");
        assert_eq!(
            span(&m, 2, 2),
            (
                Some((2, 2)),
                Change::Edited {
                    deleted: 1,
                    inserted: 1
                }
            ),
            "same place, different code: the prose needs a look"
        );
    }

    #[test]
    fn a_range_past_the_end_of_the_old_file_is_an_error() {
        let m = map("a\nb\n", "a\nb\nc\n");
        assert!(m.remap(r(2, 9)).is_err());
    }
}
