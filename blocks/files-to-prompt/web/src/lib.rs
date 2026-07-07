//! Browser-facing wasm-bindgen wrapper for /tools/files-to-prompt/.
//! Field order MUST match page/meta.toml: files, format, separator,
//! line_numbers, include_tree. The page passes every field as a string
//! (checkboxes arrive as "true"/"false"); we parse the booleans here.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    files: &str,
    format: &str,
    separator: &str,
    line_numbers: &str,
    include_tree: &str,
) -> Result<String, JsValue> {
    // line_numbers defaults OFF: only positive-truthy turns it on.
    let ln = matches!(
        line_numbers.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    // include_tree defaults ON: anything but an explicit falsey keeps it on
    // (an absent/empty field means "use the default", which is true).
    let tree = !matches!(
        include_tree.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    gizza_ai_files_to_prompt_core::build_digest(files, format, separator, ln, tree)
        .map_err(|e| JsValue::from_str(&e))
}
