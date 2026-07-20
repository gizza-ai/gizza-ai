//! gizza-ai/browser-artifact-parser — read an uploaded browser **artifact
//! database** (Chrome/Edge, Firefox, or Safari history, cookies, downloads, or
//! cache) and correlate every time-stamped record into one unified, searchable
//! forensic timeline.
//!
//! No-page block (chat + CLI surface only, like `blocks/browser-history-parser`,
//! `blocks/sqlite-table-to-csv`, and `blocks/xlsx-to-csv`): it ingests a binary
//! SQLite file, which is neither a pure-text page input nor an ffmpeg media
//! transform, so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
//!
//! Pipeline: parse `{url|ref}` + timeline options → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup) →
//! `core::parse_artifacts(bytes, &Options)` (auto-detects every recognized
//! artifact table, converts each source's timestamp epoch to UTC, decodes visit
//! types, and merges the records into one timeline — all via the reused on-disk
//! SQLite reader, no SQL engine) → render JSON or CSV → emit a text `Envelope`.
//! The LLM sees the timeline (head-truncated if large) plus counts; the UI gets
//! a downloadable file.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// pure artifact parsing, descriptor, and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_browser_artifact_parser_core::{
    parse_artifacts, render_csv, render_json, KindFilter, Options, Order,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the database input we accept. The parser holds the whole file in
/// memory and walks it lazily, but a hard cap keeps memory bounded.
const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB — cache/history DBs get large

/// Cap on the timeline text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full timeline is always available via
/// `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB of text

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Case-insensitive substring; keep only events whose URL/host/title/detail matches.
    #[serde(default)]
    search: Option<String>,
    /// Restrict to one event kind: all | visit | download | cookie | cache.
    #[serde(default)]
    kind: Option<String>,
    /// Timeline sort order: newest | oldest.
    #[serde(default)]
    order: Option<String>,
    /// Max events to return; 0 = all (default 0).
    #[serde(default)]
    limit: Option<i64>,
    /// Output format: json | csv.
    #[serde(default)]
    format: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). The drift-guard test
/// below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("search").describe(
            "Only include events whose URL, host, title, or details contain this text (case-insensitive). Omit to return every event.",
        ))
        .param(
            Param::enumv("kind", ["all", "visit", "download", "cookie", "cache"])
                .default("all")
                .describe("Restrict the timeline to one event kind: all (the default), visit (page history), download, cookie, or cache."),
        )
        .param(
            Param::enumv("order", ["newest", "oldest"])
                .default("newest")
                .describe("Timeline sort order by event time: newest (most recent first, the default) or oldest."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Maximum number of events to return; 0 means all events. Default 0."),
        )
        .param(
            Param::enumv("format", ["json", "csv"])
                .default("json")
                .describe("Output format: json (structured, the default) or csv (a spreadsheet-friendly table with a source column so exports from several artifact files merge)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Which serialization the caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Csv,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "csv" => Ok(Format::Csv),
            other => Err(format!("unknown format {other:?} (expected: json or csv)")),
        }
    }
}

/// Build the core `Options` and the output `Format` from the parsed args.
fn options_from_args(args: &Args) -> Result<(Options, Format), SkillError> {
    let kind = match args.kind.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(k) => KindFilter::parse(k).map_err(SkillError::InvalidArgs)?,
        None => KindFilter::All,
    };
    let order = match args.order.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(o) => Order::parse(o).map_err(SkillError::InvalidArgs)?,
        None => Order::Newest,
    };
    let limit = match args.limit {
        Some(n) if n < 0 => return Err(SkillError::InvalidArgs("`limit` must be >= 0".into())),
        Some(n) => n as usize,
        None => 0,
    };
    let format = match args.format.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(f) => Format::parse(f).map_err(SkillError::InvalidArgs)?,
        None => Format::Json,
    };
    let search = args
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok((
        Options {
            search,
            kind,
            order,
            limit,
        },
        format,
    ))
}

/// Build the `_for_llm` summary text.
fn summarize_for_llm(
    out: &gizza_ai_browser_artifact_parser_core::ArtifactOutput,
    body: &str,
) -> String {
    let mut head = format!(
        "Parsed {} artifact(s) [{}] from {}: {} total event(s) ({} visits, {} downloads, {} cookies, {} cache); {} matched the filters, {} returned{}.\n\n",
        out.artifacts.len(),
        out.artifacts.join(", "),
        out.browsers.join(", "),
        out.total_events,
        out.counts.visits,
        out.counts.downloads,
        out.counts.cookies,
        out.counts.cache,
        out.matched,
        out.returned,
        if out.truncated { " (truncated by limit)" } else { "" },
    );
    if body.chars().count() <= MAX_LLM_CHARS {
        head.push_str(body);
    } else {
        let b: String = body.chars().take(MAX_LLM_CHARS).collect();
        head.push_str(&format!(
            "(first {MAX_LLM_CHARS} of {} chars; full timeline in the download)\n{b}",
            body.chars().count()
        ));
    }
    head
}

#[cfg(target_arch = "wasm32")]
struct BrowserArtifactParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/browser-artifact-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge browser history, cookies, downloads, and cache databases into one forensic timeline",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse an uploaded browser artifact database into one unified, searchable forensic timeline. Auto-detects and correlates every recognized artifact: Chrome/Edge/Chromium history (urls+visits) and downloads, Chrome/Edge cookies, Firefox places.sqlite history and legacy downloads, Firefox cookies.sqlite, Safari History.db, and Safari/WebKit Cache.db — by reading the on-disk SQLite file directly (no SQL engine, read-only). Each timeline event has a readable UTC timestamp, unix seconds, event kind (visit/download/cookie/cache), source (browser + artifact), URL/host, name (page title, filename, or cookie name), and info (visit type, download size, or cookie expiry). A single History file yields both visits and downloads, merged chronologically. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Filter by substring (`search`) or `kind`, sort newest or oldest (`order`), cap the rows (`limit`), and export as json or csv.",
        parameters = schema_json()
    ),
)]
impl BrowserArtifactParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("browser-artifact-parser")?;
    let (opts, format) = options_from_args(&args)?;

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let out = parse_artifacts(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let (rendered, mime, filename) = match format {
        Format::Json => (
            render_json(&out),
            "application/json",
            "artifact-timeline.json",
        ),
        Format::Csv => (render_csv(&out), "text/csv", "artifact-timeline.csv"),
    };

    let for_llm = summarize_for_llm(&out, &rendered);
    let data_url = format!("data:{mime};base64,{}", B64.encode(rendered.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename: filename.to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration/authoring safety: the descriptor-derived chat schema must match
    /// the authored schema (drift guard). Object key order is irrelevant — both
    /// sides are parsed to `serde_json::Value` before comparison.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":   { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":   { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "search": { "type": "string", "description": "Only include events whose URL, host, title, or details contain this text (case-insensitive). Omit to return every event." },
                    "kind": {
                        "type": "string",
                        "enum": ["all", "visit", "download", "cookie", "cache"],
                        "default": "all",
                        "description": "Restrict the timeline to one event kind: all (the default), visit (page history), download, cookie, or cache."
                    },
                    "order": {
                        "type": "string",
                        "enum": ["newest", "oldest"],
                        "default": "newest",
                        "description": "Timeline sort order by event time: newest (most recent first, the default) or oldest."
                    },
                    "limit": { "type": "integer", "default": 0, "minimum": 0, "description": "Maximum number of events to return; 0 means all events. Default 0." },
                    "format": {
                        "type": "string",
                        "enum": ["json", "csv"],
                        "default": "json",
                        "description": "Output format: json (structured, the default) or csv (a spreadsheet-friendly table with a source column so exports from several artifact files merge)."
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
    fn options_defaults_when_unspecified() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/History"}"#).unwrap();
        let (o, f) = options_from_args(&a).unwrap();
        assert!(o.search.is_none());
        assert_eq!(o.kind, KindFilter::All);
        assert_eq!(o.order, Order::Newest);
        assert_eq!(o.limit, 0);
        assert_eq!(f, Format::Json);
    }

    #[test]
    fn options_parse_all_fields() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","search":"github","kind":"cookie","order":"oldest","limit":50,"format":"csv"}"#,
        )
        .unwrap();
        let (o, f) = options_from_args(&a).unwrap();
        assert_eq!(o.search.as_deref(), Some("github"));
        assert_eq!(
            o.kind,
            KindFilter::Only(gizza_ai_browser_artifact_parser_core::Kind::Cookie)
        );
        assert_eq!(o.order, Order::Oldest);
        assert_eq!(o.limit, 50);
        assert_eq!(f, Format::Csv);
    }

    #[test]
    fn blank_search_is_treated_as_none() {
        let a: Args = serde_json::from_str(r#"{"url":"u","search":"   "}"#).unwrap();
        let (o, _) = options_from_args(&a).unwrap();
        assert!(o.search.is_none());
    }

    #[test]
    fn options_reject_bad_kind() {
        let a: Args = serde_json::from_str(r#"{"url":"u","kind":"bookmarks"}"#).unwrap();
        assert!(options_from_args(&a).is_err());
    }

    #[test]
    fn options_reject_bad_order() {
        let a: Args = serde_json::from_str(r#"{"url":"u","order":"sideways"}"#).unwrap();
        assert!(options_from_args(&a).is_err());
    }

    #[test]
    fn options_reject_bad_format() {
        let a: Args = serde_json::from_str(r#"{"url":"u","format":"xml"}"#).unwrap();
        assert!(options_from_args(&a).is_err());
    }

    #[test]
    fn options_reject_negative_limit() {
        let a: Args = serde_json::from_str(r#"{"url":"u","limit":-3}"#).unwrap();
        assert!(options_from_args(&a).is_err());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"search":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }
}
