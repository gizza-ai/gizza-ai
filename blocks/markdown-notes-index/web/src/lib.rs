//! Browser-facing wasm-bindgen wrapper for /tools/markdown-notes-index/.
//! Field order MUST match meta.toml: notes, split, format, heading_depth,
//! group_by, sort, link_style, include_toc, include_stats, inline_tags.
//! Fields arrive as strings (checkboxes send "true"/"false").
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
    notes: &str,
    split: &str,
    format: &str,
    heading_depth: &str,
    group_by: &str,
    sort: &str,
    link_style: &str,
    include_toc: &str,
    include_stats: &str,
    inline_tags: &str,
) -> Result<String, JsValue> {
    let depth_text = heading_depth.trim();
    let depth: u32 = if depth_text.is_empty() {
        2
    } else {
        depth_text.parse().map_err(|_| {
            JsValue::from_str(&format!(
                "heading depth must be a whole number between 0 and 6, got \"{depth_text}\""
            ))
        })?
    };

    gizza_ai_markdown_notes_index_core::run(
        notes,
        split,
        format,
        depth,
        group_by,
        sort,
        link_style,
        truthy(include_toc),
        truthy(include_stats),
        truthy(inline_tags),
    )
    .map_err(|e| JsValue::from_str(&e))
}
