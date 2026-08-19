//! Browser-facing wasm-bindgen wrapper for /tools/regression-model-trainer/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    target: &str,
    features: &str,
    model: &str,
    alpha: &str,
    standardize: &str,
    trees: &str,
    max_depth: &str,
    test_split: &str,
    cv_folds: &str,
    seed: &str,
    header: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let d = gizza_ai_regression_model_trainer_core::Options::default();
    let opts = gizza_ai_regression_model_trainer_core::Options {
        target: value_or(target, &d.target),
        features: features.to_string(),
        model: value_or(model, &d.model),
        alpha: parse_f(alpha, d.alpha)?,
        standardize: truthy_default(standardize, d.standardize),
        trees: parse_u(trees, d.trees)?,
        max_depth: parse_u(max_depth, d.max_depth)?,
        test_split: parse_f(test_split, d.test_split)?,
        cv_folds: parse_u(cv_folds, d.cv_folds)?,
        seed: parse_u64(seed, d.seed)?,
        header: value_or(header, &d.header),
        decimals: parse_u(decimals, d.decimals)?,
        format: value_or(format, &d.format),
    };
    gizza_ai_regression_model_trainer_core::run(data, &opts).map_err(|e| JsValue::from_str(&e))
}

fn value_or(v: &str, d: &str) -> String {
    if v.trim().is_empty() {
        d.to_string()
    } else {
        v.to_string()
    }
}
fn truthy_default(v: &str, d: bool) -> bool {
    if v.trim().is_empty() {
        d
    } else {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        )
    }
}
fn parse_f(v: &str, d: f64) -> Result<f64, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected a number"))
    }
}
fn parse_u(v: &str, d: u32) -> Result<u32, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected an integer"))
    }
}
fn parse_u64(v: &str, d: u64) -> Result<u64, JsValue> {
    if v.trim().is_empty() {
        Ok(d)
    } else {
        v.trim()
            .parse()
            .map_err(|_| JsValue::from_str("expected an integer"))
    }
}
