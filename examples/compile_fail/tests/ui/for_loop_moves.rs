// Unit 14 — `for` takes ownership, because `IntoIterator` does.
//
// `for name in names` is not syntax over a collection; it is
// `IntoIterator::into_iter(names)` followed by a loop over `next()`. The impl
// chosen for `Vec<String>` is the one that takes `self` by value and yields
// `String`, so the loop consumes the vector and `names` is gone afterwards.
//
// This is the single most common iterator error, and the fix is to say which
// impl you wanted: `&names` (or `names.iter()`) yields `&String` and leaves
// the vector alone. The compiler's note names the impl it picked, which is
// the part worth reading.

fn main() {
    let names = vec![String::from("alpha"), String::from("beta")];

    for name in names {
        println!("{name}");
    }

    println!("{} names", names.len());
}
