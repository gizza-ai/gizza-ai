//! gizza-ai/data-bin — bin ONE numeric column of a CSV into equal-width,
//! quantile (equal-frequency), or custom-edge buckets and label each row.
//! Thin wrapper around the core; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_bin_core::bin;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    column: String,
    #[serde(default = "default_bins")]
    bins: u32,
    #[serde(default)]
    edges: String,
    #[serde(default)]
    labels: String,
    #[serde(default = "default_label_style")]
    label_style: String,
    #[serde(default = "default_true")]
    right: bool,
    #[serde(default = "default_precision")]
    precision: u32,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}
fn default_true() -> bool { true }
fn default_method() -> String { "equal_width".to_string() }
fn default_label_style() -> String { "range".to_string() }
fn default_output() -> String { "append".to_string() }
fn default_delimiter() -> String { "comma".to_string() }
fn default_bins() -> u32 { 4 }
fn default_precision() -> u32 { 3 }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The CSV text whose numeric column should be binned."))
        .param(Param::enumv("method", ["equal_width", "quantile", "custom"]).default("equal_width").describe("Binning method: 'equal_width' splits the value range into equal-width buckets; 'quantile' makes equal-frequency buckets (each holds ~the same number of rows); 'custom' uses your own 'edges'. Default 'equal_width'."))
        .param(Param::string("column").required().describe("The single numeric column to bin: a header name (needs a header) or a 1-based index (e.g. 'score' or '2'). Every present value in it must parse as a finite number."))
        .param(Param::integer("bins").default(4).min(1.0).max(1000.0).describe("Number of buckets for method 'equal_width' or 'quantile' (ignored for 'custom'). Default 4 (quartiles)."))
        .param(Param::string("edges").describe("For method='custom': comma-separated strictly ascending bucket edges (e.g. '0,18,65,120'). Values below the first or above the last edge get a blank label."))
        .param(Param::string("labels").describe("Comma-separated custom labels, one per bucket (e.g. 'low,mid,high'). Must match the bucket count; blank auto-generates labels using 'label_style'."))
        .param(Param::enumv("label_style", ["range", "index"]).default("range").describe("Auto-label style when 'labels' is blank: 'range' shows the interval (e.g. '(0, 50]'); 'index' shows the 1-based bucket number. Default 'range'."))
        .param(Param::boolean("right").default(true).describe("Right-closed intervals '(a, b]' (true) or left-closed '[a, b)' (false); the outermost edge is always included. Default true."))
        .param(Param::integer("precision").default(3).min(0.0).max(12.0).describe("Decimal places for the numbers shown in 'range' labels. Default 3."))
        .param(Param::enumv("output", ["append", "replace"]).default("append").describe("'append' adds a new '<column>_bin' column holding the label; 'replace' overwrites the source column with the label. Default 'append'."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it verbatim and use its names for the 'column' selector. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct DataBin;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-bin",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Bin a numeric CSV column into equal-width, quantile, or custom buckets",
    skill(
        description = "Bin (bucket) one numeric column of a CSV and label every row with the bucket it falls in. Methods: 'equal_width' splits the value range into equal-width buckets; 'quantile' makes equal-frequency buckets so each holds roughly the same number of rows (good for skewed data); 'custom' uses your own strictly-ascending 'edges'. Choose the number of buckets with 'bins' (default 4 = quartiles). Labels are custom ('labels', one per bucket) or auto-generated as interval ranges (e.g. '(0, 50]') or 1-based indexes via 'label_style'. 'right' selects right- vs left-closed intervals and 'precision' the decimals in range labels; duplicate quantile edges are merged automatically. 'output' appends a new '<column>_bin' column or replaces the source column. The target column must be numeric (every present value parses as a finite number); blank cells stay blank. Delimiters accept comma/tab/semicolon/pipe.",
        parameters = schema_json()
    ),
)]
impl DataBin {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-bin", |a: Args| {
            let method = if a.method.is_empty() { "equal_width".to_string() } else { a.method };
            let label_style = if a.label_style.is_empty() { "range".to_string() } else { a.label_style };
            let output = if a.output.is_empty() { "append".to_string() } else { a.output };
            let delimiter = if a.delimiter.is_empty() { "comma".to_string() } else { a.delimiter };
            bin(
                &a.input,
                a.header,
                &delimiter,
                &a.column,
                &method,
                a.bins,
                &a.edges,
                &a.labels,
                &label_style,
                a.right,
                a.precision,
                &output,
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
                    "input":       { "type": "string", "description": "The CSV text whose numeric column should be binned." },
                    "method":      { "type": "string", "enum": ["equal_width", "quantile", "custom"], "default": "equal_width", "description": "Binning method: 'equal_width' splits the value range into equal-width buckets; 'quantile' makes equal-frequency buckets (each holds ~the same number of rows); 'custom' uses your own 'edges'. Default 'equal_width'." },
                    "column":      { "type": "string", "description": "The single numeric column to bin: a header name (needs a header) or a 1-based index (e.g. 'score' or '2'). Every present value in it must parse as a finite number." },
                    "bins":        { "type": "integer", "minimum": 1, "maximum": 1000, "default": 4, "description": "Number of buckets for method 'equal_width' or 'quantile' (ignored for 'custom'). Default 4 (quartiles)." },
                    "edges":       { "type": "string", "description": "For method='custom': comma-separated strictly ascending bucket edges (e.g. '0,18,65,120'). Values below the first or above the last edge get a blank label." },
                    "labels":      { "type": "string", "description": "Comma-separated custom labels, one per bucket (e.g. 'low,mid,high'). Must match the bucket count; blank auto-generates labels using 'label_style'." },
                    "label_style": { "type": "string", "enum": ["range", "index"], "default": "range", "description": "Auto-label style when 'labels' is blank: 'range' shows the interval (e.g. '(0, 50]'); 'index' shows the 1-based bucket number. Default 'range'." },
                    "right":       { "type": "boolean", "default": true, "description": "Right-closed intervals '(a, b]' (true) or left-closed '[a, b)' (false); the outermost edge is always included. Default true." },
                    "precision":   { "type": "integer", "minimum": 0, "maximum": 12, "default": 3, "description": "Decimal places for the numbers shown in 'range' labels. Default 3." },
                    "output":      { "type": "string", "enum": ["append", "replace"], "default": "append", "description": "'append' adds a new '<column>_bin' column holding the label; 'replace' overwrites the source column with the label. Default 'append'." },
                    "header":      { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it verbatim and use its names for the 'column' selector. Default true." },
                    "delimiter":   { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator of the input (and output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'." }
                },
                "required": ["input", "column"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
