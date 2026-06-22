//! Browser-facing wasm-bindgen wrapper for /tools/bibtex-format/.
//! Compiled with wasm-pack for the standalone /tools/bibtex-format/ page.
use wasm_bindgen::prelude::*;

/// Validate, sort, and pretty-print BibTeX source.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean/integer params arrive as strings and are parsed here:
/// - `sort`: `"none"`/`"key"`/`"type-key"` (blank → none).
/// - `sort_fields` / `align_values`: `"true"`/`"1"`/`"yes"`/`"on"` → on.
/// - `lowercase_type` / `check_duplicates`: same truthy set; the page passes
///   "false" when unchecked and "true" when checked, so a default-checked box → on.
/// - `indent`: a count `0`–16 (blank/unparseable → 2; the core clamps the range).
///
/// Throws a JS error string on a BibTeX parse error, a duplicate cite key, or an
/// invalid sort mode.
#[wasm_bindgen]
pub fn run(
    bibtex: &str,
    sort: &str,
    sort_fields: &str,
    indent: &str,
    align_values: &str,
    lowercase_type: &str,
    check_duplicates: &str,
) -> Result<String, JsValue> {
    let truthy = |v: &str| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        )
    };
    let indent = indent.trim().parse::<u32>().unwrap_or(2);
    gizza_ai_bibtex_format_core::format(
        bibtex,
        sort,
        truthy(sort_fields),
        indent,
        truthy(align_values),
        truthy(lowercase_type),
        truthy(check_duplicates),
    )
    .map_err(|e| JsValue::from_str(&e))
}
