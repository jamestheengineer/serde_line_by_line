// Unit 06 — why the aliasing rule is not bookkeeping.
//
// `push_str` may reallocate: it can copy every byte into a larger buffer and
// free the old one. `head` points into the old buffer, so if this compiled,
// the `println!` would read freed memory.
//
// This is the case `move_and_borrow` section 5 demonstrates the legal version
// of: save indices, re-borrow after the mutation.

fn main() {
    let mut buf = String::from("serde");
    let head: &str = &buf[..5];
    buf.push_str("_core");
    println!("{head}");
}
