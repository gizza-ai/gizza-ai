//! Browser-facing wasm-bindgen wrapper for /tools/scryptenc-file/.
//! Numeric params are `f64` (wasm-bindgen turns `i64` into a JS BigInt); the
//! param order here must match the `[[input]]` order in page/meta.toml.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    password: &str,
    operation: &str,
    data_encoding: &str,
    output_encoding: &str,
    log_n: f64,
    r: f64,
    p: f64,
    salt: &str,
    max_memory_mib: f64,
) -> Result<String, JsValue> {
    gizza_ai_scryptenc_file_core::run(
        operation,
        data,
        password,
        data_encoding,
        output_encoding,
        log_n as i64,
        r as i64,
        p as i64,
        salt,
        max_memory_mib as i64,
    )
    .map_err(|e| JsValue::from_str(&e))
}
