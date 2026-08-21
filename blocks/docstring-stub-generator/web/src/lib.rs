//! Browser-facing wasm-bindgen wrapper for /tools/docstring-stub-generator/.
//! Field order MUST match page/meta.toml: signature, language, style, output,
//! types, placeholder, raises, quote_style, extended_summary, examples,
//! align_tags, indent_size. The page passes every field as a string (the pure
//! runtime does no numeric coercion), so `indent_size` arrives as text and is
//! parsed here.
use gizza_ai_docstring_stub_generator_core::generate;
use wasm_bindgen::prelude::*;

/// Page checkboxes marshal as "true"/"false"; accept the other positive forms too.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    signature: &str,
    language: &str,
    style: &str,
    output: &str,
    types: &str,
    placeholder: &str,
    raises: &str,
    quote_style: &str,
    extended_summary: &str,
    examples: &str,
    align_tags: &str,
    indent_size: &str,
) -> Result<String, JsValue> {
    let indent_size = match indent_size.trim() {
        "" => 4,
        n => n
            .parse::<i64>()
            .map_err(|_| JsValue::from_str("indent_size must be a whole number of spaces (0-8)"))?,
    };
    generate(
        signature,
        language,
        style,
        output,
        types,
        placeholder,
        raises,
        quote_style,
        truthy(extended_summary),
        truthy(examples),
        truthy(align_tags),
        indent_size,
    )
    .map_err(|e| JsValue::from_str(&e))
}
