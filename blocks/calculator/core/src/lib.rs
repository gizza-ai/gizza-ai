//! gizza-ai/calculator core — pure arithmetic evaluation shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.

/// Evaluate an arithmetic expression. Returns `Err` if meval fails to parse it,
/// or if the result is non-finite (NaN/Inf) — typically division by zero or
/// overflow.
pub fn evaluate(expr: &str) -> Result<f64, String> {
    let v = meval::eval_str(expr).map_err(|e| format!("eval failed: {e}"))?;
    if !v.is_finite() {
        return Err(format!("eval failed: non-finite result ({v})"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_simple_arithmetic() {
        assert_eq!(evaluate("2+2").unwrap(), 4.0);
        assert_eq!(evaluate("2+2*3").unwrap(), 8.0);
    }

    #[test]
    fn evaluates_named_functions() {
        assert_eq!(evaluate("sqrt(16)").unwrap(), 4.0);
        let v = evaluate("3.14 * 2^2").unwrap();
        assert!((v - 12.56).abs() < 1e-9, "got {v}");
    }

    #[test]
    fn rejects_non_finite_results() {
        let err = evaluate("1/0").unwrap_err();
        assert!(err.contains("non-finite"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_syntax() {
        let err = evaluate("nonsense === ===").unwrap_err();
        assert!(err.contains("eval failed"), "got: {err}");
    }
}
