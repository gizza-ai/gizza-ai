//! Browser-facing wasm-bindgen wrapper for /tools/ansi-log-renderer/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Render raw terminal output (with ANSI escape codes) as colored HTML, or strip
/// the codes to plain text.
///
/// The standalone tool page passes every field value as a string:
/// - `output`: `"html"` (blank → html) renders colored HTML; `"text"` strips to
///   plain text.
/// - `theme`: `"dark"` (blank → dark) or `"light"` — HTML default colors/background.
/// - `styles`: `"inline"` (blank → inline) or `"classes"` — how HTML colors apply.
///
/// Throws a JS error string on an invalid `output`, `theme`, or `styles`.
#[wasm_bindgen]
pub fn run(text: &str, output: &str, theme: &str, styles: &str) -> Result<String, JsValue> {
    gizza_ai_ansi_log_renderer_core::render(text, output, theme, styles)
        .map_err(|e| JsValue::from_str(&e))
}
