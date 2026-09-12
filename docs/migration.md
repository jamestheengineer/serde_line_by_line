# Bumping a pinned source

Every annotation, glossary entry and narrative step in this repo is keyed to
`(file, line-range)` in a tree under `vendor/`. That is what makes "every line"
a checkable property rather than a slogan — and it is what makes an ordinary
dependency update dangerous here. Swap a source under the store and nothing
fails to compile: the coverage gate still counts 12,037 claimed lines, the site
still builds, and several hundred explanations quietly describe whatever code
now occupies those line numbers.

So a bump is a migration, and it has a tool.

```
cargo xtask bump [--source NAME] <version> [--dry-run]
```

`--source` names which of the five pinned crates to move. It used to be
optional, defaulting to "the coverage source"; there are two of those since
[D12](decisions.md), so omitting it now fails with both names rather than
guessing between two crates that each key line ranges. What the bump rewrites
follows from that source's **role** and nothing else (D11):

| role | source | stores keyed to it |
|---|---|---|
| `coverage` | `serde_core` | `annotations/serde_core/`, the narrative steps citing it, and the course registry |
| `coverage` | `serde_derive` | `annotations/serde_derive/`, and the narrative steps citing it |
| `glossary` | `syn`, `quote`, `proc-macro2` | the one `glossary/` file that quotes it |

Each coverage source owns its own store directory, which is what makes "rewrite
exactly one store" a property of the filesystem rather than of a filter. The
course registry is one file over both crates and belongs to the source it names
— a bump of the other one leaves it alone, and its entries there are qualified
(`serde_derive:src/ser.rs`) so none of them is keyed to the moving tree by a
bare path.

## What the tool does

1. **Fetches and verifies.** The sha256 comes from the crates.io sparse index,
   the archive from `static.crates.io`, and the two must agree before anything
   is unpacked. Pass `--sha256 HEX` and `--archive PATH` to run offline.
2. **Stages.** The new tree is unpacked into `target/bump/`, never into
   `vendor/`. Nothing in the repository is touched until the whole migration
   has been planned.
3. **Aligns each file.** A line-level diff between the old and new copies of
   every source file, common prefix and suffix trimmed first (see
   `core/src/remap.rs`).
4. **Carries the ranges across.** Not by moving each range independently — that
   would leave one-line gaps and overlaps all over a file that had merely
   shifted — but by moving the *boundaries* between them, so two ranges that
   were adjacent stay adjacent and a tiling partition stays tiling.
5. **Classifies every annotation** as unmoved, shifted, edited, or deleted, and
   reports the last two.
6. **Rewrites everything that names the version**: the store files for that
   role and their `lines` values, `pin.toml`, `vendor/NOTICE.md`, and — where
   they exist — that source's `manifest.toml`, `course.toml`, each example's
   `serde_core = "=x.y.z"`, and the harness's `expand/Cargo.toml` requirement.
7. **Writes a report** to `docs/migrations/<name>-<old>-to-<new>.md` and lists
   every remaining hand-written mention of the old version.

The store files are edited textually rather than re-serialized. Round-tripping
468 records through a TOML writer would reflow every `body = """..."""` block
and lose the comments, burying the migration in a diff nobody can read.

A `narrative/` unit is rewritten in whole and moved in part: every step is
accounted for, and the ones citing the other source are carried through
untouched. A unit with no step citing the source being bumped is not rewritten
at all, so the diff of a bump names only the files the bump changed.

## What the tool does not do

**It does not check prose against code.** A range that shifted is safe; a range
whose *contents* changed still points at real code, but the explanation beside
it may now describe something that is no longer there. Those are listed under
*Records to review* in the report, and reading them is the actual work of a
bump.

**It refuses to discard records.** If a record's lines are all gone — the file
was removed, or the code was deleted — the bump stops:

```
9 record(s) cite lines that no longer exist. Re-cut them by hand against
the new source, or re-run with --allow-orphans to drop them.
```

Re-cutting by hand is usually right. `--allow-orphans` drops them and reproduces
each one's full text in the report, so the prose is recoverable.

**It does not renumber the course track.** A new source file is appended to
`reading_order` at the end and flagged. Where it belongs is a teaching decision.
Neither registry exists for the other two roles: both describe the reference
track.

**It computes no coverage figure for a source that never promised one.** The
`unclaimed` column is present for the `coverage` role and absent for the
others, in the printed plan and in the report.

**It does not regenerate the expansion transcripts.** `serde_derive` and the
three glossary crates are what `expand/` is built from, so moving any of them
changes what the site shows. The bump says so in the checklist; the diff from
`cargo test -p expand -- --ignored` is generated code, and reading it is part
of the migration.

**It does not edit prose.** PLAN.md's measurements and README's description mean
things; the tool lists the lines and leaves them.

## Running one

```
cargo xtask bump 1.0.230 --dry-run     # read the plan first, always
cargo xtask bump 1.0.230
cargo update -p serde_core@1.0.229 --precise 1.0.230
cargo xtask coverage
cargo test --workspace
```

A glossary or narrative source is the same five lines with `--source` and one
more step:

```
cargo xtask bump --source syn 3.0.5 --dry-run
cargo xtask bump --source syn 3.0.5
cargo update -p syn@3.0.3 --precise 3.0.5   # `-p syn` is ambiguous: syn 2 is
                                            # in the graph too
cargo test -p expand -- --ignored      # the expander is built from it
cargo xtask coverage
cargo test --workspace
```

Then work through the checklist at the bottom of the generated report.

`cargo xtask coverage` is the gate that decides whether the bump is finished. It
will fail while a file marked `complete` in its `manifest.toml` has unclaimed lines,
which is the correct outcome: a release that added code has added annotation
work, and the build should stay red until that work is done. It also checks two
manifest pins, for the same reason in both cases — the crate is built from
crates.io rather than from `vendor/`, so nothing else connects the two:

- every example pins the `serde_core` the annotations describe;
- `expand/Cargo.toml` pins each source it builds against exactly, and
  `Cargo.lock` resolved it to that version (D11). A caret requirement here
  would let one `cargo update` build the expander from a `syn` the glossary
  does not describe.

## When the tool gives up

A file whose changed region is larger than eight million diff cells is rejected
rather than remapped. At that scale there is no meaningful line correspondence
left to recover, and a remapper would produce plausible-looking ranges over
unrelated code. Re-cut that file's annotations by hand.

## Verification

The remapper's guarantees are unit-tested in `core/src/remap.rs`, including the
one the coverage gate depends on: a tiling partition stays tiling across
insertions, deletions, and in-place rewrites.

End to end, the pipeline was exercised against real releases. For the coverage
source: `1.0.229 → 1.0.228` and back, which returns every annotation file, the
vendored tree, the registries and the example manifests to byte-identical
state, with coverage at 100% at both ends; and `1.0.220 → 1.0.229` as a dry
run, which is the hard shape — a file added, a file removed, and 11 annotations
whose contents moved.

For the other two roles: `syn` `3.0.3 → 3.0.5` was performed, the first bump of
a glossary source; and `serde_derive` `1.0.229 → 1.0.228` was dry-run, which
moves eight narrative steps across five files and leaves all nine `serde_core`
crossings where they are.
