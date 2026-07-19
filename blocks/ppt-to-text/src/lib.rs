//! gizza-ai/ppt-to-text — extract readable plain text from a **legacy binary
//! PowerPoint `.ppt` (PowerPoint 97–2003)** presentation.
//!
//! Pipeline: parse `{url|ref, whitespace, notes}` → fetch the presentation bytes
//! via `block-utils` `resolve_source` (URL fetch through `wafer-run/network`, or
//! an uploaded attachment ref) → delegate to the pure `core::extract` (which
//! parses the OLE2 container + PowerPoint record tree) → return a flat JSON
//! response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the extraction,
//! emit the flat `Resp` JSON) rather than going through `run_skill`, because the
//! success shape is the flat `Resp` JSON, not the `{ "result": … }` wrapper.
//!
//! No page surface: a legacy `.ppt` is a binary file input and the output is
//! text, which fits neither the pure-text nor the ffmpeg file→media page shapes —
//! this is a chat + CLI block (the "no-page file-input" pattern, like
//! `doc-to-text` / `docx-text-extract` / `document-text-extract` /
//! `pdf-extract-text`).

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
use gizza_ai_ppt_to_text_core::{extract, Notes, Whitespace};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// How to post-process the extracted text. Defaults to `clean`.
    #[serde(default)]
    whitespace: Option<String>,
    /// Whether to include speaker notes. Defaults to `include`.
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    /// The extracted plain text.
    text: String,
    /// Whitespace-delimited word count of the extracted text.
    words: usize,
    /// Number of non-empty paragraphs (lines) in the extracted text.
    paragraphs: usize,
    /// Number of slides found in the presentation.
    slides: usize,
    /// Number of text runs (text atoms) included in the output.
    text_runs: usize,
    /// True when the text was clipped to the output cap.
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`; the `whitespace` + `notes` enums pick the
/// output policy.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("whitespace", ["clean", "raw"])
                .default("clean")
                .describe(
                    "How to post-process the extracted text: `clean` (default) collapses runs \
                     of blank lines to a single blank line and trims trailing spaces; `raw` \
                     returns the mapped text unchanged, preserving the original spacing.",
                ),
        )
        .param(
            Param::enumv("notes", ["include", "exclude", "only"])
                .default("include")
                .describe(
                    "How to handle speaker notes: `include` (default) appends the notes text \
                     after the slide text; `exclude` returns slide text only; `only` returns \
                     the speaker notes and drops the slide text.",
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

/// Build the flat response from an extraction, applying the output-cap clip.
fn build_resp(extraction: &gizza_ai_ppt_to_text_core::Extraction) -> Resp {
    let (text, clip_trunc) = clip_chars(&extraction.text, MAX_OUTPUT_CHARS);
    let words = text.split_whitespace().count();
    Resp {
        text,
        words,
        paragraphs: extraction.paragraphs,
        slides: extraction.slides,
        text_runs: extraction.text_runs,
        truncated: extraction.truncated || clip_trunc,
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ppt-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract plain text from a legacy PowerPoint 97-2003 .ppt file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract readable plain text from a legacy binary Microsoft PowerPoint \
                       .ppt (PowerPoint 97-2003) presentation. Give a presentation URL or an \
                       uploaded reference. Returns the slide (and optionally speaker-notes) \
                       plain text plus slide/word/paragraph counts. For the modern ZIP-based \
                       .pptx use a .pptx/Office Open XML tool instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(bytes) => GuestResult::respond(bytes),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("ppt-to-text")?;
    let whitespace =
        Whitespace::parse(args.whitespace.as_deref()).map_err(SkillError::InvalidArgs)?;
    let notes = Notes::parse(args.notes.as_deref()).map_err(SkillError::InvalidArgs)?;

    // 2. Resolve source — URL fetch or attachment lookup. `AssetKind::Any`: a
    //    server may label a .ppt as octet-stream, so we don't gate on MIME and
    //    instead verify the OLE2 magic bytes in the pure core.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;

    // 3. Extract via the pure core. Maps parse errors to InvalidArgs.
    let extraction = extract(&input_bytes, whitespace, notes).map_err(SkillError::InvalidArgs)?;
    let resp = build_resp(&extraction);
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize ppt-to-text response: {e}")))
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Migration/consistency guard: the descriptor-derived chat schema must match
    /// this authored blob, so the LLM sees a stable shape. `Input::Document`
    /// single-sources the `url`/`ref` `oneOf`; `whitespace` + `notes` are the two
    /// extra params.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "whitespace": {
                        "type": "string",
                        "enum": ["clean", "raw"],
                        "default": "clean",
                        "description": "How to post-process the extracted text: `clean` (default) collapses runs of blank lines to a single blank line and trims trailing spaces; `raw` returns the mapped text unchanged, preserving the original spacing."
                    },
                    "notes": {
                        "type": "string",
                        "enum": ["include", "exclude", "only"],
                        "default": "include",
                        "description": "How to handle speaker notes: `include` (default) appends the notes text after the slide text; `exclude` returns slide text only; `only` returns the speaker notes and drops the slide text."
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
    fn args_parse_url_and_defaults() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.ppt"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.ppt"));
        assert_eq!(a.whitespace, None);
        assert_eq!(a.notes, None);
    }

    #[test]
    fn args_parse_ref_and_options() {
        let a: Args =
            serde_json::from_str(r#"{"ref":"call_7","whitespace":"raw","notes":"exclude"}"#)
                .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn build_resp_counts_words_and_paragraphs() {
        let extraction = gizza_ai_ppt_to_text_core::Extraction {
            text: "Hello world\nsecond line".into(),
            paragraphs: 2,
            slides: 1,
            text_runs: 2,
            truncated: false,
        };
        let resp = build_resp(&extraction);
        assert_eq!(resp.text, "Hello world\nsecond line");
        assert_eq!(resp.words, 4);
        assert_eq!(resp.paragraphs, 2);
        assert_eq!(resp.slides, 1);
        assert_eq!(resp.text_runs, 2);
        assert!(!resp.truncated);
    }

    #[test]
    fn clip_chars_truncates_long_text() {
        let (out, trunc) = clip_chars("abcdef", 3);
        assert_eq!(out, "abc");
        assert!(trunc);
    }
}
