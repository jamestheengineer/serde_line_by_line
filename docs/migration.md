# Bumping the pinned `serde_core`

Every annotation in this repo is keyed to `(file, line-range)` in
`vendor/serde_core-<version>/`. That is what makes "every line" a checkable
property rather than a slogan — and it is what makes an ordinary dependency
update dangerous here. Swap the source under the store and nothing fails to
compile: the coverage gate still counts 12,037 claimed lines, the site still
builds, and several hundred explanations quietly describe whatever code now
occupies those line numbers.

So a bump is a migration, and it has a tool.

```
cargo xtask bump <version> [--dry-run]
```

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
6. **Rewrites everything that names the version**: each `annotations/*.toml`
   header and its `lines` values, `manifest.toml`, `course.toml`, `pin.toml`,
   `vendor/NOTICE.md`, and each example's `serde_core = "=x.y.z"`.
7. **Writes a report** to `docs/migrations/<old>-to-<new>.md` and lists every
   remaining hand-written mention of the old version.

The annotation files are edited textually rather than re-serialized. Round-
tripping 468 records through a TOML writer would reflow every `body = """..."""`
block and lose the comments, burying the migration in a diff nobody can read.

## What the tool does not do

**It does not check prose against code.** A range that shifted is safe; a range
whose *contents* changed still points at real code, but the explanation beside
it may now describe something that is no longer there. Those are listed under
*Annotations to review* in the report, and reading them is the actual work of a
bump.

**It refuses to discard annotations.** If a record's lines are all gone — the
file was removed, or the code was deleted — the bump stops:

```
9 annotation(s) claim lines that no longer exist. Re-cut them by hand against
the new source, or re-run with --allow-orphans to drop them.
```

Re-cutting by hand is usually right. `--allow-orphans` drops them and reproduces
each one's full text in the report, so the prose is recoverable.

**It does not renumber the course track.** A new source file is appended to
`reading_order` at the end and flagged. Where it belongs is a teaching decision.

**It does not edit prose.** PLAN.md's measurements and README's description mean
things; the tool lists the lines and leaves them.

## Running one

```
cargo xtask bump 1.0.230 --dry-run     # read the plan first, always
cargo xtask bump 1.0.230
cargo update -p serde_core --precise 1.0.230
cargo xtask coverage
cargo test --workspace
```

Then work through the checklist at the bottom of the generated report.

`cargo xtask coverage` is the gate that decides whether the bump is finished. It
will fail while a file marked `complete` in `manifest.toml` has unclaimed lines,
which is the correct outcome: a release that added code has added annotation
work, and the build should stay red until that work is done. It also checks that
every example pins the same version the annotations describe — the examples
build against the published crate rather than the vendored copy, so nothing else
connects the two.

## When the tool gives up

A file whose changed region is larger than eight million diff cells is rejected
rather than remapped. At that scale there is no meaningful line correspondence
left to recover, and a remapper would produce plausible-looking ranges over
unrelated code. Re-cut that file's annotations by hand.

## Verification

The remapper's guarantees are unit-tested in `core/src/remap.rs`, including the
one the coverage gate depends on: a tiling partition stays tiling across
insertions, deletions, and in-place rewrites.

End to end, the pipeline was exercised against real releases: `1.0.229 →
1.0.228` and back, which returns every annotation file, the vendored tree, the
registries and the example manifests to byte-identical state, with coverage at
100% at both ends; and `1.0.220 → 1.0.229` as a dry run, which is the hard shape
— a file added, a file removed, and 11 annotations whose contents moved.
