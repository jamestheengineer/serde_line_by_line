// Unit 06 — a shared reference is a view, not a claim.
//
// `&Config` grants permission to read, not to take. Moving `name` out would
// leave the borrowed `Config` holding a `String` that has been given away,
// and the borrow does not own it to begin with.
//
// The fixes are `c.name.clone()` to pay for a copy, or `&c.name` to keep
// borrowing — which is what most callers wanted.

struct Config {
    name: String,
}

fn steal(c: &Config) -> String {
    c.name
}

fn main() {
    let config = Config {
        name: String::from("serde"),
    };
    println!("{}", steal(&config));
}
