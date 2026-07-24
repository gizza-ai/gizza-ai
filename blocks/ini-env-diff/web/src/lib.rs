//! Browser-facing wasm-bindgen wrapper for /tools/ini-env-diff/.
//! The standalone page passes every field value as a string; the two boolean
//! checkboxes arrive as "true"/"false" and are parsed here. Field order MUST
//! match page/meta.toml: left, right, format, ignore_case, mask_secrets, output.
use wasm_bindgen::prelude::*;

/// Diff two `.env`/`.ini` config files key-by-key.
///
/// - `left`: the first (old/base) config file contents (required).
/// - `right`: the second (new/compared) config file contents (required).
/// - `format`: `auto` (default) | `env` | `ini` (blank → auto).
/// - `ignore_case`: `"true"`/`"1"`/`"yes"`/`"on"` compares keys case-insensitively
///   (default-false checkbox — anything else stays case-sensitive).
/// - `mask_secrets`: `"true"`/`"1"`/`"yes"`/`"on"` masks sensitive-looking values
///   (default-false checkbox).
/// - `output`: `report` (default) | `json` (blank → report).
///
/// Throws a JS error string on an invalid `format` or `output` mode.
#[wasm_bindgen]
pub fn run(
    left: &str,
    right: &str,
    format: &str,
    ignore_case: &str,
    mask_secrets: &str,
    output: &str,
) -> Result<String, JsValue> {
    let ignore_case = matches!(
        ignore_case.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    let mask_secrets = matches!(
        mask_secrets.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_ini_env_diff_core::diff(left, right, format, ignore_case, mask_secrets, output)
        .map_err(|e| JsValue::from_str(&e))
}
