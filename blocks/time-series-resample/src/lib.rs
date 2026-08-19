//! gizza-ai/time-series-resample — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_time_series_resample_core::resample;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default = "default_aggregate")]
    aggregate: String,
    #[serde(default)]
    time_column: String,
    #[serde(default)]
    value_columns: String,
    #[serde(default = "default_label")]
    label: String,
    #[serde(default = "default_closed")]
    closed: String,
    #[serde(default = "default_fill")]
    fill: String,
    #[serde(default = "default_origin")]
    origin: String,
    #[serde(default)]
    offset: String,
    #[serde(default = "default_time_format")]
    time_format: String,
    #[serde(default = "default_output")]
    output: String,
}
fn default_interval() -> String { "1h".into() }
fn default_aggregate() -> String { "mean".into() }
fn default_label() -> String { "start".into() }
fn default_closed() -> String { "left".into() }
fn default_fill() -> String { "skip".into() }
fn default_origin() -> String { "epoch".into() }
fn default_time_format() -> String { "iso".into() }
fn default_output() -> String { "csv".into() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The series to resample: CSV/TSV text with one timestamp column and one or more numeric value columns, e.g. 'time,temp\\n2024-05-01T10:00:00Z,10\\n2024-05-01T10:30:00Z,20'. The delimiter (comma, tab, semicolon, pipe) is auto-detected and reused in the output; a header row is optional; rows need not be sorted. Max 2,000,000 bytes and 200,000 rows."))
        .param(Param::string("interval").default("1h").describe("Bucket width as a number plus a unit: ms, s, m (minute), h, d, w, mo (month), q (quarter) or y (year) — e.g. 15m, 1h, 1d, 1w, 1mo, 1q, 1y. A bare unit means 1. A width FINER than the data upsamples: the empty buckets in between are created and then filled per `fill`. Default 1h."))
        .param(Param::enumv("aggregate", ["mean", "sum", "min", "max", "count", "median", "first", "last", "std", "var", "ohlc"]).default("mean").describe("How to combine the values inside each bucket: mean, sum, min, max, count (rows with a number), median, first/last (chronological), std/var (sample, n-1 denominator; blank for a single-value bucket), or ohlc, which expands every value column into <col>_open, <col>_high, <col>_low and <col>_close. Default mean."))
        .param(Param::string("time_column").default("").describe("Which column holds the timestamp: a header name, a case-insensitive header name, or a 1-based column number. Blank = the first column. Values may be ISO-8601/RFC-3339 (2024-05-01T13:20:00Z, with or without an offset), 'YYYY-MM-DD HH:MM', plain 'YYYY-MM-DD', or a bare epoch number (>= 1e11 is read as milliseconds, otherwise seconds)."))
        .param(Param::string("value_columns").default("").describe("Comma-separated list of the value columns to aggregate — header names or 1-based column numbers, e.g. 'temp,humidity' or '2,3'. Blank = every column other than the timestamp whose cells are all numeric or blank. Non-numeric columns are rejected with the offending line and cell."))
        .param(Param::enumv("label", ["start", "end"]).default("start").describe("Which edge of the bucket to print in the timestamp column: start (the bucket's opening instant) or end (the next bucket's opening instant). Default start."))
        .param(Param::enumv("closed", ["left", "right"]).default("left").describe("Which side of a bucket is inclusive. left = [start, end), the common convention, so a row landing exactly on an edge opens the new bucket; right = (start, end], so an exact-edge row closes the previous bucket. Default left."))
        .param(Param::enumv("fill", ["skip", "empty", "zero", "previous", "linear"]).default("skip").describe("What to do with buckets that contain no rows: skip omits them entirely (the only mode that never invents rows), empty emits the bucket with blank values, zero emits 0, previous carries the last known value forward, linear interpolates between the surrounding values. Anything but skip also creates the intermediate buckets when `interval` is finer than the data. Default skip."))
        .param(Param::enumv("origin", ["epoch", "start", "start_day"]).default("epoch").describe("Which instant the bucket grid is anchored to: epoch = the Unix epoch, so edges land on round clock times (weeks start Monday, months/quarters/years on the 1st); start = the first row's exact timestamp; start_day = UTC midnight of the first row's day. Only applies to fixed widths (ms/s/m/h/d/w) — month, quarter and year buckets always start on the 1st. Default epoch."))
        .param(Param::string("offset").default("").describe("Shift every bucket edge by a fixed duration, e.g. '30m', '-5h', '-3d'. This is how you bucket by a local day instead of a UTC one (offset '-5h' gives days starting at 05:00 UTC = midnight UTC-5). Must be a fixed duration (ms, s, m, h, d, w) — calendar units are rejected. Blank = no shift."))
        .param(Param::enumv("time_format", ["iso", "date", "datetime", "epoch_seconds", "epoch_millis"]).default("iso").describe("How to print each bucket's timestamp: iso (2024-05-01T10:00:00Z), date (2024-05-01), datetime (2024-05-01 10:00:00), epoch_seconds, or epoch_millis. Default iso."))
        .param(Param::enumv("output", ["csv", "json"]).default("csv").describe("Result format: csv reuses the input's delimiter and header names, json returns a pretty-printed array of one object per bucket with null for blank values. Default csv."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct TimeSeriesResample;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/time-series-resample",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Resample a timestamped CSV series to a coarser interval with per-bucket aggregation",
    skill(
        description = "Resample a timestamped CSV/TSV series to a different interval — minute to hour, day to week, tick to OHLC candles. `interval` is a number plus a unit (15m, 1h, 1d, 1w, 1mo, 1q, 1y) and `aggregate` combines each bucket's rows (mean, sum, min, max, count, median, first, last, std, var, or ohlc, which expands each value column into open/high/low/close). Every numeric column is aggregated at once unless `value_columns` names some; `time_column` picks the timestamp column (ISO-8601, 'YYYY-MM-DD HH:MM', plain dates, or epoch numbers, in any row order). `fill` decides what happens to empty buckets (skip, empty, zero, previous, linear) — which also makes upsampling to a finer interval work. `label`/`closed` set which edge is printed and which side is inclusive, `origin` and `offset` move the bucket grid (e.g. offset '-5h' for a non-UTC day boundary), and `output` returns CSV or JSON. All timestamps are handled in UTC.",
        parameters = schema_json()
    ),
)]
impl TimeSeriesResample {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "time-series-resample", |a: Args| {
            resample(
                &a.data,
                &a.time_column,
                &a.value_columns,
                &a.interval,
                &a.aggregate,
                &a.label,
                &a.closed,
                &a.fill,
                &a.origin,
                &a.offset,
                &a.time_format,
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
                    "data":          { "type": "string", "description": "The series to resample: CSV/TSV text with one timestamp column and one or more numeric value columns, e.g. 'time,temp\\n2024-05-01T10:00:00Z,10\\n2024-05-01T10:30:00Z,20'. The delimiter (comma, tab, semicolon, pipe) is auto-detected and reused in the output; a header row is optional; rows need not be sorted. Max 2,000,000 bytes and 200,000 rows." },
                    "interval":      { "type": "string", "default": "1h", "description": "Bucket width as a number plus a unit: ms, s, m (minute), h, d, w, mo (month), q (quarter) or y (year) — e.g. 15m, 1h, 1d, 1w, 1mo, 1q, 1y. A bare unit means 1. A width FINER than the data upsamples: the empty buckets in between are created and then filled per `fill`. Default 1h." },
                    "aggregate":     { "type": "string", "enum": ["mean", "sum", "min", "max", "count", "median", "first", "last", "std", "var", "ohlc"], "default": "mean", "description": "How to combine the values inside each bucket: mean, sum, min, max, count (rows with a number), median, first/last (chronological), std/var (sample, n-1 denominator; blank for a single-value bucket), or ohlc, which expands every value column into <col>_open, <col>_high, <col>_low and <col>_close. Default mean." },
                    "time_column":   { "type": "string", "default": "", "description": "Which column holds the timestamp: a header name, a case-insensitive header name, or a 1-based column number. Blank = the first column. Values may be ISO-8601/RFC-3339 (2024-05-01T13:20:00Z, with or without an offset), 'YYYY-MM-DD HH:MM', plain 'YYYY-MM-DD', or a bare epoch number (>= 1e11 is read as milliseconds, otherwise seconds)." },
                    "value_columns": { "type": "string", "default": "", "description": "Comma-separated list of the value columns to aggregate — header names or 1-based column numbers, e.g. 'temp,humidity' or '2,3'. Blank = every column other than the timestamp whose cells are all numeric or blank. Non-numeric columns are rejected with the offending line and cell." },
                    "label":         { "type": "string", "enum": ["start", "end"], "default": "start", "description": "Which edge of the bucket to print in the timestamp column: start (the bucket's opening instant) or end (the next bucket's opening instant). Default start." },
                    "closed":        { "type": "string", "enum": ["left", "right"], "default": "left", "description": "Which side of a bucket is inclusive. left = [start, end), the common convention, so a row landing exactly on an edge opens the new bucket; right = (start, end], so an exact-edge row closes the previous bucket. Default left." },
                    "fill":          { "type": "string", "enum": ["skip", "empty", "zero", "previous", "linear"], "default": "skip", "description": "What to do with buckets that contain no rows: skip omits them entirely (the only mode that never invents rows), empty emits the bucket with blank values, zero emits 0, previous carries the last known value forward, linear interpolates between the surrounding values. Anything but skip also creates the intermediate buckets when `interval` is finer than the data. Default skip." },
                    "origin":        { "type": "string", "enum": ["epoch", "start", "start_day"], "default": "epoch", "description": "Which instant the bucket grid is anchored to: epoch = the Unix epoch, so edges land on round clock times (weeks start Monday, months/quarters/years on the 1st); start = the first row's exact timestamp; start_day = UTC midnight of the first row's day. Only applies to fixed widths (ms/s/m/h/d/w) — month, quarter and year buckets always start on the 1st. Default epoch." },
                    "offset":        { "type": "string", "default": "", "description": "Shift every bucket edge by a fixed duration, e.g. '30m', '-5h', '-3d'. This is how you bucket by a local day instead of a UTC one (offset '-5h' gives days starting at 05:00 UTC = midnight UTC-5). Must be a fixed duration (ms, s, m, h, d, w) — calendar units are rejected. Blank = no shift." },
                    "time_format":   { "type": "string", "enum": ["iso", "date", "datetime", "epoch_seconds", "epoch_millis"], "default": "iso", "description": "How to print each bucket's timestamp: iso (2024-05-01T10:00:00Z), date (2024-05-01), datetime (2024-05-01 10:00:00), epoch_seconds, or epoch_millis. Default iso." },
                    "output":        { "type": "string", "enum": ["csv", "json"], "default": "csv", "description": "Result format: csv reuses the input's delimiter and header names, json returns a pretty-printed array of one object per bucket with null for blank values. Default csv." }
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
