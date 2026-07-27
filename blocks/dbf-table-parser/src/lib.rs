//! gizza-ai/dbf-table-parser — parse a dBase / `.dbf` table file into its column
//! definitions and rows, exportable as CSV or JSON.
//!
//! No-page block (chat + CLI surface only, like `blocks/xlsx-to-csv` /
//! `blocks/arrow-feather-to-csv`): it ingests binary `.dbf` bytes, which is
//! neither a pure-text page input nor an ffmpeg media transform, so there is no
//! standalone page. The chat schema is derived from `descriptor()` (single source
//! shared across chat + CLI).
//!
//! Pipeline: parse `{url|ref}` + options → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup; any bytes, since
//! `.dbf` files are commonly served as `application/octet-stream`) →
//! `core::parse_dbf` → emit a text `Envelope`. The LLM sees the CSV/JSON
//! (head-truncated if large); the UI gets a downloadable `data:` URL + filename.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_dbf_table_parser_core::{parse_dbf, Encoding, Format, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the `.dbf` input we accept. The whole table is held in memory and the
/// rendered CSV/JSON can be several times larger, so stay inside the wasm sandbox.
const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Cap on the text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full output is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB

fn default_format() -> String {
    "csv".to_string()
}
fn default_delimiter() -> String {
    ",".to_string()
}
fn default_header() -> bool {
    true
}
fn default_trim() -> bool {
    true
}
fn default_encoding() -> String {
    "auto".to_string()
}

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Output format: `csv` or `json`.
    #[serde(default = "default_format")]
    format: String,
    /// CSV field separator. A single character, or `"tab"`.
    #[serde(default = "default_delimiter")]
    delimiter: String,
    /// Write a leading row of column names (CSV only).
    #[serde(default = "default_header")]
    header: bool,
    /// Comma-separated columns to keep/reorder, by name or 0-based index. Empty = all.
    #[serde(default)]
    columns: String,
    /// Include records flagged as deleted.
    #[serde(default)]
    include_deleted: bool,
    /// Trim trailing padding from character fields.
    #[serde(default = "default_trim")]
    trim: bool,
    /// Character encoding: `auto`, `utf-8`, `latin1`, or `cp1252`.
    #[serde(default = "default_encoding")]
    encoding: String,
    /// Max data rows to emit; 0 = all.
    #[serde(default)]
    limit: u64,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::File` emits
/// the `url`⊕`ref` `oneOf`; the options tune the parse + output.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("format", ["csv", "json"])
                .default("csv")
                .describe(
                    "Output format. \"csv\" writes the table as delimited text (header row + rows). \"json\" writes {columns:[{name,type,length,decimal}], row_count, rows:[{col:value}]} with numeric/logical/date cells typed. Defaults to csv.",
                ),
        )
        .param(
            Param::string("delimiter").default(",").describe(
                "CSV field separator: a single character, or the word \"tab\" for a tab. Ignored for JSON. Defaults to a comma.",
            ),
        )
        .param(
            Param::boolean("header").default(true).describe(
                "Write a first CSV row of column names. Set false to omit it. Ignored for JSON. Defaults to true.",
            ),
        )
        .param(
            Param::string("columns").default("").describe(
                "Comma-separated columns to keep and reorder, by name (case-insensitive) or 0-based index, e.g. \"NAME,2,PRICE\". Leave empty to keep every column in file order.",
            ),
        )
        .param(
            Param::boolean("include_deleted").default(false).describe(
                "Include records flagged as deleted in the .dbf (marked with a leading '*'). Defaults to false (skips them, like most viewers).",
            ),
        )
        .param(
            Param::boolean("trim").default(true).describe(
                "Trim trailing padding spaces from fixed-width character fields. Set false to keep the original padding. Defaults to true.",
            ),
        )
        .param(
            Param::enumv("encoding", ["auto", "utf-8", "latin1", "cp1252"])
                .default("auto")
                .describe(
                    "Text decoding for character fields. \"auto\" uses UTF-8 when valid else Latin-1; \"latin1\" (ISO-8859-1) and \"cp1252\" (Windows-1252) cover most legacy DBFs. Defaults to auto.",
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

/// Resolve the descriptor string args into a `core::Options`.
fn build_options(args: &Args) -> Result<Options, SkillError> {
    let format = match args.format.trim().to_ascii_lowercase().as_str() {
        "csv" => Format::Csv,
        "json" => Format::Json,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "format must be \"csv\" or \"json\", got {other:?}"
            )))
        }
    };
    let delimiter = if args.delimiter.eq_ignore_ascii_case("tab") {
        '\t'
    } else {
        args.delimiter.chars().next().unwrap_or(',')
    };
    let encoding = match args.encoding.trim().to_ascii_lowercase().as_str() {
        "auto" => Encoding::Auto,
        "utf-8" | "utf8" => Encoding::Utf8,
        "latin1" | "latin-1" | "iso-8859-1" => Encoding::Latin1,
        "cp1252" | "windows-1252" => Encoding::Cp1252,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "encoding must be auto, utf-8, latin1, or cp1252, got {other:?}"
            )))
        }
    };
    Ok(Options {
        format,
        delimiter,
        header: args.header,
        columns: args.columns.clone(),
        include_deleted: args.include_deleted,
        trim: args.trim,
        encoding,
        limit: args.limit as usize,
    })
}

/// Build the `_for_llm` text: the full output when small, else a head plus a note.
fn summarize_for_llm(text: &str, filename: &str) -> String {
    if text.chars().count() <= MAX_LLM_CHARS {
        format!("Parsed {filename}:\n{text}")
    } else {
        let head: String = text.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "Parsed {filename} (first {MAX_LLM_CHARS} of {} chars; full file in the download):\n{head}",
            text.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct DbfTableParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dbf-table-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a dBase/.dbf table file into columns and rows (CSV or JSON)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse a dBase / .dbf table file into its column definitions and rows, exportable as CSV or JSON. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Reads dBase III/IV and FoxPro DBFs: field types C (character), N/F (numeric), D (date → YYYY-MM-DD), L (logical → true/false), and I (4-byte integer); memo (M) cells emit empty because the sidecar .dbt/.fpt file isn't available to a single-file tool. Options: `format` (csv default, or json with typed cells + column defs), `delimiter` (csv; single char or \"tab\"), `header` (csv), `columns` (keep/reorder a subset by name or 0-based index), `include_deleted` (default false), `trim` (default true), `encoding` (auto/utf-8/latin1/cp1252), and `limit` (row cap for previews).",
        parameters = schema_json()
    ),
)]
impl DbfTableParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("dbf-table-parser")?;
    let opts = build_options(&args)?;

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let name = if filename.is_empty() {
        "table.dbf".to_string()
    } else {
        filename.clone()
    };

    let output = parse_dbf(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let for_llm = summarize_for_llm(&output, &name);
    let (ext, mime) = match opts.format {
        Format::Csv => ("csv", "text/csv"),
        Format::Json => ("json", "application/json"),
    };
    let out_filename = replace_extension(&name, ext);
    let data_url = format!("data:{mime};base64,{}", B64.encode(output.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename: out_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored one, so the LLM sees no drift. `url`/`ref` wording is centralized
    /// in `to_schema_json` (shared by every File/media tool).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":             { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":             { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format":          { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format. \"csv\" writes the table as delimited text (header row + rows). \"json\" writes {columns:[{name,type,length,decimal}], row_count, rows:[{col:value}]} with numeric/logical/date cells typed. Defaults to csv." },
                    "delimiter":       { "type": "string", "default": ",", "description": "CSV field separator: a single character, or the word \"tab\" for a tab. Ignored for JSON. Defaults to a comma." },
                    "header":          { "type": "boolean", "default": true, "description": "Write a first CSV row of column names. Set false to omit it. Ignored for JSON. Defaults to true." },
                    "columns":         { "type": "string", "default": "", "description": "Comma-separated columns to keep and reorder, by name (case-insensitive) or 0-based index, e.g. \"NAME,2,PRICE\". Leave empty to keep every column in file order." },
                    "include_deleted": { "type": "boolean", "default": false, "description": "Include records flagged as deleted in the .dbf (marked with a leading '*'). Defaults to false (skips them, like most viewers)." },
                    "trim":            { "type": "boolean", "default": true, "description": "Trim trailing padding spaces from fixed-width character fields. Set false to keep the original padding. Defaults to true." },
                    "encoding":        { "type": "string", "enum": ["auto", "utf-8", "latin1", "cp1252"], "default": "auto", "description": "Text decoding for character fields. \"auto\" uses UTF-8 when valid else Latin-1; \"latin1\" (ISO-8859-1) and \"cp1252\" (Windows-1252) cover most legacy DBFs. Defaults to auto." },
                    "limit":           { "type": "integer", "default": 0, "description": "Maximum number of data rows to output; 0 (the default) writes every row. Use a small value to preview a large table." }
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
    fn args_defaults_apply() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.dbf"}"#).unwrap();
        assert_eq!(a.format, "csv");
        assert_eq!(a.delimiter, ",");
        assert!(a.header);
        assert_eq!(a.columns, "");
        assert!(!a.include_deleted);
        assert!(a.trim);
        assert_eq!(a.encoding, "auto");
        assert_eq!(a.limit, 0);
    }

    #[test]
    fn args_parse_overrides() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","format":"json","delimiter":"tab","header":false,"columns":"NAME,0","include_deleted":true,"trim":false,"encoding":"cp1252","limit":5}"#,
        )
        .unwrap();
        assert_eq!(a.format, "json");
        assert_eq!(a.delimiter, "tab");
        assert!(!a.header);
        assert_eq!(a.columns, "NAME,0");
        assert!(a.include_deleted);
        assert!(!a.trim);
        assert_eq!(a.encoding, "cp1252");
        assert_eq!(a.limit, 5);
    }

    #[test]
    fn build_options_maps_tab_and_json() {
        let a: Args =
            serde_json::from_str(r#"{"url":"u","format":"json","delimiter":"tab"}"#).unwrap();
        let o = build_options(&a).unwrap();
        assert_eq!(o.format, Format::Json);
        assert_eq!(o.delimiter, '\t');
        assert_eq!(o.encoding, Encoding::Auto);
    }

    #[test]
    fn build_options_rejects_bad_format() {
        let a: Args = serde_json::from_str(r#"{"url":"u","format":"xml"}"#).unwrap();
        let err = build_options(&a).unwrap_err();
        assert!(err.to_string().contains("format must be"), "got: {err}");
    }

    #[test]
    fn build_options_rejects_bad_encoding() {
        let a: Args = serde_json::from_str(r#"{"url":"u","encoding":"ebcdic"}"#).unwrap();
        let err = build_options(&a).unwrap_err();
        assert!(err.to_string().contains("encoding must be"), "got: {err}");
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"format":"csv"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn summarize_short_includes_full_text() {
        let s = summarize_for_llm("NAME,AGE\r\nAlice,30\r\n", "people.dbf");
        assert!(s.contains("Parsed people.dbf"));
        assert!(s.contains("Alice,30"));
    }

    #[test]
    fn summarize_long_truncates_with_note() {
        let big = "x".repeat(MAX_LLM_CHARS + 100);
        let s = summarize_for_llm(&big, "big.dbf");
        assert!(s.contains("full file in the download"));
        assert!(s.len() < big.len() + 200);
    }
}
