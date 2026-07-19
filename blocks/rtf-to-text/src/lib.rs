//! gizza-ai/rtf-to-text — strip RTF control words and groups to plain Unicode
//! text. Thin chat-skill wrapper around `gizza-ai-rtf-to-text-core`; the chat
//! schema is single-sourced from `descriptor()` (shared with the CLI) and the
//! handler delegates to `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    rtf: String,
    #[serde(default)]
    line_breaks: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("rtf")
                .required()
                .describe("The RTF document source — the raw {\\rtf … } markup (as you'd see if you opened a .rtf file in a text editor). Control words, groups, and formatting are stripped; the visible text is returned."),
        )
        .param(
            Param::enumv("line_breaks", ["preserve", "collapse"])
                .default("preserve")
                .describe("How to render paragraph and line breaks. 'preserve' (default) turns \\par/\\line/\\sect into newlines and \\tab/\\cell into tabs, keeping the document's layout. 'collapse' flattens every run of whitespace to a single space, producing one clean line — useful for search indexing or feeding the text to an LLM."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RtfToText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rtf-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert RTF markup to plain Unicode text",
    skill(
        description = "Convert an RTF (Rich Text Format) document to plain text. Pass the raw RTF markup — the {\\rtf … } source — as 'rtf'. Control words (\\b, \\f0, \\fs24, …), font/color tables, stylesheets, metadata, pictures, and \\* ignorable destinations are stripped, leaving only the readable text. \\par/\\line/\\sect become newlines, \\tab/\\cell become tabs, and \\'hh (Windows-1252) and \\uN Unicode escapes are decoded so accents, smart quotes, the euro sign, emoji, and non-Latin scripts survive. line_breaks='preserve' (default) keeps the paragraph layout; line_breaks='collapse' flattens everything to one space-separated line. Returns an error if the input does not begin with {\\rtf.",
        parameters = schema_json()
    ),
)]
impl RtfToText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "rtf-to-text", |a: Args| {
            gizza_ai_rtf_to_text_core::rtf_to_text(&a.rtf, &a.line_breaks)
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
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "rtf": { "type": "string", "description": "The RTF document source — the raw {\\rtf … } markup (as you'd see if you opened a .rtf file in a text editor). Control words, groups, and formatting are stripped; the visible text is returned." },
                    "line_breaks": { "type": "string", "enum": ["preserve", "collapse"], "default": "preserve", "description": "How to render paragraph and line breaks. 'preserve' (default) turns \\par/\\line/\\sect into newlines and \\tab/\\cell into tabs, keeping the document's layout. 'collapse' flattens every run of whitespace to a single space, producing one clean line — useful for search indexing or feeding the text to an LLM." }
                },
                "required": ["rtf"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
