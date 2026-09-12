//! Prints an expansion with line numbers, for writing `emits` claims against.
//!
//! `cargo run -p expand --example dump -- <case> [serialize|deserialize]`
fn main() {
    let mut args = std::env::args().skip(1);
    let case = args
        .next()
        .expect("usage: dump <case> [serialize|deserialize]");
    let which = args.next().unwrap_or_else(|| "serialize".to_string());
    let derive = match which.as_str() {
        "deserialize" => expand::Derive::Deserialize,
        _ => expand::Derive::Serialize,
    };
    let src = expand::case(&case).expect("no such case");
    for (i, line) in expand::expand(src, derive).lines().enumerate() {
        println!("{:4} {line}", i + 1);
    }
}
