//! Browser-facing wasm-bindgen wrapper for /tools/dotenv-to-shell/.
//! The standalone page passes every field value as a string.
use wasm_bindgen::prelude::*;

/// Convert a `.env` file into shell export statements (and back).
///
/// - `input`: the source text (required).
/// - `direction`: `to-shell` (default) | `to-env`.
/// - `shell`: `posix` (default) | `bash` | `fish` — dialect for to-shell.
/// - `quote`: `auto` (default) | `single` — value quoting for to-shell.
///
/// Throws a JS error string on an invalid `direction`, `shell`, or `quote`.
#[wasm_bindgen]
pub fn run(input: &str, direction: &str, shell: &str, quote: &str) -> Result<String, JsValue> {
    gizza_ai_dotenv_to_shell_core::convert(input, direction, shell, quote)
        .map_err(|e| JsValue::from_str(&e))
}
