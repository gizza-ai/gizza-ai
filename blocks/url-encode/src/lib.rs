//! gizza-ai/url-encode — percent-encodes / decodes text and URLs.
//!
//! Thin chat-skill wrapper around `gizza-ai-url-encode-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI);
//! the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    target: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text or URL to encode or decode."),
        )
        .param(
            Param::enumv("mode", ["encode", "decode"])
                .default("encode")
                .describe("Direction: 'encode' (default) percent-encodes, 'decode' reverses it."),
        )
        .param(
            Param::enumv("target", ["component", "uri"])
                .default("component")
                .describe("Encode mode only: 'component' (default) escapes reserved chars for a single value; 'uri' preserves URL delimiters. Ignored when decoding."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct UrlEncode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-encode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "URL Encode skill",
    skill(
        description = "Percent-encode or percent-decode text and URLs. Use mode='encode' (default) to make text URL-safe or mode='decode' to reverse it. When encoding, target='component' (default) escapes everything for a single query value or path segment (e.g. 'São Paulo' -> 'S%C3%A3o%20Paulo'); target='uri' encodes a whole URL while preserving its delimiters (: / ? # & = etc.).",
        parameters = schema_json()
    )
)]
impl UrlEncode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — url-encode's
        // existing success shape — and routes errors through GuestResult::error.
        match run_skill(&body, "url-encode", |a: Args| {
            gizza_ai_url_encode_core::convert(&a.text, &a.mode, &a.target)
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

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// now emits `additionalProperties: false` uniformly, which url-encode's
    /// authored schema already had — so this is an exact match.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text or URL to encode or decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) percent-encodes, 'decode' reverses it." },
                    "target": { "type": "string", "enum": ["component", "uri"], "default": "component", "description": "Encode mode only: 'component' (default) escapes reserved chars for a single value; 'uri' preserves URL delimiters. Ignored when decoding." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
