//! Example: `lifetime_basics` — what a lifetime parameter actually says.
//!
//! Course unit 07 is fully supplementary, and for a specific reason: serde_core
//! has 724 lifetime parameters and not one of them is a first example. They are
//! outlives bounds, higher-ranked bounds, and two lifetimes deliberately
//! unified — all of them load-bearing, none of them introductory. So the
//! syntax has to be learned somewhere else, and this is that somewhere.
//!
//! The one idea being made observable: **a lifetime parameter does not create
//! or extend anything.** It relates references that already exist, so the
//! compiler can prove no reference outlives what it points into. Every section
//! below checks that relationship the only way a running program can — by
//! comparing `as_ptr()` and showing the result shares a buffer with the
//! argument it was supposed to borrow from.
//!
//! What a program cannot show is the rejections, since a program that is
//! rejected does not run. Those are in `examples/compile_fail/` with rustc's
//! own diagnostics committed beside them: the two-input signature elision
//! cannot resolve, a reference returned to a dead local, a borrowing struct
//! outliving its input, and a temporary offered where `&'static str` was
//! required.
//!
//! No dependencies, nothing from serde_core. Section 6 is the bridge: a parse
//! that borrows every field out of its input instead of allocating them is
//! exactly what `Deserialize<'de>` does, and unit 08 picks it up there.

use std::fmt::Write as _;

/// The address of a string's first byte. Never printed — the number is
/// meaningless and differs every run — only compared, which is the fact.
fn buffer(s: &str) -> usize {
    s.as_ptr() as usize
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn heading(out: &mut String, title: &str) {
    let _ = write!(out, "\n{title}\n");
    // Counted in characters, not bytes: two of these headings contain an em
    // dash, and `len()` would overrun the rule by two columns.
    let _ = writeln!(out, "{}", "-".repeat(title.chars().count()));
}

// ---------------------------------------------------------------------------
// 1. A lifetime parameter relates references; it does not create them
// ---------------------------------------------------------------------------

/// Both arguments and the result share one lifetime parameter. That is not a
/// claim that the two strings live equally long — it is a request: pick a
/// region both of them cover, and hold the result to it.
fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.len() >= b.len() {
        a
    } else {
        b
    }
}

fn relates(out: &mut String) {
    heading(
        out,
        "1. A lifetime relates references, it does not create them",
    );

    let owned = String::from("a longer string this function owns");
    let literal = "short";

    let longer = longest(&owned, literal);
    let _ = writeln!(
        out,
        "longest(&owned, literal)   result borrows owned:   {}",
        yes(buffer(longer) == buffer(&owned))
    );

    let shorter = longest("ab", literal);
    let _ = writeln!(
        out,
        "longest(\"ab\", literal)     result borrows literal: {}",
        yes(buffer(shorter) == buffer(literal))
    );

    let _ = writeln!(
        out,
        "\nOne signature, two calls, and the returned pointer came from a\n\
         different argument each time. That is why the annotation is needed at\n\
         all: the compiler cannot see inside the body from a call site, so the\n\
         signature has to say \"the result may point into either argument\".\n\
         `'a` is not the lifetime of `owned`, and not a duration. It is a region\n\
         the compiler chooses — at most the overlap of the two arguments' —\n\
         and the result is held to it. At runtime this function is a length\n\
         comparison and a pointer return; the lifetimes are gone by then."
    );
}

// ---------------------------------------------------------------------------
// 2. Elision: the annotation is usually still there, just unwritten
// ---------------------------------------------------------------------------

/// One input reference, so the output can only have come from it. Elision
/// fills in what you would have written.
fn first_line(s: &str) -> &str {
    match s.find('\n') {
        Some(i) => &s[..i],
        None => s,
    }
}

/// Byte for byte the same function with the elided lifetime written out.
/// Clippy is right that this is redundant, which is exactly the point being
/// demonstrated, so the lint is silenced here rather than obeyed.
#[allow(clippy::needless_lifetimes)]
fn first_line_explicit<'a>(s: &'a str) -> &'a str {
    match s.find('\n') {
        Some(i) => &s[..i],
        None => s,
    }
}

struct Doc {
    text: String,
}

impl Doc {
    /// The `&self` rule: when a method takes a reference to `self`, an elided
    /// output lifetime is `self`'s. So this returns a view into `self.text`,
    /// and the compiler will not let it outlive the `Doc`.
    fn headline(&self) -> &str {
        first_line(&self.text)
    }
}

fn elision(out: &mut String) {
    heading(out, "2. Elision writes the annotation you did not");

    let text = String::from("headline\nand the body below it");

    let elided = first_line(&text);
    let written = first_line_explicit(&text);
    let _ = writeln!(
        out,
        "first_line(&text) and first_line_explicit(&text) agree: {}",
        yes(buffer(elided) == buffer(written) && elided == written)
    );
    let _ = writeln!(
        out,
        "  both borrow text: {}",
        yes(buffer(elided) == buffer(&text))
    );

    let doc = Doc { text };
    let head = doc.headline();
    let _ = writeln!(
        out,
        "doc.headline() borrows doc.text:      {}   ({head:?})",
        yes(buffer(head) == buffer(&doc.text))
    );

    let _ = writeln!(
        out,
        "\nThree rules, and they cover most signatures. Every elided input\n\
         reference gets its own lifetime; if there is exactly one input, the\n\
         output gets it; if one of the inputs is `&self`, the output gets\n\
         `self`'s and the others are ignored. Elision never guesses — it applies\n\
         those rules or refuses. `fn pick(a: &str, b: &str) -> &str` is the\n\
         refusal: two candidates, no `&self`, so rustc asks you to say which.\n\
         The `missing_lifetime` case in examples/compile_fail is what it says."
    );
}

// ---------------------------------------------------------------------------
// 3. A lifetime on a struct means "this type is a view"
// ---------------------------------------------------------------------------

/// A parser that owns nothing. `'a` is the whole difference between this and a
/// type holding a `String`: this one cannot outlive the text it was built
/// from, and in exchange it never copies a byte of it.
struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser { input, pos: 0 }
    }

    /// Note the return type: `&'a str`, not `&str`. Elision would have tied it
    /// to `&mut self`, and every field would then be borrowed from the parser —
    /// unusable once the parser is gone, and impossible to collect into a `Vec`
    /// while still calling `next_field`. Written out, the fields borrow the
    /// *input*, and the parser is just a cursor that may be dropped.
    fn next_field(&mut self) -> Option<&'a str> {
        if self.pos >= self.input.len() {
            return None;
        }
        let rest = &self.input[self.pos..];
        let (field, step) = match rest.find(';') {
            Some(i) => (&rest[..i], i + 1),
            None => (rest, rest.len()),
        };
        self.pos += step;
        Some(field)
    }
}

fn views(out: &mut String) {
    heading(out, "3. A lifetime on a struct means the type is a view");

    let record = String::from("alpha;beta;gamma");
    let base = buffer(&record);

    let mut fields = Vec::new();
    {
        let mut parser = Parser::new(&record);
        while let Some(field) = parser.next_field() {
            fields.push(field);
        }
        // The cursor's scope ends at this brace. The fields outlive it without
        // complaint, because they borrow the input rather than the parser.
    }

    let _ = writeln!(out, "  {:<10} {:>6} {:>4}", "field", "offset", "len");
    for field in &fields {
        let _ = writeln!(
            out,
            "  {:<10} {:>6} {:>4}",
            format!("{field:?}"),
            buffer(field) - base,
            field.len()
        );
    }

    let _ = writeln!(
        out,
        "\nThree fields read after the parser was dropped, all pointing into one\n\
         `String` at the offsets above. Nothing was allocated. `Parser<'a>` is\n\
         not a parser that happens to hold a reference — the `'a` is part of the\n\
         type, and it is the compiler's record that values of this type describe\n\
         someone else's memory. Build one from a temporary and the borrow\n\
         checker stops you: that is the `struct_outlives_input` case in\n\
         examples/compile_fail."
    );
}

// ---------------------------------------------------------------------------
// 4. `'long: 'short` — outlives, and why a bound is needed at all
// ---------------------------------------------------------------------------

/// `'long: 'short` reads "`'long` outlives `'short`", and it is what makes
/// returning `long` here legal: a reference valid for the longer region is
/// usable anywhere the shorter one is. Delete the bound and rustc rejects the
/// function, because nothing would connect the two regions.
fn prefer_long<'long: 'short, 'short>(long: &'long str, short: &'short str) -> &'short str {
    if long.len() > 3 {
        long
    } else {
        short
    }
}

/// A place to put a borrow. Storing into it is the same coercion as returning:
/// a `&'long str` may be written where a `&'short str` is expected.
struct Slot<'short> {
    held: &'short str,
}

fn outlives(out: &mut String) {
    heading(
        out,
        "4. `'long: 'short` — a longer borrow fits a shorter slot",
    );

    let outer = String::from("outer buffer, longer lived");

    {
        let inner = String::from("in");
        let picked = prefer_long(&outer, &inner);
        let _ = writeln!(
            out,
            "prefer_long(&outer, &inner)  returned the long one: {}",
            yes(buffer(picked) == buffer(&outer))
        );

        let mut slot = Slot { held: &inner };
        let _ = writeln!(out, "slot starts holding the inner borrow: {:?}", slot.held);
        slot.held = &outer;
        let _ = writeln!(
            out,
            "slot now holds the outer one:          {}",
            yes(buffer(slot.held) == buffer(&outer))
        );
    }

    let _ = writeln!(
        out,
        "\nThe result is typed `&'short str` and it is pointing into `outer`,\n\
         which lives longer than `'short`. That is fine, and the bound is what\n\
         says so: a promise valid until later satisfies a requirement of valid\n\
         until sooner. The direction is the part to hold onto — subtyping on\n\
         lifetimes only ever shortens. `Slot<'short>` shows the same coercion in\n\
         a field, and the assignment that would fail is the reverse one: an\n\
         `&'short str` cannot be stored where `&'long str` is required.\n\
         serde_core writes this bound as `impl<'de: 'a, 'a>` — the input buffer\n\
         outlives the borrow taken out of it, which is unit 08."
    );
}

// ---------------------------------------------------------------------------
// 5. `'static` is a claim about the referent, not a property of the value
// ---------------------------------------------------------------------------

/// Holds a reference that is valid for the whole program.
struct Remembered {
    text: &'static str,
}

/// `T: 'static` is a *different* question from `&'static T`. It asks whether
/// `T` contains any borrow that could expire — and an owned `String` does not,
/// so it qualifies while living and dying like anything else.
fn owns_no_borrows<T: 'static>(_value: &T) -> bool {
    true
}

fn statics(out: &mut String) {
    heading(
        out,
        "5. `'static` is about the referent, not about the value",
    );

    let remembered;
    {
        // A literal is compiled into the binary, so this reference is valid for
        // the whole run, and may escape the block that named it.
        let literal: &'static str = "baked into the binary";
        remembered = Remembered { text: literal };
    }
    let _ = writeln!(out, "read after its block ended: {:?}", remembered.text);

    let owned = String::from("allocated at runtime, dropped at the brace");
    let _ = writeln!(
        out,
        "String satisfies T: 'static (it borrows nothing): {}",
        yes(owns_no_borrows(&owned))
    );
    let _ = writeln!(
        out,
        "...and is still dropped at the end of this function, {} bytes and all",
        owned.len()
    );

    let _ = writeln!(
        out,
        "\nTwo different things wear the same word. `&'static str` says the\n\
         *pointed-at bytes* last as long as the program — true of literals,\n\
         which is why the reference survived its block. `T: 'static` says the\n\
         type contains no borrow that can expire, which a `String` satisfies\n\
         despite being freed a few lines from now. Neither one means \"lives\n\
         forever\", and reaching for `&'static str` to quiet an error usually\n\
         makes the signature demand something the caller cannot supply. The\n\
         `not_static` case in examples/compile_fail is that mistake meeting a\n\
         perfectly ordinary `String`."
    );
}

// ---------------------------------------------------------------------------
// 6. Where this is going: borrowing out of an input instead of copying it
// ---------------------------------------------------------------------------

/// Every returned `&str` points into `input`. No allocation happens for the
/// keys or the values — only for the `Vec` holding the pairs.
fn borrowed_pairs(input: &str) -> Vec<(&str, &str)> {
    input
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .collect()
}

/// The same parse, copying instead. Two allocations per field.
fn owned_pairs(input: &str) -> Vec<(String, String)> {
    borrowed_pairs(input)
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn toward_de(out: &mut String) {
    heading(out, "6. Borrowing out of the input, which is unit 08");

    let input = String::from("name=serde_core;version=1.0.229;deps=0");
    let base = buffer(&input);

    let borrowed = borrowed_pairs(&input);
    let owned = owned_pairs(&input);

    let _ = writeln!(
        out,
        "  {:<10} {:<12} {:>6}  value shares the input buffer",
        "key", "value", "offset"
    );
    for ((k, v), (_, owned_v)) in borrowed.iter().zip(owned.iter()) {
        let _ = writeln!(
            out,
            "  {:<10} {:<12} {:>6}  borrowed {} / owned {}",
            k,
            v,
            buffer(v) - base,
            yes(buffer(v) >= base && buffer(v) < base + input.len()),
            yes(buffer(owned_v.as_str()) >= base && buffer(owned_v.as_str()) < base + input.len())
        );
    }

    let _ = writeln!(
        out,
        "\nSame parse twice. The borrowed values sit at offsets inside the one\n\
         input buffer; the owned ones are six fresh allocations holding equal\n\
         bytes. The signature is the entire difference — `fn borrowed_pairs(input:\n\
         &str) -> Vec<(&str, &str)>` elides to one lifetime shared by the input\n\
         and everything in the result, so the compiler already knows those pairs\n\
         cannot outlive the text.\n\n\
         That is serde's zero-copy story with the serde removed. In unit 08 the\n\
         input is a format's buffer, the lifetime has a name — `'de` — and\n\
         `Deserialize<'de>` is the trait that says a type can be built by\n\
         borrowing out of it. Everything there is a longer version of this\n\
         function's signature."
    );
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "Lifetimes, checked against the pointers they describe.\n\
         Addresses are compared and offsets printed, never raw addresses: the\n\
         comparisons are the facts.\n",
    );

    relates(&mut out);
    elision(&mut out);
    views(&mut out);
    outlives(&mut out);
    statics(&mut out);
    toward_de(&mut out);

    out
}
