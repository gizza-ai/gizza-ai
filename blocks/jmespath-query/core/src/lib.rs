//! gizza-ai/jmespath-query core — evaluate a JMESPath expression against a JSON
//! document using the `jmespath` crate (the reference Rust implementation, pure
//! Rust with no I/O or clock use), so it runs in the gizza wafer runtime (chat SW),
//! the CLI, and the browser page alike.
//!
//! JMESPath is the query language behind the AWS CLI's `--query` flag. Unlike
//! JSONPath or jq, an expression always evaluates to exactly ONE JSON value
//! (`null` when nothing matches), so the result is a single serialized string.

use jmespath::Variable;
use serde_json::Value;

/// Evaluate `expression` (a JMESPath expression) against the JSON in `input`.
///
/// - `pretty`: indent the serialized JSON result (2 spaces).
/// - `raw`: emit strings unquoted, the way `aws --output text` / `jq -r` do. A
///   top-level array is emitted one element per line, with string elements
///   unquoted and non-string elements serialized as JSON. Non-string, non-array
///   results are serialized as JSON exactly as they would be without `raw`.
///
/// Errors on an empty expression, invalid JSON input, an expression that fails to
/// compile, or a runtime evaluation error (e.g. a built-in called on the wrong type).
pub fn run_jmespath(
    expression: &str,
    input: &str,
    pretty: bool,
    raw: bool,
) -> Result<String, String> {
    if expression.trim().is_empty() {
        return Err(
            "JMESPath expression is empty: enter an expression such as 'people[*].name'".into(),
        );
    }
    if input.trim().is_empty() {
        return Err("JSON input is empty: paste the JSON document to query".into());
    }

    let data: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid JSON input: {e}"))?;
    let expr =
        jmespath::compile(expression).map_err(|e| format!("invalid JMESPath expression: {e}"))?;
    let found = expr
        .search(&data)
        .map_err(|e| format!("JMESPath evaluation error: {e}"))?;

    // The engine returns its own `Variable` tree; go back through serde_json so the
    // output formatting (indent, raw-string handling) is shared with the rest of
    // the JSON tool family.
    let result: Value = variable_to_json(&found)?;

    if raw {
        return Ok(render_raw(&result, pretty));
    }
    serialize(&result, pretty)
}

/// Convert the engine's `Variable` result into a `serde_json::Value`.
fn variable_to_json(var: &Variable) -> Result<Value, String> {
    serde_json::to_value(var).map_err(|e| format!("serialize error: {e}"))
}

/// `raw` rendering: unquote string scalars, and flatten a top-level array to one
/// element per line so string lists paste straight into a shell.
fn render_raw(value: &Value, pretty: bool) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(s) => s.clone(),
                other => serialize(other, pretty).unwrap_or_else(|e| e),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        other => serialize(other, pretty).unwrap_or_else(|e| e),
    }
}

fn serialize(value: &Value, pretty: bool) -> Result<String, String> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
    .map_err(|e| format!("serialize error: {e}"))
}

/// The JSON type name of a result, so callers (chat/CLI) can describe the shape
/// without re-parsing the serialized string.
pub fn json_kind(serialized: &str) -> &'static str {
    match serde_json::from_str::<Value>(serialized) {
        Ok(Value::Null) => "null",
        Ok(Value::Bool(_)) => "boolean",
        Ok(Value::Number(_)) => "number",
        Ok(Value::String(_)) => "string",
        Ok(Value::Array(_)) => "array",
        Ok(Value::Object(_)) => "object",
        // `raw` output is deliberately not valid JSON (unquoted strings).
        Err(_) => "string",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEOPLE: &str = r#"{
        "people": [
            {"name": "Alice", "age": 34, "state": "WA", "skills": ["rust", "go"]},
            {"name": "Bob",   "age": 25, "state": "OR", "skills": ["python"]},
            {"name": "Carol", "age": 41, "state": "WA", "skills": ["rust"]}
        ],
        "company": {"name": "Initech", "locations": ["Seattle", "Portland"]}
    }"#;

    #[test]
    fn projects_a_field_from_every_element() {
        let out = run_jmespath("people[*].name", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#"["Alice","Bob","Carol"]"#);
    }

    #[test]
    fn filters_with_a_comparison() {
        let out = run_jmespath("people[?age > `30`].name", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#"["Alice","Carol"]"#);
    }

    #[test]
    fn filters_on_a_string_literal() {
        let out = run_jmespath("people[?state == 'WA'].name | [0]", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#""Alice""#);
    }

    #[test]
    fn builds_a_multiselect_hash() {
        let out = run_jmespath("people[0].{who: name, howOld: age}", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#"{"howOld":34,"who":"Alice"}"#);
    }

    #[test]
    fn runs_built_in_functions() {
        assert_eq!(
            run_jmespath("length(people)", PEOPLE, false, false).unwrap(),
            "3"
        );
        assert_eq!(
            run_jmespath("max_by(people, &age).name", PEOPLE, false, false).unwrap(),
            r#""Carol""#
        );
        assert_eq!(
            run_jmespath("sort_by(people, &age)[0].name", PEOPLE, false, false).unwrap(),
            r#""Bob""#
        );
        assert_eq!(
            run_jmespath("join(', ', company.locations)", PEOPLE, false, false).unwrap(),
            r#""Seattle, Portland""#
        );
    }

    #[test]
    fn flattens_nested_arrays() {
        let out = run_jmespath("people[].skills[]", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#"["rust","go","python","rust"]"#);
    }

    #[test]
    fn slices_arrays() {
        let out = run_jmespath("company.locations[:1]", PEOPLE, false, false).unwrap();
        assert_eq!(out, r#"["Seattle"]"#);
    }

    #[test]
    fn no_match_is_json_null_not_an_error() {
        assert_eq!(
            run_jmespath("missing.key", PEOPLE, false, false).unwrap(),
            "null"
        );
    }

    #[test]
    fn pretty_indents_the_result() {
        let out = run_jmespath("company", PEOPLE, true, false).unwrap();
        assert_eq!(
            out,
            "{\n  \"locations\": [\n    \"Seattle\",\n    \"Portland\"\n  ],\n  \"name\": \"Initech\"\n}"
        );
    }

    #[test]
    fn raw_unquotes_a_string_result() {
        let out = run_jmespath("company.name", PEOPLE, false, true).unwrap();
        assert_eq!(out, "Initech");
    }

    #[test]
    fn raw_prints_one_array_element_per_line() {
        let out = run_jmespath("people[*].name", PEOPLE, false, true).unwrap();
        assert_eq!(out, "Alice\nBob\nCarol");
    }

    #[test]
    fn raw_leaves_non_strings_as_json() {
        assert_eq!(
            run_jmespath("length(people)", PEOPLE, false, true).unwrap(),
            "3"
        );
        assert_eq!(
            run_jmespath("people[*].age", PEOPLE, false, true).unwrap(),
            "34\n25\n41"
        );
        assert_eq!(
            run_jmespath("company", PEOPLE, false, true).unwrap(),
            r#"{"locations":["Seattle","Portland"],"name":"Initech"}"#
        );
    }

    #[test]
    fn reports_the_json_kind() {
        assert_eq!(json_kind("null"), "null");
        assert_eq!(json_kind("3"), "number");
        assert_eq!(json_kind(r#""x""#), "string");
        assert_eq!(json_kind("[1]"), "array");
        assert_eq!(json_kind("{}"), "object");
        assert_eq!(json_kind("true"), "boolean");
        assert_eq!(json_kind("Initech"), "string");
    }

    #[test]
    fn errors_on_empty_expression() {
        let e = run_jmespath("   ", PEOPLE, false, false).unwrap_err();
        assert!(e.contains("expression is empty"), "{e}");
    }

    #[test]
    fn errors_on_empty_input() {
        let e = run_jmespath("people", "  ", false, false).unwrap_err();
        assert!(e.contains("JSON input is empty"), "{e}");
    }

    #[test]
    fn errors_on_invalid_json_input() {
        let e = run_jmespath("people", "{not json}", false, false).unwrap_err();
        assert!(e.starts_with("invalid JSON input:"), "{e}");
    }

    #[test]
    fn errors_on_invalid_expression() {
        let e = run_jmespath("people[?", PEOPLE, false, false).unwrap_err();
        assert!(e.starts_with("invalid JMESPath expression:"), "{e}");
    }

    #[test]
    fn errors_on_a_type_error_at_runtime() {
        let e = run_jmespath("length(people[0].age)", PEOPLE, false, false).unwrap_err();
        assert!(e.starts_with("JMESPath evaluation error:"), "{e}");
    }
}
