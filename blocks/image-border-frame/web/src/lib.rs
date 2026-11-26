//! Browser-facing wasm-bindgen wrapper for the standalone
//! /tools/image-border-frame/ page (`wasm-image` runtime): the same core
//! `apply` + `Params` the chat block uses, run in the page's image worker.

use wasm_bindgen::prelude::*;

use gizza_ai_image_border_frame_core::{apply, Params};
use gizza_ai_web_utils::{image_output, js_err, params_from_js};

/// `params`: `{ thickness?, color? }`; `input`: the encoded image.
/// Returns `{ bytes: Uint8Array, out_name }` or throws a JS error string.
#[wasm_bindgen]
pub fn run(params: JsValue, input: &[u8]) -> Result<JsValue, JsValue> {
    let params: Params = params_from_js(params)?;
    let (png, out_name) = apply(input, &params).map_err(js_err)?;
    image_output(&png, &out_name)
}
