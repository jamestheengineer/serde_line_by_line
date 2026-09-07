# Writing narrative units

The narrative track walks `serde_derive` from a `DeriveInput` to an emitted
`impl` ([PLAN.md §11](../PLAN.md)). One unit per file in `narrative/`, named for
its id, checked by `cargo xtask coverage` and rendered by `cargo site`.

## A step is not an annotation

| | annotation | narrative step |
|---|---|---|
| claims coverage | yes, on the `coverage` source | never |
| ranges may overlap | no — a line is claimed once | yes — a walk may pass twice |
| range is | a span of the crate being explained | wherever the path goes next |
| body explains | what this code does | why the reader is standing here |

Overlap is the difference worth holding on to. The reference track's promise is
that every line is claimed exactly once, and its gate enforces both halves. A
walk has no such promise: unit 01 reads the top of `expand_derive_serialize` for
its shape and unit 05 comes back to `build_generics` twelve lines below, and
neither is a mistake.

What is *not* relaxed is the pin. Every step cites a line range in a tree whose
checksum is in `vendor/pin.toml`, and a range that stops resolving fails the
build the same way an annotation's would.

## Shape

```toml
schema = 1

[unit]
id = "06-emitting-serialize"
title = "Emitting Serialize, line by generated line"
expand_case = "struct_rename"
summary = """One or two sentences. Shown on the track index."""
body = """The framing. Markdown."""

[[step]]
id = "s06-len"
source = "serde_derive-1.0.229"
file = "src/ser.rs"
lines = "341-359"
title = "The field count, folded into an expression"
leans_on = ["s06-serialize-struct"]
glossary = ["quote::quote"]
emits = "false as usize + 1 + if Option::is_none(&self.label) { 0 } else { 1 },"
emits_from = "serialize"
body = """…"""
```

- **The file name is the unit id.** There is no registry listing the order,
  because filename order already is the order and a registry would be a second
  place for it to live. The gate checks the two agree.
- **Step ids are unique across the whole walk**, not per unit, because
  `leans_on` addresses them globally.
- **`leans_on` must point backwards** — the [D8](decisions.md) rule, unchanged
  except that the graph now spans two pinned trees.
- **`glossary` cites borrowed vocabulary** exactly as an annotation does
  ([D9](decisions.md)). A step may not *cite a glossary source as its own
  source*: `syn` is quoted in `glossary/`, never walked.

## Crossing into serde_core

A step whose `source` is the coverage source must name the reference-track
annotation it lands in, and its range must sit inside that annotation's range:

```toml
[[step]]
id = "s06-serialize-struct-hint"
source = "serde_core-1.0.229"
file = "src/ser/mod.rs"
lines = "1190-1224"
annotation = "ser-mod-0036"
```

The rule is that the narrative may only walk into ground the reference track has
already claimed. It gets to reuse 12,037 annotated lines instead of explaining
them a second time, the reader gets a link to the full annotation, and the link
cannot rot — move the annotation's boundaries and the gate says so.

Use crossings sparingly. Nine of the walk's 85 steps are crossings, and each one
is a place where a generated line calls something the rest of this site already
explains.

## Quoting generated code

`emits` is a claim about what the cited machinery produces. It is checked, and
what the reader sees is not the string in the store:

1. `cargo site` expands the unit's `expand_case` — or the step's `emits_case`,
   for the rare step that needs a different input — with the real pinned
   `serde_derive`.
2. It locates `emits` in that expansion as a contiguous run of lines, compared
   with leading and trailing whitespace stripped.
3. It renders **the located lines from the expansion**, numbered by their
   position in it.

So a claim that no longer matches fails the build naming the step, and one that
does match is displayed as the genuine bytes. Reindentation by `prettyplease` is
tolerated; a changed token is not.

Two practical notes. Include enough of the block to be unique — a single line
that appears twice will silently match the first. And the match is on the
*formatted* expansion, so the easiest way to write one is to run
`cargo test -p expand` once and copy out of `expand/expected.txt`.

## Voice

The same as an annotation ([annotation-style.md](annotation-style.md)), with one
addition: a step is a *stop*, so it should say what the reader is looking at and
what it produces, not recap the file it lives in. Prefer the measurement to the
adjective — "453 lines coming back against 61 going out" rather than "much more
complex".
