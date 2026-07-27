//! Browser-facing wasm-bindgen wrapper for /tools/markdown-table-to-csv/.
//! tool.js passes every field value as a raw string, so parse them here and
//! funnel through the same core validation the chat/CLI surfaces use.
use gizza_ai_markdown_table_to_csv_core::{to_csv_ext, Quote};
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    markdown: &str,
    delimiter: &str,
    header: &str,
    quote: &str,
    newline: &str,
    bom: &str,
) -> Result<String, JsValue> {
    // Header checkbox: default keep (empty => true); only an explicit falsey drops it.
    let has_header = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let q = Quote::parse(quote).map_err(|e| JsValue::from_str(&e))?;
    let crlf = match newline.trim().to_ascii_lowercase().as_str() {
        "" | "lf" | "\n" => false,
        "crlf" | "\r\n" => true,
        other => return Err(JsValue::from_str(&format!("unknown newline '{other}' (use lf or crlf)"))),
    };
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    to_csv_ext(markdown, delim, has_header, q, crlf, truthy(bom))
        .map_err(|e| JsValue::from_str(&e))
}
