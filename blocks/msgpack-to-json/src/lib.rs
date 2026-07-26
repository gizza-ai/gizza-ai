//! gizza-ai/msgpack-to-json — decode a MessagePack binary blob (hex or base64)
//! into pretty-printed JSON for inspection.
//!
//! Thin chat-skill wrapper around `gizza-ai-msgpack-to-json-core`. The chat
//! schema is single-sourced from `descriptor()` (shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_format: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default)]
    binary_format: String,
}

fn default_indent() -> u64 {
    2
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The MessagePack bytes, encoded as a hex or base64 string (e.g. from a network capture, a Redis value, or a driver's msgpack output). Several MessagePack values back-to-back are decoded as a JSON array."),
        )
        .param(
            Param::enumv("input_format", ["auto", "hex", "base64"])
                .default("auto")
                .describe("How the input string is encoded: 'auto' (default; detect hex vs base64), 'hex' (whitespace, ':', '-', ',' and any '0x' markers ignored), or 'base64' (standard or URL-safe, padding optional)."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level in the pretty-printed JSON (0-8, default 2). Set 0 to minify to a single compact line."),
        )
        .param(
            Param::enumv("binary_format", ["base64", "hex"])
                .default("base64")
                .describe("How raw MessagePack binary ('bin') values and 'ext' payloads are shown in the JSON, since JSON has no binary type: 'base64' (default) or 'hex'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/msgpack-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode a MessagePack blob (hex or base64) into pretty-printed JSON.",
    skill(
        description = "Decode a MessagePack binary blob into human-readable JSON, entirely in the browser/sandbox. Give the bytes as a hex or base64 string in 'input' (input_format='auto' detects which; force with 'hex' or 'base64'). indent sets spaces per nesting level (0-8, default 2; 0 minifies to one line). Field/element order is preserved and full uint64 precision is kept. MessagePack types JSON cannot hold natively are represented by convention: raw binary ('bin') and unknown extension ('ext') payloads are shown as a base64 (default) or hex string per binary_format — an ext becomes {\"$ext\": <type>, \"data\": \"<encoded>\"}; the reserved timestamp extension (type -1, 4/8/12-byte forms) becomes an RFC 3339 UTC string; non-string map keys are stringified. Several MessagePack values back-to-back are returned as a JSON array. Returns a clear error (with a byte offset) for invalid hex/base64 or malformed MessagePack.",
        parameters = schema_json()
    )
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "msgpack-to-json", |a: Args| {
            gizza_ai_msgpack_to_json_core::run(
                &a.input,
                &a.input_format,
                a.indent as usize,
                &a.binary_format,
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
                    "input": { "type": "string", "description": "The MessagePack bytes, encoded as a hex or base64 string (e.g. from a network capture, a Redis value, or a driver's msgpack output). Several MessagePack values back-to-back are decoded as a JSON array." },
                    "input_format": { "type": "string", "enum": ["auto", "hex", "base64"], "default": "auto", "description": "How the input string is encoded: 'auto' (default; detect hex vs base64), 'hex' (whitespace, ':', '-', ',' and any '0x' markers ignored), or 'base64' (standard or URL-safe, padding optional)." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level in the pretty-printed JSON (0-8, default 2). Set 0 to minify to a single compact line." },
                    "binary_format": { "type": "string", "enum": ["base64", "hex"], "default": "base64", "description": "How raw MessagePack binary ('bin') values and 'ext' payloads are shown in the JSON, since JSON has no binary type: 'base64' (default) or 'hex'." }
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
