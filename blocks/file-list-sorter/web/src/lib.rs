//! Browser-facing wasm-bindgen wrapper for /tools/file-list-sorter/.
//! Field order MUST match meta.toml: paths, sort_by, order, ignore_case,
//! dirs_first, group_by_dir, unique, trim, format. Fields arrive as strings
//! (checkboxes send "true"/"false").
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    paths: &str,
    sort_by: &str,
    order: &str,
    ignore_case: &str,
    dirs_first: &str,
    group_by_dir: &str,
    unique: &str,
    trim: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_file_list_sorter_core::run(
        paths,
        sort_by,
        order,
        truthy(ignore_case),
        truthy(dirs_first),
        truthy(group_by_dir),
        truthy(unique),
        truthy(trim),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
