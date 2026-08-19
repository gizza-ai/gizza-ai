//! Browser-facing wasm-bindgen wrapper for /tools/correlated-sample-generator/.
//! Field order MUST match page/meta.toml: covariance, matrix_kind, sd, mean,
//! samples, method, seed, empirical, output, decimals, labels, header, tol.
//! The page driver passes every field as a raw string, so numbers and booleans
//! are parsed here and all validation stays in the shared core.
use gizza_ai_correlated_sample_generator_core::generate;
use wasm_bindgen::prelude::*;

fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "yes" | "on" => true,
        "false" | "0" | "no" | "off" => false,
        _ => default,
    }
}

fn parse_usize(s: &str, field: &str, default: usize) -> Result<usize, JsValue> {
    match s.trim() {
        "" => Ok(default),
        n => n
            .parse::<usize>()
            .map_err(|_| JsValue::from_str(&format!("{field} '{n}' is not a whole number"))),
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    covariance: &str,
    matrix_kind: &str,
    sd: &str,
    mean: &str,
    samples: &str,
    method: &str,
    seed: &str,
    empirical: &str,
    output: &str,
    decimals: &str,
    labels: &str,
    header: &str,
    tol: &str,
) -> Result<String, JsValue> {
    let n = parse_usize(samples, "samples", 100)?;
    let dec = parse_usize(decimals, "decimals", 4)?;
    let s = match seed.trim() {
        "" => 42u64,
        v => v
            .parse::<u64>()
            .map_err(|_| JsValue::from_str(&format!("seed '{v}' is not a whole number 0 or above")))?,
    };
    let t = match tol.trim() {
        "" => 1e-8,
        v => v
            .parse::<f64>()
            .map_err(|_| JsValue::from_str(&format!("tol '{v}' is not a number")))?,
    };
    generate(
        covariance,
        mean,
        n,
        matrix_kind,
        sd,
        method,
        s,
        parse_bool(empirical, false),
        t,
        output,
        dec,
        labels,
        parse_bool(header, true),
    )
    .map_err(|e| JsValue::from_str(&e))
}
