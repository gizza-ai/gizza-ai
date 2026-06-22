//! gizza-ai/jsonpath-query core — evaluate a JSONPath expression (RFC 9535) against
//! a JSON document using `serde_json_path`, a pure-Rust, RFC 9535-compliant engine.
//! No wafer/wasm-bindgen deps and no WASI/host deps, so it runs in the gizza wafer
//! runtime (chat SW), the CLI, and the browser page alike.
//!
//! A JSONPath query selects zero, one, or many nodes from the document. Each matched
//! value is returned as a serialized JSON string (one per match). `wrap` instead
//! returns the whole result list as a single JSON array string.

use serde_json::Value;
use serde_json_path::JsonPath;

/// Evaluate `path` (a JSONPath / RFC 9535 expression) against the JSON in `input`.
///
/// - `pretty`: indent each emitted JSON value.
/// - `wrap`: return a single output that is a JSON array of all matched values
///   (the canonical "result list"); otherwise return one output string per match.
///
/// Errors on invalid JSON input or an invalid JSONPath expression.
pub fn query_jsonpath(path: &str, input: &str, pretty: bool, wrap: bool) -> Result<Vec<String>, String> {
    if path.trim().is_empty() {
        return Err("JSONPath expression is empty".into());
    }
    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON input: {e}"))?;
    let jp = JsonPath::parse(path).map_err(|e| format!("invalid JSONPath: {e}"))?;

    let nodes = jp.query(&value);

    let to_str = |v: &Value| -> Result<String, String> {
        if pretty {
            serde_json::to_string_pretty(v)
        } else {
            serde_json::to_string(v)
        }
        .map_err(|e| format!("serialize error: {e}"))
    };

    if wrap {
        let list = Value::Array(nodes.iter().map(|v| (*v).clone()).collect());
        Ok(vec![to_str(&list)?])
    } else {
        let mut out = Vec::with_capacity(nodes.len());
        for v in nodes.iter() {
            out.push(to_str(v)?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STORE: &str = r#"{
        "store": {
            "book": [
                {"author": "Nigel Rees", "price": 8.95},
                {"author": "Evelyn Waugh", "price": 12.99},
                {"author": "Herman Melville", "price": 8.99}
            ],
            "bicycle": {"color": "red", "price": 19.95}
        }
    }"#;

    #[test]
    fn root_identity() {
        let out = query_jsonpath("$", "42", false, false).unwrap();
        assert_eq!(out, vec!["42"]);
    }

    #[test]
    fn field_access() {
        let out = query_jsonpath("$.name", r#"{"name":"Ada","age":36}"#, false, false).unwrap();
        assert_eq!(out, vec![r#""Ada""#]);
    }

    #[test]
    fn wildcard_over_array() {
        let out = query_jsonpath("$.store.book[*].author", STORE, false, false).unwrap();
        assert_eq!(out, vec![r#""Nigel Rees""#, r#""Evelyn Waugh""#, r#""Herman Melville""#]);
    }

    #[test]
    fn array_index() {
        let out = query_jsonpath("$.store.book[0].price", STORE, false, false).unwrap();
        assert_eq!(out, vec!["8.95"]);
    }

    #[test]
    fn filter_expression() {
        let out = query_jsonpath("$.store.book[?(@.price < 10)].author", STORE, false, false).unwrap();
        assert_eq!(out, vec![r#""Nigel Rees""#, r#""Herman Melville""#]);
    }

    #[test]
    fn recursive_descent() {
        let out = query_jsonpath("$..price", STORE, false, false).unwrap();
        // all price nodes (3 books + bicycle)
        assert_eq!(out.len(), 4);
        assert!(out.contains(&"19.95".to_string()));
    }

    #[test]
    fn no_match_is_empty() {
        let out = query_jsonpath("$.store.car", STORE, false, false).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn wrap_returns_single_array() {
        let out = query_jsonpath("$.store.book[*].author", STORE, false, true).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0], r#"["Nigel Rees","Evelyn Waugh","Herman Melville"]"#);
    }

    #[test]
    fn wrap_empty_is_empty_array() {
        let out = query_jsonpath("$.nope", STORE, false, true).unwrap();
        assert_eq!(out, vec!["[]"]);
    }

    #[test]
    fn pretty_print() {
        let out = query_jsonpath("$.store.bicycle", STORE, true, false).unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].contains('\n'));
        assert!(out[0].contains("\"color\""));
    }

    #[test]
    fn errors() {
        assert!(query_jsonpath("$", "{not json}", false, false).is_err()); // bad JSON
        assert!(query_jsonpath("", "1", false, false).is_err()); // empty path
        assert!(query_jsonpath("$.[", "1", false, false).is_err()); // invalid path
    }
}
