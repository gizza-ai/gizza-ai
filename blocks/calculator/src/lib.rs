//! gizza-ai/calculator — evaluates math expressions via meval.
//!
//! Pure-rust skill block. Takes `{ "expr": "..." }`, returns either
//! `{ "result": <number> }` on success or `{ "error": "..." }` on
//! parse/eval failure. No host calls — runs entirely inside the WASM
//! sandbox.

// The #[wafer_block] macro emits wasm-only registration code; supporting
// imports + the Args type are only used inside that impl. See image-resize
// for the rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    expr: String,
}

/// Evaluate an arithmetic expression. Returns `Err` if meval fails to parse
/// it, or if the result is non-finite (NaN/Inf) — typically division by
/// zero or overflow.
fn evaluate_expr(expr: &str) -> Result<f64, String> {
    let v = meval::eval_str(expr).map_err(|e| format!("eval failed: {e}"))?;
    if !v.is_finite() {
        return Err(format!("eval failed: non-finite result ({v})"));
    }
    Ok(v)
}

#[cfg(target_arch = "wasm32")]
struct Calculator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Calculator skill",
    skill(
        description = "Evaluate an arithmetic expression (e.g. '2+2*3'). Returns the numeric result.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "expr": { "type": "string", "description": "Arithmetic expression to evaluate (e.g. '2+2*3', 'sqrt(16)', '3.14 * 2^2')." }
            },
            "required": ["expr"],
            "additionalProperties": false
        }"#
    ),
)]
impl Calculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match evaluate_expr(&args.expr) {
            Ok(v) => {
                let body = serde_json::json!({ "result": v });
                GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
            }
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    let body = serde_json::json!({ "error": msg });
    GuestResult::respond(serde_json::to_vec(&body).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_simple_arithmetic() {
        assert_eq!(evaluate_expr("2+2").unwrap(), 4.0);
        assert_eq!(evaluate_expr("2+2*3").unwrap(), 8.0);
    }

    #[test]
    fn evaluates_named_functions() {
        assert_eq!(evaluate_expr("sqrt(16)").unwrap(), 4.0);
        // 3.14 * 2^2 — meval uses ^ for power.
        let v = evaluate_expr("3.14 * 2^2").unwrap();
        assert!((v - 12.56).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn rejects_non_finite_results() {
        // Division by zero yields Inf in IEEE 754; the explicit non-finite
        // check is what protects users from `"result": null` in the JSON.
        let err = evaluate_expr("1/0").unwrap_err();
        assert!(err.contains("non-finite"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_syntax() {
        let err = evaluate_expr("nonsense === ===").unwrap_err();
        assert!(err.contains("eval failed"), "got: {err}");
    }
}
