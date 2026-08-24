//! Browser-facing wasm-bindgen wrapper for /tools/random-projection-reducer/.
//! Field order MUST match meta.toml: data, components, method, density, eps, seed, format.
use wasm_bindgen::prelude::*;

/// Parse an optional numeric field, falling back to `fallback` when it is blank.
fn num(raw: &str, name: &str, fallback: f64) -> Result<f64, JsValue> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(fallback);
    }
    t.parse::<f64>()
        .map_err(|_| JsValue::from_str(&format!("{name} must be a number, got '{t}'")))
}

#[wasm_bindgen]
pub fn run(
    data: &str,
    components: &str,
    method: &str,
    density: &str,
    eps: &str,
    seed: &str,
    format: &str,
) -> Result<String, JsValue> {
    let k = if components.trim().is_empty() {
        "auto"
    } else {
        components
    };
    let method = if method.trim().is_empty() {
        "gaussian"
    } else {
        method
    };
    let density = num(density, "density", 0.0)?;
    let eps = num(eps, "eps", 0.1)?;
    let seed = num(seed, "seed", 42.0)?;
    let format = if format.trim().is_empty() {
        "text"
    } else {
        format
    };
    gizza_ai_random_projection_reducer_core::run(data, k, method, density, eps, seed, format)
        .map_err(|e| JsValue::from_str(&e))
}
