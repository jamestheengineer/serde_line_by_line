# serde_line_by_line

[![ci](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/ci.yml/badge.svg)](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/ci.yml)
[![pages](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/pages.yml/badge.svg)](https://github.com/jamestheengineer/serde_line_by_line/actions/workflows/pages.yml)
[![lines annotated](https://img.shields.io/endpoint?url=https%3A%2F%2Fjamestheengineer.github.io%2Fserde_line_by_line%2Fbadge.json)](https://jamestheengineer.github.io/serde_line_by_line/)

**Read it: <https://jamestheengineer.github.io/serde_line_by_line/>**

A guided walkthrough of **every line** of [`serde_core`](https://crates.io/crates/serde_core) —
the crate that holds Serde's actual trait definitions — with explanations
side-by-side with the source and runnable micro-examples.

Along the way it teaches Rust: lifetimes, trait design, associated types,
generic bounds, `macro_rules!`, and `no_std` engineering, using one of the most
carefully written crates in the ecosystem as the worked example.

> **Status: live, and complete on both tracks.** Every line of `serde_core` is
> annotated — all 19 files at 100%, listed in `annotations/manifest.toml`, so
> the coverage gate hard-fails on any gap or overlap — and the site rebuilds and
> deploys to the URL above on every push to `main`.
>
> **All 14 course units are written**, and the track is
> walkable start to finish. Five of them are supplementary in whole or in part:
> ownership and lifetime basics entirely, since serde_core exercises lifetimes
> only in their advanced forms and ordinary ownership not at all, and
> `PhantomData`, errors and iterators-and-closures in part — the crate writes
> thirteen closures in twelve thousand lines and declares no `Fn` bound at all.
> That material is written from scratch, with runnable examples and twelve
> committed compile-fail cases, and every unit labels which of it came from the
> crate and which did not. See
> **[PLAN.md](PLAN.md)** for the roadmap and
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
> TOTAL                        12037    12037     468 100.0%
>
> course track: 14/14 units written
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

## What "every line" means here

Not a slogan. `cargo xtask coverage` verifies that every line of every vendored
source file is claimed by exactly one annotation, and CI fails on regressions
once a file is marked complete.

The work is roughly **1,195 annotation units** averaging ~10 lines each — not
12,037 individual comments, because doc-comment blocks and repeated macro
invocations compress heavily.

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

Two ways to read it:

- **Reference track** — file by file, 100% coverage, the spine.
- **Course track** — the same annotations reordered as a Rust curriculum: 14
  units, sequenced by the prereq graph rather than by file. Units serde_core
  cannot supply — ownership in imperative code, lifetime basics, iterators and
  closures — are written from scratch and labelled **supplementary** in the UI,
  rather than pretending the crate demonstrates them.

## Repository layout

| path | contents |
|---|---|
| `PLAN.md` | full design and roadmap |
| `vendor/` | pinned, unmodified `serde_core` 1.0.229 (MIT/Apache-2.0) |
| `annotations/` | the explanations, as TOML keyed to line ranges |
| `annotations/course.toml` | the course track's units, ordering, and honesty labels |
| `examples/` | micro-example crates, CI-verified |
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

`vendor/serde_core-1.0.229/` is an unmodified copy of the upstream crate by
Erick Tryzelaar and David Tolnay, also MIT OR Apache-2.0. See
[`vendor/NOTICE.md`](vendor/NOTICE.md).

This project is not affiliated with or endorsed by the Serde maintainers.
