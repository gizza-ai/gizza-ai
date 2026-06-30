//! Browser-facing wasm-bindgen wrapper for /tools/latex-to-mathml/.
//! The standalone tool page passes every field value as a string, so the
//! boolean `pretty` arrives as a string and is parsed here.
use wasm_bindgen::prelude::*;

/// Convert `latex` to a MathML `<math>` element.
/// - `display`: `"block"`/`"inline"` (blank → block).
/// - `pretty`: `"true"`/`"1"`/`"yes"`/`"on"` → indent the output; else compact.
///
/// Throws a JS error string on empty input, an invalid `display`, or unparseable
/// LaTeX.
#[wasm_bindgen]
pub fn run(latex: &str, display: &str, pretty: &str) -> Result<String, JsValue> {
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_latex_to_mathml_core::run(latex, display, pretty).map_err(|e| JsValue::from_str(&e))
}
