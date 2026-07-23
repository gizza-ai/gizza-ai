//! gizza-ai/jsonl-deduplicator — remove duplicate records from an NDJSON/JSONL
//! stream by whole-line equality or a chosen set of key fields, keeping the
//! first or last occurrence. Chat schema single-sourced from descriptor();
//! handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_jsonl_deduplicator_core::{dedupe, parse_keys, Keep, OnInvalid, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    keys: String,
    #[serde(default = "default_keep")]
    keep: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_on_invalid")]
    on_invalid: String,
}

fn default_keep() -> String {
    "first".to_string()
}

fn default_on_invalid() -> String {
    "error".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The NDJSON/JSONL text to de-duplicate: one JSON value per line. Blank lines are ignored."),
        )
        .param(
            Param::string("keys")
                .default("")
                .describe("Comma-separated key fields to de-duplicate on, e.g. \"id\" or \"user.id,email\" (dot-notation reaches nested fields; a numeric segment like \"tags.0\" indexes an array). Leave blank to compare by whole-line equality (the exact text of each line)."),
        )
        .param(
            Param::enumv("keep", ["first", "last"])
                .default("first")
                .describe("Which occurrence of a duplicated record to keep: the first (default) or the last. Original stream order is preserved either way."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare records case-insensitively (e.g. so \"A@X.com\" and \"a@x.com\" collapse). Applies to the whole line or to the chosen key values."),
        )
        .param(
            Param::enumv("on_invalid", ["error", "skip", "keep"])
                .default("error")
                .describe("What to do with a line that is not valid JSON when de-duplicating by key fields: error (stop and name the line, default), skip (drop it), or keep (keep it, matched by its raw text). Ignored in whole-line mode, which never parses JSON."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonlDeduplicator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/jsonl-deduplicator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate records from an NDJSON/JSONL stream",
    skill(
        description = "Remove duplicate records from NDJSON/JSONL text (one JSON value per line) and return the de-duplicated text plus total/kept/removed record counts. Leave keys blank to de-duplicate by whole-line equality (exact line text); set keys to a comma-separated list like \"id\" or \"user.id,email\" to de-duplicate by those fields (dot-notation reaches nested fields; a numeric segment indexes an array). By default the first occurrence of each record is kept and original order is preserved; set keep=\"last\" to keep the last. ignore_case=true compares case-insensitively. In key-field mode, on_invalid controls lines that aren't valid JSON: error (default, names the line), skip (drop), or keep. Blank lines are ignored. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonlDeduplicator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "jsonl-deduplicator", |a: Args| {
            let keep = Keep::parse(&a.keep).map_err(SkillError::InvalidArgs)?;
            let on_invalid = OnInvalid::parse(&a.on_invalid).map_err(SkillError::InvalidArgs)?;
            let opts = Options {
                keys: parse_keys(&a.keys),
                keep,
                ignore_case: a.ignore_case,
                on_invalid,
            };
            dedupe(&a.data, &opts).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The NDJSON/JSONL text to de-duplicate: one JSON value per line. Blank lines are ignored." },
                    "keys": { "type": "string", "default": "", "description": "Comma-separated key fields to de-duplicate on, e.g. \"id\" or \"user.id,email\" (dot-notation reaches nested fields; a numeric segment like \"tags.0\" indexes an array). Leave blank to compare by whole-line equality (the exact text of each line)." },
                    "keep": { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which occurrence of a duplicated record to keep: the first (default) or the last. Original stream order is preserved either way." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare records case-insensitively (e.g. so \"A@X.com\" and \"a@x.com\" collapse). Applies to the whole line or to the chosen key values." },
                    "on_invalid": { "type": "string", "enum": ["error", "skip", "keep"], "default": "error", "description": "What to do with a line that is not valid JSON when de-duplicating by key fields: error (stop and name the line, default), skip (drop it), or keep (keep it, matched by its raw text). Ignored in whole-line mode, which never parses JSON." }
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
