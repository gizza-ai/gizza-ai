//! Browser-facing wasm-bindgen wrapper for /tools/hcl-to-json/.
//! Field order MUST match page/meta.toml: hcl, blocks, expressions, indent,
//! pretty, sort_keys. The page hands every field over as a string, so the
//! checkboxes are parsed here.
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as "true"/"false" from the form runtime; accept the other
/// positive spellings a deep-link might carry.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Convert HCL text to JSON.
///
/// Blank option strings fall back to the core's documented defaults, so an
/// untouched `<select>` behaves like the schema default. Returns an empty
/// string for empty input so a freshly loaded page shows nothing rather than an
/// error; every other problem (syntax error, conflicting names, oversized
/// input) throws a JS error string naming what was expected.
#[wasm_bindgen]
pub fn run(
    hcl: &str,
    blocks: &str,
    expressions: &str,
    indent: &str,
    pretty: &str,
    sort_keys: &str,
) -> Result<String, JsValue> {
    if hcl.trim().is_empty() {
        return Ok(String::new());
    }
    gizza_ai_hcl_to_json_core::convert(
        hcl,
        blocks,
        expressions,
        truthy(sort_keys),
        truthy(pretty),
        indent,
    )
    .map_err(|e| JsValue::from_str(&e))
}
