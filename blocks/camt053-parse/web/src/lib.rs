//! Browser-facing wasm-bindgen wrapper for /tools/camt053-parse/.
use gizza_ai_camt053_parse_core::run as core_run;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    // Empty defaults to true (both booleans default true); otherwise parse the flag.
    matches!(v.trim().to_ascii_lowercase().as_str(), "" | "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    output: &str,
    date_format: &str,
    delimiter: &str,
    signed_amounts: &str,
    expand_details: &str,
) -> Result<String, JsValue> {
    let out = if output.is_empty() { "json" } else { output };
    let fmt = if date_format.is_empty() { "iso" } else { date_format };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    core_run(data, out, fmt, delim, truthy(signed_amounts), truthy(expand_details))
        .map_err(|e| JsValue::from_str(&e))
}
