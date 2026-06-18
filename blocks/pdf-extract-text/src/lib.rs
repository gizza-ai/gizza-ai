//! gizza-ai/pdf-extract-text — extract the selectable text from a PDF.
//!
//! Pipeline: parse `{url|ref}` + optional `page` → fetch the PDF bytes via
//! `block-utils` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::extract` (lopdf) → return the
//! extracted text as a flat JSON response the LLM reads directly.
//!
//! No page surface: a PDF is a binary file input and the output is plain text,
//! which fits neither the pure-text nor the ffmpeg file→media page shapes — this
//! is a chat + CLI block (the F3 "no-page file-input" pattern, like web-fetch).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests. Block-local helpers stay native-compilable so
// the unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_block_utils::{AssetKind, SkillError, SkillResultExt, Source, SourceFields};
use gizza_ai_pdf_extract_text_core::extract;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars of extracted text

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Optional 1-based page number. Omitted = extract every page.
    #[serde(default)]
    page: Option<usize>,
}

#[derive(Serialize)]
struct Resp {
    text: String,
    /// Number of unicode characters in `text`.
    chars: usize,
    /// The page that was extracted, or `null` when all pages were extracted.
    page: Option<usize>,
    /// True when `text` was clipped to the output cap.
    truncated: bool,
    /// Set when some text runs could not be decoded (e.g. an unparseable font
    /// `ToUnicode` CMap), so `text` is partial. Omitted when extraction was
    /// complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
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
struct PdfExtractText;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-extract-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the selectable text from a PDF",
    requires = ["wafer-run/network"],
    skill(
        description = "Extract the selectable text from a PDF. Provide url (HTTP/HTTPS) or ref from a prior tool call, and optionally a 1-based page number (omit to extract every page). Extracts the embedded text layer only — it does not OCR scanned/image-only PDFs.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":  { "type": "string", "description": "PDF URL (HTTP/HTTPS)." },
                "ref":  { "type": "string", "description": "Reference id from a prior tool call (e.g. \"call_42\"). Use either url or ref." },
                "page": { "type": "integer", "minimum": 1, "description": "1-based page number to extract. Omit to extract all pages." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl PdfExtractText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-extract-text")?;
    if let Some(p) = args.page {
        if p == 0 {
            return Err(SkillError::InvalidArgs(
                "page must be >= 1 (pages are 1-based)".to_string(),
            ));
        }
    }

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, _mime, _filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Document, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Document, MAX_INPUT_BYTES)?,
    };

    // 3. Extract via the pure core (lopdf). Maps parse/range errors to InvalidArgs.
    let ex = extract(&input_bytes, args.page).map_err(SkillError::InvalidArgs)?;
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
        page: args.page,
        truncated,
        note,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-extract-text response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parse_url_and_page() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.pdf","page":3}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.page, Some(3));
    }

    #[test]
    fn args_parse_ref_no_page() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert_eq!(a.page, None);
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
