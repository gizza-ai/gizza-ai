//! Browser-facing wasm-bindgen wrapper for /tools/css-reset-generator/.
//! Every field arrives as a string (the page passes raw field values), so the
//! wrapper restores the descriptor defaults for blank fields and parses the
//! numeric + boolean fields itself.
use wasm_bindgen::prelude::*;

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

fn parse_f64(name: &str, s: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        Ok(fallback)
    } else {
        t.parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got `{t}`")))
    }
}

fn parse_bool(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => fallback,
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    preset: &str,
    include: &str,
    exclude: &str,
    selector_style: &str,
    layer: &str,
    line_height: &str,
    body_min_height: &str,
    comments: &str,
    minify: &str,
    indent: &str,
) -> Result<String, JsValue> {
    gizza_ai_css_reset_generator_core::run(
        &or_default(preset, "modern"),
        include,
        exclude,
        &or_default(selector_style, "standard"),
        layer,
        parse_f64("line_height", line_height, 1.5)?,
        &or_default(body_min_height, "100svh"),
        parse_bool(comments, true),
        parse_bool(minify, false),
        parse_f64("indent", indent, 2.0)?,
    )
    .map_err(|e| JsValue::from_str(&e))
}
