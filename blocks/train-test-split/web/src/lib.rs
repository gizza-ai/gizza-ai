//! Browser-facing wasm-bindgen wrapper for /tools/train-test-split/.
//! Page fields arrive as strings (numbers coerced to f64 by the wasm ABI), so
//! this layer normalizes them before handing off to the shared core.
use wasm_bindgen::prelude::*;

/// Checkboxes send "true"/"false" — parse positive-truthy.
fn flag(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

/// A blank/NaN numeric field falls back to the schema default.
fn size(v: f64, default: f64, name: &str) -> Result<f64, String> {
    if !v.is_finite() {
        return Ok(default);
    }
    if v < 0.0 {
        return Err(format!("{name} must be 0 or more (got {v})"));
    }
    Ok(v)
}

/// A whole-number field (bins, seed) with an inclusive upper bound.
fn whole(v: f64, default: u64, name: &str, max: u64) -> Result<u64, String> {
    if !v.is_finite() {
        return Ok(default);
    }
    if v < 0.0 || v.fract() != 0.0 {
        return Err(format!("{name} must be a whole number 0 or more (got {v})"));
    }
    let n = v as u64;
    if n > max {
        return Err(format!("{name} must be at most {max} (got {n})"));
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    test_size: f64,
    validation_size: f64,
    stratify_column: &str,
    stratify_bins: f64,
    group_column: &str,
    shuffle: &str,
    seed: f64,
    header: &str,
    delimiter: &str,
    output: &str,
) -> Result<String, JsValue> {
    let test_size = size(test_size, 0.2, "test_size").map_err(|e| JsValue::from_str(&e))?;
    let validation_size =
        size(validation_size, 0.0, "validation_size").map_err(|e| JsValue::from_str(&e))?;
    let stratify_bins =
        whole(stratify_bins, 0, "stratify_bins", 100).map_err(|e| JsValue::from_str(&e))?;
    let seed = whole(seed, 42, "seed", u64::MAX).map_err(|e| JsValue::from_str(&e))?;
    gizza_ai_train_test_split_core::split(
        data,
        test_size,
        validation_size,
        stratify_column,
        stratify_bins as usize,
        group_column,
        flag(shuffle),
        seed,
        flag(header),
        if delimiter.trim().is_empty() {
            "comma"
        } else {
            delimiter
        },
        if output.trim().is_empty() {
            "sections"
        } else {
            output
        },
    )
    .map_err(|e| JsValue::from_str(&e))
}
