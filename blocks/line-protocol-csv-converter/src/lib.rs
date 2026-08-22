//! gizza-ai/line-protocol-csv-converter — converts between InfluxDB line
//! protocol and CSV in both directions.
//!
//! Thin chat-skill wrapper around `gizza-ai-line-protocol-csv-converter-core`.
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI + page query-params); the handler delegates to
//! `block_utils::run_skill`. No host calls — runs entirely inside the WASM
//! sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_line_protocol_csv_converter_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    csv_layout: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    timestamp_format: String,
    #[serde(default)]
    precision: String,
    #[serde(default)]
    emit_annotations: bool,
    #[serde(default)]
    measurement: String,
    #[serde(default)]
    tag_columns: String,
    #[serde(default)]
    field_columns: String,
    #[serde(default)]
    time_column: String,
    #[serde(default)]
    number_type: String,
    #[serde(default = "default_true")]
    sort_keys: bool,
    #[serde(default)]
    on_error: String,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The text to convert: either InfluxDB line protocol (e.g. `cpu,host=host1 usage=64.23 1577836800000000000`) or CSV with a header row. Blank lines and `#` comment lines in line protocol are skipped."),
        )
        .param(
            Param::enumv("direction", ["auto", "lp-to-csv", "csv-to-lp"])
                .default("auto")
                .describe("Which way to convert. 'auto' (default) detects the input: a `#datatype`/`#constant` annotation row or a header row that does not parse as line protocol means CSV, otherwise line protocol. Force it with 'lp-to-csv' or 'csv-to-lp'."),
        )
        .param(
            Param::enumv("csv_layout", ["wide", "long"])
                .default("wide")
                .describe("Shape of the CSV when converting line protocol to CSV. 'wide' (default) writes one row per point with a column for every distinct tag and field key -- best for spreadsheets. 'long' writes one row per field value with `field` and `value` columns -- best for heterogeneous data. Ignored for csv-to-lp."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("CSV field delimiter, used for reading and writing CSV. Default 'comma'. A `sep=;` first line in the input overrides this, matching InfluxDB's csv2lp."),
        )
        .param(
            Param::enumv(
                "timestamp_format",
                ["rfc3339", "unix_ns", "unix_us", "unix_ms", "unix_s"],
            )
            .default("rfc3339")
            .describe("How the `time` column is written when converting line protocol to CSV. 'rfc3339' (default) writes 2020-01-01T00:00:00Z; the unix_* choices write a Unix integer in nanoseconds/microseconds/milliseconds/seconds. Ignored for csv-to-lp, which always accepts both forms."),
        )
        .param(
            Param::enumv("precision", ["ns", "us", "ms", "s"])
                .default("ns")
                .describe("Unit of NUMERIC timestamps on the line protocol side, both read and written. Default 'ns' (nanoseconds), which is what InfluxDB assumes. Set 's' if your line protocol carries Unix seconds. Also the unit used to read bare numeric timestamps out of CSV."),
        )
        .param(
            Param::boolean("emit_annotations")
                .default(false)
                .describe("Prefix the CSV with an InfluxDB `#datatype` annotation row (types inferred from the data) so the result can be written straight back with `influx write --format csv`. Default false. Requires csv_layout=wide."),
        )
        .param(
            Param::string("measurement")
                .default("")
                .describe("CSV to line protocol only: the measurement name. If it matches a CSV column name, that column supplies the measurement per row; otherwise it is used as a literal name for every row. Leave blank to use a `measurement` column, a `#constant measurement,<name>` row, or a `#datatype` measurement column. Example: cpu."),
        )
        .param(
            Param::string("tag_columns")
                .default("")
                .describe("CSV to line protocol only: comma-separated column names to emit as tags, e.g. `host,region`. Ignored for columns whose type is already set by a `#datatype` row or an inline `name|datatype` header. Rows with an empty tag value omit that tag."),
        )
        .param(
            Param::string("field_columns")
                .default("")
                .describe("CSV to line protocol only: comma-separated column names to emit as fields, e.g. `usage,free`. Leave blank to treat every column that is not the measurement, a tag or the time column as a field. When set, unlisted columns are ignored."),
        )
        .param(
            Param::string("time_column")
                .default("")
                .describe("CSV to line protocol only: the column holding the timestamp, e.g. `time`. Values may be RFC3339 (2020-01-01T00:00:00Z), `YYYY-MM-DD[ T]HH:MM:SS`, `YYYY-MM-DD`, or a Unix integer in the chosen precision. Leave blank to auto-detect a column named time, _time, timestamp, date or datetime; with no time column the points carry no timestamp and InfluxDB stamps them on write."),
        )
        .param(
            Param::enumv("number_type", ["float", "integer"])
                .default("float")
                .describe("CSV to line protocol only: how a cell that looks like a whole number is typed when no datatype is declared for its column. 'float' (default) emits `7`, a line protocol float; 'integer' emits `7i`. Cells with a decimal point or exponent are always floats. A `#datatype long` column always wins."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(true)
                .describe("Sort tag keys alphabetically in the output (and sort the tag/field columns in the CSV). Default true -- InfluxDB recommends sorted tag keys for write performance. Turn off to keep the order the input used."),
        )
        .param(
            Param::enumv("on_error", ["stop", "skip"])
                .default("stop")
                .describe("What to do with a row or line that cannot be parsed. 'stop' (default) fails with the 1-based line number and what was expected. 'skip' drops the bad rows and converts the rest -- useful for dirty exports, but the dropped rows are not reported."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/line-protocol-csv-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between InfluxDB line protocol and CSV in both directions.",
    skill(
        description = "Convert between InfluxDB line protocol and CSV, in both directions. Line protocol in gives a CSV table you can open in a spreadsheet -- either wide (one row per point, one column per tag/field key) or long (one row per field value) -- with timestamps as RFC3339 or Unix integers, and an optional #datatype annotation row so the CSV writes straight back with `influx write --format csv`. CSV in gives line protocol ready to import: column roles come from #datatype/#constant/#default annotation rows, the inline `name|datatype|default` header syntax, or the measurement/tag_columns/field_columns/time_column parameters. Escaping and typing follow the line protocol spec (1i integers, 1u unsigned, quoted strings, bare booleans). Pure and offline: it never contacts an InfluxDB server.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "line-protocol-csv-converter", |a: Args| {
            convert(
                &a.data,
                &a.direction,
                &a.csv_layout,
                &a.delimiter,
                &a.timestamp_format,
                &a.precision,
                a.emit_annotations,
                &a.measurement,
                &a.tag_columns,
                &a.field_columns,
                &a.time_column,
                &a.number_type,
                a.sort_keys,
                &a.on_error,
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
                    "data":             { "type": "string", "description": "The text to convert: either InfluxDB line protocol (e.g. `cpu,host=host1 usage=64.23 1577836800000000000`) or CSV with a header row. Blank lines and `#` comment lines in line protocol are skipped." },
                    "direction":        { "type": "string", "enum": ["auto", "lp-to-csv", "csv-to-lp"], "default": "auto", "description": "Which way to convert. 'auto' (default) detects the input: a `#datatype`/`#constant` annotation row or a header row that does not parse as line protocol means CSV, otherwise line protocol. Force it with 'lp-to-csv' or 'csv-to-lp'." },
                    "csv_layout":       { "type": "string", "enum": ["wide", "long"], "default": "wide", "description": "Shape of the CSV when converting line protocol to CSV. 'wide' (default) writes one row per point with a column for every distinct tag and field key -- best for spreadsheets. 'long' writes one row per field value with `field` and `value` columns -- best for heterogeneous data. Ignored for csv-to-lp." },
                    "delimiter":        { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "CSV field delimiter, used for reading and writing CSV. Default 'comma'. A `sep=;` first line in the input overrides this, matching InfluxDB's csv2lp." },
                    "timestamp_format": { "type": "string", "enum": ["rfc3339", "unix_ns", "unix_us", "unix_ms", "unix_s"], "default": "rfc3339", "description": "How the `time` column is written when converting line protocol to CSV. 'rfc3339' (default) writes 2020-01-01T00:00:00Z; the unix_* choices write a Unix integer in nanoseconds/microseconds/milliseconds/seconds. Ignored for csv-to-lp, which always accepts both forms." },
                    "precision":        { "type": "string", "enum": ["ns", "us", "ms", "s"], "default": "ns", "description": "Unit of NUMERIC timestamps on the line protocol side, both read and written. Default 'ns' (nanoseconds), which is what InfluxDB assumes. Set 's' if your line protocol carries Unix seconds. Also the unit used to read bare numeric timestamps out of CSV." },
                    "emit_annotations": { "type": "boolean", "default": false, "description": "Prefix the CSV with an InfluxDB `#datatype` annotation row (types inferred from the data) so the result can be written straight back with `influx write --format csv`. Default false. Requires csv_layout=wide." },
                    "measurement":      { "type": "string", "default": "", "description": "CSV to line protocol only: the measurement name. If it matches a CSV column name, that column supplies the measurement per row; otherwise it is used as a literal name for every row. Leave blank to use a `measurement` column, a `#constant measurement,<name>` row, or a `#datatype` measurement column. Example: cpu." },
                    "tag_columns":      { "type": "string", "default": "", "description": "CSV to line protocol only: comma-separated column names to emit as tags, e.g. `host,region`. Ignored for columns whose type is already set by a `#datatype` row or an inline `name|datatype` header. Rows with an empty tag value omit that tag." },
                    "field_columns":    { "type": "string", "default": "", "description": "CSV to line protocol only: comma-separated column names to emit as fields, e.g. `usage,free`. Leave blank to treat every column that is not the measurement, a tag or the time column as a field. When set, unlisted columns are ignored." },
                    "time_column":      { "type": "string", "default": "", "description": "CSV to line protocol only: the column holding the timestamp, e.g. `time`. Values may be RFC3339 (2020-01-01T00:00:00Z), `YYYY-MM-DD[ T]HH:MM:SS`, `YYYY-MM-DD`, or a Unix integer in the chosen precision. Leave blank to auto-detect a column named time, _time, timestamp, date or datetime; with no time column the points carry no timestamp and InfluxDB stamps them on write." },
                    "number_type":      { "type": "string", "enum": ["float", "integer"], "default": "float", "description": "CSV to line protocol only: how a cell that looks like a whole number is typed when no datatype is declared for its column. 'float' (default) emits `7`, a line protocol float; 'integer' emits `7i`. Cells with a decimal point or exponent are always floats. A `#datatype long` column always wins." },
                    "sort_keys":        { "type": "boolean", "default": true, "description": "Sort tag keys alphabetically in the output (and sort the tag/field columns in the CSV). Default true -- InfluxDB recommends sorted tag keys for write performance. Turn off to keep the order the input used." },
                    "on_error":         { "type": "string", "enum": ["stop", "skip"], "default": "stop", "description": "What to do with a row or line that cannot be parsed. 'stop' (default) fails with the 1-based line number and what was expected. 'skip' drops the bad rows and converts the rest -- useful for dirty exports, but the dropped rows are not reported." }
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
