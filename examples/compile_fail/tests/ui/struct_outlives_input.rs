// Unit 07 — a borrowing struct cannot outlive what it borrows.
//
// `Parser<'a>` is a view, not an owner: the `'a` is the compiler's record that
// values of this type describe someone else's memory. `text` dies at the inner
// closing brace, and `parser` is read after it, so `parser.input` would be a
// dangling pointer.
//
// The fix is to give the input a lifetime at least as long as the parser's —
// move the `let text` out of the inner block — or to make the struct own a
// `String` and pay for the copy.

struct Parser<'a> {
    input: &'a str,
}

fn main() {
    let parser;
    {
        let text = String::from("alpha;beta;gamma");
        parser = Parser { input: &text };
    }
    println!("{}", parser.input);
}
