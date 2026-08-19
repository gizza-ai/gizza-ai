//! gizza-ai/odt-to-markdown — convert an OpenDocument Text (`.odt`) file into
//! clean Markdown (or plain text).
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::convert` (zip + quick-xml over the ODF `content.xml`) → flat JSON the
//! LLM reads directly (title, creator, counts, content).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary file input with text output fits
//! neither the pure-text page nor the ffmpeg file→media page shape — the
//! no-page file-input pattern, like epub-to-markdown / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_odt_to_markdown_core::{convert, Mode};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB — ODT files are usually small
const MAX_OUTPUT_CHARS: usize = 2_000_000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "markdown".to_string()
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    creator: Option<String>,
    format: String,
    content: String,
    chars: usize,
    paragraphs: usize,
    tables: usize,
    images: usize,
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::enumv("format", ["markdown", "text"])
            .default("markdown")
            .describe("Output format: markdown (default — keeps headings, bold/italic, lists, tables, links and footnotes) or text (plain text, markup stripped)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct OdtToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/odt-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an OpenDocument Text (.odt) file to Markdown or plain text",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert an OpenDocument Text document (.odt, from LibreOffice/OpenOffice Writer) into clean Markdown (default) or plain text. Keeps headings (from the ODF outline level), bold and italic runs, ordered/unordered and nested lists, tables (as GitHub-flavored Markdown), hyperlinks, images and footnotes; comments and tracked changes are omitted. Flat OpenDocument XML (.fodt) is accepted too. Returns the document title and author from the file's metadata, the converted content, and counts of paragraphs, tables and images. Provide the file as url (HTTP/HTTPS) or ref from a prior tool call. Runs locally.",
        parameters = schema_json()
    ),
)]
impl OdtToMarkdown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn clip_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() > max_chars {
        (text.chars().take(max_chars).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("odt-to-markdown")?;
    let mode = Mode::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let doc = convert(&bytes, mode).map_err(SkillError::InvalidArgs)?;
    let (content, truncated) = clip_chars(&doc.content, MAX_OUTPUT_CHARS);
    let chars = content.chars().count();

    let resp = Resp {
        title: doc.title,
        creator: doc.creator,
        format: args.format,
        content,
        chars,
        paragraphs: doc.paragraphs,
        tables: doc.tables,
        images: doc.images,
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize odt-to-markdown response: {e}")))
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
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["markdown", "text"], "default": "markdown", "description": "Output format: markdown (default — keeps headings, bold/italic, lists, tables, links and footnotes) or text (plain text, markup stripped)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn clips_long_content() {
        let (out, truncated) = clip_chars("abcdef", 3);
        assert_eq!(out, "abc");
        assert!(truncated);
        let (out, truncated) = clip_chars("abc", 3);
        assert_eq!(out, "abc");
        assert!(!truncated);
    }
}
