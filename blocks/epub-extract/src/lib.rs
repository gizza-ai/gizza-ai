//! gizza-ai/epub-extract — extract an EPUB's chapter structure + metadata.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::extract` (zip + OPF spine + NCX/nav TOC + nanohtml2text) → flat JSON
//! the LLM reads directly: book metadata plus an ordered list of chapters, each
//! with its title, word count, and (optionally) readable text.
//!
//! Distinct from `epub-to-markdown`, which returns ONE concatenated Markdown/text
//! blob + a chapter count. This returns navigable per-chapter structure so a
//! caller can search, summarize, or quote an individual chapter.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a binary file input with structured JSON
//! output fits neither the pure-text page nor the ffmpeg file→media page shape —
//! the no-page file-input pattern, like epub-to-markdown / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_epub_extract_core::extract;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024; // 32 MiB — EPUBs are usually small
const MAX_OUTPUT_CHARS: usize = 2_000_000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_true")]
    include_text: bool,
    #[serde(default = "default_true")]
    include_metadata: bool,
    #[serde(default)]
    max_chapters: i64,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct ChapterOut {
    index: usize,
    title: String,
    words: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    publisher: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    chapter_count: usize,
    word_count: usize,
    chapters: Vec<ChapterOut>,
    truncated: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::boolean("include_text")
                .default(true)
                .describe("Include each chapter's readable plain text (default true). Set false for a structure-only table of contents: chapter titles and word counts, no body text."),
        )
        .param(
            Param::boolean("include_metadata")
                .default(true)
                .describe("Include book metadata — author, language, publisher, publication date — alongside the title (default true)."),
        )
        .param(
            Param::integer("max_chapters")
                .default(0)
                .describe("Maximum number of chapters to return, in reading order. 0 (default) returns every chapter."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EpubExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/epub-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract an EPUB's chapters, titles, and metadata as structured text",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the chapter structure and readable text from an EPUB e-book. Reads the EPUB's OPF spine for reading order and its table of contents (NCX / EPUB3 nav, with heading-detection fallback) to title each chapter. Returns the book metadata (title, author, language, publisher, date) plus an ordered list of chapters, each with its index, title, word count, and (optionally) plain text — so you can search, summarize, or quote a specific chapter. Set include_text=false for a lightweight table of contents. Provide the EPUB as url (HTTP/HTTPS) or ref from a prior tool call. Runs locally.",
        parameters = schema_json()
    ),
)]
impl EpubExtract {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("epub-extract")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let book = extract(&bytes).map_err(SkillError::InvalidArgs)?;

    let mut chapters = book.chapters;
    if args.max_chapters > 0 && (args.max_chapters as usize) < chapters.len() {
        chapters.truncate(args.max_chapters as usize);
    }
    let chapter_count = chapters.len();
    let word_count: usize = chapters.iter().map(|c| c.words).sum();

    let mut budget = MAX_OUTPUT_CHARS;
    let mut truncated = false;
    let out_chapters: Vec<ChapterOut> = chapters
        .into_iter()
        .map(|c| {
            let text = if args.include_text {
                let (t, tr) = clip_chars(&c.text, budget);
                budget = budget.saturating_sub(t.chars().count());
                if tr {
                    truncated = true;
                }
                Some(t)
            } else {
                None
            };
            ChapterOut {
                index: c.index,
                title: c.title,
                words: c.words,
                text,
            }
        })
        .collect();

    let meta = book.metadata;
    let resp = Resp {
        title: meta.title,
        author: args.include_metadata.then_some(meta.author).flatten(),
        language: args.include_metadata.then_some(meta.language).flatten(),
        publisher: args.include_metadata.then_some(meta.publisher).flatten(),
        date: args.include_metadata.then_some(meta.date).flatten(),
        chapter_count,
        word_count,
        chapters: out_chapters,
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize epub-extract response: {e}")))
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
                    "include_text": { "type": "boolean", "default": true, "description": "Include each chapter's readable plain text (default true). Set false for a structure-only table of contents: chapter titles and word counts, no body text." },
                    "include_metadata": { "type": "boolean", "default": true, "description": "Include book metadata — author, language, publisher, publication date — alongside the title (default true)." },
                    "max_chapters": { "type": "integer", "default": 0, "description": "Maximum number of chapters to return, in reading order. 0 (default) returns every chapter." }
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
