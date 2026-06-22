//! gizza-ai/pdf-to-epub — convert a text-based PDF into a reflowable EPUB ebook.
//!
//! Pipeline: resolve the source PDF (URL fetch or attachment ref) →
//! `core::pdf_to_epub` (lopdf text extraction + zip/EPUB assembly) → base64 EPUB
//! envelope (`build_media_envelope`) the chat UI offers as a download.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page: a binary file input with binary (EPUB) output
//! fits neither the pure-text page nor the ffmpeg file→media page shape — the
//! no-page file-input pattern (like images-to-pdf / epub-to-markdown).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_to_epub_core::pdf_to_epub;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024; // 32 MiB
const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Book title → dc:title. Defaults to "Untitled" when omitted/blank.
    #[serde(default)]
    title: String,
    /// Optional author → dc:creator.
    #[serde(default)]
    author: String,
    /// Keep the PDF's hard line breaks as `<br/>` instead of unwrapping
    /// wrapped lines into flowing reflowable text. Default false.
    #[serde(default)]
    preserve_line_breaks: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::string("title")
                .describe("Book title for the EPUB metadata (dc:title). Defaults to \"Untitled\" if omitted."),
        )
        .param(
            Param::string("author")
                .describe("Optional author for the EPUB metadata (dc:creator)."),
        )
        .param(
            Param::boolean("preserve_line_breaks")
                .default(false)
                .describe("When true, keep the PDF's hard line breaks as line breaks. When false (default), wrapped lines are merged into flowing text for clean reflow. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Derive a stable book identifier from the title + PDF size so the same input
/// produces a deterministic `dc:identifier` (no RNG needed in wafer).
fn derive_book_id(title: &str, byte_len: usize) -> String {
    // FNV-1a 64-bit over title bytes + length — small, dependency-free, stable.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in title.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    for b in byte_len.to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("urn:gizza-ai:pdf-to-epub:{h:016x}")
}

#[cfg(target_arch = "wasm32")]
struct PdfToEpub;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-to-epub",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a text-based PDF into a reflowable EPUB ebook",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert a text-based PDF into a reflowable EPUB ebook (one chapter per page, with a table of contents). Wrapped lines are unwrapped into flowing text so the result reflows on any reader; set preserve_line_breaks to keep the original line layout (e.g. for poetry or code). Provide the PDF as url (HTTP/HTTPS) or ref from a prior tool call, and optionally a title and author for the ebook metadata. Returns the EPUB file as a download. Extracts the embedded text layer only — it does not OCR scanned/image-only PDFs (those produce empty chapters).",
        parameters = schema_json()
    ),
)]
impl PdfToEpub {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-to-epub")?;

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    let book_id = derive_book_id(&args.title, bytes.len());
    let conv = pdf_to_epub(
        &bytes,
        &args.title,
        &args.author,
        &book_id,
        args.preserve_line_breaks,
    )
    .map_err(SkillError::InvalidArgs)?;

    let nbytes = conv.epub.len();
    let chapters = conv.chapters;
    let note = if conv.dropped_chunks > 0 {
        format!(
            " ({} text run(s) could not be decoded — some chapters are partial)",
            conv.dropped_chunks
        )
    } else {
        String::new()
    };

    build_media_envelope(
        &conv.epub,
        "application/epub+zip",
        "book.epub".to_string(),
        format!("converted a PDF into an EPUB ({chapters} chapter(s), {nbytes} bytes){note}"),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema. `Input::Document` emits the `url`⊕`ref` oneOf; `title`/`author`
    /// are optional strings.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "title":  { "type": "string", "description": "Book title for the EPUB metadata (dc:title). Defaults to \"Untitled\" if omitted." },
                    "author": { "type": "string", "description": "Optional author for the EPUB metadata (dc:creator)." },
                    "preserve_line_breaks": { "type": "boolean", "default": false, "description": "When true, keep the PDF's hard line breaks as line breaks. When false (default), wrapped lines are merged into flowing text for clean reflow. Default false." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_with_meta() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://x/y.pdf","title":"T","author":"A"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.title, "T");
        assert_eq!(a.author, "A");
    }

    #[test]
    fn args_parse_ref_defaults() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert_eq!(a.title, "");
        assert_eq!(a.author, "");
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn book_id_is_deterministic() {
        assert_eq!(derive_book_id("T", 100), derive_book_id("T", 100));
        assert_ne!(derive_book_id("T", 100), derive_book_id("T", 101));
        assert!(derive_book_id("T", 100).starts_with("urn:gizza-ai:pdf-to-epub:"));
    }
}
