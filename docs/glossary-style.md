# Writing glossary entries

A glossary entry quotes a **glossary source** — a crate `serde_derive` reads as
vocabulary and this project never annotates ([D9](decisions.md)). Entries live
in `glossary/<crate>.toml` and are checked by `cargo xtask coverage` alongside
everything else.

## An entry is not an annotation

| | annotation | glossary entry |
|---|---|---|
| claims coverage | yes, on a `coverage` source | never |
| has a `kind`, `tracks`, `course_unit` | yes | no |
| range is | a span of the crate being explained | the lines that *define* one borrowed item |
| body explains | what this code does | what the type is, and why `serde_derive` cares |

The last row is the one that matters. An annotation explains code the reader is
reading. An entry explains a name the reader has just tripped over on the way
to something else, and its job is to get them back to the code they came from.

## Shape

```toml
[[entry]]
id    = "syn::DeriveInput"
file  = "src/derive.rs"
lines = "10-20"
title = "What a derive macro is handed"
see_also = ["syn::Data", "syn::Generics"]
body = """
The whole input to `#[derive(Serialize)]`, already parsed. …
"""
```

- **`id` is crate-qualified**, always. `Ident` means two different things
  depending on who you ask, and both are in this glossary.
- **`lines` must contain the declaration**, not the rustdoc above it. The gate
  checks this (see below) because syn's doc comments name the item they
  document, often several times, so a range that lands in the prose looks right
  and reads right and is wrong.
- **The quoted text is never copied into the toml.** The renderer reads it from
  the pinned tree at build time. A copy here would be a second thing to keep
  true, and the pin exists so it does not have to be.
- **`title` is a row label**, in the same voice as an annotation title: `What a
  derive macro is handed`, not `The DeriveInput struct`.
- **Bodies are short.** 40–80 words. An entry that needs more is usually an
  annotation in the wrong file.

## Length

Quote the whole definition, however long. `syn::Expr` is a 169-line syntax tree
enum and its entry quotes all 169; the renderer caps the height and lets it
scroll. Quoting a *representative part* of a definition would make the site
say something untrue in the one place it is showing someone else's code.

## What the gates enforce

| check | why |
|---|---|
| the named source is pinned with `role = "glossary"` | an entry quoting a crate this project annotates would be claiming coverage through the back door |
| every range resolves in the pinned tree | the same protection annotations have had since phase 0 |
| the range **declares** the item, not just mentions it | four of the first 62 entries pointed at rustdoc; all four looked fine |
| ids are unique, `see_also` resolves | a glossary is navigated by jumping, and a dead link is a dead end |
| annotations' `glossary = [...]` citations resolve | the citation is the reader's only route to a definition the annotated crate does not contain |

## Citing one

From an annotation:

```toml
glossary = ["syn::DeriveInput", "quote::quote"]
```

It renders under the body as `reads syn::DeriveInput, quote::quote`, linking
into the glossary page. Cite the vocabulary an annotation actually leans on,
not everything it mentions — a citation is a promise that the reader needs that
definition to follow this paragraph.
