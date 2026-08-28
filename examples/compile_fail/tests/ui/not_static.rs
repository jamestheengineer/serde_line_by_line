// Unit 07 — `&'static str` demands more than most callers can give.
//
// `remember` asks for a reference to bytes that last for the whole program.
// A `String` allocated at runtime is freed at the end of `main`, so a borrow
// of it is not `'static` and never can be — the note in the diagnostic says
// exactly that: the argument requires `owned` to be borrowed for `'static`.
//
// Reaching for `&'static str` to quiet a lifetime error is the mistake this
// case exists to show. The fix is a lifetime parameter — `fn remember<'a>(s:
// &'a str) -> &'a str` — which accepts a literal *and* a borrow of a String.

fn remember(s: &'static str) -> &'static str {
    s
}

fn main() {
    let owned = String::from("allocated at runtime");
    println!("{}", remember(&owned));
}
