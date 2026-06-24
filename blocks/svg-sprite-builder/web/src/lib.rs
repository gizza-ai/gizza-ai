//! Browser-facing wasm-bindgen wrapper for /tools/svg-sprite-builder/.
//! Compiled with wasm-pack for the standalone page. The page passes every field
//! value as a string, so `hidden` (a checkbox) arrives as "true"/"false".
use wasm_bindgen::prelude::*;

/// Build a `<symbol>` sprite sheet from concatenated `<svg>` documents.
///
/// - `svgs`: one or more complete `<svg>…</svg>` documents.
/// - `prefix`: auto-id prefix (blank → "icon", handled by the core).
/// - `id_source`: "auto" | "id" | "title" (blank → auto).
/// - `hidden`: "true"/"1"/"on"/"yes" → hidden wrapper; anything else → bare.
#[wasm_bindgen]
pub fn run(svgs: &str, prefix: &str, id_source: &str, hidden: &str) -> Result<String, JsValue> {
    let hidden = matches!(
        hidden.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    gizza_ai_svg_sprite_builder_core::run(svgs, prefix, id_source, hidden)
        .map_err(|e| JsValue::from_str(&e))
}
