//! gizza-ai/docx-text-extract — convert a `.docx` (Microsoft Word) document to
//! GitHub-Flavored Markdown and/or clean plain text, preserving the document
//! structure (headings, lists, tables, hyperlinks, bold/italic).
//!
//! Pipeline: parse `{url|ref, format}` → fetch the document bytes via
//! `block-utils` `resolve_source` (URL fetch through `wafer-run/network`, or an
//! uploaded attachment ref) → delegate to the pure `core::convert` (which parses
//! the WordprocessingML and rebuilds the Markdown + plain text) → return a flat
//! JSON response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the conversion,
//! emit the flat `Resp` JSON) rather than going through `run_skill`, because the
//! success shape is the flat `Resp` JSON, not the `{ "result": … }` wrapper.
//!
//! No page surface: a document is a binary file input and the output is text,
//! which fits neither the pure-text nor the ffmpeg file→media page shapes — this
//! is a chat + CLI block (the "no-page file-input" pattern, like
//! `document-text-extract` / `pdf-extract-text` / `epub-to-markdown`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting imports
// + the Args type are only used inside that impl, so they look "unused" when
// running native unit tests. Block-local helpers stay native-compilable so the
// unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_docx_text_extract_core::convert;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars per representation

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Which representation(s) to return. Defaults to `both`.
    #[serde(default)]
    format: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    /// The reconstructed GitHub-Flavored Markdown. Omitted when `format=text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<String>,
    /// The flattened plain text. Omitted when `format=markdown`.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    /// Number of heading paragraphs detected.
    headings: usize,
    /// Number of tables rendered as Markdown pipe tables.
    tables: usize,
    /// Whitespace-delimited word count of the plain text.
    words: usize,
    /// True when either representation was clipped to the output cap.
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`; the `format` enum picks which representation(s)
/// to return.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document).param(
        Param::enumv("format", ["both", "markdown", "text"])
            .default("both")
            .describe(
                "Which representation(s) to return: `both` (default) returns the Markdown \
                 structure and the plain text; `markdown` returns only the structured \
                 GitHub-Flavored Markdown; `text` returns only the plain text.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Parse + validate the `format` param into the two output flags
/// `(want_markdown, want_text)`. Defaults to both; rejects unknown values.
fn parse_format(format: Option<&str>) -> Result<(bool, bool), SkillError> {
    match format.unwrap_or("both") {
        "both" => Ok((true, true)),
        "markdown" => Ok((true, false)),
        "text" => Ok((false, true)),
        other => Err(SkillError::InvalidArgs(format!(
            "invalid format {other:?}: expected one of \"both\", \"markdown\", \"text\""
        ))),
    }
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

/// Build the flat response from a conversion, applying the output-cap clip and
/// the `format` selection.
fn build_resp(
    conv: &gizza_ai_docx_text_extract_core::Conversion,
    want_markdown: bool,
    want_text: bool,
) -> Resp {
    let (md, md_trunc) = clip_chars(&conv.markdown, MAX_OUTPUT_CHARS);
    let (txt, txt_trunc) = clip_chars(&conv.text, MAX_OUTPUT_CHARS);
    let words = conv.text.split_whitespace().count();
    Resp {
        markdown: want_markdown.then_some(md),
        text: want_text.then_some(txt),
        headings: conv.headings,
        tables: conv.tables,
        words,
        truncated: (want_markdown && md_trunc) || (want_text && txt_trunc),
    }
}

#[cfg(target_arch = "wasm32")]
struct DocxTextExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docx-text-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a DOCX (Word) document to Markdown and plain text",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert a Microsoft Word .docx document into GitHub-Flavored Markdown and/or clean plain text, preserving the document structure: headings (from Word styles), ordered and bullet lists, tables (as Markdown pipe tables), hyperlinks, and bold/italic emphasis. Provide url (HTTP/HTTPS) or ref from a prior tool call. Set format to `both` (default), `markdown`, or `text`. Returns the requested representation(s) plus heading/table/word counts. Only .docx is supported (not legacy .doc); it reads the document text layer, not embedded images.",
        parameters = schema_json()
    ),
)]
impl DocxTextExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat Resp JSON directly (no `{ "result": … }` wrapper), so
        // it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("docx-text-extract")?;
    let (want_markdown, want_text) = parse_format(args.format.as_deref())?;

    // 2. Resolve source — URL fetch or attachment lookup. `AssetKind::Any`: a
    //    server may label a DOCX as octet-stream, so we don't gate on MIME and
    //    instead verify the magic bytes in the pure core.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;

    // 3. Convert via the pure core. Maps parse/format errors to InvalidArgs.
    let conv = convert(&input_bytes).map_err(SkillError::InvalidArgs)?;
    let resp = build_resp(&conv, want_markdown, want_text);
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize docx-text-extract response: {e}")))
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Migration/consistency guard: the descriptor-derived chat schema must match
    /// this authored blob, so the LLM sees a stable shape. `Input::Document`
    /// single-sources the `url`/`ref` `oneOf`; the `format` enum is the one extra
    /// param.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["both", "markdown", "text"],
                        "default": "both",
                        "description": "Which representation(s) to return: `both` (default) returns the Markdown structure and the plain text; `markdown` returns only the structured GitHub-Flavored Markdown; `text` returns only the plain text."
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
    fn args_parse_url_and_default_format() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.docx"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.docx"));
        assert_eq!(a.format, None);
    }

    #[test]
    fn args_parse_ref_and_format() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7","format":"markdown"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn parse_format_maps_each_value() {
        assert_eq!(parse_format(None).unwrap(), (true, true));
        assert_eq!(parse_format(Some("both")).unwrap(), (true, true));
        assert_eq!(parse_format(Some("markdown")).unwrap(), (true, false));
        assert_eq!(parse_format(Some("text")).unwrap(), (false, true));
    }

    #[test]
    fn parse_format_rejects_unknown() {
        let err = parse_format(Some("html")).unwrap_err();
        assert!(err.to_string().contains("invalid format"), "err was: {err}");
    }

    #[test]
    fn build_resp_selects_representations_and_counts_words() {
        let conv = gizza_ai_docx_text_extract_core::Conversion {
            markdown: "# Title\n\nhello world".into(),
            text: "Title\nhello world".into(),
            headings: 1,
            tables: 0,
        };
        let both = build_resp(&conv, true, true);
        assert!(both.markdown.is_some() && both.text.is_some());
        assert_eq!(both.words, 3);
        assert_eq!(both.headings, 1);
        assert!(!both.truncated);

        let md_only = build_resp(&conv, true, false);
        assert!(md_only.markdown.is_some() && md_only.text.is_none());

        let txt_only = build_resp(&conv, false, true);
        assert!(txt_only.markdown.is_none() && txt_only.text.is_some());
    }

    #[test]
    fn clip_chars_truncates_long_text() {
        let (out, trunc) = clip_chars("abcdef", 3);
        assert_eq!(out, "abc");
        assert!(trunc);
    }
}
