//! gizza-ai/data-validator — chat skill block on the shared tool abstraction.
//! Validate pasted CSV **or** JSON rows against a set of field rules (required,
//! unique, type, numeric range, length, regex, enum) and list every violation
//! with its record/line, field, value, the rule it broke, and a message. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    rules: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_header")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_max_issues")]
    max_issues: i64,
    #[serde(default = "default_format")]
    format: String,
}

fn default_input_format() -> String {
    "auto".to_string()
}
fn default_header() -> bool {
    true
}
fn default_delimiter() -> String {
    "auto".to_string()
}
fn default_max_issues() -> i64 {
    50
}
fn default_format() -> String {
    "text".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The rows to validate. CSV with a header row, or JSON as an array of objects, a single object, or JSON Lines (one object per line)."),
        )
        .param(
            Param::string("rules")
                .required()
                .describe("Field rules, one per line as `field:rule` or `field:rule=arg` (blank lines and `#` comments ignored). Rules: required, unique, type=int|float|bool|date|email|url (bare `age:int` is shorthand for type=int), min=/max= (numeric range), minlen=/maxlen= (character length), regex=… (unanchored — add ^…$ to anchor), enum=a|b|c (exact membership). Every rule except `required` is skipped for a blank/missing value."),
        )
        .param(
            Param::enumv("input_format", ["auto", "csv", "json"])
                .default("auto")
                .describe("How to read `data`: 'auto' (default) treats it as JSON when it starts with [ or {, else CSV; 'csv'; or 'json' (also accepts NDJSON / JSON Lines)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("CSV only: treat the first row as a header of field names (default true). When false, refer to columns by 1-based index, e.g. `2:type=int`."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe("CSV only: field delimiter. 'auto' (default) detects comma, tab, semicolon or pipe from the first row."),
        )
        .param(
            Param::integer("max_issues")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of violations to list; the total count is always reported. Default 50. Clamped to 1-1000."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format: 'text' (default) a human report, or 'json' the structured report (valid flag, counts, and the full violation list)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate pasted CSV or JSON rows against field rules and list every violation",
    skill(
        description = "Validate pasted CSV or JSON rows against a set of field rules and list every violation with its record, physical line, field, offending value, the rule it broke, and a human message. `data` is CSV (with a header row) or JSON (an array of objects, a single object, or JSON Lines / NDJSON). `rules` is one rule per line as `field:rule` or `field:rule=arg`: required, unique, type=int|float|bool|date|email|url (bare `age:int` shorthand), min=/max= (numeric range), minlen=/maxlen= (character length), regex=… (unanchored — add ^…$ to anchor), enum=a|b|c (exact membership). Every rule except `required` is skipped for a blank/missing value, so combine with `required` when a value must be present. input_format is auto (default) / csv / json. For CSV, header=true (default) names fields by the header row — set header=false to refer to columns by 1-based index; delimiter is auto (default) / comma / tab / semicolon / pipe. max_issues caps the listed violations (default 50, total always counted). format is text (default) or json. Report-only — the input is never modified, nothing is fetched or persisted. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-validator", |a: Args| {
            gizza_ai_data_validator_core::run(
                &a.data,
                &a.rules,
                &a.input_format,
                a.header,
                &a.delimiter,
                a.max_issues.clamp(1, 1000) as usize,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The rows to validate. CSV with a header row, or JSON as an array of objects, a single object, or JSON Lines (one object per line)." },
                    "rules": { "type": "string", "description": "Field rules, one per line as `field:rule` or `field:rule=arg` (blank lines and `#` comments ignored). Rules: required, unique, type=int|float|bool|date|email|url (bare `age:int` is shorthand for type=int), min=/max= (numeric range), minlen=/maxlen= (character length), regex=… (unanchored — add ^…$ to anchor), enum=a|b|c (exact membership). Every rule except `required` is skipped for a blank/missing value." },
                    "input_format": { "type": "string", "enum": ["auto", "csv", "json"], "default": "auto", "description": "How to read `data`: 'auto' (default) treats it as JSON when it starts with [ or {, else CSV; 'csv'; or 'json' (also accepts NDJSON / JSON Lines)." },
                    "header": { "type": "boolean", "default": true, "description": "CSV only: treat the first row as a header of field names (default true). When false, refer to columns by 1-based index, e.g. `2:type=int`." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "CSV only: field delimiter. 'auto' (default) detects comma, tab, semicolon or pipe from the first row." },
                    "max_issues": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 50, "description": "Maximum number of violations to list; the total count is always reported. Default 50. Clamped to 1-1000." },
                    "format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format: 'text' (default) a human report, or 'json' the structured report (valid flag, counts, and the full violation list)." }
                },
                "required": ["data", "rules"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
