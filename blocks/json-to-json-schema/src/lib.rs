//! gizza-ai/json-to-json-schema — infer a JSON Schema from one or more JSON examples.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_to_json_schema_core::{infer, Draft, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_draft")]
    draft: String,
    #[serde(default)]
    additional_properties: bool,
    #[serde(default = "default_true")]
    required: bool,
    #[serde(default = "default_true")]
    detect_formats: bool,
    #[serde(default)]
    title: String,
}
fn default_draft() -> String {
    "2020-12".to_string()
}
fn default_true() -> bool {
    true
}

fn draft_from(s: &str) -> Draft {
    match s.trim() {
        "draft-07" | "draft7" | "07" | "7" => Draft::Draft07,
        _ => Draft::Draft2020,
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("A JSON object or array sample to infer a schema from. If the root is an array, its elements are merged so keys missing in some become optional and differing types become unions."),
        )
        .param(
            Param::enumv("draft", ["2020-12", "draft-07"])
                .default("2020-12")
                .describe("JSON Schema dialect to emit. '2020-12' (default) or 'draft-07'. Affects the $schema URL and whether a 'uuid' format is emitted (Draft-07 has no uuid format)."),
        )
        .param(
            Param::boolean("additional_properties")
                .default(false)
                .describe("Allow properties beyond those seen. false (default) emits 'additionalProperties: false' on objects for strict validation; true omits it (permissive)."),
        )
        .param(
            Param::boolean("required")
                .default(true)
                .describe("List every key present in all merged samples of an object under 'required'. Default true."),
        )
        .param(
            Param::boolean("detect_formats")
                .default(true)
                .describe("Detect string 'format' hints (email, uri, date-time, date, uuid, ipv4). Default true."),
        )
        .param(
            Param::string("title")
                .default("")
                .describe("Optional 'title' for the root schema. Empty (default) omits it."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonToJsonSchema;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-to-json-schema",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer a JSON Schema from JSON examples",
    skill(
        description = "Infer a JSON Schema (Draft 2020-12 or Draft-07) from a pasted JSON object or array sample. Nested objects/arrays are inferred recursively; when the root (or a nested array) holds multiple elements they are merged so keys missing in some become optional and differing types become unions (e.g. [\"integer\",\"string\"]). Options: draft (2020-12 default, or draft-07), additional_properties (false default = strict, emits additionalProperties:false), required (true default, lists keys seen in all samples), detect_formats (true default, tags strings as email/uri/date-time/date/uuid/ipv4), and an optional root title. Returns the pretty-printed schema.",
        parameters = schema_json()
    ),
)]
impl JsonToJsonSchema {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-to-json-schema", |a: Args| {
            let opts = Options {
                draft: draft_from(&a.draft),
                additional_properties: a.additional_properties,
                required: a.required,
                detect_formats: a.detect_formats,
                title: a.title,
            };
            infer(&a.json, &opts).map_err(SkillError::InvalidArgs)
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
                    "json":                  { "type": "string", "description": "A JSON object or array sample to infer a schema from. If the root is an array, its elements are merged so keys missing in some become optional and differing types become unions." },
                    "draft":                 { "type": "string", "enum": ["2020-12", "draft-07"], "default": "2020-12", "description": "JSON Schema dialect to emit. '2020-12' (default) or 'draft-07'. Affects the $schema URL and whether a 'uuid' format is emitted (Draft-07 has no uuid format)." },
                    "additional_properties": { "type": "boolean", "default": false, "description": "Allow properties beyond those seen. false (default) emits 'additionalProperties: false' on objects for strict validation; true omits it (permissive)." },
                    "required":              { "type": "boolean", "default": true, "description": "List every key present in all merged samples of an object under 'required'. Default true." },
                    "detect_formats":        { "type": "boolean", "default": true, "description": "Detect string 'format' hints (email, uri, date-time, date, uuid, ipv4). Default true." },
                    "title":                 { "type": "string", "default": "", "description": "Optional 'title' for the root schema. Empty (default) omits it." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
