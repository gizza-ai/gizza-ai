//! gizza-ai/csv-to-xlsx — turn CSV / TSV / JSON table text into a real binary
//! `.xlsx` workbook (typed columns, an optional bold + frozen header, autofit
//! widths). The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` builds a base64 download envelope like create-zip
//! (pure compute, binary output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_xlsx_core::{sanitize_sheet_name, to_xlsx_with_stats, InputFormat};
use serde::Deserialize;
use wafer_sdk::*;

/// OOXML spreadsheet (`.xlsx`) MIME type.
const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
/// Cap the produced workbook so a runaway table can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    data: String,
    input_format: String,
    sheet_name: String,
    header: bool,
    detect_types: bool,
    autofit: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            data: String::new(),
            input_format: "auto".to_string(),
            sheet_name: "Sheet1".to_string(),
            header: true,
            detect_types: true,
            autofit: true,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The table as text: CSV, TSV, semicolon- or pipe-delimited, a JSON array of objects/rows, or NDJSON. Example: `name,age\\nAlice,30`."),
        )
        .param(
            Param::enumv("input_format", ["auto", "csv", "tsv", "semicolon", "pipe", "json", "ndjson"])
                .default("auto")
                .describe("Source format. 'auto' (default) sniffs CSV/TSV/semicolon/pipe/JSON/NDJSON from the text; set it explicitly to override the guess."),
        )
        .param(
            Param::string("sheet_name")
                .default("Sheet1")
                .describe("Worksheet tab name (default 'Sheet1'). Excel limits apply: at most 31 characters, and the characters []:*?/\\ are replaced with '_'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header (default true): its cells become a bold, frozen top row, and for JSON objects the keys are the header. Off writes data with no header row."),
        )
        .param(
            Param::boolean("detect_types")
                .default(true)
                .describe("Detect numbers and booleans in delimited cells and write them as native Excel types (default true); off writes every delimited cell as text. Leading-zero values like 007 always stay text. JSON/NDJSON keep their own types."),
        )
        .param(
            Param::boolean("autofit")
                .default(true)
                .describe("Auto-size each column's width to fit its content (default true)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToXlsx;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-xlsx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV or JSON table data into a real Excel .xlsx workbook",
    skill(
        description = "Convert a table (CSV, TSV, semicolon- or pipe-delimited, a JSON array of objects/rows, or NDJSON) into a real binary Excel .xlsx workbook and return it as a download. Numbers and booleans are written as native Excel cell types (leading-zero values like 007 stay text); the first row can be a bold, frozen header, and columns are auto-sized.",
        parameters = schema_json()
    ),
)]
impl CsvToXlsx {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid csv-to-xlsx args: {e}")))?;
    let fmt = InputFormat::parse(&args.input_format).map_err(SkillError::InvalidArgs)?;
    let (bytes, rows, cols) = to_xlsx_with_stats(
        &args.data,
        fmt,
        &args.sheet_name,
        args.header,
        args.detect_types,
        args.autofit,
    )
    .map_err(SkillError::InvalidArgs)?;

    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output workbook is {} bytes, over the {MAX_OUTPUT_BYTES}-byte cap",
            bytes.len()
        )));
    }

    let filename = format!("{}.xlsx", sanitize_sheet_name(&args.sheet_name));
    let out_len = bytes.len();
    let data_url = format!("data:{XLSX_MIME};base64,{}", B64.encode(&bytes));
    let env = Envelope {
        for_llm: format!(
            "wrote {rows} data row(s) × {cols} column(s) to a {out_len}-byte .xlsx workbook ({filename})"
        ),
        for_ui: ForUi {
            data_url,
            mime: XLSX_MIME.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the LLM-facing
    /// schema (and the page control the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The table as text: CSV, TSV, semicolon- or pipe-delimited, a JSON array of objects/rows, or NDJSON. Example: `name,age\\nAlice,30`." },
                    "input_format": { "type": "string", "enum": ["auto", "csv", "tsv", "semicolon", "pipe", "json", "ndjson"], "default": "auto", "description": "Source format. 'auto' (default) sniffs CSV/TSV/semicolon/pipe/JSON/NDJSON from the text; set it explicitly to override the guess." },
                    "sheet_name": { "type": "string", "default": "Sheet1", "description": "Worksheet tab name (default 'Sheet1'). Excel limits apply: at most 31 characters, and the characters []:*?/\\ are replaced with '_'." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header (default true): its cells become a bold, frozen top row, and for JSON objects the keys are the header. Off writes data with no header row." },
                    "detect_types": { "type": "boolean", "default": true, "description": "Detect numbers and booleans in delimited cells and write them as native Excel types (default true); off writes every delimited cell as text. Leading-zero values like 007 always stay text. JSON/NDJSON keep their own types." },
                    "autofit": { "type": "boolean", "default": true, "description": "Auto-size each column's width to fit its content (default true)." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
