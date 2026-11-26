//! Minimal wasm-bindgen marker for the model-backed standalone page.
//!
//! Generation lives in the shared `tool-model.js` worker runtime (the page's
//! pinned Janus-Pro model, `text-to-image` task). Keeping this tiny web crate
//! preserves the per-page build contract used by CI.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn model_page() {}
