//! Browser-facing wasm-bindgen wrapper for /tools/missing-value-imputer/.
//! Field order MUST match meta.toml: input, strategy, header, delimiter,
//! columns, na_tokens, fill_value, n_neighbors, weights. Fields arrive as
//! strings (checkboxes send "true"/"false").
use gizza_ai_missing_value_imputer_core::impute;
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

fn parse_neighbors(s: &str) -> Result<usize, JsValue> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(5);
    }
    t.parse::<usize>()
        .map(|n| n.max(1))
        .map_err(|_| JsValue::from_str("n_neighbors must be a whole number"))
}

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    strategy: &str,
    header: &str,
    delimiter: &str,
    columns: &str,
    na_tokens: &str,
    fill_value: &str,
    n_neighbors: &str,
    weights: &str,
) -> Result<String, JsValue> {
    let strat = if strategy.is_empty() { "mean" } else { strategy };
    let delim = if delimiter.is_empty() { "comma" } else { delimiter };
    let w = if weights.is_empty() { "uniform" } else { weights };
    let k = parse_neighbors(n_neighbors)?;
    impute(input, truthy(header), delim, strat, columns, na_tokens, fill_value, k, w)
        .map_err(|e| JsValue::from_str(&e))
}
