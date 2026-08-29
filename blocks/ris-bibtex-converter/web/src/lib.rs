//! Browser-facing wasm-bindgen wrapper for /tools/ris-bibtex-converter/.
//! The page driver hands every field over as a string (checkboxes arrive as
//! "true"/"false", number boxes as digits), so booleans are parsed
//! positive-truthy here and the rest is delegated verbatim to the shared core.
//! Argument order must match the `[[input]]` order in `page/meta.toml`.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    input: &str,
    direction: &str,
    key_style: &str,
    include_abstract: &str,
    include_keywords: &str,
    translate_latex: &str,
    indent: &str,
    sort: &str,
) -> Result<String, JsValue> {
    gizza_ai_ris_bibtex_converter_core::convert_str(
        input,
        direction,
        key_style,
        truthy(include_abstract),
        truthy(include_keywords),
        truthy(translate_latex),
        indent,
        sort,
    )
    .map_err(|e| JsValue::from_str(&e))
}
