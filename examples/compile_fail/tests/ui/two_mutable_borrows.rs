// Unit 06 — the aliasing rule, in its simplest form.
//
// Two `&mut` to the same value at the same time would let two pieces of code
// each believe they have exclusive access. Rust allows one, and the error
// names both the first borrow and the use that keeps it alive.

fn main() {
    let mut buf = String::from("serde");
    let first = &mut buf;
    let second = &mut buf;
    first.push('!');
    second.push('?');
}
