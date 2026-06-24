//! Browser-facing wasm-bindgen wrapper for /tools/adjacency-matrix-converter/.
//! The standalone page passes every field value as a string, so the boolean
//! params arrive as strings and are parsed here.
use wasm_bindgen::prelude::*;

/// Convert a graph between an edge list / adjacency matrix / incidence matrix.
///
/// - `from`: `"auto"` (default) | `"edges"` | `"adjacency"` | `"list"` | `"incidence"`.
/// - `to`: `"adjacency"` (default) | `"incidence"` | `"edges"` | `"list"` | `"degree"` | `"laplacian"` | `"stats"` | `"power"`.
/// - `directed` / `weighted`: `"true"`/`"1"`/`"yes"`/`"on"` → on; else off.
/// - `power`: Matrix exponent k (default 2).
#[wasm_bindgen]
pub fn run(
    input: &str,
    from: &str,
    to: &str,
    directed: &str,
    weighted: &str,
    power: &str,
) -> Result<String, JsValue> {
    let truthy =
        |s: &str| matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "on");
    let p = power.trim().parse::<i64>().unwrap_or(2);
    gizza_ai_adjacency_matrix_converter_core::convert(
        input,
        from,
        to,
        truthy(directed),
        truthy(weighted),
        p,
    )
    .map_err(|e| JsValue::from_str(&e))
}
