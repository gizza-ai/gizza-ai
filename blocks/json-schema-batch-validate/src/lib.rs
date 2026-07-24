//! gizza-ai/json-schema-batch-validate — chat skill block on the shared tool abstraction.
//! Validate a whole BATCH of JSON records (a JSON array, a single value, or an
//! NDJSON stream) against ONE JSON Schema and return a pass/fail summary plus a
//! per-record error list. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    schema: String,
    records: String,
    #[serde(default)]
    input_format: String,
    #[serde(default)]
    draft: String,
    #[serde(default = "default_max_errors")]
    max_errors: u32,
    #[serde(default)]
    output: String,
}

fn default_max_errors() -> u32 {
    50
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("schema").required().describe(
                "The JSON Schema to validate every record against, as a JSON object. Example: \
{\"type\":\"object\",\"required\":[\"id\"],\"properties\":{\"id\":{\"type\":\"integer\"}}}. If it \
carries a \"$schema\" URI and draft is 'auto', the draft is detected from it.",
            ),
        )
        .param(
            Param::string("records").required().describe(
                "The batch of records to validate. Accepts a JSON array of records \
([{...},{...}]), a single JSON value (validated as one record), or an NDJSON stream (one JSON \
value per line, blank lines skipped). See input_format to force one interpretation.",
            ),
        )
        .param(
            Param::enumv("input_format", ["auto", "json", "ndjson"])
                .default("auto")
                .describe(
                    "How to parse records. 'auto' (default) tries whole-document JSON first (an \
array becomes the batch, any other value is one record), then falls back to NDJSON. 'json' forces \
JSON, 'ndjson' forces one-JSON-value-per-line.",
                ),
        )
        .param(
            Param::enumv(
                "draft",
                ["auto", "draft4", "draft6", "draft7", "draft2019-09", "draft2020-12"],
            )
            .default("auto")
            .describe(
                "Draft label to report. 'auto' (default) reads the schema's \"$schema\" URI and \
falls back to draft2020-12 if absent/unrecognized; otherwise force one of draft4, draft6, \
draft7, draft2019-09, draft2020-12. Validation covers a common draft-agnostic keyword subset.",
            ),
        )
        .param(
            Param::integer("max_errors")
                .default(50)
                .min(1.0)
                .describe(
                    "Cap on how many individual errors are LISTED across the whole batch (default \
50). Per-record and total error COUNTS are always exact and uncapped; when more errors exist than \
are listed the report is marked truncated.",
                ),
        )
        .param(
            Param::enumv("output", ["text", "json"]).default("text").describe(
                "Output format. 'text' (default) is a human-readable PASS/FAIL summary with a \
per-record error breakdown. 'json' is the full machine-readable report object (valid, draft, \
input_format, total, passed, failed, total_errors, records[], truncated).",
            ),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-schema-batch-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a JSON array or NDJSON batch of records against one JSON Schema and get a pass/fail summary plus a per-record error list.",
    skill(
        description = "Validate a whole BATCH of JSON records against ONE JSON Schema and return a \
pass/fail summary plus a per-record error list. 'records' can be a JSON array of records, a single \
JSON value (one record), or an NDJSON stream (one JSON value per line) — set input_format to auto \
(default), json, or ndjson. Each error carries the failing value's JSON Pointer path, the schema \
keyword that rejected it (type, required, minimum, enum, pattern, …), and a message. Set draft to \
auto (detect from the schema's $schema, else draft2020-12) or force draft4/draft6/draft7/\
draft2019-09/draft2020-12 for reporting; validation covers a common draft-agnostic subset. \
max_errors caps how many errors are listed (counts stay exact). Set output to text (default, \
human report) or json (machine-readable report). Runs locally; no network, no external $ref \
fetching.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-schema-batch-validate", |a: Args| {
            gizza_ai_json_schema_batch_validate_core::render(
                &a.schema,
                &a.records,
                &a.input_format,
                &a.draft,
                a.max_errors as usize,
                &a.output,
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
                    "schema": { "type": "string", "description": "The JSON Schema to validate every record against, as a JSON object. Example: {\"type\":\"object\",\"required\":[\"id\"],\"properties\":{\"id\":{\"type\":\"integer\"}}}. If it carries a \"$schema\" URI and draft is 'auto', the draft is detected from it." },
                    "records": { "type": "string", "description": "The batch of records to validate. Accepts a JSON array of records ([{...},{...}]), a single JSON value (validated as one record), or an NDJSON stream (one JSON value per line, blank lines skipped). See input_format to force one interpretation." },
                    "input_format": { "type": "string", "enum": ["auto", "json", "ndjson"], "default": "auto", "description": "How to parse records. 'auto' (default) tries whole-document JSON first (an array becomes the batch, any other value is one record), then falls back to NDJSON. 'json' forces JSON, 'ndjson' forces one-JSON-value-per-line." },
                    "draft": { "type": "string", "enum": ["auto", "draft4", "draft6", "draft7", "draft2019-09", "draft2020-12"], "default": "auto", "description": "Draft label to report. 'auto' (default) reads the schema's \"$schema\" URI and falls back to draft2020-12 if absent/unrecognized; otherwise force one of draft4, draft6, draft7, draft2019-09, draft2020-12. Validation covers a common draft-agnostic keyword subset." },
                    "max_errors": { "type": "integer", "minimum": 1, "default": 50, "description": "Cap on how many individual errors are LISTED across the whole batch (default 50). Per-record and total error COUNTS are always exact and uncapped; when more errors exist than are listed the report is marked truncated." },
                    "output": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format. 'text' (default) is a human-readable PASS/FAIL summary with a per-record error breakdown. 'json' is the full machine-readable report object (valid, draft, input_format, total, passed, failed, total_errors, records[], truncated)." }
                },
                "required": ["schema", "records"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
