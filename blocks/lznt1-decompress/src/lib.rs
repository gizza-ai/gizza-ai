//! gizza-ai/lznt1-decompress — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    input_encoding: String,
    #[serde(default)]
    output_encoding: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The LZNT1-compressed blob, encoded per input_encoding (hex or base64). This is the raw output of Windows RtlCompressBuffer with COMPRESSION_FORMAT_LZNT1 — e.g. a compressed registry-hive cell, hibernation-file page, or malware config."),
        )
        .param(
            Param::enumv("input_encoding", ["hex", "base64"])
                .default("hex")
                .describe("How the compressed blob in 'data' is encoded: 'hex' (default; whitespace and a 0x prefix are ignored) or 'base64'."),
        )
        .param(
            Param::enumv("output_encoding", ["hex", "text", "base64"])
                .default("hex")
                .describe("How to render the decompressed bytes: 'hex' (default; safe for binary), 'text' (UTF-8 — errors if the output isn't valid UTF-8), or 'base64'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lznt1-decompress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decompress LZNT1 (RtlCompressBuffer) blobs to hex, text, or Base64.",
    skill(
        description = "Decompress an LZNT1 blob — the legacy compression format produced by Windows RtlCompressBuffer / RtlDecompressBuffer with COMPRESSION_FORMAT_LZNT1, used in NTFS compressed files, registry hives, hibernation files, and many malware configuration blobs. Provide the compressed bytes in 'data' as hex (default) or base64 via input_encoding, and choose output_encoding='hex' (default, binary-safe), 'text' (UTF-8), or 'base64' to view the recovered original data. Pure decoder of the chunk/flag-group/back-reference wire format; no host calls.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "lznt1-decompress", |a: Args| {
            gizza_ai_lznt1_decompress_core::run(&a.data, &a.input_encoding, &a.output_encoding)
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
                    "data": { "type": "string", "description": "The LZNT1-compressed blob, encoded per input_encoding (hex or base64). This is the raw output of Windows RtlCompressBuffer with COMPRESSION_FORMAT_LZNT1 — e.g. a compressed registry-hive cell, hibernation-file page, or malware config." },
                    "input_encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the compressed blob in 'data' is encoded: 'hex' (default; whitespace and a 0x prefix are ignored) or 'base64'." },
                    "output_encoding": { "type": "string", "enum": ["hex", "text", "base64"], "default": "hex", "description": "How to render the decompressed bytes: 'hex' (default; safe for binary), 'text' (UTF-8 — errors if the output isn't valid UTF-8), or 'base64'." }
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
