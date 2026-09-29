//! Browser-facing wasm-bindgen wrapper for /tools/scale-chord-finder/.
//!
//! The page passes every control through as a string in the order they are
//! declared in `page/meta.toml`, so this mirrors the block descriptor's params
//! one-for-one and hands the parsed values to the same core entry point the
//! chat and CLI surfaces use. Blank fields fall back to the core's defaults.
use gizza_ai_scale_chord_finder_core as core;
use wasm_bindgen::prelude::*;

/// A blank select/text field means "keep the default" rather than "empty".
fn or_default(v: &str, default: &str) -> String {
    if v.trim().is_empty() {
        default.to_string()
    } else {
        v.trim().to_string()
    }
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    action: &str,
    notes: &str,
    root: &str,
    key: &str,
    scale: &str,
    fit: &str,
    spelling: &str,
    include_chords: &str,
    include_modes: &str,
    chord_type: &str,
    max_results: &str,
    output: &str,
) -> Result<String, JsValue> {
    let d = core::Options::default();
    let opts = core::Options {
        action: or_default(action, &d.action),
        // `notes` is the one field whose blank value is meaningful: an empty
        // note list is what makes `auto` spell a scale instead of searching.
        notes: notes.trim().to_string(),
        root: or_default(root, &d.root),
        key: or_default(key, &d.key),
        scale: or_default(scale, &d.scale),
        fit: or_default(fit, &d.fit),
        spelling: or_default(spelling, &d.spelling),
        include_chords: core::truthy(include_chords, d.include_chords),
        include_modes: core::truthy(include_modes, d.include_modes),
        chord_type: or_default(chord_type, &d.chord_type),
        max_results: core::parse_field("max_results", max_results, d.max_results)
            .map_err(|e| JsValue::from_str(&e))?,
        output: or_default(output, &d.output),
    };
    core::run(&opts).map_err(|e| JsValue::from_str(&e))
}
