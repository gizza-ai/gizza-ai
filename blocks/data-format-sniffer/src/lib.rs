//! gizza-ai/data-format-sniffer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_format_sniffer_core::{sniff, Options, MAX_PREVIEW_ROWS, MAX_SAMPLE_LINES};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The data sample to identify: pasted text, or bytes as base64/hex.
    data: String,
    /// How to read `data`: text, base64, or hex.
    #[serde(default = "default_input_form")]
    input_form: String,
    /// Leading lines used for structure analysis.
    #[serde(default = "default_sample_lines")]
    sample_lines: usize,
    /// Extra delimiter characters to try beyond the built-in candidates.
    #[serde(default)]
    extra_delimiters: String,
    /// Lines starting with this prefix are ignored.
    #[serde(default)]
    comment_prefix: String,
    /// Infer per-column types and use them for header detection.
    #[serde(default = "default_true")]
    detect_types: bool,
    /// Parsed rows to include in the preview table.
    #[serde(default = "default_preview_rows")]
    preview_rows: usize,
    /// Output format: report or json.
    #[serde(default = "default_output")]
    output: String,
}

fn default_input_form() -> String {
    "text".into()
}
fn default_sample_lines() -> usize {
    100
}
fn default_true() -> bool {
    true
}
fn default_preview_rows() -> usize {
    5
}
fn default_output() -> String {
    "report".into()
}

impl From<Args> for Options {
    fn from(a: Args) -> Self {
        Options {
            input_form: a.input_form,
            sample_lines: a.sample_lines,
            extra_delimiters: a.extra_delimiters,
            comment_prefix: a.comment_prefix,
            detect_types: a.detect_types,
            preview_rows: a.preview_rows,
            output: a.output,
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The data sample to identify. Paste the text itself (CSV, TSV, JSON, JSON Lines, XML, HTML, a Markdown table, fixed-width text, …), or paste raw bytes as base64/hex and set input_form so encodings and binary containers can be detected. Capped at 1 MiB after decoding."),
        )
        .param(
            Param::enumv("input_form", ["text", "base64", "hex"])
                .default("text")
                .describe("How to read data: text (already-decoded characters, the default), base64 (standard or URL-safe, padding optional), or hex (two digits per byte; spaces, colons, dashes and a 0x prefix are ignored). Use base64 or hex to detect the original byte encoding or a binary container such as Parquet, Avro, SQLite, ZIP, gzip or zstd."),
        )
        .param(
            Param::integer("sample_lines")
                .default(100)
                .min(1.0)
                .max(MAX_SAMPLE_LINES as f64)
                .describe("How many leading lines to analyse for structure, delimiter and column types. Default 100, maximum 10000. The whole input is still used for the byte count, line count and whole-document JSON check."),
        )
        .param(
            Param::string("extra_delimiters")
                .default("")
                .describe("Extra single-character delimiters to try in addition to the built-in candidates comma, tab, semicolon, pipe, colon, tilde and space. Pass them with no separator, e.g. ^# for caret and hash. Quote characters and newlines are rejected."),
        )
        .param(
            Param::string("comment_prefix")
                .default("")
                .describe("Ignore sampled lines that start with this prefix, e.g. # or //. Empty by default because a leading # is often real data."),
        )
        .param(
            Param::boolean("detect_types")
                .default(true)
                .describe("Infer a type per column (null, boolean, integer, float, time, date, datetime, text) and use a first-row type mismatch to decide whether a header row is present. Default true."),
        )
        .param(
            Param::integer("preview_rows")
                .default(5)
                .min(0.0)
                .max(MAX_PREVIEW_ROWS as f64)
                .describe("How many parsed rows to show in the preview table. 0 hides the preview. Default 5, maximum 50."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format: report is an aligned human-readable summary, json is a machine-readable object with the same fields (format, confidence, encoding, delimiter, columns, column_types, delimiter_scores, preview, notes). Default report."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-format-sniffer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Identify a data sample's format, encoding, delimiter and columns.",
    skill(
        description = "Identify what a pasted data sample actually is. Detects CSV, TSV, semicolon/pipe/custom-delimited text, JSON, JSON Lines, XML, HTML, Markdown tables, fixed-width text and marker-led YAML, plus binary containers (Parquet, Avro, SQLite, ZIP, gzip, zstd) from their magic bytes when the sample is pasted as base64 or hex. Reports a confidence score, the character encoding (BOM or statistical detection over real bytes), line endings, the winning delimiter with a per-candidate score table, the quote character, whether a header row is present, column count, per-column types and a parsed preview. Returns an aligned report or JSON.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-format-sniffer", |a: Args| {
            let data = a.data.clone();
            let opts: Options = a.into();
            sniff(&data, &opts).map_err(SkillError::InvalidArgs)
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
    fn schema_json_exposes_authored_controls() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(v["required"], serde_json::json!(["data"]));
        let p = &v["properties"];
        assert_eq!(
            p["input_form"]["enum"],
            serde_json::json!(["text", "base64", "hex"])
        );
        assert_eq!(p["input_form"]["default"], "text");
        assert_eq!(p["output"]["enum"], serde_json::json!(["report", "json"]));
        assert_eq!(p["output"]["default"], "report");
        assert_eq!(p["sample_lines"]["default"], 100);
        assert_eq!(p["sample_lines"]["minimum"], 1.0);
        assert_eq!(p["sample_lines"]["maximum"], 10000.0);
        assert_eq!(p["preview_rows"]["default"], 5);
        assert_eq!(p["preview_rows"]["minimum"], 0.0);
        assert_eq!(p["preview_rows"]["maximum"], 50.0);
        assert_eq!(p["detect_types"]["default"], true);
        assert_eq!(p["extra_delimiters"]["default"], "");
        assert_eq!(p["comment_prefix"]["default"], "");
        assert!(p["data"]["description"]
            .as_str()
            .unwrap()
            .contains("base64"));
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"data":"a,b\n1,2"}"#).unwrap();
        let o: Options = a.into();
        assert_eq!(o.input_form, "text");
        assert_eq!(o.sample_lines, 100);
        assert_eq!(o.preview_rows, 5);
        assert!(o.detect_types);
        assert_eq!(o.output, "report");
    }
}
