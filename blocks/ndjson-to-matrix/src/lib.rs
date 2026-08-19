//! gizza-ai/ndjson-to-matrix — chat skill block on the shared tool abstraction.
//! The descriptor is the single source for the chat schema, CLI, and generated
//! page controls; handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_arrays")]
    arrays: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_order")]
    column_order: String,
    #[serde(default)]
    fill: String,
    #[serde(default = "default_true")]
    headers: bool,
    #[serde(default)]
    row_index: bool,
    #[serde(default)]
    numeric_only: bool,
    #[serde(default)]
    transpose: bool,
    #[serde(default)]
    max_depth: i64,
    #[serde(default)]
    limit: i64,
    #[serde(default = "default_invalid")]
    invalid: String,
}

fn default_format() -> String {
    "csv".to_string()
}
fn default_delimiter() -> String {
    "comma".to_string()
}
fn default_separator() -> String {
    ".".to_string()
}
fn default_arrays() -> String {
    "index".to_string()
}
fn default_order() -> String {
    "first-seen".to_string()
}
fn default_invalid() -> String {
    "error".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("NDJSON / JSON Lines text: one complete JSON value per non-blank line, e.g. {\"id\":1,\"latency_ms\":12}. Objects become rows keyed by their flattened paths, a bare JSON array line becomes positional columns 0,1,2…, and a bare scalar line lands in a single 'value' column. CRLF endings and a leading byte-order mark are handled. Up to 5000000 bytes and 50000 non-blank lines per run."),
        )
        .param(
            Param::enumv("format", ["csv", "tsv", "matrix", "json"])
                .default("csv")
                .describe("Output shape: 'csv' (RFC 4180, default), 'tsv' (tab-separated), 'matrix' (whitespace-aligned grid with numeric columns right-aligned) or 'json' (an array of row arrays with numeric cells as JSON numbers)."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe", "space"])
                .default("comma")
                .describe("Field separator for format='csv'. 'comma' is the default; 'semicolon' suits spreadsheets in comma-decimal locales. Ignored by the tsv, matrix and json formats."),
        )
        .param(
            Param::string("separator")
                .default(".")
                .describe("Joiner used to build nested column paths: '.' (default) gives user.geo.lat, '_' gives user_geo_lat for SQL-friendly headers, '/' also works."),
        )
        .param(
            Param::enumv("arrays", ["index", "json", "skip"])
                .default("index")
                .describe("How nested arrays become columns: 'index' (default) expands [1,2] into v.0 and v.1 so fixed-length vectors line up, 'json' keeps the whole array as compact JSON in one cell, 'skip' drops array columns entirely."),
        )
        .param(
            Param::string("columns")
                .describe("Comma-separated column paths to keep, in exactly that order, e.g. 'latency_ms, user.id'. Empty (default) keeps every discovered column and uses column_order. Naming a path that does not exist fails with the available list."),
        )
        .param(
            Param::enumv("column_order", ["first-seen", "alpha", "coverage"])
                .default("first-seen")
                .describe("Column ordering when 'columns' is empty: 'first-seen' (default) keeps the order the records wrote the keys, 'alpha' sorts paths alphabetically for stable diffs, 'coverage' puts the most-populated columns first."),
        )
        .param(
            Param::string("fill")
                .describe("Text written into cells whose record lacks that path, and into JSON null cells. Empty (default) leaves them blank; '0' or 'NaN' make the result loadable as a dense numeric matrix."),
        )
        .param(
            Param::boolean("headers")
                .default(true)
                .describe("Emit the header row of column paths. Turn it off for a bare numeric grid that numpy.loadtxt or a matrix import can read directly."),
        )
        .param(
            Param::boolean("row_index")
                .default(false)
                .describe("Prepend a 1-based 'row' column so each record has a label even when the data carries no id field."),
        )
        .param(
            Param::boolean("numeric_only")
                .default(false)
                .describe("Keep only columns whose present values are all finite numbers (numeric-looking strings count), dropping ids, labels and free text — the one-click way to get a clean numeric matrix out of a mixed log stream."),
        )
        .param(
            Param::boolean("transpose")
                .default(false)
                .describe("Swap axes: emit one row per column and one column per record. With headers on, the first cell of each row is the column path and the header row numbers the records."),
        )
        .param(
            Param::integer("max_depth")
                .min(0.0)
                .max(50.0)
                .default(0)
                .describe("Flatten only this many levels of nesting; anything deeper is written as compact JSON in one cell. 0 (default) flattens all the way down. Use 1-2 to stop a deep payload from exploding into hundreds of columns."),
        )
        .param(
            Param::integer("limit")
                .min(0.0)
                .default(0)
                .describe("Keep only the first N records, which is handy for previewing a huge stream. 0 (default) converts every record."),
        )
        .param(
            Param::enumv("invalid", ["error", "skip"])
                .default("error")
                .describe("Unparsable lines: 'error' (default) stops and reports the line number and column, 'skip' drops them and converts the rest."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct NdjsonToMatrix;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ndjson-to-matrix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert NDJSON records into an aligned 2D matrix, CSV, TSV or JSON table.",
    skill(
        description = "Convert NDJSON / JSON Lines records into one aligned, rectangular table. Every non-blank line is parsed on its own and flattened into dotted column paths (user.geo.lat), the column set is the UNION of the paths seen across all records, and missing cells take a chosen fill — so heterogeneous records still produce a dense matrix. format picks csv (RFC 4180, default), tsv, matrix (whitespace-aligned grid, numeric columns right-aligned) or json (array of row arrays with real JSON numbers). arrays='index' turns [1,2] into v.0/v.1 columns so fixed-length numeric vectors align, 'json' keeps an array as one cell, 'skip' drops it. separator sets the path joiner ('.' default, '_' for SQL-friendly headers) and max_depth caps flattening depth, writing anything deeper as compact JSON. columns selects and orders an explicit subset; otherwise column_order sorts first-seen, alphabetically, or by coverage. fill='0'/'NaN' plus headers=false yields a bare numeric grid for numpy/R; numeric_only drops every column that is not all-numeric; transpose swaps records and columns; row_index adds a 1-based row label; limit previews the first N records. Unparsable lines either stop the run with their line number and column or are skipped. Pure and deterministic — text in, text out, nothing leaves the device.",
        parameters = schema_json()
    ),
)]
impl NdjsonToMatrix {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ndjson-to-matrix", |a: Args| {
            gizza_ai_ndjson_to_matrix_core::run(
                &a.data,
                &a.format,
                &a.delimiter,
                &a.separator,
                &a.arrays,
                &a.columns,
                &a.column_order,
                &a.fill,
                a.headers,
                a.row_index,
                a.numeric_only,
                a.transpose,
                a.max_depth,
                a.limit,
                &a.invalid,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert_eq!(schema.get("type").unwrap(), "object");
        assert_eq!(schema.get("additionalProperties").unwrap(), false);
        assert_eq!(schema.get("required").unwrap(), &serde_json::json!(["data"]));
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["csv", "tsv", "matrix", "json"])
        );
        assert_eq!(
            props["delimiter"]["enum"],
            serde_json::json!(["comma", "tab", "semicolon", "pipe", "space"])
        );
        assert_eq!(
            props["arrays"]["enum"],
            serde_json::json!(["index", "json", "skip"])
        );
        assert_eq!(
            props["column_order"]["enum"],
            serde_json::json!(["first-seen", "alpha", "coverage"])
        );
        assert_eq!(props["invalid"]["enum"], serde_json::json!(["error", "skip"]));
        assert_eq!(props["format"]["default"], "csv");
        assert_eq!(props["delimiter"]["default"], "comma");
        assert_eq!(props["separator"]["default"], ".");
        assert_eq!(props["arrays"]["default"], "index");
        assert_eq!(props["column_order"]["default"], "first-seen");
        assert_eq!(props["invalid"]["default"], "error");
        assert_eq!(props["headers"]["default"], true);
        assert_eq!(props["row_index"]["default"], false);
        assert_eq!(props["numeric_only"]["default"], false);
        assert_eq!(props["transpose"]["default"], false);
        assert_eq!(props["max_depth"]["default"], 0);
        assert_eq!(props["limit"]["default"], 0);
        for key in [
            "data",
            "format",
            "delimiter",
            "separator",
            "arrays",
            "columns",
            "column_order",
            "fill",
            "headers",
            "row_index",
            "numeric_only",
            "transpose",
            "max_depth",
            "limit",
            "invalid",
        ] {
            assert!(
                props[key]["description"].as_str().unwrap().len() > 20,
                "missing .describe() for {key}"
            );
        }
    }
}
