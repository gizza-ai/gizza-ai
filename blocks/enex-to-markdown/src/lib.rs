//! gizza-ai/enex-to-markdown — convert an Evernote ENEX export into Markdown.
//!
//! Thin chat-skill wrapper around `gizza-ai-enex-to-markdown-core` (quick-xml +
//! htmd). The chat schema is single-sourced from `descriptor()` (chat + CLI +
//! page query-params); the handler delegates to `block_utils::run_skill`. Pure —
//! runs entirely inside the WASM sandbox (all backends, incl. the chat Service
//! Worker).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_enex_to_markdown_core::{convert, Format, Metadata, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_OUTPUT_CHARS: usize = 2_000_000;

#[derive(Deserialize)]
struct Args {
    enex: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_metadata")]
    metadata: String,
    #[serde(default = "default_attachments")]
    attachments: bool,
}

fn default_format() -> String {
    "markdown".to_string()
}
fn default_metadata() -> String {
    "frontmatter".to_string()
}
fn default_attachments() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    notes: usize,
    format: String,
    content: String,
    chars: usize,
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("enex")
                .required()
                .describe("The ENEX export XML (an Evernote `.enex` file's contents) to convert."),
        )
        .param(
            Param::enumv("format", ["markdown", "text"])
                .default("markdown")
                .describe("Output format: markdown (default, keeps headings/lists/links) or text (plain text)."),
        )
        .param(
            Param::enumv("metadata", ["frontmatter", "inline", "none"])
                .default("frontmatter")
                .describe("Where each note's title/dates/tags/source URL go: frontmatter (YAML block, default), inline (heading + italic line + #hashtags), or none (title only)."),
        )
        .param(
            Param::boolean("attachments")
                .default(true)
                .describe("List each note's attachments (filename, MIME type, decoded size). Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run(a: Args) -> Result<Resp, SkillError> {
    let opts = Options {
        format: Format::parse(&a.format).map_err(SkillError::InvalidArgs)?,
        metadata: Metadata::parse(&a.metadata).map_err(SkillError::InvalidArgs)?,
        attachments: a.attachments,
    };
    let conv = convert(&a.enex, opts).map_err(SkillError::InvalidArgs)?;
    let (content, truncated) = if conv.content.chars().count() > MAX_OUTPUT_CHARS {
        (conv.content.chars().take(MAX_OUTPUT_CHARS).collect(), true)
    } else {
        (conv.content, false)
    };
    let chars = content.chars().count();
    Ok(Resp {
        notes: conv.notes,
        format: a.format,
        content,
        chars,
        truncated,
    })
}

#[cfg(target_arch = "wasm32")]
struct EnexToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/enex-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an Evernote ENEX export to Markdown",
    skill(
        description = "Convert an Evernote ENEX export (`.enex` XML) into clean Markdown (default) or plain text. Each note's ENML/HTML body becomes Markdown (headings, links, lists, code, tables, emphasis); the title becomes a heading and multiple notes are joined by horizontal rules. `metadata` controls where each note's created/updated dates, tags, and source URL go (frontmatter YAML, an inline line with #hashtags, or none). `attachments` lists each note's resources (filename, MIME, decoded size) — the binary payloads are not emitted. Pass the ENEX contents as `enex`. Runs locally.",
        parameters = schema_json()
    ),
)]
impl EnexToMarkdown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "enex-to-markdown", run) {
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
                    "enex": { "type": "string", "description": "The ENEX export XML (an Evernote `.enex` file's contents) to convert." },
                    "format": { "type": "string", "enum": ["markdown", "text"], "default": "markdown", "description": "Output format: markdown (default, keeps headings/lists/links) or text (plain text)." },
                    "metadata": { "type": "string", "enum": ["frontmatter", "inline", "none"], "default": "frontmatter", "description": "Where each note's title/dates/tags/source URL go: frontmatter (YAML block, default), inline (heading + italic line + #hashtags), or none (title only)." },
                    "attachments": { "type": "boolean", "default": true, "description": "List each note's attachments (filename, MIME type, decoded size). Default true." }
                },
                "required": ["enex"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_converts_and_reports_notes() {
        let enex = r#"<en-export><note><title>N</title><content><![CDATA[<en-note><p>Hi</p></en-note>]]></content><created>20230101T090000Z</created></note></en-export>"#;
        let out = run(Args {
            enex: enex.to_string(),
            format: "markdown".into(),
            metadata: "none".into(),
            attachments: true,
        })
        .unwrap();
        assert_eq!(out.notes, 1);
        assert!(out.content.contains("# N"));
        assert!(out.content.contains("Hi"));
        assert!(!out.truncated);
    }

    #[test]
    fn run_rejects_bad_format() {
        let err = run(Args {
            enex: "<en-export><note></note></en-export>".into(),
            format: "pdf".into(),
            metadata: "none".into(),
            attachments: false,
        });
        assert!(err.is_err());
    }
}
