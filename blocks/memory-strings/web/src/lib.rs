//! Browser-facing wasm-bindgen wrapper for /tools/memory-strings/.
//! Field order MUST match page/meta.toml: dump, input_format, encoding,
//! min_length, categories, defang. Every field arrives as a string (the enum
//! selects send their canonical value, the checkbox sends "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Run a `strings`-style extraction + categorization over `dump`.
///
/// The standalone tool page passes every field value as a string; `min_length`
/// is parsed here and an empty value falls back to the schema default (4).
#[wasm_bindgen]
pub fn run(
    dump: &str,
    input_format: &str,
    encoding: &str,
    min_length: &str,
    categories: &str,
    defang: &str,
) -> Result<String, JsValue> {
    let t = min_length.trim();
    let min = if t.is_empty() {
        4usize
    } else {
        t.parse::<usize>()
            .map_err(|_| JsValue::from_str("min_length must be a whole number >= 1"))?
    };
    gizza_ai_memory_strings_core::run(
        dump,
        input_format,
        encoding,
        min.max(1),
        categories,
        truthy(defang),
    )
    .map_err(|e| JsValue::from_str(&e))
}
