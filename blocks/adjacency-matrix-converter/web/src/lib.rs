//! Browser-facing wasm-bindgen wrapper for /tools/adjacency-matrix-converter/.
//! The standalone page passes every field value as a string, so the boolean
//! params arrive as strings and are parsed here.
use wasm_bindgen::prelude::*;

/// Convert a graph between an edge list / adjacency matrix / incidence matrix.
///
/// - `from`: `"edges"` (default) | `"adjacency"`.
/// - `to`: `"adjacency"` (default) | `"incidence"` | `"edges"`.
/// - `directed` / `weighted`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
#[wasm_bindgen]
pub fn run(
    input: &str,
    from: &str,
    to: &str,
    directed: &str,
    weighted: &str,
) -> Result<String, JsValue> {
    let truthy =
        |s: &str| matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
    gizza_ai_adjacency_matrix_converter_core::convert(
        input,
        from,
        to,
        truthy(directed),
        truthy(weighted),
    )
    .map_err(|e| JsValue::from_str(&e))
}
