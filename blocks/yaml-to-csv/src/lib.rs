//! gizza-ai/yaml-to-csv — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_to_csv_core::to_csv;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_comma")]
    delimiter: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_json")]
    array_mode: String,
    #[serde(default)]
    quote_all: bool,
    #[serde(default = "default_key")]
    key_column: String,
}
fn default_true() -> bool {
    true
}
fn default_comma() -> String {
    "comma".to_string()
}
fn default_json() -> String {
    "json".to_string()
}
fn default_key() -> String {
    "key".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The YAML text: a top-level list of records, or a mapping whose values are records (or all scalars = one record)."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Column separator for the CSV output. Default 'comma'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Emit the column-name header row (default true). false writes data rows only."),
        )
        .param(
            Param::enumv("array_mode", ["json", "joined", "columns"])
                .default("json")
                .describe("How array fields render: 'json' (default) writes the whole array as a compact JSON string in one cell; 'joined' joins scalar arrays with ', ' in one cell; 'columns' expands each element into its own dot-indexed column (tags.0, tags.1)."),
        )
        .param(
            Param::boolean("quote_all")
                .default(false)
                .describe("Wrap every field in double quotes, not just the ones that need it (commas, quotes, newlines). Default false."),
        )
        .param(
            Param::string("key_column")
                .default("key")
                .describe("For a top-level mapping of records, the header of the extra column holding each entry's key (e.g. 'id'). Blank omits it. Ignored for a top-level list. Default 'key'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten a YAML list (or mapping) of records into a CSV with a unioned header.",
    skill(
        description = "Flatten a YAML list of records (or a top-level mapping whose values are records) into CSV. Columns are the union of every record's keys in first-seen order; nested mappings flatten to dot-paths (user.name); arrays render per array_mode ('json' compact string, 'joined' comma-joined, or 'columns' dot-indexed). delimiter is comma/tab/semicolon/pipe; header toggles the header row; quote_all forces quoting; key_column names the extra column that keeps a mapping entry's key. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-to-csv", |a: Args| {
            to_csv(
                &a.data,
                &a.delimiter,
                a.header,
                &a.array_mode,
                a.quote_all,
                &a.key_column,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. (Regenerate this literal when the descriptor changes.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The YAML text: a top-level list of records, or a mapping whose values are records (or all scalars = one record)." },
                    "delimiter": { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Column separator for the CSV output. Default 'comma'." },
                    "header": { "type": "boolean", "default": true, "description": "Emit the column-name header row (default true). false writes data rows only." },
                    "array_mode": { "type": "string", "enum": ["json", "joined", "columns"], "default": "json", "description": "How array fields render: 'json' (default) writes the whole array as a compact JSON string in one cell; 'joined' joins scalar arrays with ', ' in one cell; 'columns' expands each element into its own dot-indexed column (tags.0, tags.1)." },
                    "quote_all": { "type": "boolean", "default": false, "description": "Wrap every field in double quotes, not just the ones that need it (commas, quotes, newlines). Default false." },
                    "key_column": { "type": "string", "default": "key", "description": "For a top-level mapping of records, the header of the extra column holding each entry's key (e.g. 'id'). Blank omits it. Ignored for a top-level list. Default 'key'." }
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
