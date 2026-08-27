// Unit 06 — the first ownership error everyone meets.
//
// `String` owns a heap buffer, so `let moved = original;` transfers that
// ownership. Reading `original` afterwards would give one allocation two
// owners, and later two frees.
//
// The fix is to decide what you meant: borrow (`&original`) if you only want
// to look, or clone if you genuinely want a second buffer.

fn main() {
    let original = String::from("owns its bytes");
    let moved = original;
    println!("{original} then {moved}");
}
