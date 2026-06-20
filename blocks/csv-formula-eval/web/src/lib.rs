//! Browser-facing wasm-bindgen wrapper for /tools/csv-formula-eval/.
use gizza_ai_csv_formula_eval_core::eval;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(data: &str, formulas: &str, delimiter: &str) -> Result<String, JsValue> {
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    eval(data, formulas, delim).map_err(|e| JsValue::from_str(&e))
}
