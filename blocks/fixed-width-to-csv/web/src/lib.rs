//! Browser-facing wasm-bindgen wrapper for /tools/fixed-width-to-csv/.
//! tool.js passes every field value as a raw string, so parse them here and
//! funnel through the same core validation the chat/CLI surfaces use.
use gizza_ai_fixed_width_to_csv_core::{convert, HeaderMode, Options, Quote};
use wasm_bindgen::prelude::*;

/// Checkbox that defaults to ON: empty (not sent) means on; only an explicit
/// falsey value turns it off.
fn on_by_default(v: &str) -> bool {
    !matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    )
}

/// Checkbox that defaults to OFF: only an explicit truthy value turns it on.
fn off_by_default(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    spec: &str,
    header: &str,
    trim: &str,
    delimiter: &str,
    quote: &str,
    newline: &str,
    skip_lines: &str,
    comment: &str,
    skip_blank: &str,
    bom: &str,
) -> Result<String, JsValue> {
    let quote = Quote::parse(quote).map_err(|e| JsValue::from_str(&e))?;
    let header = HeaderMode::parse(header).map_err(|e| JsValue::from_str(&e))?;
    let crlf = match newline.trim().to_ascii_lowercase().as_str() {
        "" | "lf" | "\n" => false,
        "crlf" | "\r\n" => true,
        other => {
            return Err(JsValue::from_str(&format!(
                "unknown newline '{other}' (use lf or crlf)"
            )))
        }
    };
    let skip = skip_lines.trim();
    let skip_lines: usize = if skip.is_empty() {
        0
    } else {
        skip.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "skip lines must be a whole number of lines (0 or more), got '{skip}'"
            ))
        })?
    };
    let opts = Options {
        spec: spec.to_string(),
        min_gap: 1,
        header,
        trim: on_by_default(trim),
        delimiter: if delimiter.is_empty() {
            ",".to_string()
        } else {
            delimiter.to_string()
        },
        quote,
        crlf,
        bom: off_by_default(bom),
        skip_lines,
        comment: comment.to_string(),
        skip_blank: on_by_default(skip_blank),
    };
    convert(text, &opts).map_err(|e| JsValue::from_str(&e))
}
