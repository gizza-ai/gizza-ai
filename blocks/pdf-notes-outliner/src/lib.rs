//! gizza-ai/pdf-notes-outliner — extract a lecture/course PDF's text layer,
//! reconstruct its heading hierarchy, and return a structured outline (table of
//! contents) with a short extractive summary under each section.
//!
//! Pipeline: parse `{url|ref}` + options → fetch the PDF bytes via `block-utils`
//! `resolve_source` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::outline` (lopdf font-size
//! heading detection + TextRank per-section summaries) → return a flat JSON
//! response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run, emit the flat
//! `Resp` JSON) rather than going through `run_skill`, because the success shape
//! is the flat `Resp` JSON, not the `{ "result": … }` wrapper `run_skill`
//! produces.
//!
//! No page surface: a PDF is a binary file input and the output is an
//! outline/JSON, which fits neither the pure-text nor the ffmpeg file→media page
//! shapes — this is a chat + CLI block (the F3 "no-page file-input" pattern,
//! like `pdf-to-markdown` / `pdf-extract-text` / `epub-to-markdown`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_notes_outliner_core::{outline, OutlineOptions};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

fn default_max_depth() -> usize {
    3
}
fn default_summary_sentences() -> usize {
    2
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Deepest heading level to keep as its own entry (1–6). Deeper headings fold
    /// into their parent's summary.
    #[serde(default = "default_max_depth")]
    max_depth: usize,
    /// TextRank sentences per section (0–10). 0 = pure outline / table of contents.
    #[serde(default = "default_summary_sentences")]
    summary_sentences: usize,
}

#[derive(Serialize)]
struct SectionJson {
    /// Detected heading level (1 = biggest font), consistent across the document.
    level: u8,
    /// The heading text.
    title: String,
    /// 1-based page number the heading appears on.
    page: usize,
    /// Extractive summary of the section body (empty when there is none or when
    /// summary_sentences = 0).
    summary: String,
}

#[derive(Serialize)]
struct Resp {
    /// A ready-to-read nested outline (indented, with page numbers + summaries).
    outline: String,
    /// The structured sections in reading order.
    sections: Vec<SectionJson>,
    /// Number of sections in the outline.
    section_count: usize,
    /// Set when the outline is partial/degenerate (scanned/image-only PDF, no
    /// font-size heading contrast, or undecodable font runs). Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a PDF arrives via URL fetch or an attachment
/// ref); the rest are the outline knobs.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::integer("max_depth")
                .min(1.0)
                .max(6.0)
                .default(3)
                .describe(
                    "Deepest heading level to list as its own section, 1–6 (headings are ranked \
                     by font size: 1 = biggest). Headings deeper than this fold into their \
                     parent section's summary instead of getting their own entry. Default 3.",
                ),
        )
        .param(
            Param::integer("summary_sentences")
                .min(0.0)
                .max(10.0)
                .default(2)
                .describe(
                    "How many sentences to summarise each section with, 0–10. Summaries are \
                     extractive (the most important verbatim sentences via TextRank, not a \
                     rewrite). Use 0 for a pure outline / table of contents with no summaries. \
                     Default 2.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfNotesOutliner;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-notes-outliner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Outline a PDF's headings with per-section summaries",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Turn a lecture/course PDF into a structured heading outline (table of contents) with a short extractive summary under each section. Detects the heading hierarchy from document-wide font-size statistics (no bookmarks needed), tags each heading with its page number, and summarises each section's text with TextRank (top verbatim sentences, no ML model — nothing is rewritten). Provide url (HTTP/HTTPS) or ref from a prior tool call, optionally max_depth (1–6, deepest heading level kept; default 3) and summary_sentences (0–10 per section; 0 = pure outline; default 2). Extracts the embedded text layer only — it does NOT OCR scanned/image-only PDFs (those return a note and no headings), and heading detection needs font-size contrast (a uniform-font document yields one whole-document summary).",
        parameters = schema_json()
    ),
)]
impl PdfNotesOutliner {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-notes-outliner")?;

    // 2. Resolve source — URL fetch or attachment lookup, validated to the
    //    application/* document MIME class.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    // 3. Build the outline via the pure core. Maps parse errors to InvalidArgs.
    let opts = OutlineOptions {
        max_depth: args.max_depth,
        summary_sentences: args.summary_sentences,
    };
    let out = outline(&input_bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let sections: Vec<SectionJson> = out
        .sections
        .into_iter()
        .map(|s| SectionJson {
            level: s.level,
            title: s.title,
            page: s.page,
            summary: s.summary,
        })
        .collect();

    let resp = Resp {
        outline: out.rendered,
        section_count: sections.len(),
        sections,
        note: out.note,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-notes-outliner response: {e}")))
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
                    "max_depth": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 6,
                        "default": 3,
                        "description": "Deepest heading level to list as its own section, 1–6 (headings are ranked by font size: 1 = biggest). Headings deeper than this fold into their parent section's summary instead of getting their own entry. Default 3."
                    },
                    "summary_sentences": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 10,
                        "default": 2,
                        "description": "How many sentences to summarise each section with, 0–10. Summaries are extractive (the most important verbatim sentences via TextRank, not a rewrite). Use 0 for a pure outline / table of contents with no summaries. Default 2."
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
            r#"{"url":"https://x/y.pdf","max_depth":2,"summary_sentences":0}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.max_depth, 2);
        assert_eq!(a.summary_sentences, 0);
    }

    #[test]
    fn args_defaults() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert_eq!(a.max_depth, 3);
        assert_eq!(a.summary_sentences, 2);
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
