//! Browser-facing wasm-bindgen wrapper for /tools/svm-classifier/.
use gizza_ai_svm_classifier_core::Options;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(
    data: &str,
    target: &str,
    features: &str,
    kernel: &str,
    c: &str,
    gamma: &str,
    degree: &str,
    coef0: &str,
    scaling: &str,
    class_weight: &str,
    multiclass: &str,
    tol: &str,
    max_iter: &str,
    cv_folds: &str,
    test_split: &str,
    seed: &str,
    predict: &str,
    header: &str,
    decimals: &str,
    format: &str,
) -> Result<String, JsValue> {
    let defaults = Options::default();
    let opts = Options {
        target: if target.trim().is_empty() {
            defaults.target
        } else {
            target.into()
        },
        features: features.into(),
        kernel: if kernel.trim().is_empty() {
            defaults.kernel
        } else {
            kernel.into()
        },
        c: c.trim().parse().unwrap_or(defaults.c),
        gamma: if gamma.trim().is_empty() {
            defaults.gamma
        } else {
            gamma.into()
        },
        degree: degree.trim().parse().unwrap_or(defaults.degree),
        coef0: coef0.trim().parse().unwrap_or(defaults.coef0),
        scaling: if scaling.trim().is_empty() {
            defaults.scaling
        } else {
            scaling.into()
        },
        class_weight: if class_weight.trim().is_empty() {
            defaults.class_weight
        } else {
            class_weight.into()
        },
        multiclass: if multiclass.trim().is_empty() {
            defaults.multiclass
        } else {
            multiclass.into()
        },
        tol: tol.trim().parse().unwrap_or(defaults.tol),
        max_iter: max_iter.trim().parse().unwrap_or(defaults.max_iter),
        cv_folds: cv_folds.trim().parse().unwrap_or(defaults.cv_folds),
        test_split: test_split.trim().parse().unwrap_or(defaults.test_split),
        seed: seed.trim().parse().unwrap_or(defaults.seed),
        predict: predict.into(),
        header: if header.trim().is_empty() {
            defaults.header
        } else {
            header.into()
        },
        decimals: decimals.trim().parse().unwrap_or(defaults.decimals),
        format: if format.trim().is_empty() {
            defaults.format
        } else {
            format.into()
        },
    };
    gizza_ai_svm_classifier_core::run(data, &opts).map_err(|e| JsValue::from_str(&e))
}
