//! gizza-ai/pdf-extract-text — extract the selectable text from a PDF.
//!
//! Pipeline: parse `{url|ref}` + optional `page` → fetch the PDF bytes via
//! `block-utils` `resolve_source` (URL fetch through `wafer-run/network`, or an
//! uploaded attachment ref) → delegate to the pure `core::extract` (lopdf) →
//! return the extracted text as a flat JSON response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run extraction,
//! emit the flat `Resp` JSON) rather than going through `run_skill`, because
//! pdf-extract-text's success shape is the flat `Resp` JSON, not the
//! `{ "result": … }` wrapper `run_skill` produces.
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
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
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

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// `Input::Document` emits the `url`⊕`ref` `oneOf` (a PDF arrives via URL fetch
/// or an attachment ref); `page` is an optional 1-based integer.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document).param(
        Param::integer("page")
            .min(1.0)
            .describe("1-based page number to extract. Omit to extract all pages."),
    )
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
        parameters = schema_json()
    ),
)]
impl PdfExtractText {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // pdf-extract-text returns the flat Resp JSON directly (no
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
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-extract-text")?;
    if let Some(p) = args.page {
        if p == 0 {
            return Err(SkillError::InvalidArgs(
                "page must be >= 1 (pages are 1-based)".to_string(),
            ));
        }
    }

    // 2. Resolve source — URL fetch or attachment lookup, validated to the
    //    application/* document MIME class.
    let (input_bytes, _mime, _filename) = resolve_source(
        args.source.into_inner(),
        AssetKind::Document,
        MAX_INPUT_BYTES,
    )?;

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
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. Two intentional,
    /// documented deltas from the hand-authored blob:
    ///   - `additionalProperties: false` — `to_schema_json()` now emits this
    ///     uniformly (the authored schema lacked it); tool schemas reject unknown
    ///     params, a hardening change, so it is added to the expected blob.
    ///   - the `url`/`ref` descriptions — these are fixed by `Input::Document`
    ///     (single source), so the expected blob uses the descriptor-emitted
    ///     wording rather than the old hand-authored strings.
    /// `page` keeps its `integer`/`minimum: 1` shape (rendered as the JSON
    /// integer `1`, not `1.0`).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "page": { "type": "integer", "minimum": 1, "description": "1-based page number to extract. Omit to extract all pages." }
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
