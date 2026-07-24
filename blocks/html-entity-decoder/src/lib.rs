//! gizza-ai/html-entity-decoder — decodes HTML character entities into their
//! literal characters. Thin chat-skill wrapper around
//! `gizza-ai-html-entity-decoder-core`. The chat schema is single-sourced from
//! `descriptor()` (shared with the CLI); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    unknown: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text containing HTML character entities to decode, e.g. 'Fish &amp; Chips &mdash; &#169; 2026'."),
        )
        .param(
            Param::enumv("unknown", ["keep", "error"])
                .default("keep")
                .describe("How to handle a reference that can't be decoded (an unrecognized named entity or a malformed numeric reference). 'keep' (default) leaves it exactly as written; 'error' fails and names the offender."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-entity-decoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode HTML character entities into literal characters.",
    skill(
        description = "Decode HTML character entities into their literal characters. Handles the full HTML5 named entity set (&amp; &copy; &mdash; &hellip; &nbsp; &trade; …), decimal numeric references (&#169;), and hexadecimal ones (&#xA9;), including the WHATWG rules: legacy entities that omit the trailing ';' (&copy → ©) and the Windows-1252 remap for code points 128–159 (so &#151; → an em dash and &#128; → €). Invalid code points become the replacement character U+FFFD. By default (unknown='keep') any reference it can't decode is left exactly as written; set unknown='error' to fail on the first unknown or malformed reference instead. To go the other way (turn characters INTO entities) use the string-escaper tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-entity-decoder", |a: Args| {
            gizza_ai_html_entity_decoder_core::decode(&a.text, &a.unknown)
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
                    "text": { "type": "string", "description": "The text containing HTML character entities to decode, e.g. 'Fish &amp; Chips &mdash; &#169; 2026'." },
                    "unknown": { "type": "string", "enum": ["keep", "error"], "default": "keep", "description": "How to handle a reference that can't be decoded (an unrecognized named entity or a malformed numeric reference). 'keep' (default) leaves it exactly as written; 'error' fails and names the offender." }
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
