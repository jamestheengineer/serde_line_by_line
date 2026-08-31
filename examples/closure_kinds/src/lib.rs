//! Example: `closure_kinds` — `Fn`, `FnMut`, `FnOnce`, and what a closure is.
//!
//! The other half of unit 14, and the half serde_core cannot teach at all. The
//! crate writes thirteen closures in 12,037 lines, every one of them an argument
//! handed straight to a std combinator, and it declares no `Fn` bound and no
//! `dyn Fn` anywhere — it takes a `Visitor` where application code would take a
//! closure. Section 6 is that inventory, counted rather than asserted.
//!
//! The one idea being made observable: **a closure is a struct the compiler
//! wrote for you, holding what it captured, with one method.** Everything else
//! follows from that. Its size is the size of its captures. There are three
//! traits because there are three ways that method can take `self`. Two
//! closures written identically are two different types, for the same reason
//! two structs with identical fields are.
//!
//! Section 5 is the bridge: a closure has exactly one call signature, which is
//! the whole reason serde is built out of visitors instead.
//!
//! What a program cannot show is the rejections, so two of them live in
//! `examples/compile_fail/` with rustc's own diagnostics committed: two
//! closures rejected from one `vec![]`, and a counting closure offered where
//! `Fn` was demanded.
//!
//! No dependencies, nothing from serde_core.

use std::fmt::Write as _;
use std::marker::PhantomData;

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Sizes are reported in pointer-widths, never bytes.
///
/// This file runs on a 64-bit host under `cargo test` and on wasm32 in the
/// browser, where a pointer is four bytes rather than eight, so a byte count
/// would print two different transcripts. Everything measured below is built
/// out of pointers and `usize`, so the word count is the same number in both
/// places — and words are the unit that makes the point anyway: a captured
/// `&String` is one, a captured `String` is three.
fn words<T: ?Sized>(value: &T) -> usize {
    std::mem::size_of_val(value) / std::mem::size_of::<usize>()
}

fn heading(out: &mut String, title: &str) {
    let _ = write!(out, "\n{title}\n");
    // Counted in characters, not bytes: these headings contain em dashes and
    // `len()` would overrun the rule by two columns each.
    let _ = writeln!(out, "{}", "-".repeat(title.chars().count()));
}

// ---------------------------------------------------------------------------
// 1. A closure is a struct, and its fields are what it captured
// ---------------------------------------------------------------------------

fn a_closure_is_a_struct(out: &mut String) {
    heading(out, "1. A closure is a struct the compiler wrote");

    let name = String::from("serde_core");
    let heap = name.as_ptr();

    // Captures nothing. There is no state to store, so there is nothing to
    // store it in.
    let constant = || 29_usize;

    // Captures `name` by shared reference: the body only reads it, so that is
    // all the compiler takes. One pointer.
    let borrowing = || name.len();
    let seen_by_borrow = borrowing();
    // Measured here rather than below, because the borrow has to be finished
    // before the next line can move `name` — the compile error otherwise is
    // E0505, and it is the same rule unit 06 met with `push_str`.
    let borrowing_words = words(&borrowing);

    // `move` takes the `String` itself — pointer, length, capacity — which is
    // the same three words unit 06 measured across an ordinary move.
    let owning = move || name.as_ptr();

    let _ = writeln!(out, "  {:<34} {:>5}", "closure", "words");
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "|| 29             (no capture)",
        words(&constant)
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "|| name.len()     (&String)", borrowing_words
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "move || name.as_ptr() (String)",
        words(&owning)
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "String (for comparison)",
        words(&String::new())
    );

    let _ = writeln!(
        out,
        "\nborrowing closure read the original: {}",
        yes(seen_by_borrow == 10)
    );
    let _ = writeln!(
        out,
        "moved closure holds the same buffer:  {}",
        yes(owning() == heap)
    );

    let _ = writeln!(
        out,
        "\nThe sizes are the whole lesson. A closure with nothing to remember is\n\
         zero bytes — it exists only in the type system, and calling it compiles\n\
         to calling a function. Capturing by reference costs one pointer.\n\
         Capturing by value costs whatever the value costs, and the `String`\n\
         went in the way it goes anywhere else: three words copied, the same\n\
         heap buffer, no allocation. What varies is not how closures work; it is\n\
         what you asked to be remembered.\n\n\
         Nothing here is magic syntax. `|n| n + captured` is a struct with one\n\
         field and one method, and the only reason you cannot name its type is\n\
         that the compiler never gave it a name to say."
    );
}

// ---------------------------------------------------------------------------
// 2. Three traits, because there are three ways to take self
// ---------------------------------------------------------------------------

/// Requires `Fn`: calls its argument repeatedly, sharing it.
fn map_three<F: Fn(u32) -> u32>(f: F) -> Vec<u32> {
    (1..=3).map(f).collect()
}

/// Requires `FnMut`: the closure is allowed to change something, so this needs
/// its own exclusive access to it — hence `mut f`.
fn feed<F: FnMut(&str)>(items: &[&str], mut f: F) {
    for item in items {
        f(item);
    }
}

/// Requires only `FnOnce`: calls its argument exactly once, and takes it by
/// value to do so. This is the weakest demand a caller can make.
fn finish<F: FnOnce() -> String>(f: F) -> String {
    f()
}

fn three_traits(out: &mut String) {
    heading(out, "2. Three traits, because there are three receivers");

    // Written as a table of data rather than four format strings, so the
    // columns are declared once and the rows cannot drift out of alignment.
    let rows = [
        ("trait", "receiver", "may", "callable"),
        ("FnOnce", "self", "move its captures out", "once"),
        (
            "FnMut",
            "&mut self",
            "mutate its captures",
            "many, exclusively",
        ),
        ("Fn", "&self", "read its captures", "many, shared"),
    ];
    for (name, receiver, may, callable) in rows {
        let _ = writeln!(out, "  {name:<9} {receiver:<12} {may:<24} {callable}");
    }

    let offset = 100;
    let _ = writeln!(
        out,
        "\nFn:     map_three(|n| n + offset) -> {:?}",
        map_three(|n| n + offset)
    );

    let mut log = Vec::new();
    feed(&["alpha", "beta"], |s| log.push(s.to_uppercase()));
    // Readable again here: the closure's `&mut log` ended at its last use.
    let _ = writeln!(out, "FnMut:  log after two calls        -> {log:?}");

    let owned = String::from("moved out of the closure");
    let _ = writeln!(
        out,
        "FnOnce: finish(move || owned)      -> {:?}",
        finish(move || owned)
    );

    // A closure that neither mutates nor consumes satisfies all three bounds,
    // and non-capturing closures are `Copy`, so it can be passed three times.
    let plain = |n: u32| n * 2;
    let _ = writeln!(
        out,
        "\none closure, three bounds: Fn {:?}  FnMut {}  FnOnce {:?}",
        map_three(plain),
        {
            let mut collected = Vec::new();
            feed(&["x"], |s| collected.push(plain(s.len() as u32)));
            format!("{collected:?}")
        },
        finish(move || plain(21).to_string())
    );

    let _ = writeln!(
        out,
        "\nThe three traits are not three kinds of closure. They are three\n\
         questions about the one method: does calling it need to consume the\n\
         struct, mutate it, or only read it? So they nest — `Fn: FnMut` and\n\
         `FnMut: FnOnce` — and every closure implements as many of them as its\n\
         body allows. `|n| n * 2` implements all three, which is why `plain`\n\
         went into all three functions above.\n\n\
         The direction matters when you are the one writing the bound. Ask for\n\
         `FnOnce` and you may call it once but almost anything may be passed;\n\
         ask for `Fn` and you may call it whenever you like, including from\n\
         several threads, but a closure that counts something is refused. Take\n\
         the weakest bound that does the job — `Option::unwrap_or_else` asks for\n\
         `FnOnce` because it calls at most once, and `Iterator::map` asks for\n\
         `FnMut` because it calls repeatedly but never concurrently."
    );
}

// ---------------------------------------------------------------------------
// 3. Every closure has its own type
// ---------------------------------------------------------------------------

fn own_type(out: &mut String) {
    heading(out, "3. Every closure is its own type — hence Box<dyn Fn>");

    let add_one = |n: u32| n + 1;
    let also_add_one = |n: u32| n + 1;
    // The capture is a `usize` deliberately: sizes below are printed in whole
    // words, and a `u32` capture would round down to zero and say something
    // false about a closure that plainly carries data.
    let scale: usize = 3;
    let scaled = move |n: u32| n * scale as u32;

    let _ = writeln!(
        out,
        "add_one and also_add_one agree on 10: {}",
        yes(add_one(10) == also_add_one(10))
    );

    // Identical text, identical behaviour, two distinct types. These two would
    // in fact go into one `Vec` — capturing nothing, they both coerce to
    // `fn(u32) -> u32` — but `scaled` carries a capture and cannot, so the
    // collection has to be boxed. The compile_fail case is the same `vec![]`
    // with the coercion taken away.
    let table: Vec<Box<dyn Fn(u32) -> u32>> =
        vec![Box::new(add_one), Box::new(also_add_one), Box::new(scaled)];
    let applied: Vec<u32> = table.iter().map(|f| f(10)).collect();
    let _ = writeln!(out, "through Vec<Box<dyn Fn>> on 10:       {applied:?}");

    // Only a closure that captures nothing can become a plain function
    // pointer: there is nothing left to carry.
    let as_pointer: fn(u32) -> u32 = add_one;

    let _ = writeln!(out, "\n  {:<34} {:>5}", "value", "words");
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "add_one (the closure itself)",
        words(&add_one)
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "scaled  (captures one usize)",
        words(&scaled)
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "fn(u32) -> u32 pointer",
        words(&as_pointer)
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "Box<dyn Fn(u32) -> u32>",
        words(&table[0])
    );
    let _ = writeln!(
        out,
        "  {:<34} {:>5}",
        "PhantomData<u32>",
        words(&PhantomData::<u32>)
    );

    let _ = writeln!(
        out,
        "\nA `Box<dyn Fn>` is two words wide whatever it holds: a pointer to the\n\
         closure and a pointer to its vtable. That is the cost of putting\n\
         different closures in one `Vec` — an allocation each and an indirect\n\
         call — and the reason `impl Fn` is the default choice. A generic\n\
         `F: Fn(u32) -> u32` is compiled once per closure type, with the body\n\
         inlined and the zero-sized ones vanishing entirely, which is unit 11's\n\
         monomorphization arriving from the other direction: there, one macro\n\
         wrote many impls; here, one function becomes many machine functions.\n\n\
         The two `add_one` closures above are a special case worth knowing: a\n\
         closure that captures nothing has nothing to carry, so it coerces to a\n\
         plain `fn(u32) -> u32` and several of them can share one `Vec` after\n\
         all. Give either of them a capture and that door closes — which is the\n\
         `closure_types_differ` case in `examples/compile_fail`, where rustc\n\
         says it plainly: no two closures, even if identical, have the same\n\
         type.\n\n\
         `PhantomData` measuring zero next to a non-capturing closure is not a\n\
         coincidence. Both are types carrying no data, and serde uses the first\n\
         exactly where you would use the second — `impl DeserializeSeed for\n\
         PhantomData<T>` is the seed that captured nothing."
    );
}

// ---------------------------------------------------------------------------
// 4. Capture is decided per variable, and `move` changes only how
// ---------------------------------------------------------------------------

struct Record {
    name: String,
    hits: u32,
}

/// Returns a closure that outlives this function. `move` is not optional here:
/// `n` dies at the closing brace, so a borrowing closure would be a reference
/// to freed memory — unit 07's rule, arriving in a place that looks like a
/// value rather than a reference.
fn counter(start: u32) -> impl FnMut() -> u32 {
    let mut n = start;
    move || {
        n += 1;
        n
    }
}

fn captures(out: &mut String) {
    heading(out, "4. What is captured is decided one variable at a time");

    let mut record = Record {
        name: String::from("serde_core"),
        hits: 0,
    };

    // Captures `record.name`, not `record`. Since edition 2021 the compiler
    // takes the smallest path the body actually mentions, so the other field
    // is still free.
    let name_len = || record.name.len();
    record.hits += 1;
    let _ = writeln!(
        out,
        "closure holds record.name; record.hits still mutable: {} (now {})",
        yes(name_len() == 10),
        record.hits
    );

    // `move` on a `Copy` type copies. The closure gets its own `hits`, and the
    // original keeps counting from where it was.
    let mut hits = record.hits;
    let mut bump = move |by: u32| {
        hits += by;
        hits
    };
    let inside = (bump(1), bump(2));
    let _ = writeln!(
        out,
        "move || over a u32: closure reached {:?}, outer stayed {}",
        inside, record.hits
    );

    let mut tick = counter(41);
    let _ = writeln!(out, "counter(41) returned a closure: {} {}", tick(), tick());

    let _ = writeln!(
        out,
        "\nThree separate things people call \"capture\". *What* is captured is\n\
         whatever the body names — `record.name`, not all of `record`, which is\n\
         an edition 2021 change and the fix for closures that used to lock a\n\
         whole struct. *How* it is captured is the weakest mode the body needs:\n\
         shared, then mutable, then by value. `move` overrides only that second\n\
         question, forcing by-value, and it is what makes a closure that\n\
         outlives its scope possible at all — `counter` returns something that\n\
         still works after every local in it is gone, because the local moved\n\
         into the struct that was returned.\n\n\
         And `move` on a `Copy` type is a copy. The closure started from the\n\
         same 1 the outer variable held, reached 4 across two calls, and the\n\
         outer variable never moved. The keyword does not mean \"share\";\n\
         it means \"take a value of your own\", which for `u32` is a duplicate\n\
         and for `String` is the only copy there is."
    );
}

// ---------------------------------------------------------------------------
// 5. One signature per closure — which is why serde has visitors
// ---------------------------------------------------------------------------

/// A token, standing in for whatever the format found in the input.
enum Token<'a> {
    Str(&'a str),
    Num(u64),
}

/// What a closure would have to be to do serde's job: two entry points with
/// different argument types, chosen by the driver after it has seen the data.
/// Both take `self` by value, so an implementor is a bundle of `FnOnce`s that
/// happen to share their captures.
trait Sink {
    type Out;
    fn on_str(self, v: &str) -> Self::Out;
    fn on_num(self, v: u64) -> Self::Out;
}

/// Twenty-five lines of `Deserializer::deserialize_any`: look at the data,
/// then call the one method that fits it.
fn drive<S: Sink>(token: Token<'_>, sink: S) -> S::Out {
    match token {
        Token::Str(v) => sink.on_str(v),
        Token::Num(v) => sink.on_num(v),
    }
}

/// A sink with captured state — the shape `DeserializeSeed` has.
struct Describe {
    prefix: String,
}

impl Sink for Describe {
    type Out = String;

    fn on_str(self, v: &str) -> String {
        format!("{}: a string of {} bytes", self.prefix, v.len())
    }

    fn on_num(self, v: u64) -> String {
        format!("{}: the number {}", self.prefix, v)
    }
}

fn why_visitors(out: &mut String) {
    heading(
        out,
        "5. A closure has one signature — serde needs twenty-seven",
    );

    let seed = Describe {
        prefix: String::from("field"),
    };
    let _ = writeln!(out, "drive(Str) -> {:?}", drive(Token::Str("serde"), seed));

    let seed = Describe {
        prefix: String::from("field"),
    };
    let _ = writeln!(out, "drive(Num) -> {:?}", drive(Token::Num(229), seed));

    let _ = writeln!(
        out,
        "\nsize of Describe, captures included: {} words",
        words(&Describe {
            prefix: String::new()
        })
    );

    let _ = writeln!(
        out,
        "\nTry to write `drive` taking a closure and the design collapses at the\n\
         first line: `|v| ...` takes one type of argument, and the whole problem\n\
         is that the driver does not know which type it has until it has looked.\n\
         Passing two closures would work for two cases and not for twenty-seven,\n\
         and they could not share captured state without cloning it.\n\n\
         So `Visitor` is the closure with twenty-seven entry points —\n\
         `visit_bool(self, bool)`, `visit_str(self, &str)`, `visit_seq(self,\n\
         impl SeqAccess)`, and the rest — every one taking `self` by value,\n\
         because exactly one of them will ever run. That is `FnOnce`, written\n\
         out longhand and multiplied by the data model.\n\n\
         `DeserializeSeed` is the other half of the analogy and closer still:\n\
         `fn deserialize(self, deserializer: D)` is a `FnOnce` that carries\n\
         captured state into the deserialization and returns a value. Its impl\n\
         for `PhantomData<T>` is the one that captured nothing — zero words,\n\
         same as `|| 29` in section 1 — which is why the common case costs\n\
         nothing to pass around.\n\n\
         The price is honest: a visitor is thirty lines where a closure is one.\n\
         `serde_derive` exists to write those thirty lines, and that is most of\n\
         what it does."
    );
}

// ---------------------------------------------------------------------------
// 6. Every closure serde_core writes, counted
// ---------------------------------------------------------------------------

fn inventory(out: &mut String) {
    heading(
        out,
        "6. The thirteen closures in serde_core, and their bounds",
    );

    let rows = [
        (
            "de/impls.rs",
            6,
            "map_err(|_| Error::invalid_value(..))",
            "FnOnce",
        ),
        ("de/impls.rs", 2, "map(|vec| ..) / map(|()| ..)", "FnOnce"),
        (
            "de/impls.rs",
            1,
            "ok_or_else(|| Error::custom(..))",
            "FnOnce",
        ),
        (
            "de/impls.rs",
            2,
            "|(ip, port)| SocketAddrV4::new(..)",
            "FnOnce",
        ),
        (
            "ser/mod.rs",
            2,
            "try_for_each(|item| serialize_element(..))",
            "FnMut",
        ),
    ];

    let _ = writeln!(out, "  {:<13} {:>2}  {:<42} bound", "file", "n", "closure");
    for (file, n, what, bound) in rows {
        let _ = writeln!(out, "  {file:<13} {n:>2}  {what:<42} {bound}");
    }
    let total: u32 = rows.iter().map(|r| r.1).sum();
    let _ = writeln!(
        out,
        "  {:<13} {total:>2}  {:<42} no Fn at all",
        "", "in 12,037 lines"
    );

    let _ = writeln!(
        out,
        "\nThirteen closures in two files, and the shape of the list is the\n\
         point. Eleven are `FnOnce` positions — an error constructor on a path\n\
         that has already failed, run at most once. Two are `FnMut`, both of\n\
         them the same line in `collect_seq` and `collect_map`, capturing the\n\
         serializer mutably to feed it one element at a time. None is an `Fn`.\n\n\
         Two of them are worth a second look: the `|(ip, port)|` pair are\n\
         arguments to `parse_socket_impl!`, passed through the macro as `$new:\n\
         expr` and landing in `.map($new)` inside the generated impl. A closure\n\
         crossing a macro boundary is unit 11 and this unit meeting — and they\n\
         capture nothing, so they are constructors that could have been function\n\
         pointers, written as closures because the tuple pattern is neater.\n\n\
         The number that is not in the table is the one that matters most:\n\
         serde_core declares zero `Fn`, `FnMut` or `FnOnce` bounds of its own,\n\
         and zero `dyn Fn`. It never asks you for a closure. Where a library\n\
         built around callbacks would take one, this crate takes a `Serialize`,\n\
         a `Visitor`, or a `DeserializeSeed` — a named type with named methods,\n\
         which can have twenty-seven of them, can be implemented for a type you\n\
         do not own, and can be generated by a derive macro. Closures buy\n\
         brevity at one call site. Traits buy everything this crate needed."
    );
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "Closures, from what they compile to. Section 5 is why serde is built\n\
         out of visitors instead, and section 6 counts what the crate writes.\n",
    );

    a_closure_is_a_struct(&mut out);
    three_traits(&mut out);
    own_type(&mut out);
    captures(&mut out);
    why_visitors(&mut out);
    inventory(&mut out);

    out
}
