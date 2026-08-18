# serde_line_by_line

A guided walkthrough of **every line** of [`serde_core`](https://crates.io/crates/serde_core) —
the crate that holds Serde's actual trait definitions — with explanations
side-by-side with the source and runnable micro-examples.

Along the way it teaches Rust: lifetimes, trait design, associated types,
generic bounds, `macro_rules!`, and `no_std` engineering, using one of the most
carefully written crates in the ecosystem as the worked example.

> **Status: phase 1 in progress.** The pipeline and the reader both build; the
> annotations are barely started. See **[PLAN.md](PLAN.md)** for the roadmap and
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
> TOTAL                        12037       29       2   0.2%
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
- **Course track** — the same annotations reordered as a Rust curriculum, plus
  supplementary units covering what serde_core genuinely does not exercise
  (ownership in imperative code, iterators, closures, error idioms, `unsafe`).

## Repository layout

| path | contents |
|---|---|
| `PLAN.md` | full design and roadmap |
| `vendor/` | pinned, unmodified `serde_core` 1.0.229 (MIT/Apache-2.0) |
| `annotations/` | the explanations, as TOML keyed to line ranges |
| `examples/` | micro-example crates, CI-verified |
| `app/` | Axum + Askama reader |
| `xtask/` | coverage gate, build and WASM pipeline |
| `docs/` | style guide, Rust-feature vocabulary, contributing |

## Licensing

Project content and code: **MIT OR Apache-2.0**.

`vendor/serde_core-1.0.229/` is an unmodified copy of the upstream crate by
Erick Tryzelaar and David Tolnay, also MIT OR Apache-2.0. See
[`vendor/NOTICE.md`](vendor/NOTICE.md).

This project is not affiliated with or endorsed by the Serde maintainers.
