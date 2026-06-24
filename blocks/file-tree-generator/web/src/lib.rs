//! Browser-facing wasm-bindgen wrapper for /tools/file-tree-generator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Render a file list or outline as an ASCII tree.
///
/// The standalone tool page passes every field value as a string:
/// - `input`: the file list (paths mode) or indented outline.
/// - `mode`: `"paths"`/`"outline"` (blank → paths).
/// - `root`: the root label (blank → ".").
/// - `ascii`: truthy → plain-ASCII connectors (default false / off).
/// - `trailing_slash`: truthy → add "/" to directories (default true / on).
/// - `sort`: truthy → directories-first alphabetical sort (default false / off).
///
/// Throws a JS error string on an invalid `mode` or empty input.
#[wasm_bindgen]
pub fn run(
    input: &str,
    mode: &str,
    root: &str,
    ascii: &str,
    trailing_slash: &str,
    sort: &str,
) -> Result<String, JsValue> {
    let ascii = truthy(ascii, false);
    // Default-true checkbox: blank means the box is still default-checked.
    let trailing_slash = truthy(trailing_slash, true);
    let sort = truthy(sort, false);
    gizza_ai_file_tree_generator_core::generate(input, mode, root, ascii, trailing_slash, sort)
        .map_err(|e| JsValue::from_str(&e))
}

/// Interpret a checkbox value string. Blank → `default` (the box is rendered
/// checked iff the descriptor default is true, and a checked box sends "true").
fn truthy(v: &str, default: bool) -> bool {
    match v.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}
