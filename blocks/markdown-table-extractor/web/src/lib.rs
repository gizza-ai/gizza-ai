//! Browser-facing wasm-bindgen wrapper for /tools/markdown-table-extractor/.
//! Field order MUST match meta.toml: markdown, format, table, header, delimiter,
//! quote, newline, trim, strip_formatting, json_indent, labels.
use gizza_ai_markdown_table_extractor_core::{extract, parse_format, Options, Quote};
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false" strings; treat anything positive as on.
fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    markdown: &str,
    format: &str,
    table: &str,
    header: &str,
    delimiter: &str,
    quote: &str,
    newline: &str,
    trim: &str,
    strip_formatting: &str,
    json_indent: &str,
    labels: &str,
) -> Result<String, JsValue> {
    let fmt = parse_format(format).map_err(|e| JsValue::from_str(&e))?;
    let q = Quote::parse(quote).map_err(|e| JsValue::from_str(&e))?;
    let crlf = match newline.trim().to_ascii_lowercase().as_str() {
        "" | "lf" => false,
        "crlf" => true,
        other => {
            return Err(JsValue::from_str(&format!(
                "unknown newline '{other}' (expected lf or crlf)"
            )))
        }
    };
    let indent = match json_indent.trim() {
        "" => 2usize,
        n => n.parse::<usize>().map_err(|_| {
            JsValue::from_str(&format!("json_indent must be a whole number 0-8, got '{n}'"))
        })?,
    };
    if indent > 8 {
        return Err(JsValue::from_str(&format!(
            "json_indent must be between 0 and 8, got {indent}"
        )));
    }
    let opts = Options {
        format: fmt,
        table: if table.trim().is_empty() {
            "all".to_string()
        } else {
            table.to_string()
        },
        header: truthy(header),
        delimiter: if delimiter.is_empty() {
            ",".to_string()
        } else {
            delimiter.to_string()
        },
        quote: q,
        crlf,
        trim: truthy(trim),
        strip_formatting: truthy(strip_formatting),
        json_indent: indent,
        labels: truthy(labels),
    };
    extract(markdown, &opts).map_err(|e| JsValue::from_str(&e))
}
