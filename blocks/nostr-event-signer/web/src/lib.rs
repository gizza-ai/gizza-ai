//! Browser-facing wasm-bindgen wrapper for /tools/nostr-event-signer/.
//! Param order MUST match the field order in page/meta.toml.
//!
//! wasm32-unknown-unknown has no std clock, so the "sign it now" default
//! (`created_at = 0`) reads the browser clock here and hands the core an
//! explicit timestamp — the core itself stays deterministic.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    nsec: &str,
    content: &str,
    kind: f64,
    tags: &str,
    created_at: f64,
    template: &str,
    pow: f64,
    output: &str,
    pretty: bool,
) -> Result<String, JsValue> {
    let now_unix = (js_sys::Date::now() / 1000.0).floor() as i64;
    gizza_ai_nostr_event_signer_core::sign_event(
        nsec, content, kind, tags, created_at, template, pow, output, pretty, now_unix,
    )
    .map_err(|e| JsValue::from_str(&e))
}
