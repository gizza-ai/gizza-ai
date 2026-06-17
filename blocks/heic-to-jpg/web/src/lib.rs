//! Browser-facing wasm-bindgen wrapper for /tools/heic-to-jpg/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.
//!
//! Param ORDER is `(format, in_name)` — the page passes the `format` field value
//! first, then the virtual input filename (per site/tool-ffmpeg.js:
//! `mod[export](...fieldArgs, inName)`). The page field order in meta.toml must
//! match: the file input, then the `format` field.
use serde::Serialize;
use wasm_bindgen::prelude::*;

use gizza_ai_heic_to_jpg_core::{parse_format, plan};

#[derive(Serialize)]
struct Plan {
    argv: Vec<String>,
    out_name: String,
}

/// `format` is `"jpg"` (default; empty also means jpg) or `"png"`. Returns
/// `{ argv: string[], out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(format: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let fmt = parse_format(Some(format)).map_err(|e| JsValue::from_str(&e))?;
    let (argv, out_name) = plan(in_name, fmt);
    serde_wasm_bindgen::to_value(&Plan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
