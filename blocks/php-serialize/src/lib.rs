//! gizza-ai/php-serialize — converts JSON into PHP's serialize() wire format.
//!
//! Thin chat-skill wrapper around `gizza-ai-php-serialize-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("json")
            .required()
            .describe("The JSON value to convert to a PHP serialize() string. Any JSON type works: null, booleans, numbers, strings, arrays, and objects (objects and arrays both become PHP arrays)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PhpSerialize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/php-serialize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "PHP serialize() encoder",
    skill(
        description = "Convert JSON data into PHP's serialize() wire format so it can be read by PHP's unserialize() — useful for writing PHP sessions, caches, or WordPress options from a non-PHP system. Mapping: null -> 'N;', true/false -> 'b:1;'/'b:0;', whole numbers -> 'i:N;', fractional numbers -> 'd:N;', strings -> 's:<byte-length>:\"...\";' (length is in bytes, not characters), and both JSON arrays and objects -> PHP arrays ('a:<count>:{...}'), keyed by index or by string key respectively. Object key order is preserved.",
        parameters = schema_json()
    ),
)]
impl PhpSerialize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "php-serialize", |a: Args| {
            gizza_ai_php_serialize_core::json_to_php(&a.json).map_err(SkillError::InvalidArgs)
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON value to convert to a PHP serialize() string. Any JSON type works: null, booleans, numbers, strings, arrays, and objects (objects and arrays both become PHP arrays)." }
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
