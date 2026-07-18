//! gizza-ai/spreadsheet-to-sql — read a spreadsheet (.xlsx/.xlsm/.xls/.ods) and
//! emit `CREATE TABLE` + `INSERT` SQL statements, one table per worksheet.
//!
//! No-page block (chat + CLI surface only, like `blocks/xlsx-to-csv`): it
//! ingests binary spreadsheet bytes, which is neither a pure-text page input nor
//! an ffmpeg media transform, so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI).
//!
//! Pipeline: parse `{url|ref}` + options → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup, validated to
//! the `application/*` `AssetKind::Document` class) → `core::to_sql(bytes, opts)`
//! → emit a text `Envelope`. The LLM sees the SQL (head-truncated if large); the
//! UI gets a downloadable `data:application/sql` URL + `*.sql` filename.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl. `descriptor()` / `schema_json()`
// and the pure SQL summary remain native-compilable so the drift-guard + unit
// tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// pure SQL conversion, descriptor, and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_spreadsheet_to_sql_core::{to_sql, Dialect, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the spreadsheet input we accept (matches the sibling xlsx tools' 4 MiB
/// guard; calamine holds the whole workbook in memory).
const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Cap on the SQL text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full SQL is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB of SQL text

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_dialect")]
    dialect: String,
    /// Worksheet selector: a sheet name, or a 0-based index as a string.
    /// Omitted / empty → every sheet (one table each).
    #[serde(default)]
    sheet: Option<String>,
    /// Base table-name override (else derived from each sheet name).
    #[serde(default)]
    table: Option<String>,
    #[serde(default = "default_true")]
    create_table: bool,
    #[serde(default = "default_true")]
    header_row: bool,
    #[serde(default = "default_true")]
    infer_types: bool,
    #[serde(default = "default_true")]
    batch_insert: bool,
}

fn default_dialect() -> String {
    "mysql".to_string()
}
fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). The drift-guard test
/// below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("dialect", ["mysql", "postgres", "sqlite", "mssql"])
                .default("mysql")
                .describe(
                    "Target SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite=\"quotes\", mssql=[brackets]), value escaping, and inferred column types. Default mysql.",
                ),
        )
        .param(
            Param::string("sheet").describe(
                "Worksheet to convert: a sheet name, or a 0-based index as a string (e.g. \"0\"). Omit for every sheet (one CREATE TABLE + INSERT block per sheet).",
            ),
        )
        .param(
            Param::string("table").describe(
                "Base table name to use instead of the sheet name. With multiple sheets it becomes a prefix (`{table}_{sheet}`). Names are sanitized to safe identifiers.",
            ),
        )
        .param(
            Param::boolean("create_table")
                .default(true)
                .describe("Emit a CREATE TABLE statement before the inserts. Set false for INSERT statements only. Default true."),
        )
        .param(
            Param::boolean("header_row")
                .default(true)
                .describe("Treat each sheet's first row as column names. When false, columns are named col1..colN and the first row is inserted as data. Default true."),
        )
        .param(
            Param::boolean("infer_types")
                .default(true)
                .describe("Infer column SQL types (integer/float/boolean/text) from the data. When false, every column is a text type. Default true."),
        )
        .param(
            Param::boolean("batch_insert")
                .default(true)
                .describe("Emit one multi-row INSERT ... VALUES (...),(...) per sheet. When false, emit a separate INSERT per row. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Build `Options` from parsed args (dialect string → enum).
fn options_from(args: &Args) -> Result<Options, String> {
    Ok(Options {
        dialect: Dialect::parse(&args.dialect)?,
        sheet: args.sheet.clone().filter(|s| !s.trim().is_empty()),
        table: args.table.clone().filter(|s| !s.trim().is_empty()),
        create_table: args.create_table,
        header_row: args.header_row,
        infer_types: args.infer_types,
        batch_insert: args.batch_insert,
    })
}

/// Build the `_for_llm` text: the full SQL when small, else a head plus a note.
fn summarize_for_llm(sql: &str, label: &str) -> String {
    if sql.chars().count() <= MAX_LLM_CHARS {
        format!("SQL for {label}:\n{sql}")
    } else {
        let head: String = sql.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "SQL for {label} (first {MAX_LLM_CHARS} of {} chars; full script in the download):\n{head}",
            sql.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct SpreadsheetToSql;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spreadsheet-to-sql",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a spreadsheet (.xlsx/.xls/.ods) to SQL CREATE TABLE + INSERT statements",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read a spreadsheet (.xlsx, .xlsm, .xls, or .ods) and emit CREATE TABLE plus INSERT statements — one table per worksheet. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Pick the SQL `dialect` (mysql, postgres, sqlite, mssql). Optionally choose one `sheet` (name or 0-based index; omit for all sheets) and a `table` name. Toggle `create_table` (schema + inserts vs inserts only), `header_row` (first row as column names), `infer_types` (integer/float/boolean/text vs all-text), and `batch_insert` (one multi-row INSERT vs one per row). Empty cells become NULL.",
        parameters = schema_json()
    ),
)]
impl SpreadsheetToSql {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("spreadsheet-to-sql")?;

    let opts = options_from(&args).map_err(SkillError::InvalidArgs)?;

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let sql = to_sql(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let label = match opts.sheet.as_deref() {
        Some(s) => format!("sheet {s:?} of {filename}"),
        None => format!("all sheets of {filename}"),
    };
    let for_llm = summarize_for_llm(&sql, &label);

    let sql_filename = replace_extension(&filename, "sql");
    let data_url = format!("data:application/sql;base64,{}", B64.encode(sql.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/sql".to_string(),
            filename: sql_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional. The
    /// `url`/`ref` property descriptions are centralized in `to_schema_json`
    /// (shared by every Document/media tool), so the expected JSON uses that
    /// shared wording.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":   { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":   { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "dialect": { "type": "string", "enum": ["mysql", "postgres", "sqlite", "mssql"], "default": "mysql", "description": "Target SQL dialect. Sets identifier quoting (mysql=`backticks`, postgres/sqlite=\"quotes\", mssql=[brackets]), value escaping, and inferred column types. Default mysql." },
                    "sheet": { "type": "string", "description": "Worksheet to convert: a sheet name, or a 0-based index as a string (e.g. \"0\"). Omit for every sheet (one CREATE TABLE + INSERT block per sheet)." },
                    "table": { "type": "string", "description": "Base table name to use instead of the sheet name. With multiple sheets it becomes a prefix (`{table}_{sheet}`). Names are sanitized to safe identifiers." },
                    "create_table": { "type": "boolean", "default": true, "description": "Emit a CREATE TABLE statement before the inserts. Set false for INSERT statements only. Default true." },
                    "header_row": { "type": "boolean", "default": true, "description": "Treat each sheet's first row as column names. When false, columns are named col1..colN and the first row is inserted as data. Default true." },
                    "infer_types": { "type": "boolean", "default": true, "description": "Infer column SQL types (integer/float/boolean/text) from the data. When false, every column is a text type. Default true." },
                    "batch_insert": { "type": "boolean", "default": true, "description": "Emit one multi-row INSERT ... VALUES (...),(...) per sheet. When false, emit a separate INSERT per row. Default true." }
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
    fn summarize_short_sql_includes_full_text() {
        let sql = "CREATE TABLE `t` (\n  `a` INT\n);\n";
        let s = summarize_for_llm(sql, "all sheets of book.xlsx");
        assert!(s.contains("SQL for all sheets of book.xlsx"));
        assert!(s.contains("CREATE TABLE"));
    }

    #[test]
    fn summarize_long_sql_truncates_with_note() {
        let sql = "x".repeat(MAX_LLM_CHARS + 100);
        let s = summarize_for_llm(&sql, "sheet \"0\" of big.xlsx");
        assert!(s.contains("full script in the download"));
        assert!(s.len() < sql.len() + 200);
    }

    #[test]
    fn options_defaults_from_minimal_args() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.xlsx"}"#).unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.dialect, Dialect::MySql);
        assert!(o.sheet.is_none());
        assert!(o.create_table && o.header_row && o.infer_types && o.batch_insert);
    }

    #[test]
    fn options_parse_all_params() {
        let a: Args = serde_json::from_str(
            r#"{"ref":"call_1","dialect":"postgres","sheet":"Sales","table":"t","create_table":false,"header_row":false,"infer_types":false,"batch_insert":false}"#,
        )
        .unwrap();
        let o = options_from(&a).unwrap();
        assert_eq!(o.dialect, Dialect::Postgres);
        assert_eq!(o.sheet.as_deref(), Some("Sales"));
        assert_eq!(o.table.as_deref(), Some("t"));
        assert!(!o.create_table && !o.header_row && !o.infer_types && !o.batch_insert);
    }

    #[test]
    fn options_reject_unknown_dialect() {
        let a: Args = serde_json::from_str(r#"{"url":"u","dialect":"oracle"}"#).unwrap();
        assert!(options_from(&a).is_err());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"sheet":"0"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }
}
