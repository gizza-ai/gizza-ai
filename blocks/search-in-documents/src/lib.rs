//! gizza-ai/search-in-documents — search a regex or keyword inside a PDF, DOCX,
//! EPUB, or ZIP archive and return each match with the document and page/section
//! it came from.
//!
//! Pipeline: parse `{url|ref}` + the search params → fetch the document bytes via
//! `block-utils` `resolve_source` (URL fetch through `wafer-run/network`, or an
//! uploaded attachment ref) → delegate to the pure `core::search` (location-aware
//! extraction + regex/keyword match) → return the matches as a flat JSON response
//! the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the search, emit
//! the flat `Resp` JSON) rather than going through `run_skill`, because the
//! success shape is the flat `Resp` JSON, not the `{ "result": … }` wrapper.
//!
//! No page surface: the input is a binary document file and the output is
//! structured text, which fits neither the pure-text nor the ffmpeg file→media
//! page shapes — this is a chat + CLI block (the "no-page file-input" pattern,
//! like `pdf-extract-text` / `document-text-extract`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests. Block-local helpers stay native-compilable so
// the unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{derive_filename, resolve_source};
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_search_in_documents_core::{search, SearchOptions, MAX_MATCHES_CAP};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

fn default_max_matches() -> usize {
    200
}

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// The keyword or regular expression to search for.
    pattern: String,
    /// Treat `pattern` as a regular expression (default: literal substring).
    #[serde(default)]
    regex: bool,
    /// Match case exactly (default: case-insensitive).
    #[serde(default)]
    case_sensitive: bool,
    /// Only match whole words (default: off).
    #[serde(default)]
    whole_word: bool,
    /// Maximum number of matching lines to return.
    #[serde(default = "default_max_matches")]
    max_matches: usize,
}

#[derive(Serialize)]
struct MatchJson {
    /// The source document: the input's name, or the entry path inside a ZIP.
    document: String,
    /// Where the match is: `"page N"` (PDF) or `"line N"` (DOCX/EPUB/text).
    location: String,
    /// 1-based line number within the located unit.
    line: usize,
    /// The matching line, with each hit wrapped in guillemets («…»).
    text: String,
}

#[derive(Serialize)]
struct Resp {
    /// The detected format: `"pdf"`, `"docx"`, `"epub"`, or `"zip"`.
    format: String,
    /// Number of documents searched (1 for a single file; the entry count for a ZIP).
    documents_searched: usize,
    /// Number of matching lines returned.
    total_matches: usize,
    /// The matches, in document order.
    matches: Vec<MatchJson>,
    /// True when the match cap was reached and further matches were dropped.
    truncated: bool,
    /// Set when some content could not be searched (unsupported PDF font
    /// encoding, or non-text archive entries). Omitted when the search was
    /// complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a document arrives via URL fetch or an
/// attachment ref); `pattern` is the required query; the rest are the search
/// modifiers.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::string("pattern")
                .required()
                .describe("The keyword or regular expression to search for."),
        )
        .param(Param::boolean("regex").default(false).describe(
            "Treat the pattern as a regular expression. When off (default), it is \
             matched as a literal substring.",
        ))
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("Match case exactly. When off (default), the search is case-insensitive."),
        )
        .param(
            Param::boolean("whole_word")
                .default(false)
                .describe("Only match whole words (word boundaries on both sides)."),
        )
        .param(
            Param::integer("max_matches")
                .min(1.0)
                .max(MAX_MATCHES_CAP as f64)
                .default(200)
                .describe("Maximum number of matching lines to return (1–1000)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct SearchInDocuments;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/search-in-documents",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Search a regex or keyword inside a PDF, DOCX, EPUB, or ZIP archive",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Search a regex or keyword INSIDE a PDF, DOCX, EPUB, or ZIP archive and return each match with the document and location it came from. Provide url (HTTP/HTTPS) or ref from a prior tool call, and a pattern. By default the pattern is a literal, case-insensitive substring; set regex to treat it as a regular expression, case_sensitive to match case exactly, and whole_word to match whole words only. PDF matches report the page number; DOCX/EPUB/text report the line number; ZIP archives search each PDF/text entry and tag matches with the entry path. Extracts embedded text only (no OCR of scanned pages). Returns the matching lines with each hit wrapped in guillemets («…»).",
        parameters = schema_json()
    ),
)]
impl SearchInDocuments {
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
    let args: Args = serde_json::from_slice(&body).invalid_args("search-in-documents")?;
    if args.pattern.is_empty() {
        return Err(SkillError::InvalidArgs(
            "pattern must not be empty — provide a keyword or regular expression".to_string(),
        ));
    }
    if args.max_matches == 0 {
        return Err(SkillError::InvalidArgs(
            "max_matches must be >= 1".to_string(),
        ));
    }

    // 2. Resolve source — URL fetch or attachment lookup. `AssetKind::Any`: the
    //    real format is detected from the file's magic bytes (a server may label
    //    a DOCX/EPUB/ZIP as octet-stream), so don't gate on the declared MIME.
    let src = args.source.into_inner();
    let doc_name = match &src {
        gizza_ai_block_utils::Source::Url(u) => derive_filename(u, "document"),
        gizza_ai_block_utils::Source::Ref(_) => "document".to_string(),
    };
    let (input_bytes, _mime, filename) = resolve_source(src, AssetKind::Any, MAX_INPUT_BYTES)?;
    // Prefer the resolved attachment filename when present.
    let doc_name = if filename.is_empty() {
        doc_name
    } else {
        filename
    };

    // 3. Search via the pure core. Maps parse/pattern errors to InvalidArgs.
    let opts = SearchOptions {
        regex: args.regex,
        case_sensitive: args.case_sensitive,
        whole_word: args.whole_word,
        max_matches: args.max_matches,
    };
    let outcome =
        search(&input_bytes, &args.pattern, &doc_name, &opts).map_err(SkillError::InvalidArgs)?;

    let matches = outcome
        .matches
        .into_iter()
        .map(|m| MatchJson {
            document: m.document,
            location: m.location,
            line: m.line,
            text: m.text,
        })
        .collect::<Vec<_>>();

    let resp = Resp {
        format: outcome.format,
        documents_searched: outcome.documents_searched,
        total_matches: matches.len(),
        matches,
        truncated: outcome.truncated,
        note: outcome.note,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize search-in-documents response: {e}")))
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Migration/consistency guard: the descriptor-derived chat schema must match
    /// this authored blob, so the LLM sees a stable shape. `Input::Document`
    /// single-sources the `url`/`ref` `oneOf`; the search params follow.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "pattern": { "type": "string", "description": "The keyword or regular expression to search for." },
                    "regex": { "type": "boolean", "default": false, "description": "Treat the pattern as a regular expression. When off (default), it is matched as a literal substring." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "Match case exactly. When off (default), the search is case-insensitive." },
                    "whole_word": { "type": "boolean", "default": false, "description": "Only match whole words (word boundaries on both sides)." },
                    "max_matches": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 200, "description": "Maximum number of matching lines to return (1–1000)." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ],
                "required": ["pattern"]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_and_pattern() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://x/y.pdf","pattern":"invoice"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.pattern, "invoice");
        assert!(!a.regex);
        assert!(!a.case_sensitive);
        assert!(!a.whole_word);
        assert_eq!(a.max_matches, 200);
    }

    #[test]
    fn args_parse_modifiers() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_7","pattern":"\\d+","regex":true,"case_sensitive":true,"whole_word":true,"max_matches":5}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert!(a.regex);
        assert!(a.case_sensitive);
        assert!(a.whole_word);
        assert_eq!(a.max_matches, 5);
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err =
            serde_json::from_str::<Args>(r#"{"url":"u","ref":"r","pattern":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_require_pattern() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u"}"#).unwrap_err();
        assert!(err.to_string().contains("pattern"));
    }
}
