//! gizza-ai/pdf-search — search a PDF for a literal word or phrase and list the
//! matching pages with surrounding context.
//!
//! Pipeline: parse `{url|ref}` + `query` + options → fetch the PDF bytes via
//! `block-utils` `resolve_source` (URL fetch through `wafer-run/network`, or an
//! uploaded attachment ref) → delegate to the pure `core::search` (lopdf) →
//! return page-numbered `«…»`-wrapped snippets plus totals as a flat JSON
//! response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the search,
//! emit the flat `Resp` JSON) rather than going through `run_skill`, because
//! pdf-search's success shape is the flat `Resp` JSON, not the
//! `{ "result": … }` wrapper `run_skill` produces.
//!
//! No page surface: a PDF is a binary file input and the output is structured
//! text, which fits neither the pure-text nor the ffmpeg file→media page shapes
//! — this is a chat + CLI block (the "no-page file-input" pattern, like the
//! sibling `pdf-extract-text` and every other PDF-*input* block in this repo).
//!
//! Limits: text-layer PDFs only (no OCR of scanned/image-only PDFs); literal
//! word/phrase only (no regex). See the `core` module docs.

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
use gizza_ai_pdf_search_core::{search, SearchOptions};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

// Parameter defaults + bounds. Kept in one place so the descriptor (schema),
// the handler clamping, and the tests agree.
const CONTEXT_DEFAULT: usize = 60;
const CONTEXT_MAX: usize = 500;
const MAX_MATCHES_DEFAULT: usize = 100;
const MAX_MATCHES_MIN: usize = 1;
const MAX_MATCHES_MAX: usize = 1000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// The literal word or phrase to search for (required).
    query: String,
    /// Match case exactly. Default false (case-insensitive).
    #[serde(default)]
    case_sensitive: bool,
    /// Require whole-word matches (alphanumeric boundaries). Default false.
    #[serde(default)]
    whole_word: bool,
    /// Characters of context on each side of a match. Omitted = default.
    #[serde(default)]
    context: Option<usize>,
    /// Maximum number of match snippets to return. Omitted = default.
    #[serde(default)]
    max_matches: Option<usize>,
}

#[derive(Serialize)]
struct MatchOut {
    /// 1-based page number the match was found on.
    page: usize,
    /// Context snippet with the matched span wrapped in `«…»`.
    snippet: String,
}

#[derive(Serialize)]
struct Resp {
    /// The (normalized) query that was searched for.
    query: String,
    /// Matches in document order, capped at `max_matches`.
    matches: Vec<MatchOut>,
    /// Total occurrences across the whole document (may exceed `matches.len()`
    /// when the returned list was capped).
    total_matches: usize,
    /// Number of distinct pages that contained at least one match.
    pages_matched: usize,
    /// True when `matches` was capped at `max_matches`.
    truncated: bool,
    /// Set when some text runs could not be decoded (e.g. an unparseable font
    /// `ToUnicode` CMap), so the searched text was partial. Omitted otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a PDF arrives via URL fetch or an attachment
/// ref). `query` is the only required param; the rest tune matching/output.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::string("query")
                .required()
                .describe("The literal word or phrase to search for. Whitespace is normalized, so a multi-word phrase still matches where the PDF split it across lines. Not a regular expression.")
                .placeholder("invoice total"),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("Match the exact case. Default false (case-insensitive)."),
        )
        .param(
            Param::boolean("whole_word")
                .default(false)
                .describe("Only match whole words (alphanumeric word boundaries on both sides), so \"cat\" does not match \"category\". Default false."),
        )
        .param(
            Param::integer("context")
                .default(CONTEXT_DEFAULT as i64)
                .min(0.0)
                .max(CONTEXT_MAX as f64)
                .describe("Number of characters of surrounding context to show on each side of a match. Default 60."),
        )
        .param(
            Param::integer("max_matches")
                .default(MAX_MATCHES_DEFAULT as i64)
                .min(MAX_MATCHES_MIN as f64)
                .max(MAX_MATCHES_MAX as f64)
                .describe("Maximum number of match snippets to return. total_matches still counts every occurrence and truncated flags when the list is capped. Default 100."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Clamp an optional user-supplied value into `[min, max]`, falling back to
/// `default` when omitted.
fn clamp_opt(v: Option<usize>, default: usize, min: usize, max: usize) -> usize {
    v.unwrap_or(default).clamp(min, max)
}

#[cfg(target_arch = "wasm32")]
struct PdfSearch;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-search",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Search a PDF for a word or phrase and list matching pages with context",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Search a PDF for a literal word or phrase and list every matching page with surrounding context. Provide url (HTTP/HTTPS) or ref from a prior tool call, and a query. Options: case_sensitive (default false), whole_word (default false), context characters per side (default 60), max_matches (default 100). Each hit returns its 1-based page and a snippet with the match wrapped in «…». Searches the embedded selectable text layer only — it does not OCR scanned/image-only PDFs, and the query is literal text, not a regular expression.",
        parameters = schema_json()
    ),
)]
impl PdfSearch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // pdf-search returns the flat Resp JSON directly (no `{ "result": … }`
        // wrapper), so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-search")?;
    if args.query.trim().is_empty() {
        return Err(SkillError::InvalidArgs("query must not be empty".to_string()));
    }
    let opts = SearchOptions {
        case_sensitive: args.case_sensitive,
        whole_word: args.whole_word,
        context: clamp_opt(args.context, CONTEXT_DEFAULT, 0, CONTEXT_MAX),
        max_matches: clamp_opt(
            args.max_matches,
            MAX_MATCHES_DEFAULT,
            MAX_MATCHES_MIN,
            MAX_MATCHES_MAX,
        ),
    };

    // 2. Resolve source — URL fetch or attachment lookup, validated to the
    //    application/* document MIME class.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    // 3. Search via the pure core (lopdf). Maps parse/empty-query errors to
    //    InvalidArgs.
    let out = search(&input_bytes, &args.query, &opts).map_err(SkillError::InvalidArgs)?;
    let note = (out.dropped_chunks > 0).then(|| {
        format!(
            "{} text run(s) could not be decoded (unsupported font encoding); searched text is partial",
            out.dropped_chunks
        )
    });

    let resp = Resp {
        query: {
            // Report the whitespace-normalized query the core actually matched.
            let normalized: Vec<&str> = args.query.split_whitespace().collect();
            normalized.join(" ")
        },
        matches: out
            .matches
            .into_iter()
            .map(|m| MatchOut {
                page: m.page,
                snippet: m.snippet,
            })
            .collect(),
        total_matches: out.total_matches,
        pages_matched: out.pages_matched,
        truncated: out.truncated,
        note,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-search response: {e}")))
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;

    use super::*;

    /// Schema-drift guard: the descriptor-derived chat schema must stay exactly
    /// this shape, so the LLM (and the page/CLI, which read the same source)
    /// see no drift. `Input::Document` fixes the `url`/`ref` `oneOf`;
    /// `additionalProperties: false` is emitted uniformly.
    #[test]
    fn schema_json_matches_expected() {
        let expected: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "query": {
                        "type": "string",
                        "description": "The literal word or phrase to search for. Whitespace is normalized, so a multi-word phrase still matches where the PDF split it across lines. Not a regular expression."
                    },
                    "case_sensitive": {
                        "type": "boolean",
                        "default": false,
                        "description": "Match the exact case. Default false (case-insensitive)."
                    },
                    "whole_word": {
                        "type": "boolean",
                        "default": false,
                        "description": "Only match whole words (alphanumeric word boundaries on both sides), so \"cat\" does not match \"category\". Default false."
                    },
                    "context": {
                        "type": "integer",
                        "default": 60,
                        "minimum": 0,
                        "maximum": 500,
                        "description": "Number of characters of surrounding context to show on each side of a match. Default 60."
                    },
                    "max_matches": {
                        "type": "integer",
                        "default": 100,
                        "minimum": 1,
                        "maximum": 1000,
                        "description": "Maximum number of match snippets to return. total_matches still counts every occurrence and truncated flags when the list is capped. Default 100."
                    }
                },
                "additionalProperties": false,
                "required": ["query"],
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, expected, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_query_and_options() {
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.pdf","query":"fox","case_sensitive":true,"whole_word":true,"context":20,"max_matches":5}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
        assert_eq!(a.query, "fox");
        assert!(a.case_sensitive);
        assert!(a.whole_word);
        assert_eq!(a.context, Some(20));
        assert_eq!(a.max_matches, Some(5));
    }

    #[test]
    fn args_default_options_when_omitted() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7","query":"hi"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
        assert!(!a.case_sensitive);
        assert!(!a.whole_word);
        assert_eq!(a.context, None);
        assert_eq!(a.max_matches, None);
    }

    #[test]
    fn args_require_query() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u"}"#).unwrap_err();
        assert!(err.to_string().contains("query"), "got: {err}");
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r","query":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn clamp_opt_applies_default_and_bounds() {
        assert_eq!(clamp_opt(None, 60, 0, 500), 60);
        assert_eq!(clamp_opt(Some(9999), 60, 0, 500), 500);
        assert_eq!(clamp_opt(Some(0), 100, 1, 1000), 1);
        assert_eq!(clamp_opt(Some(42), 100, 1, 1000), 42);
    }
}
