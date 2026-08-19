//! Browser-facing wasm-bindgen wrapper for /tools/data-format-sniffer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    input_form: &str,
    sample_lines: &str,
    extra_delimiters: &str,
    comment_prefix: &str,
    detect_types: &str,
    preview_rows: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = gizza_ai_data_format_sniffer_core::Options {
        input_form: default_str(input_form, "text").to_string(),
        sample_lines: parse_usize_default(sample_lines, 100, "sample_lines")?,
        extra_delimiters: extra_delimiters.to_string(),
        comment_prefix: comment_prefix.to_string(),
        detect_types: truthy(detect_types, true),
        preview_rows: parse_usize_default(preview_rows, 5, "preview_rows")?,
        output: default_str(output, "report").to_string(),
    };
    gizza_ai_data_format_sniffer_core::sniff(data, &opts).map_err(|e| JsValue::from_str(&e))
}

fn default_str<'a>(v: &'a str, default: &'a str) -> &'a str {
    if v.trim().is_empty() {
        default
    } else {
        v.trim()
    }
}

fn truthy(v: &str, default: bool) -> bool {
    let s = v.trim().to_ascii_lowercase();
    if s.is_empty() {
        default
    } else {
        matches!(s.as_str(), "true" | "1" | "on" | "yes")
    }
}

fn parse_usize_default(v: &str, default: usize, name: &str) -> Result<usize, JsValue> {
    if v.trim().is_empty() {
        Ok(default)
    } else {
        v.trim()
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{name} must be a whole number")))
    }
}
