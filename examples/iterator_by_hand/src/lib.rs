//! Example: `iterator_by_hand` — the protocol behind the `for` loop.
//!
//! Unit 14 is mostly supplementary. serde_core *consumes* iterators in three
//! places and each is worth reading, but none of them teaches the protocol:
//! you meet `iter::Fuse<I>` in a struct field before you have seen what
//! `Iterator` requires, which is one method.
//!
//! The one idea being made observable: **an iterator is a value that is asked
//! for the next item, and asking is the only thing that ever happens.** Chains
//! of adapters compute nothing when they are built, `size_hint` is a claim the
//! caller may act on, and what an iterator does *after* it says `None` is not
//! covered by the trait at all — which is why `SeqDeserializer` wraps its
//! iterator in `Fuse` rather than trusting it.
//!
//! Section 4 is the one to read if you only read one. Sections 4, 5 and 6 are
//! the three places serde_core actually consumes an iterator, each reduced to
//! twenty lines that run.
//!
//! What a program cannot show is the rejections, so two of them live in
//! `examples/compile_fail/` with rustc's own diagnostics committed: a `for`
//! loop consuming the collection it was given, and a `push` while an iterator
//! over the same `Vec` is alive.
//!
//! No dependencies, nothing from serde_core.

use std::cell::Cell;
use std::fmt::Write as _;

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn heading(out: &mut String, title: &str) {
    let _ = write!(out, "\n{title}\n");
    // Counted in characters, not bytes: these headings contain em dashes and
    // `len()` would overrun the rule by two columns each.
    let _ = writeln!(out, "{}", "-".repeat(title.chars().count()));
}

// ---------------------------------------------------------------------------
// 1. The trait is one method, and everything else is built on it
// ---------------------------------------------------------------------------

/// Splits a record on `;`, yielding borrows of the input rather than copies —
/// the same shape as `Parser` in unit 07, now wearing the iterator protocol.
struct Fields<'a> {
    rest: &'a str,
    done: bool,
}

impl<'a> Fields<'a> {
    fn new(input: &'a str) -> Self {
        Fields {
            rest: input,
            done: input.is_empty(),
        }
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = &'a str;

    /// The whole implementation. `Iterator` has one required method and about
    /// seventy provided ones, so writing this earns `for`, `map`, `collect`,
    /// `zip`, `rev`-if-you-add-one-more-method, and the rest at once. That is
    /// the same default-method economy as unit 05, at a much larger scale.
    fn next(&mut self) -> Option<&'a str> {
        if self.done {
            return None;
        }
        match self.rest.find(';') {
            Some(i) => {
                let field = &self.rest[..i];
                self.rest = &self.rest[i + 1..];
                Some(field)
            }
            None => {
                self.done = true;
                Some(self.rest)
            }
        }
    }
}

fn one_method(out: &mut String) {
    heading(
        out,
        "1. The trait is one method — everything else is provided",
    );

    let record = "alpha;beta;gamma";

    let collected: Vec<&str> = Fields::new(record).collect();
    let _ = writeln!(out, "collect()          {collected:?}");
    let _ = writeln!(out, "count()            {}", Fields::new(record).count());
    let _ = writeln!(
        out,
        "map + max_by_key   {:?}",
        Fields::new(record).max_by_key(|f| f.len())
    );
    let _ = writeln!(
        out,
        "enumerate + last   {:?}",
        Fields::new(record).enumerate().last()
    );
    let _ = writeln!(
        out,
        "\nOne `next` was written. `collect`, `count`, `max_by_key`, `enumerate`\n\
         and `last` all came with the trait, and every one of them is defined in\n\
         terms of calling `next` until it returns `None`. `for f in it` is not\n\
         syntax for a collection — it is `IntoIterator::into_iter` followed by a\n\
         loop over `next`, which is why it works on anything that implements\n\
         this and on nothing that does not."
    );
}

// ---------------------------------------------------------------------------
// 2. Adapters compute nothing until something asks
// ---------------------------------------------------------------------------

fn laziness(out: &mut String) {
    heading(out, "2. Nothing happens until someone calls next()");

    // Counts how many times the closures below actually run. `Cell` because
    // the closures only take `&` to it and still need to write — the one
    // place this file needs interior mutability, and it is here to measure,
    // not to teach.
    let work = Cell::new(0u32);
    let source = [1u64, 2, 3, 4, 5, 6, 7, 8];

    let chain = source
        .iter()
        .map(|n| {
            work.set(work.get() + 1);
            n * n
        })
        .filter(|sq| sq % 2 == 1);

    let _ = writeln!(
        out,
        "  {:<34} closure ran {} times",
        "built map(...).filter(...)",
        work.get()
    );

    let mut chain = chain;
    let first = chain.next();
    let _ = writeln!(
        out,
        "  {:<34} closure ran {} time",
        format!("one next() -> {first:?}"),
        work.get()
    );

    work.set(0);
    let found = source
        .iter()
        .map(|n| {
            work.set(work.get() + 1);
            n * n
        })
        .find(|sq| *sq > 8);
    let _ = writeln!(
        out,
        "  {:<34} closure ran {} times of 8",
        format!("find(|sq| *sq > 8) -> {found:?}"),
        work.get()
    );

    let _ = writeln!(
        out,
        "\nBuilding the chain ran nothing: `map` and `filter` return *values* —\n\
         `Map<Iter<u64>, {{closure}}>` and a `Filter` wrapped around it — that\n\
         hold the source and the closure and do no work. Each `next` on the\n\
         outer one calls `next` on the inner one, exactly as often as it must.\n\
         `find` stopped at the third element because the answer was known then;\n\
         nothing after it was squared. This is why a chain of ten adapters over\n\
         a million items allocates nothing and touches only what it needs, and\n\
         why forgetting to consume one is a warning rather than a slow program."
    );
}

// ---------------------------------------------------------------------------
// 3. size_hint is a claim, and someone acts on it
// ---------------------------------------------------------------------------

fn hints(out: &mut String) {
    heading(out, "3. size_hint — a claim the caller allocates against");

    let source = [1u64, 2, 3, 4, 5];

    let row = |out: &mut String, label: &str, hint: (usize, Option<usize>)| {
        let _ = writeln!(out, "  {label:<28} {:?}", hint);
    };
    row(out, "slice.iter()", source.iter().size_hint());
    row(
        out,
        ".map(square)",
        source.iter().map(|n| n * n).size_hint(),
    );
    row(
        out,
        ".filter(is_odd)",
        source.iter().filter(|n| *n % 2 == 1).size_hint(),
    );
    row(
        out,
        ".chain(slice.iter())",
        source.iter().chain(source.iter()).size_hint(),
    );
    row(out, "Fields::new(record)", Fields::new("a;b;c").size_hint());

    let mapped: Vec<u64> = source.iter().map(|n| n * n).collect();
    let filtered: Vec<u64> = source.iter().copied().filter(|_| true).collect();
    let _ = writeln!(
        out,
        "\ncollect() from map:    {} items, capacity {}",
        mapped.len(),
        mapped.capacity()
    );
    let _ = writeln!(
        out,
        "collect() from filter: {} items, capacity {}",
        filtered.len(),
        filtered.capacity()
    );

    let _ = writeln!(
        out,
        "\nSame five values both times. `map` cannot change how many there are,\n\
         so it forwards the exact hint and `collect` allocates once, for five.\n\
         `filter` can only say \"between none and five\", `collect` believes the\n\
         lower bound, and the `Vec` reaches five by doubling — the capacity\n\
         above is the growth showing. (Capacities are this std's behaviour, not\n\
         a promise.) `Fields` reports the default `(0, None)`, because the\n\
         trait's provided `size_hint` is the honest answer for anything that has\n\
         to look at the data to know.\n\n\
         serde_core's `private/size_hint.rs` is this exact question at the other\n\
         end of the pipe. `from_bounds` returns `Some` only when the two bounds\n\
         agree — \"at least 3, at most 100\" is discarded rather than guessed at —\n\
         and `cautious` then caps whatever survives at one megabyte's worth of\n\
         elements, because past that point the number came from the input file\n\
         and a four-byte header should not be able to ask for a gigabyte."
    );
}

// ---------------------------------------------------------------------------
// 4. What the trait does *not* promise, and the Fuse that fixes it
// ---------------------------------------------------------------------------

/// An iterator that yields, stops, and then starts again. Nothing here is
/// against the rules: `Iterator` says what `next` returns, and says nothing at
/// all about what happens after the first `None`.
struct Resurrecting {
    calls: u32,
}

impl Iterator for Resurrecting {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        self.calls += 1;
        match self.calls {
            1 | 2 => Some(self.calls),
            4 => Some(99),
            _ => None,
        }
    }
}

fn drain(mut it: impl Iterator<Item = u32>, times: usize) -> Vec<Option<u32>> {
    (0..times).map(|_| it.next()).collect()
}

fn fuse(out: &mut String) {
    heading(
        out,
        "4. After None, the trait promises nothing — hence Fuse",
    );

    let raw = drain(Resurrecting { calls: 0 }, 5);
    let fused = drain(Resurrecting { calls: 0 }.fuse(), 5);

    let _ = writeln!(out, "five next() calls, unfused: {raw:?}");
    let _ = writeln!(out, "five next() calls, fused:   {fused:?}");
    let _ = writeln!(
        out,
        "the 99 came back after None:  {}",
        yes(raw.contains(&Some(99)) && !fused.contains(&Some(99)))
    );

    let loop_saw: Vec<u32> = Resurrecting { calls: 0 }.collect();
    let _ = writeln!(out, "what a `for` loop would see: {loop_saw:?}");

    let _ = writeln!(
        out,
        "\nThe `for` loop never notices, because it stops at the first `None` and\n\
         never asks again. That is what makes this a good bug: it is invisible\n\
         to the common case and only appears when the loop is somewhere else.\n\n\
         Which is exactly serde's situation. In `SeqDeserializer` the loop\n\
         belongs to the `Visitor`, and a visitor may call `next_element_seed`\n\
         as many times as it likes, including past the end — then `end()` calls\n\
         `count()` on the same iterator afterwards to check the length. A\n\
         resurrecting iterator would make those two disagree. So the field is\n\
         typed `iter: iter::Fuse<I>`, not `I`: `Fuse` latches a flag the first\n\
         time it sees `None` and returns `None` forever after, which is the\n\
         promise the trait declines to make. It is one word in a struct\n\
         definition and it is load-bearing."
    );
}

// ---------------------------------------------------------------------------
// 5. Option::take as a state machine — de/value.rs's PairVisitor
// ---------------------------------------------------------------------------

/// Hands out a key, then a value, then nothing — the shape `PairVisitor` uses
/// to feed a two-element sequence to a visitor, with no cursor field.
struct Pair<'a> {
    key: Option<&'a str>,
    value: Option<&'a str>,
}

impl<'a> Pair<'a> {
    fn next(&mut self) -> Option<&'a str> {
        if let Some(k) = self.key.take() {
            Some(k)
        } else {
            self.value.take()
        }
    }

    /// Always `Some`, which is what lets the caller `unwrap` it. Two `Option`s
    /// make the count derivable rather than tracked.
    fn size_hint(&self) -> Option<usize> {
        Some(self.key.is_some() as usize + self.value.is_some() as usize)
    }
}

fn state_machine(out: &mut String) {
    heading(out, "5. Two Options as a state machine, not a cursor");

    let mut pair = Pair {
        key: Some("version"),
        value: Some("1.0.229"),
    };

    let _ = writeln!(out, "  {:<8} {:<16} size_hint", "call", "yields");
    for _ in 0..4 {
        let hint = pair.size_hint();
        let got = pair.next();
        let _ = writeln!(
            out,
            "  {:<8} {:<16} {:?}",
            "next()",
            format!("{got:?}"),
            hint
        );
    }

    let _ = writeln!(
        out,
        "\n`take` moves the value out and leaves `None` behind, so the sequence\n\
         key, value, end, end is a consequence of the two fields rather than\n\
         something a counter has to be kept in step with. An index would need a\n\
         `match` with arms that cannot happen; here the impossible states are\n\
         unrepresentable, and the borrow checker is what verifies nothing is\n\
         handed out twice — you cannot `take` from a `None` and get a value.\n\
         `size_hint` is derived from the same two fields, which is why it can\n\
         always return `Some` and the caller in `deserialize_seq` can `unwrap`\n\
         it without a comment apologising."
    );
}

// ---------------------------------------------------------------------------
// 6. Asking twice and stopping — the one-character check in de/impls.rs
// ---------------------------------------------------------------------------

/// Wrong: `len()` counts UTF-8 bytes, so every non-ASCII character fails.
fn one_char_by_len(v: &str) -> bool {
    v.len() == 1
}

/// Right, and wasteful: `count` walks the whole string to answer a question
/// about its first two characters.
fn one_char_by_count(v: &str) -> bool {
    v.chars().count() == 1
}

/// What `Deserialize for char` does. Take two items and match on the pair:
/// "there was a first character and there was no second" is the question,
/// written out.
fn one_char_by_two_nexts(v: &str) -> bool {
    let mut chars = v.chars();
    matches!((chars.next(), chars.next()), (Some(_), None))
}

fn ask_twice(out: &mut String) {
    heading(out, "6. Asking twice, then stopping");

    let _ = writeln!(
        out,
        "  {:<10} {:>7} {:>9} {:>10}",
        "input", "len==1", "count==1", "two nexts"
    );
    for input in ["a", "\u{e9}", "", "ab", "\u{20ac}10"] {
        let _ = writeln!(
            out,
            "  {:<10} {:>7} {:>9} {:>10}",
            format!("{input:?}"),
            yes(one_char_by_len(input)),
            yes(one_char_by_count(input)),
            yes(one_char_by_two_nexts(input)),
        );
    }

    // How much of the string each correct answer actually looks at. `inspect`
    // is a passthrough adapter that runs a closure per item, which makes the
    // work countable without changing what is computed.
    let long = "\u{e9}".repeat(10_000);
    let visited = Cell::new(0u32);

    let by_count = long
        .chars()
        .inspect(|_| visited.set(visited.get() + 1))
        .count()
        == 1;
    let count_visited = visited.get();

    visited.set(0);
    let mut two = long.chars().inspect(|_| visited.set(visited.get() + 1));
    let by_two = matches!((two.next(), two.next()), (Some(_), None));
    let two_visited = visited.get();

    let _ = writeln!(
        out,
        "\non a 10,000-character string, same answer ({}), different cost:",
        yes(by_count == by_two)
    );
    let _ = writeln!(
        out,
        "  chars().count() == 1   visited {count_visited} characters"
    );
    let _ = writeln!(
        out,
        "  two nexts              visited {two_visited} characters"
    );

    let _ = writeln!(
        out,
        "\nThe third cautionary tale, and the smallest. `visit_str` for `char`\n\
         has to decide whether the input holds exactly one character, and the\n\
         two obvious spellings are each wrong in their own way: `len() == 1`\n\
         asks about bytes and rejects every accented letter, and\n\
         `chars().count() == 1` walks the whole thing — ten thousand characters\n\
         above to answer a question about the first two, and a ten-megabyte\n\
         input decoded end to end before it says no.\n\n\
         Taking two items and matching on the pair costs two `next` calls\n\
         whatever the input is, and reads as the question rather than as a\n\
         proxy for it. That is the shape worth stealing: when you want to know\n\
         how many items there are only up to some small number, ask for that\n\
         many and look at what came back. Anything else — empty, or two or more\n\
         — falls through to one error carrying the string that was there."
    );
}

// ---------------------------------------------------------------------------
// 7. What serde_core does instead of closures
// ---------------------------------------------------------------------------

fn instead_of_closures(out: &mut String) {
    heading(out, "7. Why there are no closures here to look at");

    let _ = writeln!(
        out,
        "A closure has one call signature. `|s: &str| ...` takes a `&str` and\n\
         nothing else, and that is the whole reason serde is built out of\n\
         visitors instead. A `Visitor` is what you would write if a closure\n\
         could have twenty-seven entry points with twenty-seven different\n\
         argument types — `visit_bool(bool)`, `visit_u64(u64)`,\n\
         `visit_str(&str)`, `visit_seq(impl SeqAccess)` — and be handed to a\n\
         `Deserializer` that picks one after it has seen the data.\n\n\
         So the trade is visible: application code would pass a closure and take\n\
         one shape of input; serde passes a trait object's worth of methods and\n\
         takes any of them. The cost is that a visitor is thirty lines and a\n\
         closure is one, which is what `serde_derive` exists to write for you.\n\
         The `closure_kinds` example is the other half of this unit, and it is\n\
         where `Fn`, `FnMut` and `FnOnce` are actually taught, because there is\n\
         nothing in this crate to point at for them."
    );
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "The iterator protocol, from the one method it actually requires.\n\
         Sections 4, 5 and 6 are the crate's three iterator lessons, reduced\n\
         to code that runs.\n",
    );

    one_method(&mut out);
    laziness(&mut out);
    hints(&mut out);
    fuse(&mut out);
    state_machine(&mut out);
    ask_twice(&mut out);
    instead_of_closures(&mut out);

    out
}
