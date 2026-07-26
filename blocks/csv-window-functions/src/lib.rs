//! gizza-ai/csv-window-functions — SQL-style window functions over CSV rows
//! WITHOUT collapsing them: running total, moving average, lag/lead, and
//! rank/dense_rank/row_number, each within partitions and an optional order.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_function() -> String {
    "running_total".into()
}
fn default_window() -> i64 {
    3
}
fn default_offset() -> i64 {
    1
}
fn default_true() -> bool {
    true
}
fn default_delimiter() -> String {
    ",".into()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_function")]
    function: String,
    #[serde(default)]
    column: String,
    #[serde(default)]
    partition_by: String,
    #[serde(default)]
    order_by: String,
    #[serde(default = "default_window")]
    window: i64,
    #[serde(default = "default_offset")]
    offset: i64,
    #[serde(default)]
    output_column: String,
    #[serde(default)]
    descending: bool,
    #[serde(default = "default_true")]
    has_header: bool,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().multiline().describe("Input CSV text. The first row must be a header row; every input row is preserved and one result column is appended."))
        .param(Param::enumv("function", ["running_total", "moving_average", "lag", "lead", "rank", "dense_rank", "row_number"]).default("running_total").describe("Window function to apply: running_total (cumulative sum), moving_average (trailing mean over `window` rows), lag/lead (value `offset` rows before/after), rank/dense_rank (position by value, ties share a rank), or row_number (1-based position). Default running_total."))
        .param(Param::string("column").describe("Value column (header name or 1-based index) the function reads. Required for every function except row_number."))
        .param(Param::string("partition_by").describe("Comma-separated columns to partition by. Each distinct combination is an independent window. Empty means one window over all rows."))
        .param(Param::string("order_by").describe("Comma-separated columns to sort rows by within each partition before computing. Empty means input order."))
        .param(Param::integer("window").min(1.0).max(1000000.0).default(3).describe("moving_average only: number of trailing rows in the frame, including the current row. Default 3."))
        .param(Param::integer("offset").min(0.0).max(1000000.0).default(1).describe("lag/lead only: how many rows before (lag) or after (lead) the current row to read. Default 1."))
        .param(Param::string("output_column").describe("Name of the appended result column. Empty picks a sensible default such as running_total_<column> or rank."))
        .param(Param::boolean("descending").default(false).describe("Reverse the order_by sort, and rank largest value first. Default false."))
        .param(Param::boolean("has_header").default(true).describe("Treat the first CSV row as a header row. This tool requires a header row, so leave this on. Default true."))
        .param(Param::enumv("delimiter", [",", "tab", ";", "|"]).default(",").describe("CSV field delimiter for both input and output: comma (default), tab, semicolon, or pipe."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvWindowFunctions;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-window-functions",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add SQL window-function columns (running total, moving average, lag/lead, rank) to CSV rows.",
    skill(
        description = "Compute SQL-style window functions over CSV rows without collapsing them, appending one result column to every row. Supports running_total (cumulative sum), moving_average (trailing mean), lag and lead, rank, dense_rank, and row_number. Evaluate within partitions (partition_by) and an optional in-partition sort (order_by), choose the value column by header name or 1-based index, set the moving-average window or lag/lead offset, name the output column, rank ascending or descending, and pick comma/tab/semicolon/pipe delimiters. Returns CSV text grouped by partition. Requires a header row.",
        parameters = schema_json()
    ),
)]
impl CsvWindowFunctions {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-window-functions", |a: Args| {
            gizza_ai_csv_window_functions_core::window(
                &a.data,
                &a.function,
                &a.column,
                &a.partition_by,
                &a.order_by,
                a.window,
                a.offset,
                &a.output_column,
                a.descending,
                a.has_header,
                &a.delimiter,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "data":{"type":"string","description":"Input CSV text. The first row must be a header row; every input row is preserved and one result column is appended."},
            "function":{"type":"string","enum":["running_total","moving_average","lag","lead","rank","dense_rank","row_number"],"default":"running_total","description":"Window function to apply: running_total (cumulative sum), moving_average (trailing mean over `window` rows), lag/lead (value `offset` rows before/after), rank/dense_rank (position by value, ties share a rank), or row_number (1-based position). Default running_total."},
            "column":{"type":"string","description":"Value column (header name or 1-based index) the function reads. Required for every function except row_number."},
            "partition_by":{"type":"string","description":"Comma-separated columns to partition by. Each distinct combination is an independent window. Empty means one window over all rows."},
            "order_by":{"type":"string","description":"Comma-separated columns to sort rows by within each partition before computing. Empty means input order."},
            "window":{"type":"integer","minimum":1,"maximum":1000000,"default":3,"description":"moving_average only: number of trailing rows in the frame, including the current row. Default 3."},
            "offset":{"type":"integer","minimum":0,"maximum":1000000,"default":1,"description":"lag/lead only: how many rows before (lag) or after (lead) the current row to read. Default 1."},
            "output_column":{"type":"string","description":"Name of the appended result column. Empty picks a sensible default such as running_total_<column> or rank."},
            "descending":{"type":"boolean","default":false,"description":"Reverse the order_by sort, and rank largest value first. Default false."},
            "has_header":{"type":"boolean","default":true,"description":"Treat the first CSV row as a header row. This tool requires a header row, so leave this on. Default true."},
            "delimiter":{"type":"string","enum":[",","tab",";","|"],"default":",","description":"CSV field delimiter for both input and output: comma (default), tab, semicolon, or pipe."}
          },"required":["data"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
