//! Browser-facing wasm-bindgen wrapper for /tools/stl-repair/.
//! Field order MUST match meta.toml: stl, output, weld_tolerance,
//! remove_degenerate, remove_duplicates, fix_winding, fill_holes,
//! keep_largest_shell, stl_encoding.
//! Every field arrives as a string (the page passes raw strings, no coercion),
//! so the number and the checkboxes are parsed here; enums are validated in `core`.
use gizza_ai_stl_repair_core::{repair, Options, Output, StlEncoding};
use wasm_bindgen::prelude::*;

/// Checkboxes arrive as "true"/"false"; be generous about the positive forms.
fn flag(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    stl: &str,
    output: &str,
    weld_tolerance: &str,
    remove_degenerate: &str,
    remove_duplicates: &str,
    fix_winding: &str,
    fill_holes: &str,
    keep_largest_shell: &str,
    stl_encoding: &str,
) -> Result<String, JsValue> {
    // Blank tolerance → the default; a non-numeric value is a clear error.
    let weld_tolerance = match weld_tolerance.trim() {
        "" => 0.000001,
        s => s.parse::<f64>().map_err(|_| {
            JsValue::from_str(&format!("weld tolerance '{s}' is not a number"))
        })?,
    };
    let opt = Options {
        output: Output::parse(output).map_err(|e| JsValue::from_str(&e))?,
        weld_tolerance,
        remove_degenerate: flag(remove_degenerate),
        remove_duplicates: flag(remove_duplicates),
        fix_winding: flag(fix_winding),
        fill_holes: flag(fill_holes),
        keep_largest_shell: flag(keep_largest_shell),
        stl_encoding: StlEncoding::parse(stl_encoding).map_err(|e| JsValue::from_str(&e))?,
    };
    repair(stl, &opt).map_err(|e| JsValue::from_str(&e))
}
