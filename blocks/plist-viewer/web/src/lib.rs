//! Browser-facing wasm-bindgen wrapper for /tools/plist-viewer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    format: &str,
    indent: &str,
    sort_keys: &str,
    data_encoding: &str,
) -> Result<String, JsValue> {
    use gizza_ai_plist_viewer_core::{convert, DataEncoding, Format, Options};

    let format = match format.trim().to_ascii_lowercase().as_str() {
        "" | "json" => Format::Json,
        "tree" | "outline" => Format::Tree,
        other => return Err(JsValue::from_str(&format!("unknown format '{other}' (use json or tree)"))),
    };
    let data_encoding = match data_encoding.trim().to_ascii_lowercase().as_str() {
        "" | "base64" => DataEncoding::Base64,
        "hex" => DataEncoding::Hex,
        other => return Err(JsValue::from_str(&format!("unknown data_encoding '{other}' (use base64 or hex)"))),
    };
    let indent = indent.trim().parse::<usize>().unwrap_or(2).min(8);
    let sort_keys = matches!(
        sort_keys.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let opt = Options { format, indent, sort_keys, data_encoding };
    convert(input, &opt).map_err(|e| JsValue::from_str(&e))
}
