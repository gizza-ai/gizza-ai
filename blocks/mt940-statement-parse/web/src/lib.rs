//! Browser-facing wasm-bindgen wrapper for /tools/mt940-statement-parse/.
use gizza_ai_mt940_statement_parse_core::run as core_run;
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    // Empty defaults to true (signed_amounts default); otherwise parse the flag.
    matches!(v.trim().to_ascii_lowercase().as_str(), "" | "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    output: &str,
    date_format: &str,
    delimiter: &str,
    signed_amounts: &str,
) -> Result<String, JsValue> {
    let out = if output.is_empty() { "json" } else { output };
    let fmt = if date_format.is_empty() { "iso" } else { date_format };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    core_run(data, out, fmt, delim, truthy(signed_amounts)).map_err(|e| JsValue::from_str(&e))
}
