//! Browser-facing wasm-bindgen wrapper for /tools/vcard-to-json/.
//!
//! The standalone page passes every field value as a string, so the boolean
//! params (`structured`, `pretty`) arrive as strings and are parsed here.
//! `structured` defaults ON (blank → true; only an explicit false/0/off/no turns
//! it off); `pretty` defaults OFF (blank → false). `wrap` is passed straight
//! through to the core (blank → "array").
use gizza_ai_vcard_to_json_core::{run as run_core, Wrap};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, structured: &str, wrap: &str, pretty: &str) -> Result<String, JsValue> {
    let structured = !matches!(
        structured.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let pretty = matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let wrap = Wrap::parse(wrap).map_err(|e| JsValue::from_str(&e))?;
    run_core(data, structured, wrap, pretty).map_err(|e| JsValue::from_str(&e))
}
