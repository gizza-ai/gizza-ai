//! gizza-ai/pdf-to-markdown — convert a PDF's text layer into structured
//! Markdown (headings, lists, paragraphs).
//!
//! Pipeline: parse `{url|ref}` + options → fetch the PDF bytes via `block-utils`
//! `resolve_source` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::to_markdown` (lopdf-based
//! structure reconstruction) → return the Markdown as a flat JSON response the
//! LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the
//! conversion, emit the flat `Resp` JSON) rather than going through `run_skill`,
//! because the success shape is the flat `Resp` JSON, not the `{ "result": … }`
//! wrapper `run_skill` produces.
//!
//! No page surface: a PDF is a binary file input and the output is Markdown
//! text, which fits neither the pure-text nor the ffmpeg file→media page
//! shapes — this is a chat + CLI block (the F3 "no-page file-input" pattern,
//! like `pdf-extract-text` / `epub-to-markdown` / `pdf-to-epub`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_to_markdown_core::{to_markdown, Options, PageSeparator};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars of Markdown

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Optional 1-based page number. Omitted = convert every page.
    #[serde(default)]
    page: Option<usize>,
    /// "rule" (default) or "blank".
    #[serde(default = "default_separator")]
    page_separator: String,
    #[serde(default = "default_true")]
    detect_lists: bool,
    #[serde(default = "default_true")]
    dehyphenate: bool,
}

fn default_separator() -> String {
    "rule".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    markdown: String,
    /// Number of unicode characters in `markdown`.
    chars: usize,
    /// The page that was converted, or `null` when all pages were converted.
    page: Option<usize>,
    /// True when `markdown` was clipped to the output cap.
    truncated: bool,
    /// Set when some text runs could not be decoded (e.g. an unparseable font
    /// `ToUnicode` CMap) OR the PDF had no extractable text layer, so `markdown`
    /// is partial/empty. Omitted when conversion was complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a PDF arrives via URL fetch or an attachment
/// ref); the rest are the conversion knobs.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::integer("page")
                .min(1.0)
                .describe("1-based page number to convert. Omit to convert all pages (e.g. 1)."),
        )
        .param(
            Param::enumv("page_separator", ["rule", "blank"])
                .default("rule")
                .describe(
                    "How to divide pages in the Markdown: 'rule' inserts a horizontal rule (---) \
                     between pages, 'blank' just a blank line. Default 'rule'.",
                ),
        )
        .param(
            Param::boolean("detect_lists").default(true).describe(
                "Convert lines starting with a bullet (•, -, *) or a number (1., 2)) into \
                 Markdown list items. Disable to keep them as plain text. Default true.",
            ),
        )
        .param(
            Param::boolean("dehyphenate").default(true).describe(
                "Rejoin words split by a hyphen at the end of a line (e.g. 'conver-' + 'sion' → \
                 'conversion'). Default true.",
            ),
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
struct PdfToMarkdown;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a PDF into clean structured Markdown",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert a PDF's text layer into clean, structured Markdown — headings (from font-size statistics), bullet/numbered lists, and paragraphs — rather than a flat text dump. Provide url (HTTP/HTTPS) or ref from a prior tool call, optionally a 1-based page number (omit for all pages), page_separator (rule|blank), detect_lists, and dehyphenate. Extracts the embedded text layer only — it does not OCR scanned/image-only PDFs, and does not reconstruct tables.",
        parameters = schema_json()
    ),
)]
impl PdfToMarkdown {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat Resp JSON directly (no `{ "result": … }` wrapper),
        // so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-to-markdown")?;
    if let Some(p) = args.page {
        if p == 0 {
            return Err(SkillError::InvalidArgs(
                "page must be >= 1 (pages are 1-based)".to_string(),
            ));
        }
    }
    let page_separator = PageSeparator::parse(&args.page_separator).ok_or_else(|| {
        SkillError::InvalidArgs(format!(
            "page_separator must be 'rule' or 'blank', got '{}'",
            args.page_separator
        ))
    })?;

    // 2. Resolve source — URL fetch or attachment lookup, validated to the
    //    application/* document MIME class.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    // 3. Convert via the pure core (lopdf). Maps parse/range errors to InvalidArgs.
    let opts = Options {
        page: args.page,
        page_separator,
        detect_lists: args.detect_lists,
        dehyphenate: args.dehyphenate,
    };
    let conv = to_markdown(&input_bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let (markdown, truncated) = clip_chars(&conv.markdown, MAX_OUTPUT_CHARS);
    let chars = markdown.chars().count();

    let note = if markdown.trim().is_empty() {
        Some(
            "no extractable text layer was found — the PDF may be scanned/image-only (OCR is not \
             supported)"
                .to_string(),
        )
    } else if conv.dropped_runs > 0 {
        Some(format!(
            "{} text run(s) could not be decoded (unsupported font encoding); the Markdown is partial",
            conv.dropped_runs
        ))
    } else {
        None
    };

    let resp = Resp {
        markdown,
        chars,
        page: args.page,
        truncated,
        note,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-to-markdown response: {e}")))
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// The descriptor-derived chat schema must match this authored schema, so
    /// LLM-facing drift is caught. `Input::Document` fixes the `url`/`ref`
    /// shape + the `oneOf`; `additionalProperties: false` is emitted uniformly.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "page": { "type": "integer", "minimum": 1, "description": "1-based page number to convert. Omit to convert all pages (e.g. 1)." },
                    "page_separator": {
                        "type": "string",
                        "enum": ["rule", "blank"],
                        "default": "rule",
                        "description": "How to divide pages in the Markdown: 'rule' inserts a horizontal rule (---) between pages, 'blank' just a blank line. Default 'rule'."
                    },
                    "detect_lists": {
                        "type": "boolean",
                        "default": true,
                        "description": "Convert lines starting with a bullet (•, -, *) or a number (1., 2)) into Markdown list items. Disable to keep them as plain text. Default true."
                    },
                    "dehyphenate": {
                        "type": "boolean",
                        "default": true,
                        "description": "Rejoin words split by a hyphen at the end of a line (e.g. 'conver-' + 'sion' → 'conversion'). Default true."
                    }
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
    fn args_parse_url_with_options() {
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.pdf","page":3,"page_separator":"blank","detect_lists":false}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.page, Some(3));
        assert_eq!(a.page_separator, "blank");
        assert!(!a.detect_lists);
        assert!(a.dehyphenate); // defaulted
    }

    #[test]
    fn args_defaults() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert_eq!(a.page, None);
        assert_eq!(a.page_separator, "rule");
        assert!(a.detect_lists);
        assert!(a.dehyphenate);
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
