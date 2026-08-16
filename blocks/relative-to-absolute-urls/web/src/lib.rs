//! Browser-facing wasm-bindgen wrapper for /tools/relative-to-absolute-urls/.
//! Argument order MUST match page/meta.toml: html, base, attributes,
//! use_base_tag, protocol_relative, resolve_fragments, style_urls, output.
//! Every field arrives as a string (checkboxes send "true"/"false"); the core
//! owns all validation and error messages.
use wasm_bindgen::prelude::*;

/// `"true"`/`"1"`/`"on"`/`"yes"` (case-insensitive) → `true`; anything else
/// (including blank) → `false`. Checkboxes on the page send `"true"`/`"false"`.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// Rewrite the relative URLs in `html` to absolute ones against `base`.
///
/// - `html`: the markup to rewrite (max 5,000,000 bytes).
/// - `base`: the absolute URL the relative values are relative to.
/// - `attributes`: `href-src` | `common` | `all`.
/// - `use_base_tag`: checkbox `"true"`/`"false"` (default-checked) — honour a
///   `<base href>` found in the document.
/// - `protocol_relative`: `resolve` | `keep`.
/// - `resolve_fragments`: checkbox `"true"`/`"false"` (default-unchecked) —
///   also make bare `#anchor` links absolute.
/// - `style_urls`: checkbox `"true"`/`"false"` (default-unchecked) — also
///   rewrite `url(…)` / `@import` in CSS.
/// - `output`: `html` | `report` | `urls`.
///
/// Throws a JS error string on empty input, a missing/relative/non-hierarchical
/// base URL, an unknown option value, or an over-cap document.
#[wasm_bindgen]
pub fn run(
    html: &str,
    base: &str,
    attributes: &str,
    use_base_tag: &str,
    protocol_relative: &str,
    resolve_fragments: &str,
    style_urls: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_relative_to_absolute_urls_core::absolutize(
        html,
        base,
        attributes,
        truthy(use_base_tag),
        protocol_relative,
        truthy(resolve_fragments),
        truthy(style_urls),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
