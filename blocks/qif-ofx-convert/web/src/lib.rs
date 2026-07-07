//! Browser-facing wasm-bindgen wrapper for /tools/qif-ofx-convert/.
//!
//! tool.js passes EVERY page field as a raw string (no coercion for pure tools),
//! so this export takes `&str` for every param and parses the enum/bool fields
//! here; the core owns all validation. Param order MUST match page/meta.toml's
//! [[input]] order.
use gizza_ai_qif_ofx_convert_core::{convert, DateFormat, Format};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    format: &str,
    date_format: &str,
    delimiter: &str,
    invert_amounts: &str,
    drop_empty_columns: &str,
) -> Result<String, JsValue> {
    let fmt = Format::parse(format).map_err(|e| JsValue::from_str(&e))?;
    let df = DateFormat::parse(date_format).map_err(|e| JsValue::from_str(&e))?;
    let invert = parse_bool(invert_amounts);
    let drop_empty = parse_bool(drop_empty_columns);
    convert(data, fmt, df, delimiter, invert, drop_empty).map_err(|e| JsValue::from_str(&e))
}

fn parse_bool(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
