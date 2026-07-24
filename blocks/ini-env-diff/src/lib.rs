//! gizza-ai/ini-env-diff — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The first (old/base) config file contents.
    left: String,
    /// The second (new/compared) config file contents.
    right: String,
    /// Parse format: auto | env | ini.
    #[serde(default)]
    format: String,
    /// Compare key names case-insensitively (default false).
    #[serde(default)]
    ignore_case: bool,
    /// Mask values of sensitive-looking keys (default false).
    #[serde(default)]
    mask_secrets: bool,
    /// Output form: report | json.
    #[serde(default)]
    output: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (old/base) config file contents. Accepts .env or .ini/.conf text: KEY=VALUE or 'key = value'/'key: value' pairs, # and ; comments, blank lines, single/double quotes, inline comments, an 'export ' prefix, and [section] headers (become dotted 'section.key')."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (new/compared) config file contents, in the same syntax as `left`. Keys only in `right` are reported as added, keys only in `left` as removed, and keys in both with different values as changed."),
        )
        .param(
            Param::enumv("format", ["auto", "env", "ini"])
                .default("auto")
                .describe("How to parse both files: 'auto' (default) uses INI parsing if either file has a [section] header, otherwise env; 'env' forces flat KEY=VALUE (dotenv); 'ini' forces [section]-aware parsing with dotted section.key names."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare key names case-insensitively, so DB_HOST and db_host are treated as the same key. Default false (case-sensitive)."),
        )
        .param(
            Param::boolean("mask_secrets")
                .default(false)
                .describe("Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in the output so a diff can be shared safely. Default false (show raw values)."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a grouped human summary with Added/Removed/Changed/Unchanged sections and counts; 'json' a structured object { format, summary, added, removed, changed, unchanged }."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ini-env-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Diff two .env or .ini config files key-by-key: added, removed and changed settings.",
    skill(
        description = "Diff two .env or .ini/.conf config files key-by-key. Pass the first (old/base) file in `left` and the second (new/compared) file in `right`; the tool parses both — handling KEY=VALUE and 'key = value'/'key: value' pairs, # and ; comments, blank lines, single/double quotes, inline comments, an 'export ' prefix, and [section] headers (flattened to dotted 'section.key') — then reports keys that were added (only in `right`), removed (only in `left`), changed (in both with a different value, shown old -> new), and unchanged, with a count summary. `format` picks the parser (auto|env|ini; auto detects [section] headers). Set `ignore_case` to compare key names case-insensitively. Set `mask_secrets` to redact values of sensitive-looking keys so the diff can be shared. `output` selects 'report' (default, grouped human summary) or 'json' (structured). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ini-env-diff", |a: Args| {
            gizza_ai_ini_env_diff_core::diff(
                &a.left,
                &a.right,
                &a.format,
                a.ignore_case,
                a.mask_secrets,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "left": { "type": "string", "description": "The first (old/base) config file contents. Accepts .env or .ini/.conf text: KEY=VALUE or 'key = value'/'key: value' pairs, # and ; comments, blank lines, single/double quotes, inline comments, an 'export ' prefix, and [section] headers (become dotted 'section.key')." },
                    "right": { "type": "string", "description": "The second (new/compared) config file contents, in the same syntax as `left`. Keys only in `right` are reported as added, keys only in `left` as removed, and keys in both with different values as changed." },
                    "format": { "type": "string", "enum": ["auto", "env", "ini"], "default": "auto", "description": "How to parse both files: 'auto' (default) uses INI parsing if either file has a [section] header, otherwise env; 'env' forces flat KEY=VALUE (dotenv); 'ini' forces [section]-aware parsing with dotted section.key names." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare key names case-insensitively, so DB_HOST and db_host are treated as the same key. Default false (case-sensitive)." },
                    "mask_secrets": { "type": "boolean", "default": false, "description": "Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in the output so a diff can be shared safely. Default false (show raw values)." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output form: 'report' (default) a grouped human summary with Added/Removed/Changed/Unchanged sections and counts; 'json' a structured object { format, summary, added, removed, changed, unchanged }." }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
