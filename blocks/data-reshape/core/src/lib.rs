//! gizza-ai/data-reshape core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps, so it runs in the gizza wafer runtime
//! (chat SW), the CLI, and the browser page alike.
//!
//! Parses a document in one of four common formats (JSON / YAML / CSV / XML) into
//! a common in-memory value, then reshapes it with a **JSONata-style** expression
//! — a single language that both queries (navigate/filter/aggregate) and templates
//! (construct new objects/arrays) — and serializes the result back to JSON or
//! YAML. This unifies "convert format → query → template" into one step across
//! all four input formats.
//!
//! The reshape is done by a small, self-contained evaluator (see [`expr`]) rather
//! than a general JSONata engine. It intentionally implements the practical subset
//! the tool advertises — path navigation, predicate filters, the `$sum`/`$count`/
//! `$min`/`$max`/`$average` aggregates, object/array construction, `&` string
//! concatenation and basic arithmetic/comparison — with no dependency on a
//! monotonic clock or RNG, so it runs identically on native, WASI, and the browser
//! (`wasm32-unknown-unknown`) targets. General JSONata engines such as `jsonata-rs`
//! call `std::time::Instant::now()` while evaluating, which traps (`unreachable`)
//! in the browser; keeping our own evaluator avoids that entirely.

use serde_json::Value;

mod expr;

/// The document format fed in. `Auto` sniffs the content (see [`detect_format`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    Auto,
    Json,
    Yaml,
    Csv,
    Xml,
}

impl InputFormat {
    /// Parse the `input_format` param. Empty/`auto` → auto-detect.
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "json" => Ok(InputFormat::Json),
            "yaml" | "yml" => Ok(InputFormat::Yaml),
            "csv" => Ok(InputFormat::Csv),
            "xml" => Ok(InputFormat::Xml),
            other => Err(format!(
                "unknown input format '{other}' (expected auto, json, yaml, csv, or xml)"
            )),
        }
    }
}

/// The serialized output format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Json,
    Yaml,
}

impl OutputFormat {
    /// Parse the `output_format` param. Empty/`json` → JSON.
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutputFormat::Json),
            "yaml" | "yml" => Ok(OutputFormat::Yaml),
            other => Err(format!(
                "unknown output format '{other}' (expected json or yaml)"
            )),
        }
    }
}

/// Sniff the input format from the first meaningful character. Predictable and
/// documented: `<` → XML, `{`/`[` → JSON, a first line with a comma → CSV,
/// anything else → YAML (which is also a JSON superset for scalars/flow).
pub fn detect_format(data: &str) -> InputFormat {
    let trimmed = data.trim_start();
    match trimmed.as_bytes().first().copied() {
        Some(b'<') => InputFormat::Xml,
        Some(b'{') | Some(b'[') => InputFormat::Json,
        _ => {
            let first_line = trimmed.lines().next().unwrap_or("").trim_start();
            // A YAML document/directive or comment starts with these; otherwise a
            // header row containing a comma is treated as CSV.
            let looks_yaml = first_line.starts_with('#')
                || first_line.starts_with("---")
                || first_line.starts_with("%YAML");
            if !looks_yaml && first_line.contains(',') {
                InputFormat::Csv
            } else {
                InputFormat::Yaml
            }
        }
    }
}

/// Coerce a CSV cell string into a JSON scalar: integers and floats become
/// numbers, `true`/`false` become booleans, an empty cell becomes `null`, and
/// everything else stays a string. This makes numeric fields usable in JSONata
/// arithmetic/aggregation instead of arriving as strings.
fn coerce_cell(s: &str) -> Value {
    if s.is_empty() {
        return Value::Null;
    }
    if let Ok(i) = s.parse::<i64>() {
        return Value::from(i);
    }
    // Only treat as a float if it actually contains a digit (rejects "NaN"/"inf",
    // which parse::<f64>() otherwise accepts).
    if s.chars().any(|c| c.is_ascii_digit()) {
        if let Ok(f) = s.parse::<f64>() {
            if f.is_finite() {
                if let Some(n) = serde_json::Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
        }
    }
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(s.to_string()),
    }
}

/// Parse CSV (with a header row) into a JSON array of objects.
fn csv_to_value(data: &str) -> Result<Value, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| format!("invalid CSV header: {e}"))?
        .clone();
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("invalid CSV row: {e}"))?;
        let mut obj = serde_json::Map::new();
        for (i, header) in headers.iter().enumerate() {
            let cell = rec.get(i).unwrap_or("");
            obj.insert(header.to_string(), coerce_cell(cell));
        }
        rows.push(Value::Object(obj));
    }
    Ok(Value::Array(rows))
}

/// Parse the input document (in `fmt`) into a common JSON value.
fn parse_input(data: &str, fmt: InputFormat) -> Result<Value, String> {
    let fmt = match fmt {
        InputFormat::Auto => detect_format(data),
        other => other,
    };
    match fmt {
        InputFormat::Json => {
            serde_json::from_str(data).map_err(|e| format!("invalid JSON input: {e}"))
        }
        InputFormat::Yaml => {
            serde_yml::from_str(data).map_err(|e| format!("invalid YAML input: {e}"))
        }
        InputFormat::Csv => csv_to_value(data),
        InputFormat::Xml => {
            let opt = gizza_ai_xml_to_json_core::Options {
                coerce: true,
                ..Default::default()
            };
            gizza_ai_xml_to_json_core::to_value(data, &opt)
        }
        InputFormat::Auto => unreachable!("auto resolved above"),
    }
}

/// Serialize a JSON value to a YAML string, trimming the trailing newline for a
/// clean single result value.
fn value_to_yaml(v: &Value) -> Result<String, String> {
    let mut s = serde_yml::to_string(v).map_err(|e| format!("YAML serialization error: {e}"))?;
    while s.ends_with('\n') {
        s.pop();
    }
    Ok(s)
}

/// Parse `data` (in `input_format`), reshape it with the JSONata-style `query`,
/// and serialize the result in `output_format`.
///
/// - `pretty`: indent the JSON output (ignored for YAML, which is always block-style).
///
/// A query that matches nothing yields JSON `null`. Errors on an unparseable
/// input document, an invalid expression, or an evaluation error.
pub fn reshape(
    data: &str,
    query: &str,
    input_format: InputFormat,
    output_format: OutputFormat,
    pretty: bool,
) -> Result<String, String> {
    if query.trim().is_empty() {
        return Err("query is empty".into());
    }
    if data.trim().is_empty() {
        return Err("input data is empty".into());
    }

    // 1. Parse the document into a common JSON value.
    let value = parse_input(data, input_format)?;

    // 2. Parse and run the reshape expression against that value. A no-match
    //    result normalizes to JSON null.
    let ast =
        expr::parse(query).map_err(|e| format!("invalid JSONata expression: {e}"))?;
    let result = expr::eval_root(&ast, &value)?;
    let out_value = result.unwrap_or(Value::Null);

    // 3. Serialize in the requested output format.
    match output_format {
        OutputFormat::Json => {
            let out = if pretty {
                serde_json::to_string_pretty(&out_value)
            } else {
                serde_json::to_string(&out_value)
            };
            out.map_err(|e| format!("internal serialization error: {e}"))
        }
        OutputFormat::Yaml => value_to_yaml(&out_value),
    }
}

/// Convenience wrapper taking the two format params as strings (as the CLI/web
/// surfaces supply them).
pub fn run(
    data: &str,
    query: &str,
    input_format: &str,
    output_format: &str,
    pretty: bool,
) -> Result<String, String> {
    let inf = InputFormat::parse(input_format)?;
    let outf = OutputFormat::parse(output_format)?;
    reshape(data, query, inf, outf, pretty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_reshape_object_construction() {
        let out = reshape(
            r#"{"lines":[{"amount":4},{"amount":6}]}"#,
            r#"{ "total": $sum(lines.amount), "count": $count(lines) }"#,
            InputFormat::Json,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["total"], serde_json::json!(10));
        assert_eq!(parsed["count"], serde_json::json!(2));
    }

    #[test]
    fn csv_reshape_aggregate() {
        // CSV in, aggregate the numeric column (coerced from strings).
        let out = reshape(
            "name,price\napple,2\nbanana,3\ncherry,5\n",
            "$sum($.price)",
            InputFormat::Csv,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        assert_eq!(out, "10");
    }

    #[test]
    fn csv_reshape_filter_and_project() {
        let out = reshape(
            "name,price\napple,2\nbanana,20\ncherry,30\n",
            "$[price > 10].name",
            InputFormat::Csv,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        assert_eq!(out, r#"["banana","cherry"]"#);
    }

    #[test]
    fn yaml_reshape_to_yaml_output() {
        let out = reshape(
            "user:\n  first: Ada\n  last: Lovelace\n",
            r#"{ "name": user.first & " " & user.last }"#,
            InputFormat::Yaml,
            OutputFormat::Yaml,
            false,
        )
        .unwrap();
        assert_eq!(out, "name: Ada Lovelace");
    }

    #[test]
    fn xml_reshape_navigate() {
        let out = reshape(
            "<order><item><price>10</price></item><item><price>15</price></item></order>",
            "$sum(order.item.price)",
            InputFormat::Xml,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        assert_eq!(out, "25");
    }

    #[test]
    fn csv_worked_example_with_min() {
        // The page's worked example: total/items/cheapest over CSV rows.
        let out = reshape(
            "name,price\napple,2\nbanana,3\ncherry,5\n",
            r#"{ "total": $sum($.price), "items": $count($), "cheapest": $min($.price) }"#,
            InputFormat::Csv,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["total"], serde_json::json!(10));
        assert_eq!(parsed["items"], serde_json::json!(3));
        assert_eq!(parsed["cheapest"], serde_json::json!(2));
    }

    #[test]
    fn map_object_construction_over_array() {
        // The page's "reshape each object" example: orders.{ ... }.
        let out = reshape(
            r#"{ "orders": [ { "id": 1, "lines": [ { "amount": 4 }, { "amount": 6 } ] } ] }"#,
            r#"orders.{ "id": id, "total": $sum(lines.amount) }"#,
            InputFormat::Json,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["id"], serde_json::json!(1));
        assert_eq!(parsed["total"], serde_json::json!(10));
    }

    #[test]
    fn max_and_average_aggregates() {
        let out = reshape(
            "n\n2\n4\n6\n",
            r#"{ "max": $max($.n), "avg": $average($.n) }"#,
            InputFormat::Csv,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["max"], serde_json::json!(6));
        assert_eq!(parsed["avg"], serde_json::json!(4));
    }

    #[test]
    fn auto_detects_json() {
        assert_eq!(detect_format(r#"{"a":1}"#), InputFormat::Json);
        assert_eq!(detect_format("  [1,2,3]"), InputFormat::Json);
    }

    #[test]
    fn auto_detects_xml_csv_yaml() {
        assert_eq!(detect_format("<root/>"), InputFormat::Xml);
        assert_eq!(detect_format("a,b,c\n1,2,3"), InputFormat::Csv);
        assert_eq!(detect_format("key: value\n"), InputFormat::Yaml);
        assert_eq!(
            detect_format("# a yaml comment\nkey: value"),
            InputFormat::Yaml
        );
    }

    #[test]
    fn auto_reshape_end_to_end() {
        // No explicit format — auto picks CSV, reshapes into total.
        let out = run(
            "qty,cost\n2,3\n4,5\n",
            "$sum($.(qty * cost))",
            "auto",
            "json",
            false,
        )
        .unwrap();
        assert_eq!(out, "26");
    }

    #[test]
    fn pretty_json_is_indented() {
        let out = reshape(
            r#"{"a":1}"#,
            r#"{ "a": a }"#,
            InputFormat::Json,
            OutputFormat::Json,
            true,
        )
        .unwrap();
        assert!(
            out.contains('\n'),
            "pretty output should be multi-line: {out}"
        );
    }

    #[test]
    fn no_match_returns_null() {
        let out = reshape(
            r#"{"a":1}"#,
            "b.c.d",
            InputFormat::Json,
            OutputFormat::Json,
            false,
        )
        .unwrap();
        assert_eq!(out, "null");
    }

    #[test]
    fn error_on_invalid_json() {
        let err = reshape(
            "{not json",
            "$",
            InputFormat::Json,
            OutputFormat::Json,
            false,
        )
        .unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
    }

    #[test]
    fn error_on_invalid_query() {
        let err = reshape(
            r#"{"a":1}"#,
            "{ this is not jsonata",
            InputFormat::Json,
            OutputFormat::Json,
            false,
        )
        .unwrap_err();
        assert!(
            err.contains("JSONata") || err.contains("expression"),
            "got: {err}"
        );
    }

    #[test]
    fn error_on_empty_query() {
        assert!(reshape("{}", "   ", InputFormat::Json, OutputFormat::Json, false).is_err());
    }

    #[test]
    fn unknown_format_errors() {
        assert!(InputFormat::parse("toml").is_err());
        assert!(OutputFormat::parse("xml").is_err());
    }
}
