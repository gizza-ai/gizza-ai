//! Browser-facing wasm-bindgen wrapper for /tools/chunk-list/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    items: &str,
    input_separator: &str,
    custom_separator: &str,
    chunk_size: &str,
    output: &str,
    label_chunks: &str,
) -> Result<String, JsValue> {
    fn truthy(s: &str, default: bool) -> bool {
        match s.trim().to_ascii_lowercase().as_str() {
            "" => default,
            "true" | "1" | "on" | "yes" => true,
            "false" | "0" | "off" | "no" => false,
            _ => default,
        }
    }

    let size = chunk_size.trim().parse::<usize>().unwrap_or(10);
    gizza_ai_chunk_list_core::chunk_list(
        items,
        input_separator,
        custom_separator,
        size,
        output,
        truthy(label_chunks, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
