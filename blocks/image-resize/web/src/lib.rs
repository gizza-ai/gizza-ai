//! Browser-facing wasm-bindgen wrapper for the standalone /tools/image-resize/ page.
//! Builds the ffmpeg argv (pure, shared with the chat block via core); the JS
//! page driver runs it through the browser ffmpeg bridge.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use gizza_ai_image_resize_core::{parse_fit, plan_resize};

#[derive(Serialize)]
struct Plan {
    argv: Vec<String>,
    out_name: String,
}

/// `width`/`height` of 0 mean "unset" (auto). Returns `{ argv: string[], out_name }`
/// or throws a JS error string.
#[wasm_bindgen]
pub fn build_argv(width: f64, height: f64, fit: &str, in_name: &str) -> Result<JsValue, JsValue> {
    let w = if width > 0.0 { Some(width as u32) } else { None };
    let h = if height > 0.0 { Some(height as u32) } else { None };
    let fit = parse_fit(Some(fit)).map_err(|e| JsValue::from_str(&e))?;
    let (argv, out_name) = plan_resize(in_name, w, h, fit).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
