//! Browser-facing wasm-bindgen wrapper for /tools/css-color-converter/.
//! Field order MUST match meta.toml: input, syntax, precision, uppercase_hex.
//! Every field arrives as a string (checkboxes send "true"/"false", and an empty
//! box means "leave it at the default").
use gizza_ai_css_color_converter_core::{convert, render_text, Options, Syntax};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// An empty field means "the default" — the page clears a box rather than
/// deleting the param, so the core must not see "".
fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    syntax: &str,
    precision: &str,
    uppercase_hex: &str,
) -> Result<String, JsValue> {
    let syntax = Syntax::parse(or_default(syntax, "legacy")).map_err(err)?;
    let text = precision.trim();
    let precision: u32 = if text.is_empty() {
        3
    } else {
        text.parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| v.round().clamp(0.0, 8.0) as u32)
            .ok_or_else(|| {
                err(format!(
                    "precision must be a whole number of decimal places between 0 and 8, got \"{text}\""
                ))
            })?
    };

    let opts = Options {
        syntax,
        precision,
        uppercase_hex: truthy(uppercase_hex),
    };
    convert(input, &opts).map(|c| render_text(&c)).map_err(err)
}

fn err(message: impl AsRef<str>) -> JsValue {
    JsValue::from_str(message.as_ref())
}
