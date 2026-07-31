//! Browser-facing wasm-bindgen wrapper for /tools/network-config-parser/.
//! Compiled with wasm-pack for the standalone /tools/network-config-parser/ page.
use wasm_bindgen::prelude::*;

/// Parse a network device `config` into a tree / paths / report.
///
/// The tool page passes every field value as a string, so all arguments arrive
/// as strings and are forwarded to the pure core. `syntax` is one of `auto`
/// (blank → this), `indent`, `brace`; `output` is `tree` (blank → this),
/// `paths`, `report`; `comments` is `strip` (blank → this) or `keep`; `filter`
/// is an optional case-insensitive substring (blank → no filtering).
///
/// Throws a JS error string on an empty config, unbalanced braces, or an
/// unknown option value.
#[wasm_bindgen]
pub fn run(
    config: &str,
    syntax: &str,
    output: &str,
    filter: &str,
    comments: &str,
) -> Result<String, JsValue> {
    gizza_ai_network_config_parser_core::parse(config, syntax, output, filter, comments)
        .map_err(|e| JsValue::from_str(&e))
}
