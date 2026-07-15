//! gizza-ai/document-text-extract — extract plain text from a PDF, DOCX, or
//! EPUB document, auto-detecting the format.
//!
//! Pipeline: parse `{url|ref}` → fetch the document bytes via `block-utils`
//! `resolve_source` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::extract` (which sniffs the
//! format from the file's magic bytes and routes to the lopdf / DOCX / EPUB
//! extractor) → return the extracted text as a flat JSON response the LLM reads
//! directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run extraction,
//! emit the flat `Resp` JSON) rather than going through `run_skill`, because the
//! success shape is the flat `Resp` JSON, not the `{ "result": … }` wrapper
//! `run_skill` produces.
//!
//! No page surface: a document is a binary file input and the output is plain
//! text, which fits neither the pure-text nor the ffmpeg file→media page shapes
//! — this is a chat + CLI block (the "no-page file-input" pattern, like
//! `pdf-extract-text` / `epub-to-markdown` / `web-fetch`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests. Block-local helpers stay native-compilable so
// the unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_document_text_extract_core::extract;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars of extracted text

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct Resp {
    /// The extracted plain text.
    text: String,
    /// Number of unicode characters in `text`.
    chars: usize,
    /// The detected document format: `"pdf"`, `"docx"`, or `"epub"`.
    format: String,
    /// True when `text` was clipped to the output cap.
    truncated: bool,
    /// Set when some text runs could not be decoded (PDF only — e.g. an
    /// unparseable font `ToUnicode` CMap), so `text` is partial. Omitted when
    /// extraction was complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a document arrives via URL fetch or an
/// attachment ref). No extra params: the format is auto-detected from the bytes.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Clip `text` to at most `max_chars` unicode characters. Returns
/// `(clipped, was_truncated)`.
fn clip_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() > max_chars {
        (text.chars().take(max_chars).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

#[cfg(target_arch = "wasm32")]
struct DocumentTextExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-text-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract plain text from a PDF, DOCX, or EPUB document",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the searchable plain text from a document, auto-detecting the format from the file. Supports PDF (the embedded text layer only — it does NOT OCR scanned/image-only pages), DOCX (Microsoft Word), and EPUB (e-book). Provide url (HTTP/HTTPS) or ref from a prior tool call. Returns the extracted text, its character count, and the detected format.",
        parameters = schema_json()
    ),
)]
impl DocumentTextExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // document-text-extract returns the flat Resp JSON directly (no
        // `{ "result": … }` wrapper), so it keeps a thin handle rather than
        // using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("document-text-extract")?;

    // 2. Resolve source — URL fetch or attachment lookup. `AssetKind::Any`: we
    //    detect the real format from the file's magic bytes (a server may label
    //    a DOCX/EPUB as octet-stream), so we don't gate on the declared MIME.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;

    // 3. Extract via the pure core (format auto-detected). Maps parse/format
    //    errors to InvalidArgs.
    let ex = extract(&input_bytes).map_err(SkillError::InvalidArgs)?;
    let (text, truncated) = clip_chars(&ex.text, MAX_OUTPUT_CHARS);
    let chars = text.chars().count();
    let note = (ex.dropped_chunks > 0).then(|| {
        format!(
            "{} text run(s) could not be decoded (unsupported font encoding); extracted text is partial",
            ex.dropped_chunks
        )
    });

    let resp = Resp {
        text,
        chars,
        format: ex.format.as_str().to_string(),
        truncated,
        note,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize document-text-extract response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Migration/consistency guard: the descriptor-derived chat schema must match
    /// this authored blob, so the LLM sees a stable shape. `Input::Document`
    /// single-sources the `url`/`ref` `oneOf` + descriptions; there are no extra
    /// params (the format is auto-detected).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
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
    fn args_parse_url() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.docx"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.docx"));
    }

    #[test]
    fn args_parse_ref() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn clip_chars_passes_short_text() {
        let (out, trunc) = clip_chars("hello", 100);
        assert_eq!(out, "hello");
        assert!(!trunc);
    }

    #[test]
    fn clip_chars_truncates_long_text() {
        let (out, trunc) = clip_chars("abcdef", 3);
        assert_eq!(out, "abc");
        assert!(trunc);
    }
}
