//! gizza-ai/csv-to-pdf-table — render CSV (or TSV / semicolon- / pipe-delimited)
//! table data as a clean, paginated PDF table and return it as a download.
//!
//! Pure-Rust (`csv` + `lopdf`, built-in base-14 Helvetica), so it runs on ALL
//! backends including the chat Service Worker. The chat schema is single-sourced
//! from `descriptor()` (which also drives the CLI); `handle()` builds an
//! `application/pdf` base64 download envelope like text-to-pdf / csv-to-xlsx
//! (pure compute, binary output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_to_pdf_table_core::render_csv_pdf;
use serde::Deserialize;
use wafer_sdk::*;

/// Cap the produced PDF so a runaway table can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    data: String,
    delimiter: String,
    header: bool,
    title: String,
    page_size: String,
    orientation: String,
    font_size: f64,
    row_banding: bool,
    grid: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            data: String::new(),
            delimiter: "comma".to_string(),
            header: true,
            title: String::new(),
            page_size: "letter".to_string(),
            orientation: "portrait".to_string(),
            font_size: 10.0,
            row_banding: true,
            grid: true,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The table as delimited text: CSV, TSV, semicolon- or pipe-delimited. Example: `name,age\\nAlice,30`."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"])
                .default("comma")
                .describe("Field delimiter: comma, tab, semicolon, or pipe. Default comma."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header (default true): it is drawn bold and repeated at the top of every page. Off treats every row as data."),
        )
        .param(
            Param::string("title")
                .describe("Optional heading drawn in bold above the table on the first page. Leave blank for no title."),
        )
        .param(
            Param::enumv("page_size", ["letter", "a4", "legal"])
                .default("letter")
                .describe("Page size: letter (US, default), a4 or legal."),
        )
        .param(
            Param::enumv("orientation", ["portrait", "landscape"])
                .default("portrait")
                .describe("Page orientation: portrait (default) or landscape (wider, good for many columns)."),
        )
        .param(
            Param::number("font_size")
                .default(10)
                .min(5.0)
                .max(24.0)
                .describe("Table font size in points (5–24, default 10)."),
        )
        .param(
            Param::boolean("row_banding")
                .default(true)
                .describe("Shade alternate data rows a light gray for readability (default true)."),
        )
        .param(
            Param::boolean("grid")
                .default(true)
                .describe("Draw cell grid lines around every cell (default true)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvToPdfTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-pdf-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render CSV table data as a formatted, paginated PDF table",
    skill(
        description = "Render CSV (or TSV / semicolon- / pipe-delimited) table data as a clean, paginated PDF table and return it as a download. The first row can be a bold header repeated on every page; columns are auto-sized to fit the page, numeric columns are right-aligned, and long cells are truncated with an ellipsis. Configure the delimiter, page size (letter/a4/legal), orientation, font size (5–24), an optional title, zebra row banding and cell grid lines. Uses the built-in Helvetica font (Latin-1). Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl CsvToPdfTable {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid csv-to-pdf-table args: {e}")))?;
    let pdf = render_csv_pdf(
        &args.data,
        &args.delimiter,
        args.header,
        &args.title,
        &args.page_size,
        &args.orientation,
        args.font_size,
        args.row_banding,
        args.grid,
    )
    .map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &pdf,
        "application/pdf",
        "table.pdf".to_string(),
        format!("rendered the CSV as a {}-byte PDF table (table.pdf)", pdf.len()),
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page control the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data":        { "type": "string", "description": "The table as delimited text: CSV, TSV, semicolon- or pipe-delimited. Example: `name,age\\nAlice,30`." },
                    "delimiter":   { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field delimiter: comma, tab, semicolon, or pipe. Default comma." },
                    "header":      { "type": "boolean", "default": true, "description": "Treat the first row as a header (default true): it is drawn bold and repeated at the top of every page. Off treats every row as data." },
                    "title":       { "type": "string", "description": "Optional heading drawn in bold above the table on the first page. Leave blank for no title." },
                    "page_size":   { "type": "string", "enum": ["letter", "a4", "legal"], "default": "letter", "description": "Page size: letter (US, default), a4 or legal." },
                    "orientation": { "type": "string", "enum": ["portrait", "landscape"], "default": "portrait", "description": "Page orientation: portrait (default) or landscape (wider, good for many columns)." },
                    "font_size":   { "type": "number", "minimum": 5, "maximum": 24, "default": 10, "description": "Table font size in points (5–24, default 10)." },
                    "row_banding": { "type": "boolean", "default": true, "description": "Shade alternate data rows a light gray for readability (default true)." },
                    "grid":        { "type": "boolean", "default": true, "description": "Draw cell grid lines around every cell (default true)." }
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
