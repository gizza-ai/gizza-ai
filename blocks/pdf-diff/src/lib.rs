//! gizza-ai/pdf-diff — compare an ORIGINAL and a REVISED PDF and report
//! textual and object-level visual differences page by page.
//!
//! Pipeline: resolve each source (URL/ref, `AssetKind::Document`) → pure
//! `core::diff_pdfs` (lopdf parse → per-page text/word tokenizing → similarity
//! page alignment → LCS hunks + object-level visual + metadata diff) → flat
//! JSON response. `Input::None` + a required two-item `files` source_list (the
//! merge-pdf / video-audio-sync-offset-finder multi-input pattern).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker.
//! Surfaces: chat + CLI. No standalone page (the page driver takes a single
//! upload; this tool needs two files).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_diff_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Per-file input cap — matches merge-pdf's PDF family cap.
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    files: Vec<SourceFields>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default = "default_pages")]
    pages: String,
    #[serde(default)]
    include_unchanged: bool,
}

fn default_mode() -> String {
    "words".to_string()
}
fn default_align() -> String {
    "auto".to_string()
}
fn default_pages() -> String {
    "all".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::source_list("files", 2)
                .required()
                .describe("Exactly two PDFs: the ORIGINAL (old) version first, the revised (new) version second. Each item has exactly one of `url` or `ref` (a reference id from a prior tool call). Up to 8 MiB and 2000 pages per file; password-protected PDFs are rejected — remove the password first."),
        )
        .param(
            Param::string("pages")
                .default("all")
                .describe("Which pages to compare, applied to both PDFs: 'all' (default), 'odd', 'even', or a 1-based list/range like '1,3-5'. Out-of-range pages are ignored per document."),
        )
        .param(
            Param::enumv("mode", ["words", "lines"])
                .default("words")
                .describe("Text diff granularity: 'words' (default) reports word-level add/remove/replace hunks — best for contracts and prose; 'lines' diffs whole extracted lines — better for code-like or tabular documents."),
        )
        .param(
            Param::enumv("align", ["auto", "sequential"])
                .default("auto")
                .describe("How pages are paired: 'auto' (default) aligns pages by content similarity so an inserted or deleted page is reported as added/removed instead of shifting every later page (documents up to 200 pages; beyond that it falls back to sequential); 'sequential' always compares page N with page N."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Ignore letter case in the text comparison (default false). 'Payment' vs 'payment' stops counting as a change; the reported hunks still show the original spelling."),
        )
        .param(
            Param::boolean("include_unchanged")
                .default(false)
                .describe("Also list the page pairs that did NOT change (default false — only changed pages, added/removed pages and summary counts are reported)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[derive(Serialize)]
struct Resp {
    /// Display names of the compared files.
    original: String,
    revised: String,
    #[serde(flatten)]
    report: core::Report,
}

#[cfg(target_arch = "wasm32")]
struct PdfDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two PDFs and report text and visual differences page by page",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compare an original and a revised PDF and report what changed, page by page — e.g. what changed between v1 and v2 of a contract. Provide files as a list of exactly two items (the ORIGINAL first, the revised second), each a url or a `ref` from a prior tool call, up to 8 MiB and 2000 pages per file. The selectable text layers are diffed word by word (mode=words, default) or line by line (mode=lines): every changed page gets add/remove/replace hunks with surrounding context, exact added/removed word counts and a 0–1 similarity score. Pages are paired by content similarity (align=auto, default, up to 200 pages) so an inserted or deleted page is reported as added/removed instead of shifting every later page; align=sequential forces page-N-vs-page-N. Object-level visual changes are reported per page — page size, rotation, embedded images added/removed/replaced (compared by content), and font-set changes — plus document metadata changes (/Info Title, Author, Producer, dates). pages restricts the comparison ('all' default, 'odd', 'even', or '1,3-5'); ignore_case ignores letter case; include_unchanged also lists unchanged page pairs. The result includes identical (true when nothing differs), a one-line summary, and per-document page totals with changed/unchanged/added/removed counts. Limits: compares the embedded selectable text layer only (no OCR — scanned/image-only pages compare as empty text, though their images still compare by content) and does not rasterize pages, so visual differences are object-level, not pixel-level; password-protected PDFs are rejected.",
        parameters = schema_json()
    ),
)]
impl PdfDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-diff")?;
    if args.files.len() != 2 {
        return Err(SkillError::InvalidArgs(format!(
            "pdf-diff needs exactly two files (the original first, the revised second), got {}",
            args.files.len()
        )));
    }
    let mode = match args.mode.as_str() {
        "words" => core::Mode::Words,
        "lines" => core::Mode::Lines,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "mode must be 'words' or 'lines', got '{other}'"
            )))
        }
    };
    let align = match args.align.as_str() {
        "auto" => core::Align::Auto,
        "sequential" => core::Align::Sequential,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "align must be 'auto' or 'sequential', got '{other}'"
            )))
        }
    };

    let mut sources = args.files.into_iter();
    let (a_bytes, _a_mime, a_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Document,
        MAX_INPUT_BYTES,
    )?;
    let (b_bytes, _b_mime, b_name) = resolve_source(
        sources.next().expect("len checked").into_inner(),
        AssetKind::Document,
        MAX_INPUT_BYTES,
    )?;

    let opt = core::Options {
        mode,
        align,
        ignore_case: args.ignore_case,
        pages: args.pages,
        include_unchanged: args.include_unchanged,
    };
    let report = core::diff_pdfs(&a_bytes, &b_bytes, &opt).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        original: display_name(&a_name, "original.pdf"),
        revised: display_name(&b_name, "revised.pdf"),
        report,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-diff response: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn display_name(name: &str, fallback: &str) -> String {
    if name.trim().is_empty() {
        fallback.to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Exactly two PDFs: the ORIGINAL (old) version first, the revised (new) version second. Each item has exactly one of `url` or `ref` (a reference id from a prior tool call). Up to 8 MiB and 2000 pages per file; password-protected PDFs are rejected — remove the password first.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    },
                    "pages": {
                        "type": "string",
                        "default": "all",
                        "description": "Which pages to compare, applied to both PDFs: 'all' (default), 'odd', 'even', or a 1-based list/range like '1,3-5'. Out-of-range pages are ignored per document."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["words", "lines"],
                        "default": "words",
                        "description": "Text diff granularity: 'words' (default) reports word-level add/remove/replace hunks — best for contracts and prose; 'lines' diffs whole extracted lines — better for code-like or tabular documents."
                    },
                    "align": {
                        "type": "string",
                        "enum": ["auto", "sequential"],
                        "default": "auto",
                        "description": "How pages are paired: 'auto' (default) aligns pages by content similarity so an inserted or deleted page is reported as added/removed instead of shifting every later page (documents up to 200 pages; beyond that it falls back to sequential); 'sequential' always compares page N with page N."
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "default": false,
                        "description": "Ignore letter case in the text comparison (default false). 'Payment' vs 'payment' stops counting as a change; the reported hunks still show the original spelling."
                    },
                    "include_unchanged": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also list the page pairs that did NOT change (default false — only changed pages, added/removed pages and summary counts are reported)."
                    }
                },
                "required": ["files"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
