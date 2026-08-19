//! gizza-ai/iqr-outlier-trimmer — drop (or clip/flag) the CSV rows whose chosen
//! numeric column falls outside the Tukey fences `Q1 − k·IQR … Q3 + k·IQR`.
//! Thin wrapper around the core; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_iqr_outlier_trimmer_core::trim;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_k")]
    k: f64,
    #[serde(default = "default_action")]
    action: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_quartile_method")]
    quartile_method: String,
    #[serde(default = "default_match_mode")]
    match_mode: String,
    #[serde(default = "default_non_numeric")]
    non_numeric: String,
}
fn default_true() -> bool { true }
fn default_k() -> f64 { 1.5 }
fn default_action() -> String { "remove".to_string() }
fn default_output() -> String { "csv".to_string() }
fn default_delimiter() -> String { "comma".to_string() }
fn default_quartile_method() -> String { "linear".to_string() }
fn default_match_mode() -> String { "any".to_string() }
fn default_non_numeric() -> String { "keep".to_string() }

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV/TSV text to trim, including the header row when header=true."))
        .param(Param::string("columns").describe("Comma-separated columns to fence — header names (needs a header) or 1-based indexes, e.g. 'price' or '2,3'. Blank analyses every numeric column."))
        .param(Param::number("k").default(1.5).min(0.0).max(5.0).describe("Tukey fence multiplier: a row is an outlier when the cell is below Q1 - k*IQR or above Q3 + k*IQR. 1.5 = the classic mild fence (default), 3 = extreme outliers only; 0 fences at the quartiles themselves."))
        .param(Param::enumv("action", ["remove", "keep", "clip", "flag"]).default("remove").describe("What to do with the outlier rows: 'remove' drops them (default), 'keep' returns ONLY them, 'clip' winsorizes — clamps each out-of-fence cell to its fence and keeps every row, 'flag' appends an 'outlier' column of true/false and drops nothing."))
        .param(Param::enumv("output", ["csv", "report"]).default("csv").describe("'csv' returns the resulting table (default); 'report' returns the per-column Q1/Q3/IQR, both fences, and the outlier/kept row counts and percentages instead."))
        .param(Param::boolean("header").default(true).describe("Treat the first row as a header: keep it verbatim, never fence-test it, and let 'columns' use its names. Default true."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("Field separator of the input (and of the output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'."))
        .param(Param::enumv("quartile_method", ["linear", "exclusive", "inclusive"]).default("linear").describe("Quartile convention: 'linear' interpolates between order statistics (numpy/pandas default, Excel QUARTILE.INC) — the default; 'exclusive' is Moore & McCabe / TI-83 (the median is excluded from both halves when the count is odd); 'inclusive' is Tukey's hinges (the median belongs to both halves)."))
        .param(Param::enumv("match_mode", ["any", "all"]).default("any").describe("With several analysed columns, is a row an outlier when 'any' column is out of fence (default) or only when 'all' of them are?"))
        .param(Param::enumv("non_numeric", ["keep", "remove"]).default("keep").describe("How to treat a blank or non-numeric cell in an analysed column: 'keep' counts it as in-fence (default), 'remove' counts it as an outlier so action=remove drops that row too. Either way such cells are excluded from the quartile maths."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct IqrOutlierTrimmer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/iqr-outlier-trimmer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove the CSV rows outside the IQR (Tukey) fences of a chosen column",
    skill(
        description = "Drop the outlier rows of a CSV/TSV table using Tukey's fences: for each analysed column, compute Q1, Q3 and IQR = Q3 - Q1, then treat a cell below Q1 - k*IQR or above Q3 + k*IQR as an outlier (k defaults to 1.5). Choose the columns by header name or 1-based index (blank = every numeric column), and choose what happens to the flagged rows: remove them, keep only them, clip (winsorize) the offending cells to their fence, or flag them with an extra column. output='report' returns the quartiles, fences and outlier counts instead of the table. Quartiles support the linear (numpy/pandas), exclusive (Moore & McCabe / TI-83) and inclusive (Tukey's hinges) conventions. The header row is preserved, blank/non-numeric cells never enter the quartile maths, and the delimiter of the input is used for the output.",
        parameters = schema_json()
    ),
)]
impl IqrOutlierTrimmer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "iqr-outlier-trimmer", |a: Args| {
            trim(
                &a.data,
                &a.columns,
                a.k,
                &a.action,
                &a.output,
                a.header,
                &a.delimiter,
                &a.quartile_method,
                &a.match_mode,
                &a.non_numeric,
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
                    "data":            { "type": "string", "description": "The CSV/TSV text to trim, including the header row when header=true." },
                    "columns":         { "type": "string", "description": "Comma-separated columns to fence — header names (needs a header) or 1-based indexes, e.g. 'price' or '2,3'. Blank analyses every numeric column." },
                    "k":               { "type": "number", "default": 1.5, "minimum": 0, "maximum": 5, "description": "Tukey fence multiplier: a row is an outlier when the cell is below Q1 - k*IQR or above Q3 + k*IQR. 1.5 = the classic mild fence (default), 3 = extreme outliers only; 0 fences at the quartiles themselves." },
                    "action":          { "type": "string", "enum": ["remove", "keep", "clip", "flag"], "default": "remove", "description": "What to do with the outlier rows: 'remove' drops them (default), 'keep' returns ONLY them, 'clip' winsorizes — clamps each out-of-fence cell to its fence and keeps every row, 'flag' appends an 'outlier' column of true/false and drops nothing." },
                    "output":          { "type": "string", "enum": ["csv", "report"], "default": "csv", "description": "'csv' returns the resulting table (default); 'report' returns the per-column Q1/Q3/IQR, both fences, and the outlier/kept row counts and percentages instead." },
                    "header":          { "type": "boolean", "default": true, "description": "Treat the first row as a header: keep it verbatim, never fence-test it, and let 'columns' use its names. Default true." },
                    "delimiter":       { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "Field separator of the input (and of the output): 'comma', 'tab', 'semicolon', or 'pipe'. Default 'comma'." },
                    "quartile_method": { "type": "string", "enum": ["linear", "exclusive", "inclusive"], "default": "linear", "description": "Quartile convention: 'linear' interpolates between order statistics (numpy/pandas default, Excel QUARTILE.INC) — the default; 'exclusive' is Moore & McCabe / TI-83 (the median is excluded from both halves when the count is odd); 'inclusive' is Tukey's hinges (the median belongs to both halves)." },
                    "match_mode":      { "type": "string", "enum": ["any", "all"], "default": "any", "description": "With several analysed columns, is a row an outlier when 'any' column is out of fence (default) or only when 'all' of them are?" },
                    "non_numeric":     { "type": "string", "enum": ["keep", "remove"], "default": "keep", "description": "How to treat a blank or non-numeric cell in an analysed column: 'keep' counts it as in-fence (default), 'remove' counts it as an outlier so action=remove drops that row too. Either way such cells are excluded from the quartile maths." }
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
