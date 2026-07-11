//! Browser-facing wasm-bindgen wrapper for /tools/list-set-diff/.
//! Compiled with wasm-pack for the standalone /tools/list-set-diff/ page.
use wasm_bindgen::prelude::*;

/// Positive-truthy parse of a page checkbox value (`"true"`/`"1"`/`"yes"`/`"on"`).
fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on")
}

/// Compare two lists as sets. The standalone tool page passes every field value
/// as a string; the boolean checkboxes arrive as `"true"`/`"false"` and are
/// parsed here. `separator` and `sort` are the enum values; blank falls back to
/// the core defaults (`newline` / `input`).
///
/// Throws a JS error string on an invalid `separator`/`sort`.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    list_a: &str,
    list_b: &str,
    separator: &str,
    ignore_case: &str,
    trim: &str,
    ignore_blank: &str,
    dedupe: &str,
    ignore_leading_zeros: &str,
    sort: &str,
) -> Result<String, JsValue> {
    gizza_ai_list_set_diff_core::diff(
        list_a,
        list_b,
        separator,
        truthy(ignore_case),
        truthy(trim),
        truthy(ignore_blank),
        truthy(dedupe),
        truthy(ignore_leading_zeros),
        sort,
    )
    .map_err(|e| JsValue::from_str(&e))
}
