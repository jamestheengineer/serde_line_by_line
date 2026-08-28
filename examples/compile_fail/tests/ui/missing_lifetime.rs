// Unit 07 — the one signature elision refuses to complete.
//
// Elision applies three rules or gives up; it never guesses. Here there are
// two input references and no `&self`, so nothing says whether the result
// borrows `a` or `b` — and a caller needs to know, because the answer decides
// how long the result may be held.
//
// The fix is to say it: `fn pick<'a>(a: &'a str, b: &'a str) -> &'a str` if it
// may be either, or `fn pick<'a>(a: &'a str, b: &str) -> &'a str` if only the
// first is ever returned. The second is the stronger signature when it is true.

fn pick(a: &str, b: &str) -> &str {
    if a.len() >= b.len() {
        a
    } else {
        b
    }
}

fn main() {
    println!("{}", pick("alpha", "beta"));
}
