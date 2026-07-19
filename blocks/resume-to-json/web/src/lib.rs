//! Browser-facing wasm-bindgen wrapper for /tools/resume-to-json/.
//!
//! The standalone page passes every field value as a string, so the boolean
//! params (`schema_ref`, `pretty`) arrive as strings and are parsed here.
//! `schema_ref` defaults OFF (positive truthy only); `pretty` defaults ON
//! (blank → true; only an explicit false/0/off/no turns it off). `mode` is
//! parsed by the core (blank → "auto").
use gizza_ai_resume_to_json_core::{run as run_core, Mode};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, mode: &str, schema_ref: &str, pretty: &str) -> Result<String, JsValue> {
    let mode = Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?;
    let schema_ref = matches!(
        schema_ref.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    let pretty = !matches!(
        pretty.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    run_core(data, mode, schema_ref, pretty).map_err(|e| JsValue::from_str(&e))
}
