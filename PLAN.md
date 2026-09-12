# serde_line_by_line — Project Plan

A web application that walks a reader through **every line** of the `serde_core`
crate, with explanations side-by-side with the source, and runnable micro-examples
that let the reader step through real code.

Secondary objective: teach Rust feature-by-feature using serde_core as the
worked example.

---

## 1. Target: what we are annotating

Pinned, vendored, and checksum-verified against crates.io:

| | |
|---|---|
| crate | `serde_core` |
| version | **1.0.229** (exact pin — annotations reference line numbers) |
| sha256 | `67dca2c9c51e58a4791a4b1ed58308b39c64224d349a935ab5039aa360942a48` |
| license | MIT OR Apache-2.0 (both texts retained in `vendor/`) |
| upstream | https://github.com/serde-rs/serde |
| size | 19 source files, **12,037 lines**, 3,546 of them doc comments (29%) |
| dependencies | **none** |

`serde_core` was chosen over `serde` deliberately. It is the crate that actually
contains the traits and impls; `serde` itself is a 5-file facade whose compiled
code is 94% derive-support plumbing. serde_core also has **zero dependencies**,
so the reader never has to leave the tree to understand something.

> Note: `vendor/serde_core-1.0.229/src/` is third-party code under MIT/Apache-2.0.
> It is vendored, never modified. All project-authored content lives in
> `annotations/`, `examples/`, `app/`, and `xtask/`.

### Measured shape of the codebase

This drove the whole plan, so it is recorded here rather than in a commit message.

| file | lines | doc | code | items | macro defs | macro uses | est. units | profile |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| `de/impls.rs` | 3,173 | 52 | 2,655 | 322 | 20 | 106 | 408 | macro-driven |
| `de/mod.rs` | 2,392 | 1,396 | 831 | 147 | 1 | 8 | 147 | doc-heavy |
| `ser/mod.rs` | 2,010 | 1,616 | 304 | 93 | 1 | 5 | 111 | doc-heavy |
| `de/value.rs` | 1,895 | 72 | 1,551 | 235 | 2 | 43 | 251 | macro-driven |
| `ser/impls.rs` | 1,045 | 51 | 889 | 86 | 10 | 59 | 130 | macro-driven |
| `de/ignored_any.rs` | 238 | 103 | 113 | 19 | 0 | 1 | 19 | doc-heavy |
| `macros.rs` | 230 | 105 | 119 | 4 | 3 | 30 | 12 | doc-heavy |
| `ser/impossible.rs` | 216 | 51 | 139 | 31 | 0 | 0 | 31 | logic |
| `crate_root.rs` | 171 | 3 | 136 | 10 | 2 | 1 | 10 | logic |
| `ser/fmt.rs` | 170 | 18 | 133 | 28 | 1 | 1 | 28 | logic |
| `private/doc.rs` | 165 | 0 | 154 | 21 | 4 | 33 | 21 | logic |
| `lib.rs` | 121 | 35 | 59 | 3 | 1 | 1 | 6 | logic |
| `std_error.rs` | 48 | 41 | 6 | 2 | 0 | 0 | 2 | doc-heavy |
| `private/content.rs` | 39 | 0 | 26 | 1 | 0 | 0 | 2 | logic |
| `format.rs` | 30 | 0 | 26 | 3 | 0 | 0 | 3 | logic |
| `private/size_hint.rs` | 30 | 0 | 26 | 4 | 0 | 0 | 4 | logic |
| `private/string.rs` | 23 | 0 | 11 | 2 | 0 | 0 | 2 | logic |
| `private/mod.rs` | 21 | 0 | 16 | 5 | 0 | 0 | 5 | logic |
| `private/seed.rs` | 20 | 3 | 15 | 3 | 0 | 0 | 3 | logic |
| **total** | **12,037** | **3,546** | | | **45** | **288** | **~1,195** | |

Three facts that make "every line" tractable:

1. **29% of the crate is already doc comments** written by dtolnay. `ser/mod.rs`
   is 80% docs, `de/mod.rs` 58%. For the two most conceptually important files,
   the job is *annotating and contextualizing existing prose*, not writing from
   scratch.
2. **45 `macro_rules!` definitions generate 288 invocations.** `de/impls.rs` is
   3,173 lines but has only ~20 unique macro bodies. Explain a macro once, then
   each of its 106 invocations costs a line of reference, not a paragraph.
3. The real unit of work is **~1,195 annotations averaging ~10 lines each**, not
   12,037 individual line comments.

---

## 2. Honest scope note on the Rust-teaching objective

Feature usage measured across the crate:

| exercised heavily | count | barely present / absent | count |
|---|---:|---|---:|
| lifetime parameters (`'de`) | 724 | `unsafe` | 2 |
| `where` clauses | 519 | `dyn` trait objects | 10 |
| `#[cfg]` conditional compilation | 315 | higher-ranked trait bounds | 2 |
| associated types | 180 | const generics, GATs | 0 |
| default trait methods | 157 | async, threads, channels | 0 |
| `PhantomData` | 107 | closures, iterator chains | minimal |
| `macro_rules!` | 45 | interior mutability | 0 |

**serde_core is a world-class teacher of:** lifetimes and borrow-region
reasoning, trait design, associated types, generic bounds, blanket impls,
`macro_rules!`, `no_std` / feature-gate engineering, and zero-cost abstraction.

**It will teach almost nothing about:** ownership in ordinary imperative code,
error-handling idioms, iterators and closures, concurrency, `async`, `unsafe`,
interior mutability, or collections.

Consequence for the plan: the **course track cannot ride on serde_core alone.**
Roughly a third of the course units need supplementary micro-examples written
specifically to cover those gaps (§7). This is designed in from the start rather
than discovered in month three.

---

## 3. Architecture decisions

| decision | choice | why |
|---|---|---|
| App form | **Web app, custom reader** — static site generator, Askama templates, Axum dev server ([D3](docs/decisions.md)) | Full control over the three-pane layout and narrative flow; deployable so others can read it; matches the existing Rust web stack. |
| Example execution | **Both** — WASM for deploy, local cargo for development | Real computed output in the browser with no backend; real rustc errors and debugger stepping locally. |
| Content sequencing | **Tracks in parallel** | Reference track guarantees 100% coverage; course track serves the teaching objective. One annotation store, two orderings. |
| Source handling | **Vendored + checksum-pinned** | Annotations are line-range keyed. Upstream drift must be an explicit migration, never a silent break. |

### The load-bearing decision: annotations are data, not pages

Annotations live in `annotations/<crate>/*.toml` as records keyed to
`(file, line-range)`.
They are **not** hand-written HTML or Markdown pages.

This buys three things:

- **Coverage is computable.** A tool can prove every line is claimed (§5).
- **Two tracks for free.** Reference order and course order are two queries over
  one dataset, not two copies of the content.
- **The renderer is swappable.** If the web app turns out to be the wrong shape,
  a VS Code extension or mdBook output can be generated from the same store
  without rewriting a single explanation. The content is the expensive artifact;
  it must not be welded to the first UI we try.

---

## 4. Annotation schema

`annotations/serde_core/ser_mod.toml`:

```toml
schema  = 1
source  = "serde_core-1.0.229"

[[annotation]]
id       = "ser-mod-0007"
file     = "src/ser/mod.rs"
lines    = "310-318"
title    = "Serializer::Ok — what a format produces"
kind     = "trait-item"        # trait-item | macro-def | macro-use | impl
                               # | doc-contract | plumbing | cfg-gate
tracks   = ["reference", "course"]
course_unit   = "03-associated-types"
rust_features = ["associated-types", "sized-bound"]
examples = ["ok_and_error"]
prereqs  = ["ser-mod-0001"]
body = """
`type Ok` is the value a successful serialization produces. For `serde_json`'s
string serializer it is `()` — output accumulates into a `String` the caller
already owns. For a hash-computing serializer it might be `u64`.

This is the first place serde's central trick appears: the *format* chooses the
type, not serde…
"""
```

Field notes:

- `lines` is a closed range into the pinned source. The coverage tool enforces
  that ranges within a file are non-overlapping and collectively exhaustive.
- `kind` drives rendering. A `macro-use` annotation renders compactly and links
  back to its `macro-def` rather than repeating the explanation — this is what
  keeps `de/impls.rs` from costing 3,174 paragraphs.
- `rust_features` is a controlled vocabulary (`docs/rust-features.md`). It powers
  the course track and a "where else does this appear?" index.
- `prereqs` builds a DAG so the course track can be topologically ordered and the
  reader can be warned when they jump ahead.

---

## 5. Coverage as a CI gate — the mechanism that makes "every line" real

`cargo xtask coverage` is the most important tool in the repo. It:

1. Parses every annotation.
2. Verifies every line of every vendored `.rs` file is claimed by **exactly one**
   annotation. Reports gaps and overlaps with file:line precision.
3. Verifies the vendored tree still hashes to the pinned checksum.
4. Verifies every `examples` / `prereqs` / `course_unit` reference resolves.
5. Verifies the `prereqs` graph is acyclic.
6. Emits `coverage.json` — which drives the progress bar in the UI and the
   README badge.

CI runs it on every push. Gaps are a **warning** until a file is declared
complete in that crate's `annotations/<crate>/manifest.toml`, then a **hard
failure**.

This converts "walk the user through EVERY line" from an aspiration that quietly
decays into a measurable, enforced property. It also makes progress visible,
which matters a great deal on a 1,195-unit project.

---

## 6. Micro-examples

Each example is a **real crate** under `examples/<name>/`:

```
examples/ok_and_error/
├── Cargo.toml
├── src/lib.rs          # pub fn run() -> String
└── expected.txt        # asserted in CI
```

Design rules:

- **Examples return `String`, they do not print.** This is what lets the same
  code run under WASM in the browser and produce real computed output, rather
  than the app displaying a canned answer.
- **CI compiles and runs every example and diffs against `expected.txt`.** An
  explanation can never drift from what the code actually does. This is the
  single highest-value piece of automation after coverage.
- **Every example must build for both `host` and `wasm32-unknown-unknown`.**
- Examples that intentionally *fail to compile* (teaching borrow errors, variance,
  lifetime mismatches) live in `examples/compile_fail/` and are checked with
  `trybuild`, with the expected diagnostic committed.

Local development gets the real thing: `cargo run -p ex_ok_and_error`, real rustc
diagnostics, and `rust-lldb` for stepping. The browser gets pre-compiled WASM.

---

## 7. The reference and course tracks

**Reference track** — the spine. File-by-file, dependency order, 100% coverage.
This is the "every line" promise. Navigation mirrors the source tree.

**Course track** — a curated path through the *same* annotations, ordered by
teaching progression rather than file layout. As shipped, 14 units:

| unit | topic | drawn from | annots | supplement |
|---|---|---|---:|---|
| 01 | Why serialization needs a data model | `ser/mod.rs`, `de/mod.rs` | 11 | — |
| 02 | Traits and supertraits | `ser/mod.rs` | 4 | — |
| 03 | Associated types | `de/value.rs`, `de/mod.rs`, `ser/mod.rs` | 33 | — |
| 04 | Generic bounds and `where` clauses | `de/value.rs`, `ser/impls.rs` | 11 | — |
| 05 | Default methods, and what silence means | `ser/mod.rs` | 9 | — |
| 06 | **Ownership and borrowing** | — | 0 | **fully supplementary** |
| 07 | **Lifetimes I: the basics** | — | 0 | **fully supplementary** |
| 08 | Lifetimes II: `'de` and zero-copy | `de/value.rs`, `de/impls.rs`, `de/mod.rs` | 24 | — |
| 09 | `PhantomData` and variance | `de/value.rs`, `de/impls.rs` | 9 | partial |
| 10 | Blanket impls and coherence | `de/value.rs`, `de/mod.rs` | 12 | — |
| 11 | `macro_rules!` | `de/impls.rs`, `ser/impls.rs`, `macros.rs` | 53 | — |
| 12 | `no_std` and feature gates | `de/impls.rs`, `crate_root.rs` | 22 | — |
| 13 | Errors and control flow | `de/impls.rs`, `de/value.rs` | 12 | partial |
| 14 | Iterators and closures | `de/impls.rs`, `de/value.rs` | 2 | partial |

The sketch this replaced had 12 units. Three things moved during phase 7.
Default methods earned a unit of its own rather than a section inside generic
bounds; the two fully supplementary lifetime and ownership units landed
adjacent, which pushed everything after them down a number; and "errors,
iterators, closures" split in two, because the crate leans on them very
differently. It has a real error design to study — two `Error` traits built by
`declare_error_trait!`, and `tri!` in place of `?` crate-wide — but only three
careful uses of an iterator and thirteen closures in twelve thousand lines. Both
units end up `partial`, but for opposite reasons: unit 13 supplements what the
crate opted out of, unit 14 supplies fundamentals the crate never shows.

Units marked supplementary are where serde_core genuinely does not exercise the
feature (§2). Writing them honestly — rather than pretending serde_core teaches
ownership — is what keeps the secondary objective credible.

---

## 8. Repository layout

```
serde_line_by_line/
├── PLAN.md                     ← this file
├── README.md
├── LICENSE-MIT / LICENSE-APACHE
├── vendor/
│   └── serde_core-1.0.229/     ← pinned, unmodified, MIT/Apache-2.0
├── annotations/
│   ├── course.toml             ← one course track over both annotated crates
│   ├── serde_core/
│   │   ├── manifest.toml       ← per-file completion status
│   │   └── *.toml              ← the content
│   └── serde_derive/           ← the same, from §12 onward
├── examples/
│   ├── <name>/                 ← real crates, CI-verified
│   └── compile_fail/           ← trybuild cases
├── app/                        ← Axum + Askama reader
│   ├── src/
│   ├── templates/
│   └── static/
├── xtask/                      ← coverage, build, wasm pipeline
└── docs/
    ├── rust-features.md        ← controlled vocabulary
    ├── annotation-style.md     ← voice and length conventions
    └── contributing.md
```

---

## 9. Roadmap

| phase | scope | units | exit criteria |
|---|---|---:|---|
| **0 — Foundation** | Schema, `xtask coverage`, vendor pin check, CI, example harness | 0 | `cargo xtask coverage` reports 0/12,037 without error; CI green |
| **1 — Vertical slice** | `ser/mod.rs` fully annotated + three-pane app MVP | 111 | One file at 100%, readable end-to-end in the browser, 5 examples running as WASM |
| **2 — Rest of `ser/`** | `impls.rs`, `impossible.rs`, `fmt.rs` | 189 | `ser/` at 100%; macro-def/macro-use rendering proven on `impls.rs` |
| **3 — `de/` contracts** | `de/mod.rs`, `de/ignored_any.rs` | 166 | The `'de` lifetime story is fully told |
| **4 — `de/value.rs`** | The `IntoDeserializer` machinery | 251 | — |
| **5 — `de/impls.rs`** | The largest file | 408 | Reference track at 100% |
| **6 — Plumbing** | `lib.rs`, `crate_root.rs`, `macros.rs`, `private/*`, `format.rs`, `std_error.rs` | 70 | **Every line claimed. Coverage gate hard-fails on regression.** |
| **7 — Course track** | Ordering, prereq DAG, ~12 supplementary units | — | Course track walkable start to finish |
| **8 — Ship** | Deploy, polish, contribution docs | — | Public URL |

**All eight phases are done**, and so is the derive track that followed them
(§11). The site is live at
<https://jamestheengineer.github.io/serde_line_by_line/>, rebuilt and deployed
from the annotation store on every push to `main` (D6). All three tracks are
complete: 12,037 lines claimed by 468 annotations, 14 course units written, and
9 derive units walking `serde_derive` in 85 steps.

What follows them is **§12**: the reference half of the derive track, which the
walk deliberately did not make. `serde_derive` becomes a second source that
promises every line, and the walk becomes an ordering over it.

Two more gates went in during phase 8. Listed beside the coverage gate from
phase 0, they are what makes the promises in this document checkable rather
than stated:

| gate | what it makes impossible |
|---|---|
| `cargo xtask coverage` | an unclaimed line, an overlap, a dangling reference, a prereq cycle, a course unit that leans on one the reader has not reached |
| the site's own link check | a rendered link that resolves locally and 404s under the deployed path |
| the wasm output check | an example that prints one thing in CI and another on the site |
| the example version pin check | an example running a different `serde_core` release than the annotations describe |
| `cargo xtask bump` | a source version moving without every line range moving with it |

Phase 1 is deliberately a full vertical slice: it forces the schema, the renderer,
the WASM pipeline, and the writing voice to all be proven against real content
*before* 1,000 more annotations are committed to a format that might be wrong.

Phases 2–6 are pure content throughput and can be reordered freely. Phase 5 is
the largest single block and benefits most from the macro-def/macro-use
compression proven in phase 2.

---

## 10. Risks and open questions

| risk | mitigation |
|---|---|
| **Content volume dwarfs engineering.** ~1,195 annotations is the real project; the app is a few weeks. | Keep the renderer cheap and the content portable. Never let UI work block writing. |
| **Line-range brittleness.** Any edit to vendored source invalidates annotations. | Checksum gate in CI. Version bumps are explicit migrations, performed by `cargo xtask bump` ([D7](docs/decisions.md), [migration.md](docs/migration.md)). |
| **Macro-heavy files become tedious.** `de/impls.rs` could read as 106 near-identical entries. | `kind = "macro-use"` renders compactly and links to the def. Prove this in phase 2 before committing to phase 5. |
| **Course track over-claims.** Pretending serde_core teaches all of Rust would be dishonest. | §2 is committed to the repo. Supplementary units are labeled as such in the UI. |
| **Scope creep to `serde` / `serde_derive`.** | Out of scope for v1. The reference track has since hit 100%, so the revisit was done and measured: [`docs/derive-track-scope.md`](docs/derive-track-scope.md). It is a comparable project, not an increment — 15–19 sessions against the 20 this one took. Both of its unknowns are now closed: `serde_derive` expands live in the browser (scope §9), and borrowed `syn` vocabulary is quoted and pinned rather than annotated ([D9](docs/decisions.md)). The narrative half of it shipped as §11; the reference half is now the plan, §12. |

**Resolved in phase 0** — see [`docs/decisions.md`](docs/decisions.md) for the
measurements behind each:

1. **D1 — WASM via `wasm-bindgen`**, one shared playground module, version
   pinned. The raw `wasm32-unknown-unknown` ABI saved only 4.6 KB gzipped and
   required hand-rolled unsafe to pass a `String` across the boundary.
2. **D2 — syntect at build time, class-based, per-annotation-span granularity.**
   A ten-line span renders to 3.8 KB (0.5 KB gzipped). Whole-file rendering was
   the wrong question, and a per-file generator produces spans that cross line
   boundaries, which line-keyed annotations cannot use.
3. **D3 — static site generator** with Askama templates plus an Axum dev server.
   Nothing needs a backend at request time once examples run as WASM and
   highlighting is precomputed.

4. **D4 — no scroll syncing.** The question assumed two panes that can drift
   apart; one annotation is one grid row holding both its code and its prose,
   so there is a single scroller and nothing to sync. Pairing is done with
   hover tints in CSS.

**Resolved in phase 8:**

5. **D5 — the CI gates run in a pre-push hook**, not only on the runner.
6. **D6 — the site deploys to GitHub Pages** from the Actions artifact on every
   push to `main`. `site/` stays uncommitted: it is derived, and a committed
   copy goes stale the first time an annotation lands without a rebuild.

7. **D7 — a version bump remaps boundaries, not ranges**, and rewrites the
   annotation store textually rather than through a serializer. `cargo xtask
   bump <version>` fetches and checksum-verifies the release, aligns every
   file, carries all 468 ranges across, and reports the ones whose code changed
   underneath them. This is the "remapping tool" §10 promised; the runbook is
   [`docs/migration.md`](docs/migration.md).

**Resolved after phase 8:**

8. **D8 — the course track's forward references are a build failure, not a
   warning.** Phase 7 exited on "course track walkable start to finish", and
   the coverage gate quietly disagreed: 33 prereq edges pointed at units the
   reader had not reached, and the renderer showed a *"leans on something the
   course does not reach until …"* notice at each one. Re-ordering units cannot
   fix this — unit 12 depends on unit 11 eight times, so swapping a pair trades
   one violation set for a larger one. The cause was almost always a framing
   annotation parked after the items that need it: the `de::Error` methods sat
   in unit 13 while units 10–12 called them, the `Deserializer` doc-contract sat
   in unit 08 while unit 03 declared the trait, and four macro definitions sat
   in unit 04 while their base macros were in unit 11. 31 annotations moved,
   six unit introductions were rewritten to match, and the check is now a hard
   failure so the count cannot climb back off zero.

**Still open:** one, and §12 opens it — **D12, whether the project reports
one coverage number or two.** `Pin::primary()` currently refuses a second
`coverage` source on the grounds that two crates each promising 100% make
"the" coverage table ambiguous. The reference derive track needs that refusal
lifted, and lifting it is R1's real work: the objection has to be answered in
the reporting, not argued away.

> As of 2026-09-10, `serde_core` 1.0.229 is still the newest published
> release, so there is nothing to bump *to*. The migration path is built and
> exercised — `1.0.229 → 1.0.228` and back returns the tree byte for byte, and
> `1.0.220 → 1.0.229` is verified as a dry run because it is the hard shape: a
> file added, a file removed, eleven annotations whose contents moved.
>
> One of the other four pins did move, and moving it is what made the tool
> role-aware ([D11](docs/decisions.md)). `syn` 3.0.3 → 3.0.5 is the first bump
> of a source that is quoted rather than claimed: `cargo xtask bump --source
> <name>` now migrates any pinned source, and the role decides which store it
> rewrites. Two things fell out of doing it properly. The coverage bump had a
> hole — the narrative's nine crossings into `serde_core` were keyed to a tree
> it would have replaced without retargeting them — and the expansion harness
> was free to build against a `syn` the glossary does not describe, which no
> tree hash can see, because the tree is not what moved.

---

## 11. The narrative derive track

Phases 0–8 answered "what is every line of `serde_core` for?". They left the
question most readers actually arrive with unanswered: **what does
`#[derive(Serialize)]` turn into?**

The measurement for answering it exhaustively is in
[`docs/derive-track-scope.md`](docs/derive-track-scope.md): ~500 annotations
and 15–19 sessions, a project the size of this one. **We are not doing that.**
We are doing §7 of that document — the narrative track.

### What it is

Eight to ten units that follow **one struct and one enum** from a
`syn::DeriveInput` to an emitted `impl`, citing `serde_derive` line ranges as
the path goes through them, and ending with the expansion running live in the
browser.

It is a **narrative source**, and that word is load-bearing. `serde_derive` is
not declared complete in `manifest.toml`, no file of it is claimed exhaustively,
and the coverage gate never asks it to be. The front page keeps its promise by
naming what each source is:

> Every line of every *annotated* crate is claimed. `serde_derive` is walked,
> not claimed. Borrowed vocabulary is quoted, pinned, and named as borrowed
> ([D9](docs/decisions.md)).

Coverage over a narrative source would be a lie told in a number, and the
number is the thing this project has been careful about from phase 0.

### Why this shape and not the big one

The 500-annotation version commits the project to a second 100% it defends on
every bump, for a crate whose interesting content is not evenly distributed: 43
attribute parsers in `internals/attr.rs` are worth one unit between them, and
`de/identifier.rs` is worth three on its own. A narrative picks the path
through, and the path is what a reader wants. If it lands well, phases 2–6 of
the big version are still available, and the machinery below is exactly what
they would need.

### Roadmap

| phase | scope | exit criteria |
|---|---|---|
| **N1 — Multi-source foundation** ✅ | `pin.toml` grows from one pin to a list of sources with roles; vendor `serde_derive` 1.0.229, `syn` 3.0.3, `quote` 1.0.47, `proc-macro2` 1.0.107; generalize `vendor.rs`, `coverage.rs`, `bump.rs` | `cargo xtask coverage` still reports `serde_core` at 12,037/12,037 and now verifies five pinned trees; a role a gate does not know is a build failure |
| **N2 — Glossary** ✅ | [D9](docs/decisions.md): `glossary/*.toml`, the `glossary` field on annotations, renderer support | 62 entries, every citation resolves, a quoted definition that drifts from its pinned tree fails the gate |
| **N3 — Expansion harness** ✅ | The [§9](docs/derive-track-scope.md) patch applied by `expand/build.rs`, native golden transcripts, a second wasm module fetched only on derive pages | An expansion on the site is byte-identical to the host's, and the generated code is asserted to carry the pinned version |
| **N4 — The narrative** ✅ | 8–10 units, one struct and one enum, end to end | Walkable start to finish; every cited range resolves; no forward references (the D8 check, across two sources) |
| **N5 — Ship** ✅ | Track navigation, the restated promise, `README` | The three tracks are each reachable and each honest about what they claim |

N4 shipped as nine units and 85 steps, citing 1,875 lines across the two pinned
sources, nine of them crossings into annotated `serde_core`. Thirty steps quote
the code their citation emits, and those quotations are verified against a real
expansion at build time rather than stored — [D10](docs/decisions.md).

N1 is deliberately first and deliberately boring: every later phase needs a
source that is not `serde_core`, and the pin is the thing that has protected
every line range in this repo since phase 0. It does not get loosened to make
room.

### What N5 shipped

The header separates the three tracks from the two tools they lean on, and each
track index ends by naming where the reader goes next rather than sending them
back to the header. Reference → derive is the crossing that matters: it is the
one a reader arrives wanting and the one the first eight phases could not make.

The front page now restates the promise in the form §11 asked for, with the
numbers counted during the build rather than typed:

> Every line of every *annotated* crate is claimed — that is `serde_core`, and
> the coverage gate fails the build over one unclaimed line. `serde_derive` is
> walked, not claimed: 8,975 lines of territory, 1,875 of them cited where the
> story goes through them, no file declared complete, and no percentage,
> because a percentage over a walk would be a lie told in a number. Borrowed
> vocabulary is quoted, pinned, and named as borrowed — 62 entries, 1,123
> quoted lines, never an annotation ([D9](docs/decisions.md)).

Those three sentences are one per **role** in `vendor/pin.toml`, which is the
point of the roles: what a source promises is a field in the pin, the gates
read it, and a role no gate knows fails the build. The README says the same
thing as a table, so the promise reads identically in both places a reader
might check it.

> Restated in §12's R1. The middle sentence is gone, because the thing it
> described is gone: `serde_derive` stopped being walked-not-claimed and became
> the second crate the reference track claims. The mechanism it demonstrates
> did not change — the promise is still one sentence per role, still read off
> the pin, and there are simply two rows under `coverage` now.

---

## 12. The `serde_derive` reference track

[`docs/derive-track-scope.md`](docs/derive-track-scope.md) measured two shapes
and §8 left the choice to appetite. §11 built the smaller one. **This is the
larger one**: §6 of that document — every line of `serde_derive` claimed by
exactly one annotation, the same promise the first eight phases made about
`serde_core`, defended by the same gate.

### What the narrative track becomes

Not deleted, and not left beside it. `serde_derive` stops being a `narrative`
source and becomes a second `coverage` source, and the moment it does, all 85
narrative steps are citing annotated ground. The narrative becomes what the
course track already is: **a curated ordering over the annotation store**,
which is what §3 said tracks were from the beginning — two orderings, one
dataset, not two copies of the content.

That is a real conversion, not a re-label. `check_narrative` already requires a
step citing *the* coverage source to name the reference-track annotation
containing it; nine steps do that today. After R5 all 85 must, and the
annotations they name do not exist yet. The retargeting happens per file, as
each file is declared complete.

### The measurement, re-taken

Against the vendored tree as pinned, not the projection:

| group | files | lines | lines/annot | annotations |
|---|---:|---:|---:|---:|
| codegen — `ser.rs`, `de.rs`, `de/*.rs` | 11 | 4,728 | 14 | ~338 |
| attributes and validation — `internals/*` | 9 | 3,015 | 28 | ~108 |
| plumbing — `bound.rs`, `receiver.rs`, `pretend.rs`, … | 8 | 1,232 | 22 | ~56 |
| **total** | **28** | **8,975** | **18** | **~502** |

Two numbers decide the shape of the work, and both confirm §2 of the scope doc:

- **271 `quote!` / `quote_spanned!` invocations** — 76 in `ser.rs`, 53 in
  `de/struct_.rs`, 49 in `de.rs`. Each emits different code and needs its own
  explanation, next to what it emits.
- **2 `macro_rules!` in the whole crate**, both in `fragment.rs`, against 45 in
  `serde_core`. The `macro-def` / `macro-use` compression that held
  `de/impls.rs` to 122 annotations over 3,173 lines **does not exist here**.
  Nothing else will take its place; the 502 is the real number.

### The engineering delta the scope doc did not have to name

§4 listed four deltas and N1–N3 discharged three of them. The fourth —
"a cross-crate course DAG" — is half a session and still stands. But the
narrative track never needed a second *annotation store*, so nothing forced
this out into the open until now:

**The store is single-source, and says so on purpose.** `core::load` reads
`pin.primary()`, one `annotations/manifest.toml`, one `annotations/course.toml`,
and requires every annotation file's `source` to match one id.
`Pin::primary()` does not merely assume one coverage source, it **hard-fails on
two**, with the reason written into the code:

> exactly one is allowed, because two crates each promising 100% would make
> "the" coverage table ambiguous with no reader-visible gain.

R1 overturns that, and it is a decision to be argued (D12), not a refactor to
be performed quietly. The gain is now reader-visible — it is the second track —
but the objection was never wrong: there is no such thing as "the" coverage
number any more. Percentages become per-source everywhere they are reported,
`coverage.json` grows a level, and the front page states two, because one
number spanning 21,012 lines of two crates with different promises would be
exactly the lie §11 refused to tell about the walk.

### Roadmap

| phase | scope | annots | exit criteria |
|---|---|---:|---|
| **R1 — Two coverage sources** ✅ | `Pin::primary` retires; `Store`, `manifest.toml` and `coverage.json` become per-source; D12 | 0 | `cargo xtask coverage` reports `serde_core` 12,037/12,037 **and** `serde_derive` 0/8,975 without error; every existing gate still fires on the first source; a role no gate knows is still a build failure |
| **R2 — Vertical slice: `ser.rs`** ✅ | The densest codegen file, 1,369 lines and 76 `quote!` blocks, and the one the narrative already walks | 96 | `ser.rs` at 100%; the `codegen` kind proven in the renderer; an annotation's `emits` checked against a real expansion at annotation granularity (D10); its narrative steps retargeted |
| **R3 — The rest of codegen** ✅ | `de.rs`, then `de/struct_.rs`, `de/tuple.rs`, `de/unit.rs`, `de/identifier.rs`, and the four enum representations | 165 | codegen at 100%; the four representations read as four variations, not four transcripts |
| **R4 — `internals/`** ✅ | `attr.rs` (1,818 lines, the attribute DSL), `check.rs`, `ast.rs`, `case.rs`, `name.rs`, `symbol.rs`, `ctxt.rs`, `respan.rs`, `mod.rs` | 102 | the `#[serde(...)]` surface is claimed, including what upstream rejects and why |
| **R5 — Plumbing** ✅ | `bound.rs`, `receiver.rs`, `pretend.rs`, `lib.rs`, `fragment.rs`, `deprecated.rs`, `this.rs`, `dummy.rs` | 39 | **every line of `serde_derive` claimed**; all 28 files in its manifest; the gate hard-fails on regression; all 85 narrative steps name a containing annotation |
| **R6 — Course units** ✅ | 7 new units: proc-macro basics, spans, `syn`'s AST, `quote!` interpolation, the attribute DSL, generating an impl, the four enum representations; the cross-crate prereq DAG | — | the course track spans both crates, walkable start to finish, D8's forward-reference check enforcing across sources |
| **R7 — Ship** | Navigation for four reference file-trees' worth of pages, the restated promise, `README` | — | both reference tracks reachable and each honest about what it claims |

### What R1 shipped

The refusal in `Pin::primary()` is gone and the ambiguity it named is gone with
it: no figure anywhere in the project spans the two crates, and the front page
says so where a reader would go looking for the missing one. `Store` and
`Report` became lists, `percent()` moved off both parents onto the children,
and `annotations/` became `annotations/<crate>/` so that a bump physically
cannot open the other store. The full reasoning, and the three things that
fell out of it, are [D12](docs/decisions.md).

Two of those are worth naming here because they change what the later phases
do. The gate learned a third state — a file with no annotation at all is
*counted*, not warned about, so `cargo xtask coverage` does not print 28
warnings for the 28 files R2 through R5 will write. And the narrative's
crossing rule moved from per source to **per file**: a step must name the
annotation containing it once its file is declared complete, so the walk
converts one file at a time, with the gate turning compulsory exactly when
there is something to point at. `course.toml` did not move and did not need
per-source anything — the course track is one track over both crates, which is
R6's problem and not R1's.

R2 is a vertical slice for the same reason phase 1 was: it proves the `codegen`
kind, the `emits` verification at annotation granularity, and the writing voice
against real content *before* 400 more annotations are committed to a shape
that might be wrong. `ser.rs` is the right file for it because the narrative
already walks it, so R2 is also the first retargeting and will find whatever
the conversion above actually costs.

### What R2 shipped

96 annotations, 1,369 lines, `src/ser.rs` declared complete. Four things came
out of doing it that R3 through R5 inherit.

**The `codegen` kind, and what it renders.** 79 of the 96 are `codegen`; the
other 17 are `plumbing`. 25 of them carry an `emits` claim, which is D10's
mechanism moved from the narrative onto a reference-track annotation: the store
holds a *locator*, `cargo site` finds it in a real expansion and renders the
expansion's bytes, and the page does not build if `serde_derive` stops emitting
it. Both tracks now read one expansion cache, so they cannot disagree about what
the crate emits.

**Two more committed cases.** `enum_externally_tagged` reaches all four arms of
`effective_style` in one input, and `enum_adjacently_tagged` is the one shape
whose `Serialize` side generates a helper type. Between them they took the file
from "most of these annotations cannot show their output" to a quarter that can.

**The estimate held, slightly under.** 96 against the projected 98, at 14.3
lines each against the projected 14. That is one file, and `internals/attr.rs`
is still the density risk — but the compression this file did *not* get is the
one the scope doc predicted, so there is no reason yet to move the 502.

**The narrative converted, and the ratchet worked.** Declaring `ser.rs` complete
turned exactly 13 gate errors on, one per step citing the file, and each was
fixed by naming the annotation it lands in. Five of the thirteen did not fit
inside any single annotation, which is the useful signal: four were annotation
boundaries drawn too finely for a stop the walk had already justified as one
idea, and they were merged; one was a step whose prose had outgrown what it
cited, and it was rewritten. R3 through R5 should expect the same ratio.

### What R3 shipped

The codegen group is complete: 11 files, 4,728 lines, 261 annotations including
R2's. `serde_derive` is at 52.7%.

**The density came in under the projection, and the estimate should move.**
The scope doc projected codegen at 14 lines per annotation and ~338
annotations; it landed at **18.1 and 261**. Not because anything was skipped —
every line is claimed and the file-by-file checks all pass — but because the
four enum representations compress against each other far better than the
`quote!` count suggested. `enum_untagged::deserialize_variant` is called by
three of the four; `struct_::deserialize` reads the fields of a struct variant
identically whatever encloses it; `deserialize_seq` serves tuples and structs
both. Explaining that sharing once is cheaper than explaining four transcripts,
which is exactly the exit criterion this phase was given.

Carried forward at the projected densities, the whole track now looks like
**~425 annotations rather than ~502** — within the scope doc's own stated lower
bound of 450 only if `internals/` runs dense, and below it otherwise. The 502
was never a promise; the coverage gate is. But R4 is where the remaining
uncertainty lives, because `internals/attr.rs` is 1,818 lines and the doc's
upper bound of 580 assumed it would refuse large spans.

**The narrative ratchet held again**, at a better ratio than R2's. Declaring
nine files complete turned on 19 gate errors, and 18 of the 19 steps already sat
inside a single annotation — one pair had to merge. R2's five-out-of-thirteen
was the vertical slice finding the boundaries; R3 suggests the boundaries are
now roughly right.

### What R4 shipped

`internals/` is complete: 9 files, 3,015 lines, 102 annotations.
`serde_derive` is at 86.3%, with only the eight plumbing files left.

**The density risk the scope doc flagged did not materialize, and the reason is
the opposite of what was feared.** It projected `internals/` at 28 lines per
annotation and warned the estimate could reach 580 if `attr.rs` "refuses to take
large spans". `attr.rs` took the *largest* spans in the project — 56 annotations
over 1,818 lines, 32.5 each — because it is three several-hundred-line
`if`/`else` chains over attribute names, and the interesting content is the
group an attribute belongs to rather than the individual branch.
`#[serde(from)]`, `try_from`, `into` and `remote` are one annotation, not four,
because what is worth saying is that all four parse a string into a type so a
malformed one fails under the attribute.

Carried forward, the track lands near **~420**, which holds R3's revision rather
than moving it again.

**The ratchet held at R3's ratio.** Nine more files complete, 29 gate errors,
and four steps did not fit: one annotation pair merged, one boundary shifted,
and two steps whose cited range had drifted past the arm they describe were
narrowed to it.

### What R5 shipped

**`serde_derive` is at 100%.** 28 files, 8,975 lines, 403 annotations, every
file named in `annotations/serde_derive/manifest.toml`, and the gate hard-fails
on a gap from here on — the same promise `serde_core` has carried since phase 6,
now made about a second crate.

**And the conversion R1 designed is complete.** All 85 narrative steps name the
annotation they land in; the report's crossings column reads 8, 9, 10, 7, 9, 10,
9, 12, 11 — every step in every unit. The walk that was a track with its own
citations is now an ordering over the annotation store, which is what PLAN.md §3
said a track was, and it got there one file at a time with the rule turning
compulsory exactly when there was something to point at. No flag day, and no
window where the rule was suspended.

Across R2–R5 the ratchet fired 66 times and 13 steps needed work: nine
annotation boundaries merged, one shifted, and three steps narrowed to the code
their prose actually describes. Five of those thirteen were in R2, which is the
vertical slice doing its job.

**Final count: 403 against a projected ~502**, at 22.3 lines per annotation.
The scope doc's stated floor was 450 and the overshoot is in one place —
`internals/` came in at 29.6 lines against a projected 28, codegen at 18.1
against 14. The compression the `quote!` count could not see is that the four
enum representations share three reading paths between them, and that
`attr.rs`'s attribute chains are worth explaining by group rather than by
branch.

### What R6 shipped

Seven units, 15 through 21, and the course track now runs 21 units across both
annotated crates. 138 `serde_derive` annotations are tagged onto them, and the
track's own figures move from 202 course annotations to 340.

The units are about **writing a procedural macro**, which is a different subject
from units 1–14 and is the right one for the second crate: what a derive macro
can and cannot see, where a compiler error points and why `quote_spanned!`
decides it, parsing into an AST narrower than syn's, `quote!` interpolation and
the `Fragment` type, designing an attribute language, inferring the bounds of an
impl you are generating, and the four enum representations compared by what they
cost to *read*.

**The cross-crate DAG is real, not nominal.** `reading_order` grew 23 qualified
entries (`serde_derive:src/ser.rs`), seven derive annotations declare prereqs
pointing into `serde_core` — the emitted impl header leans on `Serializer:
Sized` and `type Ok`, the generated visitor on `Visitor` — and D8's check was
verified by pointing a unit-15 annotation at a unit-21 one and watching the
build fail with the unit named. Two crates, one graph, one ordering.

One thing did not need doing. `supplement` still means what it meant; all seven
new units are `none`, because the second crate genuinely teaches all of it. The
field's doc comment moved from "taught entirely from serde_core" to "from the
annotated crates" and that was the whole change.

R3–R5 are pure content throughput and can be reordered freely, with one
exception: `internals/attr.rs` is the density risk the scope doc flagged
(450 annotations if the enum representations compress, 580 if `attr.rs` refuses
large spans), so R4 landing before R5 keeps the worst estimate away from the
finish line.

### Cost, with what N1–N5 already paid

| item | sessions | status |
|---|---:|---|
| multi-source vendoring — the pin half | 1 | **spent (N1)** |
| expansion harness | 1 | **spent (N3)** |
| glossary mechanism | 1 | **spent (N2)** |
| multi-source vendoring — the store half (R1) | 1 | new; not in the scope doc's table |
| narrative retargeting (R2–R5) | 0.5 | new; falls out of the role change |
| cross-crate course DAG (R6) | 0.5 | |
| ~502 annotations (R2–R5) | 5–7 | |
| 6–8 course units (R6) | 6–8 | |
| ship (R7) | 0.5 | |
| **remaining** | **14–18** | |

The scope doc's 15–19 was from an empty start. Three sessions of it are already
in the repo and two more were hiding in it, which is close enough to say the
estimate held. About 60% of what is left is prose, and §5 of the scope doc is
right that the course units are the long pole — 468 annotations landed across
four working days, while 14 course units took six.
