//! Browser-facing wasm-bindgen wrapper for /tools/list-dedupe-merge/.
//! Compiled with wasm-pack for the standalone /tools/list-dedupe-merge/ page.
use wasm_bindgen::prelude::*;

/// Positive-truthy parse of a page checkbox value (`"true"`/`"1"`/`"yes"`/`"on"`).
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Merge two lists into one de-duplicated list. The standalone tool page passes
/// every field value as a string; the boolean checkboxes arrive as
/// `"true"`/`"false"` and are parsed here. `separator`, `merge_order`, and `sort`
/// are the enum values; blank falls back to the core defaults
/// (`newline` / `append` / `input`).
///
/// Throws a JS error string on an invalid `separator`/`merge_order`/`sort`.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    list_a: &str,
    list_b: &str,
    separator: &str,
    merge_order: &str,
    trim: &str,
    ignore_blank: &str,
    ignore_case: &str,
    sort: &str,
    ignore_leading_zeros: &str,
) -> Result<String, JsValue> {
    gizza_ai_list_dedupe_merge_core::merge(
        list_a,
        list_b,
        separator,
        merge_order,
        truthy(trim),
        truthy(ignore_blank),
        truthy(ignore_case),
        sort,
        truthy(ignore_leading_zeros),
    )
    .map_err(|e| JsValue::from_str(&e))
}
