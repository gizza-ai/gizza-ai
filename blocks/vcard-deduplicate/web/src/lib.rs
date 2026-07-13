//! Browser-facing wasm-bindgen wrapper for /tools/vcard-deduplicate/.
//!
//! The standalone page passes every field value as a string. `match_by` goes
//! straight to the core (blank → "any"); `merge` is a checkbox that defaults ON
//! (blank/checked → true; only an explicit false/0/off/no turns it off).
use gizza_ai_vcard_deduplicate_core::{run as run_core, MatchBy};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, match_by: &str, merge: &str) -> Result<String, JsValue> {
    let match_by = MatchBy::parse(match_by).map_err(|e| JsValue::from_str(&e))?;
    let merge = !matches!(
        merge.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    run_core(data, match_by, merge).map_err(|e| JsValue::from_str(&e))
}
