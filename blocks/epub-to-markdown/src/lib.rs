//! gizza-ai/epub-to-markdown — extract an EPUB's chapters into clean Markdown
//! (or plain text).
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::convert` (zip + OPF spine + htmd/nanohtml2text) → flat JSON the LLM
//! reads directly (title, chapter count, content).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary file input with text output fits
//! neither the pure-text page nor the ffmpeg file→media page shape — the
//! no-page file-input pattern, like pdf-extract-text / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_epub_to_markdown_core::{convert, Mode};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB — EPUBs are usually small
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
    chapters: usize,
    format: String,
    content: String,
    chars: usize,
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::enumv("format", ["markdown", "text"])
            .default("markdown")
            .describe("Output format: markdown (default, keeps headings/lists/links) or text (plain text)."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EpubToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/epub-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract an EPUB's chapters into Markdown or plain text",
    requires = ["wafer-run/network"],
    skill(
        description = "Extract an EPUB e-book's chapters into clean Markdown (default) or plain text, in reading (spine) order. Reads the EPUB's OPF manifest so chapters come out in the right order. Returns the book title, the number of chapters extracted, and the combined content (chapters separated by a Markdown horizontal rule). Provide the EPUB as url (HTTP/HTTPS) or ref from a prior tool call. Runs locally.",
        parameters = schema_json()
    ),
)]
impl EpubToMarkdown {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("epub-to-markdown")?;
    let mode = Mode::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let book = convert(&bytes, mode).map_err(SkillError::InvalidArgs)?;
    let (content, truncated) = clip_chars(&book.content, MAX_OUTPUT_CHARS);
    let chars = content.chars().count();

    let resp = Resp {
        title: book.title,
        chapters: book.chapters,
        format: args.format,
        content,
        chars,
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize epub-to-markdown response: {e}")))
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
                    "format": { "type": "string", "enum": ["markdown", "text"], "default": "markdown", "description": "Output format: markdown (default, keeps headings/lists/links) or text (plain text)." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
