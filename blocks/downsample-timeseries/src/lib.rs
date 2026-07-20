//! gizza-ai/downsample-timeseries — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_downsample_timeseries_core::downsample;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_points")]
    points: u64,
    #[serde(default)]
    x_column: String,
    #[serde(default)]
    y_column: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_output")]
    output: String,
}
fn default_algorithm() -> String { "lttb".into() }
fn default_points() -> u64 { 100 }
fn default_true() -> bool { true }
fn default_output() -> String { "points".into() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The time-series to downsample: CSV text (comma, tab, or semicolon delimited; optional header; one value per line also works) or a JSON array of numbers, [x, y] pairs, or objects with time/value keys. Max 2,000,000 bytes."))
        .param(Param::integer("points").default(100).min(2.0).max(100000.0).describe("Target number of points to keep (2-100000). lttb and nth return exactly this many; minmax/m4 round down to whole buckets. A series already at or below this count is returned unchanged. Default 100."))
        .param(Param::enumv("algorithm", ["lttb", "minmax", "m4", "nth"]).default("lttb").describe("Downsampling algorithm: lttb (Largest-Triangle-Three-Buckets, best visual shape preservation), minmax (min + max per bucket, keeps spike envelopes), m4 (first/min/max/last per bucket; needs points >= 4), or nth (uniform every-n-th point incl. both endpoints). Default lttb."))
        .param(Param::string("x_column").default("").describe("Which column/key holds x (time): a header name, a 1-based column number, or 'index' to use the row number. Values may be numbers or ISO-8601 dates/times and must be sorted ascending. Blank = the first column when there are 2+ columns, else the row index."))
        .param(Param::string("y_column").default("").describe("Which column/key holds the y value (must be numeric): a header name or a 1-based column number. Blank = the second column when there are 2+ columns, else the only column."))
        .param(Param::boolean("header").default(true).describe("Treat a non-numeric first CSV row as a header (kept in the output). A fully numeric first row is always data. Set false to force the first row to be data. Default true."))
        .param(Param::enumv("output", ["points", "indices"]).default("points").describe("What to return: points = the selected rows/elements verbatim in the input's own format (all columns kept, header preserved), or indices = a JSON array of the 0-based data-row indices that were kept. Default points."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct DownsampleTimeseries;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/downsample-timeseries",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Downsample a time-series to N points with LTTB, min/max, M4, or every-nth while preserving its shape",
    skill(
        description = "Reduce a large time-series (CSV text or a JSON array) to a target number of `points` while preserving its visual shape. `algorithm` is lttb (Largest-Triangle-Three-Buckets, default — best shape preservation), minmax (min + max per bucket, keeps spikes), m4 (first/min/max/last per bucket), or nth (uniform every-n-th point). Every algorithm selects original points — no interpolation — so rows come back verbatim with all columns and the header preserved. x may be numbers or ISO-8601 timestamps (sorted ascending); pick columns with `x_column`/`y_column`. `output` returns the points themselves or the kept 0-based indices.",
        parameters = schema_json()
    ),
)]
impl DownsampleTimeseries {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "downsample-timeseries", |a: Args| {
            downsample(
                &a.data,
                &a.algorithm,
                a.points as usize,
                &a.x_column,
                &a.y_column,
                a.header,
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
                    "data":      { "type": "string", "description": "The time-series to downsample: CSV text (comma, tab, or semicolon delimited; optional header; one value per line also works) or a JSON array of numbers, [x, y] pairs, or objects with time/value keys. Max 2,000,000 bytes." },
                    "points":    { "type": "integer", "default": 100, "minimum": 2, "maximum": 100000, "description": "Target number of points to keep (2-100000). lttb and nth return exactly this many; minmax/m4 round down to whole buckets. A series already at or below this count is returned unchanged. Default 100." },
                    "algorithm": { "type": "string", "enum": ["lttb", "minmax", "m4", "nth"], "default": "lttb", "description": "Downsampling algorithm: lttb (Largest-Triangle-Three-Buckets, best visual shape preservation), minmax (min + max per bucket, keeps spike envelopes), m4 (first/min/max/last per bucket; needs points >= 4), or nth (uniform every-n-th point incl. both endpoints). Default lttb." },
                    "x_column":  { "type": "string", "default": "", "description": "Which column/key holds x (time): a header name, a 1-based column number, or 'index' to use the row number. Values may be numbers or ISO-8601 dates/times and must be sorted ascending. Blank = the first column when there are 2+ columns, else the row index." },
                    "y_column":  { "type": "string", "default": "", "description": "Which column/key holds the y value (must be numeric): a header name or a 1-based column number. Blank = the second column when there are 2+ columns, else the only column." },
                    "header":    { "type": "boolean", "default": true, "description": "Treat a non-numeric first CSV row as a header (kept in the output). A fully numeric first row is always data. Set false to force the first row to be data. Default true." },
                    "output":    { "type": "string", "enum": ["points", "indices"], "default": "points", "description": "What to return: points = the selected rows/elements verbatim in the input's own format (all columns kept, header preserved), or indices = a JSON array of the 0-based data-row indices that were kept. Default points." }
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
