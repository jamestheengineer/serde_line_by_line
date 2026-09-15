# Scoping a `serde_json` track

Everything PLAN.md planned has shipped: two crates at 100%, 871 annotations, a
21-unit course track, and a walk that is an ordering over the store. Nothing is
queued, so the next thing is a choice rather than a step. This is the
measurement for the strongest candidate — the third crate in the same tree, and
the first one that is a *format*.

Measured 2026-09-13 against `serde_json-1.0.151` (37 files, 18,329 lines), the
two finished stores in this repo, and a wasm spike that ran (§9).

The measurements are reproducible: `cargo xtask stats --dir <path>` now takes an
unpinned tree, because the effort model for a candidate is the first thing worth
knowing about it and vendoring it first would mean committing in order to find
out the cost.

---

## 1. The headline

**About 1.5× the crate that started this project, at almost exactly its
density** — 18,329 lines, roughly 700 annotations at 26.1 lines each against
`serde_core`'s 25.7, eight new course units, and one decision that has to be
made before a single annotation is written.

The size is not the interesting part. §3 is.

What is genuinely new about this candidate, and true of no other:

**The vocabulary it borrows is already annotated.** `serde_derive` needed D9 and
a glossary because it is written in `syn`, 60,486 lines this project will never
claim. `serde_json` is written in `serde_core`, 12,037 lines this project
claimed first. It implements `Deserializer` **13 times** and `Serializer`
**8 times**, and every one of those contracts is a page the reader has already
read. The third-party surface left over is 16 items (§3).

And `serde_json 1.0.151` resolves `serde_core` to exactly `1.0.229` — the
pinned version. The format crate the reader runs in the browser is compiled
against the crate they have read every line of.

---

## 2. Volume

`serde_json` is 52% larger than `serde_core` and 104% larger than
`serde_derive`. Projected at the densities **this project actually achieved**,
grouped by the profile `cargo xtask stats` assigns each file:

| profile | files | lines | annotations | lines/annot |
|---|---:|---:|---:|---:|
| doc-heavy | 5 | 4,918 | 139 | 35.4 |
| logic | 32 | 5,667 | 210 | 27.0 |
| macro-driven | 4 | 7,482 | 382 | 19.6 |
| quote-driven | 6 | 2,945 | 140 | 21.0 |
| **all, both crates** | **47** | **21,012** | **871** | **24.1** |

Carried onto `serde_json`, with two corrections the raw profile misassigns —
`ser.rs` looks macro-driven and is not (73 of its 74 macro uses are `tri!`, the
crate's `?`), and `lexical/`'s five constant-table files are a table each, not a
paragraph per row:

| group | files | lines | lines/annot | annotations |
|---|---:|---:|---:|---:|
| the engines — `de.rs`, `ser.rs`, `read.rs`, `io/*`, `iter.rs` | 6 | 6,257 | 23.2 | ~270 |
| the `Value` tree — `value/*`, `map.rs`, `number.rs`, `raw.rs` | 9 | 7,079 | 25.0 | ~283 |
| `lexical/` — the float parser (§3) | 19 | 3,708 | 35.1 | ~106 |
| plumbing — `lib.rs`, `error.rs`, `macros.rs` | 3 | 1,285 | 29.4 | ~44 |
| **total** | **37** | **18,329** | **26.1** | **~700** |

**Bounds: 600 if the 21 trait impls compress, 780 if the parser refuses large
spans.** The compression case is the one the derive track already proved: three
of four enum representations shared one reading path there, and here `de.rs` and
`value/de.rs` are the same trait surface implemented over two different inputs —
30 of their 73 and 50 function names are shared, and `ser.rs`/`value/ser.rs`
share 40. Explaining "what a `Deserializer` impl must do" is not even in scope:
`de/mod.rs` did it, in 77 annotations the reader has already passed.

The refusal case is `read.rs` and the string scanner in `ser.rs`: byte-level
state machines where the interesting unit is a branch, not a function.
`serde_json` matches on a byte literal **297 times** against `serde_core`'s 3.

**One number here has no track record.** The derive scope doc projected 210
words per annotation and the store came in at 67; `serde_core`'s came in at 133.
Word counts have been this project's least reliable projection, twice. Sessions
and annotation counts have held; the prose estimate is not repeated here.

## 3. The `lexical/` question — decide this first

`src/lexical/` is **3,708 lines, 20% of the crate**, and it is none of the
things this project has had to deal with before:

- It is **somebody else's code, inside the crate that would be claimed.** Its
  own header says so: *"derived from the `lexical` crate by @Alexhuszagh …
  copyright Alexander Huszagh"*. D9 settled that borrowed vocabulary is quoted
  and pinned rather than annotated — but D9's mechanism is a *separate pinned
  source*, and the pin cannot tell this apart from `serde_json`, because it is
  `serde_json`.
- It is **not compiled by default.** `#[cfg(feature = "float_roundtrip")] mod
  lexical;`. Under the pin's default feature set those 3,708 lines are dead.
- It is **not about JSON.** It is Grisu, bignum arithmetic and correctly-rounded
  `strtod`. A reader who wants it wants a float-parsing line-by-line, and that
  is a different project — the same sentence D9 wrote about `syn`'s parser.

And yet the lines are load-bearing, which the spike demonstrated by accident.
Parsing `2.2250738585072011e-308` — the largest subnormal double — in the
browser, twice:

| build | result | bits |
|---|---|---|
| default features | `2.2250738585072014e-308` | `0x0010000000000000` |
| `float_roundtrip` | `2.225073858507201e-308` | `0x000fffffffffffff` |

Same input, two different doubles, one ULP apart; the second is the correct one.
Those 3,708 lines *are* the difference, and a reader who types that number into
the playground finds out which build the site shipped. So "it is only an
optional feature" is not available as a reason to skip it.

Three options, and this is a decision to be argued the way D12 was, not a
refactor to perform quietly:

| | what the promise becomes | cost |
|---|---|---|
| **claim it** | unchanged: every line of every annotated crate | +3,708 lines, ~106 annotations, and one section of the site is about floats |
| **declare a feature set** | every line *the pinned build compiles* | a fourth gate state, and it leaks: `arbitrary_precision` (123 sites) and `raw_value` (65) are interleaved *inside* files, not quarantined in a module |
| **pin it as a glossary source** | every line, except borrowed code named as borrowed | dishonest as stated — a glossary source is a separate crate, and this one is not |

**Recommended: claim it.** "Every line" is the load-bearing claim of this
project and the only reason the coverage gate exists. The second option's
qualifier sounds cheap and is not: it would have to be stated on the front page
next to a percentage, and the first reader to ask "which features?" gets a worse
answer than the first reader to ask "why is there a chapter about floats?".

Two smaller mechanics fall out of whichever way it goes.

**`build.rs` is not in `src/`.** Thirty lines that emit
`fast_arithmetic = "32" | "64"`, read at 19 sites across 5 files, deciding which
of two arithmetic paths compiles. `vendor::source_files_in` walks `src/` only,
so "every line" would silently not mean those thirty. Either the walk learns
named extra files, or the promise is qualified in a second place.

**A third copyright holder.** `vendor/NOTICE.md` is generated from the pin, one
entry per source, one copyright line per entry. This tree needs two.

## 4. Engineering deltas

R1 built the multi-source machinery for the general case, and it holds: `Store`
and `Report` are lists, `percent()` lives on the children, `annotations/<crate>/`
is per source, and the sidebar groups by crate with a figure each. A third
`[[source]]` with `role = "coverage"` is mostly a `cargo xtask pin` away.

Five places still reach for the first coverage source. Four are correct and
stay — D12 kept `serde_core`'s unqualified URLs, its badge, its front-page
position, and the course registry's declared source. **One is wrong for a third
crate**, and it is the one that would bite quietly:

`xtask/src/coverage.rs:129` checks every example's `Cargo.toml` against
`pin.coverage()?[0]` alone, with the reason in a comment: *"the examples build
against the first coverage source: they are `serde_core` programs"*. Every
`serde_json` example is a `serde_json` program, and today's check would not look
at the version it pins. That is the drift the vendor checksum exists to prevent,
arriving through a door that is now open. Half a session, and it is the same
shape as D12: a refusal written for one source, correct when written.

**Examples are nearly free — measured, not assumed.** N3's expander cost a
session and 253,693 gzipped bytes because `serde_derive` is a proc macro and had
to be patched into a library. `serde_json` is an ordinary `no_std`-capable
library that compiles to `wasm32-unknown-unknown` untouched. Marginal cost over
a bare `wasm-bindgen` module: **+16,135 bytes raw, +10,046 gzipped** — 12% of
what the existing playground already ships. §9 has the numbers and the run.

| delta | cost |
|---|---|
| third coverage source, `pin`, `NOTICE`, nav | ~0.25 session (R1 and R7 generalized this) |
| example-pin check per source | ~0.25 session |
| the §3 decision and whatever gate it implies | 0.5 session |
| expansion harness | **none** — `serde_json` is a library |
| glossary | **none of consequence** — §3's 16 items, against D9's 62 |
| cross-crate course DAG | **none** — R6 built it, and the edges already point the right way |

## 5. Borrowed vocabulary, for completeness

The derive track's wall was 60,486 lines and 62 borrowed items. This one:

| crate | lines | items `serde_json` uses |
|---|---:|---|
| `itoa` 1.0 | 488 | `Buffer` (29 uses) |
| `zmij` 1.0 | 2,043 | `Buffer` (10 uses) |
| `memchr` 2 | 15,824 | `memchr2`, `memchr_iter`, `memrchr` |
| `indexmap` 2 | 13,603 | 11 types, only under `preserve_order` |
| **total** | **31,958** | **16 items** |

Sixteen glossary entries, most of them two-method buffer types. The 28 `serde_core` items
`serde_json` names are not glossary entries at all — they are annotations, and
they already exist.

## 6. What it teaches, and what it does not

PLAN.md §2 is committed to the repo precisely so the course track cannot
over-claim, and it lists what `serde_core` teaches almost nothing about.
`serde_json` closes some of that list and none of the rest:

| §2 said serde_core teaches almost nothing about | serde_json | evidence |
|---|---|---|
| ownership in ordinary imperative code | **yes** | a hand-written parser with a scratch buffer |
| error-handling idioms | **yes** | `error.rs`, position tracking, 541 lines |
| `unsafe` | **yes, honestly small** | 12 blocks and 1 `unsafe fn` against `serde_core`'s 2 |
| collections | **yes** | `map.rs`, and a `BTreeMap`/`IndexMap` swap behind a feature |
| iterators and closures | partly | 111 adapter calls against `serde_core`'s 49 — but `serde_derive` has 278 |
| concurrency, `async`, interior mutability | **no** | nothing in serde's tree has these |

It also brings subjects neither annotated crate has: byte-level state machines
(297 byte-literal matches against 3), numeric edge cases (29 checked/wrapping
operations against 5, plus §3's one-ULP story), and feature-matrix engineering
at a scale `serde_core`'s `no_std` work only hinted at — 344 `#[cfg]` sites over
seven features, `arbitrary_precision` and `raw_value` changing the *type* of
`Number` underneath the same public API.

Eight new course units, 22 through 29: bytes in and values out; writing a
`Serializer` (the other side of units 02–05); strings, escapes and borrowing
from the input; numbers and round-tripping; the `Value` tree and the `json!`
TT-muncher (which is unit 11's `macro_rules!` at its hardest); errors that carry
a position; feature-gated engineering; and `unsafe` with its invariants written
down.

## 7. Total

| item | sessions |
|---|---:|
| third coverage source, pin, notice, nav | 0.25 |
| example-pin check per source | 0.25 |
| the §3 decision and its gate | 0.5 |
| ~700 annotations | 8–11 |
| 8 course units | 6–9 |
| ship | 0.5 |
| **total** | **16–22** |

Calibration, since this project now has two finished tracks to calibrate
against: `serde_core` was ~20 sessions, 8 phases, 14 calendar days, 468
annotations. `serde_derive` was estimated at 15–19 and delivered in 7 phases,
403 annotations. Both estimates held in the unit the project actually works in —
**phases** — and this one is ~8 phases: one per group in §2's table, plus the
decision, the units, and the ship.

## 8. The smaller thing, if the answer is no

The same shape §7 of the derive scope doc offered, which worked: a **narrative
track**, 8 to 10 units following one document — `{"a":[1,true],"b":"x"}` — from
bytes through `read.rs` into `Value`, out through `ser.rs`, and separately
straight into a struct through the derive impl the reader has already watched
being generated. Three or four phases, no coverage promise, no §3 decision,
no third 100% to defend on every release.

It is also the one option that connects all three crates in a single walk, which
neither of the existing tracks does.

## 9. Recommendation

**Decided, 2026-09-15: §8, the narrative track.** This section's recommendation
was not overruled on its merits — §6 is the better answer to "what does this
project do next with `serde_json`" and stays available. It was declined on
appetite, and on the one thing §8 has that §6 does not: it is the only option
that walks all three crates in a single reading, which neither reference track
can do by construction and the course track does not attempt. It is PLAN.md
§13. §3 does not arise on that path — a narrative source is never asked to be
exhaustive — and PLAN.md §10 records the answer anyway, which is this
document's: claim it.

The rest of this section stands as the argument for the other shape.

**Reference track, and settle §3 before starting it.**

Unlike the derive track, there is no prerequisite left to spike. The vocabulary
wall is 16 items instead of 62. The expansion harness has no analogue to build.
The multi-source machinery exists and has been exercised. The wasm cost is
measured and small. Nothing structural is unknown.

What is unknown is §3, and it is unknown in the way D12 was unknown — an
argument to have in the open, with a wording consequence on the front page,
not a thing to discover in month three. Have it first. Everything else here is
throughput.

Two risks worth writing down before anyone says yes:

**Release cadence — checked, and the registry cache was the wrong sample.**
Against crates.io on 2026-09-15, since 2025-01-01: `serde_json` published 17
versions and `serde_core` 10. The counts are close and they mislead, because
nine of `serde_core`'s ten landed in the single fortnight of the crate's split
from `serde`. The number that costs something is **distinct months with a
release**, which is how many times a bump would actually be run: `serde_json`
**10 months of 20**, `serde_core` **2**. A third claimed crate is therefore a
~700-range remap roughly **six times a year**, against what the first two crates
have charged so far, which is nothing at all: `1.0.229` is still the newest of
both.

`serde_json 1.0.151` is likewise still the newest, so §2's measurement is
against current head and the track would not start behind. `cargo xtask bump`
makes each remap a command rather than a chore ([D7](decisions.md),
[D11](decisions.md)), and `1.0.135 → 1.0.151` shows what it would be absorbing:
16 releases in 18 months, none of them a major. This is a real standing cost and
it is the *only* one on this candidate that the first two crates have not already
paid.

**The promise gets a third figure, not a bigger one.** D12 settled that already,
and it settles the same way here: three crates, three percentages, no total.

---

## 10. The wasm spike, run

Measured 2026-09-13, `wasm-bindgen` 0.2.127 (the pinned version), node 25.2.1,
against `serde_json-1.0.151`, at the playground's own profile — `opt-level =
"z"`, LTO, stripped, one codegen unit.

**It works, and it is cheap.** No patch, no harness, no fork: `serde_json` is a
dependency.

```rust
#[wasm_bindgen]
pub fn run(input: &str) -> String {
    let value: serde_json::Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => return format!("line {} column {}: {e}", e.line(), e.column()),
    };
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| e.to_string())
}
```

| build | raw | gzipped |
|---|---:|---:|
| bare `wasm-bindgen` module, for the floor | 27,948 | 11,324 |
| with `serde_json`, default features | 44,083 | 21,370 |
| with `serde_json`, `float_roundtrip` | 61,062 | 30,361 |
| the existing playground, for scale | 237,754 | 82,363 |
| the expander N3 shipped, for scale | 780,037 | 253,778 |

So `serde_json` costs **10 KB gzipped**, and `lexical/` costs **9 KB** on top —
against the expander's 254 KB. D1's "one shared module, dispatched by id" holds
without amendment; this does not want a module of its own the way the expander
did.

The error path renders, which matters for a course unit about error positions:

```
input:  {"a": }
output: line 1 column 7: expected value at line 1 column 7
```

And the float behaviour in §3 is from this same spike, run twice with the
feature off and on. That is the cheapest possible demonstration of what a
feature flag buys, and it is available to the site for free.

**Untested here:** whether `preserve_order` and `arbitrary_precision` are
byte-identical between host and wasm the way the expansion cases are. They
should be — neither touches platform behaviour — but the existing output gate
is what would prove it, and it costs a case, not a session.
