# Scoping a `serde_derive` track

§10 of [PLAN.md](../PLAN.md) puts `serde` and `serde_derive` out of scope for
v1, to be revisited "only after the reference track hits 100%." It has. This is
the measurement that revisiting asks for: what a `serde_derive` track would
cost, what it would cost it *with*, and the one decision that determines
whether it is finishable at all.

Measured 2026-09-06 against `serde_derive-1.0.229` (28 files, 8,975 lines) and
against this repo's own history.

---

## 1. The headline

**About 0.8× the project that already exists** — roughly 500 annotations,
~105,000 words, four engineering sessions, and six to eight new course units.
Not a bolt-on. A second project that reuses the first one's machinery.

The size is not the interesting part. The interesting part is §3.

---

## 2. Volume

`serde_derive` is 25% smaller than `serde_core` and will need *more*
annotations, because the compression lever that carried this project does not
exist there.

`serde_core` has 50 `macro-def` and 62 `macro-use` annotations — 24% of the
store. `de/impls.rs` is 3,173 lines held by 122 annotations only because
`kind = "macro-use"` lets a hundred near-identical expansions point at one
explanation. `serde_derive` has almost no `macro_rules!`. What it has instead
is 271 `quote!`/`quote_spanned!` invocations, and a `quote!` block is the
opposite of a macro use: every one emits different code and needs its own
explanation, next to the code it emits.

Projected at the densities this project actually achieved:

| group | files | lines | lines/annot | annotations |
|---|---:|---:|---:|---:|
| codegen (`ser.rs`, `de.rs`, `de/*.rs`) | 11 | 4,728 | 14 | ~338 |
| attributes and validation (`internals/*`) | 9 | 3,015 | 28 | ~108 |
| plumbing (`bound.rs`, `pretend.rs`, `fragment.rs`, …) | 8 | 1,232 | 22 | ~56 |
| **total** | **28** | **8,975** | **18** | **~500** |

Compare `serde_core`: 12,037 lines, 468 annotations, 25.7 lines each, 186 words
each, 87,406 words in the store. Derive annotations run longer — most of them
have to show what comes *out*, not just say what the line does — so ~500 × ~210
words puts the store at ~105,000 words.

Density bounds, for honesty: 450 if the four enum representations compress
against each other better than expected, 580 if `internals/attr.rs` refuses to
take large spans.

## 3. The syn wall — the decision that matters

`serde_derive` reads `syn`, `quote` and `proc-macro2` as vocabulary. Line one of
the real work is a `syn::DeriveInput`.

| crate | lines |
|---|---:|
| `syn` 3.0.3 | 51,753 |
| `proc-macro2` 1.0.107 | 6,029 |
| `quote` 1.0.47 | 2,702 |
| **total** | **60,484** |

That is **five times this entire project**. It cannot be annotated, and the
promise on the front page — every line claimed — cannot quietly be allowed to
mean "every line except the sixty thousand the reader actually has to
understand."

So the honest form of the promise changes: *every line of `serde_derive` is
claimed; `syn` is vocabulary.* That needs a mechanism, not just a sentence — a
glossary of the borrowed types (`DeriveInput`, `Data`, `Field`, `Generics`,
`Meta`, `Span`, `TokenStream`, `ToTokens`) that an annotation cites the way it
currently cites a `macro-def`, with the coverage gate failing on a citation
that does not resolve. Schema addition, renderer support, one more gate.

**If this is not settled first, the track is not finishable.** It is the same
shape as D8: a promise that reads fine in prose and is a build failure in
practice.

## 4. Engineering deltas

The site, the schema, the coverage gate, the highlighter, the deploy and the
bump tool all carry over. Four things do not.

**Multi-source vendoring.** `core/src/vendor.rs` holds
`pub const CRATE_NAME: &str = "serde_core"`, and `vendor/pin.toml` is one pin,
not a table of them. 26 references across 9 files assume a single pinned
source. Coverage tables, `cargo xtask bump`, `store_edit` and the site's routing
all need a source dimension. Phase-0-sized: one session.

**An expansion harness.** The current playground compiles examples to WASM and
runs them live. A derive example's payload is its *expansion*, and
`serde_derive`'s modules are all private behind two `#[proc_macro_derive]`
entry points, so nothing can call `ser::expand_derive_serialize` from outside.
The way through is already in the repo's habits: the tree is vendored, so build
the vendored copy as a plain library — drop `proc-macro = true`, swap
`proc_macro::TokenStream` for `proc_macro2::TokenStream` at the two entry
points — and call the real expansion functions. No `cargo expand` dependency,
deterministic output, checked against golden files the way the wasm output
already is.

The payoff was a spike, and the spike ran — see §9. It works: the expander
runs in the browser, live.

**A glossary.** §3. One session, and the editorial decision costs more than
the code.

**A cross-crate course DAG.** D8's forward-reference check orders units within
one crate. Two crates means a reader reaching `#[derive(Serialize)]` codegen
needs `Serializer`'s contract behind them, and the check has to know that.
Half a session.

## 5. Course track

Six to eight new units — proc-macro basics, `TokenStream` and spans, hygiene,
`syn`'s AST, `quote!` interpolation, the `#[serde(...)]` attribute DSL, and
codegen for the four enum representations.

This is the long pole, and the history says so. The reference track's 468
annotations landed across four working days. Phase 7's 14 units took six
calendar days at roughly one unit per session, because a unit is a designed
piece of prose, not a claimed line range. Budget six to eight sessions and
expect them to be the slow ones.

## 6. Total

| item | sessions |
|---|---:|
| multi-source vendoring | 1 |
| expansion harness (spike spent; §9) | 1 |
| glossary mechanism | 1 |
| cross-crate course DAG | 0.5 |
| ~500 annotations | 5–7 |
| 6–8 course units | 6–8 |
| **total** | **15–19** |

The whole of `serde_core` — phase 0 through phase 8, empty repo to public URL —
took about 20. So this is not an increment on a finished project. It is a
comparable project, standing on finished machinery, and about 60% of its cost
is prose.

## 7. The smaller thing, if the answer is no

There is a version of this that costs three or four sessions instead of
fifteen: a narrative track, not a reference one. Eight to ten units that follow
*one* struct and *one* enum from `DeriveInput` to emitted `impl`, citing
`serde_derive` line ranges as they go, claiming nothing exhaustively and making
no coverage promise. `kind`, the glossary and the multi-source refactor are
still needed; the 500 annotations are not.

It answers the question most readers actually arrive with — "what does
`#[derive(Serialize)]` turn into?" — and it does not commit the project to a
second 100% it has to defend on every bump.

## 8. Recommendation

Settle §3 before anything else; it is cheap to decide and it invalidates the
rest if it goes the other way. The wasm spike is done (§9) and it came back
better than hoped, so the remaining choice is between §6 and §7 — and the
site's first readers should weigh in, since it has not had any yet.

## 9. The wasm spike, run

Measured 2026-09-06, rustc 1.95.0, wasm-bindgen 0.2.127 (the pinned version),
node 25.2.1, against `serde_derive-1.0.229`.

**It works.** `serde_derive` runs in the browser and expands a struct live.

### The patch

One file, four hunks. `src/lib.rs` only:

```
-extern crate proc_macro;
-use proc_macro::TokenStream;
+use proc_macro2::TokenStream;
-use syn::parse_macro_input;

-#[proc_macro_derive(Serialize, attributes(serde))]
 pub fn derive_serialize(input: TokenStream) -> TokenStream {
-    let mut input = parse_macro_input!(input as DeriveInput);
+    let mut input = match syn::parse2::<DeriveInput>(input) {
+        Ok(input) => input,
+        Err(err) => return err.into_compile_error(),
+    };
     ser::expand_derive_serialize(&mut input)
         .unwrap_or_else(syn::Error::into_compile_error)
-        .into()
 }
```

…and the same for `derive_deserialize`. In `Cargo.toml`, `proc-macro = true`
comes off the `[lib]` and the `proc-macro` feature comes off `proc-macro2`,
`quote` and `syn`, which drops those three into their fallback implementations.
Nothing under `internals/`, `ser.rs`, `de.rs` or `de/` is touched — 8,848 of
8,975 lines build unmodified. This is a patch a `bump` can reapply, not a fork.

### What came out

The expansion is real, not a token dump. For

```rust
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Point {
    x_coord: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}
```

the browser produces 44 lines of `Serialize` and 234 of `Deserialize`, with
`"xCoord"` in the right place and the `skip_serializing_if` branch present.
Pasted into a crate with `serde 1.0.229`, it compiles clean, and running it:

```
serialized: {"xCoord":7}
deserialized: x_coord=9 label=Some("hi")
deny_unknown_fields: unknown field `nope`, expected `xCoord` or `label`
```

The code the browser prints is the code that runs.

The error path renders too. `#[serde(rename_all = "nope")]` comes back as
serde's own diagnostic, wrapped in `compile_error!` — which means the site can
teach the attribute DSL by showing what upstream says when you get it wrong,
without inventing a single error message.

### Determinism

Five cases — struct, internally tagged enum, untagged enum, a generic with
`#[serde(borrow)]`, and an attribute error — each expanded both ways, 898 lines
of output. **Host and wasm are byte-identical.** So the existing "wasm output is
checked against the transcripts" gate extends to expansions unchanged.

### Cost

| build | raw | gzipped |
|---|---:|---:|
| expander, with `prettyplease` | 1,231,302 | **360,520** |
| expander, raw `to_string()` | 1,043,425 | 313,192 |
| the existing playground, for scale | 237,754 | 82,363 |

Both at the playground's own profile: `opt-level = "z"`, LTO, stripped, one
codegen unit. Roughly 4.4× the current payload, and 47 KB of that buys
`prettyplease`, which is the difference between readable Rust and one long
line — cheap at the price. `wasm-opt` is unmeasured; it is not installed here
and the project does not use it.

**Consequence for D1.** The playground is deliberately *one* shared module that
dispatches by id. The expander must not join it, or every page carrying any
example pays 360 KB for a struct nobody asked it to expand. It wants to be a
second module, fetched only on derive pages. That is a change to D1's shape,
not a violation of its finding.

Speed is not a concern: both expansions of the struct above take ~35 ms in
node, cold.

### Two traps, both cheap to gate

1. **`prettyplease` must be 0.3.** 0.2 is built on `syn` 2 and `serde_derive`
   is on `syn` 3; mixing them fails with a type mismatch inside
   `prettyplease::unparse`, which reads as a `syn` bug and is not one.
2. **The vendored-as-lib `version` is load-bearing.** `serde_derive` names its
   private module with `env!("CARGO_PKG_VERSION_PATCH")` — that is where
   `_serde::__private229` comes from. If the harness crate's version drifts from
   the pinned one, every expansion on the site is silently wrong in a way that
   still compiles. `cargo xtask coverage` should assert the two are equal.
