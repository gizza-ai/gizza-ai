//! Browser-facing wasm-bindgen wrapper for /tools/mv-script-generator/.
//! Page fields arrive as strings; checkboxes as "true"/"false" (empty = the
//! descriptor default), so each flag is parsed positive-truthy with its own
//! default.
use wasm_bindgen::prelude::*;

fn flag(value: &str, default: bool) -> bool {
    match value.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    mapping: &str,
    format: &str,
    shell: &str,
    base_dir: &str,
    dry_run: &str,
    overwrite: &str,
    mkdir_parents: &str,
    undo_script: &str,
    comments: &str,
) -> Result<String, JsValue> {
    let format = if format.trim().is_empty() {
        "auto"
    } else {
        format
    };
    let shell = if shell.trim().is_empty() { "bash" } else { shell };
    gizza_ai_mv_script_generator_core::generate(
        mapping,
        format,
        shell,
        base_dir,
        flag(dry_run, false),
        flag(overwrite, false),
        flag(mkdir_parents, true),
        flag(undo_script, true),
        flag(comments, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
