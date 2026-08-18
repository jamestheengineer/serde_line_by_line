# Annotation style guide

Conventions for writing the ~1,195 annotations. Consistency matters more here
than in most projects, because the reader meets these back to back for hours.

## Length

| kind | target | notes |
|---|---|---|
| `doc-contract` | 100–250 words | The source already has prose. Add what it does *not* say: why the contract is shaped this way, what breaks otherwise. |
| `trait-item` | 80–200 words | One idea per annotation. If you need two, split the range. |
| `macro-def` | 200–400 words | The expensive one. Explain it thoroughly — 106 invocations will point back here. |
| `macro-use` | 1–3 sentences | What this instance produces, and a `prereqs` link to its `macro-def`. Never re-explain the macro. |
| `impl` | 60–150 words | Focus on what is surprising about *this* impl. |
| `plumbing` / `cfg-gate` | 40–120 words | Short. Say what it is for and move on. |

A span of more than ~40 lines usually means the annotation is doing too much.
Split it.

## Voice

- **Second person, present tense.** "You are looking at…", not "The reader will
  observe…".
- **Lead with the point.** First sentence says what this code is for. Context
  and caveats follow. Do not build to a reveal.
- **Explain the *why*, not the *what*.** `type Ok;` does not need "this declares
  an associated type named Ok". It needs "the format picks this type, and here
  is what changes if it picks `()` versus `String`".
- **Name the surprise.** If something took you fifteen minutes to understand,
  say so and say why. That is the most valuable sentence in the annotation.
- **No filler.** Cut "it is important to note that", "as we can see", "simply".
- **Do not flatter the code.** "Elegant" and "beautiful" teach nothing. Describe
  the constraint it satisfies instead.

## Accuracy rules

1. **Never state a compile-time or runtime behavior you have not verified.** If
   an annotation claims something happens, there should be an example proving
   it, or you should have checked it.
2. **Quote line numbers only via `lines`.** Do not write "on line 412" in prose;
   the range is data, and prose drifts.
3. **Cross-reference with `prereqs`, not prose links.** The renderer builds
   navigation from the graph.
4. **When upstream is wrong or dated, say so plainly** and link the issue. The
   `Content` enum in `private/content.rs` carries an "obsoleted by" note; that
   is worth surfacing, not hiding.

## Rust-teaching sidebars

An annotation may teach a language feature, but the *primary* job is always
explaining this code. If a feature needs more than a paragraph, it belongs in a
course-track supplementary unit with its own example, not inline.

Tag every feature you touch in `rust_features` even when you only mention it —
that field powers the "where else does this appear?" index, which is how a
reader finds the other 723 places `'de` shows up.

## Examples

- One example per idea. An example that demonstrates three things demonstrates
  none.
- Examples must **return `String`, never print**, so they run identically under
  `cargo test` and as WASM.
- Prefer an example that *fails* to compile when that is the lesson. Put it in
  `examples/compile_fail/` with the expected diagnostic committed.
- Keep them under ~120 lines. Past that, the reader is studying the example
  instead of the crate.

## Checklist before marking a file complete

- [ ] `cargo xtask coverage` reports the file at 100% with no warnings
- [ ] Every `macro-use` has a `prereqs` link to its `macro-def`
- [ ] Every non-obvious claim has an example or a verified reference
- [ ] The file reads end to end without assuming anything not yet introduced,
      or explicitly links forward when it must
- [ ] Add it to `complete` in `annotations/manifest.toml` — this makes future
      gaps a hard CI failure
