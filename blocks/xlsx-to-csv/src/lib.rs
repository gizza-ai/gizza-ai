//! gizza-ai/xlsx-to-csv — convert a spreadsheet (.xlsx/.xlsm/.xls/.ods) to CSV.
//!
//! No-page block (chat + CLI surface only, like `blocks/web-fetch`): it ingests
//! binary spreadsheet bytes, which is neither a pure-text page input nor an
//! ffmpeg media transform, so there is no standalone page.
//!
//! Pipeline: parse `{url|ref}` + optional `sheet` → load bytes via `block-utils`
//! (`fetch_from_url` for a URL, `load_from_attachment` for an upload ref) →
//! `core::to_csv(bytes, sheet)` (calamine, RFC-4180 CSV) → emit a text `Envelope`.
//! The LLM sees the CSV (head-truncated if large); the UI gets a downloadable
//! `data:text/csv` URL + `*.csv` filename.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// Byte-loading helpers are wasm-only (they call the wasm-gated network/attachment
// host imports); the pure CSV conversion + arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
use gizza_ai_xlsx_to_csv_core::to_csv;
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the spreadsheet input we accept (matches the image/video tools' 4 MiB
/// guard; calamine holds the whole workbook in memory).
const MAX_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Cap on the CSV text fed back to the LLM (`_for_llm`). Larger results are
/// head-truncated with a note; the full CSV is always available via `_for_ui`.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB of CSV text

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Worksheet selector: a sheet name, or a 0-based index as a string.
    /// Omitted / empty → the first sheet.
    #[serde(default)]
    sheet: Option<String>,
}

/// Build the `_for_llm` text: the full CSV when small, else a head plus a note.
fn summarize_for_llm(csv: &str, sheet_label: &str) -> String {
    if csv.chars().count() <= MAX_LLM_CHARS {
        format!("CSV for {sheet_label}:\n{csv}")
    } else {
        let head: String = csv.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "CSV for {sheet_label} (first {MAX_LLM_CHARS} of {} chars; full file in the download):\n{head}",
            csv.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct XlsxToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xlsx-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a spreadsheet (.xlsx/.xls/.ods) to CSV",
    requires = ["wafer-run/network"],
    skill(
        description = "Convert a spreadsheet (.xlsx, .xlsm, .xls, or .ods) to CSV. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Optionally pick a sheet by name or 0-based index; defaults to the first sheet.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":   { "type": "string", "description": "Public HTTP/HTTPS URL of the spreadsheet to convert." },
                "ref":   { "type": "string", "description": "Attachment id of an uploaded spreadsheet (alternative to url)." },
                "sheet": { "type": "string", "description": "Worksheet to convert: a sheet name, or a 0-based index as a string (e.g. \"0\"). Defaults to the first sheet." }
            },
            "oneOf": [ { "required": ["url"] }, { "required": ["ref"] } ],
            "additionalProperties": false
        }"#
    ),
)]
impl XlsxToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("xlsx-to-csv")?;

    let (bytes, _mime, filename) = match args.source.into_inner() {
        Source::Url(url) => fetch_from_url(&url, AssetKind::Document, MAX_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Document, MAX_BYTES)?,
    };

    let sheet = args.sheet.as_deref().filter(|s| !s.trim().is_empty());
    let csv = to_csv(&bytes, sheet).map_err(SkillError::InvalidArgs)?;

    let sheet_label = match sheet {
        Some(s) => format!("sheet {s:?} of {filename}"),
        None => format!("the first sheet of {filename}"),
    };
    let for_llm = summarize_for_llm(&csv, &sheet_label);

    let csv_filename = replace_extension(&filename, "csv");
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

    #[test]
    fn summarize_short_csv_includes_full_text() {
        let csv = "a,b\r\n1,2\r\n";
        let s = summarize_for_llm(csv, "the first sheet of book.xlsx");
        assert!(s.contains("CSV for the first sheet of book.xlsx"));
        assert!(s.contains("a,b"));
        assert!(s.contains("1,2"));
    }

    #[test]
    fn summarize_long_csv_truncates_with_note() {
        let csv = "x".repeat(MAX_LLM_CHARS + 100);
        let s = summarize_for_llm(&csv, "sheet \"0\" of big.xlsx");
        assert!(s.contains("full file in the download"));
        // Head is included but not the whole thing.
        assert!(s.len() < csv.len() + 200);
    }

    #[test]
    fn args_deserialize_url_and_sheet() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://x/y.xlsx","sheet":"Sales"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(ref u) if u == "https://x/y.xlsx"));
        assert_eq!(a.sheet.as_deref(), Some("Sales"));
    }

    #[test]
    fn args_deserialize_ref_without_sheet() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_1"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(ref r) if r == "call_1"));
        assert!(a.sheet.is_none());
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
