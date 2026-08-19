//! gizza-ai/rtf-to-markdown — convert Rich Text Format source into Markdown.
//!
//! Thin chat-skill wrapper around `gizza-ai-rtf-to-markdown-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    rtf: String,
    #[serde(default)]
    headings: String,
    #[serde(default)]
    tables: String,
    #[serde(default)]
    underline: String,
    #[serde(default = "default_true")]
    links: bool,
    #[serde(default = "default_true")]
    escape_markdown: bool,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("rtf")
                .required()
                .describe("The RTF document source to convert. Paste text that begins with an RTF header such as {\\rtf1\\ansi ...}."),
        )
        .param(
            Param::enumv("headings", ["auto", "off"])
                .default("auto")
                .describe("How to handle heading styles: 'auto' (default) detects outline levels and stylesheet names such as heading 1; 'off' renders every paragraph as body text."),
        )
        .param(
            Param::enumv("tables", ["markdown", "text"])
                .default("markdown")
                .describe("How to render RTF tables: 'markdown' (default) emits GitHub pipe tables; 'text' emits tab-separated rows."),
        )
        .param(
            Param::enumv("underline", ["html", "ignore"])
                .default("html")
                .describe("How to handle underlined text: 'html' (default) wraps runs in <u>...</u>; 'ignore' keeps the text without underline markup."),
        )
        .param(
            Param::boolean("links")
                .default(true)
                .describe("When true (default), RTF HYPERLINK fields become Markdown links. When false, only the visible link text is kept."),
        )
        .param(
            Param::boolean("escape_markdown")
                .default(true)
                .describe("When true (default), literal Markdown punctuation in the RTF text is backslash-escaped so it renders as text. When false, punctuation is passed through."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rtf-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert RTF documents to clean Markdown.",
    skill(
        description = "Convert pasted Rich Text Format (RTF) source into clean Markdown or tab-separated plain text. Preserves Markdown-expressible structure: bold, italic, strikethrough, underlines as HTML, superscript/subscript, headings from outline/style metadata, bullet and numbered lists, hyperlinks, Unicode escapes, and simple tables. Options control heading detection, table rendering, underline handling, hyperlink conversion, and Markdown escaping. The input must be raw RTF text beginning with {\\rtf.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rtf-to-markdown", |a: Args| {
            gizza_ai_rtf_to_markdown_core::rtf_to_markdown(
                &a.rtf,
                &a.headings,
                &a.tables,
                &a.underline,
                a.links,
                a.escape_markdown,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "rtf": { "type": "string", "description": "The RTF document source to convert. Paste text that begins with an RTF header such as {\\rtf1\\ansi ...}." },
                    "headings": { "type": "string", "enum": ["auto", "off"], "default": "auto", "description": "How to handle heading styles: 'auto' (default) detects outline levels and stylesheet names such as heading 1; 'off' renders every paragraph as body text." },
                    "tables": { "type": "string", "enum": ["markdown", "text"], "default": "markdown", "description": "How to render RTF tables: 'markdown' (default) emits GitHub pipe tables; 'text' emits tab-separated rows." },
                    "underline": { "type": "string", "enum": ["html", "ignore"], "default": "html", "description": "How to handle underlined text: 'html' (default) wraps runs in <u>...</u>; 'ignore' keeps the text without underline markup." },
                    "links": { "type": "boolean", "default": true, "description": "When true (default), RTF HYPERLINK fields become Markdown links. When false, only the visible link text is kept." },
                    "escape_markdown": { "type": "boolean", "default": true, "description": "When true (default), literal Markdown punctuation in the RTF text is backslash-escaped so it renders as text. When false, punctuation is passed through." }
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
