//! gizza-ai/hex-byte-inspector — chat skill block on the shared tool abstraction.
//! Reads a value expressed as hex, base64 or text; reports its byte / bit /
//! hex-char length; shows the same bytes converted between hex, base64 and
//! printable text; groups the hex for readability; and (optionally) notes which
//! common cryptographic value sizes match that byte length. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_group_size")]
    group_size: i64,
    #[serde(default)]
    uppercase: bool,
    #[serde(default = "default_interpret")]
    interpret: bool,
}

fn default_input_format() -> String {
    "hex".to_string()
}
fn default_group_size() -> i64 {
    4
}
fn default_interpret() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The value to inspect. Read according to input_format: a hex string (whitespace, ':' '-' ',' '_' and 0x / \\x prefixes are ignored), a Base64 string, or literal UTF-8 text."),
        )
        .param(
            Param::enumv("input_format", ["hex", "base64", "text"])
                .default("hex")
                .describe("How to read 'input' into bytes: 'hex' (default), 'base64', or 'text' (its UTF-8 bytes)."),
        )
        .param(
            Param::integer("group_size")
                .default(4)
                .min(0.0)
                .max(64.0)
                .describe("Bytes per space-separated group in the Hex line. 0 = continuous (no grouping). Default 4. Clamped to 0-64."),
        )
        .param(
            Param::boolean("uppercase")
                .default(false)
                .describe("Uppercase the A-F hex digits in the output. Default false (lowercase)."),
        )
        .param(
            Param::boolean("interpret")
                .default(true)
                .describe("When true (default), append a Matches block listing common cryptographic values whose canonical size equals the byte length (hash digests, AES/ChaCha keys, Ed25519/secp256k1 keys & signatures, IVs)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hex-byte-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a hex/base64/text value: byte, bit & hex-char length, format conversions, and crypto-size hints",
    skill(
        description = "Inspect a value expressed as hex, base64 or text. Set input_format to 'hex' (default), 'base64', or 'text' to choose how 'input' is read into bytes. Reports the byte length, bit length and hex-char count; shows the same bytes converted between hex, base64 and printable text; and groups the hex group_size bytes per group (0 = continuous, default 4). Set uppercase=true for upper-case hex. When interpret=true (default) it appends a Matches block naming common cryptographic values whose canonical size equals that byte length (MD5 16 B, SHA-1 20 B, SHA-256 32 B, SHA-512 64 B, AES-128/192/256 keys, AES-GCM/ChaCha20 nonce 12 B, Ed25519 signature 64 B, secp256k1 pubkeys 33/65 B, etc.). The hex parser tolerates whitespace, ':' '-' ',' '_' delimiters and 0x / \\x prefixes. Returns a plain-text report. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hex-byte-inspector", |a: Args| {
            gizza_ai_hex_byte_inspector_core::inspect(
                &a.input,
                &a.input_format,
                a.group_size,
                a.uppercase,
                a.interpret,
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
                    "input": { "type": "string", "description": "The value to inspect. Read according to input_format: a hex string (whitespace, ':' '-' ',' '_' and 0x / \\x prefixes are ignored), a Base64 string, or literal UTF-8 text." },
                    "input_format": { "type": "string", "enum": ["hex", "base64", "text"], "default": "hex", "description": "How to read 'input' into bytes: 'hex' (default), 'base64', or 'text' (its UTF-8 bytes)." },
                    "group_size": { "type": "integer", "minimum": 0, "maximum": 64, "default": 4, "description": "Bytes per space-separated group in the Hex line. 0 = continuous (no grouping). Default 4. Clamped to 0-64." },
                    "uppercase": { "type": "boolean", "default": false, "description": "Uppercase the A-F hex digits in the output. Default false (lowercase)." },
                    "interpret": { "type": "boolean", "default": true, "description": "When true (default), append a Matches block listing common cryptographic values whose canonical size equals the byte length (hash digests, AES/ChaCha keys, Ed25519/secp256k1 keys & signatures, IVs)." }
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
