// Unit 07 — a reference to something the function is about to free.
//
// `owned` is dropped at the closing brace, so the returned reference would
// point at freed memory before the caller could read it. No lifetime
// annotation can rescue this: lifetimes describe how long data lives, they do
// not extend it.
//
// The fix is to return the `String` itself and let the caller own it — which
// is the honest signature for a function that allocates.

fn greeting() -> &'static str {
    let owned = String::from("built here, freed here");
    &owned
}

fn main() {
    println!("{}", greeting());
}
