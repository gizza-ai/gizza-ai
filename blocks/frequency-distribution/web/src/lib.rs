//! Browser-facing wasm-bindgen wrapper for /tools/frequency-distribution/.
//! Field order MUST match meta.toml: input, input_kind, mode, n, case_sensitive.
use gizza_ai_frequency_distribution_core::{format_table, InputKind, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_kind: &str,
    mode: &str,
    n: &str,
    case_sensitive: &str,
) -> Result<String, JsValue> {
    let kind = match input_kind.trim().to_ascii_lowercase().as_str() {
        "hex" => InputKind::Hex,
        _ => InputKind::Text,
    };
    let nn: usize = n.trim().parse().unwrap_or(2).max(1);
    let m = match mode.trim().to_ascii_lowercase().as_str() {
        "byte" => Mode::Byte,
        "ngram" => Mode::Ngram(nn),
        _ => Mode::Char,
    };
    // Checkbox default(true) sends "true" when checked; positive-truthy parse.
    let cs = matches!(
        case_sensitive.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    format_table(input, kind, m, cs).map_err(|e| JsValue::from_str(&e))
}
