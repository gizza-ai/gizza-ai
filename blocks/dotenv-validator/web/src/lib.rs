//! Browser-facing wasm-bindgen wrapper for /tools/dotenv-validator/.
//! Compiled with wasm-pack for the standalone /tools/dotenv-validator/ page.
use wasm_bindgen::prelude::*;

/// Lint the `.env` text in `env`.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean params arrive as strings and are parsed here:
/// - `allow_undefined`: comma-separated externally-provided variable names.
/// - `check_interpolation`: `"true"`/`"1"`/`"yes"`/`"on"` → on (blank → on,
///   the descriptor default); anything else → off.
/// - `require_quotes_for_spaces`: same truthy parsing (blank → on).
/// - `output`: `"report"`/`"json"` (blank → report).
///
/// Throws a JS error string only on an invalid `output` value.
#[wasm_bindgen]
pub fn run(
    env: &str,
    allow_undefined: &str,
    check_interpolation: &str,
    require_quotes_for_spaces: &str,
    output: &str,
) -> Result<String, JsValue> {
    let check_interpolation = truthy(check_interpolation);
    let require_quotes_for_spaces = truthy(require_quotes_for_spaces);
    gizza_ai_dotenv_validator_core::validate(
        env,
        allow_undefined,
        check_interpolation,
        require_quotes_for_spaces,
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Positive-truthy parse: a blank field defaults to ON (both boolean params
/// default to `true` in the descriptor, so a checked box → `"true"`, an
/// unchecked box → `"false"`, and a URL-prefill omitting the field → blank).
fn truthy(v: &str) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" | "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}
