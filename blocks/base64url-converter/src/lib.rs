//! gizza-ai/base64url-converter — transcodes between standard Base64 and
//! URL-safe Base64url. Thin chat-skill wrapper around
//! `gizza-ai-base64url-converter-core`; the chat schema is single-sourced from
//! `descriptor()` (shared shape across chat + CLI) and the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    padding: String,
    /// Verify the input decodes as Base64 before converting (default false).
    #[serde(default)]
    validate: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The Base64 or Base64url string to convert. Whitespace (incl. line wraps) is ignored, so line-wrapped MIME Base64 and pasted tokens work."),
        )
        .param(
            Param::enumv("direction", ["auto", "to-url", "to-standard"])
                .default("auto")
                .describe("Which alphabet to output. 'to-url' makes standard Base64 URL-safe (+ -> -, / -> _); 'to-standard' reverses it (- -> +, _ -> /). 'auto' (default) converts to the opposite of the input: if it already contains '-' or '_' it is treated as Base64url and converted to standard, otherwise it is treated as standard and converted to URL-safe."),
        )
        .param(
            Param::enumv("padding", ["auto", "keep", "strip"])
                .default("auto")
                .describe("How the output handles '=' padding. 'auto' (default) pads standard output and leaves Base64url output unpadded (each alphabet's canonical form); 'keep' always pads to a multiple of 4; 'strip' removes all padding."),
        )
        .param(
            Param::boolean("validate")
                .default(false)
                .describe("When true, verify the input actually decodes as Base64 (rejecting a bad length or stray characters) before converting. Default false — a plain alphabet swap does not require valid Base64."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Base64UrlConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/base64url-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between standard Base64 and URL-safe Base64url.",
    skill(
        description = "Convert a string between standard Base64 (which uses + and /) and URL-safe Base64url (which uses - and _, usually without = padding), in either direction. This is a pure alphabet + padding transform, not a decode-to-text: it swaps +/ for -_ (or back) and applies a padding policy. Use direction='to-url' to make Base64 URL/filename/JWT-safe, 'to-standard' to reverse it, or 'auto' (default) to convert to the opposite of the detected input. padding='auto' pads standard output and leaves Base64url output unpadded; 'keep' always pads, 'strip' never does. Whitespace is ignored. Set validate=true to reject input that is not valid Base64.",
        parameters = schema_json()
    ),
)]
impl Base64UrlConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "base64url-converter", |a: Args| {
            gizza_ai_base64url_converter_core::convert(&a.text, &a.direction, &a.padding, a.validate)
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
                    "text": { "type": "string", "description": "The Base64 or Base64url string to convert. Whitespace (incl. line wraps) is ignored, so line-wrapped MIME Base64 and pasted tokens work." },
                    "direction": { "type": "string", "enum": ["auto", "to-url", "to-standard"], "default": "auto", "description": "Which alphabet to output. 'to-url' makes standard Base64 URL-safe (+ -> -, / -> _); 'to-standard' reverses it (- -> +, _ -> /). 'auto' (default) converts to the opposite of the input: if it already contains '-' or '_' it is treated as Base64url and converted to standard, otherwise it is treated as standard and converted to URL-safe." },
                    "padding": { "type": "string", "enum": ["auto", "keep", "strip"], "default": "auto", "description": "How the output handles '=' padding. 'auto' (default) pads standard output and leaves Base64url output unpadded (each alphabet's canonical form); 'keep' always pads to a multiple of 4; 'strip' removes all padding." },
                    "validate": { "type": "boolean", "default": false, "description": "When true, verify the input actually decodes as Base64 (rejecting a bad length or stray characters) before converting. Default false — a plain alphabet swap does not require valid Base64." }
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
