//! gizza-ai/csv-structure-validator — lint a CSV's raw text for structural
//! faults (ragged rows, unclosed/stray quotes, blank rows, header problems,
//! whitespace, mixed line endings). Thin wrapper; the chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_structure_validator_core::validate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    quote: String,
    #[serde(default)]
    comment: String,
    #[serde(default = "default_max_issues")]
    max_issues: u64,
}
fn default_true() -> bool {
    true
}
fn default_max_issues() -> u64 {
    50
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to validate."))
        .param(Param::boolean("header").default(true).describe(
            "Treat the first row as a header naming the columns; enables the empty/duplicate header-name checks. Default true.",
        ))
        .param(
            Param::enumv("delimiter", ["auto", "comma", "tab", "semicolon", "pipe"])
                .default("auto")
                .describe(
                    "Field separator. 'auto' (default) detects comma/tab/semicolon/pipe from the first data line; or pick one explicitly.",
                ),
        )
        .param(Param::enumv("quote", ["double", "single", "none"]).default("double").describe(
            "Quote character the CSV uses: double (\") per RFC 4180 (default), single ('), or none to disable quote handling.",
        ))
        .param(Param::string("comment").default("").describe(
            "Optional comment prefix: lines starting with this single character (e.g. '#') are skipped. Empty (default) treats no line as a comment.",
        ))
        .param(
            Param::integer("max_issues")
                .default(50)
                .min(1.0)
                .max(1000.0)
                .describe(
                    "Maximum number of issues to list in the report (1-1000). Error/warning counts still cover the whole file; the report flags truncation. Default 50.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-structure-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint a CSV for ragged rows, unbalanced quotes, and stray delimiters",
    skill(
        description = "Validate the structure of a CSV: scan the raw text for ragged rows (inconsistent column counts), unclosed quotes, stray quote characters, blank or all-empty rows, empty/duplicate header names, leading/trailing whitespace, and mixed line endings. Returns a report with valid true/false, the resolved delimiter and quote style, expected column count, row counts, error/warning totals, and every issue with its 1-based line number, severity (error|warning), machine code, and message. delimiter auto-detects comma/tab/semicolon/pipe by default; quote is double|single|none; comment optionally skips lines starting with a character like '#'; max_issues (1-1000, default 50) caps the listed issues. Report-only — the CSV is never modified. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-structure-validator", |a: Args| {
            validate(
                &a.data,
                a.header,
                &a.delimiter,
                &a.quote,
                &a.comment,
                a.max_issues as usize,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":       { "type": "string", "description": "The CSV text to validate." },
                    "header":     { "type": "boolean", "default": true, "description": "Treat the first row as a header naming the columns; enables the empty/duplicate header-name checks. Default true." },
                    "delimiter":  { "type": "string", "enum": ["auto", "comma", "tab", "semicolon", "pipe"], "default": "auto", "description": "Field separator. 'auto' (default) detects comma/tab/semicolon/pipe from the first data line; or pick one explicitly." },
                    "quote":      { "type": "string", "enum": ["double", "single", "none"], "default": "double", "description": "Quote character the CSV uses: double (\") per RFC 4180 (default), single ('), or none to disable quote handling." },
                    "comment":    { "type": "string", "default": "", "description": "Optional comment prefix: lines starting with this single character (e.g. '#') are skipped. Empty (default) treats no line as a comment." },
                    "max_issues": { "type": "integer", "default": 50, "minimum": 1, "maximum": 1000, "description": "Maximum number of issues to list in the report (1-1000). Error/warning counts still cover the whole file; the report flags truncation. Default 50." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
