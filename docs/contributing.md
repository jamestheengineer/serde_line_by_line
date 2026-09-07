# Contributing

The expensive artifact in this repository is the prose. Everything else — the
schema, the coverage gate, the site generator — exists to keep roughly 1,200
explanations honest and to stop them from welding themselves to the first UI we
tried. Read [`annotation-style.md`](annotation-style.md) before writing content
and [`decisions.md`](decisions.md) before changing how any of it is built.
Borrowed vocabulary has its own conventions: [`glossary-style.md`](glossary-style.md).

## Setup

```
git clone https://github.com/jamestheengineer/serde_line_by_line
cd serde_line_by_line
git config core.hooksPath .githooks     # runs the CI gates before each push
cargo xtask coverage                    # should report 100.0% and exit 0
```

For the browser modules — the example playground and the derive expander —
you also need:

```
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127   # must match playground/Cargo.toml
```

The version has to match the pin exactly. `cargo xtask wasm` checks it up front
and names both versions if it does not, because a mismatch otherwise surfaces as
a confusing failure deep in the generated glue (decision D1).

Then:

```
cargo xtask wasm    # build the playground and the expander
cargo site          # generate site/
cargo dev           # serve it at http://127.0.0.1:8080
```

`cargo site` works without the wasm toolchain. Examples then render as
unavailable rather than failing the build, which is deliberate — writing prose
should not require a wasm target.

## The one rule about vendored source

`vendor/serde_core-<version>/` is third-party code under MIT/Apache-2.0.
**Never edit it.** Annotations are keyed to line numbers in that tree; a
one-line edit silently shifts every annotation below it. The checksum gate in
`cargo xtask coverage` exists to make this impossible to do by accident.

A version bump is a migration, not a routine change, and it has a tool:
`cargo xtask bump <version>`. Read [`migration.md`](migration.md) before
running one — the ranges move mechanically, the prose does not.

## Adding an annotation

Annotations are data, not pages. They live in `annotations/<file>.toml`, one
file per vendored source file, each record claiming a closed line range:

```toml
[[annotation]]
id       = "ser-mod-0007"          # <file-slug>-NNNN, unique repo-wide
file     = "src/ser/mod.rs"        # relative to the vendored crate root
lines    = "310-318"               # closed range, or a single line: "42"
title    = "Serializer::Ok — what a format produces"
kind     = "trait-item"            # trait-item | macro-def | macro-use | impl
                                   # | doc-contract | plumbing | cfg-gate
tracks   = ["reference", "course"]
course_unit   = "03-associated-types"   # must exist in course.toml
rust_features = ["associated-types"]    # vocabulary: docs/rust-features.md
examples = ["ok_and_error"]             # directory names under examples/
prereqs  = ["ser-mod-0001"]             # annotation ids; must be acyclic
body = """
Markdown. Two to twelve lines. See annotation-style.md.
"""
```

Two constraints the gate enforces and the schema does not:

- **Ranges within a file may not overlap**, and once the file is listed under
  `complete` in `annotations/manifest.toml`, they must cover every line. Gaps in
  an incomplete file are a warning; gaps in a complete one fail CI.
- **`macro_def` is required on `kind = "macro-use"` and rejected everywhere
  else.** It is what lets the renderer collapse a run of invocations under one
  heading. `de/impls.rs` has 106 of them; without this it would read as 106
  near-identical paragraphs.

Then run `cargo xtask coverage`. It checks vendor integrity, gaps and overlaps
with file:line precision, that every `examples` / `prereqs` / `course_unit` /
`rust_features` reference resolves, and that the prereq graph is acyclic.

## Adding an example

Examples are real crates, compiled and run in CI, so an explanation cannot drift
from what the code does. Four files:

```
examples/<name>/
├── Cargo.toml          # publish = false; edition and license from the workspace
├── src/lib.rs          # pub fn run() -> String
├── expected.txt        # the committed transcript
└── tests/expected.rs   # asserts run() against it
```

Rules, in order of how easy they are to forget:

1. **Return `String`, never print.** This is what lets identical code run under
   `cargo test` and inside the browser playground. An example that prints has no
   output in the browser at all.
2. **Register it in `playground/Cargo.toml`.** `build.rs` generates the dispatch
   table from the `examples/` directory listing, so a missing dependency entry
   fails the build naming the crate — but it fails, and it is the step people
   miss.
3. **It must build for `wasm32-unknown-unknown`.** No `std::time`, no file or
   network access, no threads. CI builds every example for the browser target.
4. **Copy `tests/expected.rs` from an existing example.** It carries an
   `#[ignore]`d `regenerate` test; `cargo test -p <name> -- --ignored` rewrites
   `expected.txt` from the current behavior. Read the diff before committing it
   — that test is the only thing standing between a wrong explanation and a
   green build.

`cargo xtask wasm` finishes by running every example out of the built module
under node and diffing it against `expected.txt`, so an example that formats
differently on wasm32 fails the build rather than quietly printing something
else on the site. That check is skipped, with a message, if node is not
installed.

Examples that must *fail* to compile — borrow errors, lifetime mismatches — are
not separate crates. They are `.rs` files under
`examples/compile_fail/tests/ui/`, checked with `trybuild` against a committed
`.stderr`. Regenerate those with
`TRYBUILD=overwrite cargo test -p compile_fail --test ui`, and read the new
diagnostic: the point of the case is the error text, so a changed message is a
content change, not a refresh.

## Adding or editing a course unit

Units live in `annotations/course.toml`. A unit is content in its own right —
the framing that turns a set of annotations into a lesson — and it carries a
`supplement` field with three honest values: `none` (taught entirely from
serde_core), `partial`, and `full` (the crate does not exercise this at all).
The UI labels the last two. PLAN.md §2 commits to that labelling, and it is the
thing that keeps the teaching claim credible: serde_core teaches lifetimes and
trait design superbly and teaches ordinary ownership not at all.

Annotations join a unit by setting `course_unit`, not by being listed in the
unit. Ordering within a unit comes from `reading_order` in `course.toml` plus
line order within each file.

## Adding or editing a narrative unit

The derive track lives in `narrative/*.toml`, one unit per file, and the file
name is the unit id — filename order is reading order, so there is no registry.
[`docs/narrative-style.md`](narrative-style.md) is the full guide; the three
things that differ most from an annotation:

- A step **cites** rather than claims. Ranges may overlap, no file is ever
  declared complete, and no coverage number is computed over `serde_derive`
  ([PLAN.md §11](../PLAN.md)).
- A step that crosses into `serde_core` must name the reference-track annotation
  it lands inside, and the gate checks the containment.
- `emits` — generated code a step claims its citation produces — is verified
  against a real expansion by `cargo site`, which renders the expansion's own
  lines rather than the string in the store ([D10](decisions.md)).

## The gates

`.github/workflows/ci.yml` and `.githooks/pre-push` run the same list, and the
two files reference each other so neither is edited alone:

| gate | catches |
|---|---|
| `cargo fmt --all --check` | the failure that has actually broken this repo |
| `cargo clippy --workspace --all-targets -- -D warnings` | lints |
| `cargo test --workspace` | every example against its transcript, trybuild cases |
| `cargo xtask coverage --json` | vendor integrity, gaps, overlaps, dangling refs, cycles |
| `cargo site` | the site builds, every internal link resolves, and every narrative `emits` claim still matches a real expansion |
| wasm build + `cargo xtask wasm` | every example compiles for the browser **and** produces the same output there as it does natively |

The hook stops at the first failure, because one broken gate cascades into the
rest and five screens of fallout buries the line that has to be fixed. Bypass it
with `git push --no-verify` when that is genuinely what you want — a WIP branch,
a known-broken bisect point.

`.github/workflows/pages.yml` deploys `site/` to the public URL on every push to
`main`. It is a deploy, not a gate; the one check it repeats is the coverage
gate, because a store with a gap reaching the published site is worse than one
reaching a branch (decision D6).

## Commits

One logical change per commit. The message says what changed and why, in
sentences — the repository's history is part of its documentation, and several
decisions are only recorded in it. Measurements go in the message or in
`docs/decisions.md`, not in a comment on the code they justify.

## Licensing

Contributions are dual-licensed **MIT OR Apache-2.0**, matching the project and
the vendored crate. By contributing you agree your work is licensed that way.

## Expansion cases

`expand/` runs the pinned `serde_derive` over the inputs in `expand/cases/`,
natively and in the browser, and both are diffed against `expand/expected.txt`.

To add a case:

1. Write the input as `expand/cases/<name>.rs` — just the item, with its
   `#[serde(...)]` attributes. No `#[derive]` line: the harness applies both.
2. Add a matching `[[case]]` to `expand/cases.toml` saying what it demonstrates.
   The site build fails if a case has no prose or prose has no case.
3. Regenerate the transcript and **read the diff**:

   ```
   cargo test -p expand -- --ignored
   ```

4. `cargo xtask wasm` re-checks that the browser produces the same bytes.

The transcript is generated output, but it is reviewed like written content.
It is the only record of what `serde_derive` actually emits, and a diff nobody
reads is a diff that can hide a change in what the site teaches.

### Why `expand/build.rs` patches a copy

`serde_derive` is a proc-macro crate; its modules are private and a proc-macro
crate cannot be linked as an ordinary dependency. The build copies the pinned
tree into `OUT_DIR` and applies five named edits to `lib.rs`, plus a mechanical
`crate::` → `crate::serde_derive_lib::` rewrite. `vendor/` itself is never
touched — it is checksum-pinned, and that pin is what makes every line range in
this repo trustworthy.

Each edit must apply. If upstream changes shape, the build fails naming the
edit that no longer matches, rather than quietly producing an expander that is
subtly not `serde_derive`.
