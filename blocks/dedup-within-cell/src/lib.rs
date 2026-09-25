//! gizza-ai/dedup-within-cell — remove duplicate items inside delimited CSV cells.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill and the pure core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_dedup_within_cell_core::dedupe_within_cells;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
#[serde(default)]
struct Args {
    data: String,
    columns: String,
    item_separator: String,
    output_separator: String,
    ignore_case: bool,
    trim_items: bool,
    drop_empty: bool,
    sort_items: String,
    delimiter: String,
    has_header: bool,
    output: String,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data: String::new(),
            columns: String::new(),
            item_separator: "comma".into(),
            output_separator: String::new(),
            ignore_case: false,
            trim_items: true,
            drop_empty: true,
            sort_items: "none".into(),
            delimiter: "comma".into(),
            has_header: true,
            output: "csv".into(),
        }
    }
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("CSV/TSV/table text to clean. Each selected cell is treated as a delimited list, e.g. `a, b, a, c` becomes `a, b, c`. A single-column input works too (one list per line). Maximum 1 MB."))
        .param(Param::string("columns").default("").describe("Comma-separated 1-based column numbers or header names to process, e.g. `2,tags`. Leave blank to process every column. Header names require has_header=true."))
        .param(Param::enumv("item_separator", ["comma", "comma-space", "semicolon", "pipe", "space", "tab", "newline"]).default("comma").describe("Separator between items inside each cell. Use comma for `a,b,a`, comma-space for `a, b, a`, semicolon, pipe, space, tab, or newline."))
        .param(Param::string("output_separator").default("").describe("Separator used to join the kept items. Leave blank to preserve the cell's original separator and spacing (for example comma-space stays comma-space)."))
        .param(Param::boolean("ignore_case").default(false).describe("Treat differently-cased items as duplicates while keeping the first occurrence's original casing."))
        .param(Param::boolean("trim_items").default(true).describe("Trim whitespace around each item before comparing and writing it back. Enabled by default."))
        .param(Param::boolean("drop_empty").default(true).describe("Drop empty items created by repeated separators, e.g. `a,,b,,,a` -> `a,b`. Enabled by default."))
        .param(Param::enumv("sort_items", ["none", "asc", "desc"]).default("none").describe("Order for the surviving items inside each cell: none keeps first-seen order, asc sorts A-Z, desc sorts Z-A."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Delimiter between table fields: comma for CSV (default), tab for TSV, semicolon, or pipe. This is not the same as item_separator inside a cell."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first row as a header. Header rows are never rewritten, and column names in `columns` are resolved from it."))
        .param(Param::enumv("output", ["csv", "stats"]).default("csv").describe("Return the rewritten table as csv (default), or stats for a plain-text count of rows, cells scanned, cells changed, and duplicate items removed."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dedup-within-cell",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove duplicate items inside delimited CSV cells",
    skill(
        description = "Remove repeated items inside selected CSV/TSV cells while preserving rows and columns. Paste table text in `data`; each selected cell is split by `item_separator`, deduped, optionally case-folded and sorted, then joined back with the original spacing or `output_separator`. Use `columns` to target header names or 1-based indices, `delimiter` for the table delimiter, and `output=stats` for a removal report.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dedup-within-cell", |a: Args| {
            dedupe_within_cells(
                &a.data,
                &a.columns,
                &a.item_separator,
                &a.output_separator,
                a.ignore_case,
                a.trim_items,
                a.drop_empty,
                &a.sort_items,
                &a.delimiter,
                a.has_header,
                &a.output,
            )
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
                    "data":             { "type": "string", "description": "CSV/TSV/table text to clean. Each selected cell is treated as a delimited list, e.g. `a, b, a, c` becomes `a, b, c`. A single-column input works too (one list per line). Maximum 1 MB." },
                    "columns":          { "type": "string", "default": "", "description": "Comma-separated 1-based column numbers or header names to process, e.g. `2,tags`. Leave blank to process every column. Header names require has_header=true." },
                    "item_separator":   { "type": "string", "enum": ["comma", "comma-space", "semicolon", "pipe", "space", "tab", "newline"], "default": "comma", "description": "Separator between items inside each cell. Use comma for `a,b,a`, comma-space for `a, b, a`, semicolon, pipe, space, tab, or newline." },
                    "output_separator": { "type": "string", "default": "", "description": "Separator used to join the kept items. Leave blank to preserve the cell's original separator and spacing (for example comma-space stays comma-space)." },
                    "ignore_case":      { "type": "boolean", "default": false, "description": "Treat differently-cased items as duplicates while keeping the first occurrence's original casing." },
                    "trim_items":       { "type": "boolean", "default": true, "description": "Trim whitespace around each item before comparing and writing it back. Enabled by default." },
                    "drop_empty":       { "type": "boolean", "default": true, "description": "Drop empty items created by repeated separators, e.g. `a,,b,,,a` -> `a,b`. Enabled by default." },
                    "sort_items":       { "type": "string", "enum": ["none", "asc", "desc"], "default": "none", "description": "Order for the surviving items inside each cell: none keeps first-seen order, asc sorts A-Z, desc sorts Z-A." },
                    "delimiter":        { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Delimiter between table fields: comma for CSV (default), tab for TSV, semicolon, or pipe. This is not the same as item_separator inside a cell." },
                    "has_header":       { "type": "boolean", "default": true, "description": "Treat the first row as a header. Header rows are never rewritten, and column names in `columns` are resolved from it." },
                    "output":           { "type": "string", "enum": ["csv", "stats"], "default": "csv", "description": "Return the rewritten table as csv (default), or stats for a plain-text count of rows, cells scanned, cells changed, and duplicate items removed." }
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
