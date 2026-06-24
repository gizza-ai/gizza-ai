//! gizza-ai/php-unserialize — parses a PHP serialize() string and renders it as
//! readable JSON.
//!
//! Thin chat-skill wrapper around `gizza-ai-php-unserialize-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox. This is the inverse of the `php-serialize`
//! tool (JSON -> PHP); here we go PHP -> JSON.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    serialized: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("serialized")
            .required()
            .describe("The PHP serialize() string to decode, e.g. 'a:1:{s:4:\"name\";s:2:\"Al\";}'. Supports all PHP serialize types: N (null), b (bool), i (int), d (double), s (string, byte-length), a (array) and O (object). The result is rendered as pretty-printed JSON."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PhpUnserialize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/php-unserialize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "PHP serialize() decoder to JSON",
    skill(
        description = "Decode a PHP serialize() string (as produced by PHP's serialize(), or read from a PHP session, cache, or WordPress wp_options row) into readable JSON. Mapping: 'N;' -> null, 'b:1;'/'b:0;' -> true/false, 'i:N;' -> integer, 'd:N;' -> number, 's:<byte-len>:\"...\";' -> string (length is in bytes), 'a:<count>:{...}' -> a JSON array when keyed by the sequential integers 0..n, otherwise a JSON object, and 'O:<len>:\"Class\":<count>:{...}' (a serialized object) -> a JSON object with the class name under a '__class' field. Output is pretty-printed JSON. Use this to inspect PHP-serialized data from a non-PHP system; the inverse of php-serialize.",
        parameters = schema_json()
    ),
)]
impl PhpUnserialize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "php-unserialize", |a: Args| {
            gizza_ai_php_unserialize_core::php_to_json(&a.serialized).map_err(SkillError::InvalidArgs)
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
                    "serialized": { "type": "string", "description": "The PHP serialize() string to decode, e.g. 'a:1:{s:4:\"name\";s:2:\"Al\";}'. Supports all PHP serialize types: N (null), b (bool), i (int), d (double), s (string, byte-length), a (array) and O (object). The result is rendered as pretty-printed JSON." }
                },
                "required": ["serialized"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
