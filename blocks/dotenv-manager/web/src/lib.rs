//! Browser-facing wasm-bindgen wrapper for /tools/dotenv-manager/.
//! The standalone page passes every field value as a string; booleans arrive as
//! "true"/"false" from the checkboxes and are parsed here.
use wasm_bindgen::prelude::*;

/// Parse, validate, merge and secret-mask a `.env` file.
///
/// - `env`: the primary `.env` contents (required).
/// - `merge`: an optional overlay `.env` (its keys override).
/// - `required_keys`: comma-separated keys that must be present.
/// - `mask_secrets`: `"false"`/`"0"`/`"no"`/`"off"` turns masking OFF; blank or
///   anything else keeps the default ON (the checkbox defaults to checked).
/// - `sort_keys`: `"true"`/`"1"`/`"yes"`/`"on"` sorts keys alphabetically.
/// - `output`: `report` | `normalized` | `example` | `json` (blank → report).
///
/// Throws a JS error string on an invalid `output` mode.
#[wasm_bindgen]
pub fn run(
    env: &str,
    merge: &str,
    required_keys: &str,
    mask_secrets: &str,
    sort_keys: &str,
    output: &str,
) -> Result<String, JsValue> {
    // Default-true checkbox: only an explicit off-value disables masking.
    let mask_secrets = !matches!(
        mask_secrets.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    );
    let sort_keys = matches!(
        sort_keys.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    );
    gizza_ai_dotenv_manager_core::manage(env, merge, required_keys, mask_secrets, sort_keys, output)
        .map_err(|e| JsValue::from_str(&e))
}
