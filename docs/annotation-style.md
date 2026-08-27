# Annotation style guide

Conventions for writing the ~1,195 annotations. Consistency matters more here
than in most projects, because the reader meets these back to back for hours.

## Length

| kind | target | notes |
|---|---|---|
| `doc-contract` | 100–250 words | The source already has prose. Add what it does *not* say: why the contract is shaped this way, what breaks otherwise. |
| `trait-item` | 80–200 words | One idea per annotation. If you need two, split the range. |
| `macro-def` | 200–400 words | The expensive one. Explain it thoroughly — 106 invocations will point back here. |
| `macro-use` | 1–3 sentences | What this instance produces, and a `macro_def` link to its definition. Never re-explain the macro. |
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

## Writing macro-use rows

Adjacent `macro-use` annotations that share a `macro_def` are merged by the
renderer into **one block**, with the full span of source on the left and one
row per annotation on the right. Hovering a row lights up exactly the lines it
claims. Sixteen `primitive_impl!` invocations become one section with seven
rows; `de/impls.rs` has 106 invocations and would otherwise be 106 blocks.

Two consequences for how you write them:

- **The title is a row label, not a heading.** It is set in monospace next to
  the line range, so keep it short and concrete — `isize → serialize_i64,
  widened`, not `A note on platform-dependent integer widths`.
- **Group by what you have to say, not by invocation.** A row may claim five
  invocations if one sentence covers all five. Splitting `i8` through `i128`
  into five rows saying the same thing is the filler this guide bans; splitting
  `isize` out because it alone carries a cast is the point.

Merging only happens across a **contiguous** span. If a run of invocations is
interrupted by an unannotated line, the renderer splits the block rather than
swallowing the gap, so an accidental split is visible rather than silent.

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
  `examples/compile_fail/tests/ui/` with the expected diagnostic committed:
  `trybuild` compiles each case under `cargo test`, asserts it fails, and diffs
  rustc's output against the `.stderr` beside it, so a case that quietly starts
  compiling breaks the build. Start the file with a comment saying what the
  case shows and what the fix is — the whole file is printed on the page, and
  the diagnostic quotes it by line number. After a rustc upgrade the wording
  may change; regenerate with `TRYBUILD=overwrite cargo test -p compile_fail`
  and read the new output before committing it, because a changed message is
  sometimes a changed rule.
- Keep them under ~120 lines. Past that, the reader is studying the example
  instead of the crate.

## Checklist before marking a file complete

- [ ] `cargo xtask coverage` reports the file at 100% with no warnings
- [ ] Every `macro-use` has a `macro_def` link to its definition (the coverage
      gate fails without one)
- [ ] Every non-obvious claim has an example or a verified reference
- [ ] The file reads end to end without assuming anything not yet introduced,
      or explicitly links forward when it must
- [ ] Add it to `complete` in `annotations/manifest.toml` — this makes future
      gaps a hard CI failure

## Course units

`annotations/course.toml` holds the course track: one record per unit, in
teaching order. A unit's *content* is the annotations tagged with its id in
`course_unit`, so writing a unit means writing its framing — `summary` for the
index, `body` for the unit page — and tagging the annotations that belong to it.

Three fields carry rules rather than prose:

- **`supplement`** is the honesty label, and the coverage gate checks it against
  the store. `none` means every part of the unit comes from serde_core and at
  least one annotation is tagged to it; `partial` means the crate shows some of
  it and written material fills the rest; `full` means the crate does not
  exercise the topic at all, and *no* annotation may be tagged to the unit. If
  you find yourself wanting to tag one annotation to a `full` unit, the unit is
  `partial` — change the label rather than the gate.
- **`prereqs`** may only name earlier units. That keeps the unit graph acyclic
  by construction, and it is the check that catches a re-ordering that broke the
  sequence.
- **`status`** is `planned` until both the framing and the supplementary
  material exist. Planned units are a warning in `cargo xtask coverage` and are
  labelled in the UI, so an unwritten unit is visible rather than empty.

Within a unit, annotations are ordered by the prereq graph first and position
second, with position taken from `reading_order` at the top of the registry —
dependency order over the files, because path order puts `de/` before `ser/`
and every explanation of `Deserializer` leans on the serializer half.

When an annotation's prereq lives in a *later* unit, the gate warns and the unit
page tells the reader so explicitly. Some of that is unavoidable: the prereq
graph was built for the reference track, and no single ordering satisfies both.
A warning that can be removed by re-tagging the annotation into the unit that
actually teaches it should be.
