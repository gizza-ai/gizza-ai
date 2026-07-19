//! Browser-facing wasm-bindgen wrapper for /tools/gpx-merge/.
//! Field order MUST match meta.toml: input, merge_mode, sort_by_time, dedupe,
//! include_waypoints, track_name. The page passes every field value as a
//! string (checkboxes send "true"/"false"; the enum sends its value).
use gizza_ai_gpx_merge_core::{merge, MergeMode, Options};
use wasm_bindgen::prelude::*;

/// Positive-truthy parse of a page checkbox value, preserving descriptor defaults
/// when the generator passes an empty string during first render.
fn truthy(s: &str, default: bool) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return default;
    }
    matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    input: &str,
    merge_mode: &str,
    sort_by_time: &str,
    dedupe: &str,
    include_waypoints: &str,
    track_name: &str,
) -> Result<String, JsValue> {
    let mode = if merge_mode.trim().is_empty() { "single-track" } else { merge_mode };
    let merge_mode = MergeMode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let track_name = if track_name.trim().is_empty() {
        "Merged track".to_string()
    } else {
        track_name.to_string()
    };
    let opt = Options {
        merge_mode,
        sort_by_time: truthy(sort_by_time, true),
        dedupe: truthy(dedupe, false),
        include_waypoints: truthy(include_waypoints, true),
        track_name,
    };
    merge(input, &opt).map_err(|e| JsValue::from_str(&e))
}
