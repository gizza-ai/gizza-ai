//! Browser-facing wasm-bindgen wrapper for /tools/color-contrast-checker/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Compute the WCAG contrast report for `foreground` on `background`.
///
/// Each colour is a hex code (`#1a2b3c`, `#fff`, bare `1a2b3c`), an rgb triple
/// (`rgb(26,43,60)` / `26,43,60`), an hsl triple (`hsl(210,40%,17%)`), or a CSS
/// colour name (`navy`). `format` is `"text"` (default/blank), `"json"`, or
/// `"suggest"`. `target` (`"aa"`/`"aaa"`/`"large"`) is used by `suggest`. Throws a
/// JS error string on an invalid colour, format, or target.
#[wasm_bindgen]
pub fn run(
    foreground: &str,
    background: &str,
    format: &str,
    target: &str,
) -> Result<String, JsValue> {
    gizza_ai_color_contrast_checker_core::run(foreground, background, format, target)
        .map_err(|e| JsValue::from_str(&e))
}
