//! Browser-facing wasm-bindgen wrapper for /tools/import-graph-extractor/.
//! The standalone page passes every field value as a string, so the boolean
//! params arrive as strings and are parsed here (blank → default).
use wasm_bindgen::prelude::*;

fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        _ => false,
    }
}

/// Extract the import graph from pasted `input`.
///
/// - `language`: `auto` (blank → auto) / `javascript` / `python` / `rust` / `go`.
/// - `format`: `text` (blank → text) / `json` / `dot` / `mermaid`.
/// - `include_external`: `"true"`/`"1"`/`"yes"`/`"on"` → include (blank → true).
/// - `detect_cycles`: same truthy parsing (blank → true).
#[wasm_bindgen]
pub fn run(
    input: &str,
    language: &str,
    format: &str,
    include_external: &str,
    detect_cycles: &str,
) -> Result<String, JsValue> {
    gizza_ai_import_graph_extractor_core::run(
        input,
        language,
        format,
        flag(include_external, true),
        flag(detect_cycles, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
