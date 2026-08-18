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
| App form | **Web app, custom reader** (Axum + Askama) | Full control over the three-pane layout and narrative flow; deployable so others can read it; matches the existing Rust web stack. |
| Example execution | **Both** — WASM for deploy, local cargo for development | Real computed output in the browser with no backend; real rustc errors and debugger stepping locally. |
| Content sequencing | **Two tracks in parallel** | Reference track guarantees 100% coverage; course track serves the teaching objective. One annotation store, two orderings. |
| Source handling | **Vendored + checksum-pinned** | Annotations are line-range keyed. Upstream drift must be an explicit migration, never a silent break. |

### The load-bearing decision: annotations are data, not pages

Annotations live in `annotations/*.toml` as records keyed to `(file, line-range)`.
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

`annotations/ser_mod.toml`:

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
complete in `annotations/manifest.toml`, then a **hard failure**.

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

## 7. The two tracks

**Reference track** — the spine. File-by-file, dependency order, 100% coverage.
This is the "every line" promise. Navigation mirrors the source tree.

**Course track** — a curated path through the *same* annotations, ordered by
teaching progression rather than file layout. Sequence sketch:

| unit | topic | drawn from | supplement needed |
|---|---|---|---|
| 01 | Why serialization needs a data model | `ser/mod.rs` head | — |
| 02 | Traits and supertraits | `ser/mod.rs` | — |
| 03 | Associated types | `ser/mod.rs`, `de/mod.rs` | — |
| 04 | Generic bounds and `where` | `ser/impls.rs` | — |
| 05 | **Ownership and borrowing** | — | **yes, fully supplementary** |
| 06 | Lifetimes I: the basics | — | **yes** |
| 07 | Lifetimes II: `'de` and zero-copy | `de/mod.rs` | — |
| 08 | `PhantomData` and variance | `de/value.rs` | partial |
| 09 | Blanket impls and coherence | `de/mod.rs` | — |
| 10 | `macro_rules!` | `de/impls.rs` | — |
| 11 | `no_std` and feature gates | `crate_root.rs`, `lib.rs` | — |
| 12 | **Errors, iterators, closures** | — | **yes, fully supplementary** |

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
│   ├── manifest.toml           ← per-file completion status
│   └── *.toml                  ← the content
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
| **Line-range brittleness.** Any edit to vendored source invalidates annotations. | Checksum gate in CI. Version bumps are explicit migrations with a remapping tool. |
| **Macro-heavy files become tedious.** `de/impls.rs` could read as 106 near-identical entries. | `kind = "macro-use"` renders compactly and links to the def. Prove this in phase 2 before committing to phase 5. |
| **Course track over-claims.** Pretending serde_core teaches all of Rust would be dishonest. | §2 is committed to the repo. Supplementary units are labeled as such in the UI. |
| **Scope creep to `serde` / `serde_derive`.** | Out of scope for v1. Revisit only after the reference track hits 100%. |

**Open questions to resolve in phase 0:**

1. WASM strategy — `wasm-bindgen` vs raw `wasm32-unknown-unknown` with a thin
   JS shim. Leaning `wasm-bindgen` for ergonomics; revisit if payload size bites.
2. Syntax highlighting — server-side `syntect` (Rust-native, no JS, but couples
   rendering to the backend) vs client-side. Leaning `syntect`.
3. Whether the app should be a static-site generator with an optional Axum dev
   server, rather than a live server. Static deploys are cheaper and the content
   is read-only; the WASM decision makes a live backend unnecessary. **Likely
   yes** — decide in phase 0.
4. Scroll-sync UX between source pane and explanation pane: anchored jumps vs
   continuous sync.
