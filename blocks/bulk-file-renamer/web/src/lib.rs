//! Browser-facing wasm-bindgen wrapper for /tools/bulk-file-renamer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    filenames: &str,
    mode: &str,
    find: &str,
    replace: &str,
    case_type: &str,
    pattern: &str,
    start: &str,
    padding: &str,
    prefix: &str,
    suffix: &str,
    preserve_extension: &str,
) -> Result<String, JsValue> {
    let start = start.trim().parse::<i64>().unwrap_or(1);
    let padding = padding.trim().parse::<i64>().unwrap_or(1);
    let preserve = match preserve_extension.trim().to_ascii_lowercase().as_str() {
        "" => true,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    };
    gizza_ai_bulk_file_renamer_core::run_named(
        filenames,
        mode,
        find,
        replace,
        case_type,
        pattern,
        start,
        padding,
        prefix,
        suffix,
        preserve,
    )
    .map_err(|e| JsValue::from_str(&e))
}
