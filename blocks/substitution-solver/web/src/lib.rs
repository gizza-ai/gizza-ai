//! Browser-facing wasm-bindgen wrapper for /tools/substitution-solver/.
//! Field order MUST match page/meta.toml: text, mode, key, cribs, effort, keep_layout.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

#[wasm_bindgen]
pub fn run(
    text: &str,
    mode: &str,
    key: &str,
    cribs: &str,
    effort: &str,
    keep_layout: &str,
) -> Result<String, JsValue> {
    let m = if mode.trim().is_empty() {
        "solve"
    } else {
        mode
    };
    let e = if effort.trim().is_empty() {
        "standard"
    } else {
        effort
    };
    gizza_ai_substitution_solver_core::run(text, m, key, cribs, e, truthy(keep_layout))
        .map_err(|err| JsValue::from_str(&err))
}
