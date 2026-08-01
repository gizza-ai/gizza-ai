//! gizza-ai/html-entity-encoder — encodes literal characters into HTML character
//! references. Thin chat-skill wrapper around
//! `gizza-ai-html-entity-encoder-core`. The chat schema is single-sourced from
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
    scope: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to encode into HTML character references, e.g. '<b>Tom & Jerry\\'s café</b>'."),
        )
        .param(
            Param::enumv("scope", ["minimal", "non-ascii", "named"])
                .default("minimal")
                .describe("Which characters to encode. 'minimal' (default) encodes only the five HTML/XML-sensitive characters & < > \" '. 'non-ascii' also encodes every character above U+007F (accents, symbols, emoji). 'named' also encodes every character that has an HTML5 named entity. The five sensitive characters are always encoded."),
        )
        .param(
            Param::enumv("format", ["named", "decimal", "hex"])
                .default("named")
                .describe("How each encoded character is written. 'named' (default) uses the HTML5 named entity where one exists (&amp;, &copy;, &mdash;) and falls back to a decimal numeric reference otherwise. 'decimal' always uses &#NNN;. 'hex' always uses &#xHH;."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-entity-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode text into HTML character entities.",
    skill(
        description = "Encode literal characters into HTML character references (entities). Two orthogonal options control the result. 'scope' selects WHICH characters are encoded: 'minimal' (default) encodes only the five HTML/XML-sensitive characters & < > \" '; 'non-ascii' also encodes every character above U+007F (accents, symbols, emoji) for pure-ASCII output; 'named' also encodes every character that has an HTML5 named entity. 'format' selects HOW each character is written: 'named' (default) uses the HTML5 named entity where available (&amp;, &copy;, &mdash;) and falls back to a decimal numeric reference otherwise; 'decimal' always uses &#NNN;; 'hex' always uses &#xHH;. The apostrophe becomes &apos; in named format and &#39; / &#x27; in the numeric formats. To go the other way (turn entities back into characters) use the html-entity-decoder tool.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-entity-encoder", |a: Args| {
            gizza_ai_html_entity_encoder_core::encode(&a.text, &a.scope, &a.format)
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
                    "text": { "type": "string", "description": "The text to encode into HTML character references, e.g. '<b>Tom & Jerry\\'s café</b>'." },
                    "scope": { "type": "string", "enum": ["minimal", "non-ascii", "named"], "default": "minimal", "description": "Which characters to encode. 'minimal' (default) encodes only the five HTML/XML-sensitive characters & < > \" '. 'non-ascii' also encodes every character above U+007F (accents, symbols, emoji). 'named' also encodes every character that has an HTML5 named entity. The five sensitive characters are always encoded." },
                    "format": { "type": "string", "enum": ["named", "decimal", "hex"], "default": "named", "description": "How each encoded character is written. 'named' (default) uses the HTML5 named entity where one exists (&amp;, &copy;, &mdash;) and falls back to a decimal numeric reference otherwise. 'decimal' always uses &#NNN;. 'hex' always uses &#xHH;." }
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
