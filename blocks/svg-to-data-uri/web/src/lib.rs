//! Browser-facing wasm-bindgen wrapper for /tools/svg-to-data-uri/.
//! Field order MUST match page/meta.toml: svg, encoding, output, quotes,
//! minify, add_xmlns. Every value arrives as a string; checkboxes send
//! "true"/"false" and blank selects fall back to the core's defaults.
use wasm_bindgen::prelude::*;

/// Checkbox values arrive as strings — accept every positive spelling.
/// A blank value means the field was absent, so honour the descriptor default.
fn truthy(s: &str, default: bool) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return default;
    }
    matches!(t.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    svg: &str,
    encoding: &str,
    output: &str,
    quotes: &str,
    minify: &str,
    add_xmlns: &str,
) -> Result<String, JsValue> {
    gizza_ai_svg_to_data_uri_core::run(
        svg,
        encoding,
        output,
        quotes,
        truthy(minify, true),
        truthy(add_xmlns, true),
    )
    .map(|r| r.output)
    .map_err(|e| JsValue::from_str(&e))
}
