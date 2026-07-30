//! Browser-facing wasm-bindgen wrapper for /tools/molecular-weight-calculator/.
//! Compiled with wasm-pack for the standalone page.
use wasm_bindgen::prelude::*;

/// Analyze a molecular formula from the generated tool page. The page passes
/// every field as a string, so `decimals` is parsed here and falls back to the
/// core default when left blank.
#[wasm_bindgen]
pub fn run(formula: &str, decimals: &str) -> Result<String, JsValue> {
    let decimals = if decimals.trim().is_empty() {
        gizza_ai_molecular_weight_calculator_core::DEFAULT_DECIMALS
    } else {
        decimals
            .trim()
            .parse::<u32>()
            .map_err(|_| JsValue::from_str("decimals must be a whole number from 0 to 10"))?
    };
    gizza_ai_molecular_weight_calculator_core::analyze_json(formula, decimals)
        .map_err(|e| JsValue::from_str(&e))
}
