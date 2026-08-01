//! Browser-facing wasm-bindgen wrapper for /tools/iso9660-toc-list/.
use gizza_ai_iso9660_toc_list_core::run as run_core;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, format: &str) -> Result<String, JsValue> {
    let fmt = if format.trim().is_empty() {
        "tree"
    } else {
        format
    };
    run_core(data, fmt).map_err(|e| JsValue::from_str(&e))
}
