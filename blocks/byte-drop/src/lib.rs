//! gizza-ai/byte-drop — remove a contiguous byte range (start offset + length)
//! from text, hex, or Base64 input and return the remainder. Chat schema
//! single-sourced from descriptor() (shared shape across chat + CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    start: i64,
    #[serde(default)]
    length: i64,
    #[serde(default)]
    format: String,
    #[serde(default)]
    output: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The data to remove bytes from — UTF-8 text, a hex byte string, or Base64 (set by 'format')."),
        )
        .param(
            Param::integer("start")
                .default(0)
                .describe("0-based byte offset of the first byte to remove (inclusive). Negative counts from the end (-1 is the last byte). Clamped to the buffer. Default 0."),
        )
        .param(
            Param::integer("length")
                .default(0)
                .describe("How many bytes to remove starting at 'start'. 0 removes nothing; a range past the end is clamped. Must be zero or positive. Default 0."),
        )
        .param(
            Param::enumv("format", ["text", "hex", "base64"])
                .default("text")
                .describe("How to interpret 'input'. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c'); 'base64' is standard Base64. Offsets and lengths are counted in raw bytes, not characters."),
        )
        .param(
            Param::enumv("output", ["text", "hex", "base64"])
                .default("text")
                .describe("How to render the remaining bytes. 'text' (default) is UTF-8 (errors if the remainder isn't valid UTF-8); 'hex' and 'base64' show the raw bytes — use them for binary data."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/byte-drop",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove a byte range from text/hex/base64",
    skill(
        description = "Remove a contiguous range of bytes from data and return the remainder. Decodes 'input' per format ('text' UTF-8, 'hex' byte string, or 'base64'), then deletes 'length' bytes starting at byte offset 'start' (0-based, inclusive). start may be negative to count from the end (-1 is the last byte); start and length are clamped to the buffer, and length=0 removes nothing. The remaining bytes are rendered per 'output' ('text', 'hex', or 'base64'). Offsets and lengths are raw bytes, not characters. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "byte-drop", |a: Args| {
            gizza_ai_byte_drop_core::drop_bytes(&a.input, a.start, a.length, &a.format, &a.output)
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The data to remove bytes from — UTF-8 text, a hex byte string, or Base64 (set by 'format')." },
                    "start": { "type": "integer", "default": 0, "description": "0-based byte offset of the first byte to remove (inclusive). Negative counts from the end (-1 is the last byte). Clamped to the buffer. Default 0." },
                    "length": { "type": "integer", "default": 0, "description": "How many bytes to remove starting at 'start'. 0 removes nothing; a range past the end is clamped. Must be zero or positive. Default 0." },
                    "format": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to interpret 'input'. 'text' (default) is UTF-8; 'hex' is a hex byte string (e.g. '48 65 6c' or '0x48656c'); 'base64' is standard Base64. Offsets and lengths are counted in raw bytes, not characters." },
                    "output": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to render the remaining bytes. 'text' (default) is UTF-8 (errors if the remainder isn't valid UTF-8); 'hex' and 'base64' show the raw bytes — use them for binary data." }
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
