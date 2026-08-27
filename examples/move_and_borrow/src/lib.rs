//! Example: `move_and_borrow` — ownership and borrowing, made observable.
//!
//! Course unit 06 is fully supplementary: serde_core is generic trait code and
//! never does the ordinary imperative ownership work a reader has to meet
//! first. So this example invents its own subject matter, and tries to do the
//! one thing prose about ownership usually cannot — *show* it.
//!
//! Two facts are being made visible.
//!
//! **A move transfers ownership, not bytes.** `let b = a;` on a `String` copies
//! three machine words — pointer, length, capacity — and leaves the heap buffer
//! exactly where it was. The example proves it by comparing `as_ptr()` across
//! moves: the address is unchanged. `clone()` is the operation that costs
//! something, and it shows up as a different address.
//!
//! **Ownership is the answer to "who runs `Drop`, and when".** A type with a
//! `Drop` impl that appends to a shared log turns scope rules into a printed
//! sequence: a value moved into a function dies inside that function, locals
//! die in reverse declaration order, and a temporary dies at the end of the
//! statement that made it.
//!
//! The borrow half is the same story from the other side: a `&str` is a view
//! into a buffer someone else owns, and the aliasing rule exists because
//! `push_str` may move that buffer. What the compiler rejects is in
//! `examples/compile_fail/` with its diagnostics committed — the errors here
//! are described, and there they are real.
//!
//! No dependencies, nothing from serde_core. Unit 08 is where this comes back:
//! `'de` is exactly the borrow in section 6, carried across a format boundary.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

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
    let _ = writeln!(out, "{}", "-".repeat(title.len()));
}

// ---------------------------------------------------------------------------
// 1. A move is a transfer of ownership, not a copy of the data
// ---------------------------------------------------------------------------

/// Takes ownership and gives it back. The buffer never notices.
fn round_trip(s: String) -> String {
    s
}

fn moves(out: &mut String) {
    heading(out, "1. A move transfers ownership, not bytes");

    let original = String::from("a heap buffer that never moves");
    let start = buffer(&original);

    // `original` is not readable after this line: the value moved. Nothing
    // happened at runtime — the compiler simply stopped letting you use the
    // old name.
    let moved = original;
    let after_move = buffer(&moved);

    let returned = round_trip(moved);
    let after_call = buffer(&returned);

    let copy = returned.clone();
    let cloned = buffer(&copy);

    let _ = writeln!(
        out,
        "let moved = original;      same buffer: {}",
        yes(start == after_move)
    );
    let _ = writeln!(
        out,
        "round_trip(moved)          same buffer: {}",
        yes(start == after_call)
    );
    let _ = writeln!(
        out,
        "returned.clone()           same buffer: {}",
        yes(start == cloned)
    );
    let _ = writeln!(out, "  ...but equal contents:   {}", yes(returned == copy));
    let _ = writeln!(
        out,
        "\nTwo moves and a function call copied 0 bytes of the heap buffer: the\n\
         address never changed, so all three names described the same {} bytes.\n\
         `clone()` is the line that allocates, and it is visible as a different\n\
         address holding equal contents.",
        returned.len()
    );
}

// ---------------------------------------------------------------------------
// 2. Ownership decides who runs Drop, and when
// ---------------------------------------------------------------------------

type Log = Rc<RefCell<Vec<String>>>;

/// A value whose destruction is audible.
struct Tracked {
    name: &'static str,
    log: Log,
}

impl Tracked {
    fn new(name: &'static str, log: &Log) -> Self {
        log.borrow_mut().push(format!("create {name}"));
        Tracked {
            name,
            log: Rc::clone(log),
        }
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        self.log.borrow_mut().push(format!("DROP   {}", self.name));
    }
}

/// Takes the value. The caller cannot use it afterwards, and the compiler is
/// not being pedantic: this function is where the value dies.
fn consume(t: Tracked) {
    t.log.borrow_mut().push(format!("consume({}) body", t.name));
}

/// Takes a view. Nothing is dropped here — this function owns nothing.
fn inspect(t: &Tracked) {
    t.log.borrow_mut().push(format!("inspect({}) body", t.name));
}

fn drops(out: &mut String) {
    heading(out, "2. Ownership decides who runs Drop, and when");

    let log: Log = Rc::new(RefCell::new(Vec::new()));
    {
        let first = Tracked::new("first", &log);
        let second = Tracked::new("second", &log);

        inspect(&second);
        consume(first);

        // A value that is never bound to a name is dropped at the end of the
        // statement that produced it, not at the end of the block.
        Tracked::new("temporary", &log);

        log.borrow_mut().push("end of inner block".to_string());
    }
    log.borrow_mut().push("after inner block".to_string());

    for line in log.borrow().iter() {
        let _ = writeln!(out, "  {line}");
    }
    let _ = writeln!(
        out,
        "\n`first` dies inside consume(), before the next statement runs.\n\
         `temporary` dies one line after it is born. `second` outlives both and\n\
         dies at the closing brace — locals drop in reverse declaration order."
    );
}

// ---------------------------------------------------------------------------
// 3. Copy types do not move
// ---------------------------------------------------------------------------

fn copies(out: &mut String) {
    heading(out, "3. Copy types do not move");

    let n: i32 = 7;
    let m = n; // a copy: `n` is still yours
    let _ = writeln!(out, "let m = n;   n is still readable: {n}, and m is {m}");

    let flag = true;
    let also = flag;
    let _ = writeln!(
        out,
        "let also = flag;   flag is still readable: {flag} / {also}"
    );

    let _ = writeln!(
        out,
        "\nSame syntax, different rule, and the difference is whether the type\n\
         owns anything. `i32` is four bytes and owns nothing, so duplicating it\n\
         is harmless. `String` owns a heap buffer, and two owners would mean two\n\
         frees — which is why the same line moves instead. The `use_after_move`\n\
         case in examples/compile_fail is what the compiler says about it."
    );
}

// ---------------------------------------------------------------------------
// 4. A borrow is a view into a buffer someone else owns
// ---------------------------------------------------------------------------

fn borrows(out: &mut String) {
    heading(out, "4. A borrow is a view into someone else's buffer");

    let sentence = String::from("owned bytes, borrowed views");
    let base = buffer(&sentence);

    let _ = writeln!(out, "  {:<18} {:>6} {:>4}", "&str", "offset", "len");
    for word in sentence.split(' ') {
        let _ = writeln!(
            out,
            "  {:<18} {:>6} {:>4}",
            format!("{word:?}"),
            buffer(word) - base,
            word.len()
        );
    }

    let _ = writeln!(
        out,
        "\nFour `&str` values, one allocation. Each is a pointer into `sentence`\n\
         plus a length; the offsets are where in that one buffer they start.\n\
         Nothing was copied, and none of them may outlive `sentence`."
    );
}

// ---------------------------------------------------------------------------
// 5. The aliasing rule, and the thing it protects
// ---------------------------------------------------------------------------

fn aliasing(out: &mut String) {
    heading(out, "5. The aliasing rule, and what it protects");

    let mut buf = String::from("serde");
    let head: &str = &buf[..5];
    let _ = writeln!(out, "shared borrow while nothing mutates: {head:?}");

    // buf.push_str("_core");    <- rejected while `head` is alive.
    // See the `mutate_while_borrowed` case in examples/compile_fail.

    let _ = writeln!(
        out,
        "last use of the borrow, so it ends here: {}",
        head.len()
    );

    // With no live borrow, the mutable one is allowed.
    buf.push_str("_core");
    let _ = writeln!(out, "after push_str: {buf:?}");

    // An index is a number, not a promise about memory. It survives exactly
    // the mutation a reference could not.
    let range = 0..5;
    let _ = writeln!(out, "re-sliced by the saved range: {:?}", &buf[range]);

    let _ = writeln!(
        out,
        "\n`push_str` may reallocate: it can move every byte to a larger buffer\n\
         and free the old one. Any `&str` taken beforehand would then point at\n\
         freed memory — so the rule is not bookkeeping, it is the reason this\n\
         program cannot read a dangling pointer. The workaround is the last\n\
         line: store indices, re-borrow after the mutation."
    );
}

// ---------------------------------------------------------------------------
// 6. Where lifetimes come in
// ---------------------------------------------------------------------------

/// The returned `&str` borrows from `s`. The lifetime is elided, but it is
/// there: `fn first_word<'a>(s: &'a str) -> &'a str`. Unit 07 starts here.
fn first_word(s: &str) -> &str {
    match s.find(' ') {
        Some(i) => &s[..i],
        None => s,
    }
}

fn toward_lifetimes(out: &mut String) {
    heading(out, "6. Where lifetimes come in");

    let owned = String::from("borrowed from the input");
    let word = first_word(&owned);
    let _ = writeln!(
        out,
        "first_word(&owned) = {word:?}, sharing the input buffer: {}",
        yes(buffer(word) == buffer(&owned))
    );

    let _ = writeln!(
        out,
        "\n`first_word` returns a reference it did not allocate, so the compiler\n\
         has to know what it points into. That is the whole job of a lifetime\n\
         parameter, and it is unit 07.\n\n\
         Unit 08 is this exact function with the input coming from a format:\n\
         `'de` names the buffer the serialized bytes live in, and a\n\
         `Deserialize<'de>` impl that borrows instead of allocating is doing\n\
         what `first_word` does here."
    );
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();
    out.push_str(
        "Ownership and borrowing, observed rather than asserted.\n\
         Addresses are compared, never printed: the comparisons are the facts.\n",
    );

    moves(&mut out);
    drops(&mut out);
    copies(&mut out);
    borrows(&mut out);
    aliasing(&mut out);
    toward_lifetimes(&mut out);

    out
}
