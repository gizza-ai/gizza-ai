//! gizza-ai/jsonata-query core — evaluate a JSONata query/transform expression
//! against a JSON document using `jsonata-rs`, a pure-Rust JSONata engine. No
//! wafer/wasm-bindgen deps and no WASI/host deps, so it runs in the gizza wafer
//! runtime (chat SW), the CLI, and the browser page alike.
//!
//! JSONata is a lightweight query and transformation language for JSON (it can
//! navigate, filter, aggregate, and reshape data — e.g. `Account.Order.Product.Price`,
//! `$sum(items.price)`, `items[price > 10].name`, `{ "total": $sum(lines.amount) }`).
//! The result is serialized back to a single JSON string; `pretty` indents it.

use bumpalo::Bump;
use jsonata_rs::JsonAta;

/// Maximum recursive evaluator depth for a single JSONata run.
const MAX_EVAL_DEPTH: usize = 1_024;

/// Native-target evaluation timeout in milliseconds.
///
/// On wasm32 targets this is intentionally best-effort only: the vendored
/// JSONata engine skips `Instant::now()` because it traps in
/// wasm32-unknown-unknown, so `MAX_EVAL_DEPTH` is the portable runaway guard.
const EVAL_TIMEOUT_MS: usize = 1_000;

/// Evaluate `expr` (a JSONata expression) against the JSON in `input`.
///
/// - `pretty`: indent the serialized JSON result.
///
/// Returns the JSON result serialized as a string. A JSONata expression that
/// matches nothing yields an "undefined" result, which we normalize to JSON `null`.
///
/// Errors on an invalid JSONata expression, invalid JSON input, or an evaluation
/// error (e.g. a type error in a built-in function).
pub fn run_jsonata(expr: &str, input: &str, pretty: bool) -> Result<String, String> {
    if expr.trim().is_empty() {
        return Err("JSONata expression is empty".into());
    }
    // Validate the input is JSON up front for a clear error message (the engine
    // parses it again internally, but its message is less specific).
    serde_json::from_str::<serde_json::Value>(input)
        .map_err(|e| format!("invalid JSON input: {e}"))?;

    let arena = Bump::new();
    let jsonata = JsonAta::new(expr, &arena).map_err(|e| format!("invalid JSONata expression: {e}"))?;
    let result = jsonata
        .evaluate_timeboxed(Some(input), Some(MAX_EVAL_DEPTH), Some(EVAL_TIMEOUT_MS))
        .map_err(|e| format!("JSONata evaluation error: {e}"))?;

    let out = result.serialize(pretty);
    // The engine serializes an "undefined" (no match) result as an empty string;
    // normalize that to a valid JSON null so the output is always valid JSON.
    if out.is_empty() {
        Ok("null".to_string())
    } else {
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_concat() {
        assert_eq!(
            run_jsonata(r#""Hello, " & name & "!""#, r#"{"name":"world"}"#, false).unwrap(),
            r#""Hello, world!""#
        );
    }

    #[test]
    fn path_navigation() {
        assert_eq!(
            run_jsonata(
                "Account.Order.Product.Price",
                r#"{"Account":{"Order":{"Product":{"Price":34.5}}}}"#,
                false
            )
            .unwrap(),
            "34.5"
        );
    }

    #[test]
    fn aggregate_sum() {
        assert_eq!(
            run_jsonata(
                "$sum(items.price)",
                r#"{"items":[{"price":2},{"price":3},{"price":5}]}"#,
                false
            )
            .unwrap(),
            "10"
        );
    }

    #[test]
    fn predicate_filter_and_field() {
        // Keep items priced over 10, project their name.
        let out = run_jsonata(
            "items[price > 10].name",
            r#"{"items":[{"name":"a","price":5},{"name":"b","price":20},{"name":"c","price":30}]}"#,
            false,
        )
        .unwrap();
        assert_eq!(out, r#"["b","c"]"#);
    }

    #[test]
    fn object_construction() {
        let out = run_jsonata(
            r#"{ "total": $sum(lines.amount), "count": $count(lines) }"#,
            r#"{"lines":[{"amount":4},{"amount":6}]}"#,
            false,
        )
        .unwrap();
        // Object key order in the serialized output is not guaranteed, so parse
        // the result and assert on the field values rather than the exact string.
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["total"], serde_json::json!(10));
        assert_eq!(parsed["count"], serde_json::json!(2));
    }

    #[test]
    fn pretty_print() {
        let out = run_jsonata("$", r#"{"a":1}"#, true).unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn no_match_yields_null() {
        assert_eq!(run_jsonata("missing.field", r#"{"a":1}"#, false).unwrap(), "null");
    }

    #[test]
    fn errors() {
        assert!(run_jsonata("", "1", false).is_err()); // empty expression
        assert!(run_jsonata("$", "{not json}", false).is_err()); // bad JSON input
        assert!(run_jsonata("a +", "1", false).is_err()); // expression parse error
    }
}
