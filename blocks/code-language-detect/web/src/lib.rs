//! Browser-facing wasm-bindgen wrapper for /tools/code-language-detect/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    code: &str,
    filename: &str,
    candidates: &str,
    top_k: &str,
    common_only: &str,
    explain: &str,
    output: &str,
) -> Result<String, JsValue> {
    let defaults = gizza_ai_code_language_detect_core::Options::default();
    let opts = gizza_ai_code_language_detect_core::Options {
        filename: filename.to_string(),
        candidates: candidates.to_string(),
        common_only: truthy(common_only),
        top_k: if top_k.trim().is_empty() {
            defaults.top_k
        } else {
            top_k
                .trim()
                .parse::<usize>()
                .map_err(|_| JsValue::from_str("top_k must be an integer"))?
        },
        explain: if explain.trim().is_empty() {
            defaults.explain
        } else {
            truthy(explain)
        },
        output: if output.trim().is_empty() {
            defaults.output
        } else {
            output.to_string()
        },
    };
    gizza_ai_code_language_detect_core::detect_to_string(code, &opts)
        .map_err(|e| JsValue::from_str(&e))
}

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}
