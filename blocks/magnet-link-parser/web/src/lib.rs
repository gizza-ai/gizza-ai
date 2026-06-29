//! Browser-facing wasm-bindgen wrapper for /tools/magnet-link-parser/.
//! Field order MUST match meta.toml: mode, magnet, info_hash, display_name,
//! trackers, web_seeds, exact_length. The page passes every field as a string,
//! so `exact_length` is parsed here. `human = true` → parse mode renders the
//! aligned human-readable view (build mode returns the assembled URI either way).
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    mode: &str,
    magnet: &str,
    info_hash: &str,
    display_name: &str,
    trackers: &str,
    web_seeds: &str,
    exact_length: &str,
) -> Result<String, JsValue> {
    let exact_length = {
        let t = exact_length.trim();
        if t.is_empty() {
            None
        } else {
            t.parse::<u64>().ok()
        }
    };
    gizza_ai_magnet_link_parser_core::dispatch(
        mode,
        magnet,
        info_hash,
        display_name,
        trackers,
        web_seeds,
        exact_length,
        true,
    )
    .map_err(|e| JsValue::from_str(&e))
}
