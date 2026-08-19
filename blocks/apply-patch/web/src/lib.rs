//! Browser-facing wasm-bindgen wrapper for /tools/apply-patch/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Apply a pasted unified diff to a pasted source file.
///
/// The standalone tool page passes every field value as a string (and in the
/// `page/meta.toml` `[[input]]` order, which mirrors the descriptor's param
/// order), so the booleans and the number arrive as strings and are parsed here:
/// - `source`:            the original file text (1 MB cap).
/// - `patch`:             the unified diff (1 MB cap).
/// - `output`:            `patched` (blank) | `report` | `json` | `rejects`.
/// - `reverse`:           default-off checkbox — `"true"`/`"1"`/`"yes"`/`"on"` unapplies.
/// - `fuzz`:              `0`–`3` context lines droppable per hunk end; blank means 2.
/// - `ignore_whitespace`: default-off checkbox — collapses whitespace when matching.
/// - `on_conflict`:       `fail` (blank) | `skip`.
/// - `file`:              which path's hunks to use in a multi-file patch. Blank = the only one.
///
/// Throws a JS error string on an empty/unparseable patch, a conflict under
/// `on_conflict=fail`, an ambiguous multi-file patch, a bad `fuzz`, or input over
/// the cap.
#[wasm_bindgen]
pub fn run(
    source: &str,
    patch: &str,
    output: &str,
    reverse: &str,
    fuzz: &str,
    ignore_whitespace: &str,
    on_conflict: &str,
    file: &str,
) -> Result<String, JsValue> {
    let reverse = truthy(reverse);
    let ignore_whitespace = truthy(ignore_whitespace);
    let fuzz = match fuzz.trim() {
        "" => 2u32,
        f => f
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= 3.0)
            .map(|v| v as u32)
            .ok_or_else(|| JsValue::from_str(&format!("fuzz must be 0-3, got {f}")))?,
    };
    gizza_ai_apply_patch_core::apply_patch(
        source,
        patch,
        output,
        reverse,
        fuzz,
        ignore_whitespace,
        on_conflict,
        file,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Default-off checkbox: only an explicit truthy string turns the flag on.
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
