//! json-schema-batch-validate core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Validate a whole BATCH of JSON records against ONE JSON Schema and return a
//! pass/fail summary plus a per-record error list. The batch can be a JSON
//! array of records, a single JSON value (one record), or an NDJSON stream
//! (one JSON value per line). Every error carries the failing value's JSON
//! Pointer path, the schema keyword that rejected it, and a human message.
//! Report-only — nothing is modified.
//!
//! Validation is done by a small in-repo, pure-Rust validator (see the
//! [`validator`] module) that supports the common JSON Schema subset — `type`,
//! `required`, `properties`, `additionalProperties`, `enum`, `const`,
//! `minimum`/`maximum`/`exclusiveMinimum`/`exclusiveMaximum`,
//! `minLength`/`maxLength`, `pattern`, `items`, and internal (`#/…`) `$ref`.
//! It carries no SIMD-using dependencies, so the compiled block instantiates
//! cleanly in the (SIMD-disabled) wafer runtime and the browser. The `draft`
//! field is still detected/selected and reported, but validation itself is
//! draft-agnostic over this subset (see the module docs for the caveats).

use serde::Serialize;
use serde_json::Value;

pub mod validator;
use validator::CompiledSchema;

/// One validation failure inside a single record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordError {
    /// JSON Pointer to the failing value within the record ("" = the record
    /// root, "/age" = its `age` member, "/tags/0" = first tag, …).
    pub path: String,
    /// The schema keyword that rejected the value (type, required, minimum,
    /// enum, pattern, …) — the last segment of the schema location.
    pub keyword: String,
    /// Human-readable description of the failure.
    pub message: String,
}

/// The result for one record in the batch.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RecordResult {
    /// 0-based position of this record in the batch (array index or NDJSON
    /// line order, blank lines skipped).
    pub index: usize,
    /// True iff the record satisfied the schema.
    pub valid: bool,
    /// Total number of errors this record produced (NOT capped by max_errors).
    pub error_count: usize,
    /// The errors, sorted by path — capped by the shared max_errors budget.
    pub errors: Vec<RecordError>,
}

/// The full batch report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// True iff EVERY record passed (an empty batch is valid).
    pub valid: bool,
    /// Resolved draft used for validation (e.g. "draft2020-12").
    pub draft: String,
    /// True when the draft was auto-detected from the schema's `$schema`.
    pub draft_detected: bool,
    /// Resolved input format actually parsed: "json" or "ndjson".
    pub input_format: String,
    /// Number of records in the batch.
    pub total: usize,
    /// Records that passed.
    pub passed: usize,
    /// Records that failed.
    pub failed: usize,
    /// Sum of every record's error_count (NOT capped by max_errors).
    pub total_errors: usize,
    /// Per-record results for the FAILING records only (index-addressable);
    /// passing records are counted in `passed`, not listed here.
    pub records: Vec<RecordResult>,
    /// True when more errors exist than were listed (max_errors budget hit).
    pub truncated: bool,
}

fn draft_from_key(key: &str) -> Result<&'static str, String> {
    Ok(match key {
        "draft4" => "draft4",
        "draft6" => "draft6",
        "draft7" => "draft7",
        "draft2019-09" => "draft2019-09",
        "draft2020-12" => "draft2020-12",
        other => {
            return Err(format!(
                "draft must be auto, draft4, draft6, draft7, draft2019-09, or draft2020-12 — got '{other}'"
            ))
        }
    })
}

/// Map a `$schema` meta-schema URI to a draft name (display only — validation
/// is draft-agnostic over the supported subset).
fn draft_from_uri(uri: &str) -> Option<&'static str> {
    let u = uri.to_ascii_lowercase();
    if u.contains("2020-12") {
        Some("draft2020-12")
    } else if u.contains("2019-09") {
        Some("draft2019-09")
    } else if u.contains("draft-07") || u.contains("draft/7") {
        Some("draft7")
    } else if u.contains("draft-06") || u.contains("draft/6") {
        Some("draft6")
    } else if u.contains("draft-04") || u.contains("draft/4") || u.contains("draft-03") {
        // draft-03 is treated as the closest supported name, draft4.
        Some("draft4")
    } else {
        None
    }
}

fn resolve_draft(spec: &str, schema: &Value) -> Result<(String, bool), String> {
    match spec.trim() {
        "" | "auto" => {
            if let Some(uri) = schema.get("$schema").and_then(Value::as_str) {
                if let Some(name) = draft_from_uri(uri) {
                    return Ok((name.to_string(), true));
                }
            }
            // No (recognized) $schema: default to the latest stable draft.
            Ok(("draft2020-12".to_string(), false))
        }
        other => Ok((draft_from_key(other)?.to_string(), false)),
    }
}

/// Split the batch text into record values per `input_format`, returning the
/// values plus the format that was actually used.
fn parse_records(records: &str, input_format: &str) -> Result<(Vec<Value>, String), String> {
    match input_format.trim() {
        "json" => Ok((parse_json_batch(records)?, "json".to_string())),
        "ndjson" => Ok((parse_ndjson(records)?, "ndjson".to_string())),
        "" | "auto" => {
            // Try to parse the whole thing as one JSON document first (covers a
            // JSON array of records, a pretty-printed object, or a single
            // scalar). If that fails, fall back to NDJSON.
            match serde_json::from_str::<Value>(records) {
                Ok(v) => Ok((flatten_top(v), "json".to_string())),
                Err(json_err) => match parse_ndjson(records) {
                    Ok(vals) => Ok((vals, "ndjson".to_string())),
                    Err(ndjson_err) => Err(format!(
                        "records is neither valid JSON nor NDJSON. As JSON: {json_err}. As NDJSON: {ndjson_err}. \
                         Set input_format explicitly (json or ndjson) if you know the format."
                    )),
                },
            }
        }
        other => Err(format!(
            "input_format must be auto, json, or ndjson — got '{other}'"
        )),
    }
}

/// A top-level JSON array is a batch of records; anything else is one record.
fn flatten_top(v: Value) -> Vec<Value> {
    match v {
        Value::Array(items) => items,
        other => vec![other],
    }
}

fn parse_json_batch(records: &str) -> Result<Vec<Value>, String> {
    let v: Value =
        serde_json::from_str(records).map_err(|e| format!("records is not valid JSON: {e}"))?;
    Ok(flatten_top(v))
}

fn parse_ndjson(records: &str) -> Result<Vec<Value>, String> {
    let mut out = Vec::new();
    for (i, raw) in records.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue; // blank lines are allowed between NDJSON records
        }
        let v: Value = serde_json::from_str(line)
            .map_err(|e| format!("NDJSON line {} is not valid JSON: {e}", i + 1))?;
        out.push(v);
    }
    Ok(out)
}

fn build_validator(schema: &Value) -> Result<CompiledSchema, String> {
    CompiledSchema::build(schema)
        .map_err(|e| format!("the schema is not a valid JSON Schema: {e}"))
}

/// Validate a batch and return the structured [`Report`].
///
/// `schema` is the JSON Schema text; `records` is the batch; `input_format` is
/// `auto`|`json`|`ndjson`; `draft` is `auto` or an explicit draft key;
/// `max_errors` caps how many errors are LISTED across the whole batch (the
/// per-record and total counts are always exact).
pub fn validate(
    schema: &str,
    records: &str,
    input_format: &str,
    draft: &str,
    max_errors: usize,
) -> Result<Report, String> {
    if schema.trim().is_empty() {
        return Err("schema is empty: paste a JSON Schema to validate against".to_string());
    }
    let max_errors = max_errors.max(1);
    let schema_val: Value =
        serde_json::from_str(schema).map_err(|e| format!("schema is not valid JSON: {e}"))?;

    let (draft_name, draft_detected) = resolve_draft(draft, &schema_val)?;
    let validator = build_validator(&schema_val)?;

    let (values, input_format_used) = parse_records(records, input_format)?;

    let mut report = Report {
        valid: true,
        draft: draft_name,
        draft_detected,
        input_format: input_format_used,
        total: values.len(),
        passed: 0,
        failed: 0,
        total_errors: 0,
        records: Vec::new(),
        truncated: false,
    };

    let mut budget = max_errors;
    for (index, value) in values.iter().enumerate() {
        // Collect + sort this record's errors for deterministic output.
        let mut errs: Vec<RecordError> = validator.errors(value);
        errs.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then_with(|| a.keyword.cmp(&b.keyword))
                .then_with(|| a.message.cmp(&b.message))
        });

        let error_count = errs.len();
        if error_count == 0 {
            report.passed += 1;
            continue;
        }
        report.failed += 1;
        report.valid = false;
        report.total_errors += error_count;

        // Spend the shared listing budget.
        let listed = errs.len().min(budget);
        if listed < error_count {
            report.truncated = true;
        }
        errs.truncate(listed);
        budget -= listed;

        report.records.push(RecordResult {
            index,
            valid: false,
            error_count,
            errors: errs,
        });
    }

    Ok(report)
}

/// Validate a batch and render it in the requested `output` mode: `text`
/// (default, the human-readable summary) or `json` (the pretty-printed
/// [`Report`], a machine-readable object). This is the single entry point the
/// chat block, the CLI, and the web page all call.
pub fn render(
    schema: &str,
    records: &str,
    input_format: &str,
    draft: &str,
    max_errors: usize,
    output: &str,
) -> Result<String, String> {
    match output.trim() {
        "" | "text" => summary(schema, records, input_format, draft, max_errors),
        "json" => {
            let r = validate(schema, records, input_format, draft, max_errors)?;
            serde_json::to_string_pretty(&r)
                .map_err(|e| format!("failed to serialize the report as JSON: {e}"))
        }
        other => Err(format!("output must be text or json — got '{other}'")),
    }
}

/// Render a batch report as a human-readable text summary (the page surface).
pub fn summary(
    schema: &str,
    records: &str,
    input_format: &str,
    draft: &str,
    max_errors: usize,
) -> Result<String, String> {
    let r = validate(schema, records, input_format, draft, max_errors)?;
    let mut out = String::new();

    let verdict = if r.valid { "PASS" } else { "FAIL" };
    out.push_str(&format!("Batch validation: {verdict}\n"));

    let draft_note = if r.draft_detected {
        format!("{} (from $schema)", r.draft)
    } else {
        r.draft.clone()
    };
    out.push_str(&format!(
        "Draft: {}    Input: {}    Records: {} ({} passed, {} failed)\n",
        draft_note, r.input_format, r.total, r.passed, r.failed
    ));

    let listed: usize = r.records.iter().map(|rec| rec.errors.len()).sum();
    if r.total_errors == 0 {
        out.push_str("Total errors: 0\n");
    } else if r.truncated {
        out.push_str(&format!(
            "Total errors: {} (showing first {})\n",
            r.total_errors, listed
        ));
    } else {
        out.push_str(&format!("Total errors: {}\n", r.total_errors));
    }

    for rec in &r.records {
        out.push('\n');
        let noun = if rec.error_count == 1 { "error" } else { "errors" };
        out.push_str(&format!(
            "Record #{} — {} {}\n",
            rec.index, rec.error_count, noun
        ));
        for e in &rec.errors {
            let where_ = if e.path.is_empty() {
                "(root)".to_string()
            } else {
                e.path.clone()
            };
            out.push_str(&format!("  {}: {}\n", where_, e.message));
        }
        if rec.error_count > rec.errors.len() {
            out.push_str(&format!(
                "  … {} more error(s) in this record\n",
                rec.error_count - rec.errors.len()
            ));
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pass_array() {
        let schema = r#"{"type":"object","properties":{"n":{"type":"integer"}},"required":["n"]}"#;
        let records = r#"[{"n":1},{"n":2}]"#;
        let r = validate(schema, records, "auto", "auto", 100).unwrap();
        assert!(r.valid);
        assert_eq!(r.total, 2);
        assert_eq!(r.passed, 2);
        assert_eq!(r.failed, 0);
        assert_eq!(r.total_errors, 0);
        assert!(r.records.is_empty());
        assert_eq!(r.input_format, "json");
    }

    #[test]
    fn per_record_errors_reported() {
        let schema =
            r#"{"type":"object","properties":{"age":{"type":"integer"}},"required":["id"]}"#;
        // record 0 ok; record 1 wrong type + missing required
        let records = r#"[{"id":"a","age":5},{"age":"x"}]"#;
        let r = validate(schema, records, "auto", "auto", 100).unwrap();
        assert!(!r.valid);
        assert_eq!(r.total, 2);
        assert_eq!(r.passed, 1);
        assert_eq!(r.failed, 1);
        assert_eq!(r.records.len(), 1);
        let rec = &r.records[0];
        assert_eq!(rec.index, 1);
        assert_eq!(rec.error_count, 2);
        // one error at /age (type), one at root (required)
        assert!(rec.errors.iter().any(|e| e.path == "/age" && e.keyword == "type"));
        assert!(rec
            .errors
            .iter()
            .any(|e| e.path.is_empty() && e.keyword == "required"));
    }

    #[test]
    fn ndjson_stream() {
        let schema = r#"{"type":"object","required":["x"]}"#;
        let records = "{\"x\":1}\n\n{\"y\":2}\n{\"x\":3}\n";
        let r = validate(schema, records, "auto", "auto", 100).unwrap();
        assert_eq!(r.input_format, "ndjson");
        assert_eq!(r.total, 3);
        assert_eq!(r.failed, 1);
        assert_eq!(r.records[0].index, 1); // the {"y":2} record (blank line skipped)
    }

    #[test]
    fn single_object_is_one_record() {
        let schema = r#"{"type":"object","required":["x"]}"#;
        let r = validate(schema, r#"{"x":1}"#, "auto", "auto", 100).unwrap();
        assert_eq!(r.total, 1);
        assert!(r.valid);
    }

    #[test]
    fn explicit_draft_selection() {
        let schema = r#"{"type":"string"}"#;
        let r = validate(schema, r#"["a","b"]"#, "json", "draft4", 100).unwrap();
        assert!(r.valid);
        assert_eq!(r.draft, "draft4");
        assert!(!r.draft_detected);
    }

    #[test]
    fn draft_detected_from_schema() {
        let schema =
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"integer"}"#;
        let r = validate(schema, r#"[1,2]"#, "json", "auto", 100).unwrap();
        assert_eq!(r.draft, "draft2020-12");
        assert!(r.draft_detected);
    }

    #[test]
    fn max_errors_caps_listing() {
        let schema = r#"{"type":"object","properties":{"a":{"type":"integer"},"b":{"type":"integer"}},"required":["a","b"]}"#;
        // two bad records, 2 errors each = 4 total; cap listing at 3
        let records = r#"[{"a":"x","b":"y"},{"a":"p","b":"q"}]"#;
        let r = validate(schema, records, "json", "auto", 3).unwrap();
        assert_eq!(r.total_errors, 4);
        assert!(r.truncated);
        let listed: usize = r.records.iter().map(|rec| rec.errors.len()).sum();
        assert_eq!(listed, 3);
    }

    #[test]
    fn bad_schema_json_errors() {
        let err = validate("{not json", "[]", "auto", "auto", 100).unwrap_err();
        assert!(err.contains("schema is not valid JSON"));
    }

    #[test]
    fn invalid_schema_errors() {
        // `type` must be a string/array of strings, not a number
        let err = validate(r#"{"type": 5}"#, "[]", "auto", "auto", 100).unwrap_err();
        assert!(err.contains("not a valid JSON Schema"), "got: {err}");
    }

    #[test]
    fn bad_ndjson_line_errors() {
        let err =
            validate(r#"{"type":"object"}"#, "{\"x\":1}\nnot json", "ndjson", "auto", 100)
                .unwrap_err();
        assert!(err.contains("NDJSON line 2"), "got: {err}");
    }

    #[test]
    fn empty_batch_is_valid() {
        let r = validate(r#"{"type":"object"}"#, "[]", "json", "auto", 100).unwrap();
        assert!(r.valid);
        assert_eq!(r.total, 0);
    }

    #[test]
    fn summary_text_shape() {
        let schema =
            r#"{"type":"object","properties":{"age":{"type":"integer"}},"required":["id"]}"#;
        let records = r#"[{"id":"a"},{"age":"x"}]"#;
        let s = summary(schema, records, "json", "auto", 100).unwrap();
        assert!(s.starts_with("Batch validation: FAIL\n"));
        assert!(s.contains("Records: 2 (1 passed, 1 failed)"));
        assert!(s.contains("Record #1 —"));
        assert!(s.contains("(root):"));
    }

    #[test]
    fn summary_all_pass() {
        let s = summary(r#"{"type":"integer"}"#, "[1,2,3]", "json", "auto", 100).unwrap();
        assert!(s.starts_with("Batch validation: PASS\n"));
        assert!(s.contains("Total errors: 0"));
    }

    #[test]
    fn render_text_matches_summary() {
        let schema = r#"{"type":"object","required":["x"]}"#;
        let records = r#"[{"x":1},{"y":2}]"#;
        let via_render = render(schema, records, "json", "auto", 100, "text").unwrap();
        let via_summary = summary(schema, records, "json", "auto", 100).unwrap();
        assert_eq!(via_render, via_summary);
        // Default (empty) output is text too.
        let via_default = render(schema, records, "json", "auto", 100, "").unwrap();
        assert_eq!(via_default, via_summary);
    }

    #[test]
    fn render_json_is_machine_readable() {
        let schema = r#"{"type":"object","required":["x"]}"#;
        let records = r#"[{"x":1},{"y":2}]"#;
        let json = render(schema, records, "json", "auto", 100, "json").unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["valid"], serde_json::json!(false));
        assert_eq!(v["total"], serde_json::json!(2));
        assert_eq!(v["passed"], serde_json::json!(1));
        assert_eq!(v["failed"], serde_json::json!(1));
        assert_eq!(v["records"][0]["index"], serde_json::json!(1));
        assert_eq!(v["records"][0]["errors"][0]["keyword"], serde_json::json!("required"));
    }

    #[test]
    fn render_bad_output_mode_errors() {
        let err = render(r#"{"type":"integer"}"#, "[1]", "json", "auto", 100, "yaml").unwrap_err();
        assert!(err.contains("output must be text or json"), "got: {err}");
    }
}
