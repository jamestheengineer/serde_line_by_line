//! Example: where a `serde_json::Error` gets its line and column.
//!
//! An error raised by the parser knows where it is. An error raised by a
//! `Deserialize` impl does not, because that code has never seen the input,
//! and the parser adds the position on the way out, if the error passes
//! through it. This runs both kinds, through both readers, then an error
//! that never passes through, and one whose position comes out of its text.

use serde::de::{self, Deserialize, Deserializer};
use serde_json::{error::Category, Error, Value};
use std::fmt::Write as _;
use std::mem::size_of;

/// A type whose `Deserialize` impl always fails, with a message of the
/// caller's choosing. It reads a string and reports it as the error.
#[derive(Debug)]
struct Refuse;

impl<'de> Deserialize<'de> for Refuse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let message = String::deserialize(deserializer)?;
        Err(de::Error::custom(message))
    }
}

pub fn run() -> String {
    let mut out = String::new();

    // The error is a `Box`, so a `Result` carrying one is no wider than it
    // has to be. Compared against a pointer because the absolute size is
    // different in the browser.
    writeln!(
        out,
        "Error is one pointer wide: {}",
        size_of::<Error>() == size_of::<usize>()
    )
    .unwrap();
    writeln!(out).unwrap();

    // Three failures, and the category each is filed under.
    for (label, input) in [
        ("a stray comma", "[1,,2]"),
        ("input that stops", "[1,"),
        ("the wrong type", "[1,\"two\"]"),
    ] {
        let err = serde_json::from_str::<Vec<u8>>(input).unwrap_err();
        report(&mut out, label, input, &err);
    }

    // The same bad input on three lines, through a slice and through a
    // reader. The slice keeps no line counter and searches for newlines when
    // asked. The reader counts as bytes go past. They have to agree.
    let input = "[\n  1,\n  x\n]";
    let from_slice = serde_json::from_str::<Value>(input).unwrap_err();
    let from_reader = serde_json::from_reader::<_, Value>(input.as_bytes()).unwrap_err();
    writeln!(out, "three lines, from a slice    {from_slice}").unwrap();
    writeln!(out, "three lines, from a reader   {from_reader}").unwrap();
    writeln!(out).unwrap();

    // An error from a `Deserialize` impl. `custom` takes a message and
    // nothing else, so the error is born with no position. Inside an array it
    // comes back out through the parser, which stamps its current position on
    // anything that arrives without one.
    let err = serde_json::from_str::<Vec<Refuse>>("[\n  \"not today\"\n]").unwrap_err();
    writeln!(out, "custom(\"not today\"), as an array element").unwrap();
    position(&mut out, &err);

    // The same impl at the top level. Its error is returned straight to
    // `from_str`, and no parser method is on the way to see it.
    let err = serde_json::from_str::<Refuse>("\n\n  \"not today\"").unwrap_err();
    writeln!(out, "custom(\"not today\"), at the top level").unwrap();
    position(&mut out, &err);

    // A message that happens to end like a position, in an input with one
    // line. `make_error` read the 3 and the 4 out of the text, and the parser
    // then left alone a position it found already set.
    let err = serde_json::from_str::<Vec<Refuse>>("[\"lost at line 3 column 4\"]").unwrap_err();
    writeln!(
        out,
        "custom(\"lost at line 3 column 4\"), as an array element"
    )
    .unwrap();
    position(&mut out, &err);

    // A serializer has no input to point into. Line 0 means no position, and
    // the message has no suffix.
    let key_is_a_list = std::collections::BTreeMap::from([(vec![1], 1)]);
    let err = serde_json::to_string(&key_is_a_list).unwrap_err();
    writeln!(out, "a map keyed by Vec<i32>, serialized").unwrap();
    position(&mut out, &err);
    out
}

fn position(out: &mut String, err: &Error) {
    writeln!(out, "  display   {err}").unwrap();
    writeln!(
        out,
        "  position  line {} column {}",
        err.line(),
        err.column()
    )
    .unwrap();
    writeln!(out, "  category  {:?}", err.classify()).unwrap();
    writeln!(out).unwrap();
}

fn report(out: &mut String, label: &str, input: &str, err: &Error) {
    let category = match err.classify() {
        Category::Io => "Io",
        Category::Syntax => "Syntax",
        Category::Data => "Data",
        Category::Eof => "Eof",
    };
    writeln!(out, "{label}: {input}").unwrap();
    writeln!(out, "  display   {err}").unwrap();
    writeln!(out, "  debug     {err:?}").unwrap();
    writeln!(out, "  category  {category}").unwrap();
    writeln!(out).unwrap();
}
