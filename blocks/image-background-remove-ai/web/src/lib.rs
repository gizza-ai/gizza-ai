//! Minimal wasm-bindgen marker for the model-backed standalone page.
//!
//! Inference lives in the shared `tool-model.js` worker runtime. Keeping this
//! tiny web crate preserves the existing per-page build contract used by CI.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn model_page() {}
