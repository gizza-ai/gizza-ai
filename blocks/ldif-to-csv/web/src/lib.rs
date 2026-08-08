//! Browser-facing wasm-bindgen wrapper for /tools/ldif-to-csv/.
use gizza_ai_ldif_to_csv_core::{to_csv, MultiValue, Options};
use wasm_bindgen::prelude::*;

fn truthy_default_on(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

fn truthy_default_off(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_i64(v: &str, fallback: i64, field: &str) -> Result<i64, JsValue> {
    if v.trim().is_empty() {
        return Ok(fallback);
    }
    v.trim()
        .parse::<i64>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number")))
}

#[wasm_bindgen]
pub fn run(
    ldif: &str,
    columns: &str,
    include_dn: &str,
    multi_value: &str,
    value_separator: &str,
    max_indexed: &str,
    decode_base64: &str,
    delimiter: &str,
    include_changetype: &str,
) -> Result<String, JsValue> {
    let multi_value = MultiValue::parse(multi_value).map_err(|e| JsValue::from_str(&e))?;
    let opts = Options {
        columns: columns.to_string(),
        include_dn: truthy_default_on(include_dn),
        multi_value,
        value_separator: if value_separator.trim().is_empty() {
            "|".to_string()
        } else {
            value_separator.to_string()
        },
        max_indexed: parse_i64(max_indexed, 10, "max_indexed")?,
        decode_base64: truthy_default_on(decode_base64),
        delimiter: if delimiter.trim().is_empty() {
            ",".to_string()
        } else {
            delimiter.to_string()
        },
        include_changetype: truthy_default_off(include_changetype),
    };
    to_csv(ldif, &opts).map_err(|e| JsValue::from_str(&e))
}
