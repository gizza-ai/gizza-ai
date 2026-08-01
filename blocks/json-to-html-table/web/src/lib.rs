//! Browser-facing wasm-bindgen wrapper for /tools/json-to-html-table/.
//! Field order MUST match page/meta.toml: json, format, nested, header,
//! null_text, caption, table_class, pretty.
use gizza_ai_json_to_html_table_core::{to_table, Format, Nested, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    json: &str,
    format: &str,
    nested: &str,
    header: &str,
    null_text: &str,
    caption: &str,
    table_class: &str,
    pretty: &str,
) -> Result<String, JsValue> {
    let hdr = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let pretty = !matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let opt = Options {
        format: Format::parse(format).map_err(|e| JsValue::from_str(&e))?,
        header: hdr,
        null_text: null_text.to_string(),
        nested: Nested::parse(nested).map_err(|e| JsValue::from_str(&e))?,
        caption: caption.to_string(),
        table_class: table_class.to_string(),
        pretty,
    };
    to_table(json, &opt).map_err(|e| JsValue::from_str(&e))
}
