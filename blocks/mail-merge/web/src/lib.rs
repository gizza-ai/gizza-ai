//! Browser-facing wasm-bindgen wrapper for /tools/mail-merge/.
//! Compiled with wasm-pack for the standalone /tools/mail-merge/ page.
use wasm_bindgen::prelude::*;

/// Fill `template` once per row of `csv` and return the joined output.
///
/// The standalone page passes every field as a string:
/// - `syntax`/`delimiter`/`on_missing`/`separator`: enum values (blank → each
///   param's default, handled by the core).
/// - `case_insensitive`: `"true"`/`"1"`/`"yes"`/`"on"` → case-insensitive header
///   matching; anything else → exact matching. (The checkbox defaults to checked,
///   so an untouched field arrives as `"true"`.)
///
/// Throws a JS error string on an invalid option, empty CSV, too many rows, or
/// (with `on_missing = "error"`) an unknown column.
#[wasm_bindgen]
pub fn run(
    template: &str,
    csv: &str,
    syntax: &str,
    delimiter: &str,
    on_missing: &str,
    case_insensitive: &str,
    separator: &str,
) -> Result<String, JsValue> {
    let case_insensitive = matches!(
        case_insensitive.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_mail_merge_core::merge(
        template,
        csv,
        syntax,
        delimiter,
        on_missing,
        case_insensitive,
        separator,
    )
    .map_err(|e| JsValue::from_str(&e))
}
