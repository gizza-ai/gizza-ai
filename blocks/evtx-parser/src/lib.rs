//! gizza-ai/evtx-parser — read a Windows Event Log (`.evtx`) into structured JSON.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref) →
//! `core::parse` (the `evtx` binary chunk parser + filtering/aggregation) → flat
//! JSON the LLM reads directly: a summary of counts plus a list of records, each
//! with its record id, timestamp, event id, provider, level, channel, computer,
//! and (optionally) the full parsed `System`/`EventData` object.
//!
//! Filters: by event id, provider name, channel, and an inclusive ISO-8601 time
//! range; cap the number of returned records; or switch to `summary=true` for
//! aggregate counts (by event id / provider / level) and the file's time span
//! instead of the records.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page — a binary file input with structured JSON
//! output fits neither the pure-text page nor the ffmpeg media page shape (the
//! no-page file-input pattern, like `epub-extract` / `pdf-extract-text`).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_evtx_parser_core::{parse, parse_bound, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB — EVTX files can be large
const DEFAULT_MAX_RECORDS: i64 = 100;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    event_ids: String,
    #[serde(default)]
    providers: String,
    #[serde(default)]
    channel: String,
    #[serde(default)]
    after: String,
    #[serde(default)]
    before: String,
    #[serde(default = "default_max_records")]
    max_records: i64,
    #[serde(default = "default_true")]
    include_data: bool,
    #[serde(default)]
    summary: bool,
}

fn default_true() -> bool {
    true
}
fn default_max_records() -> i64 {
    DEFAULT_MAX_RECORDS
}

/// Split a comma/space/semicolon-separated list into trimmed non-empty items.
fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';', '\n'])
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn build_options(args: &Args) -> Result<Options, SkillError> {
    let event_ids = split_list(&args.event_ids)
        .iter()
        .map(|t| {
            t.parse::<u64>()
                .map_err(|_| SkillError::InvalidArgs(format!("invalid event id {t:?}: expected a number")))
        })
        .collect::<Result<Vec<u64>, _>>()?;

    let channel = {
        let c = args.channel.trim();
        if c.is_empty() {
            None
        } else {
            Some(c.to_string())
        }
    };

    let after = if args.after.trim().is_empty() {
        None
    } else {
        Some(parse_bound(&args.after).map_err(SkillError::InvalidArgs)?)
    };
    let before = if args.before.trim().is_empty() {
        None
    } else {
        Some(parse_bound(&args.before).map_err(SkillError::InvalidArgs)?)
    };

    Ok(Options {
        max_records: args.max_records.max(0) as usize,
        event_ids,
        providers: split_list(&args.providers),
        channel,
        after,
        before,
        include_data: args.include_data,
        summary: args.summary,
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("event_ids")
                .describe("Only return records with these Windows Event IDs. Comma-separated list of numbers, e.g. \"4624,4634,4688\". Empty (default) returns every event."),
        )
        .param(
            Param::string("providers")
                .describe("Only return records whose provider (source) name contains one of these, case-insensitive. Comma-separated, e.g. \"Security-Auditing,Sysmon\". Empty (default) matches all providers."),
        )
        .param(
            Param::string("channel")
                .describe("Only return records on this log channel, case-insensitive exact match, e.g. \"Security\", \"System\", \"Application\". Empty (default) matches all channels."),
        )
        .param(
            Param::string("after")
                .describe("Only records at or after this instant. ISO-8601, e.g. \"2016-06-29T15:00:00Z\" or a bare date \"2016-06-29\" (start of that UTC day). Empty (default) = no lower bound."),
        )
        .param(
            Param::string("before")
                .describe("Only records at or before this instant. Same format as `after`. Empty (default) = no upper bound."),
        )
        .param(
            Param::integer("max_records")
                .default(DEFAULT_MAX_RECORDS)
                .min(0.0)
                .describe("Maximum number of records to return, in file order (0 = all). Default 100 keeps output bounded; the response's `matched_records`/`truncated` tell you if more matched."),
        )
        .param(
            Param::boolean("include_data")
                .default(true)
                .describe("Include the full parsed record object (System + EventData) under `data` for each record (default true). Set false for a compact list of just the summary fields."),
        )
        .param(
            Param::boolean("summary")
                .default(false)
                .describe("Return aggregate counts instead of the records: totals by event id, provider, and level, plus the earliest/latest timestamp over the matched records. Great for triaging a large log before drilling in."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EvtxParserTool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/evtx-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a Windows .evtx event log into filterable structured JSON",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse a Windows Event Log (.evtx) file into structured JSON. Decodes the binary EVTX chunks into a flat list of records — each with its record id, timestamp, event id, provider (source), level (with name), channel, computer, and the full parsed System/EventData object. Filter by event id, provider name, channel, and an inclusive ISO-8601 time range (after/before); cap results with max_records; or set summary=true for aggregate counts (by event id, provider, and level) plus the file's time span instead of the records — ideal for triaging a large log first. Provide the .evtx as url (HTTP/HTTPS) or ref from a prior tool call. Runs locally, pure Rust.",
        parameters = schema_json()
    ),
)]
impl EvtxParserTool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("evtx-parser")?;
    let opts = build_options(&args)?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let report = parse(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&report)
        .map_err(|e| SkillError::Serialize(format!("serialize evtx-parser response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "event_ids": { "type": "string", "description": "Only return records with these Windows Event IDs. Comma-separated list of numbers, e.g. \"4624,4634,4688\". Empty (default) returns every event." },
                    "providers": { "type": "string", "description": "Only return records whose provider (source) name contains one of these, case-insensitive. Comma-separated, e.g. \"Security-Auditing,Sysmon\". Empty (default) matches all providers." },
                    "channel": { "type": "string", "description": "Only return records on this log channel, case-insensitive exact match, e.g. \"Security\", \"System\", \"Application\". Empty (default) matches all channels." },
                    "after": { "type": "string", "description": "Only records at or after this instant. ISO-8601, e.g. \"2016-06-29T15:00:00Z\" or a bare date \"2016-06-29\" (start of that UTC day). Empty (default) = no lower bound." },
                    "before": { "type": "string", "description": "Only records at or before this instant. Same format as `after`. Empty (default) = no upper bound." },
                    "max_records": { "type": "integer", "default": 100, "minimum": 0, "description": "Maximum number of records to return, in file order (0 = all). Default 100 keeps output bounded; the response's `matched_records`/`truncated` tell you if more matched." },
                    "include_data": { "type": "boolean", "default": true, "description": "Include the full parsed record object (System + EventData) under `data` for each record (default true). Set false for a compact list of just the summary fields." },
                    "summary": { "type": "boolean", "default": false, "description": "Return aggregate counts instead of the records: totals by event id, provider, and level, plus the earliest/latest timestamp over the matched records. Great for triaging a large log before drilling in." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn split_list_handles_separators() {
        assert_eq!(split_list("4624, 4634 ;4688"), vec!["4624", "4634", "4688"]);
        assert!(split_list("   ").is_empty());
    }
}
