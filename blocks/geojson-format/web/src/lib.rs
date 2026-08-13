//! Browser-facing wasm-bindgen wrapper for /tools/geojson-format/.
//! Field order MUST match meta.toml: input, indent, indent_char, precision,
//! key_order, bbox, winding, keep_properties, drop_properties,
//! drop_empty_properties, validate.
use wasm_bindgen::prelude::*;

fn parse_i64(raw: &str, default: i64, name: &str) -> Result<i64, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        Ok(default)
    } else {
        t.parse::<i64>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}

fn truthy_default_true(raw: &str) -> bool {
    !matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_false(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    indent: &str,
    indent_char: &str,
    precision: &str,
    key_order: &str,
    bbox: &str,
    winding: &str,
    keep_properties: &str,
    drop_properties: &str,
    drop_empty_properties: &str,
    validate: &str,
) -> Result<String, JsValue> {
    let indent = parse_i64(indent, 2, "indent")?;
    let precision = parse_i64(precision, -1, "precision")?;
    let indent_char = if indent_char.trim().is_empty() {
        "space"
    } else {
        indent_char
    };
    let key_order = if key_order.trim().is_empty() {
        "keep"
    } else {
        key_order
    };
    let bbox = if bbox.trim().is_empty() { "keep" } else { bbox };
    let winding = if winding.trim().is_empty() {
        "keep"
    } else {
        winding
    };

    gizza_ai_geojson_format_core::run(
        input,
        indent,
        indent_char,
        precision,
        key_order,
        bbox,
        winding,
        keep_properties,
        drop_properties,
        truthy_default_false(drop_empty_properties),
        truthy_default_true(validate),
    )
    .map_err(|e| JsValue::from_str(&e))
}
