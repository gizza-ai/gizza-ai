//! gizza-ai/sqlite-table-to-csv — export one table of an uploaded SQLite
//! database (`.db`/`.sqlite`) as CSV.
//!
//! No-page block (chat + CLI surface only, like `blocks/xlsx-to-csv`): it
//! ingests a binary SQLite file, which is neither a pure-text page input nor an
//! ffmpeg media transform, so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
//!
//! Pipeline: parse `{url|ref}` + CSV options → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup) →
//! `core::to_csv(bytes, &Options)` (parses the on-disk SQLite page format, no
//! SQL engine) → emit a text `Envelope`. The LLM sees the CSV (head-truncated if
//! large) plus the table list; the UI gets a downloadable `data:text/csv` URL.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// pure CSV conversion, descriptor, and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_sqlite_table_to_csv_core::{to_csv, Delimiter, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the database input we accept. The parser holds the whole file in
/// memory and walks it lazily, but a hard cap keeps memory bounded.
const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Cap on the CSV text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full CSV is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB of CSV text

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Table to export. Omitted → the only table (error if the DB has several).
    #[serde(default)]
    table: Option<String>,
    /// Field separator name: comma | tab | semicolon | pipe.
    #[serde(default)]
    delimiter: Option<String>,
    /// Emit a header row of column names (default true).
    #[serde(default)]
    header: Option<bool>,
    /// Text substituted for SQL NULL cells (default empty).
    #[serde(default)]
    null_value: Option<String>,
    /// Prepend a UTF-8 BOM (default false).
    #[serde(default)]
    bom: Option<bool>,
    /// Max rows to export; 0 = all (default 0).
    #[serde(default)]
    limit: Option<i64>,
}

/// Single-source param descriptor → chat schema (and CLI). The drift-guard test
/// below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("table").describe(
            "Name of the table to export (case-insensitive). Omit it if the database has a single table; otherwise the tool lists the available tables.",
        ))
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Field separator for the output. One of: comma (CSV), tab (TSV), semicolon, or pipe. Default comma."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Include a first row of column names. Default true."),
        )
        .param(
            Param::string("null_value")
                .default("")
                .describe("Text to write for SQL NULL cells. Default is an empty field."),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a UTF-8 byte-order mark so Excel opens the CSV as UTF-8. Default false."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Maximum number of data rows to export; 0 means all rows. Default 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Build the `Options` for the core from the parsed args.
fn options_from_args(args: &Args) -> Result<Options, SkillError> {
    let delimiter = match args.delimiter.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => Delimiter::parse(d).map_err(SkillError::InvalidArgs)?,
        None => Delimiter::Comma,
    };
    let limit = match args.limit {
        Some(n) if n < 0 => return Err(SkillError::InvalidArgs("`limit` must be >= 0".into())),
        Some(n) => n as usize,
        None => 0,
    };
    Ok(Options {
        table: args.table.clone(),
        delimiter,
        header: args.header.unwrap_or(true),
        null_value: args.null_value.clone().unwrap_or_default(),
        bom: args.bom.unwrap_or(false),
        limit,
    })
}

/// Build the `_for_llm` summary text.
fn summarize_for_llm(out: &gizza_ai_sqlite_table_to_csv_core::CsvOutput) -> String {
    let mut head = format!(
        "Exported table {:?} ({} column(s), {} row(s){}). Tables in this database: {}.\n\n",
        out.table,
        out.columns.len(),
        out.row_count,
        if out.truncated { ", truncated by limit" } else { "" },
        out.available_tables.join(", "),
    );
    if out.csv.chars().count() <= MAX_LLM_CHARS {
        head.push_str(&out.csv);
    } else {
        let body: String = out.csv.chars().take(MAX_LLM_CHARS).collect();
        head.push_str(&format!(
            "(first {MAX_LLM_CHARS} of {} chars; full CSV in the download)\n{body}",
            out.csv.chars().count()
        ));
    }
    head
}

/// Sanitize a table name into a filesystem-safe CSV filename stem.
fn csv_filename(table: &str) -> String {
    let stem: String = table
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let stem = stem.trim_matches('_');
    if stem.is_empty() {
        "table.csv".to_string()
    } else {
        format!("{stem}.csv")
    }
}

#[cfg(target_arch = "wasm32")]
struct SqliteTableToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sqlite-table-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export a table from an SQLite database file as CSV",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Export one table of an SQLite database (.db/.sqlite) as CSV by parsing the on-disk file format (no SQL engine). Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Pick a table by name (omit it if the database has only one table); choose the delimiter (comma/tab/semicolon/pipe), toggle the header row, set the text for NULL cells, optionally add a UTF-8 BOM, and cap the number of rows.",
        parameters = schema_json()
    ),
)]
impl SqliteTableToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("sqlite-table-to-csv")?;
    let opts = options_from_args(&args)?;

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let out = to_csv(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let for_llm = summarize_for_llm(&out);
    let filename = csv_filename(&out.table);
    let data_url = format!("data:text/csv;base64,{}", B64.encode(out.csv.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "text/csv".to_string(),
            filename,
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
                    "table": { "type": "string", "description": "Name of the table to export (case-insensitive). Omit it if the database has a single table; otherwise the tool lists the available tables." },
                    "delimiter": {
                        "type": "string",
                        "enum": ["comma", "tab", "semicolon", "pipe"],
                        "default": "comma",
                        "description": "Field separator for the output. One of: comma (CSV), tab (TSV), semicolon, or pipe. Default comma."
                    },
                    "header": { "type": "boolean", "default": true, "description": "Include a first row of column names. Default true." },
                    "null_value": { "type": "string", "default": "", "description": "Text to write for SQL NULL cells. Default is an empty field." },
                    "bom": { "type": "boolean", "default": false, "description": "Prepend a UTF-8 byte-order mark so Excel opens the CSV as UTF-8. Default false." },
                    "limit": { "type": "integer", "default": 0, "minimum": 0, "description": "Maximum number of data rows to export; 0 means all rows. Default 0." }
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
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.db"}"#).unwrap();
        let o = options_from_args(&a).unwrap();
        assert_eq!(o.delimiter, Delimiter::Comma);
        assert!(o.header);
        assert_eq!(o.null_value, "");
        assert!(!o.bom);
        assert_eq!(o.limit, 0);
        assert!(o.table.is_none());
    }

    #[test]
    fn options_parse_all_fields() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","table":"users","delimiter":"tab","header":false,"null_value":"NULL","bom":true,"limit":10}"#,
        )
        .unwrap();
        let o = options_from_args(&a).unwrap();
        assert_eq!(o.table.as_deref(), Some("users"));
        assert_eq!(o.delimiter, Delimiter::Tab);
        assert!(!o.header);
        assert_eq!(o.null_value, "NULL");
        assert!(o.bom);
        assert_eq!(o.limit, 10);
    }

    #[test]
    fn options_reject_bad_delimiter() {
        let a: Args = serde_json::from_str(r#"{"url":"u","delimiter":"colon"}"#).unwrap();
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
        let err = serde_json::from_str::<Args>(r#"{"table":"x"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn csv_filename_sanitizes() {
        assert_eq!(csv_filename("users"), "users.csv");
        assert_eq!(csv_filename("my table!"), "my_table.csv");
        assert_eq!(csv_filename("***"), "table.csv");
    }
}
