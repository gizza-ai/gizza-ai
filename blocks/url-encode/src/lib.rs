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
    /// Convert each line of `text` independently (default false).
    #[serde(default)]
    per_line: bool,
    /// Apply the operation this many times; the core clamps to 1..=16 (0 → 1).
    #[serde(default)]
    repeat: u32,
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
            Param::enumv("target", ["component", "uri", "form"])
                .default("component")
                .describe("Encoding style. 'component' (default) escapes reserved chars for a single query value or path segment; 'uri' preserves URL delimiters for a whole URL; 'form' is application/x-www-form-urlencoded (a space becomes '+', and on decode '+' becomes a space)."),
        )
        .param(
            Param::boolean("per_line")
                .default(false)
                .describe("When true, convert each line of the input independently (rejoined with newlines) — for a batch list of values or URLs. Default false."),
        )
        .param(
            // Bounds reference the core clamp (MAX_REPEAT) so the LLM-facing
            // schema can't drift from what `convert` actually enforces.
            Param::integer("repeat")
                .default(1)
                .min(1.0)
                .max(gizza_ai_url_encode_core::MAX_REPEAT as f64)
                .describe("Apply the operation this many times, 1-16. Use >1 to un-nest multiply-encoded input when decoding (or to double-encode). Default 1."),
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
        description = "Percent-encode or percent-decode text and URLs. Use mode='encode' (default) to make text URL-safe or mode='decode' to reverse it. When encoding, target='component' (default) escapes everything for a single query value or path segment (e.g. 'São Paulo' -> 'S%C3%A3o%20Paulo'); target='uri' encodes a whole URL while preserving its delimiters (: / ? # & = etc.); target='form' is application/x-www-form-urlencoded, where a space becomes '+' (and decodes back). Set per_line=true to convert each line of a batch list independently. Set repeat>1 (up to 16) to un-nest multiply-encoded input when decoding, or to double-encode.",
        parameters = schema_json()
    )
)]
impl UrlEncode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } — url-encode's
        // existing success shape — and routes errors through GuestResult::error.
        match run_skill(&body, "url-encode", |a: Args| {
            gizza_ai_url_encode_core::convert(&a.text, &a.mode, &a.target, a.per_line, a.repeat)
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
    /// reviewed. Regenerated 2026-06-20 by `/improve-tool` when `target` gained
    /// `form` and the `per_line` / `repeat` params were added.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text or URL to encode or decode." },
                    "mode": { "type": "string", "enum": ["encode", "decode"], "default": "encode", "description": "Direction: 'encode' (default) percent-encodes, 'decode' reverses it." },
                    "target": { "type": "string", "enum": ["component", "uri", "form"], "default": "component", "description": "Encoding style. 'component' (default) escapes reserved chars for a single query value or path segment; 'uri' preserves URL delimiters for a whole URL; 'form' is application/x-www-form-urlencoded (a space becomes '+', and on decode '+' becomes a space)." },
                    "per_line": { "type": "boolean", "default": false, "description": "When true, convert each line of the input independently (rejoined with newlines) — for a batch list of values or URLs. Default false." },
                    "repeat": { "type": "integer", "minimum": 1, "maximum": 16, "default": 1, "description": "Apply the operation this many times, 1-16. Use >1 to un-nest multiply-encoded input when decoding (or to double-encode). Default 1." }
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
