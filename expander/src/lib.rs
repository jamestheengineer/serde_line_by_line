//! The expansion playground: `serde_derive` running in the browser.
//!
//! A second wasm module rather than a member of the shared playground (D1).
//! This one carries `serde_derive`, `syn` and `prettyplease` and is several
//! times the playground's size, so every page holding a micro-example would
//! pay for an expander it never calls. It is fetched only where an expansion
//! is actually shown.

use wasm_bindgen::prelude::*;

/// Expands `#[derive(Serialize)]` or `#[derive(Deserialize)]` over `input`.
#[wasm_bindgen]
pub fn expand(input: &str, derive: &str) -> Result<String, JsError> {
    let which = expand::Derive::parse(derive)
        .ok_or_else(|| JsError::new(&format!("no derive named {derive:?}")))?;
    Ok(expand::expand(input, which))
}

/// The `serde_derive` release this module was built from, so the page can say
/// so rather than the reader having to trust it.
#[wasm_bindgen]
pub fn derive_version() -> String {
    expand::DERIVE_VERSION.to_string()
}
