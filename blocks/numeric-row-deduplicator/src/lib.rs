//! gizza-ai/numeric-row-deduplicator — chat skill block on the shared tool
//! abstraction. Removes duplicate numeric rows from a table, comparing each cell
//! by NUMERIC VALUE so different textual forms of the same number collapse. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_numeric_row_deduplicator_core::dedupe_numeric;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default)]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_precision")]
    precision: i64,
    #[serde(default)]
    keep: String,
}
fn default_precision() -> i64 {
    -1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The numeric table text to de-duplicate, one row per line (CSV/TSV). Cells are compared by numeric value, so 1, 1.0, 1.00, +1 and 1e0 all count as the same number."))
        .param(Param::string("columns").default("").describe("Optional comma-separated key columns: 1-based indices and/or header names (e.g. 'id' or '1,3'). Empty = match the whole row. Default empty."))
        .param(Param::boolean("header").default(false).describe("Treat the first row as a header (kept as-is, excluded from the scan, and matchable by name). Default false — numeric tables are usually headerless."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
        .param(Param::integer("precision").default(-1).min(-1.0).max(12.0).describe("Round every numeric cell to this many decimals before comparing, so near-duplicates from float noise collapse (0.30000000000000004 == 0.3 at 2). -1 = compare the exact numeric value. Default -1."))
        .param(Param::enumv("keep", ["first", "last"]).default("first").describe("Which occurrence of each duplicate to keep. 'first' preserves first-occurrence order (default); 'last' keeps the last occurrence, still emitted in original row order."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct NumericRowDeduplicator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/numeric-row-deduplicator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate numeric rows from a table, comparing cells by numeric value",
    skill(
        description = "Remove exact duplicate numeric rows from a table (CSV/TSV), comparing each cell by NUMERIC VALUE so different textual forms of the same number — 1, 1.0, 1.00, +1, 1e0, 100e-2 — all count as duplicates, unlike a plain string deduper. Optionally key on a subset of columns (1-based indices or header names), round each numeric cell to N decimals before comparing to collapse float-noise near-duplicates, and keep the first (default) or last occurrence. Non-numeric cells fall back to a trimmed-string compare so mixed tables still work. The kept rows are emitted in their original order.",
        parameters = schema_json()
    ),
)]
impl NumericRowDeduplicator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "numeric-row-deduplicator", |a: Args| {
            let delimiter = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            let keep = if a.keep.is_empty() { "first".to_string() } else { a.keep };
            dedupe_numeric(&a.data, &a.columns, a.header, &delimiter, a.precision, &keep)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
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
                    "data":      { "type": "string", "description": "The numeric table text to de-duplicate, one row per line (CSV/TSV). Cells are compared by numeric value, so 1, 1.0, 1.00, +1 and 1e0 all count as the same number." },
                    "columns":   { "type": "string", "default": "", "description": "Optional comma-separated key columns: 1-based indices and/or header names (e.g. 'id' or '1,3'). Empty = match the whole row. Default empty." },
                    "header":    { "type": "boolean", "default": false, "description": "Treat the first row as a header (kept as-is, excluded from the scan, and matchable by name). Default false — numeric tables are usually headerless." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "precision": { "type": "integer", "default": -1, "minimum": -1, "maximum": 12, "description": "Round every numeric cell to this many decimals before comparing, so near-duplicates from float noise collapse (0.30000000000000004 == 0.3 at 2). -1 = compare the exact numeric value. Default -1." },
                    "keep":      { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which occurrence of each duplicate to keep. 'first' preserves first-occurrence order (default); 'last' keeps the last occurrence, still emitted in original row order." }
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
