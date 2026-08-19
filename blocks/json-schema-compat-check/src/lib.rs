//! gizza-ai/json-schema-compat-check — chat skill block on the shared tool abstraction.
//!
//! Compares an old JSON Schema with a proposed new schema and reports whether
//! the change is compatible for consumers, producers, or both. The chat schema
//! is single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! delegates to `block_utils::run_skill` and the pure core comparator.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    old_schema: String,
    new_schema: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default)]
    strict_required: bool,
}

fn default_direction() -> String {
    "both".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("old_schema")
                .required()
                .describe("Current or old JSON Schema document. Paste draft-7-style JSON; this is the schema existing data/producers already satisfy."),
        )
        .param(
            Param::string("new_schema")
                .required()
                .describe("Proposed new JSON Schema document to compare against old_schema."),
        )
        .param(
            Param::enumv("direction", ["both", "consumer", "producer"])
                .default("both")
                .describe("Compatibility question to answer. consumer/backward checks whether the new schema still accepts old data. producer/forward checks whether new data remains acceptable to old consumers. both reports both sides."),
        )
        .param(
            Param::boolean("strict_required")
                .default(false)
                .describe("When true, treat any required-field set change as breaking in the relevant direction. When false, added or removed required fields are still reported but with the checker's default direction-aware severity."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-schema-compat-check",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare old and new JSON Schemas for consumer and producer compatibility.",
    skill(
        description = "Compare an old JSON Schema with a proposed new schema and report compatibility findings. Consumer/backward compatibility asks whether the new schema still accepts data that satisfied the old schema. Producer/forward compatibility asks whether data produced for the new schema will still be accepted by consumers using the old schema. The checker handles common draft-7 keywords such as type, enum, const, required, properties, items, additionalProperties, numeric and string bounds, and reports warnings for composition or pattern changes it cannot prove.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "json-schema-compat-check", |a: Args| {
            gizza_ai_json_schema_compat_check_core::run(
                &a.old_schema,
                &a.new_schema,
                &a.direction,
                a.strict_required,
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
                    "old_schema": { "type": "string", "description": "Current or old JSON Schema document. Paste draft-7-style JSON; this is the schema existing data/producers already satisfy." },
                    "new_schema": { "type": "string", "description": "Proposed new JSON Schema document to compare against old_schema." },
                    "direction": { "type": "string", "enum": ["both","consumer","producer"], "default": "both", "description": "Compatibility question to answer. consumer/backward checks whether the new schema still accepts old data. producer/forward checks whether new data remains acceptable to old consumers. both reports both sides." },
                    "strict_required": { "type": "boolean", "default": false, "description": "When true, treat any required-field set change as breaking in the relevant direction. When false, added or removed required fields are still reported but with the checker's default direction-aware severity." }
                },
                "required": ["old_schema", "new_schema"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no chat-schema drift");
    }

    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(
                !p.description.is_empty(),
                "param {} needs .describe()",
                p.name
            );
        }
    }
}
