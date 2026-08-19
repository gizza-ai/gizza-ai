//! Browser-facing wasm-bindgen wrapper for /tools/env-schema-validate/.
//! Compiled with wasm-pack for the standalone /tools/env-schema-validate/ page.
use wasm_bindgen::prelude::*;

/// Validate the `.env` text in `env` against `schema`.
///
/// The standalone tool page passes every field value as a string, so the
/// boolean param arrives as a string and is parsed here:
/// - `schema_format`: `auto` (blank → auto) | `rules` | `json-schema` | `example`.
/// - `unknown_keys`: `warn` (blank → warn) | `ignore` | `error`.
/// - `empty_is_missing`: `"true"`/`"1"`/`"yes"`/`"on"` → on (blank → on, the
///   descriptor default); anything else → off.
/// - `output`: `"report"`/`"json"` (blank → report).
///
/// Throws a JS error string on an empty/unparseable schema or an invalid
/// enum value.
#[wasm_bindgen]
pub fn run(
    env: &str,
    schema: &str,
    schema_format: &str,
    unknown_keys: &str,
    empty_is_missing: &str,
    output: &str,
) -> Result<String, JsValue> {
    gizza_ai_env_schema_validate_core::validate(
        env,
        schema,
        schema_format,
        unknown_keys,
        truthy(empty_is_missing),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}

/// Positive-truthy parse: a blank field defaults to ON (`empty_is_missing`
/// defaults to `true` in the descriptor, so a checked box → `"true"`, an
/// unchecked box → `"false"`, and a URL-prefill omitting the field → blank).
fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "yes" | "on"
    )
}
