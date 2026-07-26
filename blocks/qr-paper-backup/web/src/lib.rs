//! Browser-facing wasm-bindgen wrapper for /tools/qr-paper-backup/.
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    input_encoding: &str,
    chunk_bytes: &str,
    columns: &str,
    error_correction: &str,
    show_text: &str,
) -> Result<String, JsValue> {
    let chunk = chunk_bytes.trim().parse::<u32>().unwrap_or(300);
    let cols = columns.trim().parse::<u32>().unwrap_or(3);
    gizza_ai_qr_paper_backup_core::run(
        input,
        input_encoding,
        chunk,
        cols,
        error_correction,
        truthy(show_text),
    )
    .map_err(|e| JsValue::from_str(&e))
}
