# Rust feature vocabulary

Controlled vocabulary for the `rust_features` field on annotations. The coverage
gate parses this file and rejects any slug not listed here, so the documentation
and the checker cannot drift apart.

Format is load-bearing: each entry must be a list item beginning with a
backticked slug.

## Traits and types

- `traits` — declaring and implementing traits
- `supertraits` — `trait Foo: Bar`, and the `Sized` supertrait on `Serializer`
- `associated-types` — `type Ok`, `type Error`; the format chooses the type
- `generic-params` — type parameters on functions, types, and impls
- `where-clauses` — bounds expressed in `where` position
- `blanket-impls` — `impl<T: Bound> Trait for T`, and the coherence rules
- `default-methods` — trait methods with bodies, and when to override them
- `sized-bound` — `Sized`, `?Sized`, and dynamically sized types
- `sealed-traits` — preventing downstream implementations
- `trait-objects` — `dyn Trait`, object safety, vtables
- `marker-types` — `PhantomData` and zero-sized types
- `variance` — covariance and contravariance in lifetime and type params

## Lifetimes and borrowing

- `ownership` — moves, copies, and drop
- `borrowing` — shared and mutable references, the aliasing rules
- `lifetimes` — lifetime parameters and elision
- `lifetime-de` — Serde's `'de` lifetime and zero-copy deserialization
- `hrtb` — higher-ranked trait bounds, `for<'de>`
- `borrow-splitting` — reborrowing and passing `&mut` through call chains

## Macros

- `macro-rules` — declarative macros, matchers, transcribers
- `macro-hygiene` — identifier hygiene and `$crate`
- `macro-repetition` — `$(...)*` and generating repetitive impls
- `proc-macro-boundary` — what `serde_derive` emits and why it lives elsewhere

## Modules, crates, conditional compilation

- `modules` — `mod`, paths, visibility
- `visibility` — `pub`, `pub(crate)`, `#[doc(hidden)]` as a soft-private marker
- `cfg` — `#[cfg]`, `#[cfg_attr]`, feature gates
- `no-std` — `#![no_std]`, `core` vs `alloc` vs `std`
- `feature-unification` — why Cargo features must be additive
- `msrv` — minimum supported Rust version, and version-detecting build scripts

## Errors and control flow

- `result` — `Result`, `?`, and error propagation
- `option` — `Option` and its combinators
- `custom-errors` — designing an error type; `ser::Error` / `de::Error`
- `pattern-matching` — `match`, bindings, exhaustiveness

## Everyday Rust (mostly supplementary units)

- `closures` — `Fn`, `FnMut`, `FnOnce`, captures
- `iterators` — the `Iterator` trait and adapters
- `collections` — `Vec`, `HashMap`, `BTreeMap`
- `strings` — `String`, `&str`, `Cow`, UTF-8 invariants
- `smart-pointers` — `Box`, `Rc`, `Arc`
- `interior-mutability` — `Cell`, `RefCell`, `Mutex`
- `unsafe` — `unsafe` blocks and upholding invariants

## Performance and representation

- `monomorphization` — static dispatch, code bloat, compile-time cost
- `zero-cost` — abstractions that compile away
- `inlining` — `#[inline]` and cross-crate optimization
