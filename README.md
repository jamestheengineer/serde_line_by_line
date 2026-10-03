# serde_line_by_line

[![ci](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/ci.yml/badge.svg)](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/ci.yml)
[![pages](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/pages.yml/badge.svg)](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/pages.yml)
[![serde_core annotated](https://img.shields.io/endpoint?url=https%3A%2F%2Fjamestheengineer.github.io%2Fserde_line_by_line%2Fbadge.json)](https://jamestheengineer.github.io/serde_line_by_line/)
[![serde_derive annotated](https://img.shields.io/endpoint?url=https%3A%2F%2Fjamestheengineer.github.io%2Fserde_line_by_line%2Fbadge-serde_derive.json)](https://jamestheengineer.github.io/serde_line_by_line/)

**Read it: <https://jamestheengineer.github.io/serde_line_by_line/>**

A guided walkthrough of **every line** of
[`serde_core`](https://crates.io/crates/serde_core) — the crate that holds
Serde's actual trait definitions — and of
[`serde_derive`](https://crates.io/crates/serde_derive), the crate that
generates the code calling into it. Explanations side-by-side with the source,
runnable micro-examples, the derive macro expanding live in the browser, and
one JSON document followed through
[`serde_json`](https://crates.io/crates/serde_json), so all three crates can be
seen working on the same bytes. `serde_json` is the third crate being claimed
line by line, and that work is in progress.

Along the way it teaches Rust twice over: lifetimes, trait design, associated
types, generic bounds, `macro_rules!` and `no_std` engineering from
`serde_core`; and then spans, an AST of your own, an attribute language and the
bound inference behind a generated impl from `serde_derive`. Two of the most
carefully written crates in the ecosystem as the worked examples. And it answers
the question most readers actually arrive with — **what does
`#[derive(Serialize)]` turn into?** — by following one struct and one enum all
the way through and running the expansion in your browser.

> **Status: live, with four tracks, and a third crate being claimed.** Two
> crates are claimed line by line — `serde_core` at 12,037 of 12,037 and
> `serde_derive` at 8,975 of 8,975, two hard gates and no number that spans
> them (PLAN.md §12, [D12](docs/decisions.md)). The course track spans both at
> 21 units. Two walks follow a path rather than claiming ground: one through
> `serde_derive`, and one through `serde_json` (PLAN.md §13).
>
> `serde_json` is the third `coverage` source as of PLAN.md §14. Every line of
> it is claimed — 18,359 of 18,359, in 566 annotations — and all 38 of its
> files are named complete. Its course units are being written, and until
> they are this file leaves the first two crates' sections as they stood.
>
> **Reference** — every line of both annotated crates: `serde_core`'s 19 files
> and `serde_derive`'s 28, all at 100% and all listed in their
> `annotations/<crate>/manifest.toml`, so the gate hard-fails on any gap or
> overlap. 871 annotations between them. The site rebuilds and deploys to the
> URL above on every push to `main`.
>
> **Course** — all 21 units are written and the track is walkable start to
> finish. Units 1&ndash;14 are read out of `serde_core`; units 15&ndash;21 out
> of `serde_derive`, and they are about writing a procedural macro — spans, an
> AST of your own, an attribute language, inferring the bounds of an impl you
> are generating, and what the four enum representations cost to read. Five of
> the first fourteen are supplementary in whole or in part: ownership and
> lifetime basics entirely, since serde_core exercises lifetimes only in their
> advanced forms and ordinary ownership not at all, and `PhantomData`, errors
> and iterators-and-closures in part — the crate writes thirteen closures in
> twelve thousand lines and declares no `Fn` bound at all. That material is
> written from scratch, with runnable examples and twelve committed
> compile-fail cases, and every unit labels which of it came from the crate and
> which did not.
>
> **Derive** — 9 units and 85 steps follow one struct and one enum from a
> `syn::DeriveInput` to an emitted `impl`, citing 1,649 lines of pinned
> `serde_derive`. Thirty steps quote the code their citation emits,
> and those quotations are sliced out of a real expansion at build time rather
> than stored, so the page does not build if the crate stops emitting them.
> The walk makes no coverage claim of its own — it is a path, and the
> percentage belongs to the reference track underneath it. Every one of its 85
> steps now lands inside an annotation and links there, which is what happened
> when `serde_derive` stopped being walked-not-claimed and became the second
> crate the reference track claims.
>
> **Json** — 9 units and 104 steps follow one document,
> `{"a":[1,true],"b":"x"}`, from bytes through `serde_json`'s parser into a
> `Value`, back out to the same twenty-two bytes, through an error and its
> position, and finally into a struct through the impl the derive walk
> emitted. It cites 2,258 of `serde_json`'s lines and claims none of them
> itself: the claiming is the reference track's, which has started on
> `serde_json` (PLAN.md §14). Where the walk crosses into `serde_core` or
> `serde_derive` — 21 of its stops — it lands inside an annotation and links
> there, and its other stops will as each file they stand on is finished. Its 21 claims about what the crate *produces*
> are located in the output of a real run of the pinned version, checked
> against both the host and the browser ([D13](docs/decisions.md)).
>
> See **[PLAN.md](PLAN.md)** for the roadmap and
> **[docs/decisions.md](docs/decisions.md)** for the architecture calls.
>
> ```
> cargo xtask wasm   # build the example playground (needs wasm-bindgen-cli)
> cargo site         # generate site/
> cargo dev          # serve it at http://127.0.0.1:8080
> cargo xtask coverage
> ```
>
> ```
> $ cargo xtask coverage
> serde_core-1.0.229          coverage        19    12037  every line claimed
> serde_derive-1.0.229        coverage        28     8975  every line claimed
> serde_json-1.0.151          coverage        38    18359  every line claimed
> …
> course track: 21/21 units written
> narrative track derive: 9 units, 85 steps, 1649 of serde_derive-1.0.229's 8975 lines cited
> narrative track json: 9 units, 104 steps, 2258 of serde_json-1.0.151's 18359 lines cited
> ```

---

## Why serde_core

`serde` itself is a 5-file facade — 94% of its compiled code is derive-support
plumbing. **`serde_core`** is where the real content lives:

- 19 files, 12,037 lines, **zero dependencies**
- 29% of it is already doc comments written by dtolnay
- 45 `macro_rules!` definitions driving 288 invocations

Zero dependencies matters for a teaching project: the reader never has to leave
the tree to understand something.

`serde_json` is here for the opposite reason. It is the format the other two
crates exist to serve, and the one place on the site where a byte is actually
read: the walk through it is how the trait definitions get called.

## What "every line" means here

Not a slogan, and not a claim over the whole `vendor/` tree either. Six crates
are pinned, and `vendor/pin.toml` gives each one a **role** that decides what
the gates ask of it:

| role | crate | what is promised |
|---|---|---|
| `coverage` | `serde_core`, `serde_derive`, `serde_json` | every line claimed by exactly one annotation; `cargo xtask coverage` fails on a gap, an overlap, or a dangling reference. `serde_json` took this role in PLAN.md §14 and is being claimed file by file |
| `narrative` | none today | **walked, not claimed** — a walk cites line ranges that resolve in the pinned tree and makes no coverage promise, so it gets no percentage. `serde_derive` held this role until §12 and `serde_json` until §14 |
| `glossary` | `syn`, `quote`, `proc-macro2` | **quoted, not annotated** — a definition is pinned to a line range, and the gate fails if the quotation drifts from the tree |

A role the gates do not know is a build failure, which is what keeps the
promise from quietly widening. The reasoning is D9 in
[`docs/decisions.md`](docs/decisions.md).

`serde_derive` carries the `coverage` role as of PLAN.md §12 and is claimed
file by file; it was `narrative` while the derive walk was the only thing
citing it. Two coverage sources means **two figures and never a combined one**
— the two crates promise the same thing about different amounts of code, so a
percentage spanning them would describe neither ([D12](docs/decisions.md)).
The third state the gate needs for this is *not started*: a file no annotation
claims is counted, not warned about, because it is the roadmap rather than a
defect.

The first two crates are claimed in full, and what "every line" cost in each is
worth stating side by side, because the two numbers are different for a reason:

| | lines | annotations | lines each |
|---|---:|---:|---:|
| `serde_core` | 12,037 | 468 | 25.7 |
| `serde_derive` | 8,975 | 403 | 22.3 |

`serde_core` compresses because 45 `macro_rules!` definitions generate 288
invocations and a `macro-use` annotation costs a line of reference rather than a
paragraph. `serde_derive` has 2 `macro_rules!` and 271 `quote!` blocks, every
one emitting something different — so it is 25% smaller and costs nearly as
much. What compression it does have is sharing: three of the four enum
representations read their payload through one function.

## Shape of the app

```
┌──────────────┬────────────────────────┬──────────────┐
│ TREE         │  serde_core/ser/mod.rs │ EXPLANATION  │
│              │                        │              │
│ lib.rs       │  1  pub trait Serial…  │ ## The Data  │
│ ▾ ser/       │  2      type Ok;       │ ## Model     │
│   mod.rs  ◀  │  3      type Error;    │              │
│   impls.rs   │  4  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  │ `type Ok` is │
│   fmt.rs     │  5      fn serialize_  │ what the fmt │
│ ▾ de/        │  6          bool(…)    │ produces…    │
│   mod.rs     │                        │              │
│              │  ── micro-example ──   │ [Run ▶]      │
│ ▓ 12% done   │  #[derive(...)]        │ out: true    │
└──────────────┴────────────────────────┴──────────────┘
```

Axum + Askama. Examples are real crates, compiled to WASM for the browser and
runnable under local `cargo` for real rustc errors and debugger stepping. Every
example's output is asserted in CI, so explanations cannot drift from behavior.

Four ways to read it:

- **Reference track** — file by file, 100% coverage, the spine.
- **Course track** — the same annotations reordered as a Rust curriculum: 14
  units, sequenced by the prereq graph rather than by file. Units serde_core
  cannot supply — ownership in imperative code, lifetime basics, iterators and
  closures — are written from scratch and labelled **supplementary** in the UI,
  rather than pretending the crate demonstrates them.
- **Derive track** — one struct and one enum followed out of `serde_core` and
  through `serde_derive`, from the `TokenStream` the compiler hands over to the
  `impl` it gets back. 9 units, 85 steps, and where the path crosses back into
  `serde_core` it links to the annotation rather than explaining the same lines
  twice.
- **Json track** — one document followed through `serde_json`, from bytes to a
  `Value` and back, then into a struct through the impl the derive track
  watched being emitted. 9 units, 104 steps, and every output it quotes is
  located in a real run rather than typed into the page.

And two tools the derive track leans on, reachable on their own:

- **`expand`** — `serde_derive` itself compiled to wasm, expanding the two
  worked types in the browser. A golden transcript asserts the site's output is
  byte-identical to the host's, so the page cannot show one thing and `cargo
  expand` another. Every quotation of generated code on the derive track is
  sliced out of this expansion at build time.
- **`glossary`** — the 62 `syn`, `quote` and `proc-macro2` types `serde_derive`
  reads but does not define, each with a definition and 1,123 lines quoted from
  its own pinned tree.

## Repository layout

| path | contents |
|---|---|
| `PLAN.md` | full design and roadmap |
| `vendor/` | six pinned, unmodified crates (MIT/Apache-2.0); `vendor/pin.toml` gives each a role |
| `annotations/` | the explanations, as TOML keyed to line ranges |
| `annotations/course.toml` | the course track's units, ordering, and honesty labels |
| `narrative/` | one directory per walk — `derive/` and `json/` — each a `track.toml` and its units, as TOML citing pinned line ranges |
| `glossary/` | borrowed-vocabulary entries, each pinned to a quoted range |
| `examples/` | micro-example crates, CI-verified |
| `expand/`, `expander/` | the `serde_derive` expansion harness and its wasm front end |
| `app/` | Axum + Askama reader |
| `xtask/` | coverage gate, version-bump migration, build and WASM pipeline |
| `docs/` | style guide, Rust-feature vocabulary, migration runbook, contributing |
| `.githooks/` | pre-push hook running the CI gates locally |

## Working on this

Enable the pre-push hook once per clone:

```
git config core.hooksPath .githooks
```

It runs the same gates as CI — `cargo fmt --all --check`, clippy with `-D
warnings`, `cargo test --workspace`, the coverage gate, the site build, and the
wasm build — and refuses the push if any fail, which takes a couple of seconds
warm. `git push --no-verify` skips it when that is what you want. The reasoning
is in [`docs/decisions.md`](docs/decisions.md) under D5.

[`docs/contributing.md`](docs/contributing.md) covers the rest: how to add an
annotation, how to add an example that runs both under `cargo test` and in the
browser, what the coverage gate checks, and the one rule about the vendored
tree — never edit it, because every annotation is keyed to its line numbers.

Moving to a newer `serde_core` is the one change that rewrites the whole store
at once, so it is a tool rather than a chore: `cargo xtask bump <version>`
verifies the release against the crates.io index, carries every line range
across the diff, and reports the annotations whose code changed underneath them.
[`docs/migration.md`](docs/migration.md) is the runbook.

## Licensing

Project content and code: **MIT OR Apache-2.0**.

Everything under `vendor/` is an unmodified copy of a published crate —
`serde_core`, `serde_derive` and `serde_json` by Erick Tryzelaar and David
Tolnay (with `serde_json`'s `src/lexical/` by Alexander Huszagh), `syn` and
`quote` by David Tolnay, `proc-macro2` by David Tolnay and Alex Crichton — all
MIT OR Apache-2.0. Provenance, checksums and copyright for each are in
[`vendor/NOTICE.md`](vendor/NOTICE.md).

This project is not affiliated with or endorsed by the Serde maintainers.
