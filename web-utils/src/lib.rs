//! Shared wasm-bindgen glue for tool web crates (`blocks/<slug>/web`), kept out
//! of `block-utils` so the chat/CLI blocks never pull in wasm-bindgen.
//!
//! The `wasm-image` page runtime calls a web crate's
//! `run(params: object, input: Uint8Array) -> { bytes: Uint8Array, out_name }`.
//! `params` holds the page's field values keyed by param name, with blank fields
//! omitted, so it deserializes into the same `Params` struct (serde defaults
//! included) that the block's chat handler flattens from its JSON args.

use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// Deserialize the page's params object into the block core's `Params`.
pub fn params_from_js<T: DeserializeOwned>(params: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(params).map_err(|e| JsValue::from_str(&format!("invalid options: {e}")))
}

/// Build the `wasm-image` result object `{ bytes: Uint8Array, out_name }`.
pub fn image_output(bytes: &[u8], out_name: &str) -> Result<JsValue, JsValue> {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"bytes".into(), &js_sys::Uint8Array::from(bytes).into())?;
    js_sys::Reflect::set(&obj, &"out_name".into(), &out_name.into())?;
    Ok(obj.into())
}

/// Map a core error string to the JS error the page shows.
pub fn js_err(e: String) -> JsValue {
    JsValue::from_str(&e)
}
