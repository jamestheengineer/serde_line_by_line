//! Example: the document the json walk follows, from bytes to `Value` and back.
//!
//! `{"a":[1,true],"b":"x"}` is twenty-two bytes chosen to be unremarkable and
//! to still contain every shape the parser has a separate path for: an object,
//! an array, an integer, a boolean and a string. Everything the walk says
//! about what `serde_json` produces is quoted out of this output.
//!
//! Nothing here prints. The example returns a `String` so that the same code
//! runs under `cargo test` and in the browser, which is what makes an
//! explanation checkable against real output rather than against a transcript
//! somebody typed (PLAN.md §6).

use serde_json::Value;
use std::fmt::Write as _;

/// The one document this walk follows.
pub const INPUT: &str = r#"{"a":[1,true],"b":"x"}"#;

pub fn run() -> String {
    let mut out = String::new();
    let value: Value = serde_json::from_str(INPUT).expect("the input is valid JSON");

    writeln!(out, "input   {INPUT}").unwrap();
    writeln!(out).unwrap();

    // What the parser built, shape by shape. `Value` is an enum and the
    // variant is the parser's answer to "what did those bytes mean".
    writeln!(out, "value").unwrap();
    describe(&mut out, &value, 0, "");
    writeln!(out).unwrap();

    // Back out the other side. Serializing a `Value` is the round trip the
    // walk ends on: the same bytes, because nothing in this document is
    // ambiguous.
    let back = serde_json::to_string(&value).expect("a Value always serializes");
    writeln!(out, "to_string    {back}").unwrap();
    writeln!(out, "round trips  {}", back == INPUT).unwrap();
    writeln!(out).unwrap();

    // Pretty printing is the same serializer with a different formatter, which
    // is the distinction the walk draws when it reaches `ser.rs`.
    writeln!(out, "to_string_pretty").unwrap();
    for line in serde_json::to_string_pretty(&value)
        .expect("a Value always serializes")
        .lines()
    {
        writeln!(out, "  {line}").unwrap();
    }
    writeln!(out).unwrap();

    // An error carries a position, which is the thing `error.rs` exists for
    // and the part a reader notices first.
    let bad = r#"{"a":[1,true,],"b":"x"}"#;
    match serde_json::from_str::<Value>(bad) {
        Ok(_) => writeln!(out, "error    none").unwrap(),
        Err(e) => {
            writeln!(out, "input    {bad}").unwrap();
            writeln!(out, "error    {e}").unwrap();
            writeln!(out, "at       line {} column {}", e.line(), e.column()).unwrap();
        }
    }
    out
}

/// One line per node, indented by depth: the tree the parser built.
fn describe(out: &mut String, value: &Value, depth: usize, key: &str) {
    let pad = "  ".repeat(depth + 1);
    let label = if key.is_empty() {
        String::new()
    } else {
        format!("{key}: ")
    };
    match value {
        Value::Null => writeln!(out, "{pad}{label}Null").unwrap(),
        Value::Bool(b) => writeln!(out, "{pad}{label}Bool({b})").unwrap(),
        Value::Number(n) => writeln!(out, "{pad}{label}Number({n})").unwrap(),
        Value::String(s) => writeln!(out, "{pad}{label}String({s:?})").unwrap(),
        Value::Array(items) => {
            writeln!(out, "{pad}{label}Array, {} items", items.len()).unwrap();
            for item in items {
                describe(out, item, depth + 1, "");
            }
        }
        Value::Object(map) => {
            writeln!(out, "{pad}{label}Object, {} keys", map.len()).unwrap();
            for (k, v) in map {
                describe(out, v, depth + 1, k);
            }
        }
    }
}
