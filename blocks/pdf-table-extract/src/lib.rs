//! gizza-ai/pdf-table-extract — detect tables in a text-based PDF and export
//! them as CSV/TSV or JSON, preserving rows and columns.
//!
//! No-page block (chat + CLI surface only, like `blocks/pdf-extract-text` and
//! `blocks/xlsx-to-csv`): a PDF is a binary file input and the output is
//! delimited text / JSON, which fits neither the pure-text page shape nor the
//! ffmpeg file→media page shape — so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
//!
//! Pipeline: parse `{url|ref}` + `page`/`format`/`delimiter`/`header` → resolve
//! bytes via `block_utils::resolve_source` (URL fetch or attachment lookup,
//! validated to the `application/*` `AssetKind::Document` class) →
//! `core::extract_tables` (lopdf content-stream positioning) → serialize with
//! `core::to_csv` / `core::to_json` → emit a text `Envelope`. The LLM sees the
//! detected tables (head-truncated if large); the UI gets a downloadable
//! `data:` URL + `*.csv`/`*.tsv`/`*.json` filename.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl. `descriptor()` / `schema_json()`
// remain native-compilable so the drift-guard + unit tests below can run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// descriptor and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_table_extract_core::{extract_tables, to_csv, to_json, total_rows};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the PDF input we accept (matches pdf-extract-text; lopdf holds the
/// whole document in memory).
const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Cap on the table text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full output is always in the download.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Optional 1-based page number. Omitted = scan every page.
    #[serde(default)]
    page: Option<usize>,
    /// Output format: `csv` (default) or `json`.
    #[serde(default)]
    format: Option<String>,
    /// CSV field delimiter: `comma` (default), `semicolon`, or `tab`.
    #[serde(default)]
    delimiter: Option<String>,
    /// Treat each table's first row as the header (affects json output).
    #[serde(default)]
    header: Option<bool>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`. The drift-guard test below proves the derived
/// schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::integer("page").min(1.0).describe(
                "1-based page number to extract tables from. Omit to scan every page (each page's table is returned separately).",
            ),
        )
        .param(
            Param::enumv("format", ["csv", "json"]).default("csv").describe(
                "Output format: \"csv\" (delimited rows) or \"json\" (array of {page, rows} objects). Default \"csv\".",
            ),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab"]).default("comma").describe(
                "Field delimiter for csv output: \"comma\" (.csv), \"semicolon\", or \"tab\" (.tsv). Ignored when format is json. Default \"comma\".",
            ),
        )
        .param(
            Param::boolean("header").default(false).describe(
                "Treat each table's first row as the column header. For json, data rows become objects keyed by the header; for csv the header row is kept in place. Default false.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Resolve `format` to `(serialized_text, mime, extension)`; errors on an
/// unknown format/delimiter value.
fn serialize(
    tables: &[gizza_ai_pdf_table_extract_core::Table],
    format: &str,
    delimiter: &str,
    header: bool,
) -> Result<(String, &'static str, &'static str), SkillError> {
    match format {
        "json" => Ok((to_json(tables, header), "application/json", "json")),
        "csv" => {
            let (delim, ext, mime) = match delimiter {
                "comma" => (',', "csv", "text/csv"),
                "semicolon" => (';', "csv", "text/csv"),
                "tab" => ('\t', "tsv", "text/tab-separated-values"),
                other => {
                    return Err(SkillError::InvalidArgs(format!(
                        "unknown delimiter {other:?}; use \"comma\", \"semicolon\", or \"tab\""
                    )))
                }
            };
            Ok((to_csv(tables, delim), mime, ext))
        }
        other => Err(SkillError::InvalidArgs(format!(
            "unknown format {other:?}; use \"csv\" or \"json\""
        ))),
    }
}

/// Head-truncate `text` to `MAX_LLM_CHARS` for the LLM view, with a note.
fn clip_for_llm(text: &str) -> String {
    if text.chars().count() <= MAX_LLM_CHARS {
        text.to_string()
    } else {
        let head: String = text.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "{head}\n… (truncated to {MAX_LLM_CHARS} of {} chars; full output in the download)",
            text.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct PdfTableExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-table-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect tables in a PDF and export them as CSV/JSON",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect tables in a text-based PDF and export them as CSV, TSV, or JSON, preserving rows and columns. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Optionally pick a 1-based `page` (omit to scan every page), choose `format` (csv/json), a csv `delimiter` (comma/semicolon/tab), and whether the first row is a `header`. Reads the embedded selectable-text layer via text coordinates — it does not OCR scanned/image-only PDFs, and works best on pages with clearly separated, left-aligned columns.",
        parameters = schema_json()
    ),
)]
impl PdfTableExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-table-extract")?;
    if args.page == Some(0) {
        return Err(SkillError::InvalidArgs(
            "page must be >= 1 (pages are 1-based)".to_string(),
        ));
    }

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let tables = extract_tables(&bytes, args.page).map_err(SkillError::InvalidArgs)?;

    let format = args.format.as_deref().unwrap_or("csv");
    let delimiter = args.delimiter.as_deref().unwrap_or("comma");
    let header = args.header.unwrap_or(false);
    let (text, mime, ext) = serialize(&tables, format, delimiter, header)?;

    let ntables = tables.len();
    let nrows = total_rows(&tables);
    let summary = if ntables == 0 {
        format!(
            "No tables detected in {filename}. It may be a scanned/image-only PDF (no selectable text) or have no tabular layout."
        )
    } else {
        let scope = match args.page {
            Some(p) => format!("page {p} of "),
            None => String::new(),
        };
        format!("Detected {ntables} table(s), {nrows} row(s) total in {scope}{filename}:")
    };
    let for_llm = if text.trim().is_empty() {
        summary
    } else {
        format!("{summary}\n{}", clip_for_llm(&text))
    };

    let out_filename = replace_extension(&filename, ext);
    let data_url = format!("data:{mime};base64,{}", B64.encode(text.as_bytes()));

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

    /// Migration safety / no LLM-facing chat-schema drift: the descriptor-derived
    /// schema must match this authored one. `Input::Document` supplies the
    /// centralized `url`/`ref` wording + the `oneOf`; the tool params carry their
    /// `.describe()` text, enum variants, and defaults.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "page": { "type": "integer", "minimum": 1, "description": "1-based page number to extract tables from. Omit to scan every page (each page's table is returned separately)." },
                    "format": { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Output format: \"csv\" (delimited rows) or \"json\" (array of {page, rows} objects). Default \"csv\"." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab"], "default": "comma", "description": "Field delimiter for csv output: \"comma\" (.csv), \"semicolon\", or \"tab\" (.tsv). Ignored when format is json. Default \"comma\"." },
                    "header": { "type": "boolean", "default": false, "description": "Treat each table's first row as the column header. For json, data rows become objects keyed by the header; for csv the header row is kept in place. Default false." }
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
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.pdf","page":2,"format":"json","header":true}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(ref u) if u == "https://x/y.pdf"));
        assert_eq!(a.page, Some(2));
        assert_eq!(a.format.as_deref(), Some("json"));
        assert_eq!(a.header, Some(true));
    }

    #[test]
    fn args_parse_ref_defaults() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(r#"{"ref":"call_9"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(ref r) if r == "call_9"));
        assert!(a.page.is_none());
        assert!(a.format.is_none());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn serialize_csv_tsv_json_pick_mime_and_ext() {
        let tables = vec![gizza_ai_pdf_table_extract_core::Table {
            page: 1,
            rows: vec![vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]],
        }];
        let (csv, mime, ext) = serialize(&tables, "csv", "comma", false).unwrap();
        assert_eq!((mime, ext), ("text/csv", "csv"));
        assert_eq!(csv, "a,b\r\n1,2\r\n");

        let (_tsv, mime, ext) = serialize(&tables, "csv", "tab", false).unwrap();
        assert_eq!((mime, ext), ("text/tab-separated-values", "tsv"));

        let (_json, mime, ext) = serialize(&tables, "json", "comma", false).unwrap();
        assert_eq!((mime, ext), ("application/json", "json"));
    }

    #[test]
    fn serialize_rejects_unknown_format() {
        let err = serialize(&[], "xml", "comma", false).unwrap_err();
        assert!(err.to_string().contains("unknown format"));
    }
}
