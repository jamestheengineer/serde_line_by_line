//! Example: what `serde_json` does with a number, in and out.
//!
//! JSON has one number type and Rust has a dozen. Each line below is one
//! place where that difference shows: the parser choosing between an integer
//! and a float from the text alone, a visitor refusing a value the parser was
//! happy with, and two things that do not survive a round trip.
//!
//! This is a default-features build. The last section says so again where it
//! matters.

use serde_json::Value;
use std::fmt::Write as _;

pub fn run() -> String {
    let mut out = String::new();

    // The self-describing path: nobody has said what type is wanted, so the
    // parser decides from the digits. `as_u64` and friends ask which of the
    // three shapes it chose.
    writeln!(out, "text -> Value").unwrap();
    for text in [
        "1",
        "-1",
        "1.0",
        "1e2",
        "-0",
        "18446744073709551615",
        "18446744073709551616",
        "-9223372036854775808",
        "-9223372036854775809",
    ] {
        let value: Value = serde_json::from_str(text).expect("each of these is a JSON number");
        writeln!(out, "  {text:<22} {:<8} prints as {value}", shape(&value)).unwrap();
    }
    writeln!(out).unwrap();

    // The typed path: the caller named a width. The parser still only knows
    // u64, i64 and f64, and the range check is the visitor's.
    writeln!(out, "text -> u8").unwrap();
    for text in ["255", "256", "-1", "1.0"] {
        match serde_json::from_str::<u8>(text) {
            Ok(n) => writeln!(out, "  {text:<5} ok     {n}").unwrap(),
            Err(e) => writeln!(out, "  {text:<5} error  {e}").unwrap(),
        }
    }
    writeln!(out).unwrap();

    // An integer written into a float type is accepted. The other direction
    // was refused just above.
    writeln!(out, "text -> f64").unwrap();
    for text in ["1", "0.1", "1e400"] {
        match serde_json::from_str::<f64>(text) {
            Ok(x) => writeln!(out, "  {text:<6} ok     {x:?}").unwrap(),
            Err(e) => writeln!(out, "  {text:<6} error  {e}").unwrap(),
        }
    }
    writeln!(out).unwrap();

    // Out: the shortest text that reads back as the same float, and `null`
    // for the two things JSON cannot spell.
    writeln!(out, "f64 -> text").unwrap();
    for x in [0.1, 1.0, 1e21, f64::MAX, -0.0, f64::NAN, f64::INFINITY] {
        let text = serde_json::to_string(&x).expect("a float always serializes");
        writeln!(out, "  {x:<24?} {text}").unwrap();
    }
    // The same `f32`, written as itself and written after widening. The
    // shortest text that identifies a float depends on how many floats there
    // are to tell it apart from.
    let narrow = serde_json::to_string(&0.1f32).unwrap();
    let wide = serde_json::to_string(&f64::from(0.1f32)).unwrap();
    writeln!(out, "  {:<24} {narrow}", "0.1f32").unwrap();
    writeln!(out, "  {:<24} {wide}", "0.1f32 as f64").unwrap();
    writeln!(out).unwrap();

    // The first thing that does not round trip. Serializing NaN succeeds, and
    // what it wrote is not a number.
    let nan = serde_json::to_string(&f64::NAN).unwrap();
    match serde_json::from_str::<f64>(&nan) {
        Ok(x) => writeln!(out, "NaN -> {nan} -> ok {x:?}").unwrap(),
        Err(e) => writeln!(out, "NaN -> {nan} -> error: {e}").unwrap(),
    }
    match serde_json::from_str::<Option<f64>>(&nan) {
        Ok(x) => writeln!(out, "NaN -> {nan} -> as Option<f64>: {x:?}").unwrap(),
        Err(e) => writeln!(out, "NaN -> {nan} -> as Option<f64>: error: {e}").unwrap(),
    }
    writeln!(out).unwrap();

    // The second. Text somebody else wrote may not land on the nearest
    // float. This is the largest subnormal double, written with seventeen
    // digits, and the default build is one unit in the last place high. The
    // standard library's parser is the reference: it is correctly rounded.
    let text = "2.2250738585072011e-308";
    let parsed: f64 = serde_json::from_str(text).unwrap();
    let nearest: f64 = text.parse().unwrap();
    writeln!(out, "text         {text}").unwrap();
    writeln!(out, "serde_json   {:#018x}  {parsed:?}", parsed.to_bits()).unwrap();
    writeln!(out, "str::parse   {:#018x}  {nearest:?}", nearest.to_bits()).unwrap();
    writeln!(
        out,
        "same float   {}",
        parsed.to_bits() == nearest.to_bits()
    )
    .unwrap();
    writeln!(out).unwrap();

    // What the serializer wrote itself is a different matter. These five
    // come back to the bits they started with.
    for x in [0.1, 1.0 / 3.0, f64::MAX, f64::MIN_POSITIVE, 5e-324] {
        let text = serde_json::to_string(&x).unwrap();
        let back: f64 = serde_json::from_str(&text).unwrap();
        writeln!(
            out,
            "{text:<24} back to the same bits: {}",
            back.to_bits() == x.to_bits()
        )
        .unwrap();
    }
    out
}

/// Which of `Number`'s three shapes the parser chose.
fn shape(value: &Value) -> &'static str {
    match value {
        Value::Number(n) if n.is_u64() => "PosInt",
        Value::Number(n) if n.is_i64() => "NegInt",
        Value::Number(_) => "Float",
        _ => "not a number",
    }
}
