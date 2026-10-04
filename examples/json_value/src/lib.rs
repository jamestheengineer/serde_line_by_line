//! Example: `Value`, the format that is a data structure.
//!
//! `serde_json` implements `Serializer` and `Deserializer` twice each: once
//! over bytes and once over a tree in memory. This runs one struct through
//! both, then shows what the tree's own conveniences do: the `json!` macro,
//! and bracket indexing that reads carelessly and writes by creating.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Write as _;

#[derive(Debug, Serialize, Deserialize)]
struct Point<'a> {
    name: &'a str,
    at: (i32, i32),
}

pub fn run() -> String {
    let mut out = String::new();
    let point = Point {
        name: "origin",
        at: (0, 0),
    };

    // One `Serialize` impl, two serializers. The first has `Ok = ()` and
    // writes bytes somewhere else. The second has `Ok = Value`, and what it
    // returns is the output.
    let text = serde_json::to_string(&point).unwrap();
    let tree = serde_json::to_value(&point).unwrap();
    writeln!(out, "to_string  {text}").unwrap();
    writeln!(out, "to_value   {tree:?}").unwrap();
    writeln!(out).unwrap();

    // One `Deserialize` impl, two deserializers. `&tree` is the borrowed
    // impl, which can lend its strings, so `name` points into the tree.
    let back = Point::deserialize(&tree).unwrap();
    let inside = match &tree["name"] {
        Value::String(s) => std::ptr::eq(s.as_str(), back.name),
        _ => false,
    };
    writeln!(out, "from &Value  {back:?}").unwrap();
    writeln!(out, "name points into the tree: {inside}").unwrap();
    // By value the tree is consumed and can only hand over owned strings.
    match serde_json::from_value::<(String, (i32, i32))>(json!(["origin", [0, 0]])) {
        Ok(pair) => writeln!(out, "from Value   {pair:?}").unwrap(),
        Err(e) => writeln!(out, "from Value   error: {e}").unwrap(),
    }
    writeln!(out).unwrap();

    // `json!`: a literal with Rust expressions in it. `null` is matched as a
    // token, `point` goes through `to_value`, the key `(key)` through
    // `Into<String>`, and the trailing comma is allowed.
    let key = "computed";
    let made = json!({
        "z": null,
        "a": [1, 2.5, "three", [true]],
        (key): point,
    });
    writeln!(out, "json!      {made}").unwrap();
    // The keys were written z, a, computed.
    let order: Vec<&str> = made
        .as_object()
        .map(|m| m.keys().map(String::as_str).collect())
        .unwrap_or_default();
    writeln!(out, "key order  {order:?}").unwrap();
    writeln!(out).unwrap();

    // Reading through brackets never panics. A missing key, an index past
    // the end and a string index into an array are all `Null`.
    writeln!(out, "made[\"a\"][2]       {}", made["a"][2]).unwrap();
    writeln!(out, "made[\"missing\"]    {}", made["missing"]).unwrap();
    writeln!(out, "made[\"a\"][99]      {}", made["a"][99]).unwrap();
    writeln!(out, "made[\"a\"][\"x\"][0]  {}", made["a"]["x"][0]).unwrap();
    writeln!(out).unwrap();

    // Writing through brackets creates what is missing, starting from `Null`.
    let mut data = Value::Null;
    data["a"]["b"]["c"] = json!(1);
    writeln!(out, "after data[\"a\"][\"b\"][\"c\"] = 1   {data}").unwrap();

    // A path as a string, and `take`, which leaves `null` behind.
    let taken = data.pointer_mut("/a/b").map(Value::take);
    writeln!(out, "pointer_mut(\"/a/b\") taken       {taken:?}").unwrap();
    writeln!(out, "what is left                    {data}").unwrap();
    out
}
