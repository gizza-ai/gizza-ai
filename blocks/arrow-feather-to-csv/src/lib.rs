//! gizza-ai/arrow-feather-to-csv — convert an Apache Arrow IPC / Feather (V2)
//! file to CSV.
//!
//! No-page block (chat + CLI surface only, like `blocks/xlsx-to-csv` /
//! `blocks/web-fetch`): it ingests binary Arrow bytes, which is neither a
//! pure-text page input nor an ffmpeg media transform, so there is no standalone
//! page. The chat schema is derived from `descriptor()` (single source shared
//! across chat + CLI).
//!
//! Pipeline: parse `{url|ref}` + optional `delimiter`/`header`/`null` → resolve
//! bytes via `block_utils::resolve_source` (URL fetch or attachment lookup; any
//! bytes, since Arrow files are commonly served as `application/octet-stream`)
//! → `core::to_csv` (arrow-ipc reader + arrow-csv writer) → emit a text
//! `Envelope`. The LLM sees the CSV (head-truncated if large); the UI gets a
//! downloadable `data:text/csv` URL + `*.csv` filename.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_arrow_feather_to_csv_core::to_csv;
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the Arrow input we accept. Arrow buffers decode in full and the CSV
/// text can be several times larger, so keep well inside the wasm sandbox.
const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Cap on the CSV text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full CSV is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB of CSV text

fn default_delimiter() -> String {
    ",".to_string()
}
fn default_header() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// CSV field separator. A single character, or `"tab"`.
    #[serde(default = "default_delimiter")]
    delimiter: String,
    /// Write a leading row of column names.
    #[serde(default = "default_header")]
    header: bool,
    /// Text written for null / missing cells.
    #[serde(default)]
    null: String,
    /// Comma-separated columns to keep (name or 0-based index). Empty = all.
    #[serde(default)]
    columns: String,
    /// Max data rows to emit (0 = all).
    #[serde(default)]
    limit: u64,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::File` emits
/// the `url`⊕`ref` `oneOf`; the three optional params tune the CSV output.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("delimiter").default(",").describe(
                "CSV field separator: a single character, or the word \"tab\" for a tab. Defaults to a comma.",
            ),
        )
        .param(
            Param::boolean("header").default(true).describe(
                "Write a first row of column names. Set false to omit the header. Defaults to true.",
            ),
        )
        .param(
            Param::string("null").default("").describe(
                "Text to write for null / missing values. Defaults to an empty field.",
            ),
        )
        .param(
            Param::string("columns").default("").describe(
                "Comma-separated columns to keep, by name or 0-based index, in the order given (e.g. \"name,0,price\"). Leave empty to keep every column in its original order.",
            ),
        )
        .param(
            Param::integer("limit").default(0).describe(
                "Maximum number of data rows to output; 0 (the default) writes every row. Use a small value to preview a large table.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Build the `_for_llm` text: the full CSV when small, else a head plus a note.
fn summarize_for_llm(csv: &str, filename: &str) -> String {
    if csv.chars().count() <= MAX_LLM_CHARS {
        format!("CSV from {filename}:\n{csv}")
    } else {
        let head: String = csv.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "CSV from {filename} (first {MAX_LLM_CHARS} of {} chars; full file in the download):\n{head}",
            csv.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct ArrowFeatherToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/arrow-feather-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an Apache Arrow IPC / Feather file to CSV",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert an Apache Arrow IPC / Feather (V2) file to CSV. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Reads the standard Arrow IPC file and stream formats, including LZ4-compressed buffers; every value is rendered to text (timestamps/dates as RFC-3339). Optional: `delimiter` (default comma; use \"tab\" for TSV), `header` (default true), `null` text (default empty), `columns` (keep/reorder a subset by name or 0-based index), and `limit` (cap the number of rows for a preview). ZSTD-compressed and legacy Feather V1 files are not supported.",
        parameters = schema_json()
    ),
)]
impl ArrowFeatherToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("arrow-feather-to-csv")?;

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let name = if filename.is_empty() {
        "table.arrow".to_string()
    } else {
        filename.clone()
    };

    let csv = to_csv(
        &bytes,
        &args.delimiter,
        args.header,
        &args.null,
        &args.columns,
        args.limit as usize,
    )
    .map_err(SkillError::InvalidArgs)?;

    let for_llm = summarize_for_llm(&csv, &name);
    let csv_filename = replace_extension(&name, "csv");
    let data_url = format!("data:text/csv;base64,{}", B64.encode(csv.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "text/csv".to_string(),
            filename: csv_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored one, so the LLM sees no drift. `url`/`ref` wording is
    /// centralized in `to_schema_json` (shared by every File/media tool).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field separator: a single character, or the word \"tab\" for a tab. Defaults to a comma." },
                    "header":    { "type": "boolean", "default": true, "description": "Write a first row of column names. Set false to omit the header. Defaults to true." },
                    "null":      { "type": "string", "default": "", "description": "Text to write for null / missing values. Defaults to an empty field." },
                    "columns":   { "type": "string", "default": "", "description": "Comma-separated columns to keep, by name or 0-based index, in the order given (e.g. \"name,0,price\"). Leave empty to keep every column in its original order." },
                    "limit":     { "type": "integer", "default": 0, "description": "Maximum number of data rows to output; 0 (the default) writes every row. Use a small value to preview a large table." }
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
    fn summarize_short_csv_includes_full_text() {
        let csv = "id,name\n1,Ann\n";
        let s = summarize_for_llm(csv, "data.arrow");
        assert!(s.contains("CSV from data.arrow"));
        assert!(s.contains("1,Ann"));
    }

    #[test]
    fn summarize_long_csv_truncates_with_note() {
        let csv = "x".repeat(MAX_LLM_CHARS + 100);
        let s = summarize_for_llm(&csv, "big.feather");
        assert!(s.contains("full file in the download"));
        assert!(s.len() < csv.len() + 200);
    }

    #[test]
    fn args_defaults_apply() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.arrow"}"#).unwrap();
        assert_eq!(a.delimiter, ",");
        assert!(a.header);
        assert_eq!(a.null, "");
        assert_eq!(a.columns, "");
        assert_eq!(a.limit, 0);
    }

    #[test]
    fn args_parse_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","delimiter":"tab","header":false,"null":"NA","columns":"name,id","limit":5}"#,
        )
        .unwrap();
        assert_eq!(a.delimiter, "tab");
        assert!(!a.header);
        assert_eq!(a.null, "NA");
        assert_eq!(a.columns, "name,id");
        assert_eq!(a.limit, 5);
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"delimiter":","}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }
}
