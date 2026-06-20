//! Browser-facing wasm-bindgen wrapper for the standalone /tools/change-speed/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core).
//! The page passes the `factor` field then the uploaded file's `in_name`
//! (field order in page/meta.toml MUST equal this param order).
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_change_speed_core::plan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(factor: f64, in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan(in_name, factor).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
