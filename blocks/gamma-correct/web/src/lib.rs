//! Browser-facing wasm-bindgen wrapper for the standalone /tools/gamma-correct/
//! page. Builds the ffmpeg argv (pure, shared with the chat block via core); the
//! JS page driver runs it through the browser ffmpeg bridge.
//!
//! The page passes the field values then the uploaded file's `in_name` (field
//! order in page/meta.toml MUST equal this param order). Numeric schema params
//! are coerced to `Number` by the page; the `format` field stays a string. The
//! shared `plan_named()` validates ranges and rejects out-of-range values with a
//! clear error.
use gizza_ai_block_utils::ArgvPlan;
use gizza_ai_gamma_correct_core::plan_named;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(
    gamma: f64,
    gamma_r: f64,
    gamma_g: f64,
    gamma_b: f64,
    gamma_weight: f64,
    format: &str,
    in_name: &str,
) -> Result<JsValue, JsValue> {
    let (argv, out_name) = plan_named(
        in_name,
        gamma,
        gamma_r,
        gamma_g,
        gamma_b,
        gamma_weight,
        Some(format),
    )
    .map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name })
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
