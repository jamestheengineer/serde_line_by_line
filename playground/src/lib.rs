//! The example playground (decision D1).
//!
//! One WASM module holding every micro-example, dispatched by name. Examples
//! return `String` rather than printing, which is what lets identical code run
//! here and under `cargo test`.

use wasm_bindgen::prelude::*;

include!(concat!(env!("OUT_DIR"), "/dispatch.rs"));

/// Runs one example and returns its output.
#[wasm_bindgen]
pub fn run(name: &str) -> Result<String, JsError> {
    dispatch(name).ok_or_else(|| JsError::new(&format!("no example named {name:?}")))
}

/// Newline-separated list of available examples, so the page can report what
/// this build actually contains.
#[wasm_bindgen]
pub fn examples() -> String {
    EXAMPLES.join("\n")
}
