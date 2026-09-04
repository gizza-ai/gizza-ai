//! gizza-ai/confusion-matrix — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    actual: String,
    #[serde(default)]
    predicted: String,
    #[serde(default)]
    labels: String,
    #[serde(default)]
    positive_label: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_normalize")]
    normalize: String,
    #[serde(default = "default_beta")]
    beta: f64,
    #[serde(default = "default_decimals")]
    decimals: f64,
    #[serde(default)]
    percent: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_input_format() -> String {
    "auto".to_string()
}
fn default_separator() -> String {
    "auto".to_string()
}
fn default_header() -> String {
    "auto".to_string()
}
fn default_normalize() -> String {
    "none".to_string()
}
fn default_beta() -> f64 {
    1.0
}
fn default_decimals() -> f64 {
    4.0
}
fn default_format() -> String {
    "markdown".to_string()
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("actual").required().multiline().describe("True labels, one per row; or paste a complete actual,predicted[,count] table or a square confusion-matrix count grid when predicted is empty."))
        .param(Param::string("predicted").multiline().describe("Predicted labels, one per row. Leave empty when actual contains a paired table or matrix grid."))
        .param(Param::string("labels").describe("Optional class order, separated by commas, newlines, tabs, semicolons, pipes, or spaces. Unseen labels are included with zero support."))
        .param(Param::string("positive_label").describe("Positive class for the binary summary. When blank and exactly two classes exist, the tool chooses a common positive label or the second class."))
        .param(Param::enumv("input_format", ["auto", "labels", "table", "matrix"]).default("auto").describe("Input shape: auto (default), two separate label lists, an actual/predicted table, or a square confusion-matrix grid of counts."))
        .param(Param::enumv("separator", ["auto", "newline", "comma", "tab", "semicolon", "pipe", "space"]).default("auto").describe("Separator for labels or table columns. Auto accepts newlines, comma, tab, semicolon, pipe, and falls back to whitespace."))
        .param(Param::enumv("header", ["auto", "yes", "no"]).default("auto").describe("Whether the first row is a header. Auto recognizes common actual/predicted header names."))
        .param(Param::enumv("normalize", ["none", "row", "column", "all"]).default("none").describe("Normalize matrix cells by row, by prediction column, or by all observations; none returns raw counts."))
        .param(Param::number("beta").default(1.0).min(0.1).max(10.0).describe("F-beta weight for the report. 1 gives F1; values above 1 weight recall more heavily."))
        .param(Param::integer("decimals").default(4).min(0.0).max(10.0).describe("Decimal places for rates and scores."))
        .param(Param::boolean("percent").default(false).describe("Show proportions as percentages in text, markdown, and CSV output."))
        .param(Param::enumv("format", ["markdown", "text", "csv", "json"]).default("markdown").describe("Output format for the matrix and metrics."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/confusion-matrix",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a confusion matrix and classification metrics",
    skill(
        description = "Build a confusion matrix and classification report from true vs predicted labels. Accepts two label columns, an actual/predicted table with optional counts, or a square matrix grid. Reports per-class precision, recall, F-score, specificity, support, macro/weighted/micro averages, accuracy, balanced accuracy, Cohen's kappa, Matthews correlation, and an expanded binary summary with Wilson confidence intervals when a positive class is available. Supports label ordering, row/column/all normalization, F-beta weighting, decimal precision, percentages, and markdown/text/CSV/JSON output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "confusion-matrix", |a: Args| {
            gizza_ai_confusion_matrix_core::run(
                &a.actual,
                &a.predicted,
                &a.labels,
                &a.positive_label,
                &a.input_format,
                &a.separator,
                &a.header,
                &a.normalize,
                a.beta,
                a.decimals,
                a.percent,
                &a.format,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
