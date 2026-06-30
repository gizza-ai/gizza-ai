//! gizza-ai/plist-viewer — parse Apple property lists as JSON or a key/value tree.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_plist_viewer_core::{convert, DataEncoding, Format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_indent")]
    indent: u32,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default = "default_data_encoding")]
    data_encoding: String,
}

fn default_format() -> String {
    "json".to_string()
}
fn default_indent() -> u32 {
    2
}
fn default_data_encoding() -> String {
    "base64".to_string()
}

fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "json" => Ok(Format::Json),
        "tree" | "outline" => Ok(Format::Tree),
        other => Err(format!("unknown format '{other}' (use json or tree)")),
    }
}

fn parse_data_encoding(s: &str) -> Result<DataEncoding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "base64" => Ok(DataEncoding::Base64),
        "hex" => Ok(DataEncoding::Hex),
        other => Err(format!("unknown data_encoding '{other}' (use base64 or hex)")),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("XML plist text, or Base64-encoded binary bplist data."),
        )
        .param(
            Param::enumv("format", ["json", "tree"])
                .default("json")
                .describe("Output format: pretty JSON (default) or a plutil-style tree."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .describe("Spaces per indent level, clamped from 0 to 8. Default 2."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Sort dictionary keys alphabetically instead of preserving plist order."),
        )
        .param(
            Param::enumv("data_encoding", ["base64", "hex"])
                .default("base64")
                .describe("How plist <data> byte blobs are rendered in JSON/tree output."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/plist-viewer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "View Apple plist files as JSON or a key/value tree",
    skill(
        description = "Parse an Apple property list (XML plist text or Base64-encoded binary bplist data) and render it as readable JSON or a plutil-style key/value tree. Options: format=json|tree, indent spaces, sort_keys, data_encoding=base64|hex for <data> blobs. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "plist-viewer", |a: Args| {
            let opt = Options {
                format: parse_format(&a.format).map_err(SkillError::InvalidArgs)?,
                indent: a.indent.min(8) as usize,
                sort_keys: a.sort_keys,
                data_encoding: parse_data_encoding(&a.data_encoding)
                    .map_err(SkillError::InvalidArgs)?,
            };
            convert(&a.input, &opt).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "XML plist text, or Base64-encoded binary bplist data." },
                    "format": { "type": "string", "enum": ["json", "tree"], "default": "json", "description": "Output format: pretty JSON (default) or a plutil-style tree." },
                    "indent": { "type": "integer", "default": 2, "description": "Spaces per indent level, clamped from 0 to 8. Default 2." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Sort dictionary keys alphabetically instead of preserving plist order." },
                    "data_encoding": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How plist <data> byte blobs are rendered in JSON/tree output." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
